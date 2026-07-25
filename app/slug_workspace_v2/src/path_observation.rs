/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License found in the LICENSE-APACHE file in the root directory of this
 * source tree. You may select the license that applies to you.
 */

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use allocative::Key as AllocativeKey;
use allocative::Visitor;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use starlark_map::sorted_map::SortedMap;

/// A structural path computation result.
///
/// `Need` is transient scheduling state. It deliberately has no Rust equality:
/// callers must use [`PathOutcome::complete_eq`] at DICE equality boundaries.
#[derive(Debug, Clone, Allocative, Dupe)]
pub enum PathOutcome<T> {
    Complete(T),
    Need(NeedPathObservations),
}

pub type PathResult<T, E> = PathOutcome<Result<T, E>>;

impl<T> PathOutcome<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> PathOutcome<U> {
        match self {
            Self::Complete(value) => PathOutcome::Complete(f(value)),
            Self::Need(need) => PathOutcome::Need(need),
        }
    }

    pub fn and_then<U>(self, f: impl FnOnce(T) -> PathOutcome<U>) -> PathOutcome<U> {
        match self {
            Self::Complete(value) => f(value),
            Self::Need(need) => PathOutcome::Need(need),
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    pub fn complete_eq(&self, other: &Self) -> bool
    where
        T: PartialEq,
    {
        match (self, other) {
            (Self::Complete(left), Self::Complete(right)) => left == right,
            (Self::Complete(_), Self::Need(_))
            | (Self::Need(_), Self::Complete(_))
            | (Self::Need(_), Self::Need(_)) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyPathObservationNeed;

impl fmt::Display for EmptyPathObservationNeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("path observation demand set must not be empty")
    }
}

impl std::error::Error for EmptyPathObservationNeed {}

/// A nonempty, sorted, duplicate-free set of missing observations.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct NeedPathObservations {
    demands: Arc<[PathObservationDemand]>,
}

impl NeedPathObservations {
    pub fn singleton(demand: PathObservationDemand) -> Self {
        Self {
            demands: Arc::from([demand]),
        }
    }

    pub fn try_from_iter(
        demands: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Result<Self, EmptyPathObservationNeed> {
        let mut demands = demands.into_iter().collect::<Vec<_>>();
        demands.sort_unstable();
        demands.dedup();
        if demands.is_empty() {
            return Err(EmptyPathObservationNeed);
        }
        Ok(Self {
            demands: demands.into(),
        })
    }

    pub fn demands(&self) -> &[PathObservationDemand] {
        &self.demands
    }

    pub fn union(&self, other: &Self) -> Self {
        let mut merged = Vec::with_capacity(self.demands.len() + other.demands.len());
        let mut left = self.demands.iter();
        let mut right = other.demands.iter();
        let mut left_value = left.next();
        let mut right_value = right.next();
        while let (Some(left_demand), Some(right_demand)) = (left_value, right_value) {
            match left_demand.cmp(right_demand) {
                Ordering::Less => {
                    merged.push(left_demand.dupe());
                    left_value = left.next();
                }
                Ordering::Equal => {
                    merged.push(left_demand.dupe());
                    left_value = left.next();
                    right_value = right.next();
                }
                Ordering::Greater => {
                    merged.push(right_demand.dupe());
                    right_value = right.next();
                }
            }
        }
        merged.extend(left_value.into_iter().chain(left).map(Dupe::dupe));
        merged.extend(right_value.into_iter().chain(right).map(Dupe::dupe));
        Self {
            demands: merged.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathNormalizationError {
    path: PathBuf,
}

impl PathNormalizationError {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for PathNormalizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "path observation identity must be absolute: {:?}",
            self.path
        )
    }
}

impl std::error::Error for PathNormalizationError {}

/// A lexically normalized absolute OS-native path.
///
/// Construction performs no filesystem IO. Parent traversal clamps at the
/// filesystem root, not at a workspace or repository boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe)]
pub struct NormalizedAbsolutePath {
    path: Arc<PathBuf>,
}

impl NormalizedAbsolutePath {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, PathNormalizationError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(PathNormalizationError { path });
        }

        let mut normalized = PathBuf::new();
        let mut normal_component_count = 0usize;
        for component in path.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => {
                    normalized.push(component.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if normal_component_count != 0 {
                        let popped = normalized.pop();
                        debug_assert!(popped);
                        normal_component_count -= 1;
                    }
                }
                Component::Normal(component) => {
                    normalized.push(component);
                    normal_component_count += 1;
                }
            }
        }
        debug_assert!(normalized.is_absolute());
        Ok(Self {
            path: Arc::new(normalized),
        })
    }

    pub fn as_path(&self) -> &Path {
        self.path.as_path()
    }
}

