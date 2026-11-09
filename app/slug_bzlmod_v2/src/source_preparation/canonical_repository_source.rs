/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use compact_str::CompactString;

use super::*;
use crate::HostCanonicalRepositoryRoute;
use crate::HostCanonicalRepositoryRouteKind;
use crate::RepositoryFilesystemRealization;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct HostCanonicalRepositorySourceInput {
    route: Arc<HostCanonicalRepositoryRoute>,
    disposition: HostRepositoryMaterializationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostCanonicalRepositorySourceInputError {
    Root,
    MissingRepositorySpecification,
    MissingGeneratedFileEffectPlan,
    ExtraneousGeneratedFileEffectPlan,
    MissingGeneratedFilesystemRealization,
    ExtraneousGeneratedFilesystemRealization,
    MismatchedGeneratedFilesystemRealization,
    MismatchedGeneratedWorkspaceRealization,
    ExtraneousGeneratedWorkspaceRealization,
    Projection(RepositoryMaterializationError),
}

#[derive(Debug, Clone, Copy)]
pub struct HostCanonicalRepositorySourceInputView<'a> {
    route: &'a Arc<HostCanonicalRepositoryRoute>,
    disposition: HostRepositorySourceInputDispositionView<'a>,
}

impl HostCanonicalRepositorySourceInput {
    pub fn view(&self) -> HostCanonicalRepositorySourceInputView<'_> {
        HostCanonicalRepositorySourceInputView {
            route: &self.route,
            disposition: match &self.disposition {
                HostRepositoryMaterializationDisposition::Builtin(identity) => {
                    HostRepositorySourceInputDispositionView::Builtin(identity)
                }
                HostRepositoryMaterializationDisposition::Request(request) => {
                    HostRepositorySourceInputDispositionView::Request(request)
                }
            },
        }
    }
}

impl Hash for HostCanonicalRepositorySourceInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.route.hash(state);
        std::mem::discriminant(&self.disposition).hash(state);
        match &self.disposition {
            HostRepositoryMaterializationDisposition::Builtin(identity) => identity.hash(state),
            HostRepositoryMaterializationDisposition::Request(request) => {
                request.id.hash(state);
                std::mem::discriminant(&request.kind).hash(state);
                match &request.kind {
                    RepositoryMaterializationKind::GeneratedFileEffects(plan) => plan.hash(state),
                    RepositoryMaterializationKind::GeneratedFilesystemEffects(realization) => {
                        realization.hash(state)
                    }
                    _ => {}
                }
            }
        }
    }
}

impl<'a> HostCanonicalRepositorySourceInputView<'a> {
    pub fn route(self) -> &'a Arc<HostCanonicalRepositoryRoute> {
        self.route
    }

    pub fn disposition(self) -> HostRepositorySourceInputDispositionView<'a> {
        self.disposition
    }
}

pub fn host_canonical_repository_source_input(
    route: Arc<HostCanonicalRepositoryRoute>,
    generated_plan: Option<GeneratedRepositoryFileEffectPlan>,
) -> Result<HostCanonicalRepositorySourceInput, HostCanonicalRepositorySourceInputError> {
    host_canonical_repository_source_input_with_filesystem_realization(route, generated_plan, None)
}

/// Project the original callable's complete workspace history. The reservation
/// binds the full pre-evaluation Spec; no authored-plan fallback is available.
pub fn host_canonical_repository_source_input_with_workspace_realization(
    route: Arc<HostCanonicalRepositoryRoute>,
    realization: crate::RepositoryWorkspaceRealization,
) -> Result<HostCanonicalRepositorySourceInput, HostCanonicalRepositorySourceInputError> {
    let view = route.view();
    let Some(seed) = view.generated_effect_seed() else {
        return Err(
            HostCanonicalRepositorySourceInputError::ExtraneousGeneratedWorkspaceRealization,
        );
    };
    let reservation = realization.reservation();
    let repo_spec = view
        .repo_spec()
        .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
    if seed.owner() != reservation.owner()
        || seed.ordinal() != reservation.ordinal()
        || view.workspace() != reservation.workspace()
        || view.canonical_repo() != reservation.repository()
        || repo_spec != reservation.expected_repo_spec().as_ref()
    {
        return Err(
            HostCanonicalRepositorySourceInputError::MismatchedGeneratedWorkspaceRealization,
        );
    }
    let request = RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: view.workspace().dupe(),
            canonical_repo: view.canonical_repo().clone(),
        },
        repo_spec: repo_spec.clone(),
        kind: RepositoryMaterializationKind::GeneralWorkspace(realization),
    };
    request.validate_general_workspace_binding().map_err(|_| {
        HostCanonicalRepositorySourceInputError::MismatchedGeneratedWorkspaceRealization
    })?;
    Ok(HostCanonicalRepositorySourceInput {
        route,
        disposition: HostRepositoryMaterializationDisposition::Request(Arc::new(request)),
    })
}

