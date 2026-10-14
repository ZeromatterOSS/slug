use super::*;
use crate::RetainedRunfiles;

/// Actual runfiles wrapper identity, preserved by the evaluator's freezer.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RunfilesOccurrence(Arc<()>);
impl RunfilesOccurrence {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
    pub(crate) fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl Default for RunfilesOccurrence {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for RunfilesOccurrence {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RunfilesOccurrence {}
#[derive(Debug, Clone, Allocative)]
pub struct RunfilesValue {
    retained: RetainedRunfiles,
    occurrence: RunfilesOccurrence,
}
impl RunfilesValue {
    pub fn new(retained: RetainedRunfiles, occurrence: RunfilesOccurrence) -> Self {
        Self {
            retained,
            occurrence,
        }
    }
    pub fn retained(&self) -> &RetainedRunfiles {
        &self.retained
    }
    pub fn occurrence(&self) -> &RunfilesOccurrence {
        &self.occurrence
    }
    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        state.enter_runfiles_wrapper_pair(self.occurrence.pointer(), other.occurrence.pointer())
            && self.retained.publication_eq_with(&other.retained, state)
    }
}
impl PartialEq for RunfilesValue {
    fn eq(&self, other: &Self) -> bool {
        self.occurrence == other.occurrence
            || (self.retained.is_empty() && other.retained.is_empty())
    }
}
impl Eq for RunfilesValue {}
