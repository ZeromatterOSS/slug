/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the root directory of this source tree. You
 * may select, at your option, one of the above-listed licenses.
 */

use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;

use allocative::Allocative;
use dupe::Dupe;
use starlark::values::Freeze;
use starlark::values::Trace;

/// Shared order vocabulary for Bazel-style nested sets.
///
/// This is intentionally smaller than `TransitiveSetOrdering`: Buck/Slug
/// transitive sets also expose `bfs` and `dfs` as use-site traversal orders.
#[derive(
    Debug, Clone, Dupe, Copy, Trace, Freeze, PartialEq, Eq, Hash, Allocative
)]
pub enum NestedSetOrder {
    Default,
    Postorder,
    Preorder,
    Topological,
}

impl NestedSetOrder {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "default" => Some(Self::Default),
            "postorder" => Some(Self::Postorder),
            "preorder" => Some(Self::Preorder),
            "topological" => Some(Self::Topological),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Postorder => "postorder",
            Self::Preorder => "preorder",
            Self::Topological => "topological",
        }
    }

    pub fn is_compatible_with_child(self, child: Self) -> bool {
        self == Self::Default || child == Self::Default || self == child
    }
}

/// Shared nested-set dedupe modes.
///
/// `NodeIdentity` is the transitive-set traversal model: each graph node is
/// visited once according to its identity. `ValueHashEq` keeps the same
/// node-deduped walk, then suppresses repeated output items by a caller-supplied
/// hash/equality identity.
#[derive(
    Debug, Clone, Dupe, Copy, Trace, Freeze, PartialEq, Eq, Hash, Allocative
)]
pub enum NestedSetDedup {
    NodeIdentity,
    ValueHashEq,
}

#[derive(
    Debug, Clone, Dupe, Copy, Trace, Freeze, PartialEq, Eq, Hash, Allocative
)]
pub enum NestedSetPreorderDedupe {
    OnVisit,
    OnChildEnqueue,
}

/// Collect the items stored in a nested DAG with Bazel-style traversal orders.
///
/// This is intentionally representation-agnostic: callers provide node
/// identity, direct item extraction, and child extraction. Public facades such
/// as depset and transitive_set still own their own validation and item
/// semantics.
pub fn collect_nested_set<N, I, Id, Direct, Children, Identity>(
    root: N,
    order: NestedSetOrder,
    identity: Identity,
    direct_items: Direct,
    children: Children,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    collect_nested_set_node_dedup(root, order, identity, direct_items, children)
}

pub fn collect_nested_set_with_dedup<N, I, Id, ItemId, Direct, Children, Identity, ItemIdentity>(
    root: N,
    order: NestedSetOrder,
    dedup: NestedSetDedup,
    identity: Identity,
    direct_items: Direct,
    children: Children,
    item_identity: ItemIdentity,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    ItemId: Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
    ItemIdentity: FnMut(&I) -> ItemId,
{
    match dedup {
        NestedSetDedup::NodeIdentity => {
            collect_nested_set_node_dedup(root, order, identity, direct_items, children)
        }
        NestedSetDedup::ValueHashEq => collect_nested_set_value_dedup_by(
            root,
            order,
            identity,
            direct_items,
            children,
            item_identity,
        ),
    }
}

pub fn collect_nested_set_node_dedup<N, I, Id, Direct, Children, Identity>(
    root: N,
    order: NestedSetOrder,
    identity: Identity,
    mut direct_items: Direct,
    children: Children,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    let mut result = Vec::new();
    for node in nested_set_node_iter(root, order, identity, children) {
        result.extend(direct_items(node));
    }
    result
}

pub fn collect_nested_set_value_dedup_by<
    N,
    I,
    Id,
    ItemId,
    Direct,
    Children,
    Identity,
    ItemIdentity,
>(
    root: N,
    order: NestedSetOrder,
    identity: Identity,
    direct_items: Direct,
    children: Children,
    mut item_identity: ItemIdentity,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    ItemId: Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
    ItemIdentity: FnMut(&I) -> ItemId,
{
    let collected = collect_nested_set_node_dedup(root, order, identity, direct_items, children);
    let mut result = Vec::with_capacity(collected.len());
    let mut seen = HashSet::new();

    for item in collected {
        if seen.insert(item_identity(&item)) {
            result.push(item);
        }
    }

    result
}

pub fn nested_set_node_iter<'a, N, Id, Children, Identity>(
    root: N,
    order: NestedSetOrder,
    identity: Identity,
    children: Children,
) -> Box<dyn Iterator<Item = N> + 'a>
where
    N: Copy + 'a,
    Id: Copy + Eq + Hash + 'a,
    Children: FnMut(N) -> Vec<N> + 'a,
    Identity: FnMut(N) -> Id + 'a,
{
    match order {
        NestedSetOrder::Default | NestedSetOrder::Postorder => Box::new(
            PostorderNestedSetNodeIterator::new(root, identity, children),
        ),
        NestedSetOrder::Preorder => Box::new(preorder_nested_set_node_iter(
            root,
            NestedSetPreorderDedupe::OnVisit,
            identity,
            children,
        )),
        NestedSetOrder::Topological => Box::new(TopologicalNestedSetNodeIterator::new(
            root, identity, children,
        )),
    }
}

pub fn preorder_nested_set_node_iter<'a, N, Id, Children, Identity>(
    root: N,
    dedupe: NestedSetPreorderDedupe,
    identity: Identity,
    children: Children,
) -> impl Iterator<Item = N> + 'a
where
    N: Copy + 'a,
    Id: Copy + Eq + Hash + 'a,
    Children: FnMut(N) -> Vec<N> + 'a,
    Identity: FnMut(N) -> Id + 'a,
{
    PreorderNestedSetNodeIterator::new(root, dedupe, identity, children)
}

