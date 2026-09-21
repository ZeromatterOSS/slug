//! Retained Cpp view over the sole structural configuration owner. Occurrence
//! identity is Slug-native, not a reproduction of the JVM fragment cache.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_configuration_v2::CppFragmentProjection;

use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedCppFragment {
    projection: CppFragmentProjection,
    occurrence: Arc<()>,
}

impl RetainedCppFragment {
    pub fn new(projection: CppFragmentProjection) -> Self {
        Self {
            projection,
            occurrence: Arc::new(()),
        }
    }
    pub fn projection(&self) -> &CppFragmentProjection {
        &self.projection
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.projection == other.projection
            && state.enter_cpp_fragment_pair(
                Arc::as_ptr(&self.occurrence) as usize,
                Arc::as_ptr(&other.occurrence) as usize,
            )
    }
}
impl PartialEq for RetainedCppFragment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.occurrence, &other.occurrence)
    }
}
impl Eq for RetainedCppFragment {}
impl Hash for RetainedCppFragment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.occurrence) as usize).hash(state);
    }
}

#[cfg(test)]
mod tests;
