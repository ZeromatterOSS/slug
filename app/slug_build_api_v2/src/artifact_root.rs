//! Structural artifact roots. Display paths are Slug-native, never identity keys.

use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::SlugConfigurationKind;
use slug_identity_v2::CanonicalRepoName;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
struct DerivedArtifactRootData {
    configuration: SlugConfiguration,
    path: CompactString,
}

#[derive(Debug, Clone, Dupe, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct DerivedArtifactRoot(Arc<DerivedArtifactRootData>);

impl DerivedArtifactRoot {
    pub fn bin(configuration: SlugConfiguration) -> Result<Self, String> {
        let suffix = match configuration.kind() {
            SlugConfigurationKind::Target => "",
            SlugConfigurationKind::Exec => "-exec",
            SlugConfigurationKind::HostLike => {
                return Err("HostLike output roots are unsupported".into());
            }
        };
        let path = format!(
            "bazel-out/{}{suffix}/bin",
            configuration.projection().path_component()
        )
        .into();
        Ok(Self(Arc::new(DerivedArtifactRootData {
            configuration,
            path,
        })))
    }

    pub fn path(&self) -> &str {
        &self.0.path
    }
    pub fn configuration(&self) -> &SlugConfiguration {
        &self.0.configuration
    }
    pub fn configuration_component(&self) -> &str {
        self.path()
            .strip_prefix("bazel-out/")
            .unwrap()
            .strip_suffix("/bin")
            .unwrap()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum AnalysisArtifactRoot {
    Source(CanonicalRepoName),
    Derived(DerivedArtifactRoot),
    WorkspaceStatus(crate::WorkspaceStatusRoot),
}

impl AnalysisArtifactRoot {
    pub fn path(&self) -> &str {
        match self {
            Self::Source(_) => "",
            Self::Derived(root) => root.path(),
            Self::WorkspaceStatus(root) => root.path(),
        }
    }
}

/// Ordinary declarations are repository-relative within the shared bin root.
/// Special bin-relative outputs (for example Solib) do not call this helper.
pub fn repository_relative_output_path(repo: &CanonicalRepoName, path: &str) -> String {
    if repo.is_root() {
        path.to_owned()
    } else {
        format!("external/{}/{path}", repo.as_str())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../tests/root_support/mod.rs"]
pub(crate) mod test_support;
