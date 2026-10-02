//! Shared declared image/ELF preparation, current closure and transient cleanup.

use super::*;

enum PreparationKind {
    Capture,
    Elf {
        relative_file: PathBuf,
        policy: ElfMetadataPolicy,
    },
}
enum PreparedInput {
    Captured(CapturedInputImageLease),
    Elf(ObservedElfMetadata),
}
enum ImageAttempt {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Complete(
        ObservedInputImage,
        Option<ObservedElfMetadata>,
        NativeDemandTerminalSelection,
    ),
}

#[allow(dead_code)]
impl WorkspaceRuntime {
    pub(in crate::runtime) fn capture_declared_input_image(
        &self,
        source: DeclaredImageSource,
        policy: InputImagePolicy,
    ) -> Result<CapturedInputImageLease, NativeDemandSessionError> {
        match self.prepare_declared_input(source, policy, PreparationKind::Capture)? {
            PreparedInput::Captured(image) => Ok(image),
            PreparedInput::Elf(_) => unreachable!("capture mode"),
        }
    }

    pub(in crate::runtime) fn observe_declared_elf_metadata(
        &self,
        source: DeclaredImageSource,
        image_policy: InputImagePolicy,
        relative_file: PathBuf,
        policy: ElfMetadataPolicy,
    ) -> Result<ObservedElfMetadata, NativeDemandSessionError> {
        validate_relative(&relative_file, image_policy.limits().max_path_bytes)?;
        match self.prepare_declared_input(
            source,
            image_policy,
            PreparationKind::Elf {
                relative_file,
                policy,
            },
        )? {
            PreparedInput::Elf(metadata) => Ok(metadata),
            PreparedInput::Captured(_) => unreachable!("ELF mode"),
        }
    }

