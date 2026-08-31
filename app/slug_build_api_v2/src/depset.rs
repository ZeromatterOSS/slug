/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use fxhash::FxHashMap;
use fxhash::FxHashSet;
use fxhash::hash64;

pub const MAX_DEPTH: usize = 3500;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum DepsetOrder {
    Default,
    Postorder,
    Preorder,
    Topological,
}

impl DepsetOrder {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Postorder => "postorder",
            Self::Preorder => "preorder",
            Self::Topological => "topological",
        }
    }

    pub(crate) fn compatible_with(self, child: Self) -> bool {
        self == Self::Default || child == Self::Default || self == child
    }
}

impl fmt::Display for DepsetOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DepsetOrder {
    type Err = DepsetError;

    fn from_str(order: &str) -> Result<Self, Self::Err> {
        match order {
            "default" => Ok(Self::Default),
            "postorder" => Ok(Self::Postorder),
            "preorder" => Ok(Self::Preorder),
            "topological" => Ok(Self::Topological),
            other => Err(DepsetError::InvalidOrder {
                order: other.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DepsetError {
    InvalidOrder {
        order: String,
    },
    IncompatibleOrder {
        parent: DepsetOrder,
        child: DepsetOrder,
    },
    DepthLimitExceeded {
        depth: usize,
        max: usize,
    },
    StorageLimitExceeded {
        kind: &'static str,
        count: usize,
    },
    InvalidLocalReference {
        node: usize,
        available: usize,
    },
}

impl fmt::Display for DepsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOrder { order } => write!(f, "Invalid order: {order}"),
            Self::IncompatibleOrder { parent, child } => {
                write!(f, "Order '{parent}' is incompatible with order '{child}'")
            }
            Self::DepthLimitExceeded { depth, max } => {
                write!(f, "depset depth {depth} exceeds limit ({max})")
            }
            Self::StorageLimitExceeded { kind, count } => {
                write!(
                    f,
                    "depset {kind} count {count} exceeds the retained-store limit"
                )
            }
            Self::InvalidLocalReference { node, available } => write!(
                f,
                "depset local node {node} is not available while building {available} nodes"
            ),
        }
    }
}

impl Error for DepsetError {}
#[derive(Debug, Allocative)]
pub struct Depset<T, M = ()> {
    store: Arc<DenseDepsetStore<T, M>>,
    root: u32,
}

impl<T, M> Clone for Depset<T, M> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            root: self.root,
        }
    }
}
impl<T, M> Dupe for Depset<T, M> {}

impl<T: PartialEq, M: PartialEq> PartialEq for Depset<T, M> {
    fn eq(&self, other: &Self) -> bool {
        let mut visited = FxHashSet::default();
        let mut stack = vec![(self.dupe(), other.dupe())];
        while let Some((left, right)) = stack.pop() {
            if left.shares_node_with(&right) {
                continue;
            }
            if !visited.insert((left.node_key(), right.node_key())) {
                continue;
            }
            if left.order() != right.order()
                || left.depth() != right.depth()
                || left.metadata() != right.metadata()
            {
                return false;
            }
            let mut left = left.successors();
            let mut right = right.successors();
            loop {
                match (left.next(), right.next()) {
                    (None, None) => break,
                    (Some(DepsetSuccessor::Direct(left)), Some(DepsetSuccessor::Direct(right)))
                        if left == right => {}
                    (
                        Some(DepsetSuccessor::Transitive(left)),
                        Some(DepsetSuccessor::Transitive(right)),
                    ) => stack.push((left, right)),
                    _ => return false,
                }
            }
        }
        true
    }
}

impl<T: Eq, M: Eq> Eq for Depset<T, M> {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum DepsetSuccessor<T, N> {
    Direct(T),
    Transitive(N),
}

pub trait DepsetView: Clone {
    type Item: Clone;
    type NodeKey: Clone + Eq + Hash;

