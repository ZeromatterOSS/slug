//! Exact selected image authority with no ambient open for physical roots.
use super::*;

impl RepositoryMaterializer {
    pub(in crate::runtime) fn image_source(
        &self,
        token: RepositorySessionToken,
        namespace: PathObservationNamespace,
        anchor: &NormalizedAbsolutePath,
        selected: Option<(
            &Arc<RepositoryMaterializationRequest>,
            &slug_bzlmod_v2::RepositoryMaterialization,
        )>,
    ) -> Result<super::super::input_image::ScopedImageSource, RepositorySessionError> {
        let command_scope = self.command_source_scope(token)?;
        self.check_source_scope(self.source_scope.as_ref())?;
        // Only constant-size Arc clones leave this mutex. Row's shared guard
        // continues paying for complete request/history while compared unlocked.
        let (pin, comparison_resources) = {
            let state = self
                .state
                .lock()
                .expect("repository materializer mutex poisoned");
            let active = matching_validated_active(&state, token)?;
            let pin = match (namespace, selected) {
                (PathObservationNamespace::Host, None) if anchor == &self.workspace => None,
                (PathObservationNamespace::Materialization(instance), Some((request, _))) => {
                    let entry = active
                        .entries
                        .iter()
                        .find(|entry| entry.request.id == request.id)
                        .ok_or(RepositorySessionError::InvalidImageAuthority)?;
                    let row = state
                        .accepted_roots
                        .iter()
                        .chain(&active.provisional_roots)
                        .find(|row| {
                            row.observation_instance == instance
                                && Arc::ptr_eq(&row.request, &entry.request)
                        })
                        .ok_or(RepositorySessionError::InvalidImageAuthority)?;
                    if !image_binding_matches(entry, row) {
                        return Err(RepositorySessionError::InvalidImageAuthority);
                    }
                    Some(row.clone())
                }
                _ => return Err(RepositorySessionError::InvalidImageAuthority),
            };
            #[cfg(target_os = "linux")]
            let resources = state
                .claim_domain
                .as_ref()
                .map(|domain| domain.resources().clone());
            #[cfg(not(target_os = "linux"))]
            let resources = None;
            (pin, resources)
        };
        let active = || self.session_is_active(token);
        let mut comparison = RepositoryRequestComparison::new(
            comparison_resources.as_ref(),
            command_scope.as_deref(),
            &active,
        )?;
        if let Some(row) = &pin {
            let (request, expected) =
                selected.ok_or(RepositorySessionError::InvalidImageAuthority)?;
            let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
                canonical_repo,
                repo_spec,
                source_identity,
                generation_root,
                observation_instance,
                anchor: expected_anchor,
            } = expected
            else {
                return Err(RepositorySessionError::InvalidImageAuthority);
            };
            if !comparison.equal(&row.request, request)?
                || canonical_repo != &request.id.canonical_repo
                || repo_spec != &request.repo_spec
                || source_identity != &row.source_identity
                || generation_root != row.root.path()
                || anchor.as_path() != row.root.path()
                || *observation_instance != row.observation_instance
            {
                return Err(RepositorySessionError::InvalidImageAuthority);
            }
            match &row.root {
                RetainedRootOwner::Legacy(_)
                    if row.path_policy == RepositoryMaterializationPathPolicy::GenerationRoot
                        && expected_anchor.as_ref().is_some_and(|bound| {
                            bound.namespace() == namespace && bound.root() == anchor
                        }) => {}
                RetainedRootOwner::Legacy(_)
                    if row.path_policy == RepositoryMaterializationPathPolicy::LegacyUnbounded
                        && expected_anchor.is_none() => {}
                #[cfg(target_os = "linux")]
                RetainedRootOwner::Physical(root)
                    if expected_anchor.as_ref() == Some(root.anchor()) => {}
                _ => return Err(RepositorySessionError::InvalidImageAuthority),
            }
        }
        let source = match pin.as_ref().map(|row| &row.root) {
            #[cfg(target_os = "linux")]
            Some(RetainedRootOwner::Physical(root)) => {
                if !root.state().source_bindings().is_empty() {
                    return Err(RepositorySessionError::InvalidImageAuthority);
                }
                let scope = command_scope
                    .as_ref()
                    .ok_or(RepositorySessionError::InvalidImageAuthority)?;
                root.image_source(scope, &|| self.session_is_active(token))
                    .map_err(|_| RepositorySessionError::InvalidImageAuthority)?
            }
            named => {
                let owner = match named {
                    Some(RetainedRootOwner::Legacy(root)) => Some(root.clone()),
                    _ => None,
                };
                let file = super::super::input_image::open_image_anchor(anchor)
                    .map_err(|_| RepositorySessionError::InvalidImageAuthority)?;
                match owner {
                    Some(owner) => super::super::input_image::ScopedImageSource::from_legacy(
                        namespace,
                        anchor.clone(),
                        file,
                        owner,
                    ),
                    None => super::super::input_image::ScopedImageSource::new(
                        namespace,
                        anchor.clone(),
                        file,
                        None,
                    ),
                }
                .map_err(|_| RepositorySessionError::InvalidImageAuthority)?
            }
        };
        self.command_source_scope(token)?;
        self.check_source_scope(self.source_scope.as_ref())?;
        let state = self
            .state
            .lock()
            .expect("repository materializer mutex poisoned");
        let active = matching_validated_active(&state, token)?;
        if !super::command_scope::scope_is_current(active, command_scope.as_ref()) {
            return Err(RepositorySessionError::InvalidImageAuthority);
        }
        if let Some(row) = &pin {
            let entry = active
                .entries
                .iter()
                .find(|entry| Arc::ptr_eq(&entry.request, &row.request))
                .ok_or(RepositorySessionError::InvalidImageAuthority)?;
            if !image_binding_matches(entry, row)
                || !state
                    .accepted_roots
                    .iter()
                    .chain(&active.provisional_roots)
                    .any(|now| {
                        now.observation_instance == row.observation_instance
                            && now.generation == row.generation
                            && Arc::ptr_eq(&now.request, &row.request)
                            && now.source_identity == row.source_identity
                            && now.path_policy == row.path_policy
                            && now.root.same_owner(&row.root)
                    })
            {
                return Err(RepositorySessionError::InvalidImageAuthority);
            }
        }
        Ok(source)
    }
}
fn image_binding_matches(
    entry: &RepositoryMaterializationEpochEntry,
    row: &RetainedRepositoryRoot,
) -> bool {
    matches!(&entry.result,
        RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
            source_identity, generation_root, observation_instance, path_policy,
        }) if source_identity == &row.source_identity && generation_root == row.root.path()
            && *observation_instance == row.observation_instance && *path_policy == row.path_policy)
}
