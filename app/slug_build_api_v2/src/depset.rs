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
        }
    }
}

impl Error for DepsetError {}
#[derive(Debug, Eq, PartialEq, Allocative)]
pub struct Depset<T, M = ()> {
    node: Arc<DepsetNode<T, M>>,
}

impl<T, M> Clone for Depset<T, M> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
        }
    }
}
impl<T, M> Dupe for Depset<T, M> {}

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

pub fn traverse_depset<N, E>(
    root: &N,
    mut hash: impl FnMut(&N::Item) -> Result<u64, E>,
    mut equals: impl FnMut(&N::Item, &N::Item) -> Result<bool, E>,
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
        match successor {
            DepsetSuccessor::Direct(item) => {
                if seen_values.insert(item.clone(), &mut hash, &mut equals)? {
                    out.push(item);
                }
            }
            DepsetSuccessor::Transitive(node) if seen_nodes.insert(node.node_key()) => {
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

#[derive(Debug, Eq, PartialEq, Allocative)]
struct DepsetNode<T, M> {
    order: DepsetOrder,
    successors: Arc<[DepsetSuccessor<T, Depset<T, M>>]>,
    depth: usize,
    metadata: M,
}

impl<T> Depset<T> {
    pub fn empty() -> Self {
        Self::from_canonical_successors(DepsetOrder::Default, Vec::new(), 0, ())
    }
}

impl<T, M> Depset<T, M> {
    pub fn order(&self) -> DepsetOrder {
        self.node.order
    }

    pub fn successors(&self) -> &[DepsetSuccessor<T, Depset<T, M>>] {
        &self.node.successors
    }

    pub fn depth(&self) -> usize {
        self.node.depth
    }

    pub fn is_empty(&self) -> bool {
        self.node.depth == 0
    }

    pub fn shares_node_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.node, &other.node)
    }

    pub fn shares_successors_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.node.successors, &other.node.successors)
    }

    pub fn node_key(&self) -> usize {
        Arc::as_ptr(&self.node) as usize
    }

    pub fn metadata(&self) -> &M {
        &self.node.metadata
    }

    pub(crate) fn from_canonical_successors(
        order: DepsetOrder,
        successors: Vec<DepsetSuccessor<T, Depset<T, M>>>,
        depth: usize,
        metadata: M,
    ) -> Self {
        Self {
            node: Arc::new(DepsetNode {
                order,
                successors: successors.into(),
                depth,
                metadata,
            }),
        }
    }

    pub(crate) fn rewrap(order: DepsetOrder, child: &Self, metadata: M) -> Self {
        Self {
            node: Arc::new(DepsetNode {
                order,
                successors: child.node.successors.clone(),
                depth: child.depth(),
                metadata,
            }),
        }
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
            DepsetBuild::Empty => Self::from_canonical_successors(order, Vec::new(), 0, metadata),
            DepsetBuild::Reuse(value) => value,
            DepsetBuild::Dereference(child) => Self::rewrap(order, &child, metadata),
            DepsetBuild::Node(successors, depth) => {
                Self::from_canonical_successors(order, successors, depth, metadata)
            }
        })
    }

    pub fn to_list(&self) -> Vec<T> {
        traverse_depset(
            self,
            |item| Ok::<_, Infallible>(hash64(item)),
            |left, right| Ok::<_, Infallible>(left == right),
        )
        .unwrap_or_else(|never| match never {})
    }
}

impl<T: Clone, M> DepsetView for Depset<T, M> {
    type Item = T;
    type NodeKey = usize;

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
        match self.node.successors.as_ref() {
            [DepsetSuccessor::Direct(value)] => Some(value.clone()),
            _ => None,
        }
    }

    fn for_each_successor_reverse<E>(
        &self,
        mut visitor: impl FnMut(DepsetSuccessor<Self::Item, Self>) -> Result<(), E>,
    ) -> Result<(), E> {
        for successor in self.node.successors.iter().rev() {
            visitor(successor.clone())?;
        }
        Ok(())
    }
}
