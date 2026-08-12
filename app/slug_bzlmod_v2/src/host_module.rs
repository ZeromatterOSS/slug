/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory or the Apache License, Version 2.0
 * found in the LICENSE-APACHE file in the root directory. You may select,
 * at your option, one of the above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the Host root-module activation packet.

use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EventBatch;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_workspace_v2::NeedPathObservations;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOutcome;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BuiltinBazelToolsRouteIdentity;
use crate::BuiltinBazelToolsSnapshot;
use crate::EvaluatedRootModule;
use crate::LogicalModuleFileId;
use crate::LogicalSpan;
use crate::NonrootIncludeRequest;
use crate::OverrideAttributeKey;
use crate::OverrideAttributeValue;
use crate::RepoSpec;
use crate::RootModuleBootstrapRequest;
use crate::RootModuleOverride;
use crate::RootModuleOverrides;
use crate::RootModuleRegistrations;
use crate::SourcePreparationNeeds;
use crate::SourcePreparationOutcome;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileError;
use crate::host_include::HostRootIncludeError;
use crate::host_include::preflight_root_include_horizon;
use crate::module_eval::RootModuleSourceFile;
use crate::module_eval::evaluate_root_module_closure_with_events;
use crate::module_eval::root_module_ignore_dev_dependency;
use crate::module_eval::validate_root_module_source;

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRootModuleFileValue {
    pub(crate) module: EvaluatedRootModule,
    pub(crate) overrides: RootModuleOverrides,
    pub(crate) module_file_paths: Arc<[PathBuf]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRootModuleFileError {
    CommandPolicy {
        message: CompactString,
    },
    RootFile {
        error: HostFileError,
    },
    RootValidation {
        logical_id: LogicalModuleFileId,
        message: CompactString,
    },
    IncludePreflight {
        error: HostRootIncludeError,
    },
    IncludeMissing {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
    },
    IncludeFile {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
        error: HostFileError,
    },
    IncludeValidation {
        raw_label: CompactString,
        location: LogicalSpan,
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    Evaluation {
        message: CompactString,
        include_occurrences: Arc<[NonrootIncludeRequest]>,
    },
}

impl fmt::Display for HostRootModuleFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for HostRootModuleFileError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRootModuleFileKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRootModuleFileKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRootModuleFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-root-module-file:{}", self.workspace)
    }
}

type HostRootModuleFileCarrier = Arc<Result<HostRootModuleFileValue, HostRootModuleFileError>>;
type HostRootModuleFileOutcome = SourcePreparationOutcome<HostRootModuleFileCarrier>;

fn path_need(need: NeedPathObservations) -> HostRootModuleFileOutcome {
    SourcePreparationOutcome::Need(SourcePreparationNeeds::path(need))
}

fn terminal_error(error: HostRootModuleFileError) -> HostRootModuleFileOutcome {
    SourcePreparationOutcome::Complete(Arc::new(Err(error)))
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host root-module DICE invariant failed: {error:?}"))
}

fn root_logical_id(path: &NormalizedAbsolutePath) -> LogicalModuleFileId {
    LogicalModuleFileId::new(path.as_path().display().to_string())
}

fn include_relative_path(
    package: &slug_identity_v2::PackagePath,
    target: &slug_identity_v2::TargetName,
) -> PathBuf {
    PathBuf::from(package.as_str()).join(target.as_str())
}

#[async_trait]
impl Key for HostRootModuleFileKey {
    type Value = HostRootModuleFileOutcome;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let outcome = async {
            let ignore_dev_dependency =
                match root_module_ignore_dev_dependency(ctx, self.workspace.as_path()).await {
                    Ok(value) => value,
                    Err(message) => {
                        return terminal_error(HostRootModuleFileError::CommandPolicy { message });
                    }
                };

            let root_path =
                NormalizedAbsolutePath::new(self.workspace.as_path().join("MODULE.bazel"))
                    .expect("joining the root MODULE basename remains normalized absolute");
            let root_bytes =
                match dice_invariant(ctx.compute(&HostFileBytesKey::new(root_path.dupe())).await) {
                    PathOutcome::Need(need) => return path_need(need),
                    PathOutcome::Complete(Err(error)) => {
                        return terminal_error(HostRootModuleFileError::RootFile { error });
                    }
                    PathOutcome::Complete(Ok(HostFileBytes::Missing)) => {
                        return SourcePreparationOutcome::Need(
                            SourcePreparationNeeds::root_module_bootstrap(
                                RootModuleBootstrapRequest {
                                    workspace: self.workspace.dupe(),
                                },
                            ),
                        );
                    }
                    PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => bytes,
                };
            let root_id = root_logical_id(&root_path);
            let root_inspection =
                match validate_root_module_source(root_id.clone(), root_bytes.as_ref()) {
                    Ok(inspection) => inspection,
                    Err(message) => {
                        return terminal_error(HostRootModuleFileError::RootValidation {
                            logical_id: root_id,
                            message,
                        });
                    }
                };
            let root_source = Arc::new(
                std::str::from_utf8(root_bytes.as_ref())
                    .expect("successful validation established UTF-8")
                    .to_owned(),
            );
            let mut horizon = root_inspection.includes.to_vec();
            let mut files = vec![RootModuleSourceFile {
                path: root_path.as_path().to_path_buf(),
                source: root_source,
                _inspection: root_inspection,
            }];
            let mut include_indices = SmallMap::new();
            let mut module_file_paths = vec![PathBuf::from("MODULE.bazel")];
            let mut evaluation_occurrences = Vec::new();

            while !horizon.is_empty() {
                let preflight =
                    preflight_root_include_horizon(ctx, &self.workspace, &horizon).await;
                let preflight = match preflight {
                    PathOutcome::Need(need) => return path_need(need),
                    PathOutcome::Complete(value) => match value.as_ref() {
                        Ok(value) => value.clone(),
                        Err(error) => {
                            return terminal_error(HostRootModuleFileError::IncludePreflight {
                                error: error.clone(),
                            });
                        }
                    },
                };

                let mut unique_paths = SmallSet::with_capacity(preflight.includes().len());
                for include in preflight.includes() {
                    unique_paths.insert(include.logical_path().dupe());
                }
                let computed = ctx
                    .compute_join(unique_paths, |ctx, logical_path| {
                        Box::pin(async move {
                            let outcome = dice_invariant(
                                ctx.compute(&HostFileBytesKey::new(logical_path.dupe()))
                                    .await,
                            );
                            (logical_path, outcome)
                        })
                    })
                    .await;
                let outcomes = computed.into_iter().collect::<SmallMap<_, _>>();
                let all_need: Option<NeedPathObservations> =
                    outcomes
                        .values()
                        .fold(None, |current, outcome| match outcome {
                            PathOutcome::Need(incoming) => Some(match current {
                                Some(current) => current.union(incoming),
                                None => incoming.dupe(),
                            }),
                            PathOutcome::Complete(_) => current,
                        });

                let mut next_horizon = Vec::new();
                for include in preflight.includes() {
                    let request = include.include();
                    let logical_path = include.logical_path();
                    let bytes = match outcomes
                        .get(logical_path)
                        .expect("every selected logical include path was computed")
                    {
                        PathOutcome::Need(_) => {
                            return path_need(
                                all_need.expect("the current occurrence contributed a Need"),
                            );
                        }
                        PathOutcome::Complete(Ok(HostFileBytes::Missing)) => {
                            return terminal_error(HostRootModuleFileError::IncludeMissing {
                                raw_label: CompactString::new(request.raw_label()),
                                location: request.location().clone(),
                                logical_path: logical_path.dupe(),
                            });
                        }
                        PathOutcome::Complete(Err(error)) => {
                            return terminal_error(HostRootModuleFileError::IncludeFile {
                                raw_label: CompactString::new(request.raw_label()),
                                location: request.location().clone(),
                                logical_path: logical_path.dupe(),
                                error: error.clone(),
                            });
                        }
                        PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => bytes.dupe(),
                    };
                    let relative_path =
                        include_relative_path(request.package().package(), request.target());
                    let logical_id =
                        LogicalModuleFileId::new(logical_path.as_path().display().to_string());
                    let inspection = match validate_root_module_source(logical_id, bytes.as_ref()) {
                        Ok(inspection) => inspection,
                        Err(message) => {
                            return terminal_error(HostRootModuleFileError::IncludeValidation {
                                raw_label: CompactString::new(request.raw_label()),
                                location: request.location().clone(),
                                logical_path: logical_path.dupe(),
                                message,
                            });
                        }
                    };
                    let source = Arc::new(
                        std::str::from_utf8(bytes.as_ref())
                            .expect("successful validation established UTF-8")
                            .to_owned(),
                    );
                    next_horizon.extend(inspection.includes.iter().cloned());
                    let index = files.len();
                    include_indices.insert(CompactString::new(request.raw_label()), index);
                    module_file_paths.push(relative_path.clone());
                    evaluation_occurrences.push(NonrootIncludeRequest {
                        path: CompactString::new(request.raw_label()),
                        location: request.location().clone(),
                    });
                    files.push(RootModuleSourceFile {
                        path: logical_path.as_path().to_path_buf(),
                        source,
                        _inspection: inspection,
                    });
                }
                horizon = next_horizon;
            }

            module_file_paths.sort();
            module_file_paths.dedup();
            let (evaluation, captured) = evaluate_root_module_closure_with_events(
                ignore_dev_dependency,
                files,
                include_indices,
                module_file_paths.into(),
                capture_events,
            );
            event_batch = captured;
            match evaluation {
                Ok(evaluation) => {
                    SourcePreparationOutcome::Complete(Arc::new(Ok(HostRootModuleFileValue {
                        module: evaluation.module,
                        overrides: evaluation.overrides,
                        module_file_paths: evaluation.module_file_paths,
                    })))
                }
                Err(message) => terminal_error(HostRootModuleFileError::Evaluation {
                    message,
                    include_occurrences: evaluation_occurrences.into(),
                }),
            }
        }
        .await;
        if capture_events && outcome.is_complete() {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("Host root-module key stores exactly one event batch");
        }
        outcome
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLoadingAnchor {
    carrier: HostRootModuleFileCarrier,
}

impl fmt::Debug for RootModuleLoadingAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootModuleLoadingAnchor")
    }
}

