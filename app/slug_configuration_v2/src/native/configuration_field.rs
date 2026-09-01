use allocative::Allocative;
use slug_identity_v2::CanonicalRepoName;

/// The finite Bazel 9.2 `cpp` configuration-field surface admitted by Slug.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum CppConfigurationField {
    FdoOptimize,
    XbinaryFdo,
    FdoProfile,
    CsFdoProfile,
    CustomMalloc,
    FdoPrefetchHints,
    PropellerOptimize,
    MemprofProfile,
    ProtoProfilePath,
    LibcTop,
    Zipper,
}

impl CppConfigurationField {
    pub fn from_starlark_name(name: &str) -> Option<Self> {
        match name {
            "fdo_optimize" => Some(Self::FdoOptimize),
            "xbinary_fdo" => Some(Self::XbinaryFdo),
            "fdo_profile" => Some(Self::FdoProfile),
            "cs_fdo_profile" => Some(Self::CsFdoProfile),
            "custom_malloc" => Some(Self::CustomMalloc),
            "fdo_prefetch_hints" => Some(Self::FdoPrefetchHints),
            "propeller_optimize" => Some(Self::PropellerOptimize),
            "memprof_profile" => Some(Self::MemprofProfile),
            "proto_profile_path" => Some(Self::ProtoProfilePath),
            "libc_top" => Some(Self::LibcTop),
            "zipper" => Some(Self::Zipper),
            _ => None,
        }
    }

    pub const fn starlark_name(self) -> &'static str {
        match self {
            Self::FdoOptimize => "fdo_optimize",
            Self::XbinaryFdo => "xbinary_fdo",
            Self::FdoProfile => "fdo_profile",
            Self::CsFdoProfile => "cs_fdo_profile",
            Self::CustomMalloc => "custom_malloc",
            Self::FdoPrefetchHints => "fdo_prefetch_hints",
            Self::PropellerOptimize => "propeller_optimize",
            Self::MemprofProfile => "memprof_profile",
            Self::ProtoProfilePath => "proto_profile_path",
            Self::LibcTop => "libc_top",
            Self::Zipper => "zipper",
        }
    }
}

/// The finite Bazel 9.2 `coverage` configuration-field surface admitted by
/// Slug.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum CoverageConfigurationField {
    OutputGenerator,
}

impl CoverageConfigurationField {
    pub fn from_starlark_name(name: &str) -> Option<Self> {
        match name {
            "output_generator" => Some(Self::OutputGenerator),
            _ => None,
        }
    }

    pub const fn starlark_name(self) -> &'static str {
        match self {
            Self::OutputGenerator => "output_generator",
        }
    }
}

/// Typed producer identity for one admitted Starlark `configuration_field`.
///
/// The payload is flattened into the discriminant so the retained field stays
/// one byte while callers still cross typed fragment-specific accessors.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ConfigurationField {
    CppFdoOptimize,
    CppXbinaryFdo,
    CppFdoProfile,
    CppCsFdoProfile,
    CppCustomMalloc,
    CppFdoPrefetchHints,
    CppPropellerOptimize,
    CppMemprofProfile,
    CppProtoProfilePath,
    CppLibcTop,
    CppZipper,
    CoverageOutputGenerator,
}

impl ConfigurationField {
    pub const fn cpp(field: CppConfigurationField) -> Self {
        match field {
            CppConfigurationField::FdoOptimize => Self::CppFdoOptimize,
            CppConfigurationField::XbinaryFdo => Self::CppXbinaryFdo,
            CppConfigurationField::FdoProfile => Self::CppFdoProfile,
            CppConfigurationField::CsFdoProfile => Self::CppCsFdoProfile,
            CppConfigurationField::CustomMalloc => Self::CppCustomMalloc,
            CppConfigurationField::FdoPrefetchHints => Self::CppFdoPrefetchHints,
            CppConfigurationField::PropellerOptimize => Self::CppPropellerOptimize,
            CppConfigurationField::MemprofProfile => Self::CppMemprofProfile,
            CppConfigurationField::ProtoProfilePath => Self::CppProtoProfilePath,
            CppConfigurationField::LibcTop => Self::CppLibcTop,
            CppConfigurationField::Zipper => Self::CppZipper,
        }
    }

    pub const fn coverage(field: CoverageConfigurationField) -> Self {
        match field {
            CoverageConfigurationField::OutputGenerator => Self::CoverageOutputGenerator,
        }
    }

    pub fn from_starlark_names(fragment: &str, name: &str) -> Option<Self> {
        match fragment {
            "cpp" => CppConfigurationField::from_starlark_name(name).map(Self::cpp),
            "coverage" => CoverageConfigurationField::from_starlark_name(name).map(Self::coverage),
            _ => None,
        }
    }

    pub fn is_fragment_name(fragment: &str) -> bool {
        matches!(fragment, "cpp" | "coverage")
    }

    pub const fn fragment_name(self) -> &'static str {
        match self {
            Self::CoverageOutputGenerator => "coverage",
            _ => "cpp",
        }
    }

    pub const fn field_name(self) -> &'static str {
        match (self.cpp_field(), self.coverage_field()) {
            (Some(field), None) => field.starlark_name(),
            (None, Some(field)) => field.starlark_name(),
            _ => unreachable!(),
        }
    }

    pub const fn cpp_field(self) -> Option<CppConfigurationField> {
        Some(match self {
            Self::CppFdoOptimize => CppConfigurationField::FdoOptimize,
            Self::CppXbinaryFdo => CppConfigurationField::XbinaryFdo,
            Self::CppFdoProfile => CppConfigurationField::FdoProfile,
            Self::CppCsFdoProfile => CppConfigurationField::CsFdoProfile,
            Self::CppCustomMalloc => CppConfigurationField::CustomMalloc,
            Self::CppFdoPrefetchHints => CppConfigurationField::FdoPrefetchHints,
            Self::CppPropellerOptimize => CppConfigurationField::PropellerOptimize,
            Self::CppMemprofProfile => CppConfigurationField::MemprofProfile,
            Self::CppProtoProfilePath => CppConfigurationField::ProtoProfilePath,
            Self::CppLibcTop => CppConfigurationField::LibcTop,
            Self::CppZipper => CppConfigurationField::Zipper,
            Self::CoverageOutputGenerator => return None,
        })
    }

    pub const fn coverage_field(self) -> Option<CoverageConfigurationField> {
        match self {
            Self::CoverageOutputGenerator => Some(CoverageConfigurationField::OutputGenerator),
            _ => None,
        }
    }
}

/// Bazel's late-bound identity includes the fragment field and tools repository,
/// but not the defining `.bzl` module.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConfigurationFieldIdentity {
    field: ConfigurationField,
    tools_repository: CanonicalRepoName,
}

impl ConfigurationFieldIdentity {
    pub fn new(field: ConfigurationField, tools_repository: CanonicalRepoName) -> Self {
        Self {
            field,
            tools_repository,
        }
    }

    pub const fn field(&self) -> ConfigurationField {
        self.field
    }

    pub fn tools_repository(&self) -> &CanonicalRepoName {
        &self.tools_repository
    }
}
