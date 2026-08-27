/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Loading-query evaluation backed by the retained DICE transaction.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dupe::Dupe;
use regex::Regex;
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::HostRootPackageBoundaryObservationKey;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_bzlmod_v2::RootRepositoryRouteObservationKey;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::RepositoryPackageLoadKey;
use slug_loading_v2::RepositoryPackageLoadObservationKey;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::RootPackageLoadObservationKey;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::TestRuleKind;
use slug_loading_v2::discover_build_file_companion;
use slug_loading_v2::keys::PackageLoadKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathKey;
use slug_workspace_v2::ResolvedPathObservationKey;
use slug_workspace_v2::ResolvedPathState;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::QueryPolicy;
use crate::evaluator::QueryOutputCompletion;
use crate::generic::QueryEnvironment;
use crate::generic::TargetSet;
use crate::generic::TestSuiteAttribute;
use crate::generic::TestTargetInfo;
use crate::generic::TestTargetKind;
use crate::graph::ExternalUnconfiguredPackageGraphKey;
use crate::graph::ExternalUnconfiguredPackageGraphObservationKey;
use crate::graph::QueryError;
use crate::graph::QueryLabel;
use crate::graph::QueryNode;
use crate::graph::QueryObservationMode;
use crate::graph::RootSubtreePackageSetKey;
use crate::graph::RootSubtreePackageSetObservationKey;
use crate::graph::RootUnconfiguredPackageGraphKey;
use crate::graph::RootUnconfiguredPackageGraphObservationKey;
use crate::graph::SubtreePackageSetKey;
use crate::graph::UnconfiguredPackageGraph;
use crate::graph::UnconfiguredPackageGraphKey;
use crate::output::SelectedQueryGraph;
use crate::output::SelectedQueryGraphNode;
use crate::provenance::QueryCandidate;
use crate::provenance::QueryCandidateArena;
use crate::provenance::QueryCandidateBatches;
use crate::provenance::QueryCandidateId;
use crate::provenance::QueryPackageIdentity;
use crate::traversal::ResolvedGraph;

fn map_external_load_error(error: &slug_loading_v2::RepositoryPackageLoadError) -> QueryError {
    if error.is_unsupported_feature() {
        QueryError::unsupported_feature(error.to_string())
    } else {
        QueryError::evaluation(error.to_string())
    }
}

pub(crate) struct LoadingQueryEnvironment<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    workspace: PathBuf,
    root_workspace: Option<NormalizedAbsolutePath>,
    preparation_needs: Option<LoadingPreparationNeeds>,
    observation_outer: Option<ObservedPathFrontierError>,
    observations: PathObservationEpoch,
    observation_mode: QueryObservationMode,
    policy: QueryPolicy,
    evaluation_graph: ResolvedGraph<QueryLabel>,
    node_kinds: SmallMap<QueryLabel, CompactString>,
    generated_file_labels: SmallSet<QueryLabel>,
    pub(crate) candidates: QueryCandidateArena,
}

impl<'a, 'd> LoadingQueryEnvironment<'a, 'd> {
    pub(crate) fn new(
        ctx: &'a mut DiceComputations<'d>,
        workspace: PathBuf,
        policy: QueryPolicy,
    ) -> Self {
        Self {
            ctx,
            workspace,
            root_workspace: None,
            preparation_needs: None,
            observation_outer: None,
            observations: PathObservationEpoch::empty(),
            observation_mode: QueryObservationMode::Legacy,
            policy,
            evaluation_graph: ResolvedGraph::new(),
            node_kinds: SmallMap::new(),
            generated_file_labels: SmallSet::new(),
            candidates: QueryCandidateArena::new(),
        }
    }

    pub(crate) fn new_root(
        ctx: &'a mut DiceComputations<'d>,
        workspace: NormalizedAbsolutePath,
        policy: QueryPolicy,
    ) -> Self {
        Self {
            ctx,
            workspace: workspace.as_path().to_path_buf(),
            root_workspace: Some(workspace),
            preparation_needs: None,
            observation_outer: None,
            observations: PathObservationEpoch::empty(),
            observation_mode: QueryObservationMode::Legacy,
            policy,
            evaluation_graph: ResolvedGraph::new(),
            node_kinds: SmallMap::new(),
            generated_file_labels: SmallSet::new(),
            candidates: QueryCandidateArena::new(),
        }
    }

    pub(crate) fn new_root_observed(
        ctx: &'a mut DiceComputations<'d>,
        workspace: NormalizedAbsolutePath,
        policy: QueryPolicy,
        observations: PathObservationEpoch,
    ) -> Self {
        let mut environment = Self::new_root(ctx, workspace, policy);
        environment.observation_mode = QueryObservationMode::Observed;
        environment.observations = observations;
        environment
    }

    pub(crate) fn take_preparation_needs(&mut self) -> Option<LoadingPreparationNeeds> {
        self.preparation_needs.take()
    }

    pub(crate) fn take_observation_outer(&mut self) -> Option<ObservedPathFrontierError> {
        self.observation_outer.take()
    }

    pub(crate) fn take_observations(&mut self) -> PathObservationEpoch {
        std::mem::replace(&mut self.observations, PathObservationEpoch::empty())
    }

    fn merge_observations(&mut self, incoming: &PathObservationEpoch) -> Result<(), QueryError> {
        self.observations = PathObservationEpoch::from_shared(
            self.observations
                .observations()
                .iter()
                .chain(incoming.observations())
                .map(|(demand, result)| (demand.dupe(), result.dupe())),
        )
        .map_err(|error| self.observation_restart(error.into()))?;
        Ok(())
    }

    fn observation_restart(&mut self, error: ObservedPathFrontierError) -> QueryError {
        self.observation_outer.get_or_insert(error);
        QueryError::observation_restart()
    }

    fn preparation_restart(&mut self, need: LoadingPreparationNeeds) -> QueryError {
        self.preparation_needs = Some(match self.preparation_needs.take() {
            Some(existing) => existing
                .try_union(&need)
                .expect("query preparation Needs must be compatible"),
            None => need,
        });
        QueryError::preparation_restart()
    }

    async fn package_graph(
        &mut self,
        package: &str,
    ) -> Result<Arc<UnconfiguredPackageGraph>, QueryError> {
        if let Some(workspace) = self.root_workspace.clone() {
            let package = PackagePath::parse(package).map_err(QueryError::evaluation)?;
            return match self.observation_mode {
                QueryObservationMode::Legacy => match self
                    .ctx
                    .compute(&RootUnconfiguredPackageGraphKey::new(workspace, package))
                    .await
                    .expect("root query graph DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                    LoadingPreparationOutcome::Complete(value) => value.as_ref().clone(),
                },
                QueryObservationMode::Observed => match self
                    .ctx
                    .compute(&RootUnconfiguredPackageGraphObservationKey::new(
                        workspace, package,
                    ))
                    .await
                    .expect("observed root query graph DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        Err(self.observation_restart(error))
                    }
                    LoadingPreparationOutcome::Complete(Ok(value)) => {
                        self.merge_observations(value.observations())?;
                        value.result().as_ref().clone()
                    }
                },
            };
        }
        self.ctx
            .compute(&UnconfiguredPackageGraphKey {
                workspace: self.workspace.clone(),
                package: PathBuf::from(package),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?
            .as_ref()
            .clone()
    }

    async fn repository_route(
        &mut self,
        apparent_repo: &slug_identity_v2::ApparentRepoName,
    ) -> Result<RootRepositoryRoute, QueryError> {
        let workspace = self.root_workspace.clone().ok_or_else(|| {
            QueryError::evaluation("external repository query requires Host mode")
        })?;
        let key = RootRepositoryRouteKey::new(workspace.clone(), apparent_repo.clone())
            .map_err(QueryError::evaluation)?;
        match self.observation_mode {
            QueryObservationMode::Legacy => match self
                .ctx
                .compute(&key)
                .await
                .expect("root repository route DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(value) => value
                    .as_ref()
                    .as_ref()
                    .cloned()
                    .map_err(|error| QueryError::evaluation(error.to_string())),
            },
            QueryObservationMode::Observed => match self
                .ctx
                .compute(
                    &RootRepositoryRouteObservationKey::new(workspace, apparent_repo.clone())
                        .map_err(QueryError::evaluation)?,
                )
                .await
                .expect("observed root repository route DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    Err(self.observation_restart(error.ordinary_path()))
                }
                LoadingPreparationOutcome::Complete(Ok(value)) => {
                    self.merge_observations(value.observations())?;
                    match value.result().as_ref() {
                        Ok(route) => Ok(route.clone()),
                        Err(error) => Err(QueryError::evaluation(error.to_string())),
                    }
                }
            },
        }
    }

