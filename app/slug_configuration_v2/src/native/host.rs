use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct HomeFact {
    occurrence: u32,
    home: CompactString,
}

impl HomeFact {
    pub fn new(occurrence: u32, home: CompactString) -> Self {
        Self { occurrence, home }
    }

    pub fn occurrence(&self) -> u32 {
        self.occurrence
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
    raw: Arc<[u16]>,
    outcome: WindowsOptionPathOutcome,
}

impl WindowsOptionPathFact {
    pub fn new(raw: Arc<[u16]>, outcome: WindowsOptionPathOutcome) -> Self {
        Self { raw, outcome }
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
    DuplicateHomeOccurrence,
    OutOfOrderHomeOccurrence,
    DuplicateWindowsOptionPathRaw,
    OutOfOrderWindowsOptionPathRaw,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct HostConversionInputsData {
    auto_cpu: Option<AutoCpuToken>,
    path_flavor: Option<HostPathFlavor>,
    capacity: Option<HostCapacity>,
    home_facts: Arc<[HomeFact]>,
    windows_option_path_facts: Arc<[WindowsOptionPathFact]>,
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
}

fn validate_home_facts(facts: &[HomeFact]) -> Result<(), HostConversionInputsError> {
    for pair in facts.windows(2) {
        match pair[0].occurrence.cmp(&pair[1].occurrence) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(HostConversionInputsError::DuplicateHomeOccurrence);
            }
            std::cmp::Ordering::Greater => {
                return Err(HostConversionInputsError::OutOfOrderHomeOccurrence);
            }
        }
    }
    Ok(())
}

fn validate_windows_option_path_facts(
    facts: &[WindowsOptionPathFact],
) -> Result<(), HostConversionInputsError> {
    for pair in facts.windows(2) {
        match pair[0].raw.cmp(&pair[1].raw) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(HostConversionInputsError::DuplicateWindowsOptionPathRaw);
            }
            std::cmp::Ordering::Greater => {
                return Err(HostConversionInputsError::OutOfOrderWindowsOptionPathRaw);
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
    fn tokens_flavors_and_capacity_are_structural() {
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
        for (index, token) in AutoCpuToken::ALL.iter().enumerate() {
            assert_eq!(
                AutoCpuToken::ALL
                    .iter()
                    .filter(|other| *other == token)
                    .count(),
                1
            );
            assert_eq!(token.cmp(token), std::cmp::Ordering::Equal);
            assert_eq!(hash(token), hash(&AutoCpuToken::ALL[index]));
            assert_eq!(token.as_str(), spellings[index]);
        }
        assert_ne!(HostPathFlavor::Unix, HostPathFlavor::Windows);
        let capacity = HostCapacity::new(i32::MIN, i32::MAX);
        assert_eq!(
            (capacity.host_cpus(), capacity.host_ram_mib()),
            (i32::MIN, i32::MAX)
        );
    }

    #[test]
    fn home_facts_require_strict_occurrence_order() {
        let ordered = Arc::from([
            HomeFact::new(2, CompactString::new("two")),
            HomeFact::new(9, CompactString::new("nine")),
        ]);
        assert_eq!(
            inputs(ordered, Arc::from([])).home_facts()[1].home(),
            "nine"
        );
        assert_eq!(
            HostConversionInputs::new(
                None,
                None,
                None,
                Arc::from([
                    HomeFact::new(2, CompactString::new("a")),
                    HomeFact::new(2, CompactString::new("b"))
                ]),
                Arc::from([]),
            ),
            Err(HostConversionInputsError::DuplicateHomeOccurrence),
        );
        assert_eq!(
            HostConversionInputs::new(
                None,
                None,
                None,
                Arc::from([
                    HomeFact::new(3, CompactString::new("a")),
                    HomeFact::new(2, CompactString::new("b"))
                ]),
                Arc::from([]),
            ),
            Err(HostConversionInputsError::OutOfOrderHomeOccurrence),
        );
    }

    #[test]
    fn windows_facts_preserve_raw_order_and_outcome() {
        let unpaired = WindowsOptionPathFact::new(
            raw(&[0xd800]),
            WindowsOptionPathOutcome::IOExceptionFallback,
        );
        let resolved = WindowsOptionPathFact::new(
            raw(&[0xd801]),
            WindowsOptionPathOutcome::Resolved(raw(&[0xd802])),
        );
        let value = inputs(
            Arc::from([]),
            Arc::from([unpaired.clone(), resolved.clone()]),
        );
        assert_eq!(value.windows_option_path_facts()[0].raw(), [0xd800]);
        assert_ne!(resolved.outcome(), unpaired.outcome());
        for (facts, error) in [
            (
                Arc::from([resolved.clone(), unpaired.clone()]),
                HostConversionInputsError::OutOfOrderWindowsOptionPathRaw,
            ),
            (
                Arc::from([
                    unpaired.clone(),
                    WindowsOptionPathFact::new(
                        raw(&[0xd800]),
                        WindowsOptionPathOutcome::Resolved(raw(&[0xd803])),
                    ),
                ]),
                HostConversionInputsError::DuplicateWindowsOptionPathRaw,
            ),
        ] {
            assert_eq!(
                HostConversionInputs::new(None, None, None, Arc::from([]), facts),
                Err(error)
            );
        }
    }

    #[test]
    fn aggregate_is_arc_backed_and_structural() {
        fn allocative<T: Allocative>() {}
        fn dupe<T: Dupe>() {}
        allocative::<AutoCpuToken>();
        allocative::<HostPathFlavor>();
        allocative::<HostCapacity>();
        allocative::<WindowsOptionPathOutcome>();
        allocative::<HostConversionInputsError>();
        allocative::<HostConversionInputs>();
        allocative::<HomeFact>();
        allocative::<WindowsOptionPathFact>();
        dupe::<HostConversionInputs>();

        let make = |auto_cpu, path_flavor, capacity, occurrence, home, raw_input, outcome| {
            HostConversionInputs::new(
                Some(auto_cpu),
                Some(path_flavor),
                Some(capacity),
                Arc::from([HomeFact::new(occurrence, CompactString::new(home))]),
                Arc::from([WindowsOptionPathFact::new(raw(raw_input), outcome)]),
            )
            .unwrap()
        };
        let original = make(
            AutoCpuToken::K8,
            HostPathFlavor::Unix,
            HostCapacity::new(2, 8),
            1,
            "home",
            &[1],
            WindowsOptionPathOutcome::Resolved(raw(&[2])),
        );
        let duplicate = original.dupe();
        assert_eq!(original, duplicate);
        assert_eq!(hash(&original), hash(&duplicate));
        assert!(std::ptr::eq(original.0.as_ref(), duplicate.0.as_ref()));

        let changed = [
            make(
                AutoCpuToken::Unknown,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                1,
                "home",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Windows,
                HostCapacity::new(2, 8),
                1,
                "home",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(3, 8),
                1,
                "home",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                2,
                "home",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                1,
                "other",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                1,
                "home",
                &[3],
                WindowsOptionPathOutcome::Resolved(raw(&[2])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                1,
                "home",
                &[1],
                WindowsOptionPathOutcome::Resolved(raw(&[3])),
            ),
            make(
                AutoCpuToken::K8,
                HostPathFlavor::Unix,
                HostCapacity::new(2, 8),
                1,
                "home",
                &[1],
                WindowsOptionPathOutcome::IOExceptionFallback,
            ),
        ];
        for value in changed {
            assert_ne!(original, value);
            assert_ne!(original.cmp(&value), std::cmp::Ordering::Equal);
            assert_ne!(hash(&original), hash(&value));
        }
    }
}
