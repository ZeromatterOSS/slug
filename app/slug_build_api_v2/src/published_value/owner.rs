use std::any::Any;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

use super::PublishedValueError;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum FunctionOwnerDomain {
    BzlLoad,
    ConfiguredRule,
    ConfiguredAspect,
}
/// Immutable, complete native identity inputs. Implementations must not retain
/// evaluators, mutable sessions, sinks or eventual results containing this owner.
pub trait StructuralFunctionOwnerPart: Any + Debug + Send + Sync + Allocative {
    fn as_any(&self) -> &dyn Any;
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool;
    fn hash_part(&self, state: &mut dyn Hasher);
    fn retained_bytes(&self) -> usize;
    /// Cached intrinsic accounting, excluding declared owner, shared-input and arena payloads.
    /// Reference fields, witness storage and dependency slices remain local.
    fn retained_shape(&self) -> super::PublicationShape {
        super::PublicationShape {
            nodes: 0,
            edges: 0,
            bytes: self.retained_bytes(),
            visits: 1,
            depth: 0,
        }
    }

    /// Stable immutable declarations: retain these exact guards and their full
    /// immutable shapes for this part's lifetime, including after owner audit.
    fn shared_inputs(&self) -> &[super::SharedPublicationInput] {
        &[]
    }

    fn retained_owners(&self) -> &[FunctionEvaluationOwner] {
        &[]
    }

