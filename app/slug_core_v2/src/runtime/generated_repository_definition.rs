/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionError;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionErrorDisposition;
use slug_bzlmod_v2::HostCanonicalSelectedModuleDefinitionKey;
use slug_bzlmod_v2::HostRepositoryLocalPathPolicy;
use slug_bzlmod_v2::HostRootRepositoryMapping;
use slug_bzlmod_v2::HostRootRepositoryMappingError;
use slug_bzlmod_v2::HostRootRepositoryMappingKey;
use slug_bzlmod_v2::RepoSpec;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_loading_v2::HostGeneratedRepositoryMapping;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecs;
use slug_loading_v2::HostValidatedGeneratedRepositorySpecsError;
use slug_loading_v2::HostValidatedModuleExtensionRepositoriesKey;
use slug_workspace_v2::NormalizedAbsolutePath;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinition {
    certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    ordinal: usize,
}

#[derive(Debug, Clone, Copy)]
struct HostGeneratedRepositoryDefinitionView<'a> {
    canonical_name: &'a CanonicalRepoName,
    internal_name: &'a str,
    repo_spec: &'a RepoSpec,
    mapping: HostGeneratedRepositoryMapping<'a>,
}

impl HostGeneratedRepositoryDefinition {
    fn view(&self) -> Option<HostGeneratedRepositoryDefinitionView<'_>> {
        self.certificate.iter().nth(self.ordinal).map(
            |(canonical_name, repo_spec, internal_name, mapping)| {
                HostGeneratedRepositoryDefinitionView {
                    canonical_name,
                    internal_name,
                    repo_spec,
                    mapping,
                }
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostGeneratedRepositoryDefinitionErrorKind {
    Loading(HostValidatedGeneratedRepositorySpecsError),
    LoadingCompute(Arc<str>),
    Missing {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
    },
    Duplicate {
        certificate: Arc<HostValidatedGeneratedRepositorySpecs>,
        first: usize,
        conflicting: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct HostGeneratedRepositoryDefinitionError {
    requested: CanonicalRepoName,
    kind: HostGeneratedRepositoryDefinitionErrorKind,
}

impl fmt::Display for HostGeneratedRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "generated repository '{}': {:?}",
            self.requested, self.kind
        )
    }
}

impl std::error::Error for HostGeneratedRepositoryDefinitionError {}

type HostGeneratedRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct HostGeneratedRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

impl HostGeneratedRepositoryDefinitionKey {
    fn new(workspace: NormalizedAbsolutePath, canonical_repo: CanonicalRepoName) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostGeneratedRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-generated-repository-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

fn complete(
    value: Result<HostGeneratedRepositoryDefinition, HostGeneratedRepositoryDefinitionError>,
) -> HostGeneratedRepositoryDefinitionOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UniqueOrdinalError {
    Missing,
    Duplicate { first: usize, conflicting: usize },
}

fn find_unique_ordinal<'a>(
    requested: &CanonicalRepoName,
    names: impl Iterator<Item = &'a CanonicalRepoName>,
) -> Result<usize, UniqueOrdinalError> {
    let mut first = None;
    let mut conflicting = None;
    for (ordinal, name) in names.enumerate() {
        if name != requested {
            continue;
        }
        if let Some(first) = first {
            conflicting.get_or_insert((first, ordinal));
        } else {
            first = Some(ordinal);
        }
    }
    match (first, conflicting) {
        (_, Some((first, conflicting))) => {
            Err(UniqueOrdinalError::Duplicate { first, conflicting })
        }
        (Some(first), None) => Ok(first),
        (None, None) => Err(UniqueOrdinalError::Missing),
    }
}

#[async_trait]
impl Key for HostGeneratedRepositoryDefinitionKey {
    type Value = HostGeneratedRepositoryDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let certificate = match ctx
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                self.workspace.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => Arc::new(value.clone()),
                Err(error) => {
                    return complete(Err(HostGeneratedRepositoryDefinitionError {
                        requested: self.canonical_repo.clone(),
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(error.clone()),
                    }));
                }
            },
            Err(error) => {
                return complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::LoadingCompute(
                        error.to_string().into(),
                    ),
                }));
            }
        };

        match find_unique_ordinal(
            &self.canonical_repo,
            certificate.iter().map(|(canonical, _, _, _)| canonical),
        ) {
            Ok(ordinal) => complete(Ok(HostGeneratedRepositoryDefinition {
                certificate,
                ordinal,
            })),
            Err(UniqueOrdinalError::Missing) => {
                complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { certificate },
                }))
            }
            Err(UniqueOrdinalError::Duplicate { first, conflicting }) => {
                complete(Err(HostGeneratedRepositoryDefinitionError {
                    requested: self.canonical_repo.clone(),
                    kind: HostGeneratedRepositoryDefinitionErrorKind::Duplicate {
                        certificate,
                        first,
                        conflicting,
                    },
                }))
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryDefinitionSource {
    Selected(HostCanonicalSelectedModuleDefinition),
    Generated(HostGeneratedRepositoryDefinition),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinition {
    source: HostCanonicalRepositoryDefinitionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HostCanonicalRepositoryDefinitionKind {
    Root,
    SelectedRegistry,
    SelectedNonregistry,
    Generated,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HostCanonicalRepositoryDefinitionView<'a> {
    kind: HostCanonicalRepositoryDefinitionKind,
    canonical_repo: &'a CanonicalRepoName,
    mapping_context: &'a CanonicalRepoName,
    repo_spec: Option<&'a RepoSpec>,
    local_path_policy: Option<HostRepositoryLocalPathPolicy>,
}

impl HostCanonicalRepositoryDefinition {
    pub(super) fn view(&self) -> Option<HostCanonicalRepositoryDefinitionView<'_>> {
        let (kind, canonical_repo, mapping_context, repo_spec, local_path_policy) =
            match &self.source {
                HostCanonicalRepositoryDefinitionSource::Selected(value) => {
                    let view = value.view();
                    let kind = match view.kind() {
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::Root => {
                            HostCanonicalRepositoryDefinitionKind::Root
                        }
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedRegistry => {
                            HostCanonicalRepositoryDefinitionKind::SelectedRegistry
                        }
                        slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedNonregistry => {
                            HostCanonicalRepositoryDefinitionKind::SelectedNonregistry
                        }
                    };
                    (
                        kind,
                        view.canonical_repo(),
                        view.mapping_context(),
                        view.repo_spec(),
                        view.local_path_policy(),
                    )
                }
                HostCanonicalRepositoryDefinitionSource::Generated(value) => {
                    let view = value.view()?;
                    (
                        HostCanonicalRepositoryDefinitionKind::Generated,
                        view.canonical_name,
                        view.mapping.context_repo(),
                        Some(view.repo_spec),
                        Some(HostRepositoryLocalPathPolicy::LocalUnsupported),
                    )
                }
            };
        Some(HostCanonicalRepositoryDefinitionView {
            kind,
            canonical_repo,
            mapping_context,
            repo_spec,
            local_path_policy,
        })
    }

    fn mapping_target(&self, apparent_repo: &ApparentRepoName) -> Option<&CanonicalRepoName> {
        match &self.source {
            HostCanonicalRepositoryDefinitionSource::Selected(value) => value
                .view()
                .mapping()
                .find_map(|(apparent, canonical)| (apparent == apparent_repo).then_some(canonical)),
            HostCanonicalRepositoryDefinitionSource::Generated(value) => {
                value.view()?.mapping.entries().get(apparent_repo)
            }
        }
    }
}

impl<'a> HostCanonicalRepositoryDefinitionView<'a> {
    pub(super) fn kind(self) -> HostCanonicalRepositoryDefinitionKind {
        self.kind
    }

    pub(super) fn canonical_repo(self) -> &'a CanonicalRepoName {
        self.canonical_repo
    }

    pub(super) fn mapping_context(self) -> &'a CanonicalRepoName {
        self.mapping_context
    }

    pub(super) fn repo_spec(self) -> Option<&'a RepoSpec> {
        self.repo_spec
    }

    pub(super) fn local_path_policy(self) -> Option<HostRepositoryLocalPathPolicy> {
        self.local_path_policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryDefinitionErrorKind {
    Selected(HostCanonicalSelectedModuleDefinitionError),
    SelectedCompute(Arc<str>),
    Generated {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        error: HostGeneratedRepositoryDefinitionError,
    },
    GeneratedCompute {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        message: Arc<str>,
    },
    Missing {
        selected_missing: HostCanonicalSelectedModuleDefinitionError,
        generated_missing: HostGeneratedRepositoryDefinitionError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinitionError {
    canonical_repo: CanonicalRepoName,
    kind: HostCanonicalRepositoryDefinitionErrorKind,
}

impl HostCanonicalRepositoryDefinitionError {
    pub(super) fn is_missing(&self) -> bool {
        matches!(
            self.kind,
            HostCanonicalRepositoryDefinitionErrorKind::Missing { .. }
        )
    }
}

impl fmt::Display for HostCanonicalRepositoryDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository '{}': {:?}",
            self.canonical_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryDefinitionError {}

pub(super) type HostCanonicalRepositoryDefinitionOutcome = SourcePreparationOutcome<
    Arc<Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>>,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinitionKey {
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
}

impl HostCanonicalRepositoryDefinitionKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        canonical_repo: CanonicalRepoName,
    ) -> Self {
        Self {
            workspace,
            canonical_repo,
        }
    }
}

impl fmt::Display for HostCanonicalRepositoryDefinitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-definition:{}:{}",
            self.workspace, self.canonical_repo
        )
    }
}