/// Project the natural generated producer's terminal archive binding. A plan
/// containing archives cannot fall back to recapturing unassociated effects.
pub fn host_canonical_repository_source_input_with_filesystem_realization(
    route: Arc<HostCanonicalRepositoryRoute>,
    generated_plan: Option<GeneratedRepositoryFileEffectPlan>,
    filesystem_realization: Option<RepositoryFilesystemRealization>,
) -> Result<HostCanonicalRepositorySourceInput, HostCanonicalRepositorySourceInputError> {
    let view = route.view();
    if filesystem_realization.is_some()
        && view.kind() != HostCanonicalRepositoryRouteKind::Generated
    {
        return Err(
            HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFilesystemRealization,
        );
    }
    let disposition = match view.kind() {
        HostCanonicalRepositoryRouteKind::Root => {
            return Err(HostCanonicalRepositorySourceInputError::Root);
        }
        HostCanonicalRepositoryRouteKind::Builtin => {
            if generated_plan.is_some() {
                return Err(
                    HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFileEffectPlan,
                );
            }
            HostRepositoryMaterializationDisposition::Builtin(
                view.builtin_identity()
                    .expect("a built-in canonical route retains its snapshot identity"),
            )
        }
        HostCanonicalRepositoryRouteKind::Generated => {
            let plan = generated_plan
                .ok_or(HostCanonicalRepositorySourceInputError::MissingGeneratedFileEffectPlan)?;
            let repo_spec = view
                .repo_spec()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            let kind = match filesystem_realization {
                Some(realization) => {
                    if !plan.has_filesystem_operations() {
                        return Err(HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFilesystemRealization);
                    }
                    let seed = view
                        .generated_effect_seed()
                        .expect("a generated canonical route retains its invocation seed");
                    if realization.terminal_plan() != &plan
                        || realization.owner() != seed.owner()
                        || realization.ordinal() != seed.ordinal()
                        || realization.transcript().records().iter().any(|record| {
                            record.request().generated_repository()
                                != view.canonical_repo().as_str()
                        })
                    {
                        return Err(HostCanonicalRepositorySourceInputError::MismatchedGeneratedFilesystemRealization);
                    }
                    RepositoryMaterializationKind::GeneratedFilesystemEffects(realization)
                }
                None if plan.has_filesystem_operations() => {
                    return Err(
                        HostCanonicalRepositorySourceInputError::MissingGeneratedFilesystemRealization,
                    );
                }
                None => RepositoryMaterializationKind::GeneratedFileEffects(plan),
            };
            HostRepositoryMaterializationDisposition::Request(Arc::new(
                RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: view.workspace().dupe(),
                        canonical_repo: view.canonical_repo().clone(),
                    },
                    repo_spec: repo_spec.clone(),
                    kind,
                },
            ))
        }
        HostCanonicalRepositoryRouteKind::SelectedRegistry
        | HostCanonicalRepositoryRouteKind::SelectedNonregistry => {
            if generated_plan.is_some() {
                return Err(
                    HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFileEffectPlan,
                );
            }
            let repo_spec = view
                .repo_spec()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            let local_path_policy = view
                .local_path_policy()
                .ok_or(HostCanonicalRepositorySourceInputError::MissingRepositorySpecification)?;
            let kind = request_kind(view.workspace(), repo_spec, local_path_policy)
                .map_err(HostCanonicalRepositorySourceInputError::Projection)?;
            HostRepositoryMaterializationDisposition::Request(Arc::new(
                RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: view.workspace().dupe(),
                        canonical_repo: view.canonical_repo().clone(),
                    },
                    repo_spec: repo_spec.clone(),
                    kind,
                },
            ))
        }
    };
    Ok(HostCanonicalRepositorySourceInput { route, disposition })
}

