//! Activation of admitted explicit requested roots. Provider metadata and source
//! content remain owned by their existing Analysis and observation keys.

use slug_build_api_v2::AnalysisArtifact;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteKey;
use slug_loading_v2::HostCanonicalRepositoryLoadRouteObservationKey;
use slug_loading_v2::PackageTargetKind;

use super::super::source_input::SourceArtifactInputObservationKey;
use super::*;

type ActivationResult<T> = Result<T, BuildBranchResult>;

fn analysis_failure(error: AnalysisError) -> BuildBranchResult {
    build_branch_complete(Err(BuildCommandError::new(
        BuildCommandErrorKind::Analysis(error),
    )))
}

/// Preserve a completed source observation when subsequent metadata fails.
/// Needs and outer frontier failures retain their existing retry precedence.
pub(super) fn with_source_certificate(
    failure: BuildBranchResult,
    source_certificate: Option<SourceCertificate>,
) -> BuildBranchResult {
    let Some(certificate) = source_certificate else {
        return failure;
    };
    let error = match failure {
        BuildBranchResult::Outcome(PreparationOutcome::Complete(Err(error))) => error,
        BuildBranchResult::Infrastructure(error) => BuildCommandError::infrastructure(error),
        other => return other,
    };
    build_branch_complete(Err(BuildCommandError::new(
        BuildCommandErrorKind::SourceCertified {
            error: Box::new(error),
            source_certificate: Some(Box::new(certificate)),
        },
    )))
}

/// The external build bridge and Analysis must depend on the same package
/// inventory producer. Keep the apparent route separately for source bytes.
pub(super) async fn canonical_package_input(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    source_route: &HostRepositorySourceRoute,
    mode: BuildAnalysisMode,
) -> ActivationResult<(
    Result<slug_bzlmod_v2::HostCanonicalRepositorySourceInput, BuildCommandError>,
    PathObservationEpoch,
)> {
    if let HostRepositorySourceRoute::Canonical(input) = source_route {
        return Ok((Ok(input.clone()), PathObservationEpoch::empty()));
    }
    let (route, observations) = match mode {
        BuildAnalysisMode::Legacy => {
            let key = HostCanonicalRepositoryLoadRouteKey::new(
                workspace.dupe(),
                source_route.canonical_repo().clone(),
            );
            match ctx.compute(&key).await {
                Err(error) => {
                    return Err(BuildBranchResult::Infrastructure(Arc::from(
                        error.to_string(),
                    )));
                }
                Ok(PreparationOutcome::Need(need)) => return Err(build_branch_need(need)),
                Ok(PreparationOutcome::Complete(route)) => (route, PathObservationEpoch::empty()),
            }
        }
        BuildAnalysisMode::Observed => {
            let key = HostCanonicalRepositoryLoadRouteObservationKey::new(
                workspace.dupe(),
                source_route.canonical_repo().clone(),
            );
            match ctx.compute(&key).await {
                Err(error) => {
                    return Err(BuildBranchResult::Infrastructure(Arc::from(
                        error.to_string(),
                    )));
                }
                Ok(PreparationOutcome::Need(need)) => return Err(build_branch_need(need)),
                Ok(PreparationOutcome::Complete(Err(error))) => {
                    return Err(match error.selected_frontier() {
                        slug_bzlmod_v2::HostSelectedObservationFrontier::Path(error) => {
                            BuildBranchResult::ObservedOuter(error)
                        }
                        slug_bzlmod_v2::HostSelectedObservationFrontier::Infrastructure(_) => {
                            build_branch_complete(Err(BuildCommandError::new(
                                BuildCommandErrorKind::CanonicalRepositoryRouteObservation(error),
                            )))
                        }
                    });
                }
                Ok(PreparationOutcome::Complete(Ok(observed))) => {
                    (observed.result().dupe(), observed.observations().dupe())
                }
            }
        }
    };
    let input = route
        .as_ref()
        .as_ref()
        .map(|route| route.input().clone())
        .map_err(|error| {
            BuildCommandError::new(BuildCommandErrorKind::CanonicalRepositoryRoute(
                error.clone(),
            ))
        });
    // Keep the canonical route's epoch even when its semantic result fails.
    Ok((input, observations))
}

