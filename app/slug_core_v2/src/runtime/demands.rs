#![allow(dead_code)] // Selected snapshots are consumed by the later shared command driver.

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use allocative::Allocative;
use dice::ActivationClosure;
use dice::ActivationClosureNode;
use dice::ActivationData;
use dice::ActivationTracker;
use dice::Dice;
use dice::DiceNodeId;
use dice::DynKey;
use dice::RichActivation;
use dice::RootActivation;
use dice::UserComputationData;
use dupe::Dupe;
use slug_bzlmod_v2::RepositoryMaterializationRequest;
use slug_bzlmod_v2::RepositoryMaterializationRequestId;
use slug_bzlmod_v2::source_preparation::RepositorySourceScope;
use slug_events_v2::CaptureEvaluationEvents;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathObservationDemand;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use super::events::AttemptEffectTracker;
use super::events::CommandEffectError;

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
enum DemandNodeMetadata {
    Path(PathObservationDemand),
    Repository(Arc<RepositoryMaterializationRequest>),
    Source(RepositorySourceScope),
}

#[derive(Debug)]
struct WorkspaceDemandState {
    catalogue: SmallMap<DiceNodeId, DemandNodeMetadata>,
    failure: Option<DemandProvenanceError>,
}

/// Sparse key-static demand metadata for the lifetime of one retained engine.
pub(super) struct WorkspaceDemandOwner {
    dice: Weak<Dice>,
    workspace: NormalizedAbsolutePath,
    state: Mutex<WorkspaceDemandState>,
}

/// The sole activation tracker installed by a retained workspace runtime.
struct RuntimeActivationTracker {
    demands: Arc<WorkspaceDemandOwner>,
    effects: Option<Arc<AttemptEffectTracker>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DemandProvenanceError {
    MultipleKeyMetadata {
        node: DiceNodeId,
    },
    ConflictingNodeMetadata {
        node: DiceNodeId,
    },
    MetadataWorkspaceMismatch {
        node: DiceNodeId,
    },
    RepositoryRequestConflict {
        id: RepositoryMaterializationRequestId,
    },
    ClosureNodeMissing {
        node: DiceNodeId,
    },
    SourceRequestMissing {
        node: DiceNodeId,
        scope: RepositorySourceScope,
    },
    SourceRequestConflict {
        node: DiceNodeId,
        scope: RepositorySourceScope,
    },
    SourceWorkspaceMismatch {
        node: DiceNodeId,
        scope: RepositorySourceScope,
        request_workspace: slug_workspace_v2::NormalizedAbsolutePath,
    },
    ScopeRequestConflict {
        scope: RepositorySourceScope,
    },
}

impl fmt::Display for DemandProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MultipleKeyMetadata { node } => {
                write!(f, "DICE node {node:?} provided multiple demand descriptors")
            }
            Self::ConflictingNodeMetadata { node } => {
                write!(f, "DICE node {node:?} changed its demand descriptor")
            }
            Self::MetadataWorkspaceMismatch { node } => {
                write!(
                    f,
                    "DICE node {node:?} provided demand metadata for another workspace"
                )
            }
            Self::RepositoryRequestConflict { id } => write!(
                f,
                "repository {} has conflicting exact materialization requests",
                id.canonical_repo
            ),
            Self::ClosureNodeMissing { node } => {
                write!(f, "activation closure omitted dependency node {node:?}")
            }
            Self::SourceRequestMissing { node, scope } => write!(
                f,
                "repository source node {node:?} for module {} has no exact request descendant",
                scope.module_name
            ),
            Self::SourceRequestConflict { node, scope } => write!(
                f,
                "repository source node {node:?} for module {} has conflicting request descendants",
                scope.module_name
            ),
            Self::SourceWorkspaceMismatch {
                node,
                scope,
                request_workspace,
            } => write!(
                f,
                "repository source node {node:?} for module {} belongs to workspace {} but its request belongs to {}",
                scope.module_name,
                scope.workspace.as_path().display(),
                request_workspace.as_path().display()
            ),
            Self::ScopeRequestConflict { scope } => write!(
                f,
                "repository source scope {} has conflicting exact requests",
                scope.module_name
            ),
        }
    }
}

impl std::error::Error for DemandProvenanceError {}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub(super) struct SelectedRepositoryValidation {
    scope: RepositorySourceScope,
    request: Arc<RepositoryMaterializationRequest>,
    paths: Arc<[PathObservationDemand]>,
}

impl SelectedRepositoryValidation {
    pub(super) fn scope(&self) -> &RepositorySourceScope {
        &self.scope
    }

