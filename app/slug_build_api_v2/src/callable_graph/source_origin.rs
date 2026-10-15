//! Source identity separates observed host files from bundled private catalogs.
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use allocative::Allocative;
use compact_str::CompactString;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub enum ModuleSourceOrigin {
    /// Existing source observers own normalization and filesystem admission.
    Workspace(PathBuf),
    Builtin(BuiltinSourceOrigin),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BuiltinSourceOrigin {
    catalog: CompactString,
    path: CompactString,
}

impl ModuleSourceOrigin {
    /// This records a source domain, not authority to acquire or execute it.
    /// The loader must separately validate the complete catalog and its bytes.
    pub fn builtin(catalog: &str, path: &str) -> Result<Self, String> {
        if catalog.is_empty()
            || catalog.contains('\0')
            || path.is_empty()
            || !path.ends_with(".bzl")
            || path.contains(['\\', '\0'])
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(
                "builtin source requires a catalog and normalized relative .bzl path".into(),
            );
        }
        Ok(Self::Builtin(BuiltinSourceOrigin {
            catalog: catalog.into(),
            path: path.into(),
        }))
    }

    pub fn workspace_path(&self) -> Option<&Path> {
        match self {
            Self::Workspace(path) => Some(path),
            Self::Builtin(_) => None,
        }
    }

    pub fn builtin_catalog(&self) -> Option<&str> {
        match self {
            Self::Workspace(_) => None,
            Self::Builtin(source) => Some(&source.catalog),
        }
    }

    pub fn catalog_relative_path(&self) -> Option<&str> {
        match self {
            Self::Workspace(_) => None,
            Self::Builtin(source) => Some(&source.path),
        }
    }

    /// Presentation only. A relative builtin path never grants host I/O.
    pub fn diagnostic_path(&self) -> Cow<'_, Path> {
        Cow::Borrowed(match self {
            Self::Workspace(path) => path,
            Self::Builtin(source) => Path::new(source.path.as_str()),
        })
    }

    /// Complete builtin text capacity, including the catalog namespace.
    /// Workspace paths retain their existing admitted path-length policy.
    /// Callers additionally forecast the complete inline enum/header storage.
    pub fn retained_text_bytes(&self) -> Option<usize> {
        match self {
            Self::Workspace(path) => Some(path.as_os_str().len()),
            Self::Builtin(source) => source
                .catalog
                .capacity()
                .checked_add(source.path.capacity()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_domains_and_catalogs_remain_structurally_distinct() {
        let builtin = ModuleSourceOrigin::builtin("bazel9.2", "common/value.bzl").unwrap();
        let restored = ModuleSourceOrigin::builtin("bazel9.2", "common/value.bzl").unwrap();
        assert_eq!(builtin, restored);
        assert_eq!(builtin.workspace_path(), None);
        assert_eq!(builtin.builtin_catalog(), Some("bazel9.2"));
        assert_eq!(builtin.catalog_relative_path(), Some("common/value.bzl"));
        assert_eq!(
            builtin.diagnostic_path().as_ref(),
            Path::new("common/value.bzl")
        );
        let ModuleSourceOrigin::Builtin(payload) = &builtin else {
            unreachable!()
        };
        assert_eq!(
            builtin.retained_text_bytes(),
            Some(payload.catalog.capacity() + payload.path.capacity())
        );
        assert_ne!(
            builtin,
            ModuleSourceOrigin::Workspace("common/value.bzl".into())
        );
        assert_ne!(
            builtin,
            ModuleSourceOrigin::builtin("other", "common/value.bzl").unwrap()
        );
        assert_ne!(
            builtin,
            ModuleSourceOrigin::builtin("bazel9.2", "common/other.bzl").unwrap()
        );
    }

    #[test]
    fn builtin_paths_cannot_acquire_workspace_path_authority() {
        for path in [
            "",
            "/absolute.bzl",
            "../file.bzl",
            "a/../file.bzl",
            "./file.bzl",
            "a//file.bzl",
            "a\\file.bzl",
            "file.bzl\0",
            "file.txt",
        ] {
            assert!(
                ModuleSourceOrigin::builtin("catalog", path).is_err(),
                "{path:?}"
            );
        }
        assert!(ModuleSourceOrigin::builtin("", "file.bzl").is_err());
        assert!(ModuleSourceOrigin::builtin("bad\0catalog", "file.bzl").is_err());
        let workspace = ModuleSourceOrigin::Workspace("/actual/source.bzl".into());
        assert_eq!(
            workspace.workspace_path(),
            Some(Path::new("/actual/source.bzl"))
        );
        assert_eq!(workspace.builtin_catalog(), None);
        assert_eq!(workspace.catalog_relative_path(), None);
        assert_eq!(
            workspace.retained_text_bytes(),
            Some("/actual/source.bzl".len())
        );
    }
}