    async fn verified_repository_route(
        &mut self,
        owner: &QueryPackageIdentity,
    ) -> Result<RootRepositoryRoute, QueryError> {
        let apparent_repo = owner.apparent_repo().ok_or_else(|| {
            QueryError::evaluation("external package owner lost its apparent repository route")
        })?;
        let canonical_repo = owner.canonical_repo().ok_or_else(|| {
            QueryError::evaluation("external package owner lost its canonical repository")
        })?;
        let route = self.repository_route(apparent_repo).await?;
        if route.canonical_repo() != canonical_repo {
            return Err(QueryError::evaluation(format!(
                "apparent repository '{}' now resolves to '{}' instead of retained '{}'",
                apparent_repo,
                route.canonical_repo(),
                canonical_repo
            )));
        }
        Ok(route)
    }

    async fn package_graph_for_owner(
        &mut self,
        owner: &QueryPackageIdentity,
    ) -> Result<Arc<UnconfiguredPackageGraph>, QueryError> {
        if owner.canonical_repo().is_none() {
            return self.package_graph(owner.package().as_str()).await;
        }
        let route = self.verified_repository_route(owner).await?;
        match self.observation_mode {
            QueryObservationMode::Legacy => match self
                .ctx
                .compute(&ExternalUnconfiguredPackageGraphKey::new(
                    route,
                    owner.package().clone(),
                ))
                .await
                .expect("external query graph DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(value) => value.as_ref().clone(),
            },
            QueryObservationMode::Observed => match self
                .ctx
                .compute(&ExternalUnconfiguredPackageGraphObservationKey::new(
                    route,
                    owner.package().clone(),
                ))
                .await
                .expect("observed external query graph DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    Err(self.observation_restart(error))
                }
                LoadingPreparationOutcome::Complete(Ok(value)) => {
                    self.merge_observations(value.observations())?;
                    value.result().as_ref().clone()
                }
            },
        }
    }

    async fn lookup_single(&mut self, label: QueryLabel) -> Result<QueryNode, QueryError> {
        let owner = label.owner_identity()?;
        let graph = self
            .package_graph_for_owner(&owner)
            .await
            .map_err(|error| {
                if label.is_root_repository() {
                    if error.is_preparation_restart() {
                        return error;
                    }
                    if error.message.contains("no BUILD.bazel or BUILD file")
                        || error.message.contains("package directory is absent")
                    {
                        error.with_message(format!(
                            "no such package '{}': BUILD file not found",
                            label.package()
                        ))
                    } else {
                        error
                    }
                } else {
                    error
                }
            })?;
        let node = graph.nodes.get(&label).cloned().ok_or_else(|| {
            if label.is_root_repository() {
                QueryError::target_missing(format!(
                    "no such target '{}': target '{}' not declared in package '{}'",
                    label,
                    label.target(),
                    label.package()
                ))
            } else {
                let build_file = graph
                    .nodes
                    .values()
                    .next()
                    .map(|node| node.build_file.as_str())
                    .unwrap_or("<output_base>/external/unknown/BUILD.bazel");
                QueryError::target_missing(format!(
                    "no such target '{}': target '{}' not declared in package '{}' defined by {}",
                    label,
                    label.target(),
                    label.package(),
                    build_file
                ))
            }
        })?;
        Ok(node)
    }

    async fn package_load_provenance(
        &mut self,
        package: &str,
    ) -> Result<(PathBuf, Arc<[slug_loading_v2::BzlModuleIdentity]>), QueryError> {
        if let Some(workspace) = self.root_workspace.clone() {
            let package = PackagePath::parse(package).map_err(QueryError::evaluation)?;
            return match self.observation_mode {
                QueryObservationMode::Legacy => match self
                    .ctx
                    .compute(&RootPackageLoadKey::new(workspace, package))
                    .await
                    .expect("root package loading DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                    LoadingPreparationOutcome::Complete(value) => value
                        .as_ref()
                        .as_ref()
                        .map(|loaded| (loaded.build_file.clone(), loaded.reachable_loads.clone()))
                        .map_err(|error| QueryError::evaluation(error.to_string())),
                },
                QueryObservationMode::Observed => match self
                    .ctx
                    .compute(&RootPackageLoadObservationKey::new(workspace, package))
                    .await
                    .expect("observed root package loading DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        Err(self.observation_restart(error))
                    }
                    LoadingPreparationOutcome::Complete(Ok(value)) => {
                        self.merge_observations(value.observations())?;
                        match value.result().as_ref() {
                            Ok(loaded) => {
                                Ok((loaded.build_file.clone(), loaded.reachable_loads.clone()))
                            }
                            Err(error) => Err(QueryError::evaluation(error.to_string())),
                        }
                    }
                },
            };
        }
        let value = self
            .ctx
            .compute(&PackageLoadKey {
                workspace: self.workspace.clone(),
                package: self.workspace.join(package),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        value
            .as_ref()
            .as_ref()
            .map(|loaded| (loaded.build_file.clone(), loaded.reachable_loads.clone()))
            .map_err(|error| QueryError::evaluation(error.to_string()))
    }

    async fn package_load_provenance_for_owner(
        &mut self,
        owner: &QueryPackageIdentity,
    ) -> Result<(PathBuf, Arc<[slug_loading_v2::BzlModuleIdentity]>), QueryError> {
        if owner.canonical_repo().is_none() {
            return self.package_load_provenance(owner.package().as_str()).await;
        }
        let route = self.verified_repository_route(owner).await?;
        match self.observation_mode {
            QueryObservationMode::Legacy => match self
                .ctx
                .compute(&RepositoryPackageLoadKey::new(
                    route,
                    owner.package().clone(),
                ))
                .await
                .expect("external package loading DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(value) => {
                    let loaded = value.as_ref().as_ref().map_err(map_external_load_error)?;
                    Ok((loaded.build_file.clone(), loaded.reachable_loads.clone()))
                }
            },
            QueryObservationMode::Observed => match self
                .ctx
                .compute(&RepositoryPackageLoadObservationKey::new(
                    route,
                    owner.package().clone(),
                ))
                .await
                .expect("observed external package loading DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(Err(error)) => {
                    Err(self.observation_restart(error))
                }
                LoadingPreparationOutcome::Complete(Ok(value)) => {
                    self.merge_observations(value.observations())?;
                    let loaded = match value.result().as_ref() {
                        Ok(loaded) => loaded,
                        Err(error) => return Err(map_external_load_error(error)),
                    };
                    Ok((loaded.build_file.clone(), loaded.reachable_loads.clone()))
                }
            },
        }
    }

    async fn resolve_single(&mut self, label: QueryLabel) -> Result<QueryNode, QueryError> {
        let node = self.lookup_single(label.clone()).await?;
        if matches!(node.kind, crate::QueryNodeKind::GeneratedFile) {
            self.generated_file_labels.insert(label.dupe());
        }
        self.record_node(&node);
        Ok(node)
    }

    fn record_node(&mut self, node: &QueryNode) {
        self.node_kinds
            .insert(node.label.clone(), selected_node_kind(node));
        self.evaluation_graph.record_node(node.label.clone());
    }

    fn same_package_or_java(owner: &QueryPackageIdentity, target: &QueryLabel) -> bool {
        let caller_package = owner.package().as_str();
        caller_package == target.package()
            || matches!(
                (
                    caller_package.strip_prefix("javatests/"),
                    target.package().strip_prefix("java/")
                ),
                (Some(caller_suffix), Some(target_suffix)) if caller_suffix == target_suffix
            )
    }

    async fn visible_in_package_group(
        &mut self,
        top_level: &QueryLabel,
        caller_package: &slug_identity_v2::PackageIdentifier,
    ) -> Result<bool, QueryError> {
        enum WorkItem {
            Enter(QueryLabel),
            Contents(Arc<slug_loading_v2::PackageGroupContents>),
        }

        let mut seen = SmallSet::new();
        let mut work = vec![WorkItem::Enter(top_level.clone())];
        let mut matches = false;
        while let Some(item) = work.pop() {
            match item {
                WorkItem::Contents(contents) => {
                    matches |= contents.contains_package(caller_package);
                }
                WorkItem::Enter(label) => {
                    if !seen.insert(label.clone()) {
                        continue;
                    }
                    let node = self.lookup_single(label).await.map_err(|error| {
                        if error.is_preparation_restart() {
                            return error;
                        }
                        let message = error.to_string();
                        error.with_message(format!(
                            "Invalid visibility label '{}': {message}",
                            top_level
                        ))
                    })?;
                    if !matches!(node.kind, crate::QueryNodeKind::PackageGroup) {
                        continue;
                    }
                    let Some(contents) = node.package_group_contents else {
                        continue;
                    };
                    let includes = node
                        .edges
                        .iter()
                        .filter(|edge| {
                            matches!(edge.kind, crate::QueryEdgeKind::PackageGroupInclude)
                        })
                        .map(|edge| edge.target.clone())
                        .collect::<Vec<_>>();
                    // LIFO work uses reverse-pushed includes. This evaluates
                    // source-order includes before this group's local contents.
                    work.push(WorkItem::Contents(contents));
                    for include in includes.into_iter().rev() {
                        work.push(WorkItem::Enter(include));
                    }
                }
            }
        }
        Ok(matches)
    }

    async fn visible_to(
        &mut self,
        caller: QueryCandidateId,
        target: QueryCandidateId,
    ) -> Result<bool, QueryError> {
        let caller_owner = self.candidates.get(caller).owner_identity()?;
        let target = self.candidates.get(target).clone();
        let Some(label) = target.evaluation_graph_label().cloned() else {
            return Ok(true);
        };
        let node = self.lookup_single(label).await?;
        let same_package_or_java = Self::same_package_or_java(&caller_owner, &node.label);
        match &node.effective_visibility {
            RuleVisibility::Public => Ok(true),
            RuleVisibility::Private => Ok(same_package_or_java),
            RuleVisibility::Restricted(restricted) => {
                let caller_package = caller_owner.canonical_package();
                let target_owner = node.label.owner_identity()?;
                let mut visible = restricted
                    .direct_packages()
                    .contains_package(&caller_package);
                // Do not short-circuit: Bazel resolves every top-level root so
                // a positive alternative cannot mask a later missing group.
                for group in restricted.package_groups() {
                    let group = if target_owner.canonical_repo().is_some() {
                        QueryLabel::from_canonical_in_owner(group, &target_owner)?
                    } else {
                        QueryLabel::from_canonical(group.clone())
                    };
                    visible |= self
                        .visible_in_package_group(&group, &caller_package)
                        .await?;
                }
                Ok(same_package_or_java || visible)
            }
        }
    }

    fn record_pattern_graph(
        &mut self,
        graph: &UnconfiguredPackageGraph,
        selected: &TargetSet<QueryLabel>,
    ) {
        for node in graph.nodes.values() {
            if !selected.contains(&node.label) {
                continue;
            }
            self.record_node(node);
            for edge in node
                .edges
                .iter()
                .filter(|edge| selected.contains(&edge.target))
            {
                self.evaluation_graph
                    .record_edge(node.label.clone(), edge.target.clone());
            }
        }
    }

    async fn resolve_recursive(
        &mut self,
        prefix: &str,
    ) -> Result<TargetSet<QueryLabel>, QueryError> {
        let packages = if let Some(workspace) = self.root_workspace.clone() {
            let prefix = PackagePath::parse(prefix).map_err(QueryError::evaluation)?;
            match self.observation_mode {
                QueryObservationMode::Legacy => match self
                    .ctx
                    .compute(&RootSubtreePackageSetKey::new(workspace, prefix))
                    .await
                    .expect("root subtree package-set DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => {
                        return Err(self.preparation_restart(need));
                    }
                    LoadingPreparationOutcome::Complete(packages) => packages,
                },
                QueryObservationMode::Observed => match self
                    .ctx
                    .compute(&RootSubtreePackageSetObservationKey::new(workspace, prefix))
                    .await
                    .expect("observed root subtree package-set DICE invariant")
                {
                    LoadingPreparationOutcome::Need(need) => {
                        return Err(self.preparation_restart(need));
                    }
                    LoadingPreparationOutcome::Complete(Err(error)) => {
                        return Err(self.observation_restart(error));
                    }
                    LoadingPreparationOutcome::Complete(Ok(packages)) => {
                        self.merge_observations(packages.observations())?;
                        packages.result().dupe()
                    }
                },
            }
        } else {
            self.ctx
                .compute(&SubtreePackageSetKey {
                    workspace: self.workspace.clone(),
                    prefix: PathBuf::from(prefix),
                })
                .await
                .map_err(|error| QueryError::evaluation(error.to_string()))?
        };
        let packages = packages.as_ref().as_ref().map_err(|error| error.clone())?;
        let mut result = TargetSet::default();
        let mut graphs = Vec::with_capacity(packages.packages.len());
        for package in packages.packages.iter() {
            let graph = self.package_graph(package).await?;
            for (label, node) in graph.nodes.iter() {
                if node.kind.is_rule() {
                    result.insert(label.clone());
                }
            }
            graphs.push(graph);
        }
        if result.iter().next().is_none() {
            return Err(QueryError::evaluation(format!(
                "no targets found beneath '{prefix}'"
            )));
        }
        for graph in graphs {
            self.record_pattern_graph(&graph, &result);
        }
        Ok(result)
    }

    async fn root_build_file_companion(
        &mut self,
        package: &PackagePath,
    ) -> Result<Option<QueryLabel>, QueryError> {
        let workspace = self
            .root_workspace
            .clone()
            .expect("root companion lookup requires root query mode");
        let selected_root = match self.observation_mode {
            QueryObservationMode::Legacy => match self
                .ctx
                .compute(&HostRootPackageBoundaryKey::new(workspace, package.clone()))
                .await
                .expect("Host package-boundary DICE invariant")
            {
                PathOutcome::Need(need) => {
                    return Err(self.preparation_restart(LoadingPreparationNeeds::path(need)));
                }
                PathOutcome::Complete(value) => match value.as_ref() {
                    Err(error) => return Err(QueryError::evaluation(error.to_string())),
                    Ok(boundary) if boundary.kind() != HostRootPackageBoundaryKind::Package => {
                        return Ok(None);
                    }
                    Ok(boundary) => boundary
                        .selected_package_root()
                        .expect("Package boundary retains its selected root")
                        .clone(),
                },
            },
            QueryObservationMode::Observed => match self
                .ctx
                .compute(&HostRootPackageBoundaryObservationKey::new(
                    workspace,
                    package.clone(),
                ))
                .await
                .expect("observed Host package-boundary DICE invariant")
            {
                PathOutcome::Need(need) => {
                    return Err(self.preparation_restart(LoadingPreparationNeeds::path(need)));
                }
                PathOutcome::Complete(Err(error)) => {
                    return Err(self.observation_restart(error));
                }
                PathOutcome::Complete(Ok(value)) => {
                    self.merge_observations(value.observations())?;
                    match value.result() {
                        Err(error) => return Err(QueryError::evaluation(error.to_string())),
                        Ok(boundary) if boundary.kind() != HostRootPackageBoundaryKind::Package => {
                            return Ok(None);
                        }
                        Ok(boundary) => boundary
                            .selected_package_root()
                            .expect("Package boundary retains its selected root")
                            .clone(),
                    }
                }
            },
        };
        let mut selected = None;
        for basename in ["BUILD.bazel", "BUILD"] {
            let marker = NormalizedAbsolutePath::new(
                selected_root
                    .as_path()
                    .join(package.as_str())
                    .join(basename),
            )
            .expect("selected BUILD marker remains absolute");
            let state = match self.observation_mode {
                QueryObservationMode::Legacy => match self
                    .ctx
                    .compute(&ResolvedPathKey::new(
                        PathObservationNamespace::Host,
                        marker,
                    ))
                    .await
                    .expect("resolved BUILD marker DICE invariant")
                {
                    PathOutcome::Need(need) => {
                        return Err(self.preparation_restart(LoadingPreparationNeeds::path(need)));
                    }
                    PathOutcome::Complete(result) => result
                        .as_ref()
                        .as_ref()
                        .map(|resolved| resolved.state().clone())
                        .map_err(|error| QueryError::evaluation(format!("{error:?}")))?,
                },
                QueryObservationMode::Observed => match self
                    .ctx
                    .compute(&ResolvedPathObservationKey::new(
                        PathObservationNamespace::Host,
                        marker,
                    ))
                    .await
                    .expect("observed resolved BUILD marker DICE invariant")
                {
                    PathOutcome::Need(need) => {
                        return Err(self.preparation_restart(LoadingPreparationNeeds::path(need)));
                    }
                    PathOutcome::Complete(Err(error)) => {
                        return Err(self.observation_restart(error));
                    }
                    PathOutcome::Complete(Ok(value)) => {
                        self.merge_observations(value.observations())?;
                        value
                            .result()
                            .as_ref()
                            .map(|resolved| resolved.state().clone())
                            .map_err(|error| QueryError::evaluation(format!("{error:?}")))?
                    }
                },
            };
            if matches!(
                state,
                ResolvedPathState::Present(lstat)
                    if matches!(
                        lstat.kind(),
                        PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                    )
            ) {
                selected = Some(basename);
                break;
            }
        }
        let basename = selected.expect("Package boundary selected one BUILD marker");
        Ok(Some(QueryLabel::from_canonical(
            CanonicalLabel::parse(&format!("@@//{}:{basename}", package.as_str()))
                .expect("typed package and BUILD basename form a canonical label"),
        )))
    }

    fn real_delivery(
        &mut self,
        labels: impl IntoIterator<Item = QueryLabel>,
    ) -> QueryCandidateBatches {
        QueryCandidateBatches::from_delivery(
            &mut self.candidates,
            labels.into_iter().map(QueryCandidate::real),
        )
    }

    pub(crate) fn selected_graph(
        &self,
        targets: &QueryCandidateBatches,
        completion: QueryOutputCompletion,
    ) -> SelectedQueryGraph {
        let materialized = targets.materialized_by_label(&self.candidates);
        let mut included = SmallMap::<QueryLabel, bool>::new();
        for (label, id) in materialized {
            let candidate = self.candidates.get(id);
            let real = candidate.evaluation_graph_label().is_some();
            if !real
                || self.evaluation_graph.contains(&label)
                || completion == QueryOutputCompletion::LabelKind
            {
                included.insert(label, real);
            }
        }

        let has_selected_generated_files = included
            .keys()
            .any(|label| self.generated_file_labels.contains(label));
        let mut selected = included.keys().cloned().collect::<Vec<_>>();
        if !has_selected_generated_files {
            // Preserve the established ordinary-query graph order. Generated
            // outputs retain callback/materialization order because sorting
            // here would erase producer order before Bazel's reverse visitor.
            selected.sort_unstable();
        }
        let mut target_to_index = SmallMap::with_capacity(selected.len());
        let mut nodes = Vec::with_capacity(selected.len());
        let mut generated_file_labels = SmallSet::new();
        for label in selected {
            let index: u32 = nodes
                .len()
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            target_to_index.insert(label.clone(), index);
            if self.generated_file_labels.contains(&label) {
                generated_file_labels.insert(label.output_label());
            }
            let kind = if included.get(&label).copied() == Some(true) {
                self.node_kinds.get(&label).cloned()
            } else {
                // Load/build provenance candidates have no loadable query
                // node, but Bazel reports them as input files.
                Some(CompactString::const_new("source file"))
            };
            nodes.push(SelectedQueryGraphNode {
                label: label.output_label(),
                kind,
                successors: Vec::new(),
            });
        }
        for (label, real) in &included {
            if !real {
                continue;
            }
            let Some(index) = self.evaluation_graph.target_to_index.get(label).copied() else {
                continue;
            };
            for child in self.evaluation_graph.nodes[index as usize]
                .children
                .iter()
                .copied()
            {
                let child = &self.evaluation_graph.nodes[child as usize].target;
                if included.get(child).copied() == Some(true)
                    && let (Some(from), Some(to)) =
                        (target_to_index.get(label), target_to_index.get(child))
                {
                    let successors = &mut nodes[*from as usize].successors;
                    if !successors.contains(to) {
                        successors.push(*to);
                    }
                }
            }
        }
        for node in &mut nodes {
            node.successors.sort_unstable();
        }
        SelectedQueryGraph {
            nodes,
            generated_file_labels,
        }
    }

    pub(crate) async fn complete_label_kinds(
        &mut self,
        targets: &QueryCandidateBatches,
    ) -> Result<(), QueryError> {
        for (label, id) in targets.materialized_by_label(&self.candidates) {
            if self.candidates.get(id).evaluation_graph_label().is_none()
                || self.node_kinds.contains_key(&label)
            {
                continue;
            }
            let node = self.lookup_single(label).await?;
            self.node_kinds
                .insert(node.label.clone(), selected_node_kind(&node));
        }
        Ok(())
    }

    // Text FULL is an existing public ordering contract. Keep its reverse
    // postorder implementation separate from graph rendering, whose Bazel
    // formatter uses its own graph visitor order.
    pub(crate) fn selected_full_order(&self, targets: &QueryCandidateBatches) -> Vec<QueryLabel> {
        let materialized = targets.materialized_by_label(&self.candidates);
        let mut included = SmallMap::<QueryLabel, bool>::new();
        for (label, id) in materialized {
            let candidate = self.candidates.get(id);
            let real = candidate.evaluation_graph_label().is_some();
            if !real || self.evaluation_graph.contains(&label) {
                included.insert(label, real);
            }
        }

        let mut labels = included.keys().cloned().collect::<Vec<_>>();
        labels.sort_unstable();
        let mut renderer = ResolvedGraph::new();
        for label in labels {
            renderer.record_node(label);
        }
        for (label, real) in &included {
            if !real {
                continue;
            }
            let Some(index) = self.evaluation_graph.target_to_index.get(label).copied() else {
                continue;
            };
            for child in self.evaluation_graph.nodes[index as usize]
                .children
                .iter()
                .copied()
            {
                let child = &self.evaluation_graph.nodes[child as usize].target;
                if included.get(child).copied() == Some(true) {
                    renderer.record_edge(label.clone(), child.clone());
                }
            }
        }
        renderer.deterministic_topological_order()
    }
}

