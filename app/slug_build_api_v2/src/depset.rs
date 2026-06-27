/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

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
    order: DepsetOrder,
    direct: Vec<T>,
    transitive: Vec<Depset<T>>,
    depth: usize,
}

impl<T> Depset<T> {
    pub fn empty() -> Self {
        Self {
            order: DepsetOrder::Default,
            direct: Vec::new(),
            transitive: Vec::new(),
            depth: 0,
        }
    }

    pub fn new(
        order: DepsetOrder,
        direct: Vec<T>,
        transitive: Vec<Depset<T>>,
    ) -> Result<Self, DepsetError> {
        for child in &transitive {
            if !order.compatible_with(child.order) {
                return Err(DepsetError::IncompatibleOrder {
                    parent: order,
                    child: child.order,
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
            order,
            direct,
            transitive,
            depth,
        })
    }

    pub fn from_direct(order: DepsetOrder, direct: Vec<T>) -> Result<Self, DepsetError> {
        Self::new(order, direct, Vec::new())
    }

    pub fn order(&self) -> DepsetOrder {
        self.order
    }

    pub fn direct(&self) -> &[T] {
        &self.direct
    }

    pub fn transitive(&self) -> &[Depset<T>] {
        &self.transitive
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.transitive.iter().all(Depset::is_empty)
    }
}

impl<T> Depset<T>
where
    T: Clone + Eq + Hash,
{
    pub fn to_list(&self) -> Vec<T> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        match self.order {
            DepsetOrder::Default | DepsetOrder::Postorder => {
                self.visit_postorder(&mut out, &mut seen)
            }
            DepsetOrder::Preorder => self.visit_preorder(&mut out, &mut seen),
            DepsetOrder::Topological => self.visit_topological(&mut out, &mut seen),
        }
        out
    }

    fn push_direct(&self, out: &mut Vec<T>, seen: &mut HashSet<T>) {
        for item in &self.direct {
            if seen.insert(item.clone()) {
                out.push(item.clone());
            }
        }
    }

    fn visit_postorder(&self, out: &mut Vec<T>, seen: &mut HashSet<T>) {
        for child in &self.transitive {
            child.visit_postorder(out, seen);
        }
        self.push_direct(out, seen);
    }

    fn visit_preorder(&self, out: &mut Vec<T>, seen: &mut HashSet<T>) {
        self.push_direct(out, seen);
        for child in &self.transitive {
            child.visit_preorder(out, seen);
        }
    }

    fn visit_topological(&self, out: &mut Vec<T>, seen: &mut HashSet<T>) {
        self.push_direct(out, seen);
        self.visit_topological_children(out, seen);
    }

    fn visit_topological_children(&self, out: &mut Vec<T>, seen: &mut HashSet<T>) {
        for child in &self.transitive {
            child.push_direct(out, seen);
        }
        for child in &self.transitive {
            child.visit_topological_children(out, seen);
        }
    }
}