/// Complete repository source identity used by shared package-policy owners.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum HostRepositorySourceRoute {
    Root(RootRepositoryRoute),
    Canonical(HostCanonicalRepositorySourceInput),
}

/// The existing source child selected for an unobserved repository read.
///
/// Root requests retain their exact apparent-route owner. Built-in and
/// canonical routes use the shared zero-copy source-observation owner.
#[doc(hidden)]
pub enum HostRepositorySourceReadKey {
    RootRequest(HostRepositorySourceFileKey),
    Observation(HostRepositorySourceObservationKey),
}

/// The existing source child selected for an observed repository read.
#[doc(hidden)]
pub enum HostRepositorySourceReadObservationKey {
    RootRequest(HostRepositorySourceFileObservationKey),
    Observation(HostRepositorySourceObservationEpochKey),
}

impl HostRepositorySourceRoute {
    pub fn root(route: RootRepositoryRoute) -> Self {
        Self::Root(route)
    }

    pub fn canonical(input: HostCanonicalRepositorySourceInput) -> Self {
        Self::Canonical(input)
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        match self {
            Self::Root(route) => route.workspace(),
            Self::Canonical(input) => input.route.view().workspace(),
        }
    }

    pub fn canonical_repo(&self) -> &CanonicalRepoName {
        match self {
            Self::Root(route) => route.canonical_repo(),
            Self::Canonical(input) => input.route.view().canonical_repo(),
        }
    }

    /// A borrowed preflight classification, never filesystem authority. Native
    /// consumers must still compute and validate the current normal selection,
    /// materialization result, root and session before observing source bytes.
    #[doc(hidden)]
    pub fn is_immutable_materialization(&self) -> bool {
        match self {
            Self::Root(route) => match route.source() {
                crate::RootRepositorySource::Generated { .. } => true,
                crate::RootRepositorySource::SelectedRegistry(_) => {
                    is_immutable_repository_spec(route.repo_spec())
                }
                crate::RootRepositorySource::DirectLocal(_)
                | crate::RootRepositorySource::BuiltinBazelTools(_)
                | crate::RootRepositorySource::SelectedLocal(_) => false,
            },
            Self::Canonical(input) => match input.view().disposition() {
                HostRepositorySourceInputDispositionView::Request(request) => matches!(
                    request.kind,
                    RepositoryMaterializationKind::Immutable
                        | RepositoryMaterializationKind::GeneratedFileEffects(_)
                        | RepositoryMaterializationKind::GeneratedFilesystemEffects(_)
                        | RepositoryMaterializationKind::GeneralWorkspace(_)
                ),
                HostRepositorySourceInputDispositionView::Builtin(_) => false,
            },
        }
    }

    /// Stable compact-mapping cohort for repositories generated by one
    /// selected extension or innate repository-rule owner.
    #[doc(hidden)]
    pub fn runfiles_mapping_compact_group(&self) -> Option<CompactString> {
        match self {
            Self::Root(_) => None,
            Self::Canonical(input) => input
                .view()
                .route()
                .view()
                .generated_effect_seed()
                .map(|seed| CompactString::new(seed.owner().unique_name().as_str())),
        }
    }

    #[doc(hidden)]
    pub fn source_read_key(
        &self,
        relative_path: HostRepositoryRelativePath,
    ) -> HostRepositorySourceReadKey {
        match self {
            Self::Root(route) if !route.is_builtin_bazel_tools() => {
                HostRepositorySourceReadKey::RootRequest(HostRepositorySourceFileKey::new(
                    route.clone(),
                    relative_path.as_path().to_path_buf(),
                ))
            }
            Self::Root(_) | Self::Canonical(_) => {
                HostRepositorySourceReadKey::Observation(self.source_observation_key(relative_path))
            }
        }
    }