fn selected_node_kind(node: &QueryNode) -> CompactString {
    match &node.kind {
        crate::QueryNodeKind::BuildFile | crate::QueryNodeKind::SourceFile => {
            CompactString::const_new("source file")
        }
        crate::QueryNodeKind::GeneratedFile => CompactString::const_new("generated file"),
        crate::QueryNodeKind::PackageGroup => CompactString::const_new("package group"),
        crate::QueryNodeKind::Rule(_) => {
            let capability = node
                .rule_capability
                .as_ref()
                .expect("query rule node must retain a RuleCapability");
            CompactString::new(format!("{} rule", capability.rule_class))
        }
    }
}

fn attr_matches_node(
    node: &QueryNode,
    attribute_name: &str,
    regex: &Regex,
) -> Result<bool, QueryError> {
    if !node.kind.is_rule() {
        return Ok(false);
    }
    let Some(attribute) = node
        .attributes
        .iter()
        .find(|attribute| attribute.name == attribute_name)
    else {
        return Ok(false);
    };
    let Some(value) = attribute.value.as_ref() else {
        return Ok(false);
    };
    let candidates = value
        .attr_visible_candidates(|label| node.label.output_attribute_label(label))
        .map_err(|error| {
            QueryError::evaluation(format!(
                "in '{}' of rule {}: {error}",
                attribute.name, node.label
            ))
        })?;
    Ok(candidates
        .iter()
        .any(|candidate| regex.find(candidate.as_str()).is_some()))
}