struct PreorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    stack: Vec<N>,
    seen: HashSet<Id>,
    dedupe: NestedSetPreorderDedupe,
    identity: Identity,
    children: Children,
}

impl<N, Id, Identity, Children> PreorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    pub fn new(
        root: N,
        dedupe: NestedSetPreorderDedupe,
        identity: Identity,
        children: Children,
    ) -> Self {
        Self {
            stack: vec![root],
            seen: HashSet::new(),
            dedupe,
            identity,
            children,
        }
    }
}

impl<N, Id, Identity, Children> Iterator
    for PreorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.stack.pop()?;
            match self.dedupe {
                NestedSetPreorderDedupe::OnVisit => {
                    if !self.seen.insert((self.identity)(node)) {
                        continue;
                    }
                    for child in (self.children)(node).into_iter().rev() {
                        self.stack.push(child);
                    }
                }
                NestedSetPreorderDedupe::OnChildEnqueue => {
                    for child in (self.children)(node).into_iter().rev() {
                        if self.seen.insert((self.identity)(child)) {
                            self.stack.push(child);
                        }
                    }
                }
            }
            return Some(node);
        }
    }
}

enum PostorderNestedSetNodeMark<N> {
    Enter(N),
    Exit(N),
}

struct PostorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    stack: Vec<PostorderNestedSetNodeMark<N>>,
    seen: HashSet<Id>,
    identity: Identity,
    children: Children,
}

impl<N, Id, Identity, Children> PostorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    pub fn new(root: N, identity: Identity, children: Children) -> Self {
        Self {
            stack: vec![PostorderNestedSetNodeMark::Enter(root)],
            seen: HashSet::new(),
            identity,
            children,
        }
    }
}

impl<N, Id, Identity, Children> Iterator
    for PostorderNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.stack.pop()? {
                PostorderNestedSetNodeMark::Enter(node) => {
                    if !self.seen.insert((self.identity)(node)) {
                        continue;
                    }
                    self.stack.push(PostorderNestedSetNodeMark::Exit(node));
                    for child in (self.children)(node).into_iter().rev() {
                        self.stack.push(PostorderNestedSetNodeMark::Enter(child));
                    }
                }
                PostorderNestedSetNodeMark::Exit(node) => return Some(node),
            }
        }
    }
}

struct TopologicalNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    output_stack: Vec<N>,
    instance_counts: HashMap<Id, u32>,
    identity: Identity,
    children: Children,
}

impl<N, Id, Identity, Children> TopologicalNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    pub fn new(root: N, mut identity: Identity, mut children: Children) -> Self {
        let instance_counts = count_child_instances(root, &mut identity, &mut children);
        Self {
            output_stack: vec![root],
            instance_counts,
            identity,
            children,
        }
    }
}

impl<N, Id, Identity, Children> Iterator
    for TopologicalNestedSetNodeIterator<N, Id, Identity, Children>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    type Item = N;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.output_stack.pop()?;

        for child in (self.children)(node).into_iter().rev() {
            let Some(count) = self.instance_counts.get_mut(&(self.identity)(child)) else {
                continue;
            };
            if *count == 1 {
                self.output_stack.push(child);
            }
            *count = count.saturating_sub(1);
        }

        Some(node)
    }
}

fn count_child_instances<N, Id, Children, Identity>(
    root: N,
    identity: &mut Identity,
    children: &mut Children,
) -> HashMap<Id, u32>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    let mut stack = vec![root];
    let mut instance_counts = HashMap::new();

    while let Some(node) = stack.pop() {
        for child in children(node).into_iter().rev() {
            let child_id = identity(child);
            let count = instance_counts.entry(child_id).or_insert(0);
            *count += 1;
            if *count == 1 {
                stack.push(child);
            }
        }
    }

    instance_counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestNode {
        direct: Vec<&'static str>,
        children: Vec<usize>,
    }

    fn diamond() -> Vec<TestNode> {
        vec![
            TestNode {
                direct: vec!["d"],
                children: vec![1, 2],
            },
            TestNode {
                direct: vec!["b"],
                children: vec![3],
            },
            TestNode {
                direct: vec!["c"],
                children: vec![3],
            },
            TestNode {
                direct: vec!["a"],
                children: vec![],
            },
        ]
    }

    fn collect(order: NestedSetOrder) -> Vec<&'static str> {
        let nodes = diamond();
        collect_nested_set(
            0,
            order,
            |node| node,
            |node| nodes[node].direct.clone(),
            |node| nodes[node].children.clone(),
        )
    }

    #[test]
    fn node_identity_dedupes_shared_diamond_nodes() {
        assert_eq!(vec!["a", "b", "c", "d"], collect(NestedSetOrder::Postorder));
        assert_eq!(vec!["d", "b", "a", "c"], collect(NestedSetOrder::Preorder));
        assert_eq!(
            vec!["d", "b", "c", "a"],
            collect(NestedSetOrder::Topological)
        );
    }

    #[test]
    fn value_hash_eq_dedupes_output_items_after_node_walk() {
        let nodes = vec![
            TestNode {
                direct: vec!["root", "dup"],
                children: vec![1, 2],
            },
            TestNode {
                direct: vec!["dup", "left"],
                children: vec![],
            },
            TestNode {
                direct: vec!["dup", "right"],
                children: vec![],
            },
        ];

        let collected = collect_nested_set_with_dedup(
            0,
            NestedSetOrder::Postorder,
            NestedSetDedup::ValueHashEq,
            |node| node,
            |node| nodes[node].direct.clone(),
            |node| nodes[node].children.clone(),
            |item| *item,
        );

        assert_eq!(vec!["dup", "left", "right", "root"], collected);
    }
}