impl RootModuleLoadingAnchor {
    pub fn registrations(&self) -> &RootModuleRegistrations {
        &self
            .carrier
            .as_ref()
            .as_ref()
            .expect("root module loading anchor retains a successful carrier")
            .module
            .registrations
    }
}

#[derive(Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLoadingAnchorError {
    carrier: HostRootModuleFileCarrier,
}

impl fmt::Debug for RootModuleLoadingAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootModuleLoadingAnchorError")
    }
}

impl fmt::Display for RootModuleLoadingAnchorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.carrier.as_ref() {
            Err(error) => fmt::Display::fmt(error, f),
            Ok(_) => unreachable!("root-loading anchor error retains an error carrier"),
        }
    }
}

impl std::error::Error for RootModuleLoadingAnchorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.carrier.as_ref() {
            Err(error) => std::error::Error::source(error),
            Ok(_) => unreachable!("root-loading anchor error retains an error carrier"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub struct RootModuleLoadingAnchorKey {
    workspace: NormalizedAbsolutePath,
}

impl RootModuleLoadingAnchorKey {
    pub fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for RootModuleLoadingAnchorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-loading-anchor:{}", self.workspace)
    }
}

#[async_trait]
impl Key for RootModuleLoadingAnchorKey {
    type Value = SourcePreparationOutcome<
        Arc<Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>>,
    >;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        dice_invariant(
            ctx.compute(&HostRootModuleFileKey::new(self.workspace.dupe()))
                .await,
        )
        .map(|carrier| {
            Arc::new(if carrier.is_ok() {
                Ok(RootModuleLoadingAnchor { carrier })
            } else {
                Err(RootModuleLoadingAnchorError { carrier })
            })
        })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RootRepositorySource {
    DirectLocal(RepoSpec),
    BuiltinBazelTools(BuiltinBazelToolsRouteIdentity),
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RootRepositoryRoute {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
    module_name: CompactString,
    canonical_repo: CanonicalRepoName,
    source: RootRepositorySource,
}

fn hash_override_attribute_value<H: Hasher>(value: &OverrideAttributeValue, state: &mut H) {
    std::mem::discriminant(value).hash(state);
    match value {
        OverrideAttributeValue::None => {}
        OverrideAttributeValue::Bool(value) => value.hash(state),
        OverrideAttributeValue::Int(value) => value.hash(state),
        OverrideAttributeValue::String(value) => value.hash(state),
        OverrideAttributeValue::Label(value) => value.hash(state),
        OverrideAttributeValue::Iterable(values) => {
            values.len().hash(state);
            for value in values.iter() {
                hash_override_attribute_value(value, state);
            }
        }
        OverrideAttributeValue::Map(values) => {
            hash_override_attribute_map(values, state);
        }
    }
}

fn hash_override_attribute_map<H: Hasher>(
    values: &SmallMap<OverrideAttributeKey, OverrideAttributeValue>,
    state: &mut H,
) {
    let mut entry_hashes = values
        .iter()
        .map(|(key, value)| {
            let mut entry = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut entry);
            hash_override_attribute_value(value, &mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    entry_hashes.sort_unstable();
    entry_hashes.hash(state);
}

fn hash_repo_spec<H: Hasher>(spec: &RepoSpec, state: &mut H) {
    spec.rule_id.bzl_file.hash(state);
    spec.rule_id.rule_name.hash(state);
    let mut entry_hashes = spec
        .attributes
        .iter()
        .map(|(key, value)| {
            let mut entry = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut entry);
            hash_override_attribute_value(value, &mut entry);
            entry.finish()
        })
        .collect::<Vec<_>>();
    entry_hashes.sort_unstable();
    entry_hashes.hash(state);
}

impl Hash for RootRepositorySource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::DirectLocal(spec) => hash_repo_spec(spec, state),
            Self::BuiltinBazelTools(identity) => identity.hash(state),
        }
    }
}

impl Hash for RootRepositoryRoute {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace.hash(state);
        self.apparent_repo.hash(state);
        self.module_name.hash(state);
        self.canonical_repo.hash(state);
        self.source.hash(state);
    }
}

impl fmt::Debug for RootRepositoryRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootRepositoryRoute")
            .field("apparent_repo", &self.apparent_repo)
            .field("canonical_repo", &self.canonical_repo)
            .finish_non_exhaustive()
    }
}

impl RootRepositoryRoute {
    pub fn apparent_repo(&self) -> &ApparentRepoName {
        &self.apparent_repo
    }

    pub fn canonical_repo(&self) -> &CanonicalRepoName {
        &self.canonical_repo
    }

    pub fn module_name(&self) -> &str {
        self.module_name.as_str()
    }

