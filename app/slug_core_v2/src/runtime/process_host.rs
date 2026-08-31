// The request-projection packet will make these process-owner observations
// reachable; this packet establishes their retained state and exact timing.
#![allow(dead_code)]

use std::panic::AssertUnwindSafe;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::MutexGuard;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::thread::ThreadId;

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceError {
    Unsupported,
    Retryable,
    ReadError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PropertyRead {
    Present(Arc<[u16]>),
    Absent,
    ReadError(SourceError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Property {
    BlazeOs,
    OsName,
    OsArch,
    UserHome,
    BazelSh,
    Path,
    SystemRoot,
}

trait ProcessHostSource: Send + Sync {
    fn property(&self, property: Property) -> PropertyRead;
    fn memory_bytes(&self) -> Result<i64, SourceError>;
    fn processors(&self) -> Result<i32, SourceError>;
    fn after_resource(&self) -> Result<(), SourceError>;
}

#[cfg(test)]
struct UnsupportedSource;

#[cfg(test)]
impl ProcessHostSource for UnsupportedSource {
    fn property(&self, _: Property) -> PropertyRead {
        PropertyRead::ReadError(SourceError::Unsupported)
    }

    fn memory_bytes(&self) -> Result<i64, SourceError> {
        Err(SourceError::Unsupported)
    }

    fn processors(&self) -> Result<i32, SourceError> {
        Err(SourceError::Unsupported)
    }

    fn after_resource(&self) -> Result<(), SourceError> {
        Err(SourceError::Unsupported)
    }
}

/// Rust-native process observations for production runtimes.
///
/// The source deliberately performs no observation at construction time. The
/// owner above decides when each fact is demanded and latches only the facts
/// whose Bazel-compatible conversion has class-initialization timing. Home is
/// kept fresh because eligible conversions read it per use.
struct NativeSource;

impl ProcessHostSource for NativeSource {
    fn property(&self, property: Property) -> PropertyRead {
        match property {
            // There is no Rust-native equivalent of a caller-supplied
            // `blaze.os` override. Let the existing fallback consult OsName.
            Property::BlazeOs => PropertyRead::Absent,
            Property::OsName => native_os_name()
                .map(utf16_property)
                .unwrap_or(PropertyRead::Absent),
            Property::OsArch => native_os_arch()
                .map(utf16_property)
                .unwrap_or(PropertyRead::Absent),
            // std::env::var admits only valid Unicode. A non-Unicode or
            // absent value is intentionally not projected into configuration.
            Property::UserHome => native_home()
                .as_deref()
                .map(utf16_property)
                .unwrap_or(PropertyRead::Absent),
            Property::BazelSh => native_environment("BAZEL_SH"),
            Property::Path => native_environment("PATH"),
            Property::SystemRoot => native_environment("SYSTEMROOT"),
        }
    }

    fn memory_bytes(&self) -> Result<i64, SourceError> {
        native_memory_bytes()
    }

    fn processors(&self) -> Result<i32, SourceError> {
        native_processors()
    }

    fn after_resource(&self) -> Result<(), SourceError> {
        Ok(())
    }
}

fn utf16_property(value: &str) -> PropertyRead {
    PropertyRead::Present(Arc::from(value.encode_utf16().collect::<Vec<_>>()))
}

fn native_os_name() -> Option<&'static str> {
    match std::env::consts::OS {
        "linux" => Some("Linux"),
        "windows" => Some("Windows"),
        "macos" => Some("Mac OS X"),
        "freebsd" => Some("FreeBSD"),
        "openbsd" => Some("OpenBSD"),
        _ => None,
    }
}

fn native_os_arch() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86" => Some("x86"),
        "x86_64" => Some("x86_64"),
        "powerpc" | "powerpc64" | "powerpc64le" => Some("ppc"),
        "arm" => Some("arm"),
        "aarch64" => Some("aarch64"),
        "s390x" => Some("s390x"),
        "mips64" | "mips64el" => Some("mips64"),
        "riscv64" => Some("riscv64"),
        _ => None,
    }
}

fn native_home() -> Option<String> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok()
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").ok()
    }
}

fn native_environment(name: &str) -> PropertyRead {
    std::env::var(name)
        .ok()
        .as_deref()
        .map(utf16_property)
        .unwrap_or(PropertyRead::Absent)
}

fn native_processors() -> Result<i32, SourceError> {
    let mut processors = i32::try_from(
        std::thread::available_parallelism()
            .map_err(|_| SourceError::ReadError)?
            .get(),
    )
    .map_err(|_| SourceError::ReadError)?;

    #[cfg(target_os = "linux")]
    for limit in [linux_cgroup_cpu_quota()?, linux_cgroup_cpuset()?]
        .into_iter()
        .flatten()
    {
        processors = processors.min(limit);
    }

    if processors > 0 {
        Ok(processors)
    } else {
        Err(SourceError::ReadError)
    }
}

fn native_memory_bytes() -> Result<i64, SourceError> {
    #[cfg(target_os = "linux")]
    {
        let physical = linux_physical_memory_bytes()?;
        let limit = linux_cgroup_memory_limit()?;
        return Ok(limit.map_or(physical, |limit| physical.min(limit)));
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let pages = i64::try_from(unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) })
            .map_err(|_| SourceError::ReadError)?;
        let page_size = i64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) })
            .map_err(|_| SourceError::ReadError)?;
        return checked_physical_memory_bytes(pages, page_size);
    }

    #[cfg(windows)]
    {
        return windows_physical_memory_bytes();
    }

    #[cfg(not(any(unix, windows)))]
    Err(SourceError::Unsupported)
}

fn checked_physical_memory_bytes(pages: i64, page_size: i64) -> Result<i64, SourceError> {
    if pages <= 0 || page_size <= 0 {
        return Err(SourceError::ReadError);
    }
    pages.checked_mul(page_size).ok_or(SourceError::ReadError)
}

#[cfg(target_os = "linux")]
fn linux_physical_memory_bytes() -> Result<i64, SourceError> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").map_err(|_| SourceError::ReadError)?;
    let kib = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:")?.split_whitespace().next())
        .ok_or(SourceError::ReadError)?
        .parse::<i64>()
        .map_err(|_| SourceError::ReadError)?;
    kib.checked_mul(1024).ok_or(SourceError::ReadError)
}

