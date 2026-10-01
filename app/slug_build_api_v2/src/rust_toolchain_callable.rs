//! Finite Rust-toolchain callback data. Loading owns source authentication and
//! phase helper bindings; this owner retains no code, heap, actions or context.
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RustBzlModuleIdentity {
    pub label: CanonicalLabel,
    pub workspace_path: PathBuf,
    pub repository_mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RustBzlModuleSource {
    pub identity: RustBzlModuleIdentity,
    pub digest: [u8; 32],
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RustToolchainHelperManifest {
    pub root: RustBzlModuleIdentity,
    pub direct_children: Arc<[RustBzlModuleIdentity]>,
    pub reachable: Arc<[RustBzlModuleSource]>,
    pub fingerprint: [u8; 32],
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RustToolchainCallableSourceAuthority {
    pub caller: RustBzlModuleSource,
    pub helper: RustToolchainHelperManifest,
}

#[derive(Debug, Allocative)]
pub struct RustToolchainCallableModel {
    pub source: Arc<RustToolchainCallableSourceAuthority>,
    pub identity: Arc<()>,
    /// Cached original function-name hash, not semantic source identity.
    pub name_hash: u32,
    pub cc_toolchain: AnalysisValue,
    pub feature_configuration: AnalysisValue,
    pub experimental_link_std_dylib: AnalysisValue,
    pub rust_std: AnalysisValue,
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RustToolchainCallable(Arc<RustToolchainCallableModel>);

impl RustToolchainCallable {
    pub fn new(model: RustToolchainCallableModel) -> Result<Self, String> {
        for (valid, field) in [
            (
                matches!(
                    model.cc_toolchain.kind(),
                    AnalysisValueKind::None | AnalysisValueKind::Provider(_)
                ),
                "cc_toolchain requires None or a provider",
            ),
            (
                matches!(
                    model.feature_configuration.kind(),
                    AnalysisValueKind::None | AnalysisValueKind::CcFeatureConfiguration(_)
                ),
                "feature_configuration requires None or a genuine feature configuration",
            ),
            (
                matches!(
                    model.experimental_link_std_dylib.kind(),
                    AnalysisValueKind::Boolean(_)
                ),
                "experimental_link_std_dylib requires a Boolean",
            ),
            (
                matches!(
                    model.rust_std.kind(),
                    AnalysisValueKind::ConfiguredTarget(_)
                ),
                "rust_std requires a genuine Target",
            ),
        ] {
            if !valid {
                return Err(format!("Rust toolchain callable {field}"));
            }
        }
        Ok(Self(Arc::new(model)))
    }

    pub fn model(&self) -> &RustToolchainCallableModel {
        &self.0
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let left = self.model();
        let right = other.model();
        left.source == right.source
            && left.name_hash == right.name_hash
            && state.enter_rust_callable_pair(
                Arc::as_ptr(&left.identity) as usize,
                Arc::as_ptr(&right.identity) as usize,
            )
            && left
                .cc_toolchain
                .publication_eq_with(&right.cc_toolchain, state)
            && left
                .feature_configuration
                .publication_eq_with(&right.feature_configuration, state)
            && left
                .experimental_link_std_dylib
                .publication_eq_with(&right.experimental_link_std_dylib, state)
            && left.rust_std.publication_eq_with(&right.rust_std, state)
    }
}

impl PartialEq for RustToolchainCallable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.identity, &other.0.identity)
    }
}
impl Eq for RustToolchainCallable {}
impl Hash for RustToolchainCallable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0.identity) as usize).hash(state);
    }
}

#[cfg(test)]
mod tests;