    pub fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }

    pub fn source(&self) -> &RootRepositorySource {
        &self.source
    }

    pub fn is_builtin_bazel_tools(&self) -> bool {
        matches!(self.source, RootRepositorySource::BuiltinBazelTools(_))
    }

    pub(crate) fn repo_spec(&self) -> &RepoSpec {
        match &self.source {
            RootRepositorySource::DirectLocal(spec) => spec,
            RootRepositorySource::BuiltinBazelTools(_) => {
                panic!("built-in bazel_tools has no RepoSpec")
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
        module_name: CompactString,
        canonical_repo: CanonicalRepoName,
        repo_spec: RepoSpec,
    ) -> Self {
        Self {
            workspace,
            apparent_repo,
            module_name,
            canonical_repo,
            source: RootRepositorySource::DirectLocal(repo_spec),
        }
    }

    #[cfg(test)]
    pub(crate) fn builtin_for_test(workspace: NormalizedAbsolutePath) -> Self {
        Self {
            workspace,
            apparent_repo: ApparentRepoName::new("bazel_tools")
                .expect("built-in apparent name is valid"),
            module_name: CompactString::new("bazel_tools"),
            canonical_repo: CanonicalRepoName::new("bazel_tools")
                .expect("built-in canonical name is valid"),
            source: RootRepositorySource::BuiltinBazelTools(
                BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RootRepositoryRouteErrorKind {
    Root(HostRootModuleFileError),
    Unknown {
        apparent_repo: ApparentRepoName,
    },
    Unsupported {
        apparent_repo: ApparentRepoName,
        module_name: CompactString,
    },
}

#[derive(Clone, PartialEq, Eq, Allocative)]
pub struct RootRepositoryRouteError {
    kind: RootRepositoryRouteErrorKind,
}

impl fmt::Debug for RootRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RootRepositoryRouteError")
    }
}

impl fmt::Display for RootRepositoryRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RootRepositoryRouteErrorKind::Root(error) => error.fmt(f),
            RootRepositoryRouteErrorKind::Unknown { apparent_repo } => write!(
                f,
                "no such package '@@[unknown repo '{}' requested from @@]//': The repository '@@[unknown repo '{}' requested from @@]' could not be resolved: No repository visible as '@{}' from main repository",
                apparent_repo.as_str(),
                apparent_repo.as_str(),
                apparent_repo.as_str(),
            ),
            RootRepositoryRouteErrorKind::Unsupported {
                apparent_repo,
                module_name,
            } => write!(
                f,
                "external repository '@{}' from module '{}' is not a direct local_path_override",
                apparent_repo.as_str(),
                module_name,
            ),
        }
    }
}

impl std::error::Error for RootRepositoryRouteError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootRepositoryRouteKey {
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
}

impl RootRepositoryRouteKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        apparent_repo: ApparentRepoName,
    ) -> Result<Self, String> {
        if apparent_repo.is_root() {
            return Err("external repository route requires a nonroot apparent name".to_owned());
        }
        Ok(Self {
            workspace,
            apparent_repo,
        })
    }
}

impl fmt::Display for RootRepositoryRouteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-repository-route:{}:@{}",
            self.workspace,
            self.apparent_repo.as_str()
        )
    }
}

fn is_local_path_override(spec: &RepoSpec) -> bool {
    let local_bzl = CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
        .expect("pinned local repository label is canonical");
    spec.rule_id.bzl_file == local_bzl && spec.rule_id.rule_name == "local_repository"
}

