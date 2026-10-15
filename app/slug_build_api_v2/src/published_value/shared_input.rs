//! Shared immutable native metadata graphs. Allocation identity is accounting only.
use super::*;

/// Complete immutable native metadata, with no owner, arena, evaluator, mutable
/// session, sink, executable heap or hidden publication references. Its cached
/// shape includes the full payload and Arc allocation header. Implementations
/// preserve their original typed semantic equality separately from this audit.
/// A composite owns its exact immutable child guards for its entire lifetime;
/// intrinsic accounting excludes only those child bodies. Complete conservative
/// accounting still includes every containment path and its complete depth.
pub trait ImmutablePublicationInput: Any + fmt::Debug + Send + Sync + Allocative {
    fn as_any(&self) -> &dyn Any;
    fn retained_shape(&self) -> PublicationShape;
    /// Positive local body/header, semantic reference fields and declaration
    /// storage, excluding only the bodies of the exact declared children.
    fn intrinsic_shape(&self) -> PublicationShape {
        self.retained_shape()
    }
    fn shared_inputs(&self) -> &[SharedPublicationInput] {
        &[]
    }
}
/// Guard for an existing immutable allocation, without a cookie or payload copy.
/// This type intentionally supplies no semantic equality or hashing.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct SharedPublicationInput(Arc<dyn ImmutablePublicationInput>);
impl SharedPublicationInput {
    pub fn new(input: Arc<dyn ImmutablePublicationInput>) -> Self {
        Self(input)
    }
    pub fn retained_shape(&self) -> PublicationShape {
        self.0.retained_shape()
    }
    pub fn intrinsic_shape(&self) -> PublicationShape {
        self.0.intrinsic_shape()
    }
    pub fn shared_inputs(&self) -> &[SharedPublicationInput] {
        self.0.shared_inputs()
    }
    pub(crate) fn storage_identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
}