    fn order(&self) -> DepsetOrder;
    fn depth(&self) -> usize;
    fn node_key(&self) -> Self::NodeKey;
    fn singleton_item(&self) -> Option<Self::Item>;
    fn for_each_successor_reverse<E>(
        &self,
        visitor: impl FnMut(DepsetSuccessor<Self::Item, Self>) -> Result<(), E>,
    ) -> Result<(), E>;
}

#[derive(Debug)]
pub enum DepsetBuild<T, N> {
    Empty,
    Reuse(N),
    Dereference(N),
    Node(Vec<DepsetSuccessor<T, N>>, usize),
}

#[derive(Debug)]
pub enum DepsetBuildError<E> {
    Element(E),
    Depset(DepsetError),
}

struct LeafSet<T>(FxHashMap<u64, Vec<T>>);

impl<T: Clone> LeafSet<T> {
    fn insert<E>(
        &mut self,
        value: T,
        hash: &mut impl FnMut(&T) -> Result<u64, E>,
        equals: &mut impl FnMut(&T, &T) -> Result<bool, E>,
    ) -> Result<bool, E> {
        let key = hash(&value)?;
        if let Some(bucket) = self.0.get(&key) {
            for existing in bucket {
                if equals(existing, &value)? {
                    return Ok(false);
                }
            }
        }
        self.0.entry(key).or_default().push(value);
        Ok(true)
    }
}

pub fn build_depset<T, N, E>(
    order: DepsetOrder,
    direct: Vec<T>,
    mut transitive: Vec<N>,
    mut hash: impl FnMut(&T) -> Result<u64, E>,
    mut equals: impl FnMut(&T, &T) -> Result<bool, E>,
    mut validate_direct: impl FnMut(&T) -> Result<(), E>,
    mut validate_transitive: impl FnMut(&N) -> Result<(), E>,
) -> Result<DepsetBuild<T, N>, DepsetBuildError<E>>
where
    T: Clone,
    N: DepsetView<Item = T>,
{
    let mut direct_seen = LeafSet(FxHashMap::default());
    let mut direct_unique = Vec::new();
    for item in direct {
        validate_direct(&item).map_err(DepsetBuildError::Element)?;
        if direct_seen
            .insert(item.clone(), &mut hash, &mut equals)
            .map_err(DepsetBuildError::Element)?
        {
            direct_unique.push(item);
        }
    }

    if order == DepsetOrder::Topological {
        transitive.reverse();
    }
    let mut node_seen = FxHashSet::default();
    let mut transitive_unique = Vec::new();
    for child in transitive {
        if child.depth() == 0 {
            continue;
        }
        if !order.compatible_with(child.order()) {
            return Err(DepsetBuildError::Depset(DepsetError::IncompatibleOrder {
                parent: order,
                child: child.order(),
            }));
        }
        validate_transitive(&child).map_err(DepsetBuildError::Element)?;
        if node_seen.insert(child.node_key()) {
            transitive_unique.push(child);
        }
    }

    if direct_unique.is_empty() && transitive_unique.is_empty() {
        return Ok(DepsetBuild::Empty);
    }
    if transitive_unique.len() == 1 && direct_unique.len() <= 1 {
        let candidate = &transitive_unique[0];
        let matching = if let Some(direct) = direct_unique.first() {
            candidate
                .singleton_item()
                .map(|item| equals(&item, direct))
                .transpose()
                .map_err(DepsetBuildError::Element)?
                .unwrap_or(false)
        } else {
            true
        };
        if candidate.order() == order && matching {
            return Ok(DepsetBuild::Reuse(candidate.clone()));
        }
    }

    let preorder = order == DepsetOrder::Preorder;
    let mut inserted = LeafSet(FxHashMap::default());
    let mut successors = Vec::new();
    let mut approx_depth = 0;
    for pass in 0..=1 {
        if (pass == 0) == preorder {
            let mut insert_direct = |item: &T| -> Result<(), DepsetBuildError<E>> {
                let item = item.clone();
                if inserted
                    .insert(item.clone(), &mut hash, &mut equals)
                    .map_err(DepsetBuildError::Element)?
                {
                    successors.push(DepsetSuccessor::Direct(item));
                    approx_depth = approx_depth.max(2);
                }
                Ok(())
            };
            if order == DepsetOrder::Topological {
                for item in direct_unique.iter().rev() {
                    insert_direct(item)?;
                }
            } else {
                for item in &direct_unique {
                    insert_direct(item)?;
                }
            }
        } else {
            for child in &transitive_unique {
                approx_depth = approx_depth.max(1 + child.depth());
                if let Some(item) = child.singleton_item() {
                    if inserted
                        .insert(item.clone(), &mut hash, &mut equals)
                        .map_err(DepsetBuildError::Element)?
                    {
                        successors.push(DepsetSuccessor::Direct(item));
                    }
                } else {
                    successors.push(DepsetSuccessor::Transitive(child.clone()));
                }
            }
        }
    }

    let depth = match successors.len() {
        0 => 0,
        1 => approx_depth.saturating_sub(1),
        _ => approx_depth,
    };
    if depth > MAX_DEPTH {
        return Err(DepsetBuildError::Depset(DepsetError::DepthLimitExceeded {
            depth,
            max: MAX_DEPTH,
        }));
    }
    if let [DepsetSuccessor::Transitive(child)] = successors.as_slice() {
        return Ok(DepsetBuild::Dereference(child.clone()));
    }
    Ok(DepsetBuild::Node(successors, depth))
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct DepsetTraversalStats {
    pending: usize,
    expanded_nodes: usize,
    examined_leaves: usize,
}

#[cfg(test)]
impl DepsetTraversalStats {
    fn operations(self) -> usize {
        self.pending + self.expanded_nodes + self.examined_leaves
    }
}

trait TraversalCounter {
    fn pending(&mut self) {}
    fn expanded_node(&mut self) {}
    fn examined_leaf(&mut self) {}
}

impl TraversalCounter for () {}

#[cfg(test)]
impl TraversalCounter for DepsetTraversalStats {
    fn pending(&mut self) {
        self.pending += 1;
    }

    fn expanded_node(&mut self) {
        self.expanded_nodes += 1;
    }

    fn examined_leaf(&mut self) {
        self.examined_leaves += 1;
    }
}

fn traverse_depset_impl<N, E, C: TraversalCounter>(
    root: &N,
    mut hash: impl FnMut(&N::Item) -> Result<u64, E>,
    mut equals: impl FnMut(&N::Item, &N::Item) -> Result<bool, E>,
    stats: &mut C,
) -> Result<Vec<N::Item>, E>
where
    N: DepsetView,
{
    let mut out = Vec::new();
    let mut seen_values = LeafSet(FxHashMap::default());
    let mut seen_nodes = FxHashSet::default();
    let mut stack: Vec<DepsetSuccessor<N::Item, N>> =
        vec![DepsetSuccessor::Transitive(root.clone())];
    while let Some(successor) = stack.pop() {
        stats.pending();
        match successor {
            DepsetSuccessor::Direct(item) => {
                stats.examined_leaf();
                if seen_values.insert(item.clone(), &mut hash, &mut equals)? {
                    out.push(item);
                }
            }
            DepsetSuccessor::Transitive(node) if seen_nodes.insert(node.node_key()) => {
                stats.expanded_node();
                node.for_each_successor_reverse(|successor| {
                    stack.push(successor);
                    Ok(())
                })?;
            }
            DepsetSuccessor::Transitive(_) => {}
        }
    }
    if root.order() == DepsetOrder::Topological {
        out.reverse();
    }
    Ok(out)
}

pub fn traverse_depset<N, E>(
    root: &N,
    hash: impl FnMut(&N::Item) -> Result<u64, E>,
    equals: impl FnMut(&N::Item, &N::Item) -> Result<bool, E>,
) -> Result<Vec<N::Item>, E>
where
    N: DepsetView,
{
    traverse_depset_impl(root, hash, equals, &mut ())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
struct DenseRange {
    start: u32,
    len: u32,
}

impl DenseRange {
    fn checked(start: usize, len: usize, kind: &'static str) -> Result<Self, DepsetError> {
        let start = u32::try_from(start)
            .map_err(|_| DepsetError::StorageLimitExceeded { kind, count: start })?;
        let len = u32::try_from(len)
            .map_err(|_| DepsetError::StorageLimitExceeded { kind, count: len })?;
        start
            .checked_add(len)
            .ok_or(DepsetError::StorageLimitExceeded {
                kind,
                count: usize::MAX,
            })?;
        Ok(Self { start, len })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
enum DenseRowRef {
    Local(DenseRange),
    External(u32),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
enum DenseSuccessor {
    Leaf(u32),
    Local(u32),
    External(u32),
}

#[derive(Debug, Allocative)]
struct DenseNode<M> {
    order: DepsetOrder,
    row: DenseRowRef,
    depth: usize,
    metadata: M,
}

#[derive(Debug, Allocative)]
struct DenseExternalRow<T, M> {
    store: Arc<DenseDepsetStore<T, M>>,
    range: DenseRange,
}

#[derive(Debug, Allocative)]
struct DenseDepsetStore<T, M> {
    nodes: Box<[DenseNode<M>]>,
    successors: Box<[DenseSuccessor]>,
    leaves: Box<[T]>,
    external_depsets: Box<[Depset<T, M>]>,
    external_rows: Box<[DenseExternalRow<T, M>]>,
}

#[derive(Debug)]
pub(crate) enum DenseDepsetInput<T, M> {
    Direct(T),
    Local(u32),
    External(Depset<T, M>),
}

#[derive(Debug)]
pub(crate) enum DenseDepsetRowSource<T, M> {
    Successors(Vec<DenseDepsetInput<T, M>>),
    Local(u32),
    External(Depset<T, M>),
}

#[derive(Debug)]
pub(crate) struct DenseDepsetNodeInput<T, M> {
    pub(crate) order: DepsetOrder,
    pub(crate) row: DenseDepsetRowSource<T, M>,
    pub(crate) depth: usize,
    pub(crate) metadata: M,
}

pub struct DepsetSuccessors<'a, T, M> {
    owner: &'a Arc<DenseDepsetStore<T, M>>,
    next: u32,
    end: u32,
}

impl<'a, T, M> Iterator for DepsetSuccessors<'a, T, M> {
    type Item = DepsetSuccessor<&'a T, Depset<T, M>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.next as usize;
        self.next += 1;
        Some(match self.owner.successors[index] {
            DenseSuccessor::Leaf(index) => {
                DepsetSuccessor::Direct(&self.owner.leaves[index as usize])
            }
            DenseSuccessor::Local(root) => DepsetSuccessor::Transitive(Depset {
                store: self.owner.clone(),
                root,
            }),
            DenseSuccessor::External(index) => {
                DepsetSuccessor::Transitive(self.owner.external_depsets[index as usize].dupe())
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end - self.next) as usize;
        (len, Some(len))
    }
}

impl<T, M> ExactSizeIterator for DepsetSuccessors<'_, T, M> {}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct DenseEdgeKey {
    node_owner: usize,
    node_root: u32,
    offset: u32,
}

enum DensePending<T, M> {
    Node {
        value: Depset<T, M>,
        incoming: Option<DenseEdgeKey>,
    },
    Entry {
        owner: Arc<DenseDepsetStore<T, M>>,
        index: u32,
        edge: DenseEdgeKey,
    },
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Allocative)]
pub struct DepsetStorageStats {
    pub stores: usize,
    pub nodes: usize,
    pub successors: usize,
    pub leaves: usize,
    pub external_depsets: usize,
    pub external_rows: usize,
    pub allocation_objects: usize,
    pub estimated_bytes: usize,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct DepsetConstructionStats {
    nodes: usize,
    successors: usize,
    row_aliases: usize,
}

#[cfg(test)]
impl DepsetConstructionStats {
    fn operations(self) -> usize {
        self.nodes + self.successors + self.row_aliases
    }
}

trait DenseConstructionCounter {
    fn node(&mut self) {}
    fn successor(&mut self) {}
    fn row_alias(&mut self) {}
}

impl DenseConstructionCounter for () {}

#[cfg(test)]
impl DenseConstructionCounter for DepsetConstructionStats {
    fn node(&mut self) {
        self.nodes += 1;
    }

    fn successor(&mut self) {
        self.successors += 1;
    }

    fn row_alias(&mut self) {
        self.row_aliases += 1;
    }
}

impl<T> Depset<T> {
    pub fn empty() -> Self {
        Self::from_canonical_successors(DepsetOrder::Default, Vec::new(), 0, ())
    }
}

impl<T, M> Depset<T, M> {
    fn node(&self) -> &DenseNode<M> {
        &self.store.nodes[self.root as usize]
    }

    fn canonical_row(&self) -> (&Arc<DenseDepsetStore<T, M>>, DenseRange) {
        match self.node().row {
            DenseRowRef::Local(range) => (&self.store, range),
            DenseRowRef::External(index) => {
                let row = &self.store.external_rows[index as usize];
                (&row.store, row.range)
            }
        }
    }

    pub fn order(&self) -> DepsetOrder {
        self.node().order
    }

    pub fn successors(&self) -> DepsetSuccessors<'_, T, M> {
        let (owner, range) = self.canonical_row();
        DepsetSuccessors {
            owner,
            next: range.start,
            end: range.start + range.len,
        }
    }

    pub fn depth(&self) -> usize {
        self.node().depth
    }

    pub fn is_empty(&self) -> bool {
        self.node().depth == 0
    }

    pub fn shares_node_with(&self, other: &Self) -> bool {
        self.root == other.root && Arc::ptr_eq(&self.store, &other.store)
    }

    pub fn shares_store_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.store, &other.store)
    }

    pub fn shares_successors_with(&self, other: &Self) -> bool {
        let (left_store, left_range) = self.canonical_row();
        let (right_store, right_range) = other.canonical_row();
        left_range == right_range && Arc::ptr_eq(left_store, right_store)
    }

    pub fn node_key(&self) -> (usize, u32) {
        (Arc::as_ptr(&self.store) as usize, self.root)
    }

    pub fn metadata(&self) -> &M {
        &self.node().metadata
    }

    pub fn storage_stats(&self) -> DepsetStorageStats {
        let mut result = DepsetStorageStats::default();
        let mut seen = FxHashSet::default();
        let mut stack = vec![self.store.clone()];
        while let Some(store) = stack.pop() {
            let identity = Arc::as_ptr(&store) as usize;
            if !seen.insert(identity) {
                continue;
            }
            result.stores += 1;
            result.nodes += store.nodes.len();
            result.successors += store.successors.len();
            result.leaves += store.leaves.len();
            result.external_depsets += store.external_depsets.len();
            result.external_rows += store.external_rows.len();
            result.allocation_objects += 1
                + usize::from(!store.nodes.is_empty())
                + usize::from(!store.successors.is_empty())
                + usize::from(!store.leaves.is_empty())
                + usize::from(!store.external_depsets.is_empty())
                + usize::from(!store.external_rows.is_empty());
            result.estimated_bytes += std::mem::size_of::<DenseDepsetStore<T, M>>()
                + std::mem::size_of_val(store.nodes.as_ref())
                + std::mem::size_of_val(store.successors.as_ref())
                + std::mem::size_of_val(store.leaves.as_ref())
                + std::mem::size_of_val(store.external_depsets.as_ref())
                + std::mem::size_of_val(store.external_rows.as_ref());
            stack.extend(
                store
                    .external_depsets
                    .iter()
                    .map(|value| value.store.clone()),
            );
            stack.extend(store.external_rows.iter().map(|row| row.store.clone()));
        }
        result
    }

    pub(crate) fn from_canonical_successors(
        order: DepsetOrder,
        successors: Vec<DepsetSuccessor<T, Depset<T, M>>>,
        depth: usize,
        metadata: M,
    ) -> Self {
        Self::try_from_canonical_successors(order, successors, depth, metadata)
            .expect("trusted canonical depset successors fit checked retained indexes")
    }

    pub(crate) fn try_from_canonical_successors(
        order: DepsetOrder,
        successors: Vec<DepsetSuccessor<T, Depset<T, M>>>,
        depth: usize,
        metadata: M,
    ) -> Result<Self, DepsetError> {
        let successors = successors
            .into_iter()
            .map(|successor| match successor {
                DepsetSuccessor::Direct(value) => DenseDepsetInput::Direct(value),
                DepsetSuccessor::Transitive(value) => DenseDepsetInput::External(value),
            })
            .collect();
        Ok(Self::from_dense_nodes(vec![DenseDepsetNodeInput {
            order,
            row: DenseDepsetRowSource::Successors(successors),
            depth,
            metadata,
        }])?
        .pop()
        .expect("one canonical depset node produces one handle"))
    }

    pub(crate) fn from_dense_nodes(
        inputs: Vec<DenseDepsetNodeInput<T, M>>,
    ) -> Result<Vec<Self>, DepsetError> {
        Self::from_dense_nodes_impl(inputs, &mut ())
    }

    fn from_dense_nodes_impl<C: DenseConstructionCounter>(
        inputs: Vec<DenseDepsetNodeInput<T, M>>,
        stats: &mut C,
    ) -> Result<Vec<Self>, DepsetError> {
        u32::try_from(inputs.len()).map_err(|_| DepsetError::StorageLimitExceeded {
            kind: "node",
            count: inputs.len(),
        })?;
        let mut nodes: Vec<DenseNode<M>> = Vec::with_capacity(inputs.len());
        let mut successors = Vec::new();
        let mut leaves = Vec::new();
        let mut external_depsets = Vec::new();
        let mut external_rows = Vec::new();
        let mut external_depset_indexes = FxHashMap::default();
        let mut external_row_indexes = FxHashMap::default();

        for input in inputs {
            stats.node();
            let row = match input.row {
                DenseDepsetRowSource::Successors(input_successors) => {
                    let start = successors.len();
                    for successor in input_successors {
                        stats.successor();
                        let successor = match successor {
                            DenseDepsetInput::Direct(value) => {
                                let index = u32::try_from(leaves.len()).map_err(|_| {
                                    DepsetError::StorageLimitExceeded {
                                        kind: "leaf",
                                        count: leaves.len(),
                                    }
                                })?;
                                leaves.push(value);
                                DenseSuccessor::Leaf(index)
                            }
                            DenseDepsetInput::Local(node) => {
                                if node as usize >= nodes.len() {
                                    return Err(DepsetError::InvalidLocalReference {
                                        node: node as usize,
                                        available: nodes.len(),
                                    });
                                }
                                DenseSuccessor::Local(node)
                            }
                            DenseDepsetInput::External(value) => {
                                let key = value.node_key();
                                let index =
                                    if let Some(index) = external_depset_indexes.get(&key).copied()
                                    {
                                        index
                                    } else {
                                        let index = u32::try_from(external_depsets.len()).map_err(
                                            |_| DepsetError::StorageLimitExceeded {
                                                kind: "external depset",
                                                count: external_depsets.len(),
                                            },
                                        )?;
                                        external_depsets.push(value);
                                        external_depset_indexes.insert(key, index);
                                        index
                                    };
                                DenseSuccessor::External(index)
                            }
                        };
                        successors.push(successor);
                    }
                    DenseRowRef::Local(DenseRange::checked(
                        start,
                        successors.len() - start,
                        "successor",
                    )?)
                }
                DenseDepsetRowSource::Local(node) => {
                    stats.row_alias();
                    let Some(node) = nodes.get(node as usize) else {
                        return Err(DepsetError::InvalidLocalReference {
                            node: node as usize,
                            available: nodes.len(),
                        });
                    };
                    node.row
                }
                DenseDepsetRowSource::External(value) => {
                    stats.row_alias();
                    let (store, range) = value.canonical_row();
                    let key = (Arc::as_ptr(store) as usize, range);
                    let index = if let Some(index) = external_row_indexes.get(&key).copied() {
                        index
                    } else {
                        let index = u32::try_from(external_rows.len()).map_err(|_| {
                            DepsetError::StorageLimitExceeded {
                                kind: "external row",
                                count: external_rows.len(),
                            }
                        })?;
                        external_rows.push(DenseExternalRow {
                            store: store.clone(),
                            range,
                        });
                        external_row_indexes.insert(key, index);
                        index
                    };
                    DenseRowRef::External(index)
                }
            };
            nodes.push(DenseNode {
                order: input.order,
                row,
                depth: input.depth,
                metadata: input.metadata,
            });
        }

        let store = Arc::new(DenseDepsetStore {
            nodes: nodes.into_boxed_slice(),
            successors: successors.into_boxed_slice(),
            leaves: leaves.into_boxed_slice(),
            external_depsets: external_depsets.into_boxed_slice(),
            external_rows: external_rows.into_boxed_slice(),
        });
        Ok((0..store.nodes.len())
            .map(|root| Depset {
                store: store.clone(),
                root: root as u32,
            })
            .collect())
    }

    #[cfg(test)]
    fn from_dense_nodes_counted(
        inputs: Vec<DenseDepsetNodeInput<T, M>>,
    ) -> Result<(Vec<Self>, DepsetConstructionStats), DepsetError> {
        let mut stats = DepsetConstructionStats::default();
        let values = Self::from_dense_nodes_impl(inputs, &mut stats)?;
        Ok((values, stats))
    }

    pub(crate) fn rewrap(order: DepsetOrder, child: &Self, metadata: M) -> Self {
        Self::from_dense_nodes(vec![DenseDepsetNodeInput {
            order,
            row: DenseDepsetRowSource::External(child.dupe()),
            depth: child.depth(),
            metadata,
        }])
        .expect("one dereferenced depset node fits checked retained indexes")
        .pop()
        .expect("one dereferenced depset node produces one handle")
    }
}

impl<T: Clone + Eq + Hash> Depset<T> {
    pub fn new(
        order: DepsetOrder,
        direct: Vec<T>,
        transitive: Vec<Depset<T>>,
    ) -> Result<Self, DepsetError> {
        Self::new_with_metadata(order, direct, transitive, ())
    }

    pub fn from_direct(order: DepsetOrder, direct: Vec<T>) -> Result<Self, DepsetError> {
        Self::new(order, direct, Vec::new())
    }
}

impl<T: Clone + Eq + Hash, M> Depset<T, M> {
    pub(crate) fn new_with_metadata(
        order: DepsetOrder,
        direct: Vec<T>,
        transitive: Vec<Self>,
        metadata: M,
    ) -> Result<Self, DepsetError> {
        let built = build_depset(
            order,
            direct,
            transitive,
            |item| Ok::<_, Infallible>(hash64(item)),
            |left, right| Ok::<_, Infallible>(left == right),
            |_| Ok::<_, Infallible>(()),
            |_| Ok::<_, Infallible>(()),
        )
        .map_err(|error| match error {
            DepsetBuildError::Element(never) => match never {},
            DepsetBuildError::Depset(error) => error,
        })?;
        Ok(match built {
            DepsetBuild::Empty => {
                Self::try_from_canonical_successors(order, Vec::new(), 0, metadata)?
            }
            DepsetBuild::Reuse(value) => value,
            DepsetBuild::Dereference(child) => Self::rewrap(order, &child, metadata),
            DepsetBuild::Node(successors, depth) => {
                Self::try_from_canonical_successors(order, successors, depth, metadata)?
            }
        })
    }

    pub fn visit<E>(&self, mut visitor: impl FnMut(&T) -> Result<(), E>) -> Result<(), E> {
        if self.order() == DepsetOrder::Topological {
            return self.visit_topological(&mut visitor);
        }
        let mut seen_values = LeafSet(FxHashMap::default());
        let mut seen_nodes = FxHashSet::default();
        let mut stack = vec![DensePending::Node {
            value: self.dupe(),
            incoming: None,
        }];
        while let Some(pending) = stack.pop() {
            match pending {
                DensePending::Node { value, .. } if seen_nodes.insert(value.node_key()) => {
                    let (node_owner, node_root) = value.node_key();
                    let (owner, range) = value.canonical_row();
                    for offset in (0..range.len).rev() {
                        stack.push(DensePending::Entry {
                            owner: owner.clone(),
                            index: range.start + offset,
                            edge: DenseEdgeKey {
                                node_owner,
                                node_root,
                                offset,
                            },
                        });
                    }
                }
                DensePending::Node { .. } => {}
                DensePending::Entry {
                    owner,
                    index,
                    edge: _,
                } => match owner.successors[index as usize] {
                    DenseSuccessor::Leaf(leaf) => {
                        let value = &owner.leaves[leaf as usize];
                        if seen_values
                            .insert(
                                value.clone(),
                                &mut |value| Ok::<_, Infallible>(hash64(value)),
                                &mut |left, right| Ok::<_, Infallible>(left == right),
                            )
                            .unwrap_or_else(|never| match never {})
                        {
                            visitor(value)?;
                        }
                    }
                    DenseSuccessor::Local(root) => stack.push(DensePending::Node {
                        value: Depset { store: owner, root },
                        incoming: None,
                    }),
                    DenseSuccessor::External(external) => stack.push(DensePending::Node {
                        value: owner.external_depsets[external as usize].dupe(),
                        incoming: None,
                    }),
                },
            }
        }
        Ok(())
    }

    fn visit_topological<E>(&self, visitor: &mut impl FnMut(&T) -> Result<(), E>) -> Result<(), E> {
        let mut selected_nodes = FxHashSet::default();
        let mut selected_leaves = FxHashSet::default();
        let mut seen_values = LeafSet(FxHashMap::default());
        let mut seen_nodes = FxHashSet::default();
        let mut stack = vec![DensePending::Node {
            value: self.dupe(),
            incoming: None,
        }];
        while let Some(pending) = stack.pop() {
            match pending {
                DensePending::Node { value, incoming } if seen_nodes.insert(value.node_key()) => {
                    if let Some(incoming) = incoming {
                        selected_nodes.insert(incoming);
                    }
                    let (node_owner, node_root) = value.node_key();
                    let (owner, range) = value.canonical_row();
                    for offset in (0..range.len).rev() {
                        stack.push(DensePending::Entry {
                            owner: owner.clone(),
                            index: range.start + offset,
                            edge: DenseEdgeKey {
                                node_owner,
                                node_root,
                                offset,
                            },
                        });
                    }
                }
                DensePending::Node { .. } => {}
                DensePending::Entry { owner, index, edge } => {
                    match owner.successors[index as usize] {
                        DenseSuccessor::Leaf(leaf) => {
                            let value = &owner.leaves[leaf as usize];
                            if seen_values
                                .insert(
                                    value.clone(),
                                    &mut |value| Ok::<_, Infallible>(hash64(value)),
                                    &mut |left, right| Ok::<_, Infallible>(left == right),
                                )
                                .unwrap_or_else(|never| match never {})
                            {
                                selected_leaves.insert(edge);
                            }
                        }
                        DenseSuccessor::Local(root) => stack.push(DensePending::Node {
                            value: Depset { store: owner, root },
                            incoming: Some(edge),
                        }),
                        DenseSuccessor::External(external) => stack.push(DensePending::Node {
                            value: owner.external_depsets[external as usize].dupe(),
                            incoming: Some(edge),
                        }),
                    }
                }
            }
        }

        let mut stack = vec![DensePending::Node {
            value: self.dupe(),
            incoming: None,
        }];
        while let Some(pending) = stack.pop() {
            match pending {
                DensePending::Node { value, .. } => {
                    let (node_owner, node_root) = value.node_key();
                    let (owner, range) = value.canonical_row();
                    for offset in 0..range.len {
                        stack.push(DensePending::Entry {
                            owner: owner.clone(),
                            index: range.start + offset,
                            edge: DenseEdgeKey {
                                node_owner,
                                node_root,
                                offset,
                            },
                        });
                    }
                }
                DensePending::Entry { owner, index, edge } => {
                    match owner.successors[index as usize] {
                        DenseSuccessor::Leaf(leaf) if selected_leaves.contains(&edge) => {
                            visitor(&owner.leaves[leaf as usize])?;
                        }
                        DenseSuccessor::Leaf(_) => {}
                        DenseSuccessor::Local(root) if selected_nodes.contains(&edge) => {
                            stack.push(DensePending::Node {
                                value: Depset { store: owner, root },
                                incoming: None,
                            });
                        }
                        DenseSuccessor::External(external) if selected_nodes.contains(&edge) => {
                            stack.push(DensePending::Node {
                                value: owner.external_depsets[external as usize].dupe(),
                                incoming: None,
                            });
                        }
                        DenseSuccessor::Local(_) | DenseSuccessor::External(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    pub fn to_list(&self) -> Vec<T> {
        traverse_depset(
            self,
            |item| Ok::<_, Infallible>(hash64(item)),
            |left, right| Ok::<_, Infallible>(left == right),
        )
        .unwrap_or_else(|never| match never {})
    }

    #[cfg(test)]
    fn to_list_counted(&self) -> (Vec<T>, DepsetTraversalStats) {
        let mut stats = DepsetTraversalStats::default();
        let values = traverse_depset_impl(
            self,
            |item| Ok::<_, Infallible>(hash64(item)),
            |left, right| Ok::<_, Infallible>(left == right),
            &mut stats,
        )
        .unwrap_or_else(|never| match never {});
        (values, stats)
    }
}

impl<T: Clone, M> DepsetView for Depset<T, M> {
    type Item = T;
    type NodeKey = (usize, u32);

    fn order(&self) -> DepsetOrder {
        self.order()
    }

    fn depth(&self) -> usize {
        self.depth()
    }

    fn node_key(&self) -> Self::NodeKey {
        self.node_key()
    }

    fn singleton_item(&self) -> Option<Self::Item> {
        let mut successors = self.successors();
        match (successors.next(), successors.next()) {
            (Some(DepsetSuccessor::Direct(value)), None) => Some(value.clone()),
            _ => None,
        }
    }

    fn for_each_successor_reverse<E>(
        &self,
        mut visitor: impl FnMut(DepsetSuccessor<Self::Item, Self>) -> Result<(), E>,
    ) -> Result<(), E> {
        let (owner, range) = self.canonical_row();
        for index in (range.start..range.start + range.len).rev() {
            visitor(match owner.successors[index as usize] {
                DenseSuccessor::Leaf(index) => {
                    DepsetSuccessor::Direct(owner.leaves[index as usize].clone())
                }
                DenseSuccessor::Local(root) => DepsetSuccessor::Transitive(Depset {
                    store: owner.clone(),
                    root,
                }),
                DenseSuccessor::External(index) => {
                    DepsetSuccessor::Transitive(owner.external_depsets[index as usize].dupe())
                }
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod dense_storage_tests {
    use super::*;

    // The wide graph is a deterministic upper-bound control for the real BCR
    // rulesets that selected this packet: rules_cc forwards its FDO `all_files`
    // depset through action tools, while rules_rust aggregates many independently
    // produced inputs. Keep the authenticated source identities beside the shape
    // so it cannot silently become an unauthenticated synthetic benchmark.
    const RULES_CC_FDO_SOURCE_SHA256: &str =
        "91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7";
    const RULES_RUST_ARCHIVE_SHA256: &str =
        "2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2";

    #[derive(Debug)]
    enum LegacySuccessor {
        Direct(String),
        Transitive(Arc<LegacyNode>),
    }

    #[derive(Debug)]
    struct LegacyNode {
        order: DepsetOrder,
        successors: Arc<[LegacySuccessor]>,
        depth: usize,
    }

    fn legacy_stats(root: &Arc<LegacyNode>) -> (usize, usize, usize, usize) {
        let mut seen = FxHashSet::default();
        let mut stack = vec![root.clone()];
        let mut allocations = 0;
        let mut bytes = 0;
        let mut steps = 0;
        let mut leaves = 0;
        while let Some(node) = stack.pop() {
            let identity = Arc::as_ptr(&node) as usize;
            if !seen.insert(identity) {
                continue;
            }
            let _ = (node.order, node.depth);
            allocations += 2;
            bytes += std::mem::size_of::<LegacyNode>()
                + std::mem::size_of_val(node.successors.as_ref())
                + 4 * std::mem::size_of::<usize>();
            steps += 1 + node.successors.len();
            for successor in node.successors.iter() {
                match successor {
                    LegacySuccessor::Direct(value) => {
                        let _ = value.len();
                        leaves += 1;
                    }
                    LegacySuccessor::Transitive(child) => stack.push(child.clone()),
                }
            }
        }
        (allocations, bytes, steps, steps + leaves + 1)
    }

    #[test]
    fn dense_store_reduces_legacy_arc_allocations_and_retained_bytes() {
        let inputs = vec![
            DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Direct("shared-a".to_owned()),
                    DenseDepsetInput::Direct("shared-b".to_owned()),
                ]),
                depth: 2,
                metadata: (),
            },
            DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Local(0),
                    DenseDepsetInput::Direct("left".to_owned()),
                ]),
                depth: 3,
                metadata: (),
            },
            DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Local(0),
                    DenseDepsetInput::Direct("right".to_owned()),
                ]),
                depth: 3,
                metadata: (),
            },
            DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Local(1),
                    DenseDepsetInput::Local(2),
                    DenseDepsetInput::Direct("root".to_owned()),
                ]),
                depth: 4,
                metadata: (),
            },
        ];
        let (dense, construction) = Depset::from_dense_nodes_counted(inputs).unwrap();
        let dense_root = dense.last().unwrap();

        let shared = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: vec![
                LegacySuccessor::Direct("shared-a".to_owned()),
                LegacySuccessor::Direct("shared-b".to_owned()),
            ]
            .into(),
            depth: 2,
        });
        let left = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: vec![
                LegacySuccessor::Transitive(shared.clone()),
                LegacySuccessor::Direct("left".to_owned()),
            ]
            .into(),
            depth: 3,
        });
        let right = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: vec![
                LegacySuccessor::Transitive(shared),
                LegacySuccessor::Direct("right".to_owned()),
            ]
            .into(),
            depth: 3,
        });
        let legacy = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: vec![
                LegacySuccessor::Transitive(left),
                LegacySuccessor::Transitive(right),
                LegacySuccessor::Direct("root".to_owned()),
            ]
            .into(),
            depth: 4,
        });

        let dense_stats = dense_root.storage_stats();
        let (legacy_allocations, legacy_bytes, legacy_construction, legacy_traversal) =
            legacy_stats(&legacy);
        let (cold_values, cold) = dense_root.to_list_counted();
        let (warm_values, warm) = dense_root.to_list_counted();
        assert_eq!(dense_stats.stores, 1);
        assert_eq!(dense_stats.nodes, 4);
        assert!(dense_stats.allocation_objects < legacy_allocations);
        assert!(dense_stats.estimated_bytes < legacy_bytes);
        assert_eq!(construction.operations(), 13);
        assert_eq!(cold.operations(), 19);
        assert_eq!(warm.operations(), 19);
        assert!(construction.operations() <= legacy_construction + legacy_construction / 10);
        assert!(cold.operations() <= legacy_traversal + legacy_traversal / 10);
        assert!(warm.operations() <= legacy_traversal + legacy_traversal / 10);
        assert_eq!(
            cold_values,
            ["shared-a", "shared-b", "left", "right", "root"]
        );
        assert_eq!(warm_values, cold_values);
    }

    #[test]
    fn dense_store_scales_for_ruleset_shaped_wide_fan_in() {
        assert_eq!(RULES_CC_FDO_SOURCE_SHA256.len(), 64);
        assert_eq!(RULES_RUST_ARCHIVE_SHA256.len(), 64);
        let mut inputs = (0..64)
            .map(|index| DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Direct(format!("input-{index}")),
                    DenseDepsetInput::Direct(format!("metadata-{index}")),
                ]),
                depth: 2,
                metadata: (),
            })
            .collect::<Vec<_>>();
        inputs.push(DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors((0..64).map(DenseDepsetInput::Local).collect()),
            depth: 3,
            metadata: (),
        });
        let (dense, construction) = Depset::from_dense_nodes_counted(inputs).unwrap();
        let dense_root = dense.last().unwrap();

        let children = (0..64)
            .map(|index| {
                Arc::new(LegacyNode {
                    order: DepsetOrder::Default,
                    successors: vec![
                        LegacySuccessor::Direct(format!("input-{index}")),
                        LegacySuccessor::Direct(format!("metadata-{index}")),
                    ]
                    .into(),
                    depth: 2,
                })
            })
            .collect::<Vec<_>>();
        let legacy = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: children
                .into_iter()
                .map(LegacySuccessor::Transitive)
                .collect::<Vec<_>>()
                .into(),
            depth: 3,
        });
        let dense_stats = dense_root.storage_stats();
        let (legacy_allocations, legacy_bytes, legacy_construction, legacy_traversal) =
            legacy_stats(&legacy);
        let (cold, cold_stats) = dense_root.to_list_counted();
        let (warm, warm_stats) = dense_root.to_list_counted();
        assert!(dense_stats.allocation_objects < legacy_allocations);
        assert!(dense_stats.estimated_bytes < legacy_bytes);
        assert_eq!(construction.operations(), 257);
        assert_eq!(cold_stats.operations(), 386);
        assert_eq!(warm_stats.operations(), 386);
        assert!(construction.operations() <= legacy_construction + legacy_construction / 10);
        assert!(cold_stats.operations() <= legacy_traversal + legacy_traversal / 10);
        assert!(warm_stats.operations() <= legacy_traversal + legacy_traversal / 10);
        assert_eq!(cold.len(), 128);
        assert_eq!(warm, cold);
    }

    #[test]
    fn dense_store_chain_balances_construction_cold_and_warm_operations() {
        let mut inputs = vec![DenseDepsetNodeInput {
            order: DepsetOrder::Default,
            row: DenseDepsetRowSource::Successors(vec![
                DenseDepsetInput::Direct("chain-0-a".to_owned()),
                DenseDepsetInput::Direct("chain-0-b".to_owned()),
            ]),
            depth: 2,
            metadata: (),
        }];
        for index in 1..256 {
            inputs.push(DenseDepsetNodeInput {
                order: DepsetOrder::Default,
                row: DenseDepsetRowSource::Successors(vec![
                    DenseDepsetInput::Local((index - 1) as u32),
                    DenseDepsetInput::Direct(format!("chain-{index}")),
                ]),
                depth: index + 2,
                metadata: (),
            });
        }
        let (dense, construction) = Depset::from_dense_nodes_counted(inputs).unwrap();
        let dense_root = dense.last().unwrap();

        let mut legacy = Arc::new(LegacyNode {
            order: DepsetOrder::Default,
            successors: vec![
                LegacySuccessor::Direct("chain-0-a".to_owned()),
                LegacySuccessor::Direct("chain-0-b".to_owned()),
            ]
            .into(),
            depth: 2,
        });
        for index in 1..256 {
            legacy = Arc::new(LegacyNode {
                order: DepsetOrder::Default,
                successors: vec![
                    LegacySuccessor::Transitive(legacy),
                    LegacySuccessor::Direct(format!("chain-{index}")),
                ]
                .into(),
                depth: index + 2,
            });
        }
        let (legacy_allocations, legacy_bytes, legacy_construction, legacy_traversal) =
            legacy_stats(&legacy);
        let dense_stats = dense_root.storage_stats();
        let (cold, cold_stats) = dense_root.to_list_counted();
        let (warm, warm_stats) = dense_root.to_list_counted();
        assert!(dense_stats.allocation_objects < legacy_allocations);
        assert!(dense_stats.estimated_bytes < legacy_bytes);
        assert_eq!(construction.operations(), 768);
        assert_eq!(cold_stats.operations(), 1_026);
        assert_eq!(warm_stats.operations(), 1_026);
        assert!(construction.operations() <= legacy_construction + legacy_construction / 10);
        assert!(cold_stats.operations() <= legacy_traversal + legacy_traversal / 10);
        assert!(warm_stats.operations() <= legacy_traversal + legacy_traversal / 10);
        assert_eq!(cold.len(), 257);
        assert_eq!(warm, cold);
    }
}