fn complete_canonical(
    value: Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>,
) -> HostCanonicalRepositoryDefinitionOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[async_trait]
impl Key for HostCanonicalRepositoryDefinitionKey {
    type Value = HostCanonicalRepositoryDefinitionOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let selected_missing = match ctx
            .compute(&HostCanonicalSelectedModuleDefinitionKey::new(
                self.workspace.clone(),
                self.canonical_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => {
                return SourcePreparationOutcome::Need(need);
            }
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => {
                    return complete_canonical(Ok(HostCanonicalRepositoryDefinition {
                        source: HostCanonicalRepositoryDefinitionSource::Selected(value.clone()),
                    }));
                }
                Err(error)
                    if error.disposition()
                        == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing =>
                {
                    error.clone()
                }
                Err(error) => {
                    return complete_canonical(Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo: self.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(error.clone()),
                    }));
                }
            },
            Err(error) => {
                return complete_canonical(Err(HostCanonicalRepositoryDefinitionError {
                    canonical_repo: self.canonical_repo.clone(),
                    kind: HostCanonicalRepositoryDefinitionErrorKind::SelectedCompute(
                        error.to_string().into(),
                    ),
                }));
            }
        };

        match ctx
            .compute(&HostGeneratedRepositoryDefinitionKey::new(
                self.workspace.clone(),
                self.canonical_repo.clone(),
            ))
            .await
        {
            Ok(SourcePreparationOutcome::Need(need)) => SourcePreparationOutcome::Need(need),
            Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                Ok(value) => complete_canonical(Ok(HostCanonicalRepositoryDefinition {
                    source: HostCanonicalRepositoryDefinitionSource::Generated(value.clone()),
                })),
                Err(error)
                    if matches!(
                        error.kind,
                        HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }
                    ) =>
                {
                    complete_canonical(Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo: self.canonical_repo.clone(),
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Missing {
                            selected_missing,
                            generated_missing: error.clone(),
                        },
                    }))
                }
                Err(error) => complete_canonical(Err(HostCanonicalRepositoryDefinitionError {
                    canonical_repo: self.canonical_repo.clone(),
                    kind: HostCanonicalRepositoryDefinitionErrorKind::Generated {
                        selected_missing,
                        error: error.clone(),
                    },
                })),
            },
            Err(error) => complete_canonical(Err(HostCanonicalRepositoryDefinitionError {
                canonical_repo: self.canonical_repo.clone(),
                kind: HostCanonicalRepositoryDefinitionErrorKind::GeneratedCompute {
                    selected_missing,
                    message: error.to_string().into(),
                },
            })),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMapping {
    predecessor: ApparentMappingPredecessor,
    apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMapping {
    pub(super) fn resolved_target(&self) -> Option<&CanonicalRepoName> {
        match &self.predecessor {
            ApparentMappingPredecessor::Root(predecessor) => predecessor
                .view()?
                .mapping()
                .find_map(|(name, target)| (name == &self.apparent_repo).then_some(target)),
            ApparentMappingPredecessor::Canonical(predecessor) => {
                predecessor.mapping_target(&self.apparent_repo)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum ApparentMappingPredecessor {
    Root(HostRootRepositoryMapping),
    Canonical(HostCanonicalRepositoryDefinition),
}

impl ApparentMappingPredecessor {
    fn contexts(&self) -> Option<(&CanonicalRepoName, &CanonicalRepoName)> {
        match self {
            Self::Root(predecessor) => predecessor
                .view()
                .map(|view| (view.canonical_repo(), view.mapping_context())),
            Self::Canonical(predecessor) => predecessor
                .view()
                .map(|view| (view.canonical_repo(), view.mapping_context())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum HostCanonicalRepositoryApparentMappingErrorKind {
    RootApparent,
    RootMapping(HostRootRepositoryMappingError),
    RootMappingCompute(Arc<str>),
    Definition(HostCanonicalRepositoryDefinitionError),
    DefinitionCompute(Arc<str>),
    ContextMismatch {
        predecessor: ApparentMappingPredecessor,
    },
    Missing {
        predecessor: ApparentMappingPredecessor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMappingError {
    context_repo: CanonicalRepoName,
    apparent_repo: ApparentRepoName,
    kind: HostCanonicalRepositoryApparentMappingErrorKind,
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical repository '{}' apparent mapping '{}': {:?}",
            self.context_repo, self.apparent_repo, self.kind
        )
    }
}

impl std::error::Error for HostCanonicalRepositoryApparentMappingError {}

pub(super) type HostCanonicalRepositoryApparentMappingOutcome = SourcePreparationOutcome<
    Arc<
        Result<HostCanonicalRepositoryApparentMapping, HostCanonicalRepositoryApparentMappingError>,
    >,
>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(super) struct HostCanonicalRepositoryApparentMappingKey {
    workspace: NormalizedAbsolutePath,
    context_repo: CanonicalRepoName,
    apparent_repo: ApparentRepoName,
}

impl HostCanonicalRepositoryApparentMappingKey {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        context_repo: CanonicalRepoName,
        apparent_repo: ApparentRepoName,
    ) -> Self {
        Self {
            workspace,
            context_repo,
            apparent_repo,
        }
    }
}

impl fmt::Display for HostCanonicalRepositoryApparentMappingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-canonical-repository-apparent-mapping:{}:{}:{}",
            self.workspace, self.context_repo, self.apparent_repo
        )
    }
}

fn complete_mapping(
    value: Result<
        HostCanonicalRepositoryApparentMapping,
        HostCanonicalRepositoryApparentMappingError,
    >,
) -> HostCanonicalRepositoryApparentMappingOutcome {
    SourcePreparationOutcome::Complete(Arc::new(value))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingLookupStatus {
    ContextMismatch,
    Missing,
    Found,
}

fn mapping_lookup_status(
    requested: &CanonicalRepoName,
    published: &CanonicalRepoName,
    mapping_context: &CanonicalRepoName,
    has_target: impl FnOnce() -> bool,
) -> MappingLookupStatus {
    if published != requested || mapping_context != requested {
        MappingLookupStatus::ContextMismatch
    } else if has_target() {
        MappingLookupStatus::Found
    } else {
        MappingLookupStatus::Missing
    }
}

#[async_trait]
impl Key for HostCanonicalRepositoryApparentMappingKey {
    type Value = HostCanonicalRepositoryApparentMappingOutcome;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let terminal = |kind| {
            complete_mapping(Err(HostCanonicalRepositoryApparentMappingError {
                context_repo: self.context_repo.clone(),
                apparent_repo: self.apparent_repo.clone(),
                kind,
            }))
        };
        if self.apparent_repo.is_root() && !self.context_repo.is_root() {
            return terminal(HostCanonicalRepositoryApparentMappingErrorKind::RootApparent);
        }

        let predecessor = if self.context_repo.is_root() {
            match ctx
                .compute(&HostRootRepositoryMappingKey::new(self.workspace.clone()))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                    Ok(value) => ApparentMappingPredecessor::Root(value.clone()),
                    Err(error) => {
                        return terminal(
                            HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(
                                error.clone(),
                            ),
                        );
                    }
                },
                Err(error) => {
                    return terminal(
                        HostCanonicalRepositoryApparentMappingErrorKind::RootMappingCompute(
                            error.to_string().into(),
                        ),
                    );
                }
            }
        } else {
            match ctx
                .compute(&HostCanonicalRepositoryDefinitionKey::new(
                    self.workspace.clone(),
                    self.context_repo.clone(),
                ))
                .await
            {
                Ok(SourcePreparationOutcome::Need(need)) => {
                    return SourcePreparationOutcome::Need(need);
                }
                Ok(SourcePreparationOutcome::Complete(value)) => match value.as_ref() {
                    Ok(value) => ApparentMappingPredecessor::Canonical(value.clone()),
                    Err(error) => {
                        return terminal(
                            HostCanonicalRepositoryApparentMappingErrorKind::Definition(
                                error.clone(),
                            ),
                        );
                    }
                },
                Err(error) => {
                    return terminal(
                        HostCanonicalRepositoryApparentMappingErrorKind::DefinitionCompute(
                            error.to_string().into(),
                        ),
                    );
                }
            }
        };

        let Some((canonical_repo, mapping_context)) = predecessor.contexts() else {
            return terminal(
                HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch { predecessor },
            );
        };
        match mapping_lookup_status(&self.context_repo, canonical_repo, mapping_context, || {
            match &predecessor {
                ApparentMappingPredecessor::Root(value) => value.view().is_some_and(|view| {
                    view.mapping().any(|(name, _)| name == &self.apparent_repo)
                }),
                ApparentMappingPredecessor::Canonical(value) => {
                    value.mapping_target(&self.apparent_repo).is_some()
                }
            }
        }) {
            MappingLookupStatus::ContextMismatch => {
                return terminal(
                    HostCanonicalRepositoryApparentMappingErrorKind::ContextMismatch {
                        predecessor,
                    },
                );
            }
            MappingLookupStatus::Missing => {
                return terminal(HostCanonicalRepositoryApparentMappingErrorKind::Missing {
                    predecessor,
                });
            }
            MappingLookupStatus::Found => {}
        }
        complete_mapping(Ok(HostCanonicalRepositoryApparentMapping {
            predecessor,
            apparent_repo: self.apparent_repo.clone(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
pub(super) mod tests {
    use std::cell::Cell;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::HostRepositorySourceFileKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::RegistryFileKey;
    use slug_bzlmod_v2::RegistryRequestGeneration;
    use slug_bzlmod_v2::RegistryUrls;
    use slug_bzlmod_v2::RepositoryMaterializationEpochEntry;
    use slug_bzlmod_v2::RepositoryMaterializationKey;
    use slug_bzlmod_v2::RepositoryMaterializationResult;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpoch;
    use slug_bzlmod_v2::RepositoryMaterializationResultEpochKey;
    use slug_bzlmod_v2::RepositoryMaterializationSuccess;
    use slug_bzlmod_v2::RepositoryPackageSourceKey;
    use slug_bzlmod_v2::RepositorySourceFileKey;
    use slug_bzlmod_v2::RootPackagePolicyInputs;
    use slug_bzlmod_v2::RootRepositoryRouteKey;
    use slug_loading_v2::HostValidatedGeneratedRepositorySpecsOutcome;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathNodeKind;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationEpoch;
    use slug_workspace_v2::PathObservationEpochKey;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use slug_workspace_v2::PathObservationResult;
    use slug_workspace_v2::PathOperationResult;
    use slug_workspace_v2::WorkspaceFileValue;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use starlark_map::sorted_map::SortedMap;

    use super::*;

    pub(in crate::runtime) const WORKSPACE: &str = "/generated-repository-definition";
    pub(in crate::runtime) const MODULE: &str = "module(name='bazel_tools')\ne=use_extension('//:ext.bzl','ext')\nuse_repo(e, first='first', second='second')\n";
    pub(in crate::runtime) const EXTENSION_A: &str = r#"
repo=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    repo(name='first', value='one', target=':local')
    repo(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;
    const EXTENSION_B: &str = r#"
other=repository_rule(implementation=lambda ctx: None, attrs={'value':attr.string(), 'target':attr.label()})
def impl(ctx):
    other(name='first', value='one', target=':local')
    other(name='second', value='two', target='@first//:item')
ext=module_extension(implementation=impl)
"#;

    #[derive(Default)]
    struct LookupTracker {
        canonical: Mutex<Vec<(ActivationKind, bool)>>,
        selected: Mutex<Vec<(ActivationKind, bool)>>,
        lookup: Mutex<Vec<(ActivationKind, bool)>>,
        apparent: Mutex<Vec<(ActivationKind, bool)>>,
        root_mapping: Mutex<Vec<(ActivationKind, bool)>>,
        forbidden: Mutex<Vec<&'static str>>,
    }

    impl ActivationTracker for LookupTracker {
        fn key_activated(
            &self,
            _: &DynKey,
            _: &mut dyn Iterator<Item = &DynKey>,
            _: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key
                .downcast_ref::<HostCanonicalRepositoryDefinitionKey>()
                .is_some()
            {
                self.canonical
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalSelectedModuleDefinitionKey>()
                .is_some()
            {
                self.selected
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostGeneratedRepositoryDefinitionKey>()
                .is_some()
            {
                self.lookup
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key
                .downcast_ref::<HostCanonicalRepositoryApparentMappingKey>()
                .is_some()
            {
                self.apparent
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<HostRootRepositoryMappingKey>().is_some() {
                self.root_mapping
                    .lock()
                    .unwrap()
                    .push((activation.kind(), activation.evaluation_data().is_some()));
            } else if key.downcast_ref::<RootRepositoryRouteKey>().is_some() {
                self.forbidden.lock().unwrap().push("root-route");
            } else if key.downcast_ref::<RegistryFileKey>().is_some() {
                self.forbidden.lock().unwrap().push("registry");
            } else if key.downcast_ref::<RepositoryMaterializationKey>().is_some() {
                self.forbidden.lock().unwrap().push("materialization");
            } else if key.downcast_ref::<RepositoryPackageSourceKey>().is_some()
                || key.downcast_ref::<RepositorySourceFileKey>().is_some()
                || key.downcast_ref::<HostRepositorySourceFileKey>().is_some()
            {
                self.forbidden.lock().unwrap().push("source");
            } else if key.downcast_ref::<PathObservationEpochKey>().is_some() {
                self.forbidden.lock().unwrap().push("filesystem");
            }
        }
    }

    pub(in crate::runtime) async fn transaction(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
    ) -> dice::DiceTransaction {
        transaction_with_policy(
            dice,
            module,
            extension,
            extension_present,
            tracker,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        )
        .await
    }

    pub(in crate::runtime) async fn transaction_with_command_override(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        module_name: &str,
    ) -> dice::DiceTransaction {
        let override_value = format!("{module_name}={WORKSPACE}/local");
        transaction_with_policy(
            dice,
            module,
            extension,
            true,
            None,
            BzlmodCommandPolicyKey::from_flags_with_module_overrides(
                None,
                false,
                NormalizedAbsolutePath::new(WORKSPACE).unwrap().as_path(),
                [override_value.as_str()],
            )
            .unwrap(),
        )
        .await
    }

    async fn transaction_with_policy(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        extension_present: bool,
        tracker: Option<Arc<dyn ActivationTracker>>,
        command_policy: BzlmodCommandPolicyKey,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            activation_tracker: tracker,
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(SortedMap::from_iter([
                        (
                            workspace.as_path().join("MODULE.bazel"),
                            WorkspaceFileValue::Present(Arc::new(module.to_owned())),
                        ),
                        (
                            workspace.as_path().join("ext.bzl"),
                            if extension_present {
                                WorkspaceFileValue::Present(Arc::new(extension.to_owned()))
                            } else {
                                WorkspaceFileValue::Absent
                            },
                        ),
                        (
                            workspace.as_path().join("BUILD.bazel"),
                            WorkspaceFileValue::Present(Arc::new(String::new())),
                        ),
                    ])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceRawSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceRawSnapshot {
                    files: Arc::new(SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel.lock"),
                        WorkspaceRawFileValue::Absent,
                    )])),
                }),
            )])
            .unwrap();
        slug_bzlmod_v2::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            command_policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        slug_bzlmod_v2::inject_registry_request_inputs(
            &mut updater,
            workspace.as_path(),
            RegistryUrls::new(["https://registry.invalid"]),
            RegistryRequestGeneration(1),
        )
        .unwrap();
        slug_bzlmod_v2::inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                workspace.dupe(),
                Arc::from([workspace.dupe()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let observations = ["/", WORKSPACE]
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::Directory,
                        index as i64 + 1,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                )
            })
            .chain(
                ["REPO.bazel", ".bazelignore", "BUILD", "MODULE.bazel.lock"]
                    .into_iter()
                    .map(|name| {
                        (
                            PathObservationDemand::new(
                                PathObservationNamespace::Host,
                                NormalizedAbsolutePath::new(format!("{WORKSPACE}/{name}")).unwrap(),
                                PathObservationOperation::Lstat,
                            ),
                            PathObservationResult::Lstat(PathOperationResult::Missing),
                        )
                    }),
            )
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/BUILD.bazel")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    10,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            )))
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::Lstat,
                ),
                PathObservationResult::Lstat(if extension_present {
                    PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        11,
                        1,
                        1,
                        1,
                        0o644,
                    ))
                } else {
                    PathOperationResult::Missing
                }),
            )))
            .chain([
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("{WORKSPACE}/local")).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::Directory,
                        12,
                        1,
                        1,
                        1,
                        0o755,
                    ))),
                ),
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("{WORKSPACE}/local/MODULE.bazel"))
                            .unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                        PathNodeKind::RegularFile,
                        13,
                        1,
                        1,
                        1,
                        0o644,
                    ))),
                ),
                (
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("{WORKSPACE}/local/MODULE.bazel"))
                            .unwrap(),
                        PathObservationOperation::FileBytes,
                    ),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        &b"module(name='local')\n"[..],
                    ))),
                ),
            ])
            .chain(std::iter::once((
                PathObservationDemand::new(
                    PathObservationNamespace::Host,
                    NormalizedAbsolutePath::new(format!("{WORKSPACE}/ext.bzl")).unwrap(),
                    PathObservationOperation::FileBytes,
                ),
                PathObservationResult::FileBytes(if extension_present {
                    PathOperationResult::Present(Arc::from(extension.as_bytes()))
                } else {
                    PathOperationResult::Missing
                }),
            )));
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(observations).unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    pub(in crate::runtime) async fn validated(
        transaction: &mut dice::DiceTransaction,
    ) -> HostValidatedGeneratedRepositorySpecsOutcome {
        transaction
            .compute(&HostValidatedModuleExtensionRepositoriesKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap()
    }

    pub(in crate::runtime) fn names(
        value: &HostValidatedGeneratedRepositorySpecsOutcome,
    ) -> Vec<CanonicalRepoName> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("validation must complete")
        };
        value
            .as_ref()
            .as_ref()
            .unwrap()
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect()
    }

    async fn lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        requested: Option<&CanonicalRepoName>,
    ) -> HostGeneratedRepositoryDefinitionOutcome {
        let mut tx = transaction(dice, module, extension, true, None).await;
        let mut generated = names(&validated(&mut tx).await);
        let name = requested.cloned().unwrap_or_else(|| generated.remove(0));
        tx.compute(&HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            name,
        ))
        .await
        .unwrap()
    }

    fn snapshot(value: &HostGeneratedRepositoryDefinitionOutcome) -> Vec<String> {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("lookup must complete")
        };
        let view = value.as_ref().as_ref().unwrap().view().unwrap();
        vec![
            view.canonical_name.as_str().to_owned(),
            view.internal_name.to_owned(),
            view.repo_spec.rule_id.rule_name.to_string(),
            format!("{:?}", view.repo_spec.attributes),
            view.mapping.context_repo().as_str().to_owned(),
            format!("{:?}", view.mapping.entries()),
        ]
    }

    fn mapping(
        value: &HostCanonicalRepositoryApparentMappingOutcome,
    ) -> &HostCanonicalRepositoryApparentMapping {
        let SourcePreparationOutcome::Complete(value) = value else {
            panic!("mapping must complete")
        };
        value.as_ref().as_ref().unwrap()
    }

    fn target(value: &HostCanonicalRepositoryApparentMappingOutcome) -> CanonicalRepoName {
        mapping(value).resolved_target().unwrap().clone()
    }

    async fn canonical_lookup(
        dice: &Arc<Dice>,
        module: &str,
        extension: &str,
        canonical_repo: CanonicalRepoName,
    ) -> HostCanonicalRepositoryDefinitionOutcome {
        transaction(dice, module, extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryDefinitionKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                canonical_repo,
            ))
            .await
            .unwrap()
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SyntheticDomainOutcome {
        Need,
        Success,
        Missing,
        Terminal,
    }

    fn synthetic_composition(
        selected: SyntheticDomainOutcome,
        generated: SyntheticDomainOutcome,
        generated_calls: &std::cell::Cell<usize>,
    ) -> SyntheticDomainOutcome {
        match selected {
            SyntheticDomainOutcome::Success
            | SyntheticDomainOutcome::Terminal
            | SyntheticDomainOutcome::Need => selected,
            SyntheticDomainOutcome::Missing => {
                generated_calls.set(generated_calls.get() + 1);
                generated
            }
        }
    }

    #[test]
    fn composition_branch_matrix_is_missing_only_and_selected_first() {
        use SyntheticDomainOutcome as O;

        for selected in [O::Success, O::Terminal, O::Need] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(selected, O::Success, &calls),
                selected
            );
            assert_eq!(calls.get(), 0, "same-canonical generated candidate ran");
        }
        for (generated, expected) in [
            (O::Success, O::Success),
            (O::Terminal, O::Terminal),
            (O::Missing, O::Missing),
            (O::Need, O::Need),
        ] {
            let calls = std::cell::Cell::new(0);
            assert_eq!(
                synthetic_composition(O::Missing, generated, &calls),
                expected
            );
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn complete_scan_rejects_missing_and_duplicate() {
        use std::cell::Cell;

        let requested = CanonicalRepoName::new("wanted").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        assert_eq!(
            find_unique_ordinal(&requested, [].iter()),
            Err(UniqueOrdinalError::Missing)
        );
        assert_eq!(
            find_unique_ordinal(&requested, [&other, &requested].into_iter()),
            Ok(1)
        );
        let consumed = Cell::new(0);
        let names = [&requested, &other, &requested, &other];
        assert_eq!(
            find_unique_ordinal(
                &requested,
                names
                    .into_iter()
                    .inspect(|_| consumed.set(consumed.get() + 1)),
            ),
            Err(UniqueOrdinalError::Duplicate {
                first: 0,
                conflicting: 2,
            })
        );
        assert_eq!(consumed.get(), names.len());
    }
    #[test]
    fn mapping_context_mismatch_precedes_target_lookup() {
        let requested = CanonicalRepoName::new("requested").unwrap();
        let other = CanonicalRepoName::new("other").unwrap();
        let target_checks = Cell::new(0);
        let has_target = || {
            target_checks.set(target_checks.get() + 1);
            false
        };
        assert_eq!(
            mapping_lookup_status(&requested, &other, &requested, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &other, has_target),
            MappingLookupStatus::ContextMismatch
        );
        assert_eq!(target_checks.get(), 0);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, has_target),
            MappingLookupStatus::Missing
        );
        assert_eq!(target_checks.get(), 1);
        assert_eq!(
            mapping_lookup_status(&requested, &requested, &requested, || true),
            MappingLookupStatus::Found
        );
    }
    #[tokio::test]
    async fn canonical_definition_selects_before_missing_only_generated_fallback() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        tracker.canonical.lock().unwrap().clear();
        tracker.selected.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let key = |canonical_repo| {
            HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), canonical_repo)
        };
        let root_key = key(CanonicalRepoName::root());
        let root = tx.compute(&root_key).await.unwrap();
        let SourcePreparationOutcome::Complete(root_value) = &root else {
            panic!("root definition must complete")
        };
        let root_definition = root_value.as_ref().as_ref().unwrap();
        let root_view = root_definition.view().unwrap();
        assert_eq!(
            root_view.kind(),
            HostCanonicalRepositoryDefinitionKind::Root
        );
        let HostCanonicalRepositoryDefinitionSource::Selected(root_certificate) =
            &root_definition.source
        else {
            panic!("root must be selected")
        };
        let root_view = root_certificate.view();
        assert_eq!(root_view.canonical_repo(), &CanonicalRepoName::root());
        assert!(root_view.repo_spec().is_none());
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let mut selected_error_tx = transaction(
            &dice,
            "module(name='root')\nbazel_dep(name='missing', version='1')\n",
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await;
        let selected_error = selected_error_tx
            .compute(&key(CanonicalRepoName::root()))
            .await
            .unwrap();
        assert!(matches!(
            selected_error,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Selected(error),
                        ..
                    }) if error.disposition()
                        == HostCanonicalSelectedModuleDefinitionErrorDisposition::Terminal
                )
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let selected_mapping_terminal = selected_error_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::new("missing+").unwrap(),
                ApparentRepoName::new("bazel_tools").unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(value) = selected_mapping_terminal else {
            panic!("selected terminal must complete")
        };
        assert!(matches!(
            value.as_ref(),
            Err(HostCanonicalRepositoryApparentMappingError {
                kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                ..
            })
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        tracker.forbidden.lock().unwrap().clear();
        let generated_key = key(generated[0].clone());
        let generated_value = tx.compute(&generated_key).await.unwrap();
        let SourcePreparationOutcome::Complete(value) = &generated_value else {
            panic!("generated definition must complete")
        };
        let generated_definition = value.as_ref().as_ref().unwrap();
        let generated_view = generated_definition.view().unwrap();
        assert_eq!(
            generated_view.kind(),
            HostCanonicalRepositoryDefinitionKind::Generated
        );
        let HostCanonicalRepositoryDefinitionSource::Generated(generated_certificate) =
            &generated_definition.source
        else {
            panic!("generated repository must use generated domain")
        };
        let generated_view = generated_certificate.view().unwrap();
        assert_eq!(generated_view.canonical_name, &generated[0]);
        assert_eq!(generated_view.internal_name, "first");
        assert_eq!(generated_view.repo_spec.rule_id.rule_name, "repo");
        assert_eq!(generated_view.mapping.context_repo(), &generated[0]);
        let generated_terminal = transaction(&dice, MODULE, EXTENSION_A, false, None)
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(
            &generated_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Generated {
                            selected_missing,
                            error,
                        },
                    }) if canonical_repo == &generated[0]
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            error.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Loading(_)
                        )
                )
        ));
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &generated_terminal,
        ));
        let missing_key = key(CanonicalRepoName::new("missing").unwrap());
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryDefinitionError {
                        canonical_repo,
                        kind: HostCanonicalRepositoryDefinitionErrorKind::Missing {
                            selected_missing,
                            generated_missing,
                        },
                    }) if canonical_repo.as_str() == "missing"
                        && selected_missing.disposition()
                            == HostCanonicalSelectedModuleDefinitionErrorDisposition::Missing
                        && matches!(
                            generated_missing.kind,
                            HostGeneratedRepositoryDefinitionErrorKind::Missing { .. }
                        )
                )
        ));
        let warm = tx.compute(&root_key).await.unwrap();
        assert!(HostCanonicalRepositoryDefinitionKey::equality(&root, &warm));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.lookup.lock().unwrap().len(), 2);
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let generated_b = canonical_lookup(&dice, MODULE, EXTENSION_B, generated[0].clone()).await;
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &generated_b,
        ));
        assert!(HostCanonicalRepositoryDefinitionKey::equality(
            &generated_value,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, generated[0].clone()).await,
        ));
        let selected_b = canonical_lookup(
            &dice,
            &MODULE.replace("name='bazel_tools'", "name='root', repo_name='changed'"),
            EXTENSION_A,
            CanonicalRepoName::root(),
        )
        .await;
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &root,
            &selected_b
        ));
        assert!(HostCanonicalRepositoryDefinitionKey::equality(
            &root,
            &canonical_lookup(&dice, MODULE, EXTENSION_A, CanonicalRepoName::root(),).await,
        ));
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let need = updater
            .commit()
            .await
            .compute(&generated_key)
            .await
            .unwrap();
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        assert!(!HostCanonicalRepositoryDefinitionKey::validity(&need));
        assert!(!HostCanonicalRepositoryDefinitionKey::equality(
            &need, &need
        ));
    }
    #[tokio::test]
    async fn canonical_definition_borrows_nonregistry_selected_view() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let local_module = "module(name='bazel_tools')\nlocal_path_override(module_name='local', path='local')\nbazel_dep(name='local', version='1', repo_name='local_alias')\n";
        let local_key = HostCanonicalRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
        );
        let local_mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::new("local+").unwrap(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let local_need = transaction(
            &dice,
            local_module,
            EXTENSION_A,
            true,
            Some(tracker.clone()),
        )
        .await
        .compute(&local_mapping_key)
        .await
        .unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &local_need
        ));
        assert!(tracker.lookup.lock().unwrap().is_empty());
        let SourcePreparationOutcome::Need(need) = local_need else {
            panic!("local definition must first request materialization")
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .clone();
        let mut updater = dice.updater_with_data(UserComputationData {
            activation_tracker: Some(tracker.clone()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        let mut local_tx = updater.commit().await;
        let local = local_tx.compute(&local_key).await.unwrap();
        let SourcePreparationOutcome::Complete(local) = &local else {
            panic!("local definition must complete: {local:?}")
        };
        let HostCanonicalRepositoryDefinitionSource::Selected(local) =
            &local.as_ref().as_ref().unwrap().source
        else {
            panic!("local definition must be selected")
        };
        let local_view = local.view();
        assert_eq!(
            local_view.kind(),
            slug_bzlmod_v2::HostCanonicalSelectedModuleKind::SelectedNonregistry
        );
        assert_eq!(local_view.canonical_repo().as_str(), "local+");
        assert_eq!(local_view.mapping_context().as_str(), "local+");
        assert_eq!(
            local_view.repo_spec().unwrap().rule_id.rule_name,
            "local_repository"
        );
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let mapping_value = local_tx.compute(&local_mapping_key).await.unwrap();
        let selected_target = local_view
            .mapping()
            .find_map(|(apparent, canonical)| {
                (apparent.as_str() == "bazel_tools").then_some(canonical)
            })
            .unwrap();
        assert_eq!(target(&mapping_value), selected_target.clone());
        let borrowed_target = mapping(&mapping_value).resolved_target().unwrap();
        assert!(std::ptr::eq(selected_target, borrowed_target));
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        tracker.canonical.lock().unwrap().clear();
        tracker.root_mapping.lock().unwrap().clear();
        let root_local = local_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("local_alias").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(target(&root_local).as_str(), "local+");
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
    }

    #[tokio::test]
    async fn real_lookup_borrows_exact_definition_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let validation = validated(&mut tx).await;
        let generated = names(&validation);
        assert_eq!(generated.len(), 2);
        tracker.forbidden.lock().unwrap().clear();

        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let first_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        let second_key =
            HostGeneratedRepositoryDefinitionKey::new(workspace.clone(), generated[1].clone());
        let first = tx.compute(&first_key).await.unwrap();
        let second = tx.compute(&second_key).await.unwrap();
        let warm = tx.compute(&first_key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&first));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first, &warm
        ));

        let SourcePreparationOutcome::Complete(first_value) = &first else {
            panic!("lookup must complete")
        };
        let SourcePreparationOutcome::Complete(second_value) = &second else {
            panic!("lookup must complete")
        };
        let first_view = first_value.as_ref().as_ref().unwrap().view().unwrap();
        let second_view = second_value.as_ref().as_ref().unwrap().view().unwrap();
        assert_eq!(first_view.canonical_name, &generated[0]);
        assert_eq!(first_view.internal_name, "first");
        assert_eq!(first_view.repo_spec.rule_id.rule_name.as_str(), "repo");
        assert_eq!(second_view.internal_name, "second");
        assert!(matches!(
            second_view.repo_spec.attributes.get("value"),
            Some(slug_bzlmod_v2::OverrideAttributeValue::String(value)) if value == "two"
        ));
        assert!(std::ptr::eq(
            first_view.mapping.entries(),
            second_view.mapping.entries()
        ));
        assert_eq!(first_view.mapping.context_repo(), &generated[0]);
        assert_eq!(second_view.mapping.context_repo(), &generated[1]);

        let missing_key = HostGeneratedRepositoryDefinitionKey::new(
            workspace.clone(),
            CanonicalRepoName::new("missing").unwrap(),
        );
        let missing = tx.compute(&missing_key).await.unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Missing { .. },
                        ..
                    })
                )
        ));

        let baseline = snapshot(&first);
        for (case, (module, extension, changed_fields)) in [
            (MODULE.to_owned(), EXTENSION_B.to_owned(), &[2][..]),
            (MODULE.replace("first='first'", "first='renamed'"), EXTENSION_A.replacen("name='first'", "name='renamed'", 1), &[0, 1, 4, 5]),
            (MODULE.to_owned(), EXTENSION_A.replace("value", "renamed_value"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one'", "value='changed'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("value='one', target=':local'", "target=':local', value='one'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("target=':local'", "target=':changed'"), &[3][..]),
            (MODULE.to_owned(), EXTENSION_A.replace("repo(name='first', value='one', target=':local')\n    repo(name='second', value='two', target='@first//:item')", "repo(name='second', value='two', target='@first//:item')\n    repo(name='first', value='one', target=':local')"), &[0, 1, 3, 4]),
        ]
        .into_iter()
        .enumerate()
        {
            let b = lookup(&dice, &module, &extension, None).await;
            assert!(!HostGeneratedRepositoryDefinitionKey::equality(&first, &b));
            let changed = snapshot(&b);
            assert!(changed_fields.iter().all(|index| baseline[*index] != changed[*index]), "case {case}: {baseline:?} == {changed:?}");
            let a2 = lookup(&dice, MODULE, EXTENSION_A, None).await;
            assert!(HostGeneratedRepositoryDefinitionKey::equality(&first, &a2));
        }

        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let mapping_a = lookup(&dice, &inject_a, EXTENSION_A, None).await;
        let mapping_b = lookup(&dice, &inject_b, EXTENSION_A, None).await;
        assert_ne!(snapshot(&mapping_a)[5], snapshot(&mapping_b)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &mapping_a,
            &lookup(&dice, &inject_a, EXTENSION_A, None).await,
        ));
        let overridden = lookup(
            &dice,
            &format!("{MODULE}override_repo(e, first='bazel_tools')\n"),
            EXTENSION_A,
            None,
        )
        .await;
        assert_ne!(baseline[5], snapshot(&overridden)[5]);
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &first,
            &lookup(&dice, MODULE, EXTENSION_A, None).await,
        ));

        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let request_a = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\n";
        let request_b = "module(name='bazel_tools')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, second='second')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first='first')\n";
        let order_a = lookup(&dice, request_a, &multi_extension, None).await;
        let fixed = CanonicalRepoName::new(&snapshot(&order_a)[0]).unwrap();
        let order_b = lookup(&dice, request_b, &multi_extension, Some(&fixed)).await;
        assert_eq!(&snapshot(&order_a)[..2], &snapshot(&order_b)[..2]);
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &order_a, &order_b
        ));
        assert!(HostGeneratedRepositoryDefinitionKey::equality(
            &order_a,
            &lookup(&dice, request_a, &multi_extension, Some(&fixed)).await,
        ));
        assert_eq!(
            *tracker.lookup.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Evaluated, false),
            ]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn real_apparent_mapping_borrows_effective_target_and_restores() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(LookupTracker::default());
        let mut tx = transaction(&dice, MODULE, EXTENSION_A, true, Some(tracker.clone())).await;
        let generated = names(&validated(&mut tx).await);
        let workspace = NormalizedAbsolutePath::new(WORKSPACE).unwrap();
        let definition_key =
            HostCanonicalRepositoryDefinitionKey::new(workspace.clone(), generated[0].clone());
        tx.compute(&definition_key).await.unwrap();
        tracker.canonical.lock().unwrap().clear();
        tracker.lookup.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let key = |context: CanonicalRepoName, apparent: &str| {
            HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            )
        };
        let self_key = key(generated[0].clone(), "first");
        let sibling_key = key(generated[0].clone(), "second");
        let host_key = key(generated[0].clone(), "bazel_tools");
        let self_mapping = tx.compute(&self_key).await.unwrap();
        let sibling_mapping = tx.compute(&sibling_key).await.unwrap();
        let host_mapping = tx.compute(&host_key).await.unwrap();
        let warm = tx.compute(&self_key).await.unwrap();
        assert_eq!(target(&self_mapping), generated[0]);
        assert_eq!(target(&sibling_mapping), generated[1]);
        assert_eq!(target(&host_mapping), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &sibling_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(
            *tracker.canonical.lock().unwrap(),
            [
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert!(tracker.lookup.lock().unwrap().is_empty());
        assert!(tracker.root_mapping.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let missing = tx
            .compute(&key(generated[0].clone(), "missing"))
            .await
            .unwrap();
        assert!(matches!(
            missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing { .. },
                        ..
                    })
                )
        ));
        let predecessor_activations = tracker.canonical.lock().unwrap().len();
        let root_apparent = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                generated[0].clone(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        let root_context = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        let root_self = tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                workspace.clone(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert_eq!(
            tracker.canonical.lock().unwrap().len(),
            predecessor_activations
        );
        assert!(matches!(
            root_apparent,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootApparent,
                        ..
                    })
                )
        ));
        assert_eq!(target(&root_context), generated[0]);
        assert_eq!(target(&root_self), CanonicalRepoName::root());
        let ApparentMappingPredecessor::Root(root_predecessor) =
            &mapping(&root_context).predecessor
        else {
            panic!("root lookup must retain root mapping predecessor")
        };
        let published_target = root_predecessor
            .view()
            .unwrap()
            .mapping()
            .find_map(|(name, target)| (name.as_str() == "first").then_some(target))
            .unwrap();
        assert!(std::ptr::eq(
            published_target,
            mapping(&root_context).resolved_target().unwrap(),
        ));
        tx.compute(&HostRootRepositoryMappingKey::new(workspace.clone()))
            .await
            .unwrap();
        tracker.root_mapping.lock().unwrap().clear();
        tracker.canonical.lock().unwrap().clear();
        tracker.apparent.lock().unwrap().clear();
        tracker.forbidden.lock().unwrap().clear();
        let root_builtin_key = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            CanonicalRepoName::root(),
            ApparentRepoName::new("bazel_tools").unwrap(),
        );
        let root_builtin = tx.compute(&root_builtin_key).await.unwrap();
        assert_eq!(target(&root_builtin), CanonicalRepoName::root());
        assert_eq!(
            *tracker.root_mapping.lock().unwrap(),
            [(ActivationKind::Reused, false)]
        );
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [(ActivationKind::Evaluated, false)]
        );
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        let root_builtin_warm = tx.compute(&root_builtin_key).await.unwrap();
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_builtin,
            &root_builtin_warm,
        ));
        assert_eq!(
            *tracker.apparent.lock().unwrap(),
            [
                (ActivationKind::Evaluated, false),
                (ActivationKind::Reused, false),
            ]
        );
        assert_eq!(tracker.root_mapping.lock().unwrap().len(), 1);
        assert!(tracker.canonical.lock().unwrap().is_empty());
        assert!(tracker.forbidden.lock().unwrap().is_empty());
        async fn resolve(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            let mut tx = transaction(dice, module, EXTENSION_A, true, None).await;
            let context = names(&validated(&mut tx).await).remove(0);
            tx.compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                context,
                ApparentRepoName::new(apparent).unwrap(),
            ))
            .await
            .unwrap()
        }
        async fn resolve_root(
            dice: &Arc<Dice>,
            module: &str,
            apparent: &str,
        ) -> HostCanonicalRepositoryApparentMappingOutcome {
            transaction(dice, module, EXTENSION_A, true, None)
                .await
                .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                    NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                    CanonicalRepoName::root(),
                    ApparentRepoName::new(apparent).unwrap(),
                ))
                .await
                .unwrap()
        }
        let override_module = format!("{MODULE}override_repo(e, first='bazel_tools')\n");
        let overridden = resolve(&dice, &override_module, "first").await;
        assert_eq!(target(&overridden), CanonicalRepoName::root());
        let SourcePreparationOutcome::Complete(overridden_value) = &overridden else {
            panic!("override mapping must complete")
        };
        let ApparentMappingPredecessor::Canonical(overridden_predecessor) =
            &overridden_value.as_ref().as_ref().unwrap().predecessor
        else {
            panic!("overridden mapping must retain canonical predecessor")
        };
        let HostCanonicalRepositoryDefinitionSource::Generated(overridden_certificate) =
            &overridden_predecessor.source
        else {
            panic!("overridden generated definition must stay generated")
        };
        let overridden_view = overridden_certificate.view().unwrap();
        assert_eq!(overridden_view.repo_spec.rule_id.rule_name.as_str(), "repo");
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &overridden,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &self_mapping,
            &resolve(&dice, MODULE, "first").await,
        ));
        let root_a = resolve_root(&dice, MODULE, "first").await;
        let root_b = resolve_root(&dice, &override_module, "first").await;
        assert_eq!(target(&root_a), generated[0]);
        assert_eq!(target(&root_b), CanonicalRepoName::root());
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a, &root_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let order_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','ext')\n\
            use_repo(e, second='second', first='first')\n";
        let root_order = resolve_root(&dice, order_module, "first").await;
        assert_eq!(target(&root_order), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_order,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let named_root_module = MODULE.replacen(
            "module(name='bazel_tools')",
            "module(name='bazel_tools', repo_name='root_self')",
            1,
        );
        let named_root = resolve_root(&dice, &named_root_module, "first").await;
        assert_eq!(target(&named_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &named_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let alternate_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "ext=module_extension(implementation=impl)\nother=module_extension(implementation=impl)",
        );
        let alternate_module = "module(name='bazel_tools')\n\
            e=use_extension('//:ext.bzl','other')\n\
            use_repo(e, first='first', second='second')\n";
        let alternate_root = transaction(&dice, alternate_module, &alternate_extension, true, None)
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert_ne!(target(&alternate_root), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &alternate_root,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let mut invalid_tx = transaction(
            &dice,
            "this is not valid Starlark\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let direct_error = invalid_tx
            .compute(&HostRootRepositoryMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Complete(direct_error) = direct_error else {
            panic!("invalid root mapping must complete")
        };
        let direct_error = direct_error.as_ref().as_ref().unwrap_err().clone();
        let root_terminal = invalid_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::new("first").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &root_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        context_repo,
                        apparent_repo,
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::RootMapping(error),
                        ..
                    }) if context_repo.is_root()
                        && apparent_repo.as_str() == "first"
                        && error == &direct_error
                )
        ));
        let root_injected = resolve_root(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "first",
        )
        .await;
        assert_eq!(target(&root_injected), generated[0]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &root_injected,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &self_mapping,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &root_a,
            &resolve_root(&dice, MODULE, "first").await,
        ));
        let root_missing = resolve_root(&dice, MODULE, "missing").await;
        assert!(matches!(
            root_missing,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Missing {
                            predecessor: ApparentMappingPredecessor::Root(_),
                        },
                        ..
                    })
                )
        ));
        let injected = resolve(
            &dice,
            &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
            "injected",
        )
        .await;
        assert_eq!(target(&injected), CanonicalRepoName::root());
        let injected_context = {
            let SourcePreparationOutcome::Complete(value) = &injected else {
                panic!("injected mapping must complete")
            };
            let ApparentMappingPredecessor::Canonical(predecessor) =
                &value.as_ref().as_ref().unwrap().predecessor
            else {
                panic!("injected mapping must retain canonical predecessor")
            };
            predecessor.view().unwrap().canonical_repo().clone()
        };
        let invalid_override_module = format!("{MODULE}override_repo(e, injected='bazel_tools')\n");
        let mut invalid_override_tx =
            transaction(&dice, &invalid_override_module, EXTENSION_A, true, None).await;
        let invalid_override = invalid_override_tx
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                injected_context,
                ApparentRepoName::new("injected").unwrap(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            &invalid_override,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                        ..
                    })
                )
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &invalid_override,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &injected,
            &resolve(
                &dice,
                &format!("{MODULE}inject_repo(e, injected='bazel_tools')\n"),
                "injected",
            )
            .await,
        ));
        let inject_a = format!(
            "{MODULE}inject_repo(e, injected='bazel_tools')\ninject_repo(e, other='bazel_tools')\n"
        );
        let inject_b = format!(
            "{MODULE}inject_repo(e, other='bazel_tools')\ninject_repo(e, injected='bazel_tools')\n"
        );
        let order_a = resolve(&dice, &inject_a, "injected").await;
        let order_b = resolve(&dice, &inject_b, "injected").await;
        assert_eq!(target(&order_a), target(&order_b));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a, &order_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &order_a,
            &resolve(&dice, &inject_a, "injected").await,
        ));
        let multi_extension = EXTENSION_A.replace(
            "ext=module_extension(implementation=impl)",
            "first=module_extension(implementation=impl)\nsecond=module_extension(implementation=impl)",
        );
        let multi_module = "module(name='bazel_tools')\na=use_extension('//:ext.bzl','first')\nuse_repo(a, first_a='first')\nb=use_extension('//:ext.bzl','second')\nuse_repo(b, first_b='first')\n";
        let mut multi_tx = transaction(&dice, multi_module, &multi_extension, true, None).await;
        let contexts = names(&validated(&mut multi_tx).await);
        let first_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace.clone(),
            contexts[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let second_context = HostCanonicalRepositoryApparentMappingKey::new(
            workspace,
            contexts[2].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let isolated_a = multi_tx.compute(&first_context).await.unwrap();
        let isolated_b = multi_tx.compute(&second_context).await.unwrap();
        assert_eq!(target(&isolated_a), contexts[0]);
        assert_eq!(target(&isolated_b), contexts[2]);
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &isolated_b,
        ));
        assert!(HostCanonicalRepositoryApparentMappingKey::equality(
            &isolated_a,
            &multi_tx.compute(&first_context).await.unwrap(),
        ));
    }

    #[tokio::test]
    async fn predecessor_need_and_error_precede_lookup() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut initial = transaction(&dice, MODULE, EXTENSION_A, true, None).await;
        let generated = names(&validated(&mut initial).await);
        let key = HostGeneratedRepositoryDefinitionKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
        );
        let mut updater = dice.updater_with_data(UserComputationData {
            cycle_detector: Some(slug_loading_v2::bzl_load_cycle_detector()),
            ..Default::default()
        });
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let mut need_tx = updater.commit().await;
        let need = need_tx.compute(&key).await.unwrap();
        assert!(!HostGeneratedRepositoryDefinitionKey::validity(&need));
        assert!(!HostGeneratedRepositoryDefinitionKey::equality(
            &need, &need
        ));
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let mapping_key = HostCanonicalRepositoryApparentMappingKey::new(
            NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
            generated[0].clone(),
            ApparentRepoName::new("first").unwrap(),
        );
        let mapping_need = need_tx.compute(&mapping_key).await.unwrap();
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &mapping_need
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &mapping_need,
            &mapping_need,
        ));
        let root_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let _root_tx = transaction(
            &root_dice,
            "module(name='bazel_tools')\n\
             local_path_override(module_name='local', path='local')\n\
             bazel_dep(name='local', version='1', repo_name='local_alias')\n",
            EXTENSION_A,
            true,
            None,
        )
        .await;
        let mut root_updater = root_dice.updater();
        root_updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new([]).unwrap(),
            )])
            .unwrap();
        let root_mapping_need = root_updater
            .commit()
            .await
            .compute(&HostCanonicalRepositoryApparentMappingKey::new(
                NormalizedAbsolutePath::new(WORKSPACE).unwrap(),
                CanonicalRepoName::root(),
                ApparentRepoName::root(),
            ))
            .await
            .unwrap();
        assert!(matches!(
            root_mapping_need,
            SourcePreparationOutcome::Need(_)
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::validity(
            &root_mapping_need,
        ));
        assert!(!HostCanonicalRepositoryApparentMappingKey::equality(
            &root_mapping_need,
            &root_mapping_need,
        ));

        let mut missing_source = transaction(&dice, MODULE, EXTENSION_A, false, None).await;
        let terminal = missing_source.compute(&key).await.unwrap();
        assert!(HostGeneratedRepositoryDefinitionKey::validity(&terminal));
        assert!(matches!(
            terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostGeneratedRepositoryDefinitionError {
                        kind: HostGeneratedRepositoryDefinitionErrorKind::Loading(_),
                        ..
                    })
                )
        ));
        let mapping_terminal = missing_source.compute(&mapping_key).await.unwrap();
        assert!(matches!(
            mapping_terminal,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostCanonicalRepositoryApparentMappingError {
                        kind: HostCanonicalRepositoryApparentMappingErrorKind::Definition(_),
                        ..
                    })
                )
        ));
    }
}
