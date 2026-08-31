use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
#[repr(u16)]
pub enum ActionEnvironmentHostOs {
    Linux = 0x0710,
    Windows = 0x0711,
    Macos = 0x0712,
    Freebsd = 0x0713,
    Openbsd = 0x0714,
    Unknown = 0x0715,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct ActionEnvironmentHostData {
    os: ActionEnvironmentHostOs,
    bazel_sh: Option<CompactString>,
    path: Option<CompactString>,
    system_root: Option<CompactString>,
}

/// Process-latched Host facts consumed by Bazel's configured shell environment.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe)]
pub struct ActionEnvironmentHost(Arc<ActionEnvironmentHostData>);

impl ActionEnvironmentHost {
    pub fn without_environment(os: ActionEnvironmentHostOs) -> Self {
        Self(Arc::new(ActionEnvironmentHostData {
            os,
            bazel_sh: None,
            path: None,
            system_root: None,
        }))
    }

    pub fn windows(bazel_sh: Option<&str>, path: Option<&str>, system_root: Option<&str>) -> Self {
        Self(Arc::new(ActionEnvironmentHostData {
            os: ActionEnvironmentHostOs::Windows,
            bazel_sh: bazel_sh.map(CompactString::new),
            path: path.map(CompactString::new),
            system_root: system_root.map(CompactString::new),
        }))
    }

    pub fn os(&self) -> ActionEnvironmentHostOs {
        self.0.os
    }

    pub fn bazel_sh(&self) -> Option<&str> {
        self.0.bazel_sh.as_deref()
    }

    pub fn path(&self) -> Option<&str> {
        self.0.path.as_deref()
    }