    fn prepare_declared_input(
        &self,
        source: DeclaredImageSource,
        policy: InputImagePolicy,
        kind: PreparationKind,
    ) -> Result<PreparedInput, NativeDemandSessionError> {
        let mut guard =
            NativeDemandAbortGuard::new(self.begin_native_demand_command()?.into_command());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let prefix_depth = match &source {
            DeclaredImageSource::WorkspaceSubtree(path)
            | DeclaredImageSource::ImmutableRepository { subtree: path, .. } => {
                path.components().count()
            }
        };
        let attempts = policy
            .limits()
            .max_entries
            .saturating_mul(4)
            .saturating_add(prefix_depth)
            .saturating_add(policy.limits().max_depth)
            .saturating_add(6);
        let result = (|| {
            for _ in 0..attempts {
                if std::time::Instant::now() >= deadline {
                    return Err(image_error("image preparation deadline"));
                }
                guard.begin_attempt()?;
                let attempt = self.runtime.block_on(async {
                    tokio::time::timeout_at(
                        tokio::time::Instant::from_std(deadline),
                        prepare_attempt(self, &mut guard, &source, policy, &kind, deadline),
                    )
                    .await
                    .map_err(|_| image_error("image preparation deadline"))?
                });
                // No transaction/future survives into observations or validation.
                self.runtime.block_on(self.dice.wait_for_idle());
                match attempt? {
                    ImageAttempt::Need(need) => {
                        guard.progress(&need)?;
                    }
                    ImageAttempt::Complete(certificate, metadata, selected) => {
                        let command = guard.command();
                        let request = command
                            .image_request
                            .as_ref()
                            .expect("current image request");
                        let source = request.source(command)?;
                        let active = || {
                            std::time::Instant::now() < deadline
                                && self
                                    .repository_materializer
                                    .session_is_active(command.repository_session)
                        };
                        let prepared = match metadata {
                            Some(metadata) => {
                                // Full metadata proof already bound its facts to the exact
                                // expected bytes. Revalidate current full source frontier.
                                input_image::verify_frontier(
                                    &source,
                                    &certificate,
                                    &self.image_resources,
                                    &active,
                                )
                                .map_err(NativeDemandSessionError::InputImage)?;
                                PreparedInput::Elf(metadata)
                            }
                            None => PreparedInput::Captured(
                                input_image::capture_input_image(
                                    input_image::AuthorizedObservedInputImage::new(
                                        source,
                                        certificate,
                                    ),
                                    &self.image_resources,
                                    &active,
                                )
                                .map_err(NativeDemandSessionError::InputImage)?,
                            ),
                        };
                        drop(selected);
                        return Ok(prepared);
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

async fn prepare_attempt(
    runtime: &WorkspaceRuntime,
    guard: &mut NativeDemandAbortGuard<'_>,
    source: &DeclaredImageSource,
    policy: InputImagePolicy,
    kind: &PreparationKind,
    deadline: std::time::Instant,
) -> Result<ImageAttempt, NativeDemandSessionError> {
    let mut updater = runtime
        .dice
        .updater_with_data(guard.attempt_user_computation_data()?);
    guard.inject_attempt(&mut updater)?;
    let mut transaction = runtime.request_revision.commit(updater).await;
    let (key, repository) = match select_source(runtime, &mut transaction, source, policy).await? {
        SourceSelection::Need(need) => {
            guard.seal_retry()?;
            return Ok(ImageAttempt::Need(need));
        }
        SourceSelection::Ready(key, repository) => (key, repository),
    };
    guard
        .command
        .as_mut()
        .expect("active image command")
        .image_request = Some(DeclaredImageRequest {
        key: key.clone(),
        elf: None,
        deadline,
        repository,
    });
    let certificate = match transaction
        .compute(&key)
        .await
        .map_err(|error| NativeDemandSessionError::Computation(error.into()))?
    {
        PathOutcome::Need(need) => {
            guard.seal_retry()?;
            return Ok(ImageAttempt::Need(
                slug_bzlmod_v2::SourcePreparationNeeds::path(need),
            ));
        }
        PathOutcome::Complete(result) => {
            result.map_err(|_| image_error("image producer refused source"))?
        }
    };
    let budget = runtime.image_resources.descriptor_budget();
    if !certificate.same_budget(budget) {
        return Err(image_error("foreign cached image budget"));
    }
    let metadata = match kind {
        PreparationKind::Capture => None,
        PreparationKind::Elf {
            relative_file,
            policy,
        } => {
            let root = ImageChargedPath::pin_existing(key.source_root().clone(), budget)
                .map_err(|_| image_error("ELF selected path admission failed"))?;
            let file = join_relative(
                root,
                relative_file,
                key.policy().limits().max_path_bytes,
                budget,
            )?;
            let elf_key = ObservedElfMetadataKey::from_charged(key.clone(), file.clone(), *policy)
                .map_err(|_| image_error("invalid ELF selector"))?;
            // Only a completed normal image producer may mint native allowance.
            guard
                .command
                .as_mut()
                .expect("active image command")
                .image_request
                .as_mut()
                .expect("image request")
                .elf = Some(DeclaredElfRequest {
                certificate: certificate.clone(),
                file,
                policy: *policy,
            });
            match transaction
                .compute(&elf_key)
                .await
                .map_err(|error| NativeDemandSessionError::Computation(error.into()))?
            {
                PathOutcome::Need(need) => {
                    guard.seal_retry()?;
                    return Ok(ImageAttempt::Need(
                        slug_bzlmod_v2::SourcePreparationNeeds::path(need),
                    ));
                }
                PathOutcome::Complete(result) => {
                    Some(result.map_err(|_| image_error("ELF producer refused source"))?)
                }
            }
        }
    };
    if metadata
        .as_ref()
        .is_some_and(|metadata| !metadata.same_budget(budget) || metadata.image() != &certificate)
    {
        return Err(image_error("foreign ELF certificate"));
    }
    let selected = guard.seal_terminal()?.select(&transaction).await?;
    let frontier = metadata
        .as_ref()
        .map_or(certificate.observations(), |metadata| {
            metadata.observations()
        });
    let reachable = selected.sidecars().demands();
    for (demand, observed) in frontier.observations() {
        if !reachable.unscoped_paths().contains(demand)
            || guard.command().path_observations.get(demand) != Some(observed)
        {
            return Err(image_error(
                "image/ELF frontier is not current reachable evidence",
            ));
        }
    }
    if let Some((request, _)) = &guard
        .command()
        .image_request
        .as_ref()
        .expect("image request")
        .repository
    {
        if !reachable.repository_requests().contains(request) {
            return Err(image_error("image repository is not reachable"));
        }
    }
    Ok(ImageAttempt::Complete(certificate, metadata, selected))
}

type SelectedRepository = Option<(
    Arc<RepositoryMaterializationRequest>,
    slug_bzlmod_v2::RepositoryMaterialization,
)>;
enum SourceSelection {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Ready(ObservedInputImageKey, SelectedRepository),
}

async fn select_source(
    runtime: &WorkspaceRuntime,
    transaction: &mut dice::DiceTransaction,
    source: &DeclaredImageSource,
    policy: InputImagePolicy,
) -> Result<SourceSelection, NativeDemandSessionError> {
    let budget = runtime.image_resources.descriptor_budget();
    let (namespace, anchor, subtree, repository) = match source {
        DeclaredImageSource::WorkspaceSubtree(subtree) => (
            PathObservationNamespace::Host,
            ImageChargedPath::pin_existing(
                runtime.repository_materializer.workspace().clone(),
                budget,
            )
            .map_err(|_| image_error("image root descriptor admission failed"))?,
            subtree,
            None,
        ),
        DeclaredImageSource::ImmutableRepository { request, subtree } => {
            if request.id.workspace.as_path() != runtime.workspace {
                return Err(image_error("foreign image repository"));
            }
            let result =
                slug_bzlmod_v2::source_preparation::compute_repository_materialization_result(
                    transaction,
                    request.clone(),
                )
                .await
                .map_err(NativeDemandSessionError::Computation)?;
            let materialized = match result {
                slug_bzlmod_v2::SourcePreparationOutcome::Need(need) => {
                    return Ok(SourceSelection::Need(need));
                }
                slug_bzlmod_v2::SourcePreparationOutcome::Complete(result) => result
                    .as_ref()
                    .clone()
                    .map_err(|_| image_error("image repository materialization failed"))?,
            };
            let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
                generation_root,
                observation_instance,
                ..
            } = &materialized
            else {
                return Err(image_error("image source is not immutable"));
            };
            let _normalization_credit = budget
                .try_reserve(
                    generation_root
                        .as_os_str()
                        .len()
                        .saturating_mul(2)
                        .saturating_add(128),
                )
                .map_err(|_| image_error("image root descriptor admission failed"))?;
            let normalized = NormalizedAbsolutePath::new(generation_root.clone())
                .map_err(|_| image_error("invalid materialization root"))?;
            let anchor = ImageChargedPath::pin_existing(normalized, budget)
                .map_err(|_| image_error("image root descriptor admission failed"))?;
            (
                PathObservationNamespace::Materialization(*observation_instance),
                anchor,
                subtree,
                Some((request.clone(), materialized)),
            )
        }
    };
    let root = join_relative(
        anchor.clone(),
        subtree,
        policy.limits().max_path_bytes,
        budget,
    )?;
    let key = ObservedInputImageKey::from_charged(namespace, anchor, root, policy)
        .map_err(|_| image_error("invalid image policy"))?;
    Ok(SourceSelection::Ready(key, repository))
}

fn validate_relative(path: &Path, limit: usize) -> Result<(), NativeDemandSessionError> {
    if path.is_absolute()
        || path.as_os_str().len() > limit
        || path.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(image_error("invalid image relative selector"));
    }
    Ok(())
}

fn join_relative(
    mut root: ImageChargedPath,
    relative: &Path,
    limit: usize,
    budget: &slug_workspace_v2::ImageDescriptorBudget,
) -> Result<ImageChargedPath, NativeDemandSessionError> {
    validate_relative(relative, limit)?;
    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            if root
                .as_path()
                .as_os_str()
                .len()
                .saturating_add(name.len())
                .saturating_add(1)
                > limit
            {
                return Err(image_error("image selected path too long"));
            }
            // Temporary component ownership is charged before OS-string allocation.
            let _name_credit = budget
                .try_reserve(name.len().saturating_add(64))
                .map_err(|_| image_error("image component descriptor admission failed"))?;
            let name = PathDirectoryName::new(name.to_owned())
                .map_err(|_| image_error("invalid image component"))?;
            root = root
                .try_join_component(&name)
                .map_err(|_| image_error("image path descriptor admission failed"))?;
        }
    }
    Ok(root)
}
