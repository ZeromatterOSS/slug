//! Original retry loop over precisely requested path and source bindings.
use super::*;
pub(super) async fn invoke_repository_rule_with_label_paths(
    ctx: &mut DiceComputations<'_>,
    key: &HostSelectedRepositoryFileEffectKey,
    certificate: &Arc<HostSelectedExtensionOwnerCertificate>,
    module: &crate::bzl_module::FrozenBzlModule,
    implementation: starlark::values::FrozenValue,
    input: &RepositoryRuleInvocationInput,
    platform: &slug_bzlmod_v2::RepositoryPlatform,
    transaction: &RepositoryHostInputTransaction,
    mode: EffectMode,
    mut observations: PathObservationEpoch,
    capture_enabled: bool,
) -> Result<
    (
        crate::repository_rule_context::RepositoryRuleInvocation,
        Option<InvocationPrintCapture>,
        PathObservationEpoch,
    ),
    EffectDriver,
> {
    let mut prepared_paths = PreparedRepositoryLabelPaths::new();
    let mut prepared_bindings = PreparedRepositoryWorkspaceBindings::new();
    let mut prepared_sources = PreparedRepositoryLabelSources::new();
    let mut prepared_which = PreparedRepositoryWhichCandidates::new();
    let mut prepared_host_paths = PreparedRepositoryHostPaths::new();
    let mut prepared_host_reads = SmallMap::new();
    let filesystem_epoch_hint = ctx
        .per_transaction_data()
        .data
        .get::<RepositoryFilesystemEpoch>()
        .ok()
        .cloned();
    let mut prepared_filesystem_operations = RepositoryFilesystemTranscript::empty();
    let mut filesystem_nonce = None;
    loop {
        let capture = capture_enabled.then(InvocationPrintCapture::default);
        let attempt = invoke_repository_rule_with_host_paths(
            implementation,
            &module.manifest,
            &prepared_paths,
            &prepared_sources,
            &prepared_which,
            &prepared_host_paths,
            if input.has_workspace() {
                input
                    .clone()
                    .with_host_reads(prepared_host_reads.clone())
                    .with_workspace_bindings(prepared_bindings.clone())
            } else {
                input
                    .clone()
                    .with_filesystem_operations(
                        key.owner.clone(),
                        key.ordinal,
                        prepared_filesystem_operations.clone(),
                    )
                    .with_filesystem_lease_nonce(filesystem_nonce)
            },
            platform.clone(),
            transaction.snapshot().dupe(),
            capture.as_ref().map(|capture| capture as &dyn PrintHandler),
        );
        let capture = attempt
            .general_events
            .map(|events| InvocationPrintCapture {
                events: RefCell::new(events.events().to_vec()),
            })
            .or(capture);
        if attempt.environment_enumerated {
            verify_enumerated_environment(ctx, key, certificate, transaction, &observations)
                .await?;
        }
        match attempt.result {
            Ok(invocation) => {
                if !input.has_workspace()
                    && invocation.filesystem_realization.is_none()
                    && filesystem_epoch_hint
                        .as_ref()
                        .and_then(RepositoryFilesystemEpoch::transcript)
                        .is_some_and(|transcript| {
                            transcript
                                .records_for(&key.owner, key.ordinal)
                                .next()
                                .is_some()
                        })
                {
                    // A surplus prepared record is itself an observed semantic
                    // rejection: retain the epoch edge so clearing it can retry.
                    verified_filesystem_epoch(
                        ctx,
                        key,
                        certificate,
                        filesystem_epoch_hint.as_ref(),
                        &observations,
                    )
                    .await?;
                    return Err(complete_effect_error(
                        terminal_repository_rule_invocation_error(
                            certificate,
                            key.ordinal,
                            RepositoryRuleInvocationError::FilesystemReplay,
                        ),
                        observations,
                    ));
                }
                return Ok((invocation, capture, observations));
            }
            Err(RepositoryRuleInvocationError::WorkspaceNeed(demand)) => {
                return Err(SourcePreparationOutcome::Need(
                    SourcePreparationNeeds::repository_workspace_operation(demand),
                ));
            }
            Err(RepositoryRuleInvocationError::LabelPathNeed(address)) => {
                if prepared_paths.contains_key(&address)
                    || prepared_paths.len() == RepositoryEffectLimits::LABEL_PATHS
                {
                    let message = if prepared_paths.contains_key(&address) {
                        "repository_ctx.path repeated an already prepared Label path".into()
                    } else {
                        format!(
                            "repository_ctx.path exceeds the per-invocation limit of {} distinct Labels", RepositoryEffectLimits::LABEL_PATHS
                        )
                        .into()
                    };
                    return Err(complete_effect_error(
                        HostSelectedRepositoryFileEffectError::Invocation {
                            certificate: certificate.clone(),
                            ordinal: key.ordinal,
                            message,
                        },
                        observations,
                    ));
                }
                let ((value, binding), next_observations) = resolve_repository_label_path(
                    ctx,
                    key,
                    certificate,
                    address.clone(),
                    mode,
                    observations,
                )
                .await?;
                observations = next_observations;
                if let Some(binding) = binding {
                    let repository = binding.repository().clone();
                    if prepared_bindings
                        .get(&repository)
                        .is_some_and(|previous| previous != &binding)
                    {
                        return Err(repository_source_error(
                            certificate,
                            key.ordinal,
                            "contextual source binding changed",
                            observations,
                        ));
                    }
                    prepared_bindings.insert(repository, binding);
                }
                prepared_paths.insert(address, value);
            }
            Err(RepositoryRuleInvocationError::HostReadNeed(path)) => {
                if prepared_host_reads.contains_key(&path)
                    || prepared_host_reads.len() == RepositoryEffectLimits::HOST_PATHS
                {
                    return Err(complete_effect_error(
                        host_input_error(
                            certificate,
                            key.ordinal,
                            "repository_ctx.read repeated or exceeded Host path limit",
                        ),
                        observations,
                    ));
                }
                let (bytes, next) =
                    general::read_host(ctx, key, certificate, path, mode, observations).await?;
                observations = next;
                prepared_host_reads.insert(bytes.0, bytes.1);
            }
            Err(RepositoryRuleInvocationError::HostPathNeed(path)) => {
                if prepared_host_paths.contains_key(&path)
                    || prepared_host_paths.len() == RepositoryEffectLimits::HOST_PATHS
                {
                    return Err(complete_effect_error(
                        HostSelectedRepositoryFileEffectError::Invocation {
                            certificate: certificate.clone(),
                            ordinal: key.ordinal,
                            message: "repository_ctx.path repeated or exceeded its host path limit"
                                .into(),
                        },
                        observations,
                    ));
                }
                let (exists, next_observations) = resolve_host_path_exists(
                    ctx,
                    key,
                    certificate,
                    path.clone(),
                    mode,
                    observations,
                )
                .await?;
                observations = next_observations;
                prepared_host_paths.insert(path, exists);
            }
            Err(RepositoryRuleInvocationError::RepositorySourceNeed(address)) => {
                if prepared_sources.contains_key(&address)
                    || prepared_sources.len() == RepositoryEffectLimits::LABEL_PATHS
                {
                    return Err(repository_source_error(
                        certificate,
                        key.ordinal,
                        "repository_ctx repeated or exceeded its Label source limit",
                        observations,
                    ));
                }
                let (bytes, next_observations) =
                    resolve_repository_source(ctx, key, certificate, &address, mode, observations)
                        .await?;
                observations = next_observations;
                prepared_sources.insert(address, bytes);
            }
            Err(RepositoryRuleInvocationError::WhichNeed(candidate)) => {
                if let Some(message) = which_candidate_retry_error(&prepared_which, &candidate) {
                    return Err(complete_effect_error(
                        HostSelectedRepositoryFileEffectError::Invocation {
                            certificate: certificate.clone(),
                            ordinal: key.ordinal,
                            message: message.into(),
                        },
                        observations,
                    ));
                }
                verify_which_environment(ctx, key, certificate, transaction, &observations).await?;
                let (value, next_observations) = resolve_which_candidate(
                    ctx,
                    key,
                    certificate,
                    candidate.clone(),
                    mode,
                    observations,
                )
                .await?;
                observations = next_observations;
                prepared_which.insert(candidate, value);
            }
            Err(RepositoryRuleInvocationError::FilesystemNeed(request)) => {
                let epoch = verified_filesystem_epoch(
                    ctx,
                    key,
                    certificate,
                    filesystem_epoch_hint.as_ref(),
                    &observations,
                )
                .await?;
                let transcript = epoch
                    .transcript()
                    .expect("active archive epoch has a transcript")
                    .for_owner(&key.owner, key.ordinal);
                match transcript.matching_record(&request) {
                    Ok(Some(_)) => {
                        prepared_filesystem_operations = transcript;
                        filesystem_nonce = epoch.lease_nonce();
                    }
                    Ok(None) if transcript.records().len() == request.id().call_index() => {
                        return Err(SourcePreparationOutcome::Need(
                            SourcePreparationNeeds::repository_filesystem_operation(
                                request.as_ref().clone(),
                            ),
                        ));
                    }
                    _ => {
                        return Err(complete_effect_error(
                            terminal_repository_rule_invocation_error(
                                certificate,
                                key.ordinal,
                                RepositoryRuleInvocationError::FilesystemReplay,
                            ),
                            observations,
                        ));
                    }
                }
            }
            Err(error) => {
                if matches!(error, RepositoryRuleInvocationError::WhichLimit) {
                    verify_which_environment(ctx, key, certificate, transaction, &observations)
                        .await?;
                }
                if let Some(capture) = capture {
                    ctx.store_evaluation_data(capture.into_batch())
                        .expect("repository-file invocation stores one local Complete event batch");
                }
                return Err(complete_effect_error(
                    terminal_repository_rule_invocation_error(certificate, key.ordinal, error),
                    observations,
                ));
            }
        }
    }
}