    fn imported_arenas(&self) -> &[super::PublishedValueArena] {
        &[]
    }
}
#[derive(Debug, Allocative)]
struct OwnerData {
    domain: FunctionOwnerDomain,
    intrinsic: super::PublicationShape,
    complete: super::PublicationShape,
    stable_key: Arc<dyn StructuralFunctionOwnerPart>,
    input_validity: Arc<dyn StructuralFunctionOwnerPart>,
}
// No detached scalar or public input can construct this completion witness.
// The original private OwnerData owns both immutable declaration/guard slices.
pub(in crate::published_value) struct OwnerSharedAudit<'a> {
    _owner: &'a OwnerData,
    input: &'a super::SharedPublicationInput,
}
impl<'a> OwnerSharedAudit<'a> {
    pub(in crate::published_value) fn input(self) -> &'a super::SharedPublicationInput {
        self.input
    }
}
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct FunctionEvaluationOwner(Arc<OwnerData>);
impl FunctionEvaluationOwner {
    pub(crate) fn storage_identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub fn new(
        domain: FunctionOwnerDomain,
        stable_key: Arc<dyn StructuralFunctionOwnerPart>,
        input_validity: Arc<dyn StructuralFunctionOwnerPart>,
    ) -> Result<Self, PublishedValueError> {
        let a = stable_key.retained_shape();
        let b = input_validity.retained_shape();
        if a.bytes != stable_key.retained_bytes() || b.bytes != input_validity.retained_bytes() {
            return Err(PublishedValueError::Invalid("owner shape bytes"));
        }
        let intrinsic = super::PublicationShape {
            nodes: a.nodes.saturating_add(b.nodes),
            edges: a.edges.saturating_add(b.edges),
            // Owner allocation plus strong/weak counters for it and the two
            // trait-part allocations. Shared part storage is conservatively
            // charged twice; no new semantic or storage dedup is implied.
            bytes: a
                .bytes
                .checked_add(b.bytes)
                .and_then(|n| n.checked_add(std::mem::size_of::<OwnerData>()))
                .and_then(|n| n.checked_add(6 * std::mem::size_of::<usize>()))
                .ok_or(PublishedValueError::Limit("owner header bytes"))?,
            visits: a.visits.saturating_add(b.visits),
            depth: a.depth.max(b.depth),
        };
        // Audit the borrowed draft before allocating the published owner. Prior
        // immutable owners/arenas cannot refer to this not-yet-created owner.
        let mut audit = super::PublishedValueArenaBuilder::new_strict_shared_audit();
        audit.charge_storage(crate::DepsetStorageCharge {
            nodes: intrinsic.nodes,
            edges: intrinsic.edges,
            bytes: intrinsic.bytes,
            visits: intrinsic.visits,
        })?;
        audit.observe_depth(intrinsic.depth)?;
        for part in [&stable_key, &input_validity] {
            for input in part.shared_inputs() {
                audit.audit_shared_input(input, 1)?;
            }
            for owner in part.retained_owners() {
                audit.audit_owner_shape_at_depth(owner, 1)?;
            }
            for arena in part.imported_arenas() {
                audit.audit_arena_closure(arena, 1)?;
            }
        }
        let completion =
            audit.complete_owner_shape(&*stable_key, &*input_validity, intrinsic.visits)?;
        let complete = completion.shape;
        // Constructors do not pass through arena seal: retain the full closure
        // cap here even when work and sealed-import facts are each admitted.
        if complete.visits > super::RETAINED_FACT_LIMIT {
            return Err(audit.owner_retained_visit_refusal(
                domain,
                a.visits,
                b.visits,
                intrinsic.visits,
                completion.declaration_references,
                complete.visits,
            ));
        }
        Ok(Self(Arc::new(OwnerData {
            domain,
            intrinsic,
            complete,
            stable_key,
            input_validity,
        })))
    }
    pub fn domain(&self) -> FunctionOwnerDomain {
        self.0.domain
    }
    pub fn stable_key(&self) -> &dyn StructuralFunctionOwnerPart {
        &*self.0.stable_key
    }
    pub fn input_validity(&self) -> &dyn StructuralFunctionOwnerPart {
        &*self.0.input_validity
    }
    pub fn imported_arenas(&self) -> impl Iterator<Item = &super::PublishedValueArena> {
        self.stable_key()
            .imported_arenas()
            .iter()
            .chain(self.input_validity().imported_arenas())
    }
    pub fn shared_inputs(&self) -> impl Iterator<Item = &super::SharedPublicationInput> {
        self.stable_key()
            .shared_inputs()
            .iter()
            .chain(self.input_validity().shared_inputs())
    }
    pub(in crate::published_value) fn certified_shared_inputs(
        &self,
    ) -> impl Iterator<Item = OwnerSharedAudit<'_>> {
        self.shared_inputs().map(|input| OwnerSharedAudit {
            _owner: self.0.as_ref(),
            input,
        })
    }
    pub fn retained_owners(&self) -> impl Iterator<Item = &FunctionEvaluationOwner> {
        self.stable_key()
            .retained_owners()
            .iter()
            .chain(self.input_validity().retained_owners())
    }
    pub fn intrinsic_shape(&self) -> super::PublicationShape {
        self.0.intrinsic
    }
    pub fn retained_shape(&self) -> super::PublicationShape {
        self.0.complete
    }
    pub fn retained_bytes(&self) -> usize {
        self.0.complete.bytes
    }
    pub fn hash_fields<H: Hasher>(&self, state: &mut H) {
        self.domain().hash(state);
        self.stable_key().hash_part(state);
        self.input_validity().hash_part(state);
    }
}
impl PartialEq for FunctionEvaluationOwner {
    fn eq(&self, other: &Self) -> bool {
        self.domain() == other.domain()
            && self.stable_key().eq_part(other.stable_key())
            && self.input_validity().eq_part(other.input_validity())
    }
}
impl Eq for FunctionEvaluationOwner {}
impl Hash for FunctionEvaluationOwner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash_fields(state)
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub enum FunctionSymbolName {
    Local(u64),
    Exported(Arc<str>),
}
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct PublishedFunctionSymbol {
    owner: FunctionEvaluationOwner,
    name: FunctionSymbolName,
}
impl PublishedFunctionSymbol {
    pub fn new(owner: FunctionEvaluationOwner, name: FunctionSymbolName) -> Self {
        Self { owner, name }
    }
    pub fn owner(&self) -> &FunctionEvaluationOwner {
        &self.owner
    }
    pub fn name(&self) -> &FunctionSymbolName {
        &self.name
    }
    pub fn hash_fields<H: Hasher>(&self, state: &mut H) {
        self.hash(state)
    }
}

#[cfg(test)]
#[path = "owner_header_tests.rs"]
mod header_tests;