#[async_trait]
impl QueryEnvironment for LoadingQueryEnvironment<'_, '_> {
    type Target = QueryCandidateId;
    type Set = QueryCandidateBatches;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
        let mut seen = SmallSet::new();
        let mut ids = Vec::new();
        for set in sets {
            for (label, id) in set.materialized_by_label(&self.candidates) {
                if seen.insert(label) {
                    ids.push(id);
                }
            }
        }
        QueryCandidateBatches::from_materialized_ids(ids)
    }

    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
        left.union(right)
    }

    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        left.intersection(&self.candidates, right)
    }

    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        left.except(&self.candidates, right)
    }

    fn eval_all(&self, set: &Self::Set) -> TargetSet<Self::Target> {
        let mut result = TargetSet::default();
        if let Some(batch) = set.eval_all(&self.candidates) {
            for id in batch.ids() {
                result.insert(*id);
            }
        }
        result
    }

    fn lift_one_delivery(&self, targets: TargetSet<Self::Target>) -> Self::Set {
        QueryCandidateBatches::from_materialized_ids(targets.iter().copied().collect())
    }

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
        if literal == "//..." {
            let labels = self.resolve_recursive("").await?;
            return Ok(self.real_delivery(labels.iter().cloned()));
        }
        if literal.starts_with('@') && !literal.starts_with("@@") && literal.ends_with("//...") {
            return Err(QueryError::evaluation(format!(
                "external repository query patterns are deferred: {literal}"
            )));
        }
        let pattern = TargetPattern::parse(literal).map_err(QueryError::evaluation)?;
        match pattern {
            TargetPattern::Single(label) => {
                if !label.repo().is_root() && label.target().as_str() == "*" {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                let label = if label.repo().is_root() {
                    QueryLabel::parse_root(&label.to_string())?
                } else {
                    let route = self.repository_route(label.repo()).await?;
                    QueryLabel::from_apparent_route(&label, route.canonical_repo())?
                };
                self.resolve_single(label.clone()).await?;
                Ok(self.real_delivery([label]))
            }
            TargetPattern::PackageWildcard {
                repo,
                package,
                wildcard,
            } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                if !wildcard.rules_only() {
                    return Err(QueryError::evaluation(format!(
                        "all-target package patterns are deferred: {literal}"
                    )));
                }
                let graph = self.package_graph(package.as_str()).await?;
                let mut result = TargetSet::default();
                for (label, node) in graph.nodes.iter() {
                    if node.kind.is_rule() {
                        result.insert(label.clone());
                    }
                }
                self.record_pattern_graph(&graph, &result);
                Ok(self.real_delivery(result.iter().cloned()))
            }
            TargetPattern::Recursive {
                repo,
                package,
                wildcard,
            } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                if wildcard.is_some_and(|wildcard| !wildcard.rules_only()) {
                    return Err(QueryError::evaluation(format!(
                        "all-target recursive patterns are deferred: {literal}"
                    )));
                }
                let labels = self.resolve_recursive(package.as_str()).await?;
                Ok(self.real_delivery(labels.iter().cloned()))
            }
        }
    }

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError> {
        let candidate = self.candidates.get(*target).clone();
        let Some(label) = candidate.evaluation_graph_label().cloned() else {
            return Ok(Arc::from([]));
        };
        let node = self.resolve_single(label.clone()).await?;
        let mut dependencies = Vec::with_capacity(node.edges.len());
        for edge in node.edges.iter() {
            self.evaluation_graph
                .record_edge(label.clone(), edge.target.clone());
            dependencies.push(
                self.candidates
                    .intern(QueryCandidate::real(edge.target.clone())),
            );
        }
        Ok(dependencies.into())
    }

    async fn same_pkg_direct_rdeps(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError> {
        let mut by_package = SmallMap::<QueryPackageIdentity, SmallSet<QueryLabel>>::new();
        for target in targets.iter() {
            let Some(target) = self
                .candidates
                .get(*target)
                .evaluation_graph_label()
                .cloned()
            else {
                continue;
            };
            let owner = target.owner_identity()?;
            by_package.entry(owner).or_default().insert(target);
        }

        let mut result = TargetSet::default();
        for (package, package_targets) in by_package {
            let graph = self.package_graph_for_owner(&package).await?;
            for node in graph.nodes.values() {
                self.record_node(node);
                for edge in node.edges.iter() {
                    self.evaluation_graph
                        .record_edge(node.label.clone(), edge.target.clone());
                }
                if node
                    .edges
                    .iter()
                    .any(|edge| package_targets.contains(&edge.target))
                {
                    result.insert(
                        self.candidates
                            .intern(QueryCandidate::real(node.label.clone())),
                    );
                }
            }
        }
        Ok(result)
    }

    async fn siblings(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        let packages = targets.sibling_packages(&self.candidates)?;
        let mut result = QueryCandidateBatches::empty();
        for package in packages.iter() {
            let graph = self.package_graph_for_owner(package).await?;
            let mut labels = Vec::with_capacity(graph.nodes.len());
            for node in graph.nodes.values() {
                self.record_node(node);
                labels.push(node.label.clone());
            }
            result = result.union(self.real_delivery(labels));
        }
        Ok(result)
    }

    async fn loading_files(
        &mut self,
        targets: &Self::Set,
        include_buildfiles: bool,
    ) -> Result<Self::Set, QueryError> {
        let mut seen_packages = SmallSet::new();
        let mut seen_bzl_labels = SmallSet::new();
        let mut seen_output_labels = SmallSet::new();
        let mut result = QueryCandidateBatches::empty();

        for batch in targets.batches() {
            let ids = batch.ids().to_vec();
            let mut delivered = Vec::new();
            for id in ids {
                let candidate = self.candidates.get(id).clone();
                let owner = candidate.owner_identity()?;
                if !seen_packages.insert(owner.dupe()) {
                    continue;
                }
                let (build_file, reachable_loads) =
                    self.package_load_provenance_for_owner(&owner).await?;

                let mut package_build_label = None;
                if include_buildfiles {
                    let basename = build_file
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            QueryError::evaluation("loaded BUILD file has no UTF-8 basename")
                        })?;
                    let label = QueryLabel::in_owner_package(&owner, basename)?;
                    package_build_label = Some(label.clone());
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::real(label));
                    }
                }

                for load in reachable_loads.iter() {
                    let label = if owner.canonical_repo().is_some() {
                        QueryLabel::from_canonical_in_owner(&load.label, &owner)?
                    } else {
                        QueryLabel::from_canonical(load.label.clone())
                    };
                    if !seen_bzl_labels.insert(label.clone()) {
                        continue;
                    }
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::fake(label.clone(), owner.dupe()));
                    }
                    if include_buildfiles {
                        let companion = if owner.canonical_repo().is_some() {
                            package_build_label.clone()
                        } else if self.root_workspace.is_some() {
                            self.root_build_file_companion(load.label.package().package())
                                .await?
                        } else {
                            let load_package =
                                self.workspace.join(load.label.package().package().as_str());
                            discover_build_file_companion(self.ctx, &self.workspace, &load_package)
                                .await
                                .map_err(|error| QueryError::evaluation(error.to_string()))?
                                .map(|companion| QueryLabel::from_canonical(companion.label))
                        };
                        if let Some(label) = companion {
                            if seen_output_labels.insert(label.clone()) {
                                delivered.push(QueryCandidate::fake(label, owner.dupe()));
                            }
                        }
                    }
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery(
                &mut self.candidates,
                delivered,
            ));
        }
        Ok(result)
    }

    async fn labels(
        &mut self,
        attribute: &str,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut labels = SmallSet::new();
        for (_, id) in targets.materialized_by_label(&self.candidates) {
            let candidate = self.candidates.get(id).clone();
            let Some(label) = candidate.evaluation_graph_label().cloned() else {
                continue;
            };
            let node = self.resolve_single(label.clone()).await?;
            if !node.kind.is_rule() {
                continue;
            }
            let Some(attribute) = node
                .attributes
                .iter()
                .find(|projection| projection.name == attribute)
            else {
                continue;
            };
            for label in attribute.labels.iter().cloned() {
                self.resolve_single(label.clone()).await.map_err(|error| {
                    if error.is_preparation_restart() {
                        return error;
                    }
                    let message =
                        format!("in '{}' of rule {}: {error}", attribute.name, node.label);
                    error.with_message(message)
                })?;
                labels.insert(label);
            }
        }
        Ok(self.real_delivery(labels))
    }

    async fn attr(
        &mut self,
        attribute: &str,
        regex: &Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            let ids = batch.ids().to_vec();
            let mut delivered = Vec::with_capacity(ids.len());
            for id in ids {
                let candidate = self.candidates.get(id).clone();
                let Some(label) = candidate.evaluation_graph_label().cloned() else {
                    continue;
                };
                let node = self.resolve_single(label).await?;
                if attr_matches_node(&node, attribute, regex)? {
                    delivered.push(id);
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery_ids(delivered));
        }
        Ok(result)
    }

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            let mut delivered = Vec::with_capacity(batch.ids().len());
            for id in batch.ids().iter().copied() {
                let Some(label) = self.candidates.get(id).evaluation_graph_label().cloned() else {
                    // Fake candidates have no loaded target and must neither
                    // be classified nor create a graph node/edge.
                    continue;
                };
                let node = self.resolve_single(label).await?;
                if node.rule_capability.as_ref().is_some_and(|capability| {
                    capability.executable && !capability.rule_class.ends_with("_test")
                }) {
                    delivered.push(id);
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery_ids(delivered));
        }
        Ok(result)
    }

    async fn filter(
        &mut self,
        regex: &Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            let delivered = batch
                .ids()
                .iter()
                .copied()
                .filter(|id| {
                    regex
                        .find(
                            self.candidates
                                .get(*id)
                                .printed_label()
                                .output_label()
                                .as_str(),
                        )
                        .is_some()
                })
                .collect();
            result = result.union(QueryCandidateBatches::from_delivery_ids(delivered));
        }
        Ok(result)
    }

    async fn kind(&mut self, regex: &Regex, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            let ids = batch.ids().to_vec();
            let mut delivered = Vec::with_capacity(ids.len());
            for id in ids {
                let candidate = self.candidates.get(id).clone();
                let kind = match candidate.evaluation_graph_label().cloned() {
                    None => CompactString::const_new("source file"),
                    Some(label) => {
                        let node = self.lookup_single(label).await?;
                        selected_node_kind(&node)
                    }
                };
                if regex.find(kind.as_str()).is_some() {
                    delivered.push(id);
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery_ids(delivered));
        }
        Ok(result)
    }

    async fn visible(
        &mut self,
        callers: &TargetSet<Self::Target>,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            for target in batch.ids().iter().copied() {
                let mut visible = true;
                for caller in callers.iter().copied() {
                    if !self.visible_to(caller, target).await? {
                        visible = false;
                        break;
                    }
                }
                if visible {
                    // VisibleFunction passes every retained candidate through
                    // its own callback invocation; retain that delivery shape.
                    result = result.union(QueryCandidateBatches::from_delivery_ids(vec![target]));
                }
            }
        }
        Ok(result)
    }

    fn query_policy(&self) -> QueryPolicy {
        self.policy
    }

    async fn test_target_info(
        &mut self,
        target: &Self::Target,
    ) -> Result<TestTargetInfo, QueryError> {
        let candidate = self.candidates.get(*target).clone();
        let label = CompactString::new(candidate.printed_label().to_string());
        let Some(graph_label) = candidate.evaluation_graph_label().cloned() else {
            return Ok(TestTargetInfo {
                label,
                kind: TestTargetKind::Other,
                tags: Arc::from([]),
                size: None,
            });
        };
        let node = self.resolve_single(graph_label).await?;
        let kind = match node
            .rule_capability
            .as_ref()
            .and_then(|capability| capability.test_kind)
        {
            Some(TestRuleKind::Test) => TestTargetKind::Test,
            Some(TestRuleKind::Suite) => TestTargetKind::Suite,
            None => TestTargetKind::Other,
        };
        let metadata = node.test_metadata;
        Ok(TestTargetInfo {
            label,
            kind,
            tags: metadata
                .as_ref()
                .map_or_else(|| Arc::from([]), |metadata| metadata.tags.clone()),
            size: metadata.and_then(|metadata| metadata.size),
        })
    }

    async fn test_suite_members(
        &mut self,
        suite: &Self::Target,
        attribute: TestSuiteAttribute,
    ) -> Result<Arc<[Self::Target]>, QueryError> {
        let candidate = self.candidates.get(*suite).clone();
        let Some(suite_label) = candidate.evaluation_graph_label().cloned() else {
            return Ok(Arc::from([]));
        };
        let node = self.resolve_single(suite_label.clone()).await?;
        let Some(attribute) = node
            .attributes
            .iter()
            .find(|projection| projection.name == attribute.name())
        else {
            return Ok(Arc::from([]));
        };
        let mut members = Vec::with_capacity(attribute.labels.len());
        for label in attribute.labels.iter().cloned() {
            self.evaluation_graph
                .record_edge(suite_label.clone(), label.clone());
            self.resolve_single(label.clone()).await?;
            members.push(self.candidates.intern(QueryCandidate::real(label)));
        }
        Ok(members.into())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;

    use dice::DetectCycles;
    use dice::Dice;
    use regex::Regex;
    use slug_bzlmod_v2 as bzlmod;
    use slug_bzlmod_v2::BzlmodCommandPolicyKey;
    use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
    use slug_bzlmod_v2::LockfileMode;
    use slug_bzlmod_v2::inject_root_module_request_inputs;
    use slug_identity_v2 as identity;
    use slug_loading_v2::AttributeKind;
    use slug_loading_v2::AttributeProvenance;
    use slug_loading_v2::AttributeQueryValue;
    use slug_loading_v2::CoercedAttributeValue;
    use slug_loading_v2::VisibilitySource;
    use slug_loading_v2::keys::WorkspaceDirectoryEntry;
    use slug_loading_v2::keys::WorkspaceDirectoryEntryKind;
    use slug_loading_v2::keys::WorkspaceDirectorySnapshot;
    use slug_loading_v2::keys::WorkspaceDirectorySnapshotKey;
    use slug_loading_v2::keys::WorkspaceDirectoryValue;
    use slug_loading_v2::keys::WorkspaceFileValue;
    use slug_loading_v2::keys::WorkspaceSnapshot;
    use slug_loading_v2::keys::WorkspaceSnapshotKey;
    use slug_workspace_v2::PathDirectoryEntries as DirectoryEntries;
    use slug_workspace_v2::PathLstat;
    use slug_workspace_v2::PathObservationDemand as Demand;
    use slug_workspace_v2::PathObservationEpoch as Epoch;
    use slug_workspace_v2::PathObservationEpochKey as EpochKey;
    use slug_workspace_v2::PathObservationOperation as Operation;
    use slug_workspace_v2::PathObservationResult as Observation;
    use slug_workspace_v2::PathOperationResult as OperationResult;
    use slug_workspace_v2::WorkspaceRawFileValue;
    use slug_workspace_v2::WorkspaceRawSnapshot;
    use slug_workspace_v2::WorkspaceRawSnapshotKey;

    use super::*;

    struct AttrScratch(PathBuf);

    impl Drop for AttrScratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn attr_scratch() -> AttrScratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "slug-query-attr-{}-{nanos}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        AttrScratch(path)
    }

    fn attr_write(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn attr_observations(root: &Path) -> (WorkspaceSnapshot, WorkspaceDirectorySnapshot) {
        let mut pending = vec![root.to_path_buf()];
        let mut files = Vec::new();
        let mut directories = Vec::new();
        while let Some(directory) = pending.pop() {
            let mut entries = Vec::new();
            for entry in fs::read_dir(&directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let file_type = entry.file_type().unwrap();
                let kind = if file_type.is_file() {
                    files.push((
                        path,
                        WorkspaceFileValue::Present(Arc::new(
                            fs::read_to_string(entry.path()).unwrap(),
                        )),
                    ));
                    WorkspaceDirectoryEntryKind::RegularFile
                } else if file_type.is_dir() {
                    pending.push(path);
                    WorkspaceDirectoryEntryKind::Directory
                } else if file_type.is_symlink() {
                    WorkspaceDirectoryEntryKind::Symlink
                } else {
                    WorkspaceDirectoryEntryKind::Other
                };
                entries.push(WorkspaceDirectoryEntry {
                    name: entry.file_name().to_str().unwrap().into(),
                    kind,
                });
            }
            directories.push((directory, WorkspaceDirectoryValue::present(entries)));
        }
        (
            WorkspaceSnapshot {
                files: Arc::new(files.into_iter().collect()),
            },
            WorkspaceDirectorySnapshot {
                directories: Arc::new(directories.into_iter().collect()),
            },
        )
    }

    async fn attr_transaction(dice: &Arc<Dice>, workspace: &Path) -> dice::DiceTransaction {
        let (files, directories) = attr_observations(workspace);
        let raw_files = Arc::new(WorkspaceRawSnapshot {
            files: Arc::new(
                files
                    .files
                    .iter()
                    .map(|(path, value)| {
                        let value = match value {
                            WorkspaceFileValue::Present(source) => {
                                WorkspaceRawFileValue::Present(Arc::from(source.as_bytes()))
                            }
                            WorkspaceFileValue::Absent => WorkspaceRawFileValue::Absent,
                            WorkspaceFileValue::ReadError(error) => {
                                WorkspaceRawFileValue::ReadError(error.clone())
                            }
                        };
                        (path.clone(), value)
                    })
                    .collect(),
            ),
        });
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(
                WorkspaceSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                Arc::new(files),
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceRawSnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                raw_files,
            )])
            .unwrap();
        updater
            .changed_to(vec![(
                WorkspaceDirectorySnapshotKey {
                    workspace: workspace.to_path_buf(),
                },
                Arc::new(directories),
            )])
            .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
        )
        .unwrap();
        updater.commit().await
    }

    fn attr_test_node(kind: crate::QueryNodeKind, value: Option<AttributeQueryValue>) -> QueryNode {
        QueryNode {
            label: QueryLabel::parse_root("//pkg:probe").unwrap(),
            kind,
            rule_capability: None,
            test_metadata: None,
            build_file: "pkg/BUILD.bazel".into(),
            effective_visibility: RuleVisibility::Public,
            visibility_source: VisibilitySource::AlwaysPublic,
            package_group_contents: None,
            edges: Arc::from([]),
            attributes: Arc::from([crate::QueryAttribute {
                name: "candidate".into(),
                labels: Arc::from([]),
                explicit: true,
                value,
            }]),
        }
    }

    fn attr_query_value(kind: AttributeKind, value: CoercedAttributeValue) -> AttributeQueryValue {
        AttributeQueryValue {
            kind,
            provenance: AttributeProvenance::Explicit,
            value,
        }
    }

    #[test]
    fn attr_matcher_renders_repository_labels_and_requires_typed_rule_values() {
        let labels = CoercedAttributeValue::LabelList(Arc::from([
            CanonicalLabel::parse("@@//pkg:main").unwrap(),
            CanonicalLabel::parse("@@ext+//leaf:external").unwrap(),
            CanonicalLabel::parse("@@bazel_tools//tools/test:test_wrapper").unwrap(),
        ]));
        let rule = attr_test_node(
            crate::QueryNodeKind::Rule("probe rule".into()),
            Some(attr_query_value(AttributeKind::LabelList, labels)),
        );

        for pattern in [
            r"^\[//pkg:main, @@ext\+//leaf:external, @@bazel_tools//tools/test:test_wrapper\]$",
            r"@@ext\+//leaf:external",
            r"@@bazel_tools//tools/test:test_wrapper",
        ] {
            assert!(attr_matches_node(&rule, "candidate", &Regex::new(pattern).unwrap()).unwrap());
        }
        assert!(!attr_matches_node(&rule, "missing", &Regex::new(".*").unwrap()).unwrap());

        let untyped_rule = attr_test_node(crate::QueryNodeKind::Rule("probe rule".into()), None);
        assert!(
            !attr_matches_node(&untyped_rule, "candidate", &Regex::new(".*").unwrap()).unwrap()
        );
        for kind in [
            crate::QueryNodeKind::SourceFile,
            crate::QueryNodeKind::GeneratedFile,
            crate::QueryNodeKind::PackageGroup,
        ] {
            let non_rule = attr_test_node(
                kind,
                Some(attr_query_value(
                    AttributeKind::String,
                    CoercedAttributeValue::String("visible".into()),
                )),
            );
            assert!(
                !attr_matches_node(&non_rule, "candidate", &Regex::new("visible").unwrap())
                    .unwrap()
            );
        }
    }

    #[test]
    fn attr_matcher_surfaces_contextual_typed_candidate_errors() {
        let invalid = CoercedAttributeValue::Concatenation(
            Arc::new(CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//pkg:left").unwrap(),
            )),
            Arc::new(CoercedAttributeValue::Label(
                CanonicalLabel::parse("@@//pkg:right").unwrap(),
            )),
        );
        let node = attr_test_node(
            crate::QueryNodeKind::Rule("probe rule".into()),
            Some(attr_query_value(AttributeKind::Label, invalid)),
        );

        assert_eq!(
            attr_matches_node(&node, "candidate", &Regex::new(".*").unwrap())
                .unwrap_err()
                .to_string(),
            "in 'candidate' of rule //pkg:probe: cannot concatenate attribute candidate types label and label"
        );
    }

    #[tokio::test]
    async fn attr_filter_preserves_streamed_ids_and_skips_fake_candidates() {
        let scratch = attr_scratch();
        attr_write(scratch.0.join("MODULE.bazel"), "module(name = \"root\")\n");
        attr_write(
            scratch.0.join("pkg/defs.bzl"),
            "def _impl(ctx):\n    return [DefaultInfo()]\nstring_probe = rule(implementation = _impl, attrs = {\"candidate\": attr.string()})\nlabel_probe = rule(implementation = _impl, attrs = {\"candidate\": attr.label()})\n",
        );
        attr_write(
            scratch.0.join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"label_probe\", \"string_probe\")\nconfig_setting(name = \"cfg_a\", values = {\"cpu\": \"a\"})\nconfig_setting(name = \"cfg_b\", values = {\"cpu\": \"b\"})\nfilegroup(name = \"dep\")\nstring_probe(name = \"match_first\", candidate = \"hit-first\")\nstring_probe(name = \"drop_first\", candidate = \"miss\")\nstring_probe(name = \"drop_all\", candidate = \"miss\")\nstring_probe(name = \"same\", candidate = \"hit-same\")\nstring_probe(name = \"match_last\", candidate = \"hit-last\")\nlabel_probe(name = \"bad\", candidate = select({\":cfg_a\": \":dep\"}) + select({\":cfg_b\": \":dep\"}))\n",
        );
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = attr_transaction(&dice, &scratch.0).await;
        let mut environment = LoadingQueryEnvironment::new(
            &mut transaction,
            scratch.0.clone(),
            QueryPolicy::default(),
        );
        let first_match = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:match_first").unwrap(),
        ));
        let first_drop = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:drop_first").unwrap(),
        ));
        let empty_drop = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:drop_all").unwrap(),
        ));
        let fake_owner = QueryPackageIdentity::root(PackagePath::parse("consumer").unwrap());
        let fake_same = environment.candidates.intern(QueryCandidate::fake(
            QueryLabel::parse_root("//pkg:same").unwrap(),
            fake_owner.clone(),
        ));
        let real_same = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:same").unwrap(),
        ));
        let last_match = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:match_last").unwrap(),
        ));
        let streamed = QueryCandidateBatches::from_delivery_ids(vec![first_match, first_drop])
            .union(QueryCandidateBatches::from_delivery_ids(vec![empty_drop]))
            .union(QueryCandidateBatches::from_delivery_ids(vec![
                fake_same, real_same, last_match,
            ]));

        let filtered = environment
            .attr("candidate", &Regex::new("hit").unwrap(), &streamed)
            .await
            .unwrap();

        assert_eq!(filtered.batches().len(), 2);
        assert_eq!(filtered.batches()[0].ids(), &[first_match]);
        assert_eq!(filtered.batches()[1].ids(), &[real_same, last_match]);
        assert_eq!(
            environment
                .candidates
                .get(fake_same)
                .owner_identity()
                .unwrap(),
            fake_owner
        );
        assert_eq!(
            environment
                .candidates
                .get(real_same)
                .owner_identity()
                .unwrap()
                .package()
                .as_str(),
            "pkg"
        );

        let bad = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::parse_root("//pkg:bad").unwrap(),
        ));
        let later_error = QueryCandidateBatches::from_delivery_ids(vec![first_match])
            .union(QueryCandidateBatches::from_delivery_ids(vec![bad]));
        assert_eq!(
            environment
                .attr("candidate", &Regex::new("hit").unwrap(), &later_error)
                .await
                .unwrap_err()
                .to_string(),
            "in 'candidate' of rule //pkg:bad: cannot concatenate attribute candidate types label and label"
        );
    }

    #[tokio::test]
    async fn external_restricted_visible_uses_canonical_fake_caller_without_a_second_route() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let mut epoch = SmallMap::new();
        let demand = |path, operation| {
            Demand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let lstat = |kind| {
            Observation::Lstat(OperationResult::Present(PathLstat::new(
                kind, 1, 1, 1, 1, 0o755,
            )))
        };
        for path in ["/", "/workspace", "/workspace/dep"] {
            epoch.insert(
                demand(path, Operation::Lstat),
                lstat(PathNodeKind::Directory),
            );
        }
        for path in [
            "/workspace/REPO.bazel",
            "/workspace/.bazelignore",
            "/workspace/dep/REPO.bazel",
            "/workspace/dep/.bazelignore",
        ] {
            epoch.insert(
                demand(path, Operation::Lstat),
                Observation::Lstat(OperationResult::Missing),
            );
        }
        for (path, source) in [
            ("/workspace/MODULE.bazel", &b"module(name = \"root\")\nbazel_dep(name = \"dep\", version = \"1.0.0\")\nlocal_path_override(module_name = \"dep\", path = \"dep\")\n"[..]),
            ("/workspace/dep/MODULE.bazel", &b"module(name = \"dep\", version = \"1.0.0\")\n"[..]),
            ("/workspace/dep/BUILD.bazel", &b"package_group(name = \"group\", packages = [\"//viewer\"])\nfilegroup(name = \"restricted\", visibility = [\":group\"])\n"[..]),
        ] {
            epoch.insert(
                demand(path, Operation::Lstat),
                lstat(PathNodeKind::RegularFile),
            );
            epoch.insert(
                demand(path, Operation::FileBytes),
                Observation::FileBytes(OperationResult::Present(Arc::from(source)))
            );
        }
        epoch.insert(
            demand("/workspace/dep", Operation::DirectoryEntries),
            Observation::DirectoryEntries(OperationResult::Present(DirectoryEntries::new([]))),
        );

        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut updater = dice.updater();
        updater
            .changed_to(vec![(EpochKey, Epoch::new(epoch).unwrap())])
            .unwrap();
        bzlmod::inject_root_package_policy_inputs(
            &mut updater,
            bzlmod::RootPackagePolicyInputs::new(
                workspace.clone(),
                vec![workspace.clone()],
                &[] as &[&str],
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        bzlmod::inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            bzlmod::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            bzlmod::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            bzlmod::LockfileMode::Update,
        )
        .unwrap();
        let request = Arc::new(bzlmod::RepositoryMaterializationRequest {
            id: bzlmod::RepositoryMaterializationRequestId {
                workspace: workspace.clone(),
                canonical_repo: identity::CanonicalRepoName::new("dep+").unwrap(),
            },
            repo_spec: bzlmod::RepoSpec {
                rule_id: bzlmod::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::from("path"),
                    bzlmod::OverrideAttributeValue::String("dep".into()),
                )])),
            },
            kind: bzlmod::RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
            },
        });
        updater
            .changed_to(vec![(
                bzlmod::RepositoryMaterializationResultEpochKey {
                    workspace: workspace.clone(),
                },
                bzlmod::RepositoryMaterializationResultEpoch::new(
                    workspace.clone(),
                    [bzlmod::RepositoryMaterializationEpochEntry {
                        request,
                        result: bzlmod::RepositoryMaterializationResult::Success(
                            bzlmod::RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let mut environment =
            LoadingQueryEnvironment::new_root(&mut transaction, workspace, QueryPolicy::default());

        let dep = identity::CanonicalRepoName::new("dep+").unwrap();
        let apparent = identity::ApparentRepoName::new("dep").unwrap();
        let target = environment.candidates.intern(QueryCandidate::real(
            QueryLabel::from_apparent_route(
                &identity::ApparentLabel::parse("@dep//:restricted").unwrap(),
                &dep,
            )
            .unwrap(),
        ));
        let targets = QueryCandidateBatches::from_delivery_ids(vec![target]);
        let caller = |repo| {
            QueryCandidate::fake(
                QueryLabel::parse_root("//viewer:caller").unwrap(),
                QueryPackageIdentity::external(
                    identity::CanonicalRepoName::new(repo).unwrap(),
                    apparent.clone(),
                    PackagePath::parse("viewer").unwrap(),
                )
                .unwrap(),
            )
        };
        let same = environment.candidates.intern(caller("dep+"));
        let other = environment.candidates.intern(caller("other+"));

        let visible = environment
            .visible(&TargetSet::singleton(same), &targets)
            .await
            .unwrap();
        assert_eq!(visible.batches()[0].ids(), &[target]);
        assert!(
            environment
                .visible(&TargetSet::singleton(other), &targets)
                .await
                .unwrap()
                .batches()
                .is_empty()
        );

        let same_owner = environment.candidates.get(same).owner_identity().unwrap();
        let root_target = QueryLabel::parse_root("//viewer:target").unwrap();
        assert!(
            LoadingQueryEnvironment::same_package_or_java(&same_owner, &root_target)
                && same_owner.canonical_package()
                    != root_target.owner_identity().unwrap().canonical_package()
        );
        let java_owner = QueryPackageIdentity::external(
            identity::CanonicalRepoName::new("dep+").unwrap(),
            apparent,
            PackagePath::parse("javatests/lib").unwrap(),
        )
        .unwrap();
        assert!(LoadingQueryEnvironment::same_package_or_java(
            &java_owner,
            &QueryLabel::parse_root("//java/lib:target").unwrap()
        ));
        let third = environment.candidates.intern(QueryCandidate::fake(
            QueryLabel::parse_root("//loads:third.bzl").unwrap(),
            java_owner,
        ));

        let streamed = environment
            .visible(
                &TargetSet::default(),
                &QueryCandidateBatches::from_delivery_ids(vec![same, other])
                    .union(QueryCandidateBatches::from_delivery_ids(vec![third])),
            )
            .await
            .unwrap();
        assert_eq!(streamed.batches().len(), 3);
        assert_eq!(streamed.batches()[0].ids(), &[same]);
        assert_eq!(streamed.batches()[1].ids(), &[other]);
        assert_eq!(streamed.batches()[2].ids(), &[third]);
    }

    #[tokio::test]
    async fn regex_filters_preserve_fake_delivery_boundaries_and_owners() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let updater = dice.updater();
        let mut transaction = updater.commit().await;
        let mut environment = LoadingQueryEnvironment::new(
            &mut transaction,
            PathBuf::from("/workspace"),
            QueryPolicy::default(),
        );
        let owner_a = QueryPackageIdentity::root(PackagePath::parse("consumer_a").unwrap());
        let owner_b = QueryPackageIdentity::root(PackagePath::parse("consumer_b").unwrap());
        let fake = |label: &str, owner: QueryPackageIdentity| {
            QueryCandidate::fake(QueryLabel::parse_root(label).unwrap(), owner)
        };
        let first = environment
            .candidates
            .intern(fake("//loads:keep_first.bzl", owner_a.clone()));
        let second = environment
            .candidates
            .intern(fake("//loads:keep_second.bzl", owner_a.clone()));
        let first_dropped = environment
            .candidates
            .intern(fake("//loads:drop_first.bzl", owner_a.clone()));
        let emptied = environment
            .candidates
            .intern(fake("//loads:drop_all.bzl", owner_b.clone()));
        let third = environment
            .candidates
            .intern(fake("//loads:keep_third.bzl", owner_b.clone()));
        let third_dropped = environment
            .candidates
            .intern(fake("//loads:drop_third.bzl", owner_b.clone()));
        let streamed = QueryCandidateBatches::from_delivery_ids(vec![first, second, first_dropped])
            .union(QueryCandidateBatches::from_delivery_ids(vec![emptied]))
            .union(QueryCandidateBatches::from_delivery_ids(vec![
                third,
                third_dropped,
            ]));

        let filtered = environment
            .filter(&Regex::new("keep").unwrap(), &streamed)
            .await
            .unwrap();
        assert_eq!(filtered.batches().len(), 2);
        assert_eq!(filtered.batches()[0].ids(), &[first, second]);
        assert_eq!(filtered.batches()[1].ids(), &[third]);
        for (id, owner) in [(first, &owner_a), (second, &owner_a), (third, &owner_b)] {
            let QueryCandidate::Fake {
                consuming_owner, ..
            } = environment.candidates.get(id)
            else {
                panic!("regex filter must retain fake candidate provenance");
            };
            assert_eq!(consuming_owner, owner);
        }

        let kinds = environment
            .kind(&Regex::new("^source file$").unwrap(), &streamed)
            .await
            .unwrap();
        assert_eq!(kinds.batches().len(), 3);
        assert_eq!(kinds.batches()[0].ids(), &[first, second, first_dropped]);
        assert_eq!(kinds.batches()[1].ids(), &[emptied]);
        assert_eq!(kinds.batches()[2].ids(), &[third, third_dropped]);
        assert!(
            environment
                .kind(&Regex::new("^generated file$").unwrap(), &streamed)
                .await
                .unwrap()
                .batches()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn all_target_wildcards_fail_closed_before_loading() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        let mut environment = LoadingQueryEnvironment::new_root(
            &mut transaction,
            NormalizedAbsolutePath::new("/must-not-be-read").unwrap(),
            QueryPolicy::default(),
        );
        for (raw, expected) in [
            ("//pkg:*", "all-target package patterns are deferred"),
            (
                "//pkg:all-targets",
                "all-target package patterns are deferred",
            ),
            ("//pkg/...:*", "all-target recursive patterns are deferred"),
            (
                "//pkg/...:all-targets",
                "all-target recursive patterns are deferred",
            ),
            (
                "@dep//pkg:*",
                "external repository query patterns are deferred",
            ),
            (
                "@dep//pkg/...:all-targets",
                "external repository query patterns are deferred",
            ),
        ] {
            let error = environment.resolve_literal(raw).await.unwrap_err();
            assert_eq!(error.to_string(), format!("{expected}: {raw}"));
        }
    }
}