impl fmt::Display for NormalizedAbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_path())
    }
}

/// An operational materialization incarnation. The allocating runtime owns its
/// lifetime and uniqueness; this type has no process-global allocator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub struct PathObservationInstanceId {
    value: u64,
}

impl PathObservationInstanceId {
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub enum PathObservationNamespace {
    Host,
    Materialization(PathObservationInstanceId),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub enum PathObservationOperation {
    Lstat,
    ReadLink,
    FileBytes,
    DirectoryEntries,
}

/// One exact operation requested from the outside-DICE observer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe)]
pub struct PathObservationDemand {
    namespace: PathObservationNamespace,
    path: NormalizedAbsolutePath,
    operation: PathObservationOperation,
}

impl PathObservationDemand {
    pub fn new(
        namespace: PathObservationNamespace,
        path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
    ) -> Self {
        Self {
            namespace,
            path,
            operation,
        }
    }

    pub const fn namespace(&self) -> PathObservationNamespace {
        self.namespace
    }

    pub fn path(&self) -> &NormalizedAbsolutePath {
        &self.path
    }

    pub const fn operation(&self) -> PathObservationOperation {
        self.operation
    }
}

/// Stable owned classification of an OS I/O error. `raw_os_error` on
/// [`PathObservationError::Io`] retains platform detail for `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub enum PathIoErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    HostUnreachable,
    NetworkUnreachable,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    NetworkDown,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    ReadOnlyFilesystem,
    StaleNetworkFileHandle,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    StorageFull,
    NotSeekable,
    QuotaExceeded,
    FileTooLarge,
    ResourceBusy,
    ExecutableFileBusy,
    Deadlock,
    CrossesDevices,
    TooManyLinks,
    InvalidFilename,
    ArgumentListTooLong,
    Interrupted,
    Unsupported,
    UnexpectedEof,
    OutOfMemory,
    Other,
}

impl From<std::io::ErrorKind> for PathIoErrorKind {
    fn from(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset => Self::ConnectionReset,
            std::io::ErrorKind::HostUnreachable => Self::HostUnreachable,
            std::io::ErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            std::io::ErrorKind::ConnectionAborted => Self::ConnectionAborted,
            std::io::ErrorKind::NotConnected => Self::NotConnected,
            std::io::ErrorKind::AddrInUse => Self::AddrInUse,
            std::io::ErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            std::io::ErrorKind::NetworkDown => Self::NetworkDown,
            std::io::ErrorKind::BrokenPipe => Self::BrokenPipe,
            std::io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            std::io::ErrorKind::WouldBlock => Self::WouldBlock,
            std::io::ErrorKind::NotADirectory => Self::NotADirectory,
            std::io::ErrorKind::IsADirectory => Self::IsADirectory,
            std::io::ErrorKind::DirectoryNotEmpty => Self::DirectoryNotEmpty,
            std::io::ErrorKind::ReadOnlyFilesystem => Self::ReadOnlyFilesystem,
            std::io::ErrorKind::StaleNetworkFileHandle => Self::StaleNetworkFileHandle,
            std::io::ErrorKind::InvalidInput => Self::InvalidInput,
            std::io::ErrorKind::InvalidData => Self::InvalidData,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            std::io::ErrorKind::WriteZero => Self::WriteZero,
            std::io::ErrorKind::StorageFull => Self::StorageFull,
            std::io::ErrorKind::NotSeekable => Self::NotSeekable,
            std::io::ErrorKind::QuotaExceeded => Self::QuotaExceeded,
            std::io::ErrorKind::FileTooLarge => Self::FileTooLarge,
            std::io::ErrorKind::ResourceBusy => Self::ResourceBusy,
            std::io::ErrorKind::ExecutableFileBusy => Self::ExecutableFileBusy,
            std::io::ErrorKind::Deadlock => Self::Deadlock,
            std::io::ErrorKind::CrossesDevices => Self::CrossesDevices,
            std::io::ErrorKind::TooManyLinks => Self::TooManyLinks,
            std::io::ErrorKind::InvalidFilename => Self::InvalidFilename,
            std::io::ErrorKind::ArgumentListTooLong => Self::ArgumentListTooLong,
            std::io::ErrorKind::Interrupted => Self::Interrupted,
            std::io::ErrorKind::Unsupported => Self::Unsupported,
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Dupe
)]
pub enum PathNodeKind {
    RegularFile,
    Directory,
    Symlink,
    SpecialFile,
}

/// The no-follow metadata retained from Bazel 9.2's `FileStatus` surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub struct PathLstat {
    kind: PathNodeKind,
    size: i64,
    mtime_millis: i64,
    ctime_millis: i64,
    node_id: i64,
    permissions: i32,
}

