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
use slug_bzlmod_v2::HostRootPackageBoundaryKey;
use slug_bzlmod_v2::HostRootPackageBoundaryKind;
use slug_bzlmod_v2::RootRepositoryRoute;
use slug_bzlmod_v2::RootRepositoryRouteKey;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::PackagePath;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::LoadingPreparationNeeds;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_loading_v2::RootPackageLoadKey;
use slug_loading_v2::RuleVisibility;
use slug_loading_v2::TestRuleKind;
use slug_loading_v2::discover_build_file_companion;
use slug_loading_v2::keys::PackageLoadKey;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathOutcome;
use slug_workspace_v2::ResolvedPathKey;
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
use crate::graph::QueryError;
use crate::graph::QueryLabel;
use crate::graph::QueryNode;
use crate::graph::RootSubtreePackageSetKey;
use crate::graph::RootUnconfiguredPackageGraphKey;
use crate::graph::SubtreePackageSetKey;
use crate::graph::UnconfiguredPackageGraph;
use crate::graph::UnconfiguredPackageGraphKey;
use crate::output::SelectedQueryGraph;
use crate::output::SelectedQueryGraphNode;
use crate::provenance::QueryCandidate;
use crate::provenance::QueryCandidateArena;
use crate::provenance::QueryCandidateBatches;
use crate::provenance::QueryCandidateId;
use crate::traversal::ResolvedGraph;