    pub(super) fn request(&self) -> &Arc<RepositoryMaterializationRequest> {
        &self.request
    }

    pub(super) fn paths(&self) -> &[PathObservationDemand] {
        &self.paths
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Allocative)]
pub(super) struct SelectedWorkspaceDemands {
    repository_requests: Arc<[Arc<RepositoryMaterializationRequest>]>,
    repository_validations: Arc<[SelectedRepositoryValidation]>,
    unscoped_paths: Arc<[PathObservationDemand]>,
}

impl SelectedWorkspaceDemands {
    pub(super) fn empty() -> Self {
        Self {
            repository_requests: Arc::from([]),
            repository_validations: Arc::from([]),
            unscoped_paths: Arc::from([]),
        }
    }

    pub(super) fn with_additional_unscoped_paths(
        mut self,
        paths: impl IntoIterator<Item = PathObservationDemand>,
    ) -> Self {
        let mut unscoped_paths = self.unscoped_paths.to_vec();
        unscoped_paths.extend(paths);
        unscoped_paths.sort_unstable();
        unscoped_paths.dedup();
        self.unscoped_paths = unscoped_paths.into();
        self
    }

    #[cfg(test)]
    pub(super) fn for_test(
        repository_requests: impl Into<Arc<[Arc<RepositoryMaterializationRequest>]>>,
        unscoped_paths: impl Into<Arc<[PathObservationDemand]>>,
    ) -> Self {
        Self {
            repository_requests: repository_requests.into(),
            repository_validations: Arc::from([]),
            unscoped_paths: unscoped_paths.into(),
        }
    }

    #[cfg(test)]
    pub(super) fn for_test_with_validation(
        repository_requests: impl Into<Arc<[Arc<RepositoryMaterializationRequest>]>>,
        validation_request: Arc<RepositoryMaterializationRequest>,
        validation_path: PathObservationDemand,
    ) -> Self {
        Self {
            repository_requests: repository_requests.into(),
            repository_validations: Arc::from([SelectedRepositoryValidation {
                scope: RepositorySourceScope {
                    workspace: validation_request.id.workspace.clone(),
                    module_name: "test_validation".into(),
                },
                request: validation_request,
                paths: Arc::from([validation_path]),
            }]),
            unscoped_paths: Arc::from([]),
        }
    }

    pub(super) fn repository_requests(&self) -> &[Arc<RepositoryMaterializationRequest>] {
        &self.repository_requests
    }

    pub(super) fn repository_validations(&self) -> &[SelectedRepositoryValidation] {
        &self.repository_validations
    }

    pub(super) fn unscoped_paths(&self) -> &[PathObservationDemand] {
        &self.unscoped_paths
    }
}

#[derive(Debug)]
struct AnchorSelection {
    scope: RepositorySourceScope,
    requests: SmallMap<RepositoryMaterializationRequestId, Arc<RepositoryMaterializationRequest>>,
    paths: SmallSet<PathObservationDemand>,
}

#[derive(Debug)]
struct ScopeSelection {
    request: Arc<RepositoryMaterializationRequest>,
    paths: SmallSet<PathObservationDemand>,
}

impl WorkspaceDemandOwner {
    pub(super) fn new(dice: &Arc<Dice>, workspace: NormalizedAbsolutePath) -> Arc<Self> {
        Arc::new(Self {
            dice: Arc::downgrade(dice),
            workspace,
            state: Mutex::new(WorkspaceDemandState {
                catalogue: SmallMap::new(),
                failure: None,
            }),
        })
    }

    pub(super) fn install(
        self: &Arc<Self>,
        dice: &Arc<Dice>,
        data: &mut UserComputationData,
        effects: Option<Arc<AttemptEffectTracker>>,
    ) -> Result<(), CommandEffectError> {
        if data.activation_tracker.is_some() {
            return Err(CommandEffectError::ActivationTrackerAlreadyInstalled);
        }
        if !self.belongs_to(dice) {
            return Err(CommandEffectError::ForeignDemandOwner);
        }
        let tracker: Arc<dyn ActivationTracker> = Arc::new(RuntimeActivationTracker {
            demands: self.dupe(),
            effects: effects.dupe(),
        });
        if let Some(effects) = effects {
            effects.reserve_install(self)?;
            data.data.set(CaptureEvaluationEvents);
        }
        data.activation_tracker = Some(tracker);
        Ok(())
    }

    pub(super) fn belongs_to(&self, dice: &Arc<Dice>) -> bool {
        self.dice
            .upgrade()
            .is_some_and(|owner_dice| Arc::ptr_eq(&owner_dice, dice))
    }

