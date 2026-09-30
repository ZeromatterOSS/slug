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
    Features,
    HostFeatures,
    Fission,
    Strip,
    Linkopt,
    HostLinkopt,
    ExperimentalOmitfp,
    ForcePic,
    AppleGenerateDsym,
    ExperimentalUseLlvmCovmap,
    IncompatibleDontEnableHostNonhostCrosstoolFeatures,
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
    CheckVisibility,
    IncompatibleFilegroupRunfilesForData,
    CoverageOutputGenerator,
    Copt,
    Conlyopt,
    Cxxopt,
    HostCopt,
    HostConlyopt,
    HostCxxopt,
    CcDotdFiles,
    SaveTemps,
    ProcessHeadersInDependencies,
    ActionEnv,
    HostActionEnv,
    IncompatibleStrictActionEnv,
    StrictSystemIncludes,
    MinimumOsVersion,
    Stamp,
    ExperimentalCcImplementationDeps,
    StartEndLib,
}

impl NativeCommandOption {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "compilation_mode" => Self::CompilationMode,
            "host_compilation_mode" => Self::HostCompilationMode,
            "features" => Self::Features,
            "host_features" => Self::HostFeatures,
            "fission" => Self::Fission,
            "strip" => Self::Strip,
            "linkopt" => Self::Linkopt,
            "host_linkopt" => Self::HostLinkopt,
            "experimental_omitfp" => Self::ExperimentalOmitfp,
            "force_pic" => Self::ForcePic,
            "experimental_cc_implementation_deps" => Self::ExperimentalCcImplementationDeps,
            "start_end_lib" => Self::StartEndLib,
            "apple_generate_dsym" => Self::AppleGenerateDsym,
            "experimental_use_llvm_covmap" => Self::ExperimentalUseLlvmCovmap,
            "incompatible_dont_enable_host_nonhost_crosstool_features" => {
                Self::IncompatibleDontEnableHostNonhostCrosstoolFeatures
            }
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
            "check_visibility" => Self::CheckVisibility,
            "incompatible_filegroup_runfiles_for_data" => {
                Self::IncompatibleFilegroupRunfilesForData
            }
            "coverage_output_generator" => Self::CoverageOutputGenerator,
            "copt" => Self::Copt,
            "conlyopt" => Self::Conlyopt,
            "cxxopt" => Self::Cxxopt,
            "host_copt" => Self::HostCopt,
            "host_conlyopt" => Self::HostConlyopt,
            "host_cxxopt" => Self::HostCxxopt,
            "cc_dotd_files" => Self::CcDotdFiles,
            "save_temps" => Self::SaveTemps,
            "process_headers_in_dependencies" => Self::ProcessHeadersInDependencies,
            "strict_system_includes" => Self::StrictSystemIncludes,
            "minimum_os_version" => Self::MinimumOsVersion,
            "stamp" => Self::Stamp,
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
            Self::Features => "features",
            Self::HostFeatures => "host_features",
            Self::Fission => "fission",
            Self::Strip => "strip",
            Self::Linkopt => "linkopt",
            Self::HostLinkopt => "host_linkopt",
            Self::ExperimentalOmitfp => "experimental_omitfp",
            Self::ForcePic => "force_pic",
            Self::ExperimentalCcImplementationDeps => "experimental_cc_implementation_deps",
            Self::StartEndLib => "start_end_lib",
            Self::AppleGenerateDsym => "apple_generate_dsym",
            Self::ExperimentalUseLlvmCovmap => "experimental_use_llvm_covmap",
            Self::IncompatibleDontEnableHostNonhostCrosstoolFeatures => {
                "incompatible_dont_enable_host_nonhost_crosstool_features"
            }
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
            Self::CheckVisibility => "check_visibility",
            Self::IncompatibleFilegroupRunfilesForData => {
                "incompatible_filegroup_runfiles_for_data"
            }
            Self::CoverageOutputGenerator => "coverage_output_generator",
            Self::Copt => "copt",
            Self::Conlyopt => "conlyopt",
            Self::Cxxopt => "cxxopt",
            Self::HostCopt => "host_copt",
            Self::HostConlyopt => "host_conlyopt",
            Self::HostCxxopt => "host_cxxopt",
            Self::CcDotdFiles => "cc_dotd_files",
            Self::SaveTemps => "save_temps",
            Self::ProcessHeadersInDependencies => "process_headers_in_dependencies",
            Self::StrictSystemIncludes => "strict_system_includes",
            Self::MinimumOsVersion => "minimum_os_version",
            Self::Stamp => "stamp",
            Self::ActionEnv => "action_env",
            Self::HostActionEnv => "host_action_env",
            Self::IncompatibleStrictActionEnv => "incompatible_strict_action_env",
        }
    }

    pub const fn is_boolean(self) -> bool {
        matches!(
            self,
            Self::CollectCodeCoverage
                | Self::CheckVisibility
                | Self::IncompatibleFilegroupRunfilesForData
                | Self::CcDotdFiles
                | Self::SaveTemps
                | Self::ProcessHeadersInDependencies
                | Self::StrictSystemIncludes
                | Self::Stamp
                | Self::ExperimentalOmitfp
                | Self::IncompatibleStrictActionEnv
                | Self::ForcePic
                | Self::ExperimentalCcImplementationDeps
                | Self::StartEndLib
                | Self::AppleGenerateDsym
                | Self::ExperimentalUseLlvmCovmap
                | Self::IncompatibleDontEnableHostNonhostCrosstoolFeatures
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