    #[doc(hidden)]
    pub fn source_read_observation_key(
        &self,
        relative_path: HostRepositoryRelativePath,
    ) -> HostRepositorySourceReadObservationKey {
        match self {
            Self::Root(route) if !route.is_builtin_bazel_tools() => {
                HostRepositorySourceReadObservationKey::RootRequest(
                    HostRepositorySourceFileObservationKey::new(
                        route.clone(),
                        relative_path.as_path().to_path_buf(),
                    ),
                )
            }
            Self::Root(_) | Self::Canonical(_) => {
                HostRepositorySourceReadObservationKey::Observation(
                    self.source_observation_epoch_key(relative_path),
                )
            }
        }
    }

    pub(crate) fn root_route(&self) -> Option<&RootRepositoryRoute> {
        match self {
            Self::Root(route) => Some(route),
            Self::Canonical(_) => None,
        }
    }

    pub(crate) fn is_builtin_bazel_tools(&self) -> bool {
        match self {
            Self::Root(route) => route.is_builtin_bazel_tools(),
            Self::Canonical(input) => matches!(
                &input.disposition,
                HostRepositoryMaterializationDisposition::Builtin(_)
            ),
        }
    }

    pub(crate) fn materialization_disposition(
        &self,
    ) -> Result<HostRepositoryMaterializationDisposition, RepositoryMaterializationError> {
        match self {
            Self::Root(route) => {
                host_repository_materialization_request(&route.source_capability())
            }
            Self::Canonical(input) => Ok(input.disposition.clone()),
        }
    }

    pub(crate) fn source_observation_key(
        &self,
        relative_path: HostRepositoryRelativePath,
    ) -> HostRepositorySourceObservationKey {
        match self {
            Self::Root(route) => HostRepositorySourceObservationKey::new(
                host_repository_source_input(route.source_capability())
                    .expect("a complete root route has a valid source input"),
                relative_path,
            ),
            Self::Canonical(input) => {
                HostRepositorySourceObservationKey::new_canonical(input.clone(), relative_path)
            }
        }
    }

    pub(crate) fn source_observation_epoch_key(
        &self,
        relative_path: HostRepositoryRelativePath,
    ) -> HostRepositorySourceObservationEpochKey {
        match self {
            Self::Root(route) => HostRepositorySourceObservationEpochKey::new(
                host_repository_source_input(route.source_capability())
                    .expect("a complete root route has a valid source input"),
                relative_path,
            ),
            Self::Canonical(input) => {
                HostRepositorySourceObservationEpochKey::new_canonical(input.clone(), relative_path)
            }
        }
    }
}

impl Hash for HostRepositorySourceRoute {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Root(route) => route.hash(state),
            Self::Canonical(input) => {
                1_u8.hash(state);
                input.hash(state);
            }
        }
    }
}