    fn metadata(
        &self,
        node: DiceNodeId,
        key: &DynKey,
    ) -> Result<Option<DemandNodeMetadata>, DemandProvenanceError> {
        let path = key.request_value::<PathObservationDemand>();
        let repository = key.request_value::<Arc<RepositoryMaterializationRequest>>();
        let source = key.request_value::<RepositorySourceScope>();
        let supplied = usize::from(path.is_some())
            + usize::from(repository.is_some())
            + usize::from(source.is_some());
        if supplied > 1 {
            return Err(DemandProvenanceError::MultipleKeyMetadata { node });
        }
        let metadata = path
            .map(DemandNodeMetadata::Path)
            .or_else(|| repository.map(DemandNodeMetadata::Repository))
            .or_else(|| source.map(DemandNodeMetadata::Source));
        match &metadata {
            Some(DemandNodeMetadata::Repository(request))
                if request.id.workspace != self.workspace =>
            {
                Err(DemandProvenanceError::MetadataWorkspaceMismatch { node })
            }
            Some(DemandNodeMetadata::Source(scope)) if scope.workspace != self.workspace => {
                Err(DemandProvenanceError::MetadataWorkspaceMismatch { node })
            }
            _ => Ok(metadata),
        }
    }

    fn record_key(&self, node: DiceNodeId, key: &DynKey) {
        let metadata = match self.metadata(node, key) {
            Ok(Some(metadata)) => metadata,
            Ok(None) => return,
            Err(error) => {
                self.latch(error);
                return;
            }
        };
        let mut state = self
            .state
            .lock()
            .expect("workspace demand owner mutex poisoned");
        if state.failure.is_some() {
            return;
        }
        match state.catalogue.get(&node) {
            Some(existing) if existing != &metadata => {
                state.failure = Some(DemandProvenanceError::ConflictingNodeMetadata { node });
            }
            Some(_) => {}
            None => {
                state.catalogue.insert(node, metadata);
            }
        }
    }

    fn latch(&self, error: DemandProvenanceError) {
        let mut state = self
            .state
            .lock()
            .expect("workspace demand owner mutex poisoned");
        if state.failure.is_none() {
            state.failure = Some(error);
        }
    }

    pub(super) fn select(
        &self,
        closure: &ActivationClosure,
    ) -> Result<SelectedWorkspaceDemands, DemandProvenanceError> {
        let catalogue = {
            let state = self
                .state
                .lock()
                .expect("workspace demand owner mutex poisoned");
            if let Some(error) = &state.failure {
                return Err(error.clone());
            }
            let mut subset = SmallMap::new();
            for node in closure.nodes() {
                if let Some(metadata) = state.catalogue.get(&node.node()) {
                    subset.insert(node.node(), metadata.clone());
                }
            }
            subset
        };
        select_demands(closure, &catalogue)
    }

    #[cfg(test)]
    fn only_catalogued_node(&self) -> DiceNodeId {
        let state = self
            .state
            .lock()
            .expect("workspace demand owner mutex poisoned");
        assert_eq!(state.catalogue.len(), 1);
        *state.catalogue.keys().next().expect("one catalogued node")
    }
}

impl ActivationTracker for RuntimeActivationTracker {
    fn key_activated(
        &self,
        _key: &DynKey,
        _deps: &mut dyn Iterator<Item = &DynKey>,
        _activation_data: ActivationData,
    ) {
    }

    fn tracks_rich_activations(&self) -> bool {
        true
    }

    fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
        self.demands.record_key(activation.node(), key);
        if let Some(effects) = &self.effects {
            effects.record_activation(activation);
        }
    }

    fn root_activated(&self, _key: &DynKey, activation: RootActivation) {
        if let Some(effects) = &self.effects {
            effects.record_root(activation);
        }
    }
}

