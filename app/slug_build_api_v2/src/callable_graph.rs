//! Bounded native semantic graph for actual callable publication.
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;

use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ProviderIdentity;

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
#[derive(Debug, Allocative)]
struct CallableSourceAuthorityStorage {
    data: CallableSourceAuthorityData,
    // Only new() can create this complete, successfully admitted certificate.
    fact_count: usize,
}
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CallableSourceAuthority(Arc<CallableSourceAuthorityStorage>);
impl PartialEq for CallableSourceAuthority {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.data() == other.data()
    }
}
impl Eq for CallableSourceAuthority {}
// Own the exact validated input until the caller has checked its paid forecast.
// No independently mutable Data or detached byte certificate can escape.
pub(crate) struct PreparedCallableSourceAuthority {
    data: CallableSourceAuthorityData,
    fact_count: usize,
    bytes: usize,
}
impl PreparedCallableSourceAuthority {
    pub(crate) fn byte_cost(&self) -> usize {
        self.bytes
    }
    pub(crate) fn finish(self) -> CallableSourceAuthority {
        CallableSourceAuthority(Arc::new(CallableSourceAuthorityStorage {
            data: self.data,
            fact_count: self.fact_count,
        }))
    }
}
impl CallableSourceAuthority {
    pub fn new(data: CallableSourceAuthorityData) -> Result<Self, CallableGraphError> {
        Self::prepare(data).map(PreparedCallableSourceAuthority::finish)
    }
    pub(crate) fn prepare(
        data: CallableSourceAuthorityData,
    ) -> Result<PreparedCallableSourceAuthority, CallableGraphError> {
        if data.reachable.len() > CALLABLE_GRAPH_NODE_LIMIT
            || data.direct_children.len() > CALLABLE_GRAPH_NODE_LIMIT
        {
            return Err(CallableGraphError::Limit("source facts"));
        }
        let fact_count = Self::data_fact_count(&data);
        if fact_count > CALLABLE_GRAPH_NODE_LIMIT {
            return Err(CallableGraphError::Limit("source facts"));
        }
        let bytes = Self::data_byte_cost(&data);
        if bytes > CALLABLE_GRAPH_BYTE_LIMIT {
            return Err(CallableGraphError::Limit("source bytes"));
        }
        Ok(PreparedCallableSourceAuthority {
            data,
            fact_count,
            bytes,
        })
    }
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(crate) fn storage_identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    /// Fixed allocation bytes beyond the public inline data: the private
    /// certificate, layout padding and Arc counters. Dynamic arrays/payloads
    /// remain the caller's separate complete pre-copy forecast.
    pub fn storage_overhead_bytes() -> usize {
        let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
            .extend(std::alloc::Layout::new::<CallableSourceAuthorityStorage>())
            .expect("fixed source authority allocation layout");
        layout.pad_to_align().size() - std::mem::size_of::<CallableSourceAuthorityData>()
    }
    pub fn data(&self) -> &CallableSourceAuthorityData {
        &self.0.data
    }
    pub(crate) fn fact_count(&self) -> usize {
        self.0.fact_count
    }
    fn data_fact_count(data: &CallableSourceAuthorityData) -> usize {
        std::iter::once(&data.root)
            .chain(data.direct_children.iter())
            .chain(data.reachable.iter().map(|s| &s.identity))
            .fold(0usize, |n, m| {
                n.saturating_add(1)
                    .saturating_add(m.repository_mapping.len())
            })
    }
    pub(crate) fn byte_cost(&self) -> usize {
        Self::data_byte_cost(self.data())
    }
    fn data_byte_cost(data: &CallableSourceAuthorityData) -> usize {
        fn module(m: &CallableModuleIdentity) -> usize {
            m.repository_mapping.iter().fold(
                std::mem::size_of::<CallableModuleIdentity>()
                    .saturating_add(m.label.package().repo().as_str().len())
                    .saturating_add(m.label.package().package().as_str().len())
                    .saturating_add(m.label.target().as_str().len())
                    .saturating_add(m.label.mapping_id().map_or(0, |id| id.as_str().len()))
                    .saturating_add(m.workspace_path.as_os_str().len())
                    .saturating_add(2 * std::mem::size_of::<usize>()),
                |n, (a, c)| {
                    n.saturating_add(std::mem::size_of::<(ApparentRepoName, CanonicalRepoName)>())
                        .saturating_add(a.as_str().len())
                        .saturating_add(c.as_str().len())
                },
            )
        }
        let n = data
            .direct_children
            .iter()
            .fold(module(&data.root), |n, m| n.saturating_add(module(m)));
        data.reachable
            .iter()
            .fold(n, |n, s| {
                n.saturating_add(module(&s.identity)).saturating_add(
                    std::mem::size_of::<CallableModuleSource>()
                        - std::mem::size_of::<CallableModuleIdentity>(),
                )
            })
            // The root's inline identity was counted above. Admit every other
            // envelope field/padding, Arc counters, and both array Arc headers
            // before constructing or retaining this complete immutable owner.
            .saturating_add(
                std::mem::size_of::<CallableSourceAuthorityData>()
                    - std::mem::size_of::<CallableModuleIdentity>(),
            )
            .saturating_add(Self::storage_overhead_bytes())
            .saturating_add(4 * std::mem::size_of::<usize>())
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
pub enum CallableGlobalAccess {
    Read,
    Write,
    ReadWrite,
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
// Callable descriptors are views of the ONE publication arena, never a
// separate executable/native graph owner.
pub use crate::published_value::PublishedCode as CallableCode;
pub use crate::published_value::PublishedEdgeRange as CallableEdgeRange;
pub use crate::published_value::PublishedFunction as CallableFunction;
pub use crate::published_value::PublishedGlobal as CallableGlobal;
pub use crate::published_value::PublishedIntrinsic as CallableIntrinsic;
pub use crate::published_value::PublishedNode as CallableNode;
pub use crate::published_value::PublishedNodeId as CallableNodeId;
pub use crate::published_value::PublishedParameter as CallableParameter;
pub use crate::published_value::PublishedParameterRequirement as CallableParameterRequirement;
pub use crate::published_value::PublishedValueArena as CallableGraph;
pub use crate::published_value::PublishedValueArenaBuilder as CallableGraphBuilder;

#[cfg(test)]
#[path = "callable_graph/source_certificate_tests.rs"]
mod source_certificate_tests;