impl HostRepositorySourceObservationError {
    #[doc(hidden)]
    pub fn request_error(&self) -> Option<&RepositorySourceFileError> {
        match &self.kind {
            HostRepositorySourceObservationErrorKind::Request(error) => Some(error),
            HostRepositorySourceObservationErrorKind::BuiltinPath
            | HostRepositorySourceObservationErrorKind::Builtin(_)
            | HostRepositorySourceObservationErrorKind::BuiltinCompute(_)
            | HostRepositorySourceObservationErrorKind::RequestCompute(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GeneratedRepositoryFileEffectPlan;
    use crate::HostBuiltinBazelToolsRepositoryMapping;
    use crate::HostSelectedExtensionOwner;
    use crate::HostSelectedExtensionOwnerKind;
    use crate::RepoRuleId;

    fn generated_route(
        owner: Arc<HostSelectedExtensionOwner>,
        repo: &str,
    ) -> HostRepositorySourceRoute {
        let canonical_repo = CanonicalRepoName::new(repo).unwrap();
        let route = HostCanonicalRepositoryRoute::generated(
            NormalizedAbsolutePath::new("/runfiles-group").unwrap(),
            canonical_repo.clone(),
            owner,
            0,
            "generated",
            RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: slug_identity_v2::CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
                    rule_name: "repo".into(),
                },
                attributes: Arc::new(SmallMap::new()),
            },
            canonical_repo,
            SmallMap::new(),
        )
        .unwrap();
        HostRepositorySourceRoute::Canonical(
            host_canonical_repository_source_input(
                Arc::new(route),
                Some(GeneratedRepositoryFileEffectPlan::build([]).unwrap()),
            )
            .unwrap(),
        )
    }

    #[test]
    fn original_workspace_projection_requires_full_natural_binding_even_without_operations() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("+original"));
        let HostRepositorySourceRoute::Canonical(input) =
            generated_route(owner.clone(), "+original+repo")
        else {
            unreachable!()
        };
        let route = input.route;
        let spec = route.view().repo_spec().unwrap().clone();
        let realization = |ordinal, expected_spec| {
            let reservation = Arc::new(
                crate::RepositoryWorkspaceReservation::new(
                    route.view().workspace().clone(),
                    owner.clone(),
                    route.view().canonical_repo().clone(),
                    ordinal,
                    Arc::new(expected_spec),
                    NormalizedAbsolutePath::new("/__slug_repositories__/v1/original").unwrap(),
                    PathObservationInstanceId::new(71),
                    9,
                    crate::RepositoryWorkspaceRetentionLimits::new(
                        128,
                        16 * 1024 * 1024,
                        1024 * 1024 * 1024,
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            crate::RepositoryWorkspaceTranscript::new(reservation, 0o755)
                .unwrap()
                .realize(0)
                .unwrap()
        };
        let projected = host_canonical_repository_source_input_with_workspace_realization(
            route.clone(),
            realization(0, spec.clone()),
        )
        .unwrap();
        let HostRepositorySourceInputDispositionView::Request(request) =
            projected.view().disposition()
        else {
            unreachable!()
        };
        assert!(matches!(
            request.kind,
            RepositoryMaterializationKind::GeneralWorkspace(_)
        ));
        assert!(
            HostRepositorySourceRoute::Canonical(projected.clone()).is_immutable_materialization()
        );
        assert_eq!(
            host_canonical_repository_source_input_with_workspace_realization(
                route.clone(),
                realization(1, spec.clone())
            )
            .unwrap_err(),
            HostCanonicalRepositorySourceInputError::MismatchedGeneratedWorkspaceRealization
        );
        let mut changed = spec;
        let mut attributes = (*changed.attributes).clone();
        attributes.insert(
            "different".into(),
            OverrideAttributeValue::String("value".into()),
        );
        changed.attributes = Arc::new(attributes);
        assert_eq!(
            host_canonical_repository_source_input_with_workspace_realization(
                route.clone(),
                realization(0, changed.clone())
            )
            .unwrap_err(),
            HostCanonicalRepositorySourceInputError::MismatchedGeneratedWorkspaceRealization
        );
        let mut forged = request.as_ref().clone();
        forged.repo_spec = changed;
        assert!(
            RepositoryMaterializationResultEpoch::new(
                route.view().workspace().clone(),
                [RepositoryMaterializationEpochEntry {
                    request: Arc::new(forged),
                    result: RepositoryMaterializationResult::SpecError("foreign binding".into())
                }]
            )
            .is_err()
        );
    }

    #[test]
    fn generated_routes_project_owner_cohort_not_repository_identity() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("+extension"));
        let first = generated_route(owner.clone(), "+extension+first");
        let second = generated_route(owner, "+extension+second");
        let other = generated_route(
            Arc::new(HostSelectedExtensionOwner::testing("+other")),
            "+other+first",
        );

        assert_eq!(
            first.runfiles_mapping_compact_group().as_deref(),
            Some("+extension")
        );
        assert_eq!(
            first.runfiles_mapping_compact_group(),
            second.runfiles_mapping_compact_group()
        );
        assert_ne!(
            first.runfiles_mapping_compact_group(),
            other.runfiles_mapping_compact_group()
        );

        let innate_owner = Arc::new(HostSelectedExtensionOwner::testing_with_kind(
            "+repo",
            HostSelectedExtensionOwnerKind::InnateRepositoryRule,
        ));
        let innate_first = generated_route(innate_owner.clone(), "+repo+first");
        let innate_second = generated_route(innate_owner, "+repo+second");
        assert_eq!(
            innate_first.runfiles_mapping_compact_group().as_deref(),
            Some("+repo")
        );
        assert_eq!(
            innate_first.runfiles_mapping_compact_group(),
            innate_second.runfiles_mapping_compact_group()
        );
        assert_ne!(
            first.runfiles_mapping_compact_group(),
            innate_first.runfiles_mapping_compact_group()
        );
    }

