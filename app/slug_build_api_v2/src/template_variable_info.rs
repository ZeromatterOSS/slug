//! Immutable native template-variable snapshot with occurrence identity.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use starlark_map::small_map::SmallMap;

use crate::analysis_value::PublicationEqState;

/// The allocation is the occurrence: copying the handle preserves native
/// equality, while even an empty new construction has a distinct identity.
#[derive(Debug, Clone, Dupe, Allocative)]
pub struct TemplateVariableInfo(Arc<SmallMap<CompactString, CompactString>>);

impl TemplateVariableInfo {
    pub fn new(variables: SmallMap<CompactString, CompactString>) -> Self {
        Self(Arc::new(variables))
    }

    pub fn variables(&self) -> &SmallMap<CompactString, CompactString> {
        &self.0
    }

    fn pointer(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        // Native dict equality ignores order, but .variables iteration exposes
        // it. Publication also preserves shared-versus-distinct occurrences.
        self.0.eq_ordered(&other.0)
            && state.enter_template_variable_pair(self.pointer(), other.pointer())
    }
}

impl PartialEq for TemplateVariableInfo {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for TemplateVariableInfo {}
impl Hash for TemplateVariableInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pointer().hash(state);
    }
}

#[cfg(test)]
mod tests;