#[cfg(target_os = "linux")]
fn linux_cgroup_cpu_quota() -> Result<Option<i32>, SourceError> {
    if let Some(paths) = linux_cgroup_v2_ancestors()? {
        let mut limit = None;
        for path in paths {
            if let Some(value) = read_optional(&path.join("cpu.max"))? {
                let mut fields = value.split_whitespace();
                let quota = fields.next().ok_or(SourceError::ReadError)?;
                let period = fields
                    .next()
                    .ok_or(SourceError::ReadError)?
                    .parse::<i64>()
                    .map_err(|_| SourceError::ReadError)?;
                if quota != "max" {
                    limit = minimum_limit(
                        limit,
                        cgroup_cpu_count(
                            quota.parse::<i64>().map_err(|_| SourceError::ReadError)?,
                            period,
                        )?,
                    );
                }
            }
        }
        return Ok(limit);
    }

    let mut limit = None;
    for path in linux_cgroup_v1_ancestors("cpu")?.unwrap_or_default() {
        let quota = read_optional(&path.join("cpu.cfs_quota_us"))?;
        let period = read_optional(&path.join("cpu.cfs_period_us"))?;
        match (quota, period) {
            (Some(quota), Some(period)) => {
                limit = minimum_limit(
                    limit,
                    cgroup_cpu_count(
                        quota.trim().parse().map_err(|_| SourceError::ReadError)?,
                        period.trim().parse().map_err(|_| SourceError::ReadError)?,
                    )?,
                );
            }
            (None, None) => {}
            _ => return Err(SourceError::ReadError),
        }
    }
    Ok(limit)
}

#[cfg(target_os = "linux")]
fn cgroup_cpu_count(quota: i64, period: i64) -> Result<Option<i32>, SourceError> {
    if quota < 0 {
        return Ok(None);
    }
    if quota == 0 || period <= 0 {
        return Err(SourceError::ReadError);
    }
    i32::try_from((quota + period - 1) / period)
        .map(Some)
        .map_err(|_| SourceError::ReadError)
}