impl PathLstat {
    pub const fn new(
        kind: PathNodeKind,
        size: i64,
        mtime_millis: i64,
        ctime_millis: i64,
        node_id: i64,
        permissions: i32,
    ) -> Self {
        Self {
            kind,
            size,
            mtime_millis,
            ctime_millis,
            node_id,
            permissions,
        }
    }

    pub const fn kind(self) -> PathNodeKind {
        self.kind
    }

    pub const fn size(self) -> i64 {
        self.size
    }

    pub const fn mtime_millis(self) -> i64 {
        self.mtime_millis
    }

    pub const fn ctime_millis(self) -> i64 {
        self.ctime_millis
    }

    pub const fn node_id(self) -> i64 {
        self.node_id
    }

    /// POSIX mode bits, or `-1` when unavailable, matching Bazel `FileStatus`.
    pub const fn permissions(self) -> i32 {
        self.permissions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Allocative, Dupe)]
pub enum PathObservationError {
    Io {
        kind: PathIoErrorKind,
        raw_os_error: Option<i32>,
    },
    NotALink,
    WrongKind {
        expected: PathNodeKind,
        actual: PathNodeKind,
    },
    InconsistentState {
        before: Option<PathLstat>,
        after: Option<PathLstat>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum PathOperationResult<T> {
    Present(T),
    Missing,
    Error(PathObservationError),
}

/// One validated raw directory entry name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathDirectoryName {
    name: OsString,
}

impl PathDirectoryName {
    pub fn new(name: impl Into<OsString>) -> Result<Self, InvalidPathDirectoryName> {
        let name = name.into();
        let mut components = Path::new(&name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(InvalidPathDirectoryName { name });
        }
        Ok(Self { name })
    }

    pub fn as_os_str(&self) -> &OsStr {
        &self.name
    }
}

impl Allocative for PathDirectoryName {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if self.name.capacity() != 0 {
            {
                let mut allocation = visitor
                    .enter_unique(AllocativeKey::new("ptr"), std::mem::size_of::<*const u8>());
                let len = self.name.as_os_str().len();
                allocation.visit_simple(AllocativeKey::new("name"), len);
                allocation.visit_simple(
                    AllocativeKey::new("unused_capacity"),
                    self.name.capacity() - len,
                );
                allocation.exit();
            }
        }
        visitor.exit();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPathDirectoryName {
    name: OsString,
}

impl InvalidPathDirectoryName {
    pub fn name(&self) -> &OsStr {
        &self.name
    }
}

impl fmt::Display for InvalidPathDirectoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "directory observation name must be one normal OS-native component: {:?}",
            self.name
        )
    }
}

impl std::error::Error for InvalidPathDirectoryName {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicatePathDirectoryName {
    name: OsString,
}

impl DuplicatePathDirectoryName {
    pub fn name(&self) -> &OsStr {
        &self.name
    }
}

impl fmt::Display for DuplicatePathDirectoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "duplicate directory observation name: {:?}", self.name)
    }
}

impl std::error::Error for DuplicatePathDirectoryName {}

/// Sorted, unique raw OS-native direct directory entry names.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct PathDirectoryEntries {
    names: Arc<[PathDirectoryName]>,
}