    pub fn system_root(&self) -> Option<&str> {
        self.0.system_root.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum AutoCpuToken {
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

impl AutoCpuToken {
    pub const ALL: [Self; 15] = [
        Self::DarwinX86_64,
        Self::DarwinArm64,
        Self::Freebsd,
        Self::Openbsd,
        Self::X64Windows,
        Self::Arm64Windows,
        Self::Piii,
        Self::K8,
        Self::Ppc,
        Self::Arm,
        Self::Aarch64,
        Self::S390x,
        Self::Mips64,
        Self::Riscv64,
        Self::Unknown,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DarwinX86_64 => "darwin_x86_64",
            Self::DarwinArm64 => "darwin_arm64",
            Self::Freebsd => "freebsd",
            Self::Openbsd => "openbsd",
            Self::X64Windows => "x64_windows",
            Self::Arm64Windows => "arm64_windows",
            Self::Piii => "piii",
            Self::K8 => "k8",
            Self::Ppc => "ppc",
            Self::Arm => "arm",
            Self::Aarch64 => "aarch64",
            Self::S390x => "s390x",
            Self::Mips64 => "mips64",
            Self::Riscv64 => "riscv64",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum HostPathFlavor {
    Unix,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct HostCapacity {
    host_cpus: i32,
    host_ram_mib: i32,
}

impl HostCapacity {
    pub fn new(host_cpus: i32, host_ram_mib: i32) -> Self {
        Self {
            host_cpus,
            host_ram_mib,
        }
    }

    pub fn host_cpus(self) -> i32 {
        self.host_cpus
    }

    pub fn host_ram_mib(self) -> i32 {
        self.host_ram_mib
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConverterCallId(u32);

impl ConverterCallId {
    pub const fn first() -> Self {
        Self(0)
    }

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct HomeFact {
    call_id: ConverterCallId,
    home: CompactString,
}

impl HomeFact {
    pub fn new(call_id: ConverterCallId, home: CompactString) -> Self {
        Self { call_id, home }
    }

    pub const fn call_id(&self) -> ConverterCallId {
        self.call_id
    }

    pub fn home(&self) -> &str {
        &self.home
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum WindowsOptionPathOutcome {
    Resolved(Arc<[u16]>),
    IOExceptionFallback,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct WindowsOptionPathFact {
    call_id: ConverterCallId,
    raw: Arc<[u16]>,
    outcome: WindowsOptionPathOutcome,
}

impl WindowsOptionPathFact {
    pub fn new(
        call_id: ConverterCallId,
        raw: Arc<[u16]>,
        outcome: WindowsOptionPathOutcome,
    ) -> Self {
        Self {
            call_id,
            raw,
            outcome,
        }
    }

    pub const fn call_id(&self) -> ConverterCallId {
        self.call_id
    }

    pub fn raw(&self) -> &[u16] {
        &self.raw
    }

    pub fn outcome(&self) -> &WindowsOptionPathOutcome {
        &self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum HostConversionInputsError {
    DuplicateHomeCallId,
    OutOfOrderHomeCallId,
    DuplicateWindowsOptionPathCallId,
    OutOfOrderWindowsOptionPathCallId,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct HostConversionInputsData {
    auto_cpu: Option<AutoCpuToken>,
    path_flavor: Option<HostPathFlavor>,
    capacity: Option<HostCapacity>,
    home_facts: Arc<[HomeFact]>,
    windows_option_path_facts: Arc<[WindowsOptionPathFact]>,
    action_environment_host: Option<ActionEnvironmentHost>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative, Dupe)]
pub struct HostConversionInputs(Arc<HostConversionInputsData>);

impl HostConversionInputs {
    pub fn new(
        auto_cpu: Option<AutoCpuToken>,
        path_flavor: Option<HostPathFlavor>,
        capacity: Option<HostCapacity>,
        home_facts: Arc<[HomeFact]>,
        windows_option_path_facts: Arc<[WindowsOptionPathFact]>,
    ) -> Result<Self, HostConversionInputsError> {
        validate_home_facts(&home_facts)?;
        validate_windows_option_path_facts(&windows_option_path_facts)?;
        Ok(Self(Arc::new(HostConversionInputsData {
            auto_cpu,
            path_flavor,
            capacity,
            home_facts,
            windows_option_path_facts,
            action_environment_host: None,
        })))
    }

    pub fn auto_cpu(&self) -> Option<AutoCpuToken> {
        self.0.auto_cpu
    }

    pub fn path_flavor(&self) -> Option<HostPathFlavor> {
        self.0.path_flavor
    }

    pub fn capacity(&self) -> Option<HostCapacity> {
        self.0.capacity
    }

    pub fn home_facts(&self) -> &[HomeFact] {
        &self.0.home_facts
    }

    pub fn windows_option_path_facts(&self) -> &[WindowsOptionPathFact] {
        &self.0.windows_option_path_facts
    }

    pub fn with_action_environment_host(&self, host: ActionEnvironmentHost) -> Self {
        let mut data = self.0.as_ref().clone();
        data.action_environment_host = Some(host);
        Self(Arc::new(data))
    }

    pub fn action_environment_host(&self) -> Option<&ActionEnvironmentHost> {
        self.0.action_environment_host.as_ref()
    }
}

fn validate_home_facts(facts: &[HomeFact]) -> Result<(), HostConversionInputsError> {
    for pair in facts.windows(2) {
        match pair[0].call_id.cmp(&pair[1].call_id) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(HostConversionInputsError::DuplicateHomeCallId);
            }
            std::cmp::Ordering::Greater => {
                return Err(HostConversionInputsError::OutOfOrderHomeCallId);
            }
        }
    }
    Ok(())
}

fn validate_windows_option_path_facts(
    facts: &[WindowsOptionPathFact],
) -> Result<(), HostConversionInputsError> {
    for pair in facts.windows(2) {
        match pair[0].call_id.cmp(&pair[1].call_id) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(HostConversionInputsError::DuplicateWindowsOptionPathCallId);
            }
            std::cmp::Ordering::Greater => {
                return Err(HostConversionInputsError::OutOfOrderWindowsOptionPathCallId);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use allocative::Allocative;
    use compact_str::CompactString;
    use dupe::Dupe;

    use super::*;

    fn raw(values: &[u16]) -> Arc<[u16]> {
        Arc::from(values)
    }

    fn call(value: u32) -> ConverterCallId {
        ConverterCallId::new(value)
    }

    fn inputs(
        home_facts: Arc<[HomeFact]>,
        windows_facts: Arc<[WindowsOptionPathFact]>,
    ) -> HostConversionInputs {
        HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            Some(HostCapacity::new(i32::MIN, i32::MAX)),
            home_facts,
            windows_facts,
        )
        .unwrap()
    }

    fn hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn tokens_process_facts_and_call_ids_are_structural() {
        let spellings = [
            "darwin_x86_64",
            "darwin_arm64",
            "freebsd",
            "openbsd",
            "x64_windows",
            "arm64_windows",
            "piii",
            "k8",
            "ppc",
            "arm",
            "aarch64",
            "s390x",
            "mips64",
            "riscv64",
            "unknown",
        ];
        assert_eq!(AutoCpuToken::ALL.len(), spellings.len());
        for (token, spelling) in AutoCpuToken::ALL.into_iter().zip(spellings) {
            assert_eq!(token.as_str(), spelling);
        }
        assert_ne!(HostPathFlavor::Unix, HostPathFlavor::Windows);
        assert_eq!(
            HostCapacity::new(i32::MIN, i32::MAX),
            HostCapacity::new(i32::MIN, i32::MAX)
        );
        assert_eq!(ConverterCallId::first().next(), Some(call(1)));
        assert_eq!(call(u32::MAX).next(), None);
        assert_eq!(call(7).value(), 7);
        assert_eq!(hash(&call(7)), hash(&call(7)));
    }

    #[test]
    fn home_facts_require_strict_call_id_order_but_not_consecutiveness() {
        let value = inputs(
            Arc::from([
                HomeFact::new(call(2), CompactString::new("two")),
                HomeFact::new(call(9), CompactString::new("nine")),
            ]),
            Arc::from([]),
        );
        assert_eq!(value.home_facts()[1].call_id(), call(9));
        assert_eq!(value.home_facts()[1].home(), "nine");
        for (facts, error) in [
            (
                Arc::from([
                    HomeFact::new(call(2), CompactString::new("a")),
                    HomeFact::new(call(2), CompactString::new("b")),
                ]),
                HostConversionInputsError::DuplicateHomeCallId,
            ),
            (
                Arc::from([
                    HomeFact::new(call(3), CompactString::new("a")),
                    HomeFact::new(call(2), CompactString::new("b")),
                ]),
                HostConversionInputsError::OutOfOrderHomeCallId,
            ),
        ] {
            assert_eq!(
                HostConversionInputs::new(None, None, None, facts, Arc::from([])),
                Err(error)
            );
        }
    }

    #[test]
    fn windows_facts_keep_duplicate_raw_outcomes_and_share_call_ids_with_home() {
        let fallback = WindowsOptionPathFact::new(
            call(2),
            raw(&[0xd800]),
            WindowsOptionPathOutcome::IOExceptionFallback,
        );
        let resolved = WindowsOptionPathFact::new(
            call(9),
            raw(&[0xd800]),
            WindowsOptionPathOutcome::Resolved(raw(&[0xd802])),
        );
        let value = inputs(
            Arc::from([HomeFact::new(call(2), CompactString::new("home"))]),
            Arc::from([fallback.clone(), resolved.clone()]),
        );
        assert_eq!(value.home_facts()[0].call_id(), call(2));
        assert_eq!(value.windows_option_path_facts()[0].call_id(), call(2));
        assert_eq!(value.windows_option_path_facts()[1].raw(), [0xd800]);
        assert_ne!(fallback.outcome(), resolved.outcome());
        for (facts, error) in [
            (
                Arc::from([resolved.clone(), fallback.clone()]),
                HostConversionInputsError::OutOfOrderWindowsOptionPathCallId,
            ),
            (
                Arc::from([
                    fallback.clone(),
                    WindowsOptionPathFact::new(
                        call(2),
                        raw(&[0xd801]),
                        WindowsOptionPathOutcome::Resolved(raw(&[0xd803])),
                    ),
                ]),
                HostConversionInputsError::DuplicateWindowsOptionPathCallId,
            ),
        ] {
            assert_eq!(
                HostConversionInputs::new(None, None, None, Arc::from([]), facts),
                Err(error)
            );
        }
    }

    #[test]
    fn aggregate_is_arc_backed_structural_and_allows_empty_process_schedule() {
        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<AutoCpuToken>();
        allocative::<HostPathFlavor>();
        allocative::<HostCapacity>();
        allocative::<ActionEnvironmentHostOs>();
        allocative::<ActionEnvironmentHost>();
        allocative::<ConverterCallId>();
        allocative::<HomeFact>();
        allocative::<WindowsOptionPathOutcome>();
        allocative::<WindowsOptionPathFact>();
        allocative::<HostConversionInputsError>();
        allocative::<HostConversionInputs>();
        dupe::<ActionEnvironmentHost>();
        dupe::<HostConversionInputs>();

        let empty =
            HostConversionInputs::new(None, None, None, Arc::from([]), Arc::from([])).unwrap();
        assert_eq!(empty.auto_cpu(), None);
        assert_eq!(empty.path_flavor(), None);
        assert_eq!(empty.capacity(), None);
        assert_eq!(empty.action_environment_host(), None);

        let windows = ActionEnvironmentHost::windows(
            Some("D:/bash.exe"),
            Some("C:/bin"),
            Some("D:\\Windows"),
        );
        let with_environment = empty.with_action_environment_host(windows.dupe());
        assert_eq!(with_environment.action_environment_host(), Some(&windows));
        assert_eq!(windows.os(), ActionEnvironmentHostOs::Windows);
        assert_eq!(windows.bazel_sh(), Some("D:/bash.exe"));

        let make = |home_call, windows_call, outcome| {
            HostConversionInputs::new(
                Some(AutoCpuToken::K8),
                Some(HostPathFlavor::Unix),
                Some(HostCapacity::new(2, 8)),
                Arc::from([HomeFact::new(call(home_call), CompactString::new("home"))]),
                Arc::from([WindowsOptionPathFact::new(
                    call(windows_call),
                    raw(&[1]),
                    outcome,
                )]),
            )
            .unwrap()
        };
        let original = make(1, 1, WindowsOptionPathOutcome::Resolved(raw(&[2])));
        let duplicate = original.dupe();
        assert_eq!(original, duplicate);
        assert_eq!(hash(&original), hash(&duplicate));
        assert!(std::ptr::eq(original.0.as_ref(), duplicate.0.as_ref()));

        let with_changed = |data: HostConversionInputsData| HostConversionInputs(Arc::new(data));
        for changed in [
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.auto_cpu = Some(AutoCpuToken::Unknown);
                data
            }),
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.path_flavor = Some(HostPathFlavor::Windows);
                data
            }),
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.capacity = Some(HostCapacity::new(3, 8));
                data
            }),
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.home_facts = Arc::from([HomeFact::new(call(1), CompactString::new("other"))]);
                data
            }),
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.windows_option_path_facts = Arc::from([WindowsOptionPathFact::new(
                    call(1),
                    raw(&[3]),
                    WindowsOptionPathOutcome::Resolved(raw(&[2])),
                )]);
                data
            }),
            with_changed({
                let mut data = original.0.as_ref().clone();
                data.action_environment_host = Some(ActionEnvironmentHost::without_environment(
                    ActionEnvironmentHostOs::Linux,
                ));
                data
            }),
            make(2, 1, WindowsOptionPathOutcome::Resolved(raw(&[2]))),
            make(1, 2, WindowsOptionPathOutcome::Resolved(raw(&[2]))),
            make(1, 1, WindowsOptionPathOutcome::Resolved(raw(&[3]))),
            make(1, 1, WindowsOptionPathOutcome::IOExceptionFallback),
        ] {
            assert_ne!(original, changed);
            assert_ne!(original.cmp(&changed), std::cmp::Ordering::Equal);
            assert_ne!(hash(&original), hash(&changed));
        }
    }
}