    #[test]
    fn ordinary_canonical_routes_have_no_compact_group() {
        let route = HostCanonicalRepositoryRoute::builtin(
            NormalizedAbsolutePath::new("/ordinary-runfiles-group").unwrap(),
            HostBuiltinBazelToolsRepositoryMapping::testing(),
        );
        let input = host_canonical_repository_source_input(Arc::new(route), None).unwrap();
        assert!(
            HostRepositorySourceRoute::canonical(input)
                .runfiles_mapping_compact_group()
                .is_none()
        );
    }

    fn filesystem_realization(
        owner: Arc<HostSelectedExtensionOwner>,
        ordinal: usize,
        repo: &str,
        nonce: u64,
        suffix: &'static [u8],
    ) -> RepositoryFilesystemRealization {
        let archive = crate::GeneratedRepositoryArchiveEffect::new(
            Arc::from(["https://arbitrary.test/content.tar.xz".into()]),
            Some([7; 32]),
            "".into(),
            crate::GeneratedRepositoryArchiveFormat::TarXz,
            "".into(),
            "".into(),
        )
        .unwrap();
        let request = crate::RepositoryFilesystemRequest::new(
            crate::RepositoryFilesystemCallId::new(owner.clone(), ordinal, 0),
            repo.into(),
            crate::RepositoryFilesystemOperation::Archive(archive.clone()),
            GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        );
        let transcript = crate::RepositoryFilesystemTranscript::empty()
            .append(crate::RepositoryFilesystemRecord::new(
                Arc::new(request),
                crate::RepositoryFilesystemResult::Archive([7; 32]),
            ))
            .unwrap();
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        builder.push_archive_effect(archive).unwrap();
        builder
            .push("terminal.txt".into(), Arc::from(suffix), false)
            .unwrap();
        RepositoryFilesystemRealization::new(nonce, owner, ordinal, transcript, builder.finish())
            .unwrap()
    }

