//! Shared declared image/ELF preparation, current closure and transient cleanup.

use super::*;

pub(super) enum PreparationKind {
    Capture,
    SelectedElf(ElfMetadataPolicy),
    Elf {
        relative_file: PathBuf,
        policy: ElfMetadataPolicy,
    },
}
pub(super) enum PreparedInput {
    Captured(
        CapturedInputImageLease,
        Option<slug_bzlmod_v2::ObservedSelectedLabelSource>,
    ),
    Elf(
        ObservedElfMetadata,
        Option<slug_bzlmod_v2::ObservedSelectedLabelSource>,
    ),
}
enum ImageAttempt {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Complete(
        ObservedInputImage,
        Option<ObservedElfMetadata>,
        NativeDemandTerminalSelection,
        Option<slug_bzlmod_v2::ObservedSelectedLabelSource>,
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
            PreparedInput::Captured(image, _) => Ok(image),
            PreparedInput::Elf(..) => unreachable!("capture mode"),
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
            PreparedInput::Elf(metadata, _) => Ok(metadata),
            PreparedInput::Captured(..) => unreachable!("ELF mode"),
        }
    }

    pub(super) fn prepare_declared_input(
        &self,
        source: DeclaredImageSource,
        policy: InputImagePolicy,
        kind: PreparationKind,
    ) -> Result<PreparedInput, NativeDemandSessionError> {
        self.prepare_declared_input_inner(
            source,
            policy,
            kind,
            #[cfg(test)]
            None,
        )
    }

    pub(super) fn prepare_declared_input_inner(
        &self,
        source: DeclaredImageSource,
        policy: InputImagePolicy,
        kind: PreparationKind,
        #[cfg(test)] before_final: Option<&dyn Fn()>,
    ) -> Result<PreparedInput, NativeDemandSessionError> {
        if matches!(
            (&source, &kind),
            (
                DeclaredImageSource::SelectedLabel(_),
                PreparationKind::Elf { .. }
            ) | (
                DeclaredImageSource::WorkspaceSubtree(_)
                    | DeclaredImageSource::ImmutableRepository { .. },
                PreparationKind::SelectedElf(_)
            )
        ) {
            return Err(image_error("invalid selected Label preparation mode"));
        }
        if let DeclaredImageSource::SelectedLabel(key) = &source {
            label_source::check_key(self, key, policy)?;
        }
        // Label resolution observes ancestors to '/'. Create the private empty
        // output before command preflight, then fill that same directory later.
        let mut prepared_stage = if matches!(
            (&source, &kind),
            (
                DeclaredImageSource::SelectedLabel(_),
                PreparationKind::Capture
            )
        ) {
            Some(
                input_image::prepare_input_image_stage(&self.image_resources)
                    .map_err(NativeDemandSessionError::InputImage)?,
            )
        } else {
            None
        };
        let mut guard =
            NativeDemandAbortGuard::new(self.begin_native_demand_command()?.into_command());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let prefix_depth = match &source {
            DeclaredImageSource::SelectedLabel(key) => key
                .recipe()
                .workspace()
                .as_path()
                .components()
                .count()
                .saturating_add(
                    key.recipe()
                        .label()
                        .package()
                        .package()
                        .as_str()
                        .split('/')
                        .count(),
                ),
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
                    ImageAttempt::Complete(certificate, metadata, selected, label) => {
                        let command = guard.command();
                        let request = command
                            .image_request
                            .as_ref()
                            .expect("current image request");
                        if let Some(label) = &label {
                            label_source::check_declaration(self, request, label)?;
                        }
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
                                #[cfg(test)]
                                if let Some(hook) = before_final {
                                    hook();
                                }
                                if let Some(label) = &label {
                                    label_source::verify_current(
                                        command,
                                        label,
                                        &certificate,
                                        Some(&metadata),
                                    )?;
                                }
                                PreparedInput::Elf(metadata, label)
                            }
                            None => {
                                let authorized = input_image::AuthorizedObservedInputImage::new(
                                    source,
                                    certificate.clone(),
                                );
                                let captured = match prepared_stage.take() {
                                    Some(stage) => input_image::capture_prepared_input_image(
                                        authorized,
                                        &self.image_resources,
                                        &active,
                                        stage,
                                    ),
                                    None => input_image::capture_input_image(
                                        authorized,
                                        &self.image_resources,
                                        &active,
                                    ),
                                }
                                .map_err(NativeDemandSessionError::InputImage)?;
                                #[cfg(test)]
                                if let Some(hook) = before_final {
                                    hook();
                                }
                                if let Some(label) = &label {
                                    label_source::verify_current(
                                        command,
                                        label,
                                        &certificate,
                                        None,
                                    )?;
                                }
                                PreparedInput::Captured(captured, label)
                            }
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
    let (key, repository, label) =
        match select_source(runtime, &mut transaction, source, policy).await? {
            SourceSelection::Need(need) => {
                guard.seal_retry()?;
                return Ok(ImageAttempt::Need(need));
            }
            SourceSelection::Ready(key, repository, label) => (key, repository, label),
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
    if let Some(label) = &label {
        label_source::check_member(runtime, label, &certificate)?;
    }
    let selected_elf_kind = if let PreparationKind::SelectedElf(policy) = kind {
        let label = label
            .as_ref()
            .ok_or_else(|| image_error("missing selected Label"))?;
        Some((label.selected_file().clone(), *policy))
    } else {
        None
    };
    let legacy_elf_file = match kind {
        PreparationKind::Elf {
            relative_file,
            policy,
        } => {
            let root = ImageChargedPath::pin_existing(key.source_root().clone(), budget)
                .map_err(|_| image_error("ELF selected path admission failed"))?;
            Some((
                join_relative(
                    root,
                    relative_file,
                    key.policy().limits().max_path_bytes,
                    budget,
                )?,
                *policy,
            ))
        }
        _ => None,
    };
    let metadata = match selected_elf_kind.or(legacy_elf_file) {
        None => None,
        Some((file, policy)) => {
            let elf_key = ObservedElfMetadataKey::from_charged(key.clone(), file.clone(), policy)
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
                policy,
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
    if let Some(label) = &label {
        label_source::check_reachable(guard.command(), label, reachable)?;
    }
    for (demand, observed) in frontier.observations() {
        if !reachable.unscoped_paths().contains(demand)
            || guard.command().path_observations.get(demand) != Some(observed)
        {
            return Err(image_error(
                "image/ELF frontier is not current reachable evidence",
            ));
        }
    }
    if let Some(repository) = &guard
        .command()
        .image_request
        .as_ref()
        .expect("image request")
        .repository
    {
        let (request, _) = repository.selection();
        if !reachable.repository_requests().contains(request) {
            return Err(image_error("image repository is not reachable"));
        }
    }
    Ok(ImageAttempt::Complete(
        certificate,
        metadata,
        selected,
        label,
    ))
}

type SelectedRepository = Option<DeclaredImageRepository>;
pub(super) enum SourceSelection {
    Need(slug_bzlmod_v2::SourcePreparationNeeds),
    Ready(
        ObservedInputImageKey,
        SelectedRepository,
        Option<slug_bzlmod_v2::ObservedSelectedLabelSource>,
    ),
}

async fn select_source(
    runtime: &WorkspaceRuntime,
    transaction: &mut dice::DiceTransaction,
    source: &DeclaredImageSource,
    policy: InputImagePolicy,
) -> Result<SourceSelection, NativeDemandSessionError> {
    let budget = runtime.image_resources.descriptor_budget();
    if let DeclaredImageSource::SelectedLabel(key) = source {
        return label_source::select(runtime, transaction, key, policy).await;
    }
    let (namespace, anchor, subtree, repository) = match source {
        DeclaredImageSource::SelectedLabel(_) => unreachable!("selected Label handled above"),
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
                Some(DeclaredImageRepository::Explicit(
                    request.clone(),
                    materialized,
                )),
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
    Ok(SourceSelection::Ready(key, repository, None))
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
