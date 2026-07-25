/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::any::Any;
use std::fmt;

use dupe::Dupe;

use crate::DynKey;
use crate::VersionNumber;

/// Opaque identity for a node in one DICE engine.
#[derive(
    allocative::Allocative,
    Clone,
    Copy,
    Debug,
    Dupe,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd
)]
pub struct DiceNodeId {
    engine: u64,
    node: u32,
}

impl DiceNodeId {
    pub(crate) fn new(engine: u64, node: u32) -> Self {
        Self { engine, node }
    }

    pub(crate) fn engine(self) -> u64 {
        self.engine
    }

    pub(crate) fn node(self) -> u32 {
        self.node
    }
}

/// Whether an activated node was evaluated or reused.
#[derive(allocative::Allocative, Clone, Copy, Debug, Dupe, Eq, PartialEq)]
pub enum ActivationKind {
    Evaluated,
    Reused,
}

/// Borrowed metadata for the rich activation callback.
pub struct RichActivation<'a> {
    node: DiceNodeId,
    version: VersionNumber,
    kind: ActivationKind,
    evaluation_data: Option<&'a (dyn Any + Send + Sync + 'static)>,
    dependencies: &'a [DiceNodeId],
}

impl<'a> RichActivation<'a> {
    pub(crate) fn new(
        node: DiceNodeId,
        version: VersionNumber,
        kind: ActivationKind,
        evaluation_data: Option<&'a (dyn Any + Send + Sync + 'static)>,
        dependencies: &'a [DiceNodeId],
    ) -> Self {
        Self {
            node,
            version,
            kind,
            evaluation_data,
            dependencies,
        }
    }

    pub fn node(&self) -> DiceNodeId {
        self.node
    }

    pub fn version(&self) -> VersionNumber {
        self.version
    }

    pub fn kind(&self) -> ActivationKind {
        self.kind
    }

    pub fn evaluation_data(&self) -> Option<&(dyn Any + Send + Sync + 'static)> {
        self.evaluation_data
    }

    pub fn dependencies(&self) -> &[DiceNodeId] {
        self.dependencies
    }
}

/// Metadata for a parentless transaction request.
#[derive(allocative::Allocative, Clone, Copy, Debug, Dupe, Eq, PartialEq)]
pub struct RootActivation {
    node: DiceNodeId,
    version: VersionNumber,
    ordinal: u64,
}

impl RootActivation {
    pub(crate) fn new(node: DiceNodeId, version: VersionNumber, ordinal: u64) -> Self {
        Self {
            node,
            version,
            ordinal,
        }
    }

    pub fn node(&self) -> DiceNodeId {
        self.node
    }

    pub fn version(&self) -> VersionNumber {
        self.version
    }

    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}

/// One node in an exact-version activation closure.
#[derive(allocative::Allocative, Clone, Debug, Dupe, Eq, PartialEq)]
pub struct ActivationClosureNode {
    node: DiceNodeId,
    dependencies: std::sync::Arc<[DiceNodeId]>,
}

impl ActivationClosureNode {
    pub(crate) fn new(node: DiceNodeId, dependencies: Vec<DiceNodeId>) -> Self {
        Self {
            node,
            dependencies: dependencies.into(),
        }
    }

    pub fn node(&self) -> DiceNodeId {
        self.node
    }

    pub fn dependencies(&self) -> &[DiceNodeId] {
        &self.dependencies
    }
}

/// Read-only, dependency-first activation graph at one exact DICE version.
#[derive(allocative::Allocative, Clone, Debug, Dupe, Eq, PartialEq)]
pub struct ActivationClosure {
    version: VersionNumber,
    roots: std::sync::Arc<[DiceNodeId]>,
    nodes: std::sync::Arc<[ActivationClosureNode]>,
}

impl ActivationClosure {
    pub(crate) fn new(
        version: VersionNumber,
        roots: Vec<DiceNodeId>,
        nodes: Vec<ActivationClosureNode>,
    ) -> Self {
        Self {
            version,
            roots: roots.into(),
            nodes: nodes.into(),
        }
    }

    pub fn version(&self) -> VersionNumber {
        self.version
    }

    /// Ordered roots exactly as supplied by the caller, including duplicates.
    pub fn roots(&self) -> &[DiceNodeId] {
        &self.roots
    }

    /// Dependency-before-parent nodes, deduplicated at first encounter.
    pub fn nodes(&self) -> &[ActivationClosureNode] {
        &self.nodes
    }
}

/// Exact-version closure lookup failures.
#[derive(allocative::Allocative, Clone, Copy, Debug, Dupe, Eq, PartialEq)]
#[non_exhaustive]
pub enum ActivationClosureError {
    ForeignEngine {
        node: DiceNodeId,
    },
    UnavailableRoot {
        root: DiceNodeId,
    },
    UnavailableNode {
        node: DiceNodeId,
    },
    Dirty {
        node: DiceNodeId,
        version: VersionNumber,
    },
    NotVerified {
        node: DiceNodeId,
        version: VersionNumber,
    },
    Cycle {
        node: DiceNodeId,
    },
}

impl fmt::Display for ActivationClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignEngine { node } => write!(f, "foreign DICE node ID: {node:?}"),
            Self::UnavailableRoot { root } => {
                write!(f, "unavailable root DICE node ID: {root:?}")
            }
            Self::UnavailableNode { node } => write!(f, "unavailable DICE node ID: {node:?}"),
            Self::Dirty { node, version } => {
                write!(f, "DICE node {node:?} is dirty at {version}")
            }
            Self::NotVerified { node, version } => {
                write!(f, "DICE node {node:?} is not verified at {version}")
            }
            Self::Cycle { node } => write!(f, "cycle in activation provenance at {node:?}"),
        }
    }
}

impl std::error::Error for ActivationClosureError {}

/// An ActivationTracker can be used to identify which keys were either reused or computed during a
/// transaction.
pub trait ActivationTracker: Send + Sync + 'static {
    /// Receives when a key was activated (computed, or reused). The caller will want to downcast
    /// the key and deps to types they care about. The caller also receives whatever the key passed
    /// to `store_evaluation_data` (if any).
    fn key_activated(
        &self,
        key: &DynKey,
        deps: &mut dyn Iterator<Item = &DynKey>,
        activation_data: ActivationData,
    );

    /// Whether this tracker consumes rich per-node activation callbacks.
    ///
    /// Keeping this false avoids exact-dependency reads for legacy-only trackers.
    fn tracks_rich_activations(&self) -> bool {
        false
    }

    /// Receives a rich activation without changing the legacy callback contract.
    fn key_activated_rich(&self, _key: &DynKey, _activation: RichActivation<'_>) {}

    /// Receives each parentless request after indexing and before cache lookup.
    fn root_activated(&self, _key: &DynKey, _activation: RootActivation) {}
}

/// Describes the kind of activation, and possibly carries data passed by the key's evaluation.
pub enum ActivationData {
    /// This key was evaluated. Evaluation data will be passed if the key's evaluation set any.
    Evaluated(Option<Box<dyn Any + Send + Sync + 'static>>),

    /// This key was reused. No data is passed.
    Reused,
}

impl ActivationData {
    pub(crate) fn kind(&self) -> ActivationKind {
        match self {
            Self::Evaluated(_) => ActivationKind::Evaluated,
            Self::Reused => ActivationKind::Reused,
        }
    }

    pub(crate) fn evaluation_data(&self) -> Option<&(dyn Any + Send + Sync + 'static)> {
        match self {
            Self::Evaluated(data) => data.as_deref(),
            Self::Reused => None,
        }
    }
}
