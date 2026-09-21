//! Owned publication of native C++ variables. Live evaluator values and caches
//! are lowered by Analysis; no evaluator heap or mutable lookup state is retained.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use crate::AnalysisValue;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum CcVariablesKind {
    Map,
    Single,
}

/// Identity is observable when a node becomes another node's parent. It is not
/// the node's own value equality and never supplies semantic DICE identity bytes.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcVariablesOccurrence(Arc<()>);
impl CcVariablesOccurrence {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
    pub fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl Default for CcVariablesOccurrence {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for CcVariablesOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CcVariablesOccurrence {}
impl Hash for CcVariablesOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}

/// Sequence entries snapshot only the sequence, retaining the raw elements;
/// Value entries may contain a live structure before normal publication.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum CcVariableCacheEntry<V> {
    Value(V),
    Sequence(Box<[V]>),
    Missing,
    Error(CompactString),
}
impl<V> CcVariableCacheEntry<V> {
    pub fn try_map<U, E>(
        &self,
        mut map: impl FnMut(&V) -> Result<U, E>,
    ) -> Result<CcVariableCacheEntry<U>, E> {
        Ok(match self {
            Self::Value(value) => CcVariableCacheEntry::Value(map(value)?),
            Self::Sequence(values) => CcVariableCacheEntry::Sequence(
                values.iter().map(map).collect::<Result<Box<[_]>, _>>()?,
            ),
            Self::Missing => CcVariableCacheEntry::Missing,
            Self::Error(message) => CcVariableCacheEntry::Error(message.clone()),
        })
    }
    pub fn values(&self) -> &[V] {
        match self {
            Self::Value(value) => std::slice::from_ref(value),
            Self::Sequence(values) => values,
            Self::Missing | Self::Error(_) => &[],
        }
    }
}

#[derive(Debug, Allocative)]
pub struct CcVariablesModel {
    pub kind: CcVariablesKind,
    pub occurrence: CcVariablesOccurrence,
    pub parent: Option<CcToolchainVariables>,
    pub locals: Box<[(CompactString, AnalysisValue)]>,
    pub cache: Box<[(CompactString, CcVariableCacheEntry<AnalysisValue>)]>,
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcToolchainVariables(Arc<CcVariablesModel>);
impl CcToolchainVariables {
    pub fn new(mut model: CcVariablesModel) -> Self {
        // Native MapVariables sorts keys with Java String ordering. Cache order
        // is not observable, but canonical order simplifies structural comparison.
        model
            .locals
            .sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        model
            .cache
            .sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        Self(Arc::new(model))
    }
    pub fn model(&self) -> &CcVariablesModel {
        &self.0
    }
    pub fn occurrence(&self) -> &CcVariablesOccurrence {
        &self.0.occurrence
    }
    pub fn is_hashable(&self) -> bool {
        self.0.kind == CcVariablesKind::Map
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let (left, right) = (self.model(), other.model());
        match state.enter_cc_variables_pair(left.occurrence.pointer(), right.occurrence.pointer()) {
            Err(()) => return false,
            Ok(false) => return true,
            Ok(true) => {}
        }
        left.kind == right.kind
            && match (&left.parent, &right.parent) {
                (Some(left), Some(right)) => left.publication_eq_with(right, state),
                (None, None) => true,
                _ => false,
            }
            && left.locals.len() == right.locals.len()
            && left
                .locals
                .iter()
                .zip(right.locals.iter())
                .all(|((ln, lv), (rn, rv))| ln == rn && lv.publication_eq_with(rv, state))
            && left.cache.len() == right.cache.len()
            && left
                .cache
                .iter()
                .zip(right.cache.iter())
                .all(|((ln, lv), (rn, rv))| ln == rn && cache_publication_eq(lv, rv, state))
    }
}

fn cache_publication_eq(
    left: &CcVariableCacheEntry<AnalysisValue>,
    right: &CcVariableCacheEntry<AnalysisValue>,
    state: &mut PublicationEqState,
) -> bool {
    match (left, right) {
        (CcVariableCacheEntry::Value(left), CcVariableCacheEntry::Value(right)) => {
            left.publication_eq_with(right, state)
        }
        (CcVariableCacheEntry::Sequence(left), CcVariableCacheEntry::Sequence(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(l, r)| l.publication_eq_with(r, state))
        }
        (CcVariableCacheEntry::Missing, CcVariableCacheEntry::Missing) => true,
        (CcVariableCacheEntry::Error(left), CcVariableCacheEntry::Error(right)) => left == right,
        _ => false,
    }
}

impl PartialEq for CcToolchainVariables {
    fn eq(&self, other: &Self) -> bool {
        let (left, right) = (self.model(), other.model());
        left.kind == right.kind
            && left.parent.as_ref().map(Self::occurrence)
                == right.parent.as_ref().map(Self::occurrence)
            && left.locals == right.locals
    }
}
impl Eq for CcToolchainVariables {}
impl Hash for CcToolchainVariables {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.kind.hash(state);
        self.0.parent.as_ref().map(Self::occurrence).hash(state);
        // Raw payload hashing intentionally bypasses Starlark list/dict barriers.
        self.0.locals.hash(state);
    }
}
