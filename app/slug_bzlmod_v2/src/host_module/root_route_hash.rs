//! Shared immutable route data, separate from operational bucket routing.
use std::collections::hash_map::DefaultHasher;

use super::*;

impl RootRepositoryRoute {
    /// Build complete fields and their bucket hash before publishing one Arc.
    /// Clones share this owner; they neither copy fields nor recompute the hash.
    pub(super) fn from_fields(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        module_name: CompactString,
        canonical_repo: CanonicalRepoName,
        source: RootRepositorySource,
    ) -> Self {
        let mut data = RootRepositoryRouteData {
            workspace,
            apparent_repo,
            module_name,
            canonical_repo,
            source,
            routing_hash: 0,
        };
        let mut hasher = DefaultHasher::new();
        // Same ordered five-field stream as generic RootRepositoryRoute::Hash.
        data.hash_fields(&mut hasher);
        data.routing_hash = hasher.finish();
        Self {
            data: Arc::new(data),
        }
    }

    /// Weak DICE/cycle bucket input only. Full structural equality remains the
    /// authority. The generic full writer and separately owned canonical/content
    /// streams remain unchanged.
    #[inline]
    pub(crate) fn hash_for_routing<H: Hasher>(&self, state: &mut H) {
        self.data.routing_hash.hash(state);
    }
}

impl PartialEq for RootRepositoryRoute {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Normal Arc Eq handles shared owners; foreign data uses the full Eq.
        self.data == other.data
    }
}
impl Eq for RootRepositoryRoute {}

impl PartialEq for RootRepositoryRouteData {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            workspace,
            apparent_repo,
            module_name,
            canonical_repo,
            source,
            routing_hash: _,
        } = self;
        let Self {
            workspace: other_workspace,
            apparent_repo: other_apparent,
            module_name: other_module,
            canonical_repo: other_canonical,
            source: other_source,
            routing_hash: _,
        } = other;
        workspace == other_workspace
            && apparent_repo == other_apparent
            && module_name == other_module
            && canonical_repo == other_canonical
            && source == other_source
    }
}
impl Eq for RootRepositoryRouteData {}

impl RootRepositoryRouteData {
    /// Complete original stream, shared by construction and the generic writer.
    pub(super) fn hash_fields<H: Hasher>(&self, state: &mut H) {
        let Self {
            workspace,
            apparent_repo,
            module_name,
            canonical_repo,
            source,
            routing_hash: _,
        } = self;
        workspace.hash(state);
        apparent_repo.hash(state);
        module_name.hash(state);
        canonical_repo.hash(state);
        source.hash(state);
    }
}