pub(super) async fn compute_node(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    node: ConfiguredNodeKey,
    mode: BuildAnalysisMode,
) -> ActivationResult<Arc<ConfiguredNodeResult>> {
    let value = match mode {
        BuildAnalysisMode::Legacy => {
            let key =
                ConfiguredNodeAnalysisKey::new(workspace.dupe(), node).map_err(analysis_failure)?;
            match ctx.compute(&key).await {
                Err(error) => {
                    return Err(BuildBranchResult::Infrastructure(Arc::from(
                        error.to_string(),
                    )));
                }
                Ok(PreparationOutcome::Need(need)) => return Err(build_branch_need(need)),
                Ok(PreparationOutcome::Complete(value)) => value,
            }
        }
        BuildAnalysisMode::Observed => {
            let key = ConfiguredNodeAnalysisObservationKey::new(workspace.dupe(), node)
                .map_err(analysis_failure)?;
            match ctx.compute(&key).await {
                Err(error) => {
                    return Err(BuildBranchResult::Infrastructure(Arc::from(
                        error.to_string(),
                    )));
                }
                Ok(PreparationOutcome::Need(need)) => return Err(build_branch_need(need)),
                Ok(PreparationOutcome::Complete(Err(error))) => {
                    return Err(BuildBranchResult::ObservedOuter(error));
                }
                Ok(PreparationOutcome::Complete(Ok(value))) => value,
            }
        }
    };
    value.as_ref().clone().map_err(analysis_failure)
}

async fn prepare_analysis(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: CanonicalLabel,
    configuration: &ConfigurationKey,
    mode: BuildAnalysisMode,
) -> ActivationResult<Arc<ConfiguredNodeResult>> {
    let node = match mode {
        BuildAnalysisMode::Legacy => match prepare_configured_node_analysis(
            ctx,
            workspace.dupe(),
            label,
            configuration.clone(),
        )
        .await
        {
            PreparationOutcome::Need(need) => return Err(build_branch_need(need)),
            PreparationOutcome::Complete(Err(error)) => return Err(analysis_failure(error)),
            PreparationOutcome::Complete(Ok(key)) => key.node().clone(),
        },
        BuildAnalysisMode::Observed => match prepare_configured_node_analysis_observed(
            ctx,
            workspace.dupe(),
            label,
            configuration.clone(),
        )
        .await
        {
            PreparationOutcome::Need(need) => return Err(build_branch_need(need)),
            PreparationOutcome::Complete(Err(error)) => {
                return Err(BuildBranchResult::ObservedOuter(error));
            }
            PreparationOutcome::Complete(Ok(Err(error))) => return Err(analysis_failure(error)),
            PreparationOutcome::Complete(Ok(Ok(key))) => key.node().clone(),
        },
    };
    compute_node(ctx, workspace, node, mode).await
}

async fn initialize_revision(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    eligible: bool,
) -> ActivationResult<()> {
    if eligible {
        ctx.compute(&RequestRevisionKey::new(workspace.dupe()))
            .await
            .map_err(|error| BuildBranchResult::Infrastructure(Arc::from(error.to_string())))?;
    }
    Ok(())
}

async fn observe_root_source(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    label: &slug_identity_v2::ApparentLabel,
    revision_eligible: bool,
) -> ActivationResult<Option<SourceCertificate>> {
    initialize_revision(ctx, workspace, revision_eligible).await?;
    let path = workspace
        .as_path()
        .join(label.package().as_str())
        .join(label.target().as_str());
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(path).expect("root package target stays within the workspace"),
        PathObservationOperation::FileBytes,
    );
    let result = match ctx.compute(&PathObservationKey::new(demand.dupe())).await {
        Err(error) => {
            return Err(BuildBranchResult::Infrastructure(Arc::from(
                error.to_string(),
            )));
        }
        Ok(PathOutcome::Need(need)) => {
            return Err(build_branch_need(
                slug_bzlmod_v2::SourcePreparationNeeds::path(need),
            ));
        }
        Ok(PathOutcome::Complete(result)) => result,
    };
    let certificate = revision_eligible.then(|| SourceCertificate::new(demand, result.clone()));
    if matches!(
        result.as_ref(),
        PathObservationResult::FileBytes(PathOperationResult::Present(_))
    ) {
        Ok(certificate)
    } else {
        Err(build_branch_complete(Err(BuildCommandError::new(
            BuildCommandErrorKind::RootSource {
                observation: (*result).clone(),
                source_certificate: certificate.map(Box::new),
            },
        ))))
    }
}

async fn observe_alias_source(
    ctx: &mut DiceComputations<'_>,
    workspace: &NormalizedAbsolutePath,
    actual: &ConfiguredNodeResult,
    revision_eligible: bool,
    mode: BuildAnalysisMode,
) -> ActivationResult<Option<SourceCertificate>> {
    initialize_revision(ctx, workspace, revision_eligible).await?;
    // The actual node was authoritatively validated as SourceFile. Its canonical
    // identity is the source identity; repository routes remain observation-owned.
    let artifact = AnalysisArtifact::Source(actual.key().label().clone());
    let key = SourceArtifactInputObservationKey::new(workspace.dupe(), &artifact)
        .expect("authoritative source node has a source artifact");
    let observed = match ctx.compute(&key).await {
        Err(error) => {
            return Err(BuildBranchResult::Infrastructure(Arc::from(
                error.to_string(),
            )));
        }
        Ok(PreparationOutcome::Need(need)) => return Err(build_branch_need(need)),
        Ok(PreparationOutcome::Complete(Err(error))) => {
            return Err(match mode {
                BuildAnalysisMode::Observed => BuildBranchResult::ObservedOuter(error),
                // This shared source owner is observed-only. The legacy build
                // boundary has no outer frontier channel; report its failure
                // without violating that boundary's no-outer-error invariant.
                BuildAnalysisMode::Legacy => {
                    build_branch_complete(Err(BuildCommandError::infrastructure(error)))
                }
            });
        }
        Ok(PreparationOutcome::Complete(Ok(observed))) => observed,
    };
    complete_alias_source_observation(
        observed.result(),
        observed.observations(),
        revision_eligible,
    )
}

