//! Original-value publication: finite native descriptors and immutable imports.
use std::any::Any;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use crate::AnalysisValueType;
use crate::CallableSourceAuthority;
use crate::analysis_value::PublicationEqState;
mod builder;
mod equality;
mod ids;
mod native;
mod owner;
pub(crate) mod projection;
mod rows;
mod runfiles;
mod shared_input;
pub use builder::PublicationByteContext;
pub use builder::PublicationByteRefusal;
pub use builder::PublicationRecord;
pub use builder::PublicationScratch;
pub use builder::PublicationSourcePreparation;
pub use builder::PublishedValueArenaBuilder;
pub use ids::*;
pub use owner::*;
pub use rows::*;
pub use runfiles::*;
pub use shared_input::ImmutablePublicationInput;
pub use shared_input::SharedPublicationInput;
pub const NODE_LIMIT: usize = 65_536;
/// Full retained facts in a completed publication or evaluation owner.
/// This Slug-native resource bound is separate from paid work and node IDs.
pub const RETAINED_FACT_LIMIT: usize = 2 * NODE_LIMIT;
pub const EDGE_LIMIT: usize = 524_288;
/// Fixed aggregate publication forecast policy, shared by construction,
/// retained owner/import/shared closure and post-seal projection admission.
/// Independent source/token, runtime and Engine limits are unchanged.
pub const BYTE_LIMIT: usize = 128 * 1024 * 1024;
pub const DEPTH_LIMIT: usize = 256;
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PublishedValueError {
    Limit(&'static str),
    ByteBudget(Box<PublicationByteRefusal>),
    Invalid(&'static str),
    Unsupported(&'static str),
}
impl fmt::Display for PublishedValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteBudget(refusal) => {
                write!(f, "published value: bytes admission refused: {refusal:?}")
            }
            _ => write!(f, "published value: {self:?}"),
        }
    }
}
impl std::error::Error for PublishedValueError {}
pub trait PublishedOriginalBinding: Any + fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn root_count(&self) -> usize;
    fn imported_arenas(&self) -> &[PublishedValueArena];
}
#[derive(Debug, Default, Clone, Copy, Allocative)]
pub struct PublicationShape {
    pub nodes: usize,
    pub edges: usize,
    pub bytes: usize,
    pub visits: usize,
    pub depth: usize,
}
#[derive(Debug, Allocative)]
struct PublishedValueArenaData {
    rows: Box<[PublishedNode]>,
    edges: Box<[PublishedEdge]>,
    imports: Arc<[PublishedValueArena]>,
    owners: Arc<[FunctionEvaluationOwner]>,
    #[allocative(skip)]
    originals: Arc<dyn PublishedOriginalBinding>,
    original_slots: Box<[Option<PublishedRootSlot>]>,
    sources: Arc<[CallableSourceAuthority]>,
    shape: PublicationShape,
    local_shape: PublicationShape,
}
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct PublishedValueArena(Arc<PublishedValueArenaData>);
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct PublishedValueRef {
    arena: PublishedValueArena,
    node: PublishedNodeId,
}
impl PublishedValueArena {
    pub fn identity(&self) -> PublishedArenaIdentity {
        PublishedArenaIdentity(self.clone())
    }
    pub fn value(&self, node: PublishedNodeId) -> Result<PublishedValueRef, PublishedValueError> {
        self.node(node)?;
        Ok(PublishedValueRef {
            arena: self.dupe(),
            node,
        })
    }
    pub fn node(&self, node: PublishedNodeId) -> Result<&PublishedNode, PublishedValueError> {
        self.0
            .rows
            .get(node.index())
            .ok_or(PublishedValueError::Invalid("node"))
    }
    pub fn edges(
        &self,
        range: PublishedEdgeRange,
    ) -> Result<&[PublishedEdge], PublishedValueError> {
        self.0
            .edges
            .get(
                range.start as usize
                    ..(range.start as usize)
                        .checked_add(range.len as usize)
                        .ok_or(PublishedValueError::Invalid("edge range"))?,
            )
            .ok_or(PublishedValueError::Invalid("edge range"))
    }
    pub fn resolve(&self, edge: PublishedEdge) -> Result<PublishedValueRef, PublishedValueError> {
        match edge {
            PublishedEdge::Local(id) => self.value(id),
            PublishedEdge::Imported { arena, node } => self
                .0
                .imports
                .get(arena.index())
                .ok_or(PublishedValueError::Invalid("import"))?
                .value(node),
        }
    }
    pub fn owners(&self) -> &[FunctionEvaluationOwner] {
        &self.0.owners
    }
    pub fn imports(&self) -> &[PublishedValueArena] {
        &self.0.imports
    }
    pub fn validate_root(&self, node: PublishedNodeId) -> Result<(), PublishedValueError> {
        if matches!(
            self.node(node)?,
            PublishedNode::Function(_)
                | PublishedNode::Intrinsic(_)
                | PublishedNode::ProviderConstructor(_)
                | PublishedNode::RawProviderConstructor { .. }
        ) {
            Ok(())
        } else {
            Err(PublishedValueError::Invalid("callable root"))
        }
    }
    pub fn sources(&self) -> &[CallableSourceAuthority] {
        &self.0.sources
    }
    pub fn originals(&self) -> &dyn PublishedOriginalBinding {
        &*self.0.originals
    }
    pub fn original_slot(&self, node: PublishedNodeId) -> Option<PublishedRootSlot> {
        self.0.original_slots.get(node.index()).copied().flatten()
    }
    pub fn shape(&self) -> PublicationShape {
        self.0.shape
    }
}
impl PublishedValueRef {
    pub fn identity(&self) -> PublishedValueIdentity {
        PublishedValueIdentity(self.clone())
    }
    pub fn project_provider(
        &self,
        fields: impl IntoIterator<Item = (impl Into<compact_str::CompactString>, crate::AnalysisValue)>,
    ) -> Result<crate::ProviderOccurrence, PublishedValueError> {
        let PublishedNode::Provider(row) = self.node() else {
            return Err(PublishedValueError::Invalid("provider projection root"));
        };
        let mut p = crate::ProviderOccurrence::new(row.identity.clone(), fields)
            .with_schema_classification(row.has_schema);
        if let Some(hidden) = &row.files_to_run {
            p = p.with_files_to_run(Arc::new(hidden.value().clone()));
        }
        crate::ProviderOccurrence::with_original_origin(p, self.clone())
    }
    pub fn same_original(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
    pub fn arena(&self) -> &PublishedValueArena {
        &self.arena
    }
    pub fn node_id(&self) -> PublishedNodeId {
        self.node
    }
    pub fn node(&self) -> &PublishedNode {
        &self.arena.0.rows[self.node.index()]
    }
    pub fn original_slot(&self) -> Option<PublishedRootSlot> {
        self.arena.original_slot(self.node)
    }
    pub fn value_type(&self) -> AnalysisValueType {
        projection::value_type(self.node())
    }
    pub fn publication_eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        equality::equal(self, other, state)
    }
    pub(crate) fn key(&self) -> (usize, u32) {
        (Arc::as_ptr(&self.arena.0) as usize, self.node.0)
    }
}

/// Owning key for phase-local projection memos. Address identity never enters
/// semantic Eq/hash; retaining the arena prevents address reuse while keyed.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct PublishedValueIdentity(PublishedValueRef);
impl PartialEq for PublishedValueIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_original(&other.0)
    }
}
impl Eq for PublishedValueIdentity {}
impl std::hash::Hash for PublishedValueIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.0.key(), state)
    }
}

#[cfg(test)]
mod tests;

/// Owning arena identity for phase-local quota/memo deduplication only.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct PublishedArenaIdentity(PublishedValueArena);
impl PartialEq for PublishedArenaIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.0, &other.0.0)
    }
}
impl Eq for PublishedArenaIdentity {}
impl std::hash::Hash for PublishedArenaIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&(Arc::as_ptr(&self.0.0) as usize), state)
    }
}
