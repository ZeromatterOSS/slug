/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

use fxhash::FxHashSet;

pub const MAX_DEPTH: usize = 3500;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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

    fn compatible_with(self, child: Self) -> bool {
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Depset<T> {
    node: Arc<DepsetNode<T>>,
}

/// Immutable shared node for Bazel depsets.
///
/// Composition only clones `Arc` pointers to child nodes. Flattening is the
/// explicit consuming operation in [`Depset::to_list`], matching the V1
/// nested-set traversal lesson without retaining its Buck-facing types.
#[derive(Debug, Eq, PartialEq)]
struct DepsetNode<T> {
    order: DepsetOrder,
    direct: Arc<[T]>,
    transitive: Arc<[Depset<T>]>,
    depth: usize,
}

impl<T> Depset<T> {
    pub fn empty() -> Self {
        Self {
            node: Arc::new(DepsetNode {
                order: DepsetOrder::Default,
                direct: Arc::from([]),
                transitive: Arc::from([]),
                depth: 0,
            }),
        }
    }

    pub fn new(
        order: DepsetOrder,
        direct: Vec<T>,
        transitive: Vec<Depset<T>>,
    ) -> Result<Self, DepsetError> {
        for child in &transitive {
            if !order.compatible_with(child.order()) {
                return Err(DepsetError::IncompatibleOrder {
                    parent: order,
                    child: child.order(),
                });
            }
        }

        let max_child_depth = transitive
            .iter()
            .map(Depset::depth)
            .max()
            .unwrap_or_default();
        let depth = if direct.is_empty() {
            max_child_depth
        } else {
            max_child_depth + 1
        };
        if depth > MAX_DEPTH {
            return Err(DepsetError::DepthLimitExceeded {
                depth,
                max: MAX_DEPTH,
            });
        }

        Ok(Self {
            node: Arc::new(DepsetNode {
                order,
                direct: Arc::from(direct),
                transitive: Arc::from(transitive),
                depth,
            }),
        })
    }

    pub fn from_direct(order: DepsetOrder, direct: Vec<T>) -> Result<Self, DepsetError> {
        Self::new(order, direct, Vec::new())
    }

    pub fn order(&self) -> DepsetOrder {
        self.node.order
    }

    pub fn direct(&self) -> &[T] {
        &self.node.direct
    }

    pub fn transitive(&self) -> &[Depset<T>] {
        &self.node.transitive
    }

    pub fn depth(&self) -> usize {
        self.node.depth
    }

    pub fn is_empty(&self) -> bool {
        self.node.direct.is_empty() && self.node.transitive.iter().all(Depset::is_empty)
    }

    /// Whether two handles retain the same immutable nested-set node.
    /// This is a structural performance invariant, not Bazel-visible equality.
    pub fn shares_node_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.node, &other.node)
    }
}

impl<T> Depset<T>
where
    T: Clone + Eq + Hash,
{
    pub fn to_list(&self) -> Vec<T> {
        let mut out = Vec::new();
        let mut seen = FxHashSet::default();
        match self.order() {
            DepsetOrder::Default | DepsetOrder::Postorder => {
                self.visit_postorder(&mut out, &mut seen)
            }
            DepsetOrder::Preorder => self.visit_preorder(&mut out, &mut seen),
            DepsetOrder::Topological => self.visit_topological(&mut out, &mut seen),
        }
        out
    }

    fn push_direct(&self, out: &mut Vec<T>, seen: &mut FxHashSet<T>) {
        for item in self.node.direct.iter() {
            if seen.insert(item.clone()) {
                out.push(item.clone());
            }
        }
    }

    fn visit_postorder(&self, out: &mut Vec<T>, seen: &mut FxHashSet<T>) {
        for child in self.node.transitive.iter() {
            child.visit_postorder(out, seen);
        }
        self.push_direct(out, seen);
    }

    fn visit_preorder(&self, out: &mut Vec<T>, seen: &mut FxHashSet<T>) {
        self.push_direct(out, seen);
        for child in self.node.transitive.iter() {
            child.visit_preorder(out, seen);
        }
    }

    fn visit_topological(&self, out: &mut Vec<T>, seen: &mut FxHashSet<T>) {
        self.push_direct(out, seen);
        self.visit_topological_children(out, seen);
    }

    fn visit_topological_children(&self, out: &mut Vec<T>, seen: &mut FxHashSet<T>) {
        for child in self.node.transitive.iter() {
            child.push_direct(out, seen);
        }
        for child in self.node.transitive.iter() {
            child.visit_topological_children(out, seen);
        }
    }
}