fn select_demands(
    closure: &ActivationClosure,
    catalogue: &SmallMap<DiceNodeId, DemandNodeMetadata>,
) -> Result<SelectedWorkspaceDemands, DemandProvenanceError> {
    let mut nodes: SmallMap<DiceNodeId, &ActivationClosureNode> =
        SmallMap::with_capacity(closure.nodes().len());
    let mut requests = SmallMap::new();
    for node in closure.nodes() {
        nodes.insert(node.node(), node);
        if let Some(DemandNodeMetadata::Repository(request)) = catalogue.get(&node.node()) {
            match requests.get(&request.id) {
                Some(existing) if existing != request => {
                    return Err(DemandProvenanceError::RepositoryRequestConflict {
                        id: request.id.clone(),
                    });
                }
                Some(_) => {}
                None => {
                    requests.insert(request.id.clone(), request.dupe());
                }
            }
        }
    }

    let mut anchors = SmallMap::<DiceNodeId, AnchorSelection>::new();
    let mut unscoped_paths = SmallSet::new();
    let mut visited = SmallSet::new();
    let mut pending = closure
        .roots()
        .iter()
        .rev()
        .map(|node| (*node, None))
        .collect::<Vec<_>>();
    while let Some((node, inherited_anchor)) = pending.pop() {
        let metadata = catalogue.get(&node);
        let active_anchor = match metadata {
            Some(DemandNodeMetadata::Source(scope)) => {
                if !anchors.contains_key(&node) {
                    anchors.insert(
                        node,
                        AnchorSelection {
                            scope: scope.clone(),
                            requests: SmallMap::new(),
                            paths: SmallSet::new(),
                        },
                    );
                }
                Some(node)
            }
            _ => inherited_anchor,
        };
        if !visited.insert((node, active_anchor)) {
            continue;
        }
        match metadata {
            Some(DemandNodeMetadata::Path(path)) => match active_anchor {
                Some(anchor) => {
                    anchors
                        .get_mut(&anchor)
                        .expect("active source anchor was inserted")
                        .paths
                        .insert(path.clone());
                }
                None => {
                    unscoped_paths.insert(path.clone());
                }
            },
            Some(DemandNodeMetadata::Repository(request)) => {
                if let Some(anchor) = active_anchor {
                    let anchor = anchors
                        .get_mut(&anchor)
                        .expect("active source anchor was inserted");
                    anchor.requests.insert(request.id.clone(), request.dupe());
                }
            }
            Some(DemandNodeMetadata::Source(_)) | None => {}
        }
        let closure_node = nodes
            .get(&node)
            .ok_or(DemandProvenanceError::ClosureNodeMissing { node })?;
        for dependency in closure_node.dependencies().iter().rev() {
            pending.push((*dependency, active_anchor));
        }
    }

    let mut scopes = SmallMap::<RepositorySourceScope, ScopeSelection>::new();
    for (node, anchor) in anchors {
        if anchor.paths.is_empty() {
            continue;
        }
        let mut requests = anchor.requests.into_iter();
        let Some((_, request)) = requests.next() else {
            return Err(DemandProvenanceError::SourceRequestMissing {
                node,
                scope: anchor.scope,
            });
        };
        if requests.next().is_some() {
            return Err(DemandProvenanceError::SourceRequestConflict {
                node,
                scope: anchor.scope,
            });
        }
        if request.id.workspace != anchor.scope.workspace {
            return Err(DemandProvenanceError::SourceWorkspaceMismatch {
                node,
                scope: anchor.scope,
                request_workspace: request.id.workspace.dupe(),
            });
        }
        match scopes.get_mut(&anchor.scope) {
            Some(existing) if existing.request != request => {
                return Err(DemandProvenanceError::ScopeRequestConflict {
                    scope: anchor.scope,
                });
            }
            Some(existing) => existing.paths.extend(anchor.paths),
            None => {
                scopes.insert(
                    anchor.scope,
                    ScopeSelection {
                        request,
                        paths: anchor.paths,
                    },
                );
            }
        }
    }

    let mut repository_requests = requests.into_values().collect::<Vec<_>>();
    repository_requests.sort_by(|left, right| {
        left.id
            .workspace
            .cmp(&right.id.workspace)
            .then_with(|| left.id.canonical_repo.cmp(&right.id.canonical_repo))
    });
    let mut repository_validations = scopes
        .into_iter()
        .map(|(scope, selection)| {
            let mut paths = selection.paths.into_iter().collect::<Vec<_>>();
            paths.sort_unstable();
            paths.dedup();
            SelectedRepositoryValidation {
                scope,
                request: selection.request,
                paths: paths.into(),
            }
        })
        .collect::<Vec<_>>();
    repository_validations.sort_by(|left, right| {
        left.scope
            .workspace
            .cmp(&right.scope.workspace)
            .then_with(|| left.scope.module_name.cmp(&right.scope.module_name))
    });
    let mut unscoped_paths = unscoped_paths.into_iter().collect::<Vec<_>>();
    unscoped_paths.sort_unstable();
    unscoped_paths.dedup();
    Ok(SelectedWorkspaceDemands {
        repository_requests: repository_requests.into(),
        repository_validations: repository_validations.into(),
        unscoped_paths: unscoped_paths.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use allocative::Allocative;
    use async_trait::async_trait;
    use compact_str::CompactString;
    use dice::CancellationContext;
    use dice::DetectCycles;
    use dice::Dice;
    use dice::DiceComputations;
    use dice::DynKey;
    use dice::Key;
    use dice::UserComputationData;
    use slug_bzlmod_v2::RepoRuleId;
    use slug_bzlmod_v2::RepoSpec;
    use slug_bzlmod_v2::RepositoryMaterializationKind;
    use slug_bzlmod_v2::RepositoryMaterializationRequest;
    use slug_bzlmod_v2::RepositoryMaterializationRequestId;
    use slug_bzlmod_v2::source_preparation::RepositorySourceScope;
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationEvent;
    use slug_events_v2::EventBatch;
    use slug_identity_v2::CanonicalLabel;
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;
    use tokio::sync::Notify;

    use super::DemandProvenanceError;
    use super::SelectedWorkspaceDemands;
    use super::WorkspaceDemandOwner;
    use crate::runtime::events::CommandEffectError;
    use crate::runtime::events::CommandEffectOwner;

    #[derive(Clone, Debug, Allocative)]
    enum TestDescriptor {
        None,
        Path(PathObservationDemand),
        Request(Arc<RepositoryMaterializationRequest>),
        Scope(RepositorySourceScope),
    }

    #[derive(Clone, Debug, Allocative)]
    struct DemandTestKey {
        id: u64,
        descriptor: TestDescriptor,
        dependencies: Arc<[DemandTestKey]>,
        event: Option<&'static str>,
        #[allocative(skip)]
        probes: Option<Arc<AtomicUsize>>,
        #[allocative(skip)]
        gate: Option<Arc<TestGate>>,
    }

    #[derive(Debug)]
    struct TestGate {
        entered: Notify,
        release: Notify,
    }

    impl PartialEq for DemandTestKey {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl Eq for DemandTestKey {}

    impl Hash for DemandTestKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }

    impl fmt::Display for DemandTestKey {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "demand-test-{}", self.id)
        }
    }

    #[async_trait]
    impl Key for DemandTestKey {
        type Value = ();

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            _cancellations: &CancellationContext,
        ) -> Self::Value {
            if let Some(gate) = &self.gate {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
            for dependency in self.dependencies.iter() {
                ctx.compute(dependency).await.unwrap();
            }
            if let Some(event) = self.event {
                ctx.store_evaluation_data(batch(event)).unwrap();
            }
        }

        fn equality(_left: &Self::Value, _right: &Self::Value) -> bool {
            true
        }

        fn provide<'a>(&'a self, demand: &mut dice::Demand<'a>) {
            match &self.descriptor {
                TestDescriptor::None => {}
                TestDescriptor::Path(path) => {
                    demand.provide_value_with(|| {
                        if let Some(probes) = &self.probes {
                            probes.fetch_add(1, Ordering::Relaxed);
                        }
                        path.clone()
                    });
                }
                TestDescriptor::Request(request) => {
                    demand.provide_value_with(|| {
                        if let Some(probes) = &self.probes {
                            probes.fetch_add(1, Ordering::Relaxed);
                        }
                        request.clone()
                    });
                }
                TestDescriptor::Scope(scope) => {
                    demand.provide_value_with(|| {
                        if let Some(probes) = &self.probes {
                            probes.fetch_add(1, Ordering::Relaxed);
                        }
                        scope.clone()
                    });
                }
            }
        }
    }

    fn key(id: u64, descriptor: TestDescriptor, dependencies: Vec<DemandTestKey>) -> DemandTestKey {
        DemandTestKey {
            id,
            descriptor,
            dependencies: dependencies.into(),
            event: None,
            probes: None,
            gate: None,
        }
    }

    fn instrumented_key(
        id: u64,
        descriptor: TestDescriptor,
        dependencies: Vec<DemandTestKey>,
        event: Option<&'static str>,
        probes: Option<Arc<AtomicUsize>>,
        gate: Option<Arc<TestGate>>,
    ) -> DemandTestKey {
        DemandTestKey {
            id,
            descriptor,
            dependencies: dependencies.into(),
            event,
            probes,
            gate,
        }
    }

    fn plain(id: u64, dependencies: Vec<DemandTestKey>) -> DemandTestKey {
        key(id, TestDescriptor::None, dependencies)
    }

    fn path(value: &str) -> PathObservationDemand {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(value).unwrap(),
            PathObservationOperation::FileBytes,
        )
    }

    fn scope(module_name: &str) -> RepositorySourceScope {
        RepositorySourceScope {
            workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
            module_name: module_name.into(),
        }
    }

    fn request(repo: &str, rule_name: &str) -> Arc<RepositoryMaterializationRequest> {
        Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
                canonical_repo: CanonicalRepoName::new(repo).unwrap(),
            },
            repo_spec: RepoSpec {
                rule_id: RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                    )
                    .unwrap(),
                    rule_name: rule_name.into(),
                },
                attributes: Arc::default(),
            },
            kind: RepositoryMaterializationKind::Immutable,
        })
    }

    fn batch(text: &str) -> EventBatch {
        EventBatch::from_events([EvaluationEvent::StarlarkPrint {
            location: slug_events_v2::StarlarkSourceLocation::new(Arc::from("synthetic.bzl"), 1, 6),
            text: CompactString::new(text),
        }])
    }

    fn demand_owner(dice: &Arc<Dice>) -> Arc<WorkspaceDemandOwner> {
        WorkspaceDemandOwner::new(dice, NormalizedAbsolutePath::new("/workspace").unwrap())
    }

    async fn selected(
        dice: &Arc<Dice>,
        owner: &Arc<WorkspaceDemandOwner>,
        root: &DemandTestKey,
    ) -> Result<SelectedWorkspaceDemands, CommandEffectError> {
        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt()?;
        let mut data = UserComputationData::default();
        owner.install(dice, &mut data, Some(attempt.clone()))?;
        let mut transaction = dice.updater_with_data(data).commit().await;
        transaction
            .compute(root)
            .await
            .expect("synthetic demand graph computes");
        let sealed = attempt.seal_terminal()?;
        Ok(sealed.select(&transaction).await?.demands().clone())
    }

    fn paths(selected: &SelectedWorkspaceDemands, module: &str) -> Vec<PathObservationDemand> {
        selected
            .repository_validations()
            .iter()
            .find(|validation| validation.scope().module_name == module)
            .expect("selected repository validation")
            .paths()
            .to_vec()
    }

    #[tokio::test]
    async fn catalogue_records_evaluated_and_reused_metadata_and_latches_conflicts()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let expected = path("/shared");
        let root = key(1, TestDescriptor::Path(expected.clone()), Vec::new());

        let evaluated_owner = demand_owner(&dice);
        assert_eq!(
            selected(&dice, &evaluated_owner, &root)
                .await?
                .unscoped_paths(),
            &[expected.clone()]
        );

        let reused_owner = demand_owner(&dice);
        assert_eq!(
            selected(&dice, &reused_owner, &root)
                .await?
                .unscoped_paths(),
            &[expected]
        );

        let node = reused_owner.only_catalogued_node();
        let conflicting =
            DynKey::from_key(key(1, TestDescriptor::Path(path("/other")), Vec::new()));
        reused_owner.record_key(node, &conflicting);
        assert_eq!(
            selected(&dice, &reused_owner, &root).await,
            Err(CommandEffectError::Demand(
                DemandProvenanceError::ConflictingNodeMetadata { node }
            ))
        );
        Ok(())
    }

    #[test]
    fn foreign_and_expired_demand_owner_installs_are_atomic_and_do_not_reserve_attempts() {
        let first_dice = Dice::builder().build(DetectCycles::Enabled);
        let second_dice = Dice::builder().build(DetectCycles::Enabled);
        let first_owner = demand_owner(&first_dice);
        let second_owner = demand_owner(&second_dice);

        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt().unwrap();
        let mut foreign_data = UserComputationData::default();
        assert_eq!(
            first_owner.install(&second_dice, &mut foreign_data, Some(attempt.clone())),
            Err(CommandEffectError::ForeignDemandOwner)
        );
        assert!(foreign_data.activation_tracker.is_none());
        assert!(foreign_data.data.get::<CaptureEvaluationEvents>().is_err());
        let mut recovered_data = UserComputationData::default();
        second_owner
            .install(&second_dice, &mut recovered_data, Some(attempt.clone()))
            .unwrap();
        assert!(recovered_data.activation_tracker.is_some());
        assert!(recovered_data.data.get::<CaptureEvaluationEvents>().is_ok());
        attempt.finish_suppressed().unwrap();

        let expired_owner = {
            let expired_dice = Dice::builder().build(DetectCycles::Enabled);
            demand_owner(&expired_dice)
        };
        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt().unwrap();
        let mut expired_data = UserComputationData::default();
        assert_eq!(
            expired_owner.install(&second_dice, &mut expired_data, Some(attempt.clone())),
            Err(CommandEffectError::ForeignDemandOwner)
        );
        assert!(expired_data.activation_tracker.is_none());
        assert!(expired_data.data.get::<CaptureEvaluationEvents>().is_err());
        let mut recovered_data = UserComputationData::default();
        second_owner
            .install(&second_dice, &mut recovered_data, Some(attempt.clone()))
            .unwrap();
        assert!(recovered_data.activation_tracker.is_some());
        assert!(recovered_data.data.get::<CaptureEvaluationEvents>().is_ok());
        attempt.finish_suppressed().unwrap();
    }

    #[tokio::test]
    async fn retained_owner_selects_untouched_and_late_demands_without_replaying_events()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let effects = CommandEffectOwner::new();
        let reachable_probes = Arc::new(AtomicUsize::new(0));
        let sibling_probes = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(TestGate {
            entered: Notify::new(),
            release: Notify::new(),
        });
        let reachable = instrumented_key(
            100,
            TestDescriptor::Path(path("/reachable")),
            Vec::new(),
            Some("late-reachable"),
            Some(reachable_probes.clone()),
            Some(gate.clone()),
        );
        let sibling = instrumented_key(
            101,
            TestDescriptor::Path(path("/unreachable")),
            Vec::new(),
            Some("unreachable"),
            Some(sibling_probes.clone()),
            None,
        );
        let parent = instrumented_key(
            102,
            TestDescriptor::None,
            vec![reachable],
            Some("late-parent"),
            None,
            None,
        );

        let abandoned = effects.begin_attempt()?;
        let mut abandoned_data = UserComputationData::default();
        owner.install(&dice, &mut abandoned_data, Some(abandoned.clone()))?;
        let mut abandoned_transaction = dice.updater_with_data(abandoned_data).commit().await;
        abandoned_transaction.compute(&sibling).await?;
        let pending = abandoned_transaction.compute(&parent);
        tokio::pin!(pending);
        tokio::select! {
            result = &mut pending => panic!("gated demand completed before retry seal: {result:?}"),
            () = gate.entered.notified() => {}
        }
        abandoned.seal_retry()?;
        gate.release.notify_one();
        pending.await?;
        assert_eq!(reachable_probes.load(Ordering::Relaxed), 1);
        assert_eq!(sibling_probes.load(Ordering::Relaxed), 1);

        let terminal = effects.begin_attempt()?;
        let mut terminal_data = UserComputationData::default();
        owner.install(&dice, &mut terminal_data, Some(terminal.clone()))?;
        let mut terminal_transaction = dice.updater_with_data(terminal_data).commit().await;
        terminal_transaction.compute(&parent).await?;
        assert_eq!(
            reachable_probes.load(Ordering::Relaxed),
            1,
            "untouched descendant must not need a later rich callback"
        );
        let sealed = terminal.seal_terminal()?;
        let selected = sealed.select(&terminal_transaction).await?;

        assert_eq!(selected.demands().unscoped_paths(), &[path("/reachable")]);
        let events = selected
            .events()
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                EvaluationEvent::Diagnostic { .. } => {
                    unreachable!("diagnostic events are not produced by this packet")
                }
            })
            .collect::<Vec<_>>();
        assert!(events.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn terminal_selection_uses_the_exact_owner_bound_at_install() -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let effects = CommandEffectOwner::new();
        let attempt = effects.begin_attempt()?;
        let mut data = UserComputationData::default();
        owner.install(&dice, &mut data, Some(attempt.clone()))?;
        let mut transaction = dice.updater_with_data(data).commit().await;
        transaction
            .compute(&key(
                200,
                TestDescriptor::Path(path("/selected")),
                Vec::new(),
            ))
            .await?;
        let sealed = attempt.seal_terminal()?;
        assert_eq!(
            sealed
                .select(&transaction)
                .await?
                .demands()
                .unscoped_paths(),
            &[path("/selected")]
        );
        Ok(())
    }

    #[tokio::test]
    async fn semantic_selector_tracks_nested_equal_shared_and_unscoped_reachability()
    -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let request_a = request("a+", "http_archive");
        let request_b = request("b+", "http_archive");
        let shared = key(1, TestDescriptor::Path(path("/shared")), Vec::new());
        let path_a = key(2, TestDescriptor::Path(path("/a")), Vec::new());
        let path_b = key(3, TestDescriptor::Path(path("/b")), Vec::new());
        let request_a_key = key(4, TestDescriptor::Request(request_a.clone()), Vec::new());
        let request_b_key = key(5, TestDescriptor::Request(request_b.clone()), Vec::new());
        let source_b = key(
            6,
            TestDescriptor::Scope(scope("b")),
            vec![request_b_key, path_b, shared.clone()],
        );
        let source_a = key(
            7,
            TestDescriptor::Scope(scope("a")),
            vec![request_a_key.clone(), path_a, shared.clone(), source_b],
        );
        let equal_source_a = key(
            8,
            TestDescriptor::Scope(scope("a")),
            vec![request_a_key, shared.clone()],
        );
        let request_pathless = request("pathless+", "http_archive");
        let pathless = key(
            9,
            TestDescriptor::Scope(scope("pathless")),
            vec![key(
                10,
                TestDescriptor::Request(request_pathless.clone()),
                Vec::new(),
            )],
        );
        let root = plain(11, vec![shared, source_a, equal_source_a, pathless]);

        let selected = selected(&dice, &owner, &root).await?;

        assert_eq!(
            selected.repository_requests(),
            &[
                request_a.clone(),
                request_b.clone(),
                request_pathless.clone()
            ]
        );
        assert_eq!(paths(&selected, "a"), [path("/a"), path("/shared")]);
        assert_eq!(paths(&selected, "b"), [path("/b"), path("/shared")]);
        assert_eq!(selected.unscoped_paths(), &[path("/shared")]);
        assert!(
            selected
                .repository_validations()
                .iter()
                .all(|validation| validation.scope().module_name != "pathless")
        );
        assert_eq!(
            selected
                .repository_validations()
                .iter()
                .find(|validation| validation.scope().module_name == "a")
                .unwrap()
                .request(),
            &request_a
        );
        Ok(())
    }

    #[tokio::test]
    async fn selector_rejects_request_identity_and_source_scope_conflicts() -> anyhow::Result<()> {
        let request_a = request("a+", "http_archive");
        let conflicting_a = request("a+", "git_repository");
        let request_b = request("b+", "http_archive");

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let root = plain(
            3,
            vec![
                key(1, TestDescriptor::Request(request_a.clone()), Vec::new()),
                key(
                    2,
                    TestDescriptor::Request(conflicting_a.clone()),
                    Vec::new(),
                ),
            ],
        );
        assert_eq!(
            selected(&dice, &owner, &root).await,
            Err(CommandEffectError::Demand(
                DemandProvenanceError::RepositoryRequestConflict {
                    id: request_a.id.clone()
                }
            ))
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let missing = key(
            2,
            TestDescriptor::Scope(scope("missing")),
            vec![key(1, TestDescriptor::Path(path("/missing")), Vec::new())],
        );
        assert!(matches!(
            selected(&dice, &owner, &missing).await,
            Err(CommandEffectError::Demand(
                DemandProvenanceError::SourceRequestMissing {
                    scope: actual_scope,
                    ..
                }
            )) if actual_scope == scope("missing")
        ));

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let conflicting = key(
            4,
            TestDescriptor::Scope(scope("conflicting")),
            vec![
                key(1, TestDescriptor::Request(request_a), Vec::new()),
                key(2, TestDescriptor::Request(request_b), Vec::new()),
                key(3, TestDescriptor::Path(path("/conflicting")), Vec::new()),
            ],
        );
        assert!(matches!(
            selected(&dice, &owner, &conflicting).await,
            Err(CommandEffectError::Demand(
                DemandProvenanceError::SourceRequestConflict {
                    scope: actual_scope,
                    ..
                }
            )) if actual_scope == scope("conflicting")
        ));
        Ok(())
    }

    #[tokio::test]
    async fn nested_equal_scope_anchor_cannot_borrow_its_outer_request() -> anyhow::Result<()> {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = demand_owner(&dice);
        let nested = key(
            3,
            TestDescriptor::Scope(scope("equal")),
            vec![key(2, TestDescriptor::Path(path("/nested")), Vec::new())],
        );
        let outer = key(
            5,
            TestDescriptor::Scope(scope("equal")),
            vec![
                key(
                    1,
                    TestDescriptor::Request(request("equal+", "http_archive")),
                    Vec::new(),
                ),
                key(4, TestDescriptor::Path(path("/outer")), Vec::new()),
                nested,
            ],
        );

        assert!(matches!(
            selected(&dice, &owner, &outer).await,
            Err(CommandEffectError::Demand(
                DemandProvenanceError::SourceRequestMissing {
                    scope: actual_scope,
                    ..
                }
            )) if actual_scope == scope("equal")
        ));
        Ok(())
    }
}