    #[test]
    fn delete_only_projection_requires_live_filesystem_realization() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("+extension"));
        let repo = "+extension+repo";
        let HostRepositorySourceRoute::Canonical(input) = generated_route(owner.clone(), repo)
        else {
            unreachable!()
        };
        let route = input.view().route().clone();
        let request = crate::RepositoryFilesystemRequest::new(
            crate::RepositoryFilesystemCallId::new(owner.clone(), 0, 0),
            repo.into(),
            crate::RepositoryFilesystemOperation::delete("absent".into()).unwrap(),
            GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        );
        let plan = request
            .prefix()
            .with_operation(request.operation())
            .unwrap();
        assert!(!plan.has_archives());
        let transcript = crate::RepositoryFilesystemTranscript::empty()
            .append(crate::RepositoryFilesystemRecord::new(
                Arc::new(request),
                crate::RepositoryFilesystemResult::Delete(false),
            ))
            .unwrap();
        let realization =
            RepositoryFilesystemRealization::new(17, owner, 0, transcript, plan.clone()).unwrap();
        assert!(matches!(
            host_canonical_repository_source_input(route.clone(), Some(plan.clone())),
            Err(HostCanonicalRepositorySourceInputError::MissingGeneratedFilesystemRealization)
        ));
        let projected = host_canonical_repository_source_input_with_filesystem_realization(
            route,
            Some(plan),
            Some(realization),
        )
        .unwrap();
        assert!(
            matches!(projected.view().disposition(), HostRepositorySourceInputDispositionView::Request(request)
            if matches!(request.kind, RepositoryMaterializationKind::GeneratedFilesystemEffects(_)))
        );
    }

    #[test]
    fn archive_projection_requires_exact_natural_invocation_and_terminal_plan() {
        let owner = Arc::new(HostSelectedExtensionOwner::testing("+extension"));
        let repo = "+extension+repo";
        let HostRepositorySourceRoute::Canonical(input) = generated_route(owner.clone(), repo)
        else {
            unreachable!()
        };
        let route = input.view().route().clone();
        let realization = filesystem_realization(owner.clone(), 0, repo, 1, b"terminal");
        let plan = realization.terminal_plan().clone();
        assert!(matches!(
            host_canonical_repository_source_input(route.clone(), Some(plan.clone())),
            Err(HostCanonicalRepositorySourceInputError::MissingGeneratedFilesystemRealization)
        ));
        let projected = host_canonical_repository_source_input_with_filesystem_realization(
            route.clone(),
            Some(plan.clone()),
            Some(realization.clone()),
        )
        .unwrap();
        assert!(
            matches!(projected.view().disposition(), HostRepositorySourceInputDispositionView::Request(request) if matches!(&request.kind, RepositoryMaterializationKind::GeneratedFilesystemEffects(actual) if actual == &realization))
        );
        assert!(
            HostRepositorySourceRoute::canonical(projected.clone()).is_immutable_materialization()
        );
        for mismatch in [
            filesystem_realization(
                Arc::new(HostSelectedExtensionOwner::testing("+foreign")),
                0,
                repo,
                1,
                b"terminal",
            ),
            filesystem_realization(owner.clone(), 1, repo, 1, b"terminal"),
            filesystem_realization(owner.clone(), 0, "+extension+other", 1, b"terminal"),
            filesystem_realization(owner.clone(), 0, repo, 1, b"different suffix"),
        ] {
            assert!(matches!(
                host_canonical_repository_source_input_with_filesystem_realization(
                    route.clone(),
                    Some(plan.clone()),
                    Some(mismatch)
                ),
                Err(HostCanonicalRepositorySourceInputError::MismatchedGeneratedFilesystemRealization)
            ));
        }
        let changed = host_canonical_repository_source_input_with_filesystem_realization(
            route.clone(),
            Some(plan.clone()),
            Some(filesystem_realization(owner, 0, repo, 2, b"terminal")),
        )
        .unwrap();
        assert_ne!(projected, changed);
        let digest = |input: &HostCanonicalRepositorySourceInput| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            input.hash(&mut hash);
            hash.finish()
        };
        assert_ne!(digest(&projected), digest(&changed));
        assert_eq!(
            host_canonical_repository_source_input_with_filesystem_realization(
                route,
                Some(plan),
                Some(realization.clone())
            )
            .unwrap(),
            projected
        );
        let builtin = Arc::new(HostCanonicalRepositoryRoute::builtin(
            NormalizedAbsolutePath::new("/builtin-archive-refusal").unwrap(),
            HostBuiltinBazelToolsRepositoryMapping::testing(),
        ));
        assert!(matches!(
            host_canonical_repository_source_input_with_filesystem_realization(
                builtin,
                None,
                Some(realization)
            ),
            Err(HostCanonicalRepositorySourceInputError::ExtraneousGeneratedFilesystemRealization)
        ));
    }

    #[test]
    fn immutable_rule_preflight_preserves_complete_label_identity() {
        use slug_identity_v2::ApparentLabel;
        use slug_identity_v2::RepositoryMapping;
        use slug_identity_v2::RepositoryMappingId;
        let workspace = NormalizedAbsolutePath::new("/immutable-preflight").unwrap();
        for (file, rule) in [("http.bzl", "http_archive"), ("git.bzl", "git_repository")] {
            let mut spec = RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(&format!(
                        "@@bazel_tools//tools/build_defs/repo:{file}"
                    ))
                    .unwrap(),
                    rule_name: rule.into(),
                },
                attributes: Arc::default(),
            };
            assert!(is_immutable_repository_spec(&spec));
            assert!(matches!(
                request_kind(
                    &workspace,
                    &spec,
                    HostRepositoryLocalPathPolicy::LocalUnsupported
                )
                .unwrap(),
                RepositoryMaterializationKind::Immutable
            ));
            let mapping =
                RepositoryMapping::new(RepositoryMappingId::new("different-provenance").unwrap());
            let mapped =
                ApparentLabel::parse(&format!("@bazel_tools//tools/build_defs/repo:{file}"))
                    .unwrap()
                    .resolve(&mapping);
            assert_eq!(mapped.to_string(), spec.rule_id.bzl_file.to_string());
            spec.rule_id.bzl_file = mapped;
            assert!(!is_immutable_repository_spec(&spec));
            assert!(
                request_kind(
                    &workspace,
                    &spec,
                    HostRepositoryLocalPathPolicy::LocalUnsupported
                )
                .is_err()
            );
        }
        let mut spec = RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:http.bzl")
                    .unwrap(),
                rule_name: "git_repository".into(),
            },
            attributes: Arc::default(),
        };
        assert!(!is_immutable_repository_spec(&spec));
        spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@other//tools/build_defs/repo:git.bzl").unwrap();
        assert!(!is_immutable_repository_spec(&spec));
        spec.rule_id.bzl_file = CanonicalLabel::parse("@@bazel_tools//wrong:git.bzl").unwrap();
        assert!(!is_immutable_repository_spec(&spec));
        spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:other.bzl").unwrap();
        assert!(!is_immutable_repository_spec(&spec));
        spec.rule_id.bzl_file =
            CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl").unwrap();
        spec.rule_id.rule_name = "local_repository".into();
        spec.attributes = Arc::new(SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::String("dep".into()),
        )]));
        assert!(!is_immutable_repository_spec(&spec));
        assert!(matches!(
            request_kind(
                &workspace,
                &spec,
                HostRepositoryLocalPathPolicy::WorkspaceRelative
            )
            .unwrap(),
            RepositoryMaterializationKind::Local { .. }
        ));
        assert!(
            request_kind(
                &workspace,
                &spec,
                HostRepositoryLocalPathPolicy::LocalUnsupported
            )
            .is_err()
        );
    }

    #[test]
    fn immutable_source_preflight_is_borrowed_and_not_authority() {
        let workspace = NormalizedAbsolutePath::new("/immutable-preflight").unwrap();
        let spec = RepoSpec {
            rule_id: RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@//:generated.bzl").unwrap(),
                rule_name: "generated".into(),
            },
            attributes: Arc::default(),
        };
        let local = crate::RootRepositoryRoute::for_test(
            workspace.clone(),
            ApparentRepoName::new("dep").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            spec.clone(),
        );
        assert!(!HostRepositorySourceRoute::Root(local).is_immutable_materialization());
        assert!(
            !HostRepositorySourceRoute::Root(crate::RootRepositoryRoute::builtin_for_test(
                workspace.clone()
            ))
            .is_immutable_materialization()
        );
        let generated = crate::RootRepositoryRoute::for_generated_repo_spec(
            workspace,
            ApparentRepoName::new("dep").unwrap(),
            CanonicalRepoName::new("dep+").unwrap(),
            spec,
            HostRepositoryLocalPathPolicy::LocalUnsupported,
            GeneratedRepositoryFileEffectPlan::build([]).unwrap(),
        )
        .unwrap();
        assert!(HostRepositorySourceRoute::Root(generated).is_immutable_materialization());
        let source = generated_route(
            Arc::new(HostSelectedExtensionOwner::testing("+extension")),
            "+extension+preflight",
        );
        assert!(source.is_immutable_materialization());
        // The classifier is deliberately driven by the retained disposition,
        // never an allocating request reprojection or a grant based on route kind.
        let HostRepositorySourceRoute::Canonical(mut input) = source else {
            unreachable!()
        };
        let HostRepositoryMaterializationDisposition::Request(request) = &input.disposition else {
            unreachable!()
        };
        let mut request = request.as_ref().clone();
        request.kind = RepositoryMaterializationKind::Immutable;
        input.disposition =
            HostRepositoryMaterializationDisposition::Request(Arc::new(request.clone()));
        assert!(HostRepositorySourceRoute::Canonical(input.clone()).is_immutable_materialization());
        request.kind = RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new("/refused-local").unwrap(),
        };
        input.disposition = HostRepositoryMaterializationDisposition::Request(Arc::new(request));
        assert!(!HostRepositorySourceRoute::Canonical(input).is_immutable_materialization());
        let builtin = HostCanonicalRepositoryRoute::builtin(
            NormalizedAbsolutePath::new("/builtin-preflight").unwrap(),
            HostBuiltinBazelToolsRepositoryMapping::testing(),
        );
        let builtin = host_canonical_repository_source_input(Arc::new(builtin), None).unwrap();
        assert!(!HostRepositorySourceRoute::Canonical(builtin).is_immutable_materialization());
    }
}
