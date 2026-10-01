//! The private, declared normal-compute bridge for input-image preparation.
//! Image observations are transient here; process owners are not activated.
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageObservationOperation;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::ObservedInputImage;
use slug_workspace_v2::ObservedInputImageKey;
use slug_workspace_v2::PathDirectoryName;
use slug_workspace_v2::PathOutcome;

use super::*;
use crate::runtime::input_image::CapturedInputImageLease;
use crate::runtime::input_image::{self};

#[allow(dead_code)]
#[derive(Clone)]
pub(super) enum DeclaredImageSource {
    WorkspaceSubtree(PathBuf),
    ImmutableRepository {
        request: Arc<RepositoryMaterializationRequest>,
        subtree: PathBuf,
    },
}

#[derive(Clone)]
pub(super) struct DeclaredImageRequest {
    key: ObservedInputImageKey,
    deadline: std::time::Instant,
    repository: Option<(
        Arc<RepositoryMaterializationRequest>,
        slug_bzlmod_v2::RepositoryMaterialization,
    )>,
}

impl DeclaredImageRequest {
    fn source(
        &self,
        command: &NativeDemandCommand<'_>,
    ) -> Result<input_image::ScopedImageSource, NativeDemandSessionError> {
        if std::time::Instant::now() >= self.deadline {
            return Err(image_error("image preparation deadline"));
        }
        command
            .runtime
            .repository_materializer
            .image_source(
                command.repository_session,
                self.key.namespace(),
                self.key.anchor_root(),
                self.repository
                    .as_ref()
                    .map(|(request, result)| (request, result)),
            )
            .map_err(NativeDemandSessionError::Repository)
    }

    fn allows(&self, demand: &PathObservationDemand) -> bool {
        let PathObservationOperation::InputImage(request) = demand.operation() else {
            return false;
        };
        let limits = self.key.policy().limits();
        if demand.namespace() != self.key.namespace()
            || request.scope_anchor_root() != self.key.anchor_root()
        {
            return false;
        }
        let path = demand.path().as_path();
        if !path.starts_with(self.key.anchor_root().as_path())
            || !(path.starts_with(self.key.source_root().as_path())
                || self.key.source_root().as_path().starts_with(path))
        {
            return false;
        }
        match request.operation() {
            ImageObservationOperation::Lstat => true,
            ImageObservationOperation::BoundedDigest { max_bytes } => {
                max_bytes == limits.max_file_bytes
            }
            ImageObservationOperation::BoundedDirectory(bounds) => {
                bounds.max_entries <= limits.max_entries
                    && bounds.max_name_bytes_total <= limits.max_descriptor_bytes
                    && bounds.max_component_bytes <= limits.max_path_bytes
            }
            ImageObservationOperation::BoundedReadLink { max_target_bytes } => {
                max_target_bytes <= limits.max_path_bytes
            }
        }
    }
}

pub(super) fn observe_command_paths(
    command: &NativeDemandCommand<'_>,
    demands: Vec<PathObservationDemand>,
) -> Result<PathObservationEpoch, NativeDemandSessionError> {
    let mut observations = Vec::with_capacity(demands.len());
    let mut legacy = Vec::new();
    let budget = command.runtime.image_resources.descriptor_budget();
    // Validate the entire image batch before doing even the first read.
    for demand in &demands {
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            let request = command
                .image_request
                .as_ref()
                .ok_or_else(|| image_error("undeclared image observation"))?;
            let PathObservationOperation::InputImage(scope) = demand.operation() else {
                unreachable!()
            };
            if !request.allows(demand)
                || !scope.same_budget(budget)
                || !demand
                    .image_path()
                    .is_some_and(|path| path.same_budget(budget))
            {
                return Err(image_error("foreign image observation"));
            }
        }
    }
    let source = demands
        .iter()
        .any(|demand| demand.image_path().is_some())
        .then(|| {
            command
                .image_request
                .as_ref()
                .expect("validated image request")
                .source(command)
        })
        .transpose()?;
    let active = || {
        command
            .image_request
            .as_ref()
            .is_none_or(|request| std::time::Instant::now() < request.deadline)
            && command
                .runtime
                .repository_materializer
                .session_is_active(command.repository_session)
    };
    for demand in demands {
        if matches!(demand.operation(), PathObservationOperation::InputImage(_)) {
            let result = source
                .as_ref()
                .expect("authorized image source")
                .observe(&demand, budget, &active)
                .map_err(NativeDemandSessionError::InputImage)?;
            observations.push((demand, Arc::new(result)));
        } else {
            legacy.push(demand);
        }
    }
    if !legacy.is_empty() {
        let observed = command
            .runtime
            .repository_materializer
            .observe_native(command.repository_session, legacy)
            .map_err(NativeDemandSessionError::Repository)?;
        observations.extend(
            observed
                .observations()
                .iter()
                .map(|(demand, result)| (demand.clone(), result.dupe())),
        );
    }
    PathObservationEpoch::from_shared(observations).map_err(NativeDemandSessionError::PathEpoch)
}

