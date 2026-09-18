//! Occurrence identity for the admitted empty C++ HeaderInfo value.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcHeaderInfoOccurrence(Arc<()>);
impl CcHeaderInfoOccurrence {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
    pub(crate) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl Default for CcHeaderInfoOccurrence {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for CcHeaderInfoOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CcHeaderInfoOccurrence {}
impl Hash for CcHeaderInfoOccurrence {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}
