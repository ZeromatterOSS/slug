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
                if let RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind {
                    plan.hash(state);
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
    let view = route.view();
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
            HostRepositoryMaterializationDisposition::Request(Arc::new(
                RepositoryMaterializationRequest {
                    id: RepositoryMaterializationRequestId {
                        workspace: view.workspace().dupe(),
                        canonical_repo: view.canonical_repo().clone(),
                    },
                    repo_spec: repo_spec.clone(),
                    kind: RepositoryMaterializationKind::GeneratedFileEffects(plan),
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
    pub(crate) fn request_error(&self) -> Option<&RepositorySourceFileError> {
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
}