#[cfg(target_os = "linux")]
fn linux_cgroup_cpuset() -> Result<Option<i32>, SourceError> {
    if let Some(paths) = linux_cgroup_v2_ancestors()? {
        for path in paths {
            if let Some(value) = read_optional(&path.join("cpuset.cpus.effective"))? {
                return parse_cpuset(&value).map(Some);
            }
        }
        return Ok(None);
    }
    for path in linux_cgroup_v1_ancestors("cpuset")?.unwrap_or_default() {
        if let Some(value) = read_optional(&path.join("cpuset.cpus"))? {
            return parse_cpuset(&value).map(Some);
        }
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
fn linux_cgroup_memory_limit() -> Result<Option<i64>, SourceError> {
    if let Some(paths) = linux_cgroup_v2_ancestors()? {
        let mut limit = None;
        for path in paths {
            if let Some(value) = read_optional(&path.join("memory.max"))? {
                limit = minimum_limit(limit, parse_memory_limit(&value)?);
            }
        }
        return Ok(limit);
    }
    let mut limit = None;
    for path in linux_cgroup_v1_ancestors("memory")?.unwrap_or_default() {
        if let Some(value) = read_optional(&path.join("memory.limit_in_bytes"))? {
            // v1 signals an unlimited cgroup using a near-i64-max sentinel.
            let value = parse_memory_limit(&value)?.filter(|limit| *limit < i64::MAX / 2);
            limit = minimum_limit(limit, value);
        }
    }
    Ok(limit)
}

#[cfg(target_os = "linux")]
fn minimum_limit<T: Ord>(current: Option<T>, next: Option<T>) -> Option<T> {
    match (current, next) {
        (Some(current), Some(next)) => Some(current.min(next)),
        (Some(current), None) => Some(current),
        (None, Some(next)) => Some(next),
        (None, None) => None,
    }
}

#[cfg(target_os = "linux")]
fn parse_memory_limit(value: &str) -> Result<Option<i64>, SourceError> {
    let value = value.trim();
    if value == "max" {
        Ok(None)
    } else {
        let bytes = value.parse::<i64>().map_err(|_| SourceError::ReadError)?;
        (bytes > 0)
            .then_some(bytes)
            .ok_or(SourceError::ReadError)
            .map(Some)
    }
}

#[cfg(target_os = "linux")]
fn parse_cpuset(value: &str) -> Result<i32, SourceError> {
    let mut count = 0_i32;
    for entry in value.trim().split(',') {
        let (first, last) = match entry.split_once('-') {
            Some((first, last)) => (first, last),
            None => (entry, entry),
        };
        let first = first.parse::<i32>().map_err(|_| SourceError::ReadError)?;
        let last = last.parse::<i32>().map_err(|_| SourceError::ReadError)?;
        if first < 0 || last < first {
            return Err(SourceError::ReadError);
        }
        count = count
            .checked_add(last - first + 1)
            .ok_or(SourceError::ReadError)?;
    }
    (count > 0).then_some(count).ok_or(SourceError::ReadError)
}

#[cfg(target_os = "linux")]
fn read_optional(path: &Path) -> Result<Option<String>, SourceError> {
    match std::fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(SourceError::ReadError),
    }
}

#[cfg(target_os = "linux")]
fn linux_cgroup_v2_ancestors() -> Result<Option<Vec<PathBuf>>, SourceError> {
    let cgroups =
        std::fs::read_to_string("/proc/self/cgroup").map_err(|_| SourceError::ReadError)?;
    let Some(relative) = cgroups.lines().find_map(|line| line.strip_prefix("0::")) else {
        return Ok(None);
    };
    cgroup_ancestors(Path::new("/sys/fs/cgroup"), relative).map(Some)
}

#[cfg(target_os = "linux")]
fn linux_cgroup_v1_ancestors(controller: &str) -> Result<Option<Vec<PathBuf>>, SourceError> {
    let cgroups =
        std::fs::read_to_string("/proc/self/cgroup").map_err(|_| SourceError::ReadError)?;
    let Some(relative) = cgroups.lines().find_map(|line| {
        let mut fields = line.splitn(3, ':');
        let _hierarchy = fields.next()?;
        let controllers = fields.next()?;
        let path = fields.next()?;
        controllers
            .split(',')
            .any(|candidate| candidate == controller)
            .then_some(path)
    }) else {
        return Ok(None);
    };
    let roots: &[&str] = match controller {
        "cpu" => &["/sys/fs/cgroup/cpu", "/sys/fs/cgroup/cpu,cpuacct"],
        "cpuset" => &["/sys/fs/cgroup/cpuset"],
        "memory" => &["/sys/fs/cgroup/memory"],
        _ => return Err(SourceError::ReadError),
    };
    for root in roots {
        if Path::new(root).is_dir() {
            return cgroup_ancestors(Path::new(root), relative).map(Some);
        }
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
fn cgroup_ancestors(root: &Path, relative: &str) -> Result<Vec<PathBuf>, SourceError> {
    let relative = relative.trim_start_matches('/');
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            std::path::Component::Normal(component) => current.push(component),
            std::path::Component::CurDir => {}
            _ => return Err(SourceError::ReadError),
        }
    }
    let mut ancestors = Vec::new();
    loop {
        ancestors.push(current.clone());
        if current == root {
            break;
        }
        if !current.pop() {
            return Err(SourceError::ReadError);
        }
    }
    Ok(ancestors)
}

#[cfg(windows)]
fn windows_physical_memory_bytes() -> Result<i64, SourceError> {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_physical: u64,
        available_physical: u64,
        total_page_file: u64,
        available_page_file: u64,
        total_virtual: u64,
        available_virtual: u64,
        available_extended_virtual: u64,
    }

    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut memory = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
        return Err(SourceError::ReadError);
    }
    i64::try_from(memory.total_physical).map_err(|_| SourceError::ReadError)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClassInitFailure {
    Source(SourceError),
    SourceUnwind,
    MutexPoisoned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClassAccessError {
    InitialFailure(Arc<ClassInitFailure>),
    ErroneousReuse(Arc<ClassInitFailure>),
    SameThreadReentry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProcessHostError {
    Class(ClassAccessError),
    Source(SourceError),
    HomeMissing,
    UnsupportedUtf16,
    MutexPoisoned,
}

enum ClassCellState<T> {
    Vacant,
    Initializing { thread: ThreadId },
    Ready(T),
    Failed(Arc<ClassInitFailure>),
}

struct ClassCell<T> {
    state: Mutex<ClassCellState<T>>,
    changed: Condvar,
    #[cfg(test)]
    waiters: AtomicUsize,
}

impl<T> ClassCell<T> {
    fn new() -> Self {
        Self {
            state: Mutex::new(ClassCellState::Vacant),
            changed: Condvar::new(),
            #[cfg(test)]
            waiters: AtomicUsize::new(0),
        }
    }

    fn fail_poisoned(&self, mut state: MutexGuard<'_, ClassCellState<T>>) -> ClassAccessError {
        let error = Arc::new(ClassInitFailure::MutexPoisoned);
        self.state.clear_poison();
        *state = ClassCellState::Failed(error.clone());
        self.changed.notify_all();
        ClassAccessError::ErroneousReuse(error)
    }

    #[cfg(test)]
    fn waiter_count(&self) -> usize {
        self.waiters.load(Ordering::Acquire)
    }
}

impl<T: Clone> ClassCell<T> {
    fn get_or_init(
        &self,
        init: impl FnOnce() -> Result<T, ClassInitFailure>,
    ) -> Result<T, ClassAccessError> {
        let thread = std::thread::current().id();
        let mut init = Some(init);
        loop {
            let mut state = match self.state.lock() {
                Ok(state) => state,
                Err(poisoned) => return Err(self.fail_poisoned(poisoned.into_inner())),
            };
            match &*state {
                ClassCellState::Ready(value) => return Ok(value.clone()),
                ClassCellState::Failed(error) => {
                    return Err(ClassAccessError::ErroneousReuse(error.clone()));
                }
                ClassCellState::Initializing { thread: owner } if *owner == thread => {
                    return Err(ClassAccessError::SameThreadReentry);
                }
                ClassCellState::Initializing { .. } => {
                    #[cfg(test)]
                    self.waiters.fetch_add(1, Ordering::Release);
                    let waited = self.changed.wait(state);
                    #[cfg(test)]
                    self.waiters.fetch_sub(1, Ordering::Release);
                    state = match waited {
                        Ok(state) => state,
                        Err(poisoned) => return Err(self.fail_poisoned(poisoned.into_inner())),
                    };
                    drop(state);
                }
                ClassCellState::Vacant => {
                    *state = ClassCellState::Initializing { thread };
                    drop(state);
                    let result = std::panic::catch_unwind(AssertUnwindSafe(
                        init.take().expect("ClassCell initializer runs once"),
                    ))
                    .unwrap_or_else(|_| Err(ClassInitFailure::SourceUnwind));
                    let mut state = match self.state.lock() {
                        Ok(state) => state,
                        Err(poisoned) => return Err(self.fail_poisoned(poisoned.into_inner())),
                    };
                    if let ClassCellState::Failed(error) = &*state {
                        return Err(ClassAccessError::ErroneousReuse(error.clone()));
                    }
                    match result {
                        Ok(value) => {
                            *state = ClassCellState::Ready(value.clone());
                            self.changed.notify_all();
                            return Ok(value);
                        }
                        Err(error) => {
                            let error = Arc::new(error);
                            *state = ClassCellState::Failed(error.clone());
                            self.changed.notify_all();
                            return Err(ClassAccessError::InitialFailure(error));
                        }
                    }
                }
            }
        }
    }
}

enum CapacityCellState<T> {
    Vacant,
    Initializing { thread: ThreadId },
    Ready(T),
}

struct CapacityCell<T> {
    state: Mutex<CapacityCellState<T>>,
    changed: Condvar,
}

impl<T> CapacityCell<T> {
    fn new() -> Self {
        Self {
            state: Mutex::new(CapacityCellState::Vacant),
            changed: Condvar::new(),
        }
    }

    fn release_poisoned(&self, mut state: MutexGuard<'_, CapacityCellState<T>>) {
        self.state.clear_poison();
        *state = CapacityCellState::Vacant;
        self.changed.notify_all();
    }
}

impl<T: Clone> CapacityCell<T> {
    fn get_or_try_init(
        &self,
        init: impl FnOnce() -> Result<T, ProcessHostError>,
    ) -> Result<T, ProcessHostError> {
        let thread = std::thread::current().id();
        let mut init = Some(init);
        loop {
            let mut state = match self.state.lock() {
                Ok(state) => state,
                Err(poisoned) => {
                    self.release_poisoned(poisoned.into_inner());
                    return Err(ProcessHostError::MutexPoisoned);
                }
            };
            match &*state {
                CapacityCellState::Ready(value) => return Ok(value.clone()),
                CapacityCellState::Initializing { thread: owner } if *owner == thread => {
                    return Err(ProcessHostError::Class(ClassAccessError::SameThreadReentry));
                }
                CapacityCellState::Initializing { .. } => {
                    state = match self.changed.wait(state) {
                        Ok(state) => state,
                        Err(poisoned) => {
                            self.release_poisoned(poisoned.into_inner());
                            return Err(ProcessHostError::MutexPoisoned);
                        }
                    };
                    drop(state);
                }
                CapacityCellState::Vacant => {
                    *state = CapacityCellState::Initializing { thread };
                    drop(state);
                    let result = std::panic::catch_unwind(AssertUnwindSafe(
                        init.take().expect("capacity initializer runs once"),
                    ))
                    .unwrap_or(Err(ProcessHostError::Class(
                        ClassAccessError::InitialFailure(Arc::new(ClassInitFailure::SourceUnwind)),
                    )));
                    let mut state = match self.state.lock() {
                        Ok(state) => state,
                        Err(poisoned) => {
                            self.release_poisoned(poisoned.into_inner());
                            return Err(ProcessHostError::MutexPoisoned);
                        }
                    };
                    if !matches!(&*state, CapacityCellState::Initializing { thread: owner } if *owner == thread)
                    {
                        return Err(ProcessHostError::MutexPoisoned);
                    }
                    match result {
                        Ok(value) => {
                            *state = CapacityCellState::Ready(value.clone());
                            self.changed.notify_all();
                            return Ok(value);
                        }
                        Err(error) => {
                            *state = CapacityCellState::Vacant;
                            self.changed.notify_all();
                            return Err(error);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostOs {
    Linux,
    Windows,
    Macos,
    Freebsd,
    Openbsd,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostCpu {
    X86_32,
    X86_64,
    Ppc,
    Arm,
    Aarch64,
    S390x,
    Mips64,
    Riscv64,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AutoCpuToken {
    DarwinX86_64,
    DarwinArm64,
    Freebsd,
    Openbsd,
    X64Windows,
    Arm64Windows,
    Piii,
    K8,
    Ppc,
    Arm,
    Aarch64,
    S390x,
    Mips64,
    Riscv64,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostPathFlavor {
    Unix,
    Windows,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ResourceSample {
    memory_mib: f64,
    processors: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostCapacity {
    host_cpus: i32,
    host_ram_mib: i32,
}

pub struct ProcessHostOwner {
    source: Arc<dyn ProcessHostSource>,
    os: ClassCell<HostOs>,
    cpu: ClassCell<HostCpu>,
    resources: ClassCell<ResourceSample>,
    action_environment: ClassCell<slug_configuration_v2::native::host::ActionEnvironmentHost>,
    capacity: CapacityCell<ResourceSample>,
}

impl ProcessHostOwner {
    /// Creates a fresh process-latched Rust-native Host owner for a production
    /// runtime. Each retained runtime gets its own latch set, matching the
    /// owner boundary rather than using mutable process-global configuration.
    pub fn native() -> Arc<Self> {
        Self::with_source(Arc::new(NativeSource))
    }

    /// Test-only source boundary used to prove failures are explicit. Production
    /// constructors must use [`Self::native`].
    #[cfg(test)]
    pub fn unsupported() -> Arc<Self> {
        Self::with_source(Arc::new(UnsupportedSource))
    }

    fn with_source(source: Arc<dyn ProcessHostSource>) -> Arc<Self> {
        Arc::new(Self {
            source,
            os: ClassCell::new(),
            cpu: ClassCell::new(),
            resources: ClassCell::new(),
            action_environment: ClassCell::new(),
            capacity: CapacityCell::new(),
        })
    }

    fn os(&self) -> Result<HostOs, ProcessHostError> {
        self.os
            .get_or_init(|| {
                let name = match self.source.property(Property::BlazeOs) {
                    PropertyRead::Present(value) => value,
                    PropertyRead::Absent => match self.source.property(Property::OsName) {
                        PropertyRead::Present(value) => value,
                        PropertyRead::Absent => Arc::from([]),
                        PropertyRead::ReadError(error) => {
                            return Err(ClassInitFailure::Source(error));
                        }
                    },
                    PropertyRead::ReadError(error) => return Err(ClassInitFailure::Source(error)),
                };
                Ok(parse_os(&name))
            })
            .map_err(ProcessHostError::Class)
    }

    fn cpu(&self) -> Result<HostCpu, ProcessHostError> {
        self.cpu
            .get_or_init(|| match self.source.property(Property::OsArch) {
                PropertyRead::Present(value) => Ok(parse_cpu(&value)),
                PropertyRead::Absent => Ok(HostCpu::Unknown),
                PropertyRead::ReadError(error) => Err(ClassInitFailure::Source(error)),
            })
            .map_err(ProcessHostError::Class)
    }

    fn path_flavor(&self) -> Result<HostPathFlavor, ProcessHostError> {
        Ok(match self.os()? {
            HostOs::Windows => HostPathFlavor::Windows,
            _ => HostPathFlavor::Unix,
        })
    }

    fn auto_cpu(&self) -> Result<AutoCpuToken, ProcessHostError> {
        let os = self.os()?;
        match os {
            HostOs::Linux | HostOs::Windows | HostOs::Macos => {
                Ok(auto_cpu_token(os, Some(self.cpu()?)))
            }
            _ => Ok(auto_cpu_token(os, None)),
        }
    }

    fn home(&self) -> Result<Arc<[u16]>, ProcessHostError> {
        match self.source.property(Property::UserHome) {
            PropertyRead::Present(value) if has_unpaired_utf16(&value) => {
                Err(ProcessHostError::UnsupportedUtf16)
            }
            PropertyRead::Present(value) => Ok(value),
            PropertyRead::Absent => Err(ProcessHostError::HomeMissing),
            PropertyRead::ReadError(error) => Err(ProcessHostError::Source(error)),
        }
    }

    fn resource(&self) -> Result<ResourceSample, ProcessHostError> {
        self.resources
            .get_or_init(|| {
                let bytes = self
                    .source
                    .memory_bytes()
                    .map_err(ClassInitFailure::Source)?;
                let memory_mib = bytes as f64 / (1024.0 * 1024.0);
                let processors = self.source.processors().map_err(ClassInitFailure::Source)?;
                Ok(ResourceSample {
                    memory_mib,
                    processors,
                })
            })
            .map_err(ProcessHostError::Class)
    }

    fn capacity_resource(&self) -> Result<ResourceSample, ProcessHostError> {
        self.capacity.get_or_try_init(|| {
            let sample = self.resource()?;
            self.source
                .after_resource()
                .map_err(ProcessHostError::Source)?;
            Ok(sample)
        })
    }

    fn capacity_keywords(&self) -> Result<HostCapacity, ProcessHostError> {
        let sample = self.capacity_resource()?;
        Ok(HostCapacity {
            host_cpus: java_double_to_int((sample.processors as f64).ceil()),
            host_ram_mib: java_double_to_int(sample.memory_mib.ceil()),
        })
    }

    fn action_environment_host(
        &self,
    ) -> Result<slug_configuration_v2::native::host::ActionEnvironmentHost, ProcessHostError> {
        use slug_configuration_v2::native::host::ActionEnvironmentHost;
        use slug_configuration_v2::native::host::ActionEnvironmentHostOs;

        let os = self.os()?;
        self.action_environment
            .get_or_init(|| {
                let configuration_os = match os {
                    HostOs::Linux => ActionEnvironmentHostOs::Linux,
                    HostOs::Windows => ActionEnvironmentHostOs::Windows,
                    HostOs::Macos => ActionEnvironmentHostOs::Macos,
                    HostOs::Freebsd => ActionEnvironmentHostOs::Freebsd,
                    HostOs::Openbsd => ActionEnvironmentHostOs::Openbsd,
                    HostOs::Unknown => ActionEnvironmentHostOs::Unknown,
                };
                if os != HostOs::Windows {
                    return Ok(ActionEnvironmentHost::without_environment(configuration_os));
                }
                let read = |property| match self.source.property(property) {
                    PropertyRead::Present(value) => String::from_utf16(&value)
                        .map(Some)
                        .map_err(|_| ClassInitFailure::Source(SourceError::ReadError)),
                    PropertyRead::Absent => Ok(None),
                    PropertyRead::ReadError(error) => Err(ClassInitFailure::Source(error)),
                };
                let bazel_sh = read(Property::BazelSh)?;
                let path = read(Property::Path)?;
                let system_root = read(Property::SystemRoot)?;
                Ok(ActionEnvironmentHost::windows(
                    bazel_sh.as_deref(),
                    path.as_deref(),
                    system_root.as_deref(),
                ))
            })
            .map_err(ProcessHostError::Class)
    }

    /// Supplies precisely the Host facts needed for the admitted default
    /// configuration. This deliberately does not observe capacity or home:
    /// neither default conversion requests them.
    pub(super) fn default_configuration_inputs(
        &self,
    ) -> Result<slug_configuration_v2::native::host::HostConversionInputs, String> {
        self.default_configuration_inputs_inner()
            .map_err(|error| format!("reading Rust-native Host configuration inputs: {error:?}"))
    }

    pub(super) fn repository_platform(&self) -> Result<slug_bzlmod_v2::RepositoryPlatform, String> {
        let os_name = match self
            .os()
            .map_err(|error| format!("reading Rust-native repository Host OS: {error:?}"))?
        {
            HostOs::Linux => "linux",
            HostOs::Windows => "windows",
            HostOs::Macos => "mac os x",
            HostOs::Freebsd => "freebsd",
            HostOs::Openbsd => "openbsd",
            HostOs::Unknown => return Err("unsupported Rust-native repository Host OS".to_owned()),
        };
        let arch = match self.cpu().map_err(|error| {
            format!("reading Rust-native repository Host architecture: {error:?}")
        })? {
            HostCpu::X86_32 => "x86",
            HostCpu::X86_64 => "x86_64",
            HostCpu::Ppc => "ppc",
            HostCpu::Arm => "arm",
            HostCpu::Aarch64 => "aarch64",
            HostCpu::S390x => "s390x",
            HostCpu::Mips64 => "mips64",
            HostCpu::Riscv64 => "riscv64",
            HostCpu::Unknown => {
                return Err("unsupported Rust-native repository Host architecture".to_owned());
            }
        };
        Ok(slug_bzlmod_v2::RepositoryPlatform::new(os_name, arch))
    }

    fn default_configuration_inputs_inner(
        &self,
    ) -> Result<slug_configuration_v2::native::host::HostConversionInputs, ProcessHostError> {
        use slug_configuration_v2::native::host::AutoCpuToken as ConfigurationAutoCpu;
        use slug_configuration_v2::native::host::HostConversionInputs;
        use slug_configuration_v2::native::host::HostPathFlavor as ConfigurationPathFlavor;

        let auto_cpu = match self.auto_cpu()? {
            AutoCpuToken::DarwinX86_64 => ConfigurationAutoCpu::DarwinX86_64,
            AutoCpuToken::DarwinArm64 => ConfigurationAutoCpu::DarwinArm64,
            AutoCpuToken::Freebsd => ConfigurationAutoCpu::Freebsd,
            AutoCpuToken::Openbsd => ConfigurationAutoCpu::Openbsd,
            AutoCpuToken::X64Windows => ConfigurationAutoCpu::X64Windows,
            AutoCpuToken::Arm64Windows => ConfigurationAutoCpu::Arm64Windows,
            AutoCpuToken::Piii => ConfigurationAutoCpu::Piii,
            AutoCpuToken::K8 => ConfigurationAutoCpu::K8,
            AutoCpuToken::Ppc => ConfigurationAutoCpu::Ppc,
            AutoCpuToken::Arm => ConfigurationAutoCpu::Arm,
            AutoCpuToken::Aarch64 => ConfigurationAutoCpu::Aarch64,
            AutoCpuToken::S390x => ConfigurationAutoCpu::S390x,
            AutoCpuToken::Mips64 => ConfigurationAutoCpu::Mips64,
            AutoCpuToken::Riscv64 => ConfigurationAutoCpu::Riscv64,
            AutoCpuToken::Unknown => ConfigurationAutoCpu::Unknown,
        };
        let path_flavor = match self.path_flavor()? {
            HostPathFlavor::Unix => ConfigurationPathFlavor::Unix,
            HostPathFlavor::Windows => ConfigurationPathFlavor::Windows,
        };
        let inputs = HostConversionInputs::new(
            Some(auto_cpu),
            Some(path_flavor),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .expect("empty default configuration Host facts are ordered");
        Ok(inputs.with_action_environment_host(self.action_environment_host()?))
    }
}

fn parse_os(value: &[u16]) -> HostOs {
    if value.starts_with(&[76, 105, 110, 117, 120]) {
        HostOs::Linux
    } else if value.starts_with(&[87, 105, 110, 100, 111, 119, 115]) {
        HostOs::Windows
    } else if value.starts_with(&[77, 97, 99, 32, 79, 83, 32, 88]) {
        HostOs::Macos
    } else if value.starts_with(&[70, 114, 101, 101, 66, 83, 68]) {
        HostOs::Freebsd
    } else if value.starts_with(&[79, 112, 101, 110, 66, 83, 68]) {
        HostOs::Openbsd
    } else {
        HostOs::Unknown
    }
}

fn parse_cpu(value: &[u16]) -> HostCpu {
    if matches!(
        value,
        [105, 51, 56, 54]
            | [105, 52, 56, 54]
            | [105, 53, 56, 54]
            | [105, 54, 56, 54]
            | [105, 55, 56, 54]
            | [120, 56, 54]
    ) {
        HostCpu::X86_32
    } else if matches!(
        value,
        [97, 109, 100, 54, 52] | [120, 56, 54, 95, 54, 52] | [120, 54, 52]
    ) {
        HostCpu::X86_64
    } else if matches!(
        value,
        [112, 112, 99] | [112, 112, 99, 54, 52] | [112, 112, 99, 54, 52, 108, 101]
    ) {
        HostCpu::Ppc
    } else if matches!(value, [97, 114, 109] | [97, 114, 109, 118, 55, 108]) {
        HostCpu::Arm
    } else if value == [97, 97, 114, 99, 104, 54, 52] {
        HostCpu::Aarch64
    } else if matches!(value, [115, 51, 57, 48, 120] | [115, 51, 57, 48]) {
        HostCpu::S390x
    } else if matches!(
        value,
        [109, 105, 112, 115, 54, 52] | [109, 105, 112, 115, 54, 52, 101, 108]
    ) {
        HostCpu::Mips64
    } else if value == [114, 105, 115, 99, 118, 54, 52] {
        HostCpu::Riscv64
    } else {
        HostCpu::Unknown
    }
}

fn auto_cpu_token(os: HostOs, cpu: Option<HostCpu>) -> AutoCpuToken {
    match (os, cpu) {
        (HostOs::Macos, Some(HostCpu::X86_64)) => AutoCpuToken::DarwinX86_64,
        (HostOs::Macos, Some(HostCpu::Aarch64)) => AutoCpuToken::DarwinArm64,
        (HostOs::Freebsd, _) => AutoCpuToken::Freebsd,
        (HostOs::Openbsd, _) => AutoCpuToken::Openbsd,
        (HostOs::Windows, Some(HostCpu::X86_64)) => AutoCpuToken::X64Windows,
        (HostOs::Windows, Some(HostCpu::Aarch64)) => AutoCpuToken::Arm64Windows,
        (HostOs::Linux, Some(HostCpu::X86_32)) => AutoCpuToken::Piii,
        (HostOs::Linux, Some(HostCpu::X86_64)) => AutoCpuToken::K8,
        (HostOs::Linux, Some(HostCpu::Ppc)) => AutoCpuToken::Ppc,
        (HostOs::Linux, Some(HostCpu::Arm)) => AutoCpuToken::Arm,
        (HostOs::Linux, Some(HostCpu::Aarch64)) => AutoCpuToken::Aarch64,
        (HostOs::Linux, Some(HostCpu::S390x)) => AutoCpuToken::S390x,
        (HostOs::Linux, Some(HostCpu::Mips64)) => AutoCpuToken::Mips64,
        (HostOs::Linux, Some(HostCpu::Riscv64)) => AutoCpuToken::Riscv64,
        _ => AutoCpuToken::Unknown,
    }
}

fn java_double_to_int(value: f64) -> i32 {
    value as i32
}

fn has_unpaired_utf16(value: &[u16]) -> bool {
    let mut index = 0;
    while index < value.len() {
        let unit = value[index];
        if (0xd800..=0xdbff).contains(&unit) {
            if index + 1 == value.len() || !(0xdc00..=0xdfff).contains(&value[index + 1]) {
                return true;
            }
            index += 2;
        } else if (0xdc00..=0xdfff).contains(&unit) {
            return true;
        } else {
            index += 1;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Barrier;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn physical_memory_inputs_fail_closed_before_checked_multiplication() {
        assert_eq!(
            checked_physical_memory_bytes(-1, -1),
            Err(SourceError::ReadError)
        );
        assert_eq!(
            checked_physical_memory_bytes(0, 4096),
            Err(SourceError::ReadError)
        );
        assert_eq!(
            checked_physical_memory_bytes(1, 0),
            Err(SourceError::ReadError)
        );
        assert_eq!(
            checked_physical_memory_bytes(i64::MAX, 2),
            Err(SourceError::ReadError)
        );
        assert_eq!(checked_physical_memory_bytes(3, 4096), Ok(12_288));
    }

    #[derive(Default)]
    struct FakeState {
        properties: VecDeque<PropertyRead>,
        memory: VecDeque<Result<i64, SourceError>>,
        processors: VecDeque<Result<i32, SourceError>>,
        after_resource: VecDeque<Result<(), SourceError>>,
        calls: Vec<Property>,
        resource_calls: Vec<&'static str>,
    }

    #[derive(Default)]
    struct FakeSource(Mutex<FakeState>);

    impl FakeSource {
        fn owner(state: FakeState) -> (Arc<Self>, Arc<ProcessHostOwner>) {
            let source = Arc::new(Self(Mutex::new(state)));
            let owner = ProcessHostOwner::with_source(source.clone());
            (source, owner)
        }

        fn calls(&self) -> Vec<Property> {
            self.0.lock().unwrap().calls.clone()
        }
    }

    impl ProcessHostSource for FakeSource {
        fn property(&self, property: Property) -> PropertyRead {
            let mut state = self.0.lock().unwrap();
            state.calls.push(property);
            state.properties.pop_front().unwrap_or(PropertyRead::Absent)
        }

        fn memory_bytes(&self) -> Result<i64, SourceError> {
            let mut state = self.0.lock().unwrap();
            state.resource_calls.push("memory");
            state
                .memory
                .pop_front()
                .unwrap_or(Err(SourceError::ReadError))
        }

        fn processors(&self) -> Result<i32, SourceError> {
            let mut state = self.0.lock().unwrap();
            state.resource_calls.push("processors");
            state
                .processors
                .pop_front()
                .unwrap_or(Err(SourceError::ReadError))
        }

        fn after_resource(&self) -> Result<(), SourceError> {
            let mut state = self.0.lock().unwrap();
            state.resource_calls.push("after_resource");
            state
                .after_resource
                .pop_front()
                .unwrap_or(Err(SourceError::ReadError))
        }
    }

    fn property(value: &str) -> PropertyRead {
        PropertyRead::Present(Arc::from(value.encode_utf16().collect::<Vec<_>>()))
    }

    #[test]
    fn auto_cpu_has_all_pinned_routes_and_reads_cpu_only_when_needed() {
        let cases = [
            (
                HostOs::Macos,
                Some(HostCpu::X86_64),
                AutoCpuToken::DarwinX86_64,
            ),
            (
                HostOs::Macos,
                Some(HostCpu::Aarch64),
                AutoCpuToken::DarwinArm64,
            ),
            (HostOs::Freebsd, None, AutoCpuToken::Freebsd),
            (HostOs::Openbsd, None, AutoCpuToken::Openbsd),
            (
                HostOs::Windows,
                Some(HostCpu::X86_64),
                AutoCpuToken::X64Windows,
            ),
            (
                HostOs::Windows,
                Some(HostCpu::Aarch64),
                AutoCpuToken::Arm64Windows,
            ),
            (HostOs::Linux, Some(HostCpu::X86_32), AutoCpuToken::Piii),
            (HostOs::Linux, Some(HostCpu::X86_64), AutoCpuToken::K8),
            (HostOs::Linux, Some(HostCpu::Ppc), AutoCpuToken::Ppc),
            (HostOs::Linux, Some(HostCpu::Arm), AutoCpuToken::Arm),
            (HostOs::Linux, Some(HostCpu::Aarch64), AutoCpuToken::Aarch64),
            (HostOs::Linux, Some(HostCpu::S390x), AutoCpuToken::S390x),
            (HostOs::Linux, Some(HostCpu::Mips64), AutoCpuToken::Mips64),
            (HostOs::Linux, Some(HostCpu::Riscv64), AutoCpuToken::Riscv64),
            (HostOs::Unknown, None, AutoCpuToken::Unknown),
        ];
        assert_eq!(cases.len(), 15);
        for (os, cpu, expected) in cases {
            assert_eq!(auto_cpu_token(os, cpu), expected);
        }

        let (source, owner) = FakeSource::owner(FakeState {
            properties: VecDeque::from([PropertyRead::Absent, property("FreeBSD")]),
            ..Default::default()
        });
        assert_eq!(owner.auto_cpu(), Ok(AutoCpuToken::Freebsd));
        assert_eq!(source.calls(), vec![Property::BlazeOs, Property::OsName]);
        assert_eq!(owner.path_flavor(), Ok(HostPathFlavor::Unix));

        let (source, owner) = FakeSource::owner(FakeState {
            properties: VecDeque::from([
                PropertyRead::Absent,
                property("Linux"),
                property("x86_64"),
            ]),
            ..Default::default()
        });
        assert_eq!(owner.auto_cpu(), Ok(AutoCpuToken::K8));
        assert_eq!(
            source.calls(),
            vec![Property::BlazeOs, Property::OsName, Property::OsArch]
        );
    }

    #[test]
    fn properties_latch_class_errors_but_home_is_fresh_and_lossless() {
        let (_, failed) = FakeSource::owner(FakeState {
            properties: VecDeque::from([PropertyRead::ReadError(SourceError::ReadError)]),
            ..Default::default()
        });
        assert!(matches!(
            failed.os(),
            Err(ProcessHostError::Class(ClassAccessError::InitialFailure(_)))
        ));
        assert!(matches!(
            failed.os(),
            Err(ProcessHostError::Class(ClassAccessError::ErroneousReuse(_)))
        ));

        let (source, owner) = FakeSource::owner(FakeState {
            properties: VecDeque::from([
                PropertyRead::Present(Arc::from([0x0061u16, 0x0062])),
                PropertyRead::Absent,
                PropertyRead::Present(Arc::from([0xd800u16])),
                PropertyRead::ReadError(SourceError::ReadError),
            ]),
            ..Default::default()
        });
        assert_eq!(owner.home(), Ok(Arc::from([0x0061u16, 0x0062])));
        assert_eq!(owner.home(), Err(ProcessHostError::HomeMissing));
        assert_eq!(owner.home(), Err(ProcessHostError::UnsupportedUtf16));
        assert_eq!(
            owner.home(),
            Err(ProcessHostError::Source(SourceError::ReadError))
        );
        assert_eq!(source.calls(), vec![Property::UserHome; 4]);
    }

    #[test]
    fn capacity_caches_only_successful_resources_and_converts_afterward() {
        let (source, owner) = FakeSource::owner(FakeState {
            memory: VecDeque::from([Ok(1_572_864_001)]),
            processors: VecDeque::from([Ok(3)]),
            after_resource: VecDeque::from([Err(SourceError::Retryable), Ok(())]),
            ..Default::default()
        });
        assert_eq!(
            owner.capacity_resource(),
            Err(ProcessHostError::Source(SourceError::Retryable))
        );
        assert_eq!(
            owner.capacity_keywords(),
            Ok(HostCapacity {
                host_cpus: 3,
                host_ram_mib: 1501,
            })
        );
        assert_eq!(owner.capacity_keywords().unwrap().host_ram_mib, 1501);
        assert_eq!(source.0.lock().unwrap().memory.len(), 0);
        assert_eq!(source.0.lock().unwrap().processors.len(), 0);
        assert_eq!(source.0.lock().unwrap().after_resource.len(), 0);
        assert_eq!(
            source.0.lock().unwrap().resource_calls,
            ["memory", "processors", "after_resource", "after_resource"]
        );
        assert_eq!(java_double_to_int(f64::NAN), 0);
        assert_eq!(java_double_to_int(f64::INFINITY), i32::MAX);
        assert_eq!(java_double_to_int(f64::NEG_INFINITY), i32::MIN);

        let (_, failed) = FakeSource::owner(FakeState {
            memory: VecDeque::from([Err(SourceError::ReadError)]),
            ..Default::default()
        });
        assert!(matches!(
            failed.capacity_resource(),
            Err(ProcessHostError::Class(ClassAccessError::InitialFailure(_)))
        ));
        assert!(matches!(
            failed.capacity_resource(),
            Err(ProcessHostError::Class(ClassAccessError::ErroneousReuse(_)))
        ));
    }

    #[test]
    fn class_reentry_and_unwind_fail_closed_without_sticking() {
        let cell = Arc::new(ClassCell::<u8>::new());
        let nested = cell.clone();
        let seen = Arc::new(Mutex::new(None));
        let captured = seen.clone();
        assert_eq!(
            cell.get_or_init(|| {
                *captured.lock().unwrap() = Some(nested.get_or_init(|| Ok(2)).unwrap_err());
                Ok(1)
            }),
            Ok(1)
        );
        assert_eq!(
            *seen.lock().unwrap(),
            Some(ClassAccessError::SameThreadReentry)
        );

        let unwound = ClassCell::<u8>::new();
        assert!(matches!(
            unwound.get_or_init(|| panic!("source unwind")),
            Err(ClassAccessError::InitialFailure(error)) if *error == ClassInitFailure::SourceUnwind
        ));
        assert!(matches!(
            unwound.get_or_init(|| Ok(1)),
            Err(ClassAccessError::ErroneousReuse(error)) if *error == ClassInitFailure::SourceUnwind
        ));

        let shared = Arc::new(ClassCell::<u8>::new());
        let gate = Arc::new(Barrier::new(2));
        let first = shared.clone();
        let first_gate = gate.clone();
        let worker = std::thread::spawn(move || {
            first.get_or_init(|| {
                first_gate.wait();
                panic!("source unwind after waiter starts")
            })
        });
        gate.wait();
        assert!(matches!(
            shared.get_or_init(|| Ok(1)),
            Err(ClassAccessError::ErroneousReuse(error)) if *error == ClassInitFailure::SourceUnwind
        ));
        assert!(matches!(
            worker.join().unwrap(),
            Err(ClassAccessError::InitialFailure(_))
        ));

        let poisoned = Arc::new(ClassCell::<u8>::new());
        let initialized = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let first = poisoned.clone();
        let first_initialized = initialized.clone();
        let first_release = release.clone();
        let initializer = std::thread::spawn(move || {
            first.get_or_init(|| {
                first_initialized.wait();
                first_release.wait();
                Ok(1)
            })
        });
        initialized.wait();

        let (waited_tx, waited_rx) = mpsc::channel();
        let waiter_cell = poisoned.clone();
        let waiter = std::thread::spawn(move || {
            let result = waiter_cell.get_or_init(|| Ok(2));
            waited_tx.send(result).unwrap();
        });
        for _ in 0..10_000 {
            if poisoned.waiter_count() == 1 {
                break;
            }
            std::thread::yield_now();
        }
        assert_eq!(poisoned.waiter_count(), 1);

        let poison_target = poisoned.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = poison_target.state.lock().unwrap();
                panic!("poison state while a waiter sleeps");
            })
            .join()
            .is_err()
        );
        assert!(matches!(
            poisoned.get_or_init(|| Ok(3)),
            Err(ClassAccessError::ErroneousReuse(error))
                if *error == ClassInitFailure::MutexPoisoned
        ));
        assert!(matches!(
            waited_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(ClassAccessError::ErroneousReuse(error))
                if *error == ClassInitFailure::MutexPoisoned
        ));
        release.wait();
        assert!(matches!(
            initializer.join().unwrap(),
            Err(ClassAccessError::ErroneousReuse(error))
                if *error == ClassInitFailure::MutexPoisoned
        ));
        waiter.join().unwrap();

        let capacity = Arc::new(CapacityCell::<u8>::new());
        let capacity_target = capacity.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = capacity_target.state.lock().unwrap();
                panic!("poison capacity state");
            })
            .join()
            .is_err()
        );
        assert_eq!(
            capacity.get_or_try_init(|| Ok(1)),
            Err(ProcessHostError::MutexPoisoned)
        );
        assert_eq!(capacity.get_or_try_init(|| Ok(2)), Ok(2));
    }

    #[test]
    fn unsupported_owner_never_has_a_readable_source_and_owners_are_distinct() {
        let first = ProcessHostOwner::unsupported();
        let second = ProcessHostOwner::unsupported();
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(
            matches!(first.os(), Err(ProcessHostError::Class(ClassAccessError::InitialFailure(error))) if *error == ClassInitFailure::Source(SourceError::Unsupported))
        );
        assert!(matches!(
            first.home(),
            Err(ProcessHostError::Source(SourceError::Unsupported))
        ));
    }

    #[test]
    fn native_source_uses_explicit_bazel_spelling_and_unicode_home() {
        assert_eq!(
            NativeSource.property(Property::BlazeOs),
            PropertyRead::Absent
        );
        let expected_os = match std::env::consts::OS {
            "linux" => Some("Linux"),
            "windows" => Some("Windows"),
            "macos" => Some("Mac OS X"),
            "freebsd" => Some("FreeBSD"),
            "openbsd" => Some("OpenBSD"),
            _ => None,
        };
        assert_eq!(native_os_name(), expected_os);
        assert_eq!(
            NativeSource.property(Property::OsName),
            expected_os
                .map(utf16_property)
                .unwrap_or(PropertyRead::Absent)
        );
        assert!(matches!(
            NativeSource.property(Property::OsArch),
            PropertyRead::Present(_) | PropertyRead::Absent
        ));
    }

    #[test]
    fn default_configuration_inputs_demand_only_auto_cpu_and_path_flavor() {
        let (source, owner) = FakeSource::owner(FakeState {
            properties: VecDeque::from([
                PropertyRead::Absent,
                property("Linux"),
                property("x86_64"),
            ]),
            ..Default::default()
        });
        let inputs = owner.default_configuration_inputs().unwrap();
        assert_eq!(
            inputs.auto_cpu(),
            Some(slug_configuration_v2::native::host::AutoCpuToken::K8)
        );
        assert_eq!(
            inputs.path_flavor(),
            Some(slug_configuration_v2::native::host::HostPathFlavor::Unix)
        );
        assert_eq!(inputs.capacity(), None);
        assert!(inputs.home_facts().is_empty());
        assert!(inputs.windows_option_path_facts().is_empty());
        assert_eq!(
            inputs.action_environment_host().unwrap().os(),
            slug_configuration_v2::native::host::ActionEnvironmentHostOs::Linux
        );
        assert_eq!(
            source.calls(),
            vec![Property::BlazeOs, Property::OsName, Property::OsArch]
        );
        assert!(source.0.lock().unwrap().resource_calls.is_empty());
    }

    #[test]
    fn windows_action_environment_properties_latch_once_across_concurrent_requests() {
        let (source, owner) = FakeSource::owner(FakeState {
            properties: VecDeque::from([
                PropertyRead::Absent,
                property("Windows"),
                property("x86_64"),
                property("D:/tools/bin/bash.exe"),
                property("C:/client/bin"),
                property("D:\\Windows"),
            ]),
            ..Default::default()
        });
        let gate = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for _ in 0..2 {
            let owner = owner.clone();
            let gate = gate.clone();
            workers.push(std::thread::spawn(move || {
                gate.wait();
                owner.default_configuration_inputs().unwrap()
            }));
        }
        gate.wait();
        for worker in workers {
            let inputs = worker.join().unwrap();
            let host = inputs.action_environment_host().unwrap();
            assert_eq!(host.bazel_sh(), Some("D:/tools/bin/bash.exe"));
            assert_eq!(host.path(), Some("C:/client/bin"));
            assert_eq!(host.system_root(), Some("D:\\Windows"));
        }
        assert_eq!(
            source.calls(),
            vec![
                Property::BlazeOs,
                Property::OsName,
                Property::OsArch,
                Property::BazelSh,
                Property::Path,
                Property::SystemRoot,
            ]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn container_limit_parsers_are_bounded_and_fail_closed() {
        assert_eq!(cgroup_cpu_count(-1, 100_000), Ok(None));
        assert_eq!(cgroup_cpu_count(100_001, 100_000), Ok(Some(2)));
        assert_eq!(cgroup_cpu_count(1, 0), Err(SourceError::ReadError));
        assert_eq!(parse_cpuset("0-3,8,10-11"), Ok(7));
        assert_eq!(parse_cpuset("3-0"), Err(SourceError::ReadError));
        assert_eq!(parse_memory_limit("max\n"), Ok(None));
        assert_eq!(parse_memory_limit("1073741824"), Ok(Some(1_073_741_824)));
        assert_eq!(parse_memory_limit("0"), Err(SourceError::ReadError));
        assert_eq!(minimum_limit(Some(4), Some(2)), Some(2));
        assert_eq!(minimum_limit(Some(4), None), Some(4));
        assert_eq!(
            cgroup_ancestors(Path::new("/cgroup"), "/user.slice/worker"),
            Ok(vec![
                PathBuf::from("/cgroup/user.slice/worker"),
                PathBuf::from("/cgroup/user.slice"),
                PathBuf::from("/cgroup"),
            ])
        );
    }
}
