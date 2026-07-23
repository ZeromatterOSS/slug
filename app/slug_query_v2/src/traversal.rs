/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Request-local generic query traversal.

use std::collections::VecDeque;
use std::hash::Hash;

use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::generic::QueryEnvironment;
use crate::generic::TargetSet;
use crate::graph::QueryError;

pub(crate) async fn transitive_closure<E>(
    environment: &mut E,
    roots: TargetSet<E::Target>,
    max_depth: Option<i32>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let mut result = TargetSet::default();
    async_depth_limited_traversal(environment, roots.iter().cloned(), max_depth, |target| {
        result.insert(target)
    })
    .await?;
    Ok(result)
}

#[derive(Clone)]
pub(crate) struct ResolvedGraphNode<T> {
    pub(crate) target: T,
    pub(crate) children: Vec<u32>,
}

/// A request-local, integer-indexed graph adapted from Buck2
/// `query/graph/graph.rs::Graph`. DICE continues to own immutable package
/// nodes; this type owns only traversal state for one query evaluation.
pub(crate) struct ResolvedGraph<T> {
    pub(crate) nodes: Vec<ResolvedGraphNode<T>>,
    pub(crate) target_to_index: SmallMap<T, u32>,
}

impl<T> ResolvedGraph<T>
where
    T: Clone + Eq + Hash,
{
    pub(crate) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            target_to_index: SmallMap::new(),
        }
    }

    pub(crate) fn record_node(&mut self, target: T) {
        self.get_or_create(target);
    }

    pub(crate) fn record_edge(&mut self, from: T, to: T) {
        let (from, _) = self.get_or_create(from);
        let (to, _) = self.get_or_create(to);
        let children = &mut self.nodes[from as usize].children;
        if !children.contains(&to) {
            children.push(to);
        }
    }

    pub(crate) fn deterministic_topological_order(&self) -> Vec<T> {
        let mut visited = SmallSet::new();
        let mut postorder = Vec::with_capacity(self.nodes.len());
        for root in 0..self.nodes.len() {
            let root: u32 = root
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            if !visited.insert(root) {
                continue;
            }
            let mut stack = vec![(root, 0_usize)];
            while let Some((index, next_child)) = stack.last_mut() {
                if let Some(child) = self.nodes[*index as usize]
                    .children
                    .get(*next_child)
                    .copied()
                {
                    *next_child += 1;
                    if visited.insert(child) {
                        stack.push((child, 0));
                    }
                } else {
                    let (index, _) = stack.pop().expect("query DFS stack is non-empty");
                    postorder.push(self.nodes[index as usize].target.clone());
                }
            }
        }
        postorder.reverse();
        postorder
    }

    async fn build_stable_forward<E>(
        environment: &mut E,
        roots: impl IntoIterator<Item = T>,
    ) -> Result<Self, QueryError>
    where
        E: QueryEnvironment<Target = T> + Send,
    {
        let roots = roots.into_iter().collect::<Vec<_>>();
        let mut graph = Self {
            nodes: Vec::new(),
            target_to_index: SmallMap::new(),
        };
        let mut pending = VecDeque::new();
        for root in roots.iter().cloned() {
            let (index, created) = graph.get_or_create(root);
            if created {
                pending.push_back(index);
            }
        }

        while let Some(index) = pending.pop_front() {
            let target = graph.nodes[index as usize].target.clone();
            let dependencies = environment.dependencies(&target).await?;
            let mut children = Vec::with_capacity(dependencies.len());
            for dependency in dependencies.iter().cloned() {
                let (child, created) = graph.get_or_create(dependency);
                children.push(child);
                if created {
                    pending.push_back(child);
                }
            }
            children.shrink_to_fit();
            graph.nodes[index as usize].children = children;
        }

        Ok(graph.stable_dfs_order(roots))
    }

    fn get_or_create(&mut self, target: T) -> (u32, bool) {
        if let Some(index) = self.target_to_index.get(&target) {
            return (*index, false);
        }
        let index: u32 = self
            .nodes
            .len()
            .try_into()
            .expect("query graph exceeds u32 node capacity");
        self.target_to_index.insert(target.clone(), index);
        self.nodes.push(ResolvedGraphNode {
            target,
            children: Vec::new(),
        });
        (index, true)
    }

    fn stable_dfs_order(self, roots: Vec<T>) -> Self {
        let mut visited = SmallSet::new();
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = roots
            .iter()
            .rev()
            .filter_map(|root| self.target_to_index.get(root).copied())
            .collect::<Vec<_>>();
        while let Some(index) = stack.pop() {
            if !visited.insert(index) {
                continue;
            }
            order.push(index);
            for child in self.nodes[index as usize].children.iter().rev() {
                stack.push(*child);
            }
        }
        debug_assert_eq!(order.len(), self.nodes.len());
        self.remap(order)
    }

    fn remap(self, old_indices: Vec<u32>) -> Self {
        let mut old_to_new = vec![None; self.nodes.len()];
        for (new, old) in old_indices.iter().copied().enumerate() {
            old_to_new[old as usize] = Some(
                new.try_into()
                    .expect("query graph exceeds u32 node capacity"),
            );
        }

        let mut nodes = Vec::with_capacity(old_indices.len());
        let mut target_to_index = SmallMap::with_capacity(old_indices.len());
        for old in old_indices {
            let old_node = &self.nodes[old as usize];
            let new_index: u32 = nodes
                .len()
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            let target = old_node.target.clone();
            let children = old_node
                .children
                .iter()
                .filter_map(|child| old_to_new[*child as usize])
                .collect();
            target_to_index.insert(target.clone(), new_index);
            nodes.push(ResolvedGraphNode { target, children });
        }
        Self {
            nodes,
            target_to_index,
        }
    }

    fn reverse(mut self) -> Self {
        let mut reversed = (0..self.nodes.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for (parent, node) in self.nodes.iter().enumerate() {
            for child in node.children.iter() {
                reversed[*child as usize].push(
                    parent
                        .try_into()
                        .expect("query graph exceeds u32 node capacity"),
                );
            }
        }
        for (node, children) in self.nodes.iter_mut().zip(reversed) {
            node.children = children;
        }
        self
    }

    fn take_max_depth(self, roots: &[T], max_depth: i32) -> Self {
        let mut visited = SmallSet::new();
        let mut retained = Vec::new();
        let mut edge = VecDeque::new();
        for root in roots {
            let Some(index) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if visited.insert(index) {
                retained.push(index);
                edge.push_back((index, 0_i32));
            }
        }

        while let Some((index, depth)) = edge.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for child in self.nodes[index as usize].children.iter().copied() {
                if visited.insert(child) {
                    retained.push(child);
                    edge.push_back((child, depth.saturating_add(1)));
                }
            }
        }
        if retained.len() == self.nodes.len() {
            self
        } else {
            self.remap(retained)
        }
    }

    pub(crate) fn contains(&self, target: &T) -> bool {
        self.target_to_index.get(target).is_some()
    }

    fn depth_first_postorder(&self, roots: &[T]) -> TargetSet<T> {
        let mut result = TargetSet::default();
        let mut visited = SmallSet::new();
        for root in roots {
            let Some(root) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if !visited.insert(root) {
                continue;
            }
            let mut stack = vec![(root, 0_usize)];
            while let Some((index, next_child)) = stack.last_mut() {
                let node = &self.nodes[*index as usize];
                if let Some(child) = node.children.get(*next_child).copied() {
                    *next_child += 1;
                    if visited.insert(child) {
                        stack.push((child, 0));
                    }
                } else {
                    let (index, _) = stack.pop().expect("query DFS stack is non-empty");
                    result.insert(self.nodes[index as usize].target.clone());
                }
            }
        }
        result
    }

    /// Compact integer-index BFS and parent reconstruction adapted from Buck2
    /// `query/graph/async_bfs.rs::async_bfs_find_path`. The forward graph is
    /// already resolved through the serial mutable-DICE transaction, so this
    /// V2 seam queues integer indices rather than lookup futures.
    fn shortest_path(&self, roots: &[T], destinations: &TargetSet<T>) -> TargetSet<T> {
        let mut visited = SmallSet::new();
        let mut parents = vec![None; self.nodes.len()];
        let mut pending = VecDeque::new();

        for root in roots {
            let Some(root_index) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if destinations.contains(root) {
                return TargetSet::singleton(root.clone());
            }
            if visited.insert(root_index) {
                pending.push_back(root_index);
            }
        }

        while let Some(parent) = pending.pop_front() {
            for child in self.nodes[parent as usize].children.iter().copied() {
                if !visited.insert(child) {
                    continue;
                }
                parents[child as usize] = Some(parent);
                if destinations.contains(&self.nodes[child as usize].target) {
                    let mut path = vec![child];
                    let mut cursor = child;
                    while let Some(parent) = parents[cursor as usize] {
                        path.push(parent);
                        cursor = parent;
                    }
                    path.reverse();
                    let mut result = TargetSet::default();
                    for index in path {
                        result.insert(self.nodes[index as usize].target.clone());
                    }
                    return result;
                }
                pending.push_back(child);
            }
        }

        TargetSet::default()
    }
}