impl PathDirectoryEntries {
    pub fn new(
        names: impl IntoIterator<Item = PathDirectoryName>,
    ) -> Result<Self, DuplicatePathDirectoryName> {
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort_unstable();
        if let Some(duplicate) = names
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0].name.clone())
        {
            return Err(DuplicatePathDirectoryName { name: duplicate });
        }
        Ok(Self {
            names: names.into(),
        })
    }

    pub fn names(&self) -> &[PathDirectoryName] {
        &self.names
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub enum PathObservationResult {
    Lstat(PathOperationResult<PathLstat>),
    ReadLink(PathOperationResult<Arc<PathBuf>>),
    FileBytes(PathOperationResult<Arc<[u8]>>),
    DirectoryEntries(PathOperationResult<PathDirectoryEntries>),
}

impl PathObservationResult {
    pub const fn operation(&self) -> PathObservationOperation {
        match self {
            Self::Lstat(_) => PathObservationOperation::Lstat,
            Self::ReadLink(_) => PathObservationOperation::ReadLink,
            Self::FileBytes(_) => PathObservationOperation::FileBytes,
            Self::DirectoryEntries(_) => PathObservationOperation::DirectoryEntries,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathObservationEpochError {
    DuplicateDemand(PathObservationDemand),
    OperationMismatch {
        demand: PathObservationDemand,
        result_operation: PathObservationOperation,
    },
}

impl fmt::Display for PathObservationEpochError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateDemand(demand) => write!(
                f,
                "duplicate path observation demand for {:?} ({:?})",
                demand.path(),
                demand.operation()
            ),
            Self::OperationMismatch {
                demand,
                result_operation,
            } => write!(
                f,
                "path observation operation mismatch for {:?}: demanded {:?}, observed {:?}",
                demand.path(),
                demand.operation(),
                result_operation
            ),
        }
    }
}

impl std::error::Error for PathObservationEpochError {}

/// The exact immutable operational observations injected for one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct PathObservationEpoch {
    observations: Arc<SortedMap<PathObservationDemand, Arc<PathObservationResult>>>,
}

impl PathObservationEpoch {
    pub fn empty() -> Self {
        Self {
            observations: Arc::new(SortedMap::new()),
        }
    }

    pub fn new(
        observations: impl IntoIterator<Item = (PathObservationDemand, PathObservationResult)>,
    ) -> Result<Self, PathObservationEpochError> {
        let mut observations = observations.into_iter().collect::<Vec<_>>();
        observations.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        for pair in observations.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(PathObservationEpochError::DuplicateDemand(pair[0].0.dupe()));
            }
        }
        for (demand, result) in &observations {
            if demand.operation() != result.operation() {
                return Err(PathObservationEpochError::OperationMismatch {
                    demand: demand.dupe(),
                    result_operation: result.operation(),
                });
            }
        }
        Ok(Self {
            observations: Arc::new(
                observations
                    .into_iter()
                    .map(|(demand, result)| (demand, Arc::new(result)))
                    .collect(),
            ),
        })
    }

    pub fn observations(&self) -> &SortedMap<PathObservationDemand, Arc<PathObservationResult>> {
        &self.observations
    }

    pub fn get(&self, demand: &PathObservationDemand) -> Option<&Arc<PathObservationResult>> {
        self.observations.get(demand)
    }
}

/// The singleton injected owner for the operational path epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct PathObservationEpochKey;

impl fmt::Display for PathObservationEpochKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("path-observation-epoch")
    }
}

impl InjectedKey for PathObservationEpochKey {
    type Value = PathObservationEpoch;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// The transient DICE projection for one exact observation demand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct PathObservationKey {
    demand: PathObservationDemand,
}

impl PathObservationKey {
    pub fn new(demand: PathObservationDemand) -> Self {
        Self { demand }
    }

    pub fn demand(&self) -> &PathObservationDemand {
        &self.demand
    }
}

impl fmt::Display for PathObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "path-observation:{:?}:{:?}:{:?}",
            self.demand.namespace(),
            self.demand.path().as_path(),
            self.demand.operation()
        )
    }
}

#[async_trait]
impl Key for PathObservationKey {
    type Value = PathOutcome<Arc<PathObservationResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let epoch = match ctx.compute(&PathObservationEpochKey).await {
            Ok(epoch) => epoch,
            Err(_) => {
                return PathOutcome::Need(NeedPathObservations::singleton(self.demand.dupe()));
            }
        };
        match epoch.get(&self.demand) {
            Some(result) => PathOutcome::Complete(result.dupe()),
            None => PathOutcome::Need(NeedPathObservations::singleton(self.demand.dupe())),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use dice::DetectCycles;
    use dice::Dice;
    use dice::InjectedKey;
    use dice::Key;
    use dupe::Dupe;

    use super::NeedPathObservations;
    use super::NormalizedAbsolutePath;
    use super::PathDirectoryEntries;
    use super::PathDirectoryName;
    use super::PathIoErrorKind;
    use super::PathLstat;
    use super::PathNodeKind;
    use super::PathObservationDemand;
    use super::PathObservationEpoch;
    use super::PathObservationEpochError;
    use super::PathObservationEpochKey;
    use super::PathObservationError;
    use super::PathObservationInstanceId;
    use super::PathObservationKey;
    use super::PathObservationNamespace;
    use super::PathObservationOperation;
    use super::PathObservationResult;
    use super::PathOperationResult;
    use super::PathOutcome;

    fn path(value: &str) -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new(value).unwrap()
    }