fn complete_alias_source_observation(
    result: &Result<
        super::super::source_input::SourceArtifactInput,
        super::super::source_input::SourceArtifactInputError,
    >,
    observations: &PathObservationEpoch,
    revision_eligible: bool,
) -> ActivationResult<Option<SourceCertificate>> {
    match result {
        Ok(_) if revision_eligible => SourceCertificate::from_epoch(observations.dupe())
            .map(Some)
            .map_err(|error| build_branch_complete(Err(BuildCommandError::infrastructure(error)))),
        Ok(_) => Ok(None),
        Err(error) => {
            // Route failures may precede the first path observation. Preserve
            // that failure without manufacturing a certificate for an empty epoch.
            let certificate = revision_eligible
                .then(|| SourceCertificate::from_epoch(observations.dupe()).ok())
                .flatten();
            Err(with_source_certificate(
                build_branch_complete(Err(BuildCommandError::new(
                    BuildCommandErrorKind::SourceArtifactInput(error.clone()),
                ))),
                certificate,
            ))
        }
    }
}

pub(super) async fn compute_loaded_build_branch(
    ctx: &mut DiceComputations<'_>,
    key: &BuildCommandRootKey,
    pattern: Arc<str>,
    parsed: TargetPattern,
    package: PackagePath,
    package_value: LoadedPackage,
    configuration: &ConfigurationKey,
    mode: BuildAnalysisMode,
) -> BuildBranchResult {
    let revision_eligible = key.initializes_request_revision()
        || matches!(mode, BuildAnalysisMode::Observed) && key.observed_multi_root();
    let mut analysis = None;
    let mut completion = BuildTargetCompletion::LoadedOnly;
    let mut source_certificate = None;
    if let TargetPattern::Single(label) = parsed {
        let Some(target) = package_value
            .targets
            .iter()
            .find(|target| target.name == label.target().as_str())
        else {
            return build_branch_complete(Err(BuildCommandError::target_not_found(
                pattern,
                package,
                label.target().clone(),
                package_value.build_file.clone(),
            )));
        };
        let source = matches!(target.kind, PackageTargetKind::ExportedFile);
        if source {
            source_certificate =
                match observe_root_source(ctx, &key.workspace, &label, revision_eligible).await {
                    Ok(certificate) => certificate,
                    Err(failure) => return failure,
                };
            completion = BuildTargetCompletion::ObservedExportedSource;
        }
        if source
            || matches!(
                target.kind,
                PackageTargetKind::StarlarkRule(_)
                    | PackageTargetKind::Alias { .. }
                    | PackageTargetKind::GeneratedFile { .. }
            )
        {
            let canonical =
                CanonicalLabel::parse(&format!("@@//{}:{}", label.package(), label.target()))
                    .expect("validated root apparent label has a canonical projection");
            let requested =
                match prepare_analysis(ctx, &key.workspace, canonical, configuration, mode).await {
                    Ok(analysis) => analysis,
                    Err(failure) => return with_source_certificate(failure, source_certificate),
                };
            if requested.kind() == &ConfiguredNodeKind::Alias {
                let actual = match compute_node(
                    ctx,
                    &key.workspace,
                    requested.actual_target().clone(),
                    mode,
                )
                .await
                {
                    Ok(actual) => actual,
                    Err(failure) => return failure,
                };
                match actual.kind() {
                    ConfiguredNodeKind::Rule | ConfiguredNodeKind::GeneratedFile => {}
                    ConfiguredNodeKind::SourceFile => {
                        source_certificate = match observe_alias_source(
                            ctx,
                            &key.workspace,
                            &actual,
                            revision_eligible,
                            mode,
                        )
                        .await
                        {
                            Ok(certificate) => certificate,
                            Err(failure) => return failure,
                        };
                    }
                    kind => {
                        return analysis_failure(AnalysisError::message(format!(
                            "requested alias {} has unsupported actual target kind {kind:?}",
                            requested.key().label(),
                        )));
                    }
                }
            }
            analysis = Some(requested);
            if !source {
                completion = BuildTargetCompletion::Analyzed;
            }
        }
    }
    build_branch_complete(Ok(BuildRequestedTarget {
        pattern,
        package: package_value,
        analysis,
        completion,
        source_certificate,
    }))
}

#[cfg(test)]
#[path = "build_requested_target/tests.rs"]
mod tests;
