//! Starlark-free deferred Args invocation and native input ownership.
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;

use crate::AnalysisArtifact;
use crate::AnalysisDepset;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::PublishedValueError;
use crate::PublishedValueRef;
use crate::analysis_value::PublicationEqState;
use crate::callable_graph::CALLABLE_GRAPH_BYTE_LIMIT;
use crate::callable_graph::CALLABLE_GRAPH_DEPTH_LIMIT;
use crate::callable_graph::CALLABLE_GRAPH_EDGE_LIMIT;
use crate::callable_graph::CALLABLE_GRAPH_NODE_LIMIT;
use crate::callable_graph::CallableGraph;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ArgsExpansionError {
    Unsupported(CompactString),
    Callback(CompactString),
    InvalidReturn(CompactString),
    Limit(CompactString),
    Cancelled,
}
impl fmt::Display for ArgsExpansionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(s) => write!(f, "Args capability unsupported: {s}"),
            Self::Callback(s) => write!(f, "Args callback: {s}"),
            Self::InvalidReturn(s) => write!(f, "Args callback return: {s}"),
            Self::Limit(s) => write!(f, "Args expansion limit: {s}"),
            Self::Cancelled => f.write_str("Args expansion cancelled"),
        }
    }
}
impl std::error::Error for ArgsExpansionError {}

/// A supplied semantic capability, never ambient directory discovery. Query
/// consumers have no metadata; execution consumers may supply tracked children.
pub trait ArgsDirectoryContext: fmt::Debug + Send + Sync {
    /// Cancellation belongs to the current expansion consumer, not the analysis
    /// request that originally published the retained mapper.
    fn is_cancelled(&self) -> bool {
        false
    }
    fn list(
        &self,
        artifact: &AnalysisArtifact,
    ) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError>;
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopArgsDirectoryContext;
impl ArgsDirectoryContext for NoopArgsDirectoryContext {
    fn list(
        &self,
        artifact: &AnalysisArtifact,
    ) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        Ok(vec![artifact.clone()])
    }
}

/// Execution without tracked tree children is an explicit capability refusal.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedTreeArgsDirectoryContext;
impl ArgsDirectoryContext for UnsupportedTreeArgsDirectoryContext {
    fn list(
        &self,
        artifact: &AnalysisArtifact,
    ) -> Result<Vec<AnalysisArtifact>, ArgsExpansionError> {
        if matches!(artifact, AnalysisArtifact::Derived { output, .. } if output.kind() != crate::ActionOutputKind::File && output.kind() != crate::ActionOutputKind::Symlink)
        {
            return Err(ArgsExpansionError::Unsupported(
                "execution directory expansion requires tracked tree metadata".into(),
            ));
        }
        Ok(vec![artifact.clone()])
    }
}

