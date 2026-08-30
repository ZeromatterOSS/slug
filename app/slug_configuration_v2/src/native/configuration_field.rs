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
            Self::FdoPrefetchHints => "fdo_prefetch_hints",
            Self::PropellerOptimize => "propeller_optimize",
            Self::MemprofProfile => "memprof_profile",
            Self::ProtoProfilePath => "proto_profile_path",
            Self::LibcTop => "libc_top",
            Self::Zipper => "zipper",
        }
    }
}

/// Typed producer identity for one admitted Starlark `configuration_field`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ConfigurationField(CppConfigurationField);

impl ConfigurationField {
    pub const fn cpp(field: CppConfigurationField) -> Self {
        Self(field)
    }

    pub fn from_starlark_names(fragment: &str, name: &str) -> Option<Self> {
        match fragment {
            "cpp" => match CppConfigurationField::from_starlark_name(name) {
                Some(field) => Some(Self::cpp(field)),
                None => None,
            },
            _ => None,
        }
    }

    pub const fn fragment_name(self) -> &'static str {
        "cpp"
    }

    pub const fn field_name(self) -> &'static str {
        self.0.starlark_name()
    }

    pub const fn cpp_field(self) -> CppConfigurationField {
        self.0
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