fn image_error(message: &'static str) -> NativeDemandSessionError {
    NativeDemandSessionError::Computation(anyhow::anyhow!(message))
}

enum ImageAttempt {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Complete(ObservedInputImage, NativeDemandTerminalSelection),
}

impl WorkspaceRuntime {
    /// Only a declared source enters this primitive; the caller cannot supply
    /// a certificate. Compute, used-closure selection and capture are one route.
    #[allow(dead_code)]
    pub(super) fn capture_declared_input_image(
        &self,
        source: DeclaredImageSource,
        policy: InputImagePolicy,
    ) -> Result<CapturedInputImageLease, NativeDemandSessionError> {
        let mut guard =
            NativeDemandAbortGuard::new(self.begin_native_demand_command()?.into_command());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let result = (|| {
            let source_prefix_depth = match &source {
                DeclaredImageSource::WorkspaceSubtree(subtree)
                | DeclaredImageSource::ImmutableRepository { subtree, .. } => {
                    subtree.components().count()
                }
            };
            let maximum_attempts = policy
                .limits()
                .max_entries
                .saturating_mul(4)
                .saturating_add(source_prefix_depth)
                .saturating_add(policy.limits().max_depth)
                .saturating_add(4);
            for _ in 0..maximum_attempts {
                if std::time::Instant::now() >= deadline {
                    return Err(image_error("image preparation deadline"));
                }
                guard.begin_attempt()?;
                let attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), async {
                    let mut updater = self.dice.updater_with_data(guard.attempt_user_computation_data()?);
                    guard.inject_attempt(&mut updater)?;
                    let mut transaction = self.request_revision.commit(updater).await;
                    let (namespace, anchor, subtree, repository) = match &source {
                        DeclaredImageSource::WorkspaceSubtree(subtree) => (
                            PathObservationNamespace::Host,
                            ImageChargedPath::pin_existing(self.repository_materializer.workspace().clone(), self.image_resources.descriptor_budget())
                                .map_err(|_| image_error("image root descriptor admission failed"))?,
                            subtree, None,
                        ),
                        DeclaredImageSource::ImmutableRepository { request, subtree } => {
                            if request.id.workspace.as_path() != self.workspace { return Err(image_error("foreign image repository")); }
                            let observed = slug_bzlmod_v2::source_preparation::compute_repository_materialization_result(&mut transaction, request.clone())
                                .await.map_err(NativeDemandSessionError::Computation)?;
                            let materialized = match observed {
                                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                                    guard.seal_retry()?;
                                    return Ok(ImageAttempt::Need(need));
                                }
                                slug_bzlmod_v2::SourcePreparationOutcome::Complete(result) => result.as_ref().clone().map_err(|_| image_error("image repository materialization failed"))?,
                            };
                            let slug_bzlmod_v2::RepositoryMaterialization::Immutable { generation_root, observation_instance, .. } = &materialized else {
                                return Err(image_error("image source is not immutable"));
                            };
                            let _normalization_credit = self.image_resources.descriptor_budget()
                                .try_reserve(generation_root.as_os_str().len().saturating_mul(2).saturating_add(128))
                                .map_err(|_| image_error("image root descriptor admission failed"))?;
                            let normalized = NormalizedAbsolutePath::new(generation_root.clone()).map_err(|_| image_error("invalid materialization root"))?;
                            let anchor = ImageChargedPath::pin_existing(normalized, self.image_resources.descriptor_budget())
                                .map_err(|_| image_error("image root descriptor admission failed"))?;
                            (PathObservationNamespace::Materialization(*observation_instance), anchor,
                                subtree, Some((request.clone(), materialized)))
                        }
                    };
                    if subtree.is_absolute() || subtree.as_os_str().len() > policy.limits().max_path_bytes
                        || subtree.components().any(|component| !matches!(component, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
                        return Err(image_error("invalid image subtree"));
                    }
                    let mut selected_root = anchor.clone();
                    for component in subtree.components() {
                        if let std::path::Component::Normal(name) = component {
                            let _name_credit = self.image_resources.descriptor_budget().try_reserve(name.len().saturating_add(64))
                                .map_err(|_| image_error("image component descriptor admission failed"))?;
                            let name = PathDirectoryName::new(name.to_owned()).map_err(|_| image_error("invalid image component"))?;
                            selected_root = selected_root.try_join_component(&name).map_err(|_| image_error("image path descriptor admission failed"))?;
                        }
                    }
                    let key = ObservedInputImageKey::from_charged(namespace, anchor, selected_root, policy).map_err(|_| image_error("invalid image policy"))?;
                    guard.command.as_mut().expect("active image command").image_request = Some(DeclaredImageRequest { key: key.clone(), deadline, repository });
                    let outcome = transaction.compute(&key).await.map_err(|error| NativeDemandSessionError::Computation(error.into()))?;
                    match outcome {
                        PathOutcome::Need(need) => {
                            guard.seal_retry()?;
                            Ok(ImageAttempt::Need(slug_bzlmod_v2::SourcePreparationNeeds::path(need)))
                        }
                        PathOutcome::Complete(result) => {
                            let certificate = result.map_err(|_| image_error("image producer refused source"))?;
                            if !certificate.same_budget(self.image_resources.descriptor_budget()) {
                                return Err(image_error("foreign cached image budget"));
                            }
                            let selected = guard.seal_terminal()?.select(&transaction).await?;
                            let reachable = selected.sidecars().demands();
                            for (demand, observed) in certificate.observations().observations() {
                                if !reachable.unscoped_paths().contains(demand)
                                    || guard.command().path_observations.get(demand) != Some(observed) {
                                    return Err(image_error("image frontier is not current reachable evidence"));
                                }
                            }
                            if let Some((request, _)) = &guard.command().image_request.as_ref().expect("image request").repository {
                                if !reachable.repository_requests().contains(request) { return Err(image_error("image repository is not reachable")); }
                            }
                            Ok(ImageAttempt::Complete(certificate, selected))
                        }
                    }
                    }).await.map_err(|_| image_error("image preparation deadline"))?
                });
                // All transactions/compute futures have dropped; drain their
                // cancelled workers before native observations or copying.
                self.runtime.block_on(self.dice.wait_for_idle());
                match attempt? {
                    ImageAttempt::Need(need) => {
                        guard.progress(&need)?;
                    }
                    ImageAttempt::Complete(certificate, selected) => {
                        let command = guard.command();
                        let request = command
                            .image_request
                            .as_ref()
                            .expect("current image request");
                        let source = request.source(command)?;
                        let authorized =
                            input_image::AuthorizedObservedInputImage::new(source, certificate);
                        let active = || {
                            std::time::Instant::now() < deadline
                                && self
                                    .repository_materializer
                                    .session_is_active(command.repository_session)
                        };
                        let captured = input_image::capture_input_image(
                            authorized,
                            &self.image_resources,
                            &active,
                        )
                        .map_err(NativeDemandSessionError::InputImage);
                        // This prerequisite does not publish process epochs or
                        // put image demands in accepted generic preflight.
                        drop(selected);
                        return captured;
                    }
                }
            }
            Err(image_error("image preparation made no bounded progress"))
        })();
        guard.suppress_attempt()?;
        let cleanup = guard
            .command
            .as_mut()
            .expect("image guard owns command")
            .discard_in_place();
        match cleanup {
            Ok(()) => {
                guard.phase = NativeDemandAbortPhase::Closed;
                guard.command = None;
                result
            }
            Err(error) => {
                guard.phase = NativeDemandAbortPhase::FailClosed;
                Err(error)
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "input_image_bridge/tests.rs"]
mod tests;