pub(crate) struct LoadingQueryEnvironment<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    workspace: PathBuf,
    root_workspace: Option<NormalizedAbsolutePath>,
    preparation_needs: Option<LoadingPreparationNeeds>,
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
            policy,
            evaluation_graph: ResolvedGraph::new(),
            node_kinds: SmallMap::new(),
            generated_file_labels: SmallSet::new(),
            candidates: QueryCandidateArena::new(),
        }
    }

    pub(crate) fn take_preparation_needs(&mut self) -> Option<LoadingPreparationNeeds> {
        self.preparation_needs.take()
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
            return match self
                .ctx
                .compute(&RootUnconfiguredPackageGraphKey::new(workspace, package))
                .await
                .expect("root query graph DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
                LoadingPreparationOutcome::Complete(value) => value.as_ref().clone(),
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
        let key = RootRepositoryRouteKey::new(workspace, apparent_repo.clone())
            .map_err(QueryError::evaluation)?;
        match self
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
        }
    }

    async fn external_package_graph(
        &mut self,
        route: RootRepositoryRoute,
        package: &str,
    ) -> Result<Arc<UnconfiguredPackageGraph>, QueryError> {
        let package = PackagePath::parse(package).map_err(QueryError::evaluation)?;
        match self
            .ctx
            .compute(&ExternalUnconfiguredPackageGraphKey::new(route, package))
            .await
            .expect("external query graph DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => Err(self.preparation_restart(need)),
            LoadingPreparationOutcome::Complete(value) => value.as_ref().clone(),
        }
    }

    async fn lookup_single(&mut self, label: QueryLabel) -> Result<QueryNode, QueryError> {
        let graph = if label.is_root_repository() {
            self.package_graph(label.package()).await.map_err(|error| {
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
            })?
        } else {
            let apparent_repo = label
                .apparent_repo()
                .cloned()
                .ok_or_else(|| QueryError::evaluation("external query label lost render route"))?;
            let route = self.repository_route(&apparent_repo).await?;
            self.external_package_graph(route, label.package()).await?
        };
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
            return match self
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
        let caller_package = self.candidates.get(caller).owner_package();
        let target = self.candidates.get(target).clone();
        let Some(label) = target.evaluation_graph_label().cloned() else {
            return Ok(true);
        };
        let node = self.lookup_single(label).await?;
        let same_package_or_java = caller_package == node.label.package()
            || matches!(
                (
                    caller_package.strip_prefix("javatests/"),
                    node.label.package().strip_prefix("java/")
                ),
                (Some(caller_suffix), Some(target_suffix)) if caller_suffix == target_suffix
            );
        match &node.effective_visibility {
            RuleVisibility::Public => Ok(true),
            RuleVisibility::Private => Ok(same_package_or_java),
            RuleVisibility::Restricted(restricted) => {
                let caller = CanonicalLabel::parse(&format!("@@//{caller_package}:__pkg__"))
                    .map_err(QueryError::evaluation)?;
                let caller_package = caller.package().clone();
                let mut visible = restricted
                    .direct_packages()
                    .contains_package(&caller_package);
                // Do not short-circuit: Bazel resolves every top-level root so
                // a positive alternative cannot mask a later missing group.
                for group in restricted.package_groups() {
                    let group = QueryLabel::from_canonical(group.clone());
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
            match self
                .ctx
                .compute(&RootSubtreePackageSetKey::new(workspace, prefix))
                .await
                .expect("root subtree package-set DICE invariant")
            {
                LoadingPreparationOutcome::Need(need) => {
                    return Err(self.preparation_restart(need));
                }
                LoadingPreparationOutcome::Complete(packages) => packages,
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
        let boundary = self
            .ctx
            .compute(&HostRootPackageBoundaryKey::new(workspace, package.clone()))
            .await
            .expect("Host package-boundary DICE invariant");
        let selected_root = match boundary {
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
            match self
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
                PathOutcome::Complete(Err(error)) => {
                    return Err(QueryError::evaluation(format!("{error:?}")));
                }
                PathOutcome::Complete(Ok(resolved))
                    if matches!(
                        resolved.state(),
                        ResolvedPathState::Present(lstat)
                            if matches!(
                                lstat.kind(),
                                PathNodeKind::RegularFile | PathNodeKind::SpecialFile
                            )
                    ) =>
                {
                    selected = Some(basename);
                    break;
                }
                PathOutcome::Complete(Ok(_)) => {}
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
            TargetPattern::PackageAll { repo, package } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
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
            TargetPattern::Recursive { repo, package } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
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
        let mut by_package = SmallMap::<CompactString, SmallSet<QueryLabel>>::new();
        for target in targets.iter() {
            let Some(target) = self
                .candidates
                .get(*target)
                .evaluation_graph_label()
                .cloned()
            else {
                continue;
            };
            by_package
                .entry(CompactString::new(target.package()))
                .or_default()
                .insert(target);
        }

        let mut result = TargetSet::default();
        for (package, package_targets) in by_package {
            let graph = self.package_graph(&package).await?;
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
        let packages = targets.sibling_packages(&self.candidates);
        let mut result = QueryCandidateBatches::empty();
        for package in packages.iter() {
            let graph = self.package_graph(package).await?;
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
                let candidate_package = CompactString::new(candidate.printed_label().package());
                if !seen_packages.insert(candidate_package) {
                    continue;
                }
                let owner = candidate.owner_package();
                let (build_file, reachable_loads) =
                    self.package_load_provenance(owner.as_str()).await?;

                if include_buildfiles {
                    let basename = build_file
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            QueryError::evaluation("loaded BUILD file has no UTF-8 basename")
                        })?;
                    let label = QueryLabel::parse_root(&format!("//{owner}:{basename}"))?;
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::real(label));
                    }
                }

                for load in reachable_loads.iter() {
                    let label = QueryLabel::from_canonical(load.label.clone());
                    if !seen_bzl_labels.insert(label.clone()) {
                        continue;
                    }
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::fake(label.clone(), owner.clone()));
                    }
                    if include_buildfiles {
                        let companion = if self.root_workspace.is_some() {
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
                                delivered.push(QueryCandidate::fake(label, owner.clone()));
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
    use dice::DetectCycles;
    use dice::Dice;

    use super::*;

    #[tokio::test]
    async fn visible_retains_each_passing_streamed_candidate_as_a_singleton_delivery() {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        let mut environment =
            LoadingQueryEnvironment::new(&mut transaction, PathBuf::new(), QueryPolicy::default());
        let first = environment.candidates.intern(QueryCandidate::fake(
            QueryLabel::parse_root("//loads:first.bzl").unwrap(),
            "consumer",
        ));
        let second = environment.candidates.intern(QueryCandidate::fake(
            QueryLabel::parse_root("//loads:second.bzl").unwrap(),
            "consumer",
        ));
        let third = environment.candidates.intern(QueryCandidate::fake(
            QueryLabel::parse_root("//loads:third.bzl").unwrap(),
            "consumer",
        ));
        let targets = QueryCandidateBatches::from_delivery_ids(vec![first, second])
            .union(QueryCandidateBatches::from_delivery_ids(vec![third]));

        let result = environment
            .visible(&TargetSet::default(), &targets)
            .await
            .unwrap();
        assert_eq!(
            result
                .batches()
                .iter()
                .map(|batch| batch.ids())
                .collect::<Vec<_>>(),
            [first, second, third]
                .iter()
                .map(std::slice::from_ref)
                .collect::<Vec<_>>()
        );
    }
}