/// One fresh transient evaluator per batch. No evaluator, producer action/ctx,
/// locks or lookup table is exposed through this native interface.
pub trait RuntimeMapper: fmt::Debug + Send + Sync {
    fn map_batch(
        &self,
        values: &[AnalysisValue],
        directory_context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError>;
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedArgsMapper(Arc<RetainedArgsMapperData>);
#[derive(Debug, Allocative)]
struct RetainedArgsMapperData {
    callable: PublishedValueRef,
    // The foreign executable heap has its own lifetime/accounting owner. Only
    // the native descriptor participates in semantic equality.
    #[allocative(skip)]
    runtime: Arc<dyn RuntimeMapper>,
}
impl RetainedArgsMapper {
    pub fn from_published(
        callable: PublishedValueRef,
        runtime: Arc<dyn RuntimeMapper>,
    ) -> Result<Self, PublishedValueError> {
        callable.arena().validate_root(callable.node_id())?;
        if callable.original_slot().is_none() {
            return Err(PublishedValueError::Invalid("mapper original root"));
        }
        Ok(Self(Arc::new(RetainedArgsMapperData { callable, runtime })))
    }
    pub fn published_callable(&self) -> &PublishedValueRef {
        &self.0.callable
    }
    pub fn graph(&self) -> &CallableGraph {
        self.0.callable.arena()
    }
    pub fn root(&self) -> crate::PublishedNodeId {
        self.0.callable.node_id()
    }
    pub fn map_batch(
        &self,
        values: &[AnalysisValue],
        context: &dyn ArgsDirectoryContext,
    ) -> Result<Vec<CompactString>, ArgsExpansionError> {
        self.0.runtime.map_batch(values, context)
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0
            .callable
            .publication_eq_with(&other.0.callable, state)
    }
}
impl PartialEq for RetainedArgsMapper {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for RetainedArgsMapper {}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedArgsInputs {
    /// Membership snapshotted at registration; actual values lowered together
    /// with callable captures and providers after the evaluator closes.
    Sequence(Arc<[AnalysisValue]>),
    Depset(AnalysisDepset),
}
impl RetainedArgsInputs {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Sequence(s) => s.is_empty(),
            Self::Depset(s) => s.is_empty(),
        }
    }
    pub fn values(&self) -> Result<Vec<AnalysisValue>, ArgsExpansionError> {
        let mut values = Vec::new();
        let mut budget = NativeArgsBudget::default();
        let mut add = |value: &AnalysisValue| {
            if values.len() >= CALLABLE_GRAPH_NODE_LIMIT {
                return Err(ArgsExpansionError::Limit("input count".into()));
            }
            budget.audit(value)?;
            values.push(value.clone());
            Ok(())
        };
        match self {
            Self::Sequence(s) => {
                for value in &**s {
                    add(value)?;
                }
            }
            Self::Depset(s) => s.visit(add)?,
        }
        Ok(values)
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Sequence(l), Self::Sequence(r)) => {
                l.len() == r.len()
                    && l.iter()
                        .zip(r.iter())
                        .all(|(l, r)| l.publication_eq_with(r, state))
            }
            (Self::Depset(l), Self::Depset(r)) => l.publication_eq_with(r, state),
            _ => false,
        }
    }
}
impl PartialEq for RetainedArgsInputs {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for RetainedArgsInputs {}

/// Scratch audit before native input copies; nested values remain cheap owned
/// native handles. No callback or arbitrary representation participates.
#[derive(Debug, Default)]
pub(crate) struct NativeArgsBudget {
    visits: usize,
    bytes: usize,
    arenas: fxhash::FxHashSet<crate::PublishedArenaIdentity>,
}
impl NativeArgsBudget {
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn audit(&mut self, value: &AnalysisValue) -> Result<(), ArgsExpansionError> {
        self.audit_at(value, 0)
    }
    fn audit_at(
        &mut self,
        value: &AnalysisValue,
        initial_depth: usize,
    ) -> Result<(), ArgsExpansionError> {
        let mut stack = vec![(value, initial_depth)];
        while let Some((value, depth)) = stack.pop() {
            self.visits = self
                .visits
                .checked_add(1)
                .ok_or_else(|| ArgsExpansionError::Limit("native input visits".into()))?;
            if self.visits > CALLABLE_GRAPH_EDGE_LIMIT || depth > CALLABLE_GRAPH_DEPTH_LIMIT {
                return Err(ArgsExpansionError::Limit("native input graph".into()));
            }
            let mut bytes = std::mem::size_of::<AnalysisValue>();
            if let Some(origin) = value.original_origin() {
                if origin.original_slot().is_none() {
                    return Err(ArgsExpansionError::Unsupported(
                        "input original slot missing".into(),
                    ));
                }
                if self.arenas.insert(origin.arena().identity()) {
                    let shape = origin.arena().shape();
                    self.visits = self
                        .visits
                        .checked_add(shape.visits)
                        .ok_or_else(|| ArgsExpansionError::Limit("native arena visits".into()))?;
                    if self.visits > CALLABLE_GRAPH_NODE_LIMIT
                        || shape.edges > CALLABLE_GRAPH_EDGE_LIMIT
                    {
                        return Err(ArgsExpansionError::Limit("native arena graph".into()));
                    }
                    bytes = bytes
                        .checked_add(shape.bytes)
                        .ok_or_else(|| ArgsExpansionError::Limit("native arena bytes".into()))?;
                }
                self.bytes = self
                    .bytes
                    .checked_add(bytes)
                    .filter(|n| *n <= CALLABLE_GRAPH_BYTE_LIMIT)
                    .ok_or_else(|| ArgsExpansionError::Limit("native input bytes".into()))?;
                continue;
            }
            match value.kind() {
                AnalysisValueKind::DottedVersion(v) => {
                    if depth
                        .checked_add(2)
                        .is_none_or(|n| n > CALLABLE_GRAPH_DEPTH_LIMIT)
                    {
                        return Err(ArgsExpansionError::Limit(
                            "native dotted version depth".into(),
                        ));
                    }
                    self.visits = self
                        .visits
                        .checked_add(1)
                        .and_then(|n| n.checked_add(v.components().len()))
                        .filter(|n| *n <= CALLABLE_GRAPH_NODE_LIMIT)
                        .ok_or_else(|| {
                            ArgsExpansionError::Limit("native dotted version work".into())
                        })?;
                    bytes = bytes
                        .checked_add(crate::analysis_value::published_projection_forecast())
                        .and_then(|n| n.checked_add(v.storage().retained_bytes()))
                        .ok_or_else(|| {
                            ArgsExpansionError::Limit("native dotted version bytes".into())
                        })?;
                }
                AnalysisValueKind::String(s) => bytes = bytes.saturating_add(s.len()),
                AnalysisValueKind::Number(n) => {
                    if let Some(integer) = n.as_integer() {
                        bytes = bytes.saturating_add(integer.magnitude().len());
                    }
                }
                AnalysisValueKind::List(s) | AnalysisValueKind::Tuple(s) => {
                    if stack.len().saturating_add(s.len()) > CALLABLE_GRAPH_EDGE_LIMIT {
                        return Err(ArgsExpansionError::Limit("native input edges".into()));
                    }
                    for child in s {
                        stack.push((child, depth + 1));
                    }
                }
                AnalysisValueKind::Dictionary(s) => {
                    if stack.len().saturating_add(s.len().saturating_mul(2))
                        > CALLABLE_GRAPH_EDGE_LIMIT
                    {
                        return Err(ArgsExpansionError::Limit("native input edges".into()));
                    }
                    for (k, v) in s {
                        stack.push((k, depth + 1));
                        stack.push((v, depth + 1));
                    }
                }
                AnalysisValueKind::Struct(s) => {
                    if stack.len().saturating_add(s.len()) > CALLABLE_GRAPH_EDGE_LIMIT {
                        return Err(ArgsExpansionError::Limit("native input edges".into()));
                    }
                    for (name, v) in s.iter() {
                        bytes = bytes.saturating_add(name.len());
                        stack.push((v, depth + 1));
                    }
                }
                AnalysisValueKind::Provider(p) => {
                    if stack.len().saturating_add(p.fields().len()) > CALLABLE_GRAPH_EDGE_LIMIT {
                        return Err(ArgsExpansionError::Limit("native input edges".into()));
                    }
                    for (name, v) in p.fields().iter() {
                        bytes = bytes.saturating_add(name.len());
                        stack.push((v, depth + 1));
                    }
                }
                AnalysisValueKind::Depset(set) => {
                    use crate::DepsetStorageEvent;
                    if depth.saturating_add(set.depth()) > CALLABLE_GRAPH_DEPTH_LIMIT {
                        return Err(ArgsExpansionError::Limit("native depset depth".into()));
                    }
                    set.visit_storage_bounded(|event| match event {
                        DepsetStorageEvent::Charge(c) => {
                            self.visits = self
                                .visits
                                .checked_add(c.visits)
                                .filter(|n| *n <= CALLABLE_GRAPH_NODE_LIMIT)
                                .ok_or_else(|| {
                                    ArgsExpansionError::Limit("native depset work".into())
                                })?;
                            self.bytes = self
                                .bytes
                                .checked_add(c.bytes)
                                .filter(|n| *n <= CALLABLE_GRAPH_BYTE_LIMIT)
                                .ok_or_else(|| {
                                    ArgsExpansionError::Limit("native depset bytes".into())
                                })?;
                            if c.nodes > CALLABLE_GRAPH_NODE_LIMIT
                                || c.edges > CALLABLE_GRAPH_EDGE_LIMIT
                            {
                                return Err(ArgsExpansionError::Limit(
                                    "native depset graph".into(),
                                ));
                            }
                            Ok(())
                        }
                        DepsetStorageEvent::Node {
                            depth: node_depth, ..
                        } => {
                            if depth.saturating_add(node_depth) > CALLABLE_GRAPH_DEPTH_LIMIT {
                                Err(ArgsExpansionError::Limit("native depset depth".into()))
                            } else {
                                Ok(())
                            }
                        }
                        DepsetStorageEvent::Value {
                            value,
                            depth: node_depth,
                        } => self.audit_at(value, depth.saturating_add(node_depth)),
                    })?;
                }
                AnalysisValueKind::ConfiguredTarget(target) => {
                    for (_, provider) in target.providers().iter() {
                        if let crate::ProviderValue::Occurrence(p) = provider {
                            for (name, v) in p.fields().iter() {
                                bytes = bytes.saturating_add(name.len());
                                self.audit_at(v, depth + 1)?;
                            }
                        }
                    }
                }
                AnalysisValueKind::RunEnvironmentInfo(_) => {
                    return Err(ArgsExpansionError::Unsupported(
                        "RunEnvironmentInfo requires a borrowed Args metadata audit".into(),
                    ));
                }
                AnalysisValueKind::Function(_)
                | AnalysisValueKind::ProviderConstructor(_)
                | AnalysisValueKind::RawProviderConstructor(_)
                | AnalysisValueKind::Runfiles(_) => {
                    return Err(ArgsExpansionError::Unsupported(
                        "callable/runfiles native input requires original arena admission".into(),
                    ));
                }
                _ => {}
            }
            if stack.len() > CALLABLE_GRAPH_EDGE_LIMIT {
                return Err(ArgsExpansionError::Limit("native input edges".into()));
            }
            self.bytes = self
                .bytes
                .checked_add(bytes)
                .ok_or_else(|| ArgsExpansionError::Limit("native input bytes".into()))?;
            if self.bytes > CALLABLE_GRAPH_BYTE_LIMIT {
                return Err(ArgsExpansionError::Limit("native input bytes".into()));
            }
        }
        Ok(())
    }
}

/// Compare all publication roots with one alias bijection. The caller verifies
/// pair count, execution context and all remaining result fields before calling.
pub fn publication_eq_providers_and_actions<'a>(
    left: &crate::ProviderCollection,
    right: &crate::ProviderCollection,
    pairs: impl IntoIterator<Item = (&'a crate::ActionSpec, &'a crate::ActionSpec)>,
) -> bool {
    let mut state = PublicationEqState::default();
    left.publication_eq_with(right, &mut state)
        && pairs
            .into_iter()
            .all(|(l, r)| l.publication_eq_with(r, &mut state))
}

#[cfg(test)]
mod tests;
