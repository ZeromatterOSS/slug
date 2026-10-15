//! Borrow the actual source owner's raw graph and separate environment edges.
use slug_loading_v2::BzlEnvironmentSourceBinding;
use slug_loading_v2::BzlModuleSourceContext;
use slug_loading_v2::BzlModuleSourceProvenance;

use super::*;

#[derive(Clone, Copy)]
pub(super) enum SourceManifest<'a> {
    Raw(&'a BzlLoadManifest),
    Effective(&'a BzlModuleSourceContext),
}

impl<'a> SourceManifest<'a> {
    pub(super) fn raw(self) -> &'a BzlLoadManifest {
        match self {
            Self::Raw(raw) => raw,
            Self::Effective(context) => context.raw_load_manifest(),
        }
    }
    pub(super) fn root(self) -> &'a BzlModuleIdentity {
        &self.raw().root
    }
    pub(super) fn direct_children(self) -> &'a [BzlModuleIdentity] {
        &self.raw().direct_children
    }
    pub(super) fn environment_bindings(self) -> &'a [BzlEnvironmentSourceBinding] {
        match self {
            Self::Raw(_) => &[],
            Self::Effective(context) => context.environment_bindings(),
        }
    }
    pub(super) fn reachable(self) -> &'a [BzlModuleSourceProvenance] {
        match self {
            Self::Raw(raw) => &raw.reachable,
            Self::Effective(context) => context.effective_reachable(),
        }
    }
    pub(super) fn fingerprint(self) -> &'a [u8; 32] {
        match self {
            Self::Raw(raw) => &raw.fingerprint,
            Self::Effective(context) => context.fingerprint(),
        }
    }
}