pub(crate) async fn reverse_dependencies<E>(
    environment: &mut E,
    universe: TargetSet<E::Target>,
    from: TargetSet<E::Target>,
    max_depth: Option<i32>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let graph = ResolvedGraph::build_stable_forward(environment, universe.iter().cloned()).await?;
    if max_depth.is_some_and(|depth| depth < 0) {
        return Ok(TargetSet::default());
    }
    let graph = graph.reverse();
    let roots = from
        .iter()
        .filter(|target| graph.contains(*target))
        .cloned()
        .collect::<Vec<_>>();
    let graph = match max_depth {
        Some(depth) => graph.take_max_depth(&roots, depth),
        None => graph,
    };
    Ok(graph.depth_first_postorder(&roots))
}

pub(crate) async fn some_path<E>(
    environment: &mut E,
    from: TargetSet<E::Target>,
    to: TargetSet<E::Target>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let graph = ResolvedGraph::build_stable_forward(environment, from.iter().cloned()).await?;
    let roots = from.iter().cloned().collect::<Vec<_>>();
    Ok(graph.shortest_path(&roots, &to))
}

/// Generic depth-limited traversal adapted from Buck2
/// `buck2_query/src/query/traversal.rs::async_depth_limited_traversal`.
///
/// Buck2 can queue concurrent immutable node lookups. Loading query owns a
/// mutable `DiceComputations`, so this port preserves the generic lookup /
/// visited / ordered-queue abstraction while resolving one node at a time.
async fn async_depth_limited_traversal<E>(
    environment: &mut E,
    roots: impl IntoIterator<Item = E::Target>,
    max_depth: Option<i32>,
    mut visit: impl FnMut(E::Target),
) -> Result<(), QueryError>
where
    E: QueryEnvironment + Send,
{
    if max_depth.is_some_and(|depth| depth < 0) {
        return Ok(());
    }
    let mut visited = SmallSet::new();
    let mut pending = VecDeque::new();
    for root in roots {
        if visited.insert(root.clone()) {
            pending.push_back((root, 0));
        }
    }
    while let Some((target, depth)) = pending.pop_front() {
        visit(target.clone());
        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }
        for dependency in environment.dependencies(&target).await?.iter() {
            if visited.insert(dependency.clone()) {
                pending.push_back((dependency.clone(), depth.saturating_add(1)));
            }
        }
    }
    Ok(())
}