#[async_trait]
impl Key for RootRepositoryRouteKey {
    type Value =
        SourcePreparationOutcome<Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let carrier = match dice_invariant(
            ctx.compute(&HostRootModuleFileKey::new(self.workspace.dupe()))
                .await,
        ) {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(carrier) => carrier,
        };
        let value = match carrier.as_ref() {
            Err(error) => Err(RootRepositoryRouteError {
                kind: RootRepositoryRouteErrorKind::Root(error.clone()),
            }),
            Ok(root) => {
                if self.apparent_repo.as_str() == "bazel_tools" {
                    return SourcePreparationOutcome::Complete(Arc::new(Ok(RootRepositoryRoute {
                        workspace: self.workspace.dupe(),
                        apparent_repo: self.apparent_repo.clone(),
                        module_name: CompactString::new("bazel_tools"),
                        canonical_repo: CanonicalRepoName::new("bazel_tools")
                            .expect("built-in repository name is canonical"),
                        source: RootRepositorySource::BuiltinBazelTools(
                            BuiltinBazelToolsSnapshot::CURRENT.route_identity(),
                        ),
                    })));
                }
                let dependency = root.module.dependencies.iter().find(|dependency| {
                    !dependency.nodep
                        && dependency
                            .repo_name
                            .as_deref()
                            .unwrap_or(dependency.name.as_str())
                            == self.apparent_repo.as_str()
                });
                match dependency {
                    None => Err(RootRepositoryRouteError {
                        kind: RootRepositoryRouteErrorKind::Unknown {
                            apparent_repo: self.apparent_repo.clone(),
                        },
                    }),
                    Some(dependency) => {
                        let repo_spec = match root.overrides.get(dependency.name.as_str()) {
                            Some(RootModuleOverride::NonRegistry(spec))
                                if is_local_path_override(spec) =>
                            {
                                spec.clone()
                            }
                            _ => {
                                return SourcePreparationOutcome::Complete(Arc::new(Err(
                                    RootRepositoryRouteError {
                                        kind: RootRepositoryRouteErrorKind::Unsupported {
                                            apparent_repo: self.apparent_repo.clone(),
                                            module_name: dependency.name.clone(),
                                        },
                                    },
                                )));
                            }
                        };
                        let canonical_repo =
                            CanonicalRepoName::new(format!("{}+", dependency.name))
                                .expect("validated module name forms a canonical repository");
                        Ok(RootRepositoryRoute {
                            workspace: self.workspace.dupe(),
                            apparent_repo: self.apparent_repo.clone(),
                            module_name: dependency.name.clone(),
                            canonical_repo,
                            source: RootRepositorySource::DirectLocal(repo_spec),
                        })
                    }
                }
            }
        };
        SourcePreparationOutcome::Complete(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::error::Error as _;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    use dice::ActivationData;
    use dice::ActivationKind;
    use dice::ActivationTracker;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DynKey;
    use dice::Key;
    use dice::RichActivation;
    use dice::UserComputationData;
    use dupe::Dupe;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_identity_v2::ApparentRepoName;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::NormalizedAbsolutePath;
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
    use slug_workspace_v2::WorkspaceSnapshot;
    use slug_workspace_v2::WorkspaceSnapshotKey;
    use starlark_map::small_map::SmallMap;
    use starlark_map::sorted_map::SortedMap;

    use super::HostRootModuleFileError;
    use super::HostRootModuleFileKey;
    use super::HostRootModuleFileValue;
    use super::RootModuleLoadingAnchor;
    use super::RootModuleLoadingAnchorError;
    use super::RootModuleLoadingAnchorKey;
    use super::RootRepositoryRoute;
    use super::RootRepositoryRouteKey;
    use super::RootRepositorySource;
    use crate::BzlmodCommandPolicyKey;
    use crate::BzlmodEnvironmentPolicyKey;
    use crate::EvaluatedRootModule;
    use crate::LockfileMode;
    use crate::RootModuleOverrides;
    use crate::RootPackagePolicyInputs;
    use crate::SourcePreparationNeeds;
    use crate::SourcePreparationOutcome;
    use crate::host_file::HostFileBytesKey;
    use crate::inject_root_module_request_inputs;
    use crate::inject_root_package_policy_inputs;
    use crate::module_eval::clear_validated_root_module_logical_ids;
    use crate::module_eval::take_validated_root_module_logical_ids;
    use crate::repo_file::HostRepoFileKey;

    fn workspace() -> NormalizedAbsolutePath {
        NormalizedAbsolutePath::new("/workspace").unwrap()
    }

    fn empty_value() -> HostRootModuleFileValue {
        HostRootModuleFileValue {
            module: EvaluatedRootModule {
                header: None,
                dependencies: [].into(),
                registrations: Default::default(),
            },
            overrides: RootModuleOverrides::default(),
            module_file_paths: ["MODULE.bazel".into()].into(),
        }
    }

    #[derive(Default)]
    struct EpochBuilder {
        entries: SmallMap<PathObservationDemand, PathObservationResult>,
    }

    impl EpochBuilder {
        fn demand(path: &str, operation: PathObservationOperation) -> PathObservationDemand {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        }

        fn directory(&mut self, path: &str, variant: i64) {
            self.node(path, PathNodeKind::Directory, variant);
        }

        fn node(&mut self, path: &str, kind: PathNodeKind, variant: i64) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    kind, variant, variant, variant, variant, 0o755,
                ))),
            );
        }

        fn missing(&mut self, path: &str) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            );
        }

        fn file(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
            self.node(path, PathNodeKind::RegularFile, variant);
            self.file_kind(path, source, variant, PathNodeKind::RegularFile);
        }

        fn special(&mut self, path: &str, source: impl AsRef<[u8]>, variant: i64) {
            self.node(path, PathNodeKind::SpecialFile, variant);
            self.file_kind(path, source, variant, PathNodeKind::SpecialFile);
        }

        fn file_kind(
            &mut self,
            path: &str,
            source: impl AsRef<[u8]>,
            _variant: i64,
            _kind: PathNodeKind,
        ) {
            self.entries.insert(
                Self::demand(path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                    source.as_ref(),
                ))),
            );
        }

        fn root(source: impl AsRef<[u8]>, variant: i64) -> Self {
            let mut builder = Self::default();
            builder.directory("/", variant);
            builder.directory("/workspace", variant);
            builder.file("/workspace/MODULE.bazel", source, variant);
            builder
        }

        fn repository_policy(&mut self, roots: &[&str], variant: i64) {
            self.missing("/workspace/REPO.bazel");
            for root in roots {
                self.directory(root, variant);
                self.missing(&format!("{root}/.bazelignore"));
            }
        }

        fn package(&mut self, root: &str, package: &str, variant: i64) {
            let mut current = PathBuf::from(root);
            for component in package.split('/') {
                current.push(component);
                self.directory(current.to_str().unwrap(), variant);
            }
            self.node(
                &format!("{root}/{package}/BUILD.bazel"),
                PathNodeKind::RegularFile,
                variant,
            );
        }

        fn build(self) -> PathObservationEpoch {
            PathObservationEpoch::new(self.entries).unwrap()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TrackedBatch {
        key: String,
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[derive(Default)]
    struct EventTracker {
        entries: Mutex<Vec<TrackedBatch>>,
        anchor_dependencies: Mutex<Vec<Vec<String>>>,
    }

    impl EventTracker {
        fn take(&self) -> Vec<TrackedBatch> {
            std::mem::take(&mut *self.entries.lock().unwrap())
        }

        fn take_anchor_dependencies(&self) -> Vec<Vec<String>> {
            std::mem::take(&mut *self.anchor_dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for EventTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<RootModuleLoadingAnchorKey>().is_some() {
                self.anchor_dependencies
                    .lock()
                    .unwrap()
                    .push(deps.map(ToString::to_string).collect());
            }
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<HostRootModuleFileKey>().is_none()
                && key.downcast_ref::<RootModuleLoadingAnchorKey>().is_none()
                && key.downcast_ref::<HostRepoFileKey>().is_none()
                && key.downcast_ref::<HostFileBytesKey>().is_none()
            {
                return;
            }
            self.entries.lock().unwrap().push(TrackedBatch {
                key: key.to_string(),
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    #[derive(Default)]
    struct DependencyTracker {
        dependencies: Mutex<Vec<String>>,
    }

    impl DependencyTracker {
        fn take(&self) -> Vec<String> {
            std::mem::take(&mut *self.dependencies.lock().unwrap())
        }
    }

    impl ActivationTracker for DependencyTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            if key.downcast_ref::<HostRootModuleFileKey>().is_some() {
                self.dependencies
                    .lock()
                    .unwrap()
                    .extend(deps.map(ToString::to_string));
            }
        }
    }

    fn policy(roots: &[&str]) -> RootPackagePolicyInputs {
        RootPackagePolicyInputs::new(
            workspace(),
            roots
                .iter()
                .map(|root| NormalizedAbsolutePath::new(*root).unwrap())
                .collect::<Vec<_>>(),
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap()
    }

    fn snapshot(lockfile: Option<&str>) -> Arc<WorkspaceSnapshot> {
        let files = lockfile
            .into_iter()
            .map(|source| {
                (
                    PathBuf::from("/workspace/MODULE.bazel.lock"),
                    WorkspaceFileValue::Present(Arc::new(source.to_owned())),
                )
            })
            .collect::<SortedMap<_, _>>();
        Arc::new(WorkspaceSnapshot {
            files: Arc::new(files),
        })
    }

    async fn observed(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        roots: &[&str],
        capture_events: bool,
        tracker: Option<Arc<EventTracker>>,
        environment: Option<&str>,
        lockfile_mode: LockfileMode,
        lockfile: Option<&str>,
    ) -> super::HostRootModuleFileOutcome {
        let mut user_data = UserComputationData {
            activation_tracker: tracker.map(|tracker| tracker as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        if capture_events {
            user_data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(lockfile),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(roots)).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(environment).unwrap(),
            lockfile_mode,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootModuleFileKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_anchor(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        tracker: &Arc<EventTracker>,
    ) -> SourcePreparationOutcome<Arc<Result<RootModuleLoadingAnchor, RootModuleLoadingAnchorError>>>
    {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: PathBuf::from("/workspace"),
                },
                snapshot(None),
            )])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&RootModuleLoadingAnchorKey::new(workspace()))
            .await
            .unwrap()
    }

    async fn observed_route(
        dice: &Arc<Dice>,
        epoch: PathObservationEpoch,
        apparent_repo: &str,
    ) -> <RootRepositoryRouteKey as Key>::Value {
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(
                &RootRepositoryRouteKey::new(
                    workspace(),
                    ApparentRepoName::new(apparent_repo).unwrap(),
                )
                .unwrap(),
            )
            .await
            .unwrap()
    }

    fn complete_value(outcome: &super::HostRootModuleFileOutcome) -> &HostRootModuleFileValue {
        match outcome {
            SourcePreparationOutcome::Complete(value) => value.as_ref().as_ref().unwrap(),
            SourcePreparationOutcome::Need(need) => panic!("unexpected Need: {need:?}"),
        }
    }

    fn event_texts(batch: &EventBatch) -> Vec<&str> {
        batch
            .events()
            .iter()
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => "<diagnostic>",
            })
            .collect()
    }

    #[test]
    fn loading_anchor_identity_equality_and_opacity_are_exact() {
        let key = RootModuleLoadingAnchorKey::new(workspace());
        assert_eq!(key, RootModuleLoadingAnchorKey::new(workspace()));
        let other_workspace = NormalizedAbsolutePath::new("/other-workspace").unwrap();
        assert_ne!(key, RootModuleLoadingAnchorKey::new(other_workspace));
        assert_eq!(key.to_string(), "root-module-loading-anchor:\"/workspace\"");

        let success = |value| RootModuleLoadingAnchor {
            carrier: Arc::new(Ok(value)),
        };
        let success_a = success(empty_value());
        assert_eq!(format!("{success_a:?}"), "RootModuleLoadingAnchor");
        let equal_success_a = SourcePreparationOutcome::Complete(Arc::new(Ok(success_a)));
        let equal_success_b =
            SourcePreparationOutcome::Complete(Arc::new(Ok(success(empty_value()))));
        assert!(RootModuleLoadingAnchorKey::equality(
            &equal_success_a,
            &equal_success_b
        ));
        assert!(RootModuleLoadingAnchorKey::validity(&equal_success_a));
        let mut different_value = empty_value();
        different_value.module_file_paths = Arc::from([PathBuf::from("different")]);
        let unequal_success =
            SourcePreparationOutcome::Complete(Arc::new(Ok(success(different_value))));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &equal_success_a,
            &unequal_success
        ));
        let private_error = HostRootModuleFileError::CommandPolicy {
            message: "PRIVATE_DIAGNOSTIC".into(),
        };
        let expected_display = private_error.to_string();
        let error = |message: &str| RootModuleLoadingAnchorError {
            carrier: Arc::new(Err(HostRootModuleFileError::CommandPolicy {
                message: message.into(),
            })),
        };
        let error_a = error("PRIVATE_DIAGNOSTIC");
        assert_eq!(format!("{error_a:?}"), "RootModuleLoadingAnchorError");
        assert!(!format!("{error_a:?}").contains("PRIVATE_DIAGNOSTIC"));
        assert_eq!(error_a.to_string(), expected_display);
        assert!(error_a.source().is_none());
        let equal_error_a = SourcePreparationOutcome::Complete(Arc::new(Err(error_a)));
        let equal_error_b =
            SourcePreparationOutcome::Complete(Arc::new(Err(error("PRIVATE_DIAGNOSTIC"))));
        assert!(RootModuleLoadingAnchorKey::equality(
            &equal_error_a,
            &equal_error_b
        ));
        assert!(RootModuleLoadingAnchorKey::validity(&equal_error_a));
        let unequal_error =
            SourcePreparationOutcome::Complete(Arc::new(Err(error("DIFFERENT_DIAGNOSTIC"))));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &equal_error_a,
            &unequal_error
        ));
    }

    #[tokio::test]
    async fn repository_route_maps_direct_alias_and_rejects_unknown_without_legacy_graph() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let source = "module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\", repo_name = \"dep_alias\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n";
        let route = observed_route(&dice, EpochBuilder::root(source, 1).build(), "dep_alias").await;
        let SourcePreparationOutcome::Complete(route) = route else {
            panic!("complete root module returned Need");
        };
        let route = route.as_ref().as_ref().unwrap();
        assert_eq!(route.apparent_repo().as_str(), "dep_alias");
        assert_eq!(route.module_name(), "dep");
        assert_eq!(route.canonical_repo().as_str(), "dep+");
        assert_eq!(route.workspace(), &workspace());
        let hash = |route: &super::RootRepositoryRoute| {
            let mut state = DefaultHasher::new();
            route.hash(&mut state);
            state.finish()
        };
        let mut changed_spec = route.clone();
        let RootRepositorySource::DirectLocal(spec) = &mut changed_spec.source else {
            panic!("test route is direct local");
        };
        Arc::make_mut(&mut spec.attributes).insert(
            "path".into(),
            crate::OverrideAttributeValue::String("other-dep".into()),
        );
        assert_ne!(route, &changed_spec);
        assert_ne!(hash(route), hash(&changed_spec));

        let builtin =
            observed_route(&dice, EpochBuilder::root(source, 1).build(), "bazel_tools").await;
        let SourcePreparationOutcome::Complete(builtin) = builtin else {
            panic!("complete root module returned Need");
        };
        let builtin = builtin.as_ref().as_ref().unwrap();
        assert_eq!(builtin.canonical_repo().as_str(), "bazel_tools");
        assert_eq!(builtin.module_name(), "bazel_tools");
        let RootRepositorySource::BuiltinBazelTools(identity) = builtin.source() else {
            panic!("reserved route must use built-in source identity");
        };
        assert_eq!(
            identity.snapshot(),
            crate::BuiltinBazelToolsSnapshot::Bazel9_2
        );
        assert_eq!(
            hex::encode(identity.manifest_sha256()),
            "95b4af7c011047d34f6c469d23efdd523c56ce4892791e49f9d2159353262897"
        );

        let unknown = observed_route(&dice, EpochBuilder::root(source, 1).build(), "missing").await;
        let SourcePreparationOutcome::Complete(unknown) = unknown else {
            panic!("complete root module returned Need");
        };
        assert_eq!(
            unknown.as_ref().as_ref().unwrap_err().to_string(),
            "no such package '@@[unknown repo 'missing' requested from @@]//': The repository '@@[unknown repo 'missing' requested from @@]' could not be resolved: No repository visible as '@missing' from main repository"
        );

        let nodep = observed_route(
            &dice,
            EpochBuilder::root(
                "module(name = \"root\")\nbazel_dep(name = \"nodep_dep\", version = \"1.0.0\", repo_name = None)\nlocal_path_override(module_name = \"nodep_dep\", path = \"dep\")\n",
                2,
            )
            .build(),
            "nodep_dep",
        )
        .await;
        let SourcePreparationOutcome::Complete(nodep) = nodep else {
            panic!("complete root module returned Need");
        };
        assert!(nodep.as_ref().is_err());

        let unsupported = observed_route(
            &dice,
            EpochBuilder::root(
                "module(name = \"root\")\nbazel_dep(name = \"registry_dep\", version = \"1.0.0\")\n",
                3,
            )
            .build(),
            "registry_dep",
        )
        .await;
        let SourcePreparationOutcome::Complete(unsupported) = unsupported else {
            panic!("complete root module returned Need");
        };
        assert!(
            unsupported
                .as_ref()
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("is not a direct local_path_override")
        );
    }

    #[tokio::test]
    async fn builtin_route_root_edit_restore_keeps_immutable_source_identity() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let route = |value: <RootRepositoryRouteKey as Key>::Value| {
            let SourcePreparationOutcome::Complete(value) = value else {
                panic!("complete root module returned Need");
            };
            value.as_ref().as_ref().unwrap().clone()
        };
        let first = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"first\")\n", 1).build(),
                "bazel_tools",
            )
            .await,
        );
        let middle = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"middle\")\n", 2).build(),
                "bazel_tools",
            )
            .await,
        );
        let restored = route(
            observed_route(
                &dice,
                EpochBuilder::root("module(name = \"first\")\n", 3).build(),
                "bazel_tools",
            )
            .await,
        );
        assert_eq!(first.source(), middle.source());
        assert_eq!(first, restored);

        let other_workspace = RootRepositoryRoute::builtin_for_test(
            NormalizedAbsolutePath::new("/other-workspace").unwrap(),
        );
        assert_ne!(first, other_workspace);
        assert_eq!(first.source(), other_workspace.source());
    }

    #[tokio::test]
    async fn loading_anchor_retained_dice_lifecycle_and_event_closure_are_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let source = |reversed: bool| {
            if reversed {
                "print('ANCHOR_EVENT')\nregister_execution_platforms('//:second', '//:first')\n"
            } else {
                "print('ANCHOR_EVENT')\nregister_execution_platforms('//:first', '//:second')\n"
            }
        };

        let first_path = observed_anchor(&dice, EpochBuilder::default().build(), &tracker).await;
        let SourcePreparationOutcome::Need(first_path_need) = &first_path else {
            panic!("expected first-path Need");
        };
        assert_eq!(
            first_path_need.path_observations().unwrap().demands()[0]
                .path()
                .as_path(),
            std::path::Path::new("/")
        );
        assert!(first_path_need.root_module_bootstrap_request().is_none());
        assert!(first_path_need.repository_materializations().is_empty());
        assert!(!RootModuleLoadingAnchorKey::validity(&first_path));
        assert!(!RootModuleLoadingAnchorKey::equality(
            &first_path,
            &first_path
        ));
        assert!(tracker.take().iter().all(|entry| {
            (!entry.key.starts_with("root-module-loading-anchor:")
                && !entry.key.starts_with("host-root-module-file:"))
                || entry.batch.is_none()
        }));
        tracker.take_anchor_dependencies();

        let mut missing = EpochBuilder::default();
        missing.directory("/", 1);
        missing.directory("/workspace", 1);
        missing.missing("/workspace/MODULE.bazel");
        let bootstrap = observed_anchor(&dice, missing.build(), &tracker).await;
        let SourcePreparationOutcome::Need(bootstrap_need) = &bootstrap else {
            panic!("expected root-bootstrap Need");
        };
        assert!(bootstrap_need.path_observations().is_none());
        assert!(bootstrap_need.root_module_bootstrap_request().is_some());
        assert!(bootstrap_need.repository_materializations().is_empty());
        assert!(tracker.take().iter().all(|entry| {
            (!entry.key.starts_with("root-module-loading-anchor:")
                && !entry.key.starts_with("host-root-module-file:"))
                || entry.batch.is_none()
        }));
        tracker.take_anchor_dependencies();

        let success = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 2).build(),
            &tracker,
        )
        .await;
        assert!(matches!(success, SourcePreparationOutcome::Complete(ref value) if value.is_ok()));
        let SourcePreparationOutcome::Complete(success_value) = &success else {
            unreachable!()
        };
        assert_eq!(
            success_value
                .as_ref()
                .as_ref()
                .unwrap()
                .registrations()
                .execution_platforms()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["//:first", "//:second"]
        );
        let success_events = tracker.take();
        let wrapper = success_events
            .iter()
            .find(|entry| entry.key.starts_with("root-module-loading-anchor:"))
            .unwrap();
        assert!(wrapper.batch.is_none());
        let producer = success_events
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .unwrap();
        assert_eq!(
            event_texts(producer.batch.as_ref().unwrap()),
            ["ANCHOR_EVENT"]
        );
        assert_eq!(
            tracker.take_anchor_dependencies(),
            vec![vec![String::from("host-root-module-file:\"/workspace\"")]]
        );

        let reversed =
            observed_anchor(&dice, EpochBuilder::root(source(true), 3).build(), &tracker).await;
        assert!(!RootModuleLoadingAnchorKey::equality(&success, &reversed));
        let SourcePreparationOutcome::Complete(reversed_value) = &reversed else {
            unreachable!()
        };
        assert_eq!(
            reversed_value
                .as_ref()
                .as_ref()
                .unwrap()
                .registrations()
                .execution_platforms()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["//:second", "//:first"]
        );
        tracker.take();
        tracker.take_anchor_dependencies();

        let error = observed_anchor(
            &dice,
            EpochBuilder::root("unknown_identifier\n", 4).build(),
            &tracker,
        )
        .await;
        assert!(matches!(error, SourcePreparationOutcome::Complete(ref value) if value.is_err()));
        tracker.take();
        tracker.take_anchor_dependencies();

        let restored = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 5).build(),
            &tracker,
        )
        .await;
        assert!(RootModuleLoadingAnchorKey::equality(&success, &restored));
        tracker.take();
        tracker.take_anchor_dependencies();

        let warm = observed_anchor(
            &dice,
            EpochBuilder::root(source(false), 5).build(),
            &tracker,
        )
        .await;
        assert!(RootModuleLoadingAnchorKey::equality(&restored, &warm));
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));
    }

    #[test]
    fn key_is_workspace_identified_and_equality_is_complete_only() {
        assert_eq!(
            HostRootModuleFileKey::new(workspace()),
            HostRootModuleFileKey::new(workspace())
        );

        let complete = SourcePreparationOutcome::Complete(std::sync::Arc::new(Ok(empty_value())));
        assert!(HostRootModuleFileKey::equality(&complete, &complete));
        assert!(HostRootModuleFileKey::validity(&complete));

        let need = SourcePreparationOutcome::Need(SourcePreparationNeeds::path(
            NeedPathObservations::singleton(slug_workspace_v2::PathObservationDemand::new(
                slug_workspace_v2::PathObservationNamespace::Host,
                NormalizedAbsolutePath::new("/workspace/MODULE.bazel").unwrap(),
                slug_workspace_v2::PathObservationOperation::Lstat,
            )),
        ));
        assert!(!HostRootModuleFileKey::equality(&need, &need));
        assert!(!HostRootModuleFileKey::validity(&need));
    }

    #[tokio::test]
    async fn missing_root_is_the_sole_bootstrap_need() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut epoch = EpochBuilder::default();
        epoch.directory("/", 1);
        epoch.directory("/workspace", 1);
        epoch.missing("/workspace/MODULE.bazel");
        let outcome = observed(
            &dice,
            epoch.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = outcome else {
            panic!("expected bootstrap Need");
        };
        assert_eq!(
            need.root_module_bootstrap_request()
                .unwrap()
                .module_path()
                .as_path(),
            std::path::Path::new("/workspace/MODULE.bazel")
        );
        assert!(need.path_observations().is_none());
        assert!(need.repository_materializations().is_empty());
    }

    #[tokio::test]
    async fn root_observation_needs_accumulate_and_root_lifecycle_recovers() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let stages = [
            EpochBuilder::default(),
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch
            },
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch.directory("/workspace", 1);
                epoch
            },
            {
                let mut epoch = EpochBuilder::default();
                epoch.directory("/", 1);
                epoch.directory("/workspace", 1);
                epoch.node("/workspace/MODULE.bazel", PathNodeKind::RegularFile, 1);
                epoch
            },
        ];
        let expected = [
            ("/", PathObservationOperation::Lstat),
            ("/workspace", PathObservationOperation::Lstat),
            ("/workspace/MODULE.bazel", PathObservationOperation::Lstat),
            (
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
        ];
        for (epoch, (path, operation)) in stages.into_iter().zip(expected) {
            let outcome = observed(
                &dice,
                epoch.build(),
                &["/workspace"],
                false,
                None,
                None,
                LockfileMode::Update,
                None,
            )
            .await;
            let SourcePreparationOutcome::Need(need) = outcome else {
                panic!("expected cumulative path Need");
            };
            let demands = need.path_observations().unwrap().demands();
            assert_eq!(demands.len(), 1);
            assert_eq!(demands[0].path().as_path(), std::path::Path::new(path));
            assert_eq!(demands[0].operation(), operation);
        }

        let a = observed(
            &dice,
            EpochBuilder::root("module(name='a')\n", 2).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a).module.header.as_ref().unwrap().name, "a");
        let b = observed(
            &dice,
            EpochBuilder::root("module(name='b')\n", 3).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&b).module.header.as_ref().unwrap().name, "b");

        let mut deleted = EpochBuilder::default();
        deleted.directory("/", 4);
        deleted.directory("/workspace", 4);
        deleted.missing("/workspace/MODULE.bazel");
        let deleted = observed(
            &dice,
            deleted.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Need(need)
                if need.root_module_bootstrap_request().is_some()
        ));

        let restored = observed(
            &dice,
            EpochBuilder::root("module(name='a')\n", 5).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&restored));
    }

    #[tokio::test]
    async fn root_and_next_horizon_prepare_failures_block_later_dependencies() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let root_failure =
            EpochBuilder::root("include('//pkg:a.MODULE.bazel')\nunknown_identifier\n", 1);
        let outcome = observed(
            &dice,
            root_failure.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::RootValidation { message, .. })
                        if message.contains("unknown_identifier")
                )
        ));
        assert!(tracker.take().iter().any(|entry| {
            entry.key.starts_with("host-root-module-file:")
                && entry
                    .batch
                    .as_ref()
                    .is_some_and(|batch| batch.events().is_empty())
        }));

        let mut horizon_failure = EpochBuilder::root("include('//pkg:a.MODULE.bazel')\n", 2);
        horizon_failure.repository_policy(&["/workspace"], 2);
        horizon_failure.package("/workspace", "pkg", 2);
        horizon_failure.file(
            "/workspace/pkg/a.MODULE.bazel",
            "include('//next:n.MODULE.bazel')\nunknown_identifier\n",
            2,
        );
        let outcome = observed(
            &dice,
            horizon_failure.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation {
                        raw_label,
                        location,
                        message,
                        ..
                    }) if raw_label == "//pkg:a.MODULE.bazel"
                        && location.start_line == 1
                        && message.contains("unknown_identifier")
                )
        ));

        let tracker = Arc::new(EventTracker::default());
        let mut full_closure_failure = EpochBuilder::root(
            "\
print('ROOT_PREFIX')
fail('earlier runtime failure')
include('//pkg:bad.MODULE.bazel')
",
            3,
        );
        full_closure_failure.repository_policy(&["/workspace"], 3);
        full_closure_failure.package("/workspace", "pkg", 3);
        full_closure_failure.file("/workspace/pkg/bad.MODULE.bazel", "unknown_identifier\n", 3);
        let outcome = observed(
            &dice,
            full_closure_failure.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation { raw_label, .. })
                        if raw_label == "//pkg:bad.MODULE.bazel"
                )
        ));
        let root_batch = tracker
            .take()
            .into_iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch)
            .unwrap();
        assert!(root_batch.events().is_empty());
    }

    #[tokio::test]
    async fn first_source_utf8_prepare_and_evaluation_failures_retain_include_context() {
        let root = "\
print('ROOT_PREFIX')
include('//a:a.MODULE.bazel')
include('//b:b.MODULE.bazel')
";
        let base = |variant| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.repository_policy(&["/workspace"], variant);
            epoch.package("/workspace", "a", variant);
            epoch.package("/workspace", "b", variant);
            epoch
        };

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut utf8 = base(1);
        utf8.file("/workspace/a/a.MODULE.bazel", [0xff], 1);
        let outcome = observed(
            &dice,
            utf8.build(),
            &["/workspace"],
            true,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeValidation {
                        raw_label,
                        location,
                        message,
                        ..
                    }) if raw_label == "//a:a.MODULE.bazel"
                        && location.start_line == 2
                        && message.contains("UTF-8")
                )
        ));

        let tracker = Arc::new(EventTracker::default());
        let mut evaluation = base(2);
        evaluation.file(
            "/workspace/a/a.MODULE.bazel",
            "print('A_PREFIX')\nfail('A_FAIL')\n",
            2,
        );
        evaluation.file(
            "/workspace/b/b.MODULE.bazel",
            "print('B_MUST_NOT_RUN')\nfail('B_FAIL')\n",
            2,
        );
        let outcome = observed(
            &dice,
            evaluation.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Complete(value) = &outcome else {
            panic!("expected terminal evaluation failure");
        };
        let HostRootModuleFileError::Evaluation {
            message,
            include_occurrences,
        } = value.as_ref().as_ref().unwrap_err()
        else {
            panic!("expected evaluation failure: {value:?}");
        };
        assert!(message.contains("A_FAIL"), "{message}");
        assert_eq!(include_occurrences[0].path, "//a:a.MODULE.bazel");
        assert_eq!(include_occurrences[0].location.start_line, 2);
        assert_eq!(include_occurrences[1].path, "//b:b.MODULE.bazel");
        assert_eq!(include_occurrences[1].location.start_line, 3);
        let root_batch = tracker
            .take()
            .into_iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch)
            .unwrap();
        assert_eq!(event_texts(&root_batch), ["ROOT_PREFIX", "A_PREFIX"]);
    }

    #[tokio::test]
    async fn grouped_byte_needs_union_and_obey_source_order_terminal_precedence() {
        let root = "include('//a:a.MODULE.bazel')\ninclude('//b:b.MODULE.bazel')\n";
        let base = |variant| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.repository_policy(&["/workspace"], variant);
            epoch.package("/workspace", "a", variant);
            epoch.package("/workspace", "b", variant);
            epoch
        };

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let package_tracker = Arc::new(EventTracker::default());
        let mut package_barrier = EpochBuilder::root(root, 0);
        package_barrier.repository_policy(&["/workspace"], 0);
        package_barrier.directory("/workspace/a", 0);
        package_barrier.missing("/workspace/a/BUILD.bazel");
        package_barrier.missing("/workspace/a/BUILD");
        let outcome = observed(
            &dice,
            package_barrier.build(),
            &["/workspace"],
            false,
            Some(package_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludePreflight { error })
                        if matches!(
                            error,
                            crate::host_include::HostRootIncludeError::Package {
                                raw_label,
                                ..
                            } if raw_label == "//a:a.MODULE.bazel"
                        )
                )
        ));
        let package_activations = package_tracker.take();
        assert!(package_activations.iter().all(|entry| {
            !entry.key.contains("/workspace/a/a.MODULE.bazel")
                && !entry.key.contains("/workspace/b/b.MODULE.bazel")
        }));
        assert!(
            package_activations
                .iter()
                .any(|entry| { entry.key.contains("/workspace/MODULE.bazel") })
        );

        let outcome = observed(
            &dice,
            base(1).build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        let SourcePreparationOutcome::Need(need) = outcome else {
            panic!("expected grouped byte Need");
        };
        let demands = need.path_observations().unwrap().demands();
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/a/a.MODULE.bazel")
        }));
        assert!(demands.iter().any(|demand| {
            demand.path().as_path() == std::path::Path::new("/workspace/b/b.MODULE.bazel")
        }));

        let mut terminal_first = base(2);
        terminal_first.missing("/workspace/a/a.MODULE.bazel");
        let outcome = observed(
            &dice,
            terminal_first.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeMissing { raw_label, .. })
                        if raw_label == "//a:a.MODULE.bazel"
                )
        ));

        let mut need_first = base(3);
        need_first.missing("/workspace/b/b.MODULE.bazel");
        let outcome = observed(
            &dice,
            need_first.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(outcome, SourcePreparationOutcome::Need(_)));
    }

    #[tokio::test]
    async fn occurrence_execution_and_path_set_collapse_are_distinct_from_dependency_dedupe() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let mut epoch = EpochBuilder::root(
            "\
include('//pkg:sub/x.MODULE.bazel')
include('//pkg/sub:x.MODULE.bazel')
include('//pkg:sub/x.MODULE.bazel')
",
            1,
        );
        epoch.repository_policy(&["/workspace"], 1);
        epoch.package("/workspace", "pkg", 1);
        epoch.package("/workspace", "pkg/sub", 1);
        epoch.package("/workspace", "nested", 1);
        epoch.file(
            "/workspace/pkg/sub/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nprint('X')\n",
            1,
        );
        epoch.file("/workspace/nested/n.MODULE.bazel", "print('NESTED')\n", 1);
        clear_validated_root_module_logical_ids();
        let outcome = observed(
            &dice,
            epoch.build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(
            take_validated_root_module_logical_ids()
                .iter()
                .map(|logical_id| logical_id.0.as_str())
                .collect::<Vec<_>>(),
            [
                "/workspace/MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/pkg/sub/x.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
                "/workspace/nested/n.MODULE.bazel",
            ],
            "compile validation preserves every include occurrence and horizon order"
        );
        let value = complete_value(&outcome);
        assert_eq!(
            value.module_file_paths.as_ref(),
            [
                PathBuf::from("MODULE.bazel"),
                PathBuf::from("nested/n.MODULE.bazel"),
                PathBuf::from("pkg/sub/x.MODULE.bazel")
            ]
        );
        let entries = tracker.take();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.key.contains("pkg/sub/x.MODULE.bazel"))
                .count(),
            1,
            "one grouped HostFileBytesKey owns all three include occurrences"
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.key.contains("nested/n.MODULE.bazel"))
                .count(),
            1,
            "the duplicated nested horizon also shares one HostFileBytesKey"
        );
        let root = entries
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(
            event_texts(root),
            ["NESTED", "X", "NESTED", "X", "NESTED", "X"]
        );

        let mut reordered = EpochBuilder::root(
            "\
include('//pkg/sub:x.MODULE.bazel')
include('//pkg:sub/x.MODULE.bazel')
include('//pkg/sub:x.MODULE.bazel')
",
            2,
        );
        reordered.repository_policy(&["/workspace"], 2);
        reordered.package("/workspace", "pkg", 2);
        reordered.package("/workspace", "pkg/sub", 2);
        reordered.package("/workspace", "nested", 2);
        reordered.file(
            "/workspace/pkg/sub/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nprint('X')\n",
            2,
        );
        reordered.file("/workspace/nested/n.MODULE.bazel", "print('NESTED')\n", 2);
        let reordered = observed(
            &dice,
            reordered.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&outcome), complete_value(&reordered));

        let marker_off_tracker = Arc::new(EventTracker::default());
        let mut epoch = EpochBuilder::root("print('DIRECT')\n", 3);
        epoch.repository_policy(&["/workspace"], 3);
        let outcome = observed(
            &Dice::builder().build(DetectCycles::Enabled),
            epoch.build(),
            &["/workspace"],
            false,
            Some(marker_off_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&outcome);
        assert!(
            marker_off_tracker
                .take()
                .iter()
                .all(|entry| entry.batch.is_none())
        );
    }

    #[tokio::test]
    async fn repo_child_and_root_include_events_keep_separate_membership_across_need_retry() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(EventTracker::default());
        let scenario = |include_bytes: bool, variant: i64| {
            let mut epoch = EpochBuilder::root(
                "\
print('ROOT_BEFORE')
include('//pkg:x.MODULE.bazel')
print('ROOT_AFTER')
",
                variant,
            );
            epoch.directory("/workspace/pkg", variant);
            epoch.file(
                "/workspace/REPO.bazel",
                "print('REPO')\nignore_directories([])\n",
                variant,
            );
            epoch.missing("/workspace/.bazelignore");
            epoch.package("/workspace", "pkg", variant);
            if include_bytes {
                epoch.file(
                    "/workspace/pkg/x.MODULE.bazel",
                    "print('INCLUDE')\n",
                    variant,
                );
            }
            epoch
        };

        let need = observed(
            &dice,
            scenario(false, 1).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(need, SourcePreparationOutcome::Need(_)));
        let first_attempt = tracker.take();
        let repo_batch = first_attempt
            .iter()
            .find(|entry| entry.key.starts_with("host-repo-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(event_texts(repo_batch), ["REPO"]);
        assert!(first_attempt.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:") || entry.batch.is_none()
        }));

        let complete = observed(
            &dice,
            scenario(true, 2).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&complete);
        let retry = tracker.take();
        let root_batch = retry
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(
            event_texts(root_batch),
            ["ROOT_BEFORE", "INCLUDE", "ROOT_AFTER"]
        );
        assert!(retry.iter().all(|entry| {
            !entry.key.starts_with("host-root-module-file:")
                || entry
                    .batch
                    .as_ref()
                    .is_some_and(|batch| !event_texts(batch).contains(&"REPO"))
        }));

        let warm = observed(
            &dice,
            scenario(true, 2).build(),
            &["/workspace"],
            true,
            Some(tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        complete_value(&warm);
        assert!(tracker.take().iter().all(|entry| entry.batch.is_none()));

        let terminal_dice = Dice::builder().build(DetectCycles::Enabled);
        let terminal_tracker = Arc::new(EventTracker::default());
        let mut terminal = EpochBuilder::root("include('//missing:x.MODULE.bazel')\n", 3);
        terminal.file(
            "/workspace/REPO.bazel",
            "print('REPO_TERMINAL')\nignore_directories([])\n",
            3,
        );
        terminal.missing("/workspace/.bazelignore");
        terminal.directory("/workspace/missing", 3);
        terminal.missing("/workspace/missing/BUILD.bazel");
        terminal.missing("/workspace/missing/BUILD");
        let outcome = observed(
            &terminal_dice,
            terminal.build(),
            &["/workspace"],
            true,
            Some(terminal_tracker.dupe()),
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            outcome,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludePreflight { .. })
                )
        ));
        let terminal_events = terminal_tracker.take();
        let repo_batch = terminal_events
            .iter()
            .find(|entry| entry.key.starts_with("host-repo-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert_eq!(event_texts(repo_batch), ["REPO_TERMINAL"]);
        let root_batch = terminal_events
            .iter()
            .find(|entry| entry.key.starts_with("host-root-module-file:"))
            .and_then(|entry| entry.batch.as_ref())
            .unwrap();
        assert!(root_batch.events().is_empty());
        assert!(
            terminal_events
                .iter()
                .all(|entry| !entry.key.contains("/workspace/missing/x.MODULE.bazel"))
        );
    }

    #[tokio::test]
    async fn alternate_special_include_and_root_include_lifecycle_recover_on_retained_dice() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let root = "include('//pkg:x.MODULE.bazel')\n";
        let scenario = |source: Option<&str>, variant: i64| {
            let mut epoch = EpochBuilder::root(root, variant);
            epoch.directory("/alternate", variant);
            epoch.repository_policy(&["/alternate"], variant);
            epoch.package("/alternate", "pkg", variant);
            match source {
                Some(source) => {
                    epoch.special("/alternate/pkg/x.MODULE.bazel", source, variant);
                }
                None => epoch.missing("/alternate/pkg/x.MODULE.bazel"),
            }
            epoch
        };
        let a = observed(
            &dice,
            scenario(Some("bazel_dep(name='a', version='1.0')\n"), 1).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a).module.dependencies[0].name.as_str(), "a");
        assert_eq!(
            complete_value(&a).module_file_paths.as_ref(),
            [
                PathBuf::from("MODULE.bazel"),
                PathBuf::from("pkg/x.MODULE.bazel")
            ]
        );

        let mut workspace_selected = EpochBuilder::root(root, 10);
        workspace_selected.repository_policy(&["/workspace"], 10);
        workspace_selected.package("/workspace", "pkg", 10);
        workspace_selected.file(
            "/workspace/pkg/x.MODULE.bazel",
            "bazel_dep(name='a', version='1.0')\n",
            10,
        );
        let workspace_dice = Dice::builder().build(DetectCycles::Enabled);
        let workspace_selected = observed(
            &workspace_dice,
            workspace_selected.build(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&workspace_selected));

        let b = observed(
            &dice,
            scenario(Some("bazel_dep(name='b', version='2.0')\n"), 2).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&b).module.dependencies[0].name.as_str(), "b");

        let deleted = observed(
            &dice,
            scenario(None, 3).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert!(matches!(
            deleted,
            SourcePreparationOutcome::Complete(value)
                if matches!(
                    value.as_ref(),
                    Err(HostRootModuleFileError::IncludeMissing { .. })
                )
        ));

        let restored = observed(
            &dice,
            scenario(Some("bazel_dep(name='a', version='1.0')\n"), 4).build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&restored));

        let mut invalid = EpochBuilder::root(root, 5);
        invalid.directory("/alternate", 5);
        invalid.repository_policy(&["/alternate"], 5);
        invalid.package("/alternate", "pkg", 5);
        invalid.special(
            "/alternate/pkg/x.MODULE.bazel",
            "include('//nested:n.MODULE.bazel')\nunknown_identifier\n",
            5,
        );
        let invalid = observed(
            &dice,
            invalid.build(),
            &["/alternate"],
            false,
            None,
            None,
            LockfileMode::Update,
            None,
        )
        .await;
        match invalid {
            SourcePreparationOutcome::Complete(value) => match value.as_ref() {
                Err(HostRootModuleFileError::IncludeValidation {
                    raw_label,
                    logical_path,
                    message,
                    ..
                }) => {
                    assert_eq!(raw_label.as_str(), "//pkg:x.MODULE.bazel");
                    assert_eq!(
                        logical_path.as_path(),
                        std::path::Path::new("/alternate/pkg/x.MODULE.bazel")
                    );
                    assert!(message.contains("/alternate/pkg/x.MODULE.bazel"));
                    assert!(message.contains("unknown_identifier"));
                }
                other => panic!("expected alternate-root include validation error, got {other:?}"),
            },
            SourcePreparationOutcome::Need(need) => {
                panic!("validation must precede nested include preflight, got {need:?}")
            }
        }
    }

    #[tokio::test]
    async fn lockfile_environment_and_mode_changes_are_not_root_dependencies() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(DependencyTracker::default());
        let epoch = || {
            let mut epoch = EpochBuilder::root("module(name='root')\n", 1);
            epoch.repository_policy(&["/workspace"], 1);
            epoch.build()
        };
        let a = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Update,
            Some("{\"lockFileVersion\":28,\"facts\":{\"a\":{}}}\n"),
        )
        .await;
        let b = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            Some("all"),
            LockfileMode::Off,
            Some("{\"lockFileVersion\":28,\"facts\":{\"b\":{}}}\n"),
        )
        .await;
        let restored = observed(
            &dice,
            epoch(),
            &["/workspace"],
            false,
            None,
            None,
            LockfileMode::Error,
            Some("{\"lockFileVersion\":28,\"facts\":{\"a\":{}}}\n"),
        )
        .await;
        assert_eq!(complete_value(&a), complete_value(&b));
        assert_eq!(complete_value(&a), complete_value(&restored));

        let dependency_dice = Dice::builder().build(DetectCycles::Enabled);
        let user_data = UserComputationData {
            activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
            ..Default::default()
        };
        let mut updater = dependency_dice.updater_with_data(user_data);
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch())])
            .unwrap();
        inject_root_package_policy_inputs(&mut updater, policy(&["/workspace"])).unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace().as_path(),
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(Some("all")).unwrap(),
            LockfileMode::Refresh,
        )
        .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRootModuleFileKey::new(workspace()))
            .await
            .unwrap();
        let dependencies = tracker.take();
        assert!(
            dependencies
                .iter()
                .any(|dependency| dependency.starts_with("bzlmod-host-file-bytes:")),
            "the tracker observed the Host root key's direct dependency set"
        );
        assert!(dependencies.iter().all(|dependency| {
            !dependency.starts_with("visible-lockfile:")
                && !dependency.starts_with("root-module-environment-policy:")
                && !dependency.starts_with("root-module-lockfile-mode:")
                && !dependency.starts_with("root-module-graph:")
        }));
    }
}
