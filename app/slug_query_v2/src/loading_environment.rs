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
use slug_identity_v2::TargetPattern;
use slug_loading_v2::discover_build_file_companion;
use slug_loading_v2::keys::PackageLoadKey;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::generic::QueryEnvironment;
use crate::generic::TargetSet;
use crate::graph::QueryError;
use crate::graph::QueryLabel;
use crate::graph::QueryNode;
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
    evaluation_graph: ResolvedGraph<QueryLabel>,
    generated_file_labels: SmallSet<QueryLabel>,
    pub(crate) candidates: QueryCandidateArena,
}

impl<'a, 'd> LoadingQueryEnvironment<'a, 'd> {
    pub(crate) fn new(ctx: &'a mut DiceComputations<'d>, workspace: PathBuf) -> Self {
        Self {
            ctx,
            workspace,
            evaluation_graph: ResolvedGraph::new(),
            generated_file_labels: SmallSet::new(),
            candidates: QueryCandidateArena::new(),
        }
    }

    async fn package_graph(
        &mut self,
        package: &str,
    ) -> Result<Arc<UnconfiguredPackageGraph>, QueryError> {
        let value = self
            .ctx
            .compute(&UnconfiguredPackageGraphKey {
                workspace: self.workspace.clone(),
                package: PathBuf::from(package),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        value.as_ref().clone()
    }

    async fn resolve_single(&mut self, label: QueryLabel) -> Result<QueryNode, QueryError> {
        if !label.is_root_repository() {
            return Err(QueryError::evaluation(format!(
                "external repository query labels are deferred: {label}"
            )));
        }
        let graph = self.package_graph(label.package()).await.map_err(|error| {
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
        })?;
        let node = graph.nodes.get(&label).cloned().ok_or_else(|| {
            QueryError::evaluation(format!(
                "no such target '{}': target '{}' not declared in package '{}'",
                label,
                label.target(),
                label.package()
            ))
        })?;
        if matches!(node.kind, crate::QueryNodeKind::GeneratedFile) {
            self.generated_file_labels.insert(label.dupe());
        }
        self.evaluation_graph.record_node(label);
        Ok(node)
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
            self.evaluation_graph.record_node(node.label.clone());
            for dependency in node
                .dependencies
                .iter()
                .filter(|dependency| selected.contains(dependency))
            {
                self.evaluation_graph
                    .record_edge(node.label.clone(), dependency.clone());
            }
        }
    }

    async fn resolve_recursive(
        &mut self,
        prefix: &str,
    ) -> Result<TargetSet<QueryLabel>, QueryError> {
        let packages = self
            .ctx
            .compute(&SubtreePackageSetKey {
                workspace: self.workspace.clone(),
                prefix: PathBuf::from(prefix),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
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

    fn real_delivery(
        &mut self,
        labels: impl IntoIterator<Item = QueryLabel>,
    ) -> QueryCandidateBatches {
        QueryCandidateBatches::from_delivery(
            &mut self.candidates,
            labels.into_iter().map(QueryCandidate::real),
        )
    }

    pub(crate) fn selected_graph(&self, targets: &QueryCandidateBatches) -> SelectedQueryGraph {
        let materialized = targets.materialized_by_label(&self.candidates);
        let mut included = SmallMap::<QueryLabel, bool>::new();
        for (label, id) in materialized {
            let candidate = self.candidates.get(id);
            let real = candidate.evaluation_graph_label().is_some();
            if !real || self.evaluation_graph.contains(&label) {
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
                generated_file_labels.insert(CompactString::new(label.to_string()));
            }
            nodes.push(SelectedQueryGraphNode {
                label: CompactString::new(label.to_string()),
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
        let pattern = TargetPattern::parse(literal).map_err(QueryError::evaluation)?;
        match pattern {
            TargetPattern::Single(label) => {
                let label = QueryLabel::parse_root(&label.to_string())?;
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
        let mut dependencies = Vec::with_capacity(node.dependencies.len());
        for dependency in node.dependencies.iter() {
            self.evaluation_graph
                .record_edge(label.clone(), dependency.clone());
            dependencies.push(
                self.candidates
                    .intern(QueryCandidate::real(dependency.clone())),
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
                self.evaluation_graph.record_node(node.label.clone());
                for dependency in node.dependencies.iter() {
                    self.evaluation_graph
                        .record_edge(node.label.clone(), dependency.clone());
                }
                if node
                    .dependencies
                    .iter()
                    .any(|dependency| package_targets.contains(dependency))
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
            for label in graph.nodes.keys() {
                self.evaluation_graph.record_node(label.clone());
                labels.push(label.clone());
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
                let package = self.workspace.join(owner.as_str());
                let value = self
                    .ctx
                    .compute(&PackageLoadKey {
                        workspace: self.workspace.clone(),
                        package,
                    })
                    .await
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;
                let loaded = value
                    .as_ref()
                    .as_ref()
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;

                if include_buildfiles {
                    let basename = loaded
                        .build_file
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

                for load in loaded.reachable_loads.iter() {
                    let label = QueryLabel::from_canonical(load.label.clone());
                    if !seen_bzl_labels.insert(label.clone()) {
                        continue;
                    }
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::fake(label.clone(), owner.clone()));
                    }
                    if include_buildfiles {
                        let load_package =
                            self.workspace.join(load.label.package().package().as_str());
                        let companion =
                            discover_build_file_companion(self.ctx, &self.workspace, &load_package)
                                .await
                                .map_err(|error| QueryError::evaluation(error.to_string()))?;
                        if let Some(companion) = companion {
                            let label = QueryLabel::from_canonical(companion.label);
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
}
