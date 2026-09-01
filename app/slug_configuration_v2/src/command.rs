//! Immutable command inputs which can affect structural configuration.

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use serde::Deserialize;
use serde::Serialize;

/// The closed Bazel 9.2 native-option set currently admitted at the command
/// boundary. Keeping this typed prevents command capture from becoming an
/// unchecked string-to-configuration mutation API.
#[repr(u8)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Allocative,
    Serialize,
    Deserialize
)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandOption {
    CompilationMode,
    HostCompilationMode,
    FdoOptimize,
    XbinaryFdo,
    FdoProfile,
    CsFdoProfile,
    CustomMalloc,
    FdoPrefetchHints,
    PropellerOptimize,
    MemprofProfile,
    ProtoProfilePath,
    GrteTop,
    FdoInstrument,
    CsFdoInstrument,
    CollectCodeCoverage,
    CoverageOutputGenerator,
    Copt,
    ActionEnv,
    HostActionEnv,
    IncompatibleStrictActionEnv,
}

impl NativeCommandOption {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "compilation_mode" => Self::CompilationMode,
            "host_compilation_mode" => Self::HostCompilationMode,
            "fdo_optimize" => Self::FdoOptimize,
            "xbinary_fdo" => Self::XbinaryFdo,
            "fdo_profile" => Self::FdoProfile,
            "cs_fdo_profile" => Self::CsFdoProfile,
            "custom_malloc" => Self::CustomMalloc,
            "fdo_prefetch_hints" => Self::FdoPrefetchHints,
            "propeller_optimize" => Self::PropellerOptimize,
            "memprof_profile" => Self::MemprofProfile,
            "proto_profile_path" => Self::ProtoProfilePath,
            "grte_top" => Self::GrteTop,
            "fdo_instrument" => Self::FdoInstrument,
            "cs_fdo_instrument" => Self::CsFdoInstrument,
            "collect_code_coverage" => Self::CollectCodeCoverage,
            "coverage_output_generator" => Self::CoverageOutputGenerator,
            "copt" => Self::Copt,
            "action_env" => Self::ActionEnv,
            "host_action_env" => Self::HostActionEnv,
            "incompatible_strict_action_env" | "experimental_strict_action_env" => {
                Self::IncompatibleStrictActionEnv
            }
            _ => return None,
        })
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::CompilationMode => "compilation_mode",
            Self::HostCompilationMode => "host_compilation_mode",
            Self::FdoOptimize => "fdo_optimize",
            Self::XbinaryFdo => "xbinary_fdo",
            Self::FdoProfile => "fdo_profile",
            Self::CsFdoProfile => "cs_fdo_profile",
            Self::CustomMalloc => "custom_malloc",
            Self::FdoPrefetchHints => "fdo_prefetch_hints",
            Self::PropellerOptimize => "propeller_optimize",
            Self::MemprofProfile => "memprof_profile",
            Self::ProtoProfilePath => "proto_profile_path",
            Self::GrteTop => "grte_top",
            Self::FdoInstrument => "fdo_instrument",
            Self::CsFdoInstrument => "cs_fdo_instrument",
            Self::CollectCodeCoverage => "collect_code_coverage",
            Self::CoverageOutputGenerator => "coverage_output_generator",
            Self::Copt => "copt",
            Self::ActionEnv => "action_env",
            Self::HostActionEnv => "host_action_env",
            Self::IncompatibleStrictActionEnv => "incompatible_strict_action_env",
        }
    }

    pub const fn is_boolean(self) -> bool {
        matches!(
            self,
            Self::CollectCodeCoverage | Self::IncompatibleStrictActionEnv
        )
    }

    pub const fn requires_repository_mapping(self) -> bool {
        matches!(
            self,
            Self::XbinaryFdo
                | Self::FdoProfile
                | Self::CsFdoProfile
                | Self::CustomMalloc
                | Self::FdoPrefetchHints
                | Self::PropellerOptimize
                | Self::MemprofProfile
                | Self::ProtoProfilePath
                | Self::GrteTop
                | Self::CoverageOutputGenerator
        )
    }
}

/// One command occurrence retained until contextual configuration preparation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommandConfigurationOccurrence {
    Starlark {
        apparent_label: CompactString,
        raw_value: Option<CompactString>,
        negated: bool,
    },
    ExtraToolchains {
        raw_value: CompactString,
    },
    ExtraExecutionPlatforms {
        raw_value: CompactString,
    },
    Native {
        option: NativeCommandOption,
        raw_value: Option<CompactString>,
        negated: bool,
    },
}

impl CommandConfigurationOccurrence {
    pub fn starlark(
        apparent_label: impl Into<CompactString>,
        raw_value: Option<impl Into<CompactString>>,
        negated: bool,
    ) -> Self {
        Self::Starlark {
            apparent_label: apparent_label.into(),
            raw_value: raw_value.map(Into::into),
            negated,
        }
    }

    pub fn extra_toolchains(raw_value: impl Into<CompactString>) -> Self {
        Self::ExtraToolchains {
            raw_value: raw_value.into(),
        }
    }

    pub fn extra_execution_platforms(raw_value: impl Into<CompactString>) -> Self {
        Self::ExtraExecutionPlatforms {
            raw_value: raw_value.into(),
        }
    }

    pub fn native(
        option: NativeCommandOption,
        raw_value: Option<impl Into<CompactString>>,
        negated: bool,
    ) -> Self {
        Self::Native {
            option,
            raw_value: raw_value.map(Into::into),
            negated,
        }
    }

    pub const fn requires_repository_mapping(&self) -> bool {
        matches!(
            self,
            Self::Native { option, .. } if option.requires_repository_mapping()
        )
    }
}

/// Compact immutable occurrence sequence shared by all command transports.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Hash,
    Allocative,
    Dupe,
    Serialize,
    Deserialize
)]
#[serde(transparent)]
pub struct CommandConfigurationOverlay(Arc<[CommandConfigurationOccurrence]>);

impl CommandConfigurationOverlay {
    pub fn new(occurrences: impl Into<Arc<[CommandConfigurationOccurrence]>>) -> Self {
        Self(occurrences.into())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CommandConfigurationOccurrence> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn requires_repository_mapping(&self) -> bool {
        self.0
            .iter()
            .any(CommandConfigurationOccurrence::requires_repository_mapping)
    }

    /// Whether two carriers share the same immutable occurrence allocation.
    pub fn shares_allocation_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[cfg(test)]
    pub fn as_ptr(&self) -> *const CommandConfigurationOccurrence {
        self.0.as_ptr()
    }
}

impl From<Vec<CommandConfigurationOccurrence>> for CommandConfigurationOverlay {
    fn from(value: Vec<CommandConfigurationOccurrence>) -> Self {
        if value.is_empty() {
            Self::default()
        } else {
            Self(Arc::from(value))
        }
    }
}
