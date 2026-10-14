//! Bounded native semantic graph for actual callable publication.
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use fxhash::FxHashSet;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ProviderIdentity;
use crate::analysis_value::PublicationEqState;

pub const CALLABLE_GRAPH_NODE_LIMIT: usize = 65_536;
pub const CALLABLE_GRAPH_EDGE_LIMIT: usize = 524_288;
pub const CALLABLE_GRAPH_BYTE_LIMIT: usize = 64 * 1024 * 1024;
pub const CALLABLE_GRAPH_DEPTH_LIMIT: usize = 256;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CallableGraphError {
    Limit(&'static str),
    InvalidNode(usize),
    InvalidEdges,
    UninitializedNode(usize),
    AlreadyInitializedNode(usize),
    InvalidMetadata(&'static str),
    UnsupportedNative(&'static str),
    NotCallableRoot,
}
impl fmt::Display for CallableGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "callable graph: {self:?}")
    }
}
impl std::error::Error for CallableGraphError {}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub struct CallableNodeId(u32);
impl CallableNodeId {
    pub fn new(index: usize) -> Result<Self, CallableGraphError> {
        if index >= CALLABLE_GRAPH_NODE_LIMIT {
            return Err(CallableGraphError::InvalidNode(index));
        }
        Ok(Self(index as u32))
    }
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub struct CallableEdgeRange {
    start: u32,
    len: u32,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CallableModuleIdentity {
    pub label: CanonicalLabel,
    pub workspace_path: PathBuf,
    pub repository_mapping: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CallableModuleSource {
    pub identity: CallableModuleIdentity,
    pub digest: [u8; 32],
}
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct CallableSourceAuthorityData {
    pub root: CallableModuleIdentity,
    pub direct_children: Arc<[CallableModuleIdentity]>,
    pub reachable: Arc<[CallableModuleSource]>,
    pub fingerprint: [u8; 32],
}
#[derive(Debug, Clone, Dupe, Eq, PartialEq, Allocative)]
pub struct CallableSourceAuthority(Arc<CallableSourceAuthorityData>);
impl CallableSourceAuthority {
    pub fn new(data: CallableSourceAuthorityData) -> Result<Self, CallableGraphError> {
        if data.reachable.len() > CALLABLE_GRAPH_NODE_LIMIT
            || data.direct_children.len() > CALLABLE_GRAPH_NODE_LIMIT
        {
            return Err(CallableGraphError::Limit("source facts"));
        }
        let result = Self(Arc::new(data));
        if result.byte_cost() > CALLABLE_GRAPH_BYTE_LIMIT {
            return Err(CallableGraphError::Limit("source bytes"));
        }
        Ok(result)
    }
    pub fn data(&self) -> &CallableSourceAuthorityData {
        &self.0
    }
    fn byte_cost(&self) -> usize {
        fn module(m: &CallableModuleIdentity) -> usize {
            m.label
                .package()
                .repo()
                .as_str()
                .len()
                .saturating_add(m.label.package().package().as_str().len())
                .saturating_add(m.label.target().as_str().len())
                + m.workspace_path.as_os_str().len()
                + m.repository_mapping
                    .iter()
                    .map(|(a, c)| a.as_str().len() + c.as_str().len())
                    .sum::<usize>()
        }
        module(&self.0.root)
            + self.0.direct_children.iter().map(module).sum::<usize>()
            + self
                .0
                .reachable
                .iter()
                .map(|s| module(&s.identity) + 32)
                .sum::<usize>()
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub struct CallableDefinitionSpan {
    pub start: u32,
    pub end: u32,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum CallableParameterMode {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    Args,
    Kwargs,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum CallableParameterRequirement {
    Required,
    Optional,
    Defaulted(CallableNodeId),
    Variadic,
}
#[derive(Debug, Clone, Allocative)]
pub struct CallableParameter {
    pub name: CompactString,
    pub mode: CallableParameterMode,
    pub requirement: CallableParameterRequirement,
    pub annotation: Option<CallableNodeId>,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative)]
pub enum CallableGlobalAccess {
    Read,
    Write,
    ReadWrite,
}
#[derive(Debug, Clone, Allocative)]
pub struct CallableGlobal {
    pub name: CompactString,
    pub access: CallableGlobalAccess,
    pub value: Option<CallableNodeId>,
}
#[derive(Debug, Clone, Allocative)]
pub struct CallableCode {
    pub source: CallableSourceAuthority,
    pub name: CompactString,
    pub definition: CallableDefinitionSpan,
    pub dependencies: CallableEdgeRange,
    pub globals: Arc<[CallableGlobal]>,
}
#[derive(Debug, Clone, Allocative)]
pub struct CallableFunction {
    pub code: CallableCode,
    pub parameters: Arc<[CallableParameter]>,
    pub return_annotation: Option<CallableNodeId>,
    pub capture_names: Arc<[CompactString]>,
    pub captures: CallableEdgeRange,
}

/// A host-certified canonical native API identity, never a callable name/repr
/// classifier. The Engine/host must validate the complete actual wrapper before
/// constructing this identity; evaluated parameter defaults remain graph edges.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct NativeCallableApiId {
    canonical: CompactString,
    version: u32,
}
impl NativeCallableApiId {
    pub fn new(canonical: &str, version: u32) -> Result<Self, CallableGraphError> {
        if version == 0
            || canonical.is_empty()
            || canonical.split('.').any(|part| {
                part.is_empty() || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            })
        {
            return Err(CallableGraphError::InvalidMetadata("native API identity"));
        }
        if canonical.len() > CALLABLE_GRAPH_BYTE_LIMIT {
            return Err(CallableGraphError::Limit("native API bytes"));
        }
        Ok(Self {
            canonical: canonical.into(),
            version,
        })
    }
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
    pub fn version(&self) -> u32 {
        self.version
    }
}
#[derive(Debug, Clone, Allocative)]
pub struct CallableIntrinsic {
    pub api: NativeCallableApiId,
    pub parameters: Arc<[CallableParameter]>,
    pub return_annotation: Option<CallableNodeId>,
    pub receiver: Option<CallableNodeId>,
}
#[derive(Debug, Clone, Allocative)]
pub enum CallableNode {
    Function(CallableFunction),
    Code(CallableCode),
    Cell {
        contents: Option<CallableNodeId>,
    },
    List(CallableEdgeRange),
    Tuple(CallableEdgeRange),
    Dict(CallableEdgeRange),
    Struct {
        names: Arc<[CompactString]>,
        values: CallableEdgeRange,
    },
    Provider {
        identity: ProviderIdentity,
        names: Arc<[CompactString]>,
        values: CallableEdgeRange,
        has_schema: bool,
    },
    Native(AnalysisValue),
    Intrinsic(CallableIntrinsic),
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CallableGraph(Arc<CallableGraphData>);
#[derive(Debug, Allocative)]
struct CallableGraphData {
    nodes: Box<[CallableNode]>,
    edges: Box<[CallableNodeId]>,
}
impl CallableGraph {
    pub fn node(&self, id: CallableNodeId) -> Result<&CallableNode, CallableGraphError> {
        self.0
            .nodes
            .get(id.index())
            .ok_or(CallableGraphError::InvalidNode(id.index()))
    }
    pub fn edges(&self, range: CallableEdgeRange) -> Result<&[CallableNodeId], CallableGraphError> {
        let end = (range.start as usize)
            .checked_add(range.len as usize)
            .ok_or(CallableGraphError::InvalidEdges)?;
        self.0
            .edges
            .get(range.start as usize..end)
            .ok_or(CallableGraphError::InvalidEdges)
    }
    pub fn validate_root(&self, root: CallableNodeId) -> Result<(), CallableGraphError> {
        match self.node(root)? {
            CallableNode::Function(_) | CallableNode::Intrinsic(_) => Ok(()),
            _ => Err(CallableGraphError::NotCallableRoot),
        }
    }
    pub fn publication_eq(
        &self,
        root: CallableNodeId,
        other: &Self,
        other_root: CallableNodeId,
    ) -> bool {
        self.publication_eq_with(root, other, other_root, &mut PublicationEqState::default())
    }
    pub(crate) fn publication_eq_with(
        &self,
        root: CallableNodeId,
        other: &Self,
        other_root: CallableNodeId,
        state: &mut PublicationEqState,
    ) -> bool {
        let mut work = vec![(root, other_root)];
        while let Some((left, right)) = work.pop() {
            let Ok(l) = self.node(left) else {
                return false;
            };
            let Ok(r) = other.node(right) else {
                return false;
            };
            match state.enter_callable_node_pair(
                (Arc::as_ptr(&self.0) as usize, left.0),
                (Arc::as_ptr(&other.0) as usize, right.0),
            ) {
                Err(()) => return false,
                Ok(false) => continue,
                Ok(true) => {}
            }
            if !compare_nodes(self, l, other, r, state, &mut work) {
                return false;
            }
        }
        true
    }
}

/// Phase-local assembler. Call charge_bytes/observe_depth on borrowed source
/// facts before creating owned copies, and charge every syntax visit externally.
/// reserve permits forward/cyclic references; finish refuses incomplete nodes.
#[derive(Debug, Default)]
pub struct CallableGraphBuilder {
    nodes: Vec<Option<CallableNode>>,
    edges: Vec<CallableNodeId>,
    bytes: usize,
    sources: FxHashSet<usize>,
    inline_edges: usize,
    native_budget: crate::args_mapper::NativeArgsBudget,
}
impl CallableGraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn charge_bytes(&mut self, amount: usize) -> Result<(), CallableGraphError> {
        let total = self
            .bytes
            .checked_add(amount)
            .ok_or(CallableGraphError::Limit("bytes"))?;
        if total > CALLABLE_GRAPH_BYTE_LIMIT {
            return Err(CallableGraphError::Limit("bytes"));
        }
        self.bytes = total;
        Ok(())
    }
    pub fn observe_depth(&self, depth: usize) -> Result<(), CallableGraphError> {
        if depth > CALLABLE_GRAPH_DEPTH_LIMIT {
            Err(CallableGraphError::Limit("depth"))
        } else {
            Ok(())
        }
    }
    pub fn reserve(&mut self) -> Result<CallableNodeId, CallableGraphError> {
        if self.nodes.len() >= CALLABLE_GRAPH_NODE_LIMIT {
            return Err(CallableGraphError::Limit("nodes"));
        }
        self.charge_bytes(std::mem::size_of::<CallableNode>())?;
        let id = CallableNodeId::new(self.nodes.len())?;
        self.nodes.push(None);
        Ok(id)
    }
    pub fn append_edges(
        &mut self,
        edges: &[CallableNodeId],
    ) -> Result<CallableEdgeRange, CallableGraphError> {
        let end = self
            .edges
            .len()
            .checked_add(edges.len())
            .ok_or(CallableGraphError::Limit("edges"))?;
        if end.saturating_add(self.inline_edges) > CALLABLE_GRAPH_EDGE_LIMIT {
            return Err(CallableGraphError::Limit("edges"));
        }
        for edge in edges {
            self.check_id(*edge)?;
        }
        self.charge_bytes(
            edges
                .len()
                .checked_mul(std::mem::size_of::<CallableNodeId>())
                .ok_or(CallableGraphError::Limit("bytes"))?,
        )?;
        let range = CallableEdgeRange {
            start: self.edges.len() as u32,
            len: edges.len() as u32,
        };
        self.edges.extend_from_slice(edges);
        Ok(range)
    }
    fn check_id(&self, id: CallableNodeId) -> Result<(), CallableGraphError> {
        if id.index() < self.nodes.len() {
            Ok(())
        } else {
            Err(CallableGraphError::InvalidNode(id.index()))
        }
    }
    pub fn set_node(
        &mut self,
        id: CallableNodeId,
        node: CallableNode,
    ) -> Result<(), CallableGraphError> {
        self.check_id(id)?;
        if self.nodes[id.index()].is_some() {
            return Err(CallableGraphError::AlreadyInitializedNode(id.index()));
        }
        validate_node(&node, &self.edges, self.nodes.len())?;
        let (mut metadata_bytes, inline_edges) = node_metadata_cost(&node);
        if let CallableNode::Native(value) = &node {
            let before = self.native_budget.bytes();
            self.native_budget.audit(value).map_err(|_| {
                CallableGraphError::UnsupportedNative(
                    "native value graph exceeds capability or budget",
                )
            })?;
            metadata_bytes = metadata_bytes.saturating_add(self.native_budget.bytes() - before);
        }
        let total_edges = self
            .inline_edges
            .checked_add(inline_edges)
            .ok_or(CallableGraphError::Limit("edges"))?;
        if total_edges.saturating_add(self.edges.len()) > CALLABLE_GRAPH_EDGE_LIMIT {
            return Err(CallableGraphError::Limit("edges"));
        }
        self.charge_bytes(metadata_bytes)?;
        self.inline_edges = total_edges;
        if let Some(source) = node_source(&node) {
            let key = Arc::as_ptr(&source.0) as usize;
            if !self.sources.contains(&key) {
                self.charge_bytes(source.byte_cost())?;
                self.sources.insert(key);
            }
        }
        self.nodes[id.index()] = Some(node);
        Ok(())
    }
    pub fn finish(self) -> Result<CallableGraph, CallableGraphError> {
        let nodes = self
            .nodes
            .into_iter()
            .enumerate()
            .map(|(i, node)| node.ok_or(CallableGraphError::UninitializedNode(i)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CallableGraph(Arc::new(CallableGraphData {
            nodes: nodes.into_boxed_slice(),
            edges: self.edges.into_boxed_slice(),
        })))
    }
}

/// Owned node metadata is charged even when a host preflight charged borrowed
/// construction inputs. The host must still check before making those copies.
fn node_metadata_cost(node: &CallableNode) -> (usize, usize) {
    fn parameters(p: &[CallableParameter]) -> (usize, usize) {
        let mut bytes = p
            .len()
            .saturating_mul(std::mem::size_of::<CallableParameter>());
        let mut edges = 0usize;
        for p in p {
            bytes = bytes.saturating_add(p.name.len());
            edges += usize::from(matches!(
                p.requirement,
                CallableParameterRequirement::Defaulted(_)
            )) + usize::from(p.annotation.is_some());
        }
        (bytes, edges)
    }
    fn code(c: &CallableCode) -> (usize, usize) {
        let mut bytes = c.name.len().saturating_add(
            c.globals
                .len()
                .saturating_mul(std::mem::size_of::<CallableGlobal>()),
        );
        let mut edges = 0usize;
        for g in &*c.globals {
            bytes = bytes.saturating_add(g.name.len());
            edges += usize::from(g.value.is_some());
        }
        (bytes, edges)
    }
    match node {
        CallableNode::Function(f) => {
            let (a, b) = code(&f.code);
            let (c, d) = parameters(&f.parameters);
            (
                a.saturating_add(c)
                    .saturating_add(f.capture_names.iter().map(|n| n.len()).sum::<usize>()),
                b + d + usize::from(f.return_annotation.is_some()),
            )
        }
        CallableNode::Code(c) => code(c),
        CallableNode::Cell { contents } => (0, usize::from(contents.is_some())),
        CallableNode::Intrinsic(i) => {
            let (a, b) = parameters(&i.parameters);
            (
                a.saturating_add(i.api.canonical().len()),
                b + usize::from(i.return_annotation.is_some()) + usize::from(i.receiver.is_some()),
            )
        }
        CallableNode::Struct { names, .. } | CallableNode::Provider { names, .. } => {
            (names.iter().map(|n| n.len()).sum(), 0)
        }
        CallableNode::Native(v) => (
            match v.kind() {
                AnalysisValueKind::String(s) => s.len(),
                _ => 0,
            },
            0,
        ),
        _ => (0, 0),
    }
}

fn node_source(node: &CallableNode) -> Option<&CallableSourceAuthority> {
    match node {
        CallableNode::Function(f) => Some(&f.code.source),
        CallableNode::Code(c) => Some(&c.source),
        _ => None,
    }
}
fn validate_node(
    node: &CallableNode,
    edges: &[CallableNodeId],
    nodes: usize,
) -> Result<(), CallableGraphError> {
    let check = |id: CallableNodeId| {
        if id.index() < nodes {
            Ok(())
        } else {
            Err(CallableGraphError::InvalidNode(id.index()))
        }
    };
    let range = |r: CallableEdgeRange| {
        edges
            .get(
                r.start as usize
                    ..(r.start as usize)
                        .checked_add(r.len as usize)
                        .ok_or(CallableGraphError::InvalidEdges)?,
            )
            .ok_or(CallableGraphError::InvalidEdges)
    };
    let parameters = |parameters: &[CallableParameter]| -> Result<(), CallableGraphError> {
        for p in parameters {
            if let CallableParameterRequirement::Defaulted(id) = p.requirement {
                check(id)?;
            }
            if let Some(id) = p.annotation {
                check(id)?;
            }
        }
        Ok(())
    };
    let code = |code: &CallableCode| -> Result<(), CallableGraphError> {
        if code.definition.start > code.definition.end {
            return Err(CallableGraphError::InvalidMetadata("definition span"));
        }
        range(code.dependencies)?;
        for global in &*code.globals {
            if let Some(id) = global.value {
                check(id)?;
            }
        }
        Ok(())
    };
    match node {
        CallableNode::Function(f) => {
            code(&f.code)?;
            parameters(&f.parameters)?;
            if let Some(id) = f.return_annotation {
                check(id)?;
            }
            if range(f.captures)?.len() != f.capture_names.len() {
                return Err(CallableGraphError::InvalidMetadata("capture names"));
            }
        }
        CallableNode::Code(c) => code(c)?,
        CallableNode::Cell { contents: Some(id) } => check(*id)?,
        CallableNode::List(r) | CallableNode::Tuple(r) => {
            range(*r)?;
        }
        CallableNode::Dict(r) => {
            if range(*r)?.len() % 2 != 0 {
                return Err(CallableGraphError::InvalidMetadata("dict entries"));
            }
        }
        CallableNode::Struct { names, values } | CallableNode::Provider { names, values, .. } => {
            if names.len() != range(*values)?.len() {
                return Err(CallableGraphError::InvalidMetadata("field names"));
            }
        }
        CallableNode::Intrinsic(i) => {
            parameters(&i.parameters)?;
            if let Some(id) = i.return_annotation {
                check(id)?;
            }
            if let Some(id) = i.receiver {
                check(id)?;
            }
        }
        CallableNode::Native(v) => {
            crate::args_mapper::NativeArgsBudget::default()
                .audit(v)
                .map_err(|_| {
                    CallableGraphError::UnsupportedNative(
                        "native value graph exceeds capability or budget",
                    )
                })?;
        }
        CallableNode::Cell { contents: None } => {}
    }
    Ok(())
}

fn compare_nodes(
    left_graph: &CallableGraph,
    left: &CallableNode,
    right_graph: &CallableGraph,
    right: &CallableNode,
    state: &mut PublicationEqState,
    work: &mut Vec<(CallableNodeId, CallableNodeId)>,
) -> bool {
    fn optional(
        left: Option<CallableNodeId>,
        right: Option<CallableNodeId>,
        work: &mut Vec<(CallableNodeId, CallableNodeId)>,
    ) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(l), Some(r)) => {
                work.push((l, r));
                true
            }
            _ => false,
        }
    }
    fn range(
        lg: &CallableGraph,
        l: CallableEdgeRange,
        rg: &CallableGraph,
        r: CallableEdgeRange,
        work: &mut Vec<(CallableNodeId, CallableNodeId)>,
    ) -> bool {
        let (Ok(l), Ok(r)) = (lg.edges(l), rg.edges(r)) else {
            return false;
        };
        if l.len() != r.len() {
            return false;
        }
        work.extend(l.iter().copied().zip(r.iter().copied()));
        true
    }
    fn params(
        l: &[CallableParameter],
        r: &[CallableParameter],
        work: &mut Vec<(CallableNodeId, CallableNodeId)>,
    ) -> bool {
        l.len() == r.len()
            && l.iter().zip(r).all(|(l, r)| {
                l.name == r.name
                    && l.mode == r.mode
                    && optional(l.annotation, r.annotation, work)
                    && match (l.requirement, r.requirement) {
                        (
                            CallableParameterRequirement::Defaulted(l),
                            CallableParameterRequirement::Defaulted(r),
                        ) => {
                            work.push((l, r));
                            true
                        }
                        (l, r) => l == r,
                    }
            })
    }
    fn code(
        lg: &CallableGraph,
        l: &CallableCode,
        rg: &CallableGraph,
        r: &CallableCode,
        work: &mut Vec<(CallableNodeId, CallableNodeId)>,
    ) -> bool {
        l.source == r.source
            && l.name == r.name
            && l.definition == r.definition
            && range(lg, l.dependencies, rg, r.dependencies, work)
            && l.globals.len() == r.globals.len()
            && l.globals.iter().zip(r.globals.iter()).all(|(l, r)| {
                l.name == r.name && l.access == r.access && optional(l.value, r.value, work)
            })
    }
    match (left, right) {
        (CallableNode::Native(l), CallableNode::Native(r)) => l.publication_eq_with(r, state),
        (CallableNode::Cell { contents: l }, CallableNode::Cell { contents: r }) => {
            optional(*l, *r, work)
        }
        (CallableNode::List(l), CallableNode::List(r))
        | (CallableNode::Tuple(l), CallableNode::Tuple(r))
        | (CallableNode::Dict(l), CallableNode::Dict(r)) => {
            range(left_graph, *l, right_graph, *r, work)
        }
        (
            CallableNode::Struct {
                names: ln,
                values: l,
            },
            CallableNode::Struct {
                names: rn,
                values: r,
            },
        ) => ln == rn && range(left_graph, *l, right_graph, *r, work),
        (
            CallableNode::Provider {
                identity: li,
                names: ln,
                values: l,
                has_schema: ls,
            },
            CallableNode::Provider {
                identity: ri,
                names: rn,
                values: r,
                has_schema: rs,
            },
        ) => li == ri && ln == rn && ls == rs && range(left_graph, *l, right_graph, *r, work),
        (CallableNode::Code(l), CallableNode::Code(r)) => code(left_graph, l, right_graph, r, work),
        (CallableNode::Function(l), CallableNode::Function(r)) => {
            code(left_graph, &l.code, right_graph, &r.code, work)
                && params(&l.parameters, &r.parameters, work)
                && optional(l.return_annotation, r.return_annotation, work)
                && l.capture_names == r.capture_names
                && range(left_graph, l.captures, right_graph, r.captures, work)
        }
        (CallableNode::Intrinsic(l), CallableNode::Intrinsic(r)) => {
            l.api == r.api
                && params(&l.parameters, &r.parameters, work)
                && optional(l.return_annotation, r.return_annotation, work)
                && optional(l.receiver, r.receiver, work)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
