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
/// This is intentionally smaller than `TransitiveSetOrdering`: Buck/Kuro
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
    match order {
        NestedSetOrder::Default | NestedSetOrder::Postorder => {
            collect_postorder(root, identity, direct_items, children)
        }
        NestedSetOrder::Preorder => collect_preorder(root, identity, direct_items, children),
        NestedSetOrder::Topological => collect_topological(root, identity, direct_items, children),
    }
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

fn collect_preorder<N, I, Id, Direct, Children, Identity>(
    root: N,
    mut identity: Identity,
    mut direct_items: Direct,
    mut children: Children,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if !seen.insert(identity(node)) {
            continue;
        }
        result.extend(direct_items(node));
        for child in children(node).into_iter().rev() {
            stack.push(child);
        }
    }

    result
}

fn collect_postorder<N, I, Id, Direct, Children, Identity>(
    root: N,
    mut identity: Identity,
    mut direct_items: Direct,
    mut children: Children,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    enum Mark<N> {
        Enter(N),
        Exit(N),
    }

    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![Mark::Enter(root)];

    while let Some(mark) = stack.pop() {
        match mark {
            Mark::Enter(node) => {
                if !seen.insert(identity(node)) {
                    continue;
                }
                stack.push(Mark::Exit(node));
                for child in children(node).into_iter().rev() {
                    stack.push(Mark::Enter(child));
                }
            }
            Mark::Exit(node) => result.extend(direct_items(node)),
        }
    }

    result
}

fn collect_topological<N, I, Id, Direct, Children, Identity>(
    root: N,
    mut identity: Identity,
    mut direct_items: Direct,
    mut children: Children,
) -> Vec<I>
where
    N: Copy,
    Id: Copy + Eq + Hash,
    Direct: FnMut(N) -> Vec<I>,
    Children: FnMut(N) -> Vec<N>,
    Identity: FnMut(N) -> Id,
{
    let mut result = Vec::new();
    let mut output_stack = vec![root];
    let mut instance_counts = count_child_instances(root, &mut identity, &mut children);

    while let Some(node) = output_stack.pop() {
        result.extend(direct_items(node));
        for child in children(node).into_iter().rev() {
            let Some(count) = instance_counts.get_mut(&identity(child)) else {
                continue;
            };
            if *count == 1 {
                output_stack.push(child);
            }
            *count = count.saturating_sub(1);
        }
    }

    result
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
