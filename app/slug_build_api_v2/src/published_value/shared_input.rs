//! Shared immutable native leaf facts. Allocation identity is accounting only.
use super::*;

/// Complete immutable leaf metadata, with no owner, arena, evaluator, mutable
/// session, sink, executable heap or hidden publication references. Its cached
/// shape includes the full payload and Arc allocation header. Implementations
/// preserve their original typed semantic equality separately from this audit.
pub trait ImmutablePublicationInput: Any + fmt::Debug + Send + Sync + Allocative {
    fn as_any(&self) -> &dyn Any;
    fn retained_shape(&self) -> PublicationShape;
}
/// Guard for an existing leaf allocation, without a cookie or payload copy.
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
    pub(crate) fn storage_identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
}