    fn demand(
        namespace: PathObservationNamespace,
        value: &str,
        operation: PathObservationOperation,
    ) -> PathObservationDemand {
        PathObservationDemand::new(namespace, path(value), operation)
    }

    fn file_bytes(bytes: &'static [u8]) -> PathObservationResult {
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes)))
    }

    #[test]
    fn path_observation_normalizes_lexically_and_clamps_only_at_root() {
        assert_eq!(
            path("/workspace/repo/../../outside/./file").as_path(),
            Path::new("/outside/file")
        );
        assert_eq!(path("/../../outside").as_path(), Path::new("/outside"));
        assert_eq!(path("/workspace/../..").as_path(), Path::new("/"));
        assert!(NormalizedAbsolutePath::new("relative/path").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn path_observation_preserves_windows_absolute_anchors() {
        assert_eq!(
            NormalizedAbsolutePath::new(r"C:\workspace\..\outside")
                .unwrap()
                .as_path(),
            Path::new(r"C:\outside")
        );
        assert_eq!(
            NormalizedAbsolutePath::new(r"\\server\share\workspace\..\outside")
                .unwrap()
                .as_path(),
            Path::new(r"\\server\share\outside")
        );
        assert_eq!(
            NormalizedAbsolutePath::new(r"\\?\C:\workspace\..\outside")
                .unwrap()
                .as_path(),
            Path::new(r"\\?\C:\outside")
        );
        assert!(NormalizedAbsolutePath::new(r"C:relative").is_err());
        assert!(NormalizedAbsolutePath::new(r"\root-relative").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn path_observation_preserves_non_utf8_path_link_and_directory_names() {
        use std::os::unix::ffi::OsStringExt;

        let mut absolute = OsString::from("/");
        absolute.push(OsString::from_vec(vec![b'a', 0xff]));
        let normalized = NormalizedAbsolutePath::new(PathBuf::from(absolute)).unwrap();
        assert_eq!(
            normalized.as_path().as_os_str().as_encoded_bytes(),
            &[b'/', b'a', 0xff]
        );

        let target = Arc::new(PathBuf::from(OsString::from_vec(vec![b'.', b'/', 0xfe])));
        let observed = PathObservationResult::ReadLink(PathOperationResult::Present(target.dupe()));
        let PathObservationResult::ReadLink(PathOperationResult::Present(actual)) = observed else {
            panic!("expected raw link target");
        };
        assert_eq!(actual.as_os_str().as_encoded_bytes(), &[b'.', b'/', 0xfe]);

        let name = PathDirectoryName::new(OsString::from_vec(vec![b'n', 0xfd])).unwrap();
        let entries = PathDirectoryEntries::new([name]).unwrap();
        assert_eq!(
            entries.names()[0].as_os_str().as_encoded_bytes(),
            &[b'n', 0xfd]
        );
    }

    #[test]
    fn path_observation_demand_identity_includes_namespace_path_and_operation() {
        let host = demand(
            PathObservationNamespace::Host,
            "/workspace/file",
            PathObservationOperation::Lstat,
        );
        let materialized = demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            "/workspace/file",
            PathObservationOperation::Lstat,
        );
        let other_path = demand(
            PathObservationNamespace::Host,
            "/workspace/other",
            PathObservationOperation::Lstat,
        );
        let other_operation = demand(
            PathObservationNamespace::Host,
            "/workspace/file",
            PathObservationOperation::FileBytes,
        );
        assert_ne!(host, materialized);
        assert_ne!(host, other_path);
        assert_ne!(host, other_operation);
    }

    #[test]
    fn path_observation_need_is_nonempty_sorted_deduplicated_union() {
        let one = demand(
            PathObservationNamespace::Host,
            "/workspace/a",
            PathObservationOperation::FileBytes,
        );
        let two = demand(
            PathObservationNamespace::Host,
            "/workspace/b",
            PathObservationOperation::Lstat,
        );
        assert!(NeedPathObservations::try_from_iter([]).is_err());
        let left =
            NeedPathObservations::try_from_iter([two.dupe(), one.dupe(), two.dupe()]).unwrap();
        assert_eq!(left.demands(), &[one.dupe(), two.dupe()]);
        let right = NeedPathObservations::singleton(two.dupe());
        assert_eq!(left.union(&right).demands(), &[one, two]);
    }

    #[test]
    fn path_observation_outcome_maps_and_need_is_never_complete_equal() {
        let demand = demand(
            PathObservationNamespace::Host,
            "/workspace/file",
            PathObservationOperation::FileBytes,
        );
        let complete = PathOutcome::Complete(2u32);
        assert!(complete.complete_eq(&PathOutcome::Complete(2)));
        assert_eq!(
            match complete.map(|value| value + 1) {
                PathOutcome::Complete(value) => value,
                PathOutcome::Need(_) => panic!("complete became need"),
            },
            3
        );
        let need = PathOutcome::<u32>::Need(NeedPathObservations::singleton(demand));
        assert!(!need.is_complete());
        assert!(!need.complete_eq(&need));
        let still_need = need.and_then(|_| PathOutcome::Complete(4));
        assert!(matches!(still_need, PathOutcome::Need(_)));
    }

    #[test]
    fn path_observation_directory_entries_sort_and_reject_duplicates() {
        let a = PathDirectoryName::new("a").unwrap();
        let b = PathDirectoryName::new("b").unwrap();
        let entries = PathDirectoryEntries::new([b, a]).unwrap();
        assert_eq!(entries.names()[0].as_os_str(), OsString::from("a"));
        assert_eq!(entries.names()[1].as_os_str(), OsString::from("b"));
        assert!(
            PathDirectoryEntries::new([
                PathDirectoryName::new("same").unwrap(),
                PathDirectoryName::new("same").unwrap(),
            ])
            .is_err()
        );
        assert!(PathDirectoryName::new("../bad").is_err());
    }

    #[test]
    fn path_observation_results_cover_operations_missing_and_errors() {
        let before = PathLstat::new(PathNodeKind::RegularFile, 4, 10, 11, 12, 0o755);
        let after = PathLstat::new(PathNodeKind::RegularFile, 5, 20, 21, 12, -1);
        assert_eq!(before.size(), 4);
        assert_eq!(before.mtime_millis(), 10);
        assert_eq!(before.ctime_millis(), 11);
        assert_eq!(before.node_id(), 12);
        assert_eq!(before.permissions(), 0o755);
        assert_eq!(after.permissions(), -1);
        assert_eq!(before.kind(), PathNodeKind::RegularFile);

        let io = PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        let not_a_link = PathObservationError::NotALink;
        let wrong_kind = PathObservationError::WrongKind {
            expected: PathNodeKind::Directory,
            actual: PathNodeKind::RegularFile,
        };
        let inconsistent = PathObservationError::InconsistentState {
            before: Some(before),
            after: Some(after),
        };
        let results = [
            PathObservationResult::Lstat(PathOperationResult::Present(before)),
            PathObservationResult::ReadLink(PathOperationResult::Missing),
            PathObservationResult::FileBytes(PathOperationResult::Error(io)),
            PathObservationResult::DirectoryEntries(PathOperationResult::Error(wrong_kind)),
        ];
        assert_eq!(
            results.map(|result| result.operation()),
            [
                PathObservationOperation::Lstat,
                PathObservationOperation::ReadLink,
                PathObservationOperation::FileBytes,
                PathObservationOperation::DirectoryEntries,
            ]
        );
        assert_ne!(io, wrong_kind);
        assert_eq!(not_a_link, PathObservationError::NotALink);
        assert_ne!(not_a_link, io);
        assert_ne!(not_a_link, wrong_kind);
        assert_ne!(not_a_link, inconsistent);
        assert_ne!(wrong_kind, inconsistent);
        assert_eq!(
            PathIoErrorKind::from(std::io::ErrorKind::ConnectionReset),
            PathIoErrorKind::ConnectionReset
        );
        let not_a_directory = PathObservationError::Io {
            kind: PathIoErrorKind::from(std::io::ErrorKind::NotADirectory),
            raw_os_error: None,
        };
        let is_a_directory = PathObservationError::Io {
            kind: PathIoErrorKind::from(std::io::ErrorKind::IsADirectory),
            raw_os_error: None,
        };
        assert_ne!(not_a_directory, is_a_directory);
        assert_eq!(
            PathObservationResult::Lstat(PathOperationResult::Error(inconsistent)).operation(),
            PathObservationOperation::Lstat
        );
        assert_ne!(PathNodeKind::SpecialFile, PathNodeKind::RegularFile);
    }

    #[test]
    fn path_observation_epoch_rejects_duplicate_and_mismatched_operations() {
        let demand = demand(
            PathObservationNamespace::Host,
            "/workspace/file",
            PathObservationOperation::FileBytes,
        );
        assert!(matches!(
            PathObservationEpoch::new([
                (demand.dupe(), file_bytes(b"a")),
                (demand.dupe(), file_bytes(b"b")),
            ]),
            Err(PathObservationEpochError::DuplicateDemand(_))
        ));
        assert!(matches!(
            PathObservationEpoch::new([(
                demand,
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )]),
            Err(PathObservationEpochError::OperationMismatch { .. })
        ));
    }

    #[tokio::test]
    async fn path_observation_dice_distinguishes_need_observed_missing_and_change() {
        let present = demand(
            PathObservationNamespace::Host,
            "/workspace/file",
            PathObservationOperation::FileBytes,
        );
        let absent_demand = demand(
            PathObservationNamespace::Host,
            "/workspace/other",
            PathObservationOperation::FileBytes,
        );

        let uninjected_dice = Dice::builder().build(DetectCycles::Enabled);
        let mut uninjected_transaction = uninjected_dice.updater().commit().await;
        let uninjected = uninjected_transaction
            .compute(&PathObservationKey::new(present.dupe()))
            .await
            .unwrap();
        let PathOutcome::Need(uninjected_need) = &uninjected else {
            panic!("an uninjected epoch must request the exact demand");
        };
        assert_eq!(uninjected_need.demands(), &[present.dupe()]);
        assert!(!PathObservationKey::validity(&uninjected));

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        let epoch = PathObservationEpoch::new([(
            present.dupe(),
            PathObservationResult::FileBytes(PathOperationResult::Missing),
        )])
        .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;

        let observed = transaction
            .compute(&PathObservationKey::new(present.dupe()))
            .await
            .unwrap();
        assert!(matches!(
            observed,
            PathOutcome::Complete(result)
                if matches!(
                    result.as_ref(),
                    PathObservationResult::FileBytes(PathOperationResult::Missing)
                )
        ));
        let needed = transaction
            .compute(&PathObservationKey::new(absent_demand))
            .await
            .unwrap();
        assert!(matches!(needed, PathOutcome::Need(_)));
        assert!(!PathObservationKey::validity(&needed));
        assert!(!PathObservationKey::equality(&needed, &needed));

        let mut updater = transaction.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([(present.dupe(), file_bytes(b"changed"))]).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let changed = transaction
            .compute(&PathObservationKey::new(present))
            .await
            .unwrap();
        assert!(matches!(
            changed,
            PathOutcome::Complete(result)
                if matches!(
                    result.as_ref(),
                    PathObservationResult::FileBytes(
                        PathOperationResult::Present(bytes)
                    ) if bytes.as_ref() == b"changed"
                )
        ));
    }

    #[test]
    fn path_observation_epoch_equality_is_exact_but_payload_can_be_semantically_equal() {
        let workspace = path("/workspace/file");
        let demand_for = |instance| {
            PathObservationDemand::new(
                PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
                workspace.dupe(),
                PathObservationOperation::FileBytes,
            )
        };
        let old_result = file_bytes(b"same");
        let new_result = file_bytes(b"same");
        let old = PathObservationEpoch::new([(demand_for(1), old_result.dupe())]).unwrap();
        let new = PathObservationEpoch::new([(demand_for(2), new_result.dupe())]).unwrap();
        assert!(!<PathObservationEpochKey as InjectedKey>::equality(
            &old, &new
        ));

        let old_payload = PathOutcome::Complete(Arc::new(old_result));
        let new_payload = PathOutcome::Complete(Arc::new(new_result));
        assert!(old_payload.complete_eq(&new_payload));
    }
}
