//! Advisory largest FULL-wrapper candidate; no model body is inspected.
use super::*;
use crate::cc_features::CcToolchainFeatureModel;

#[derive(Default)]
pub(super) struct Candidate<'a> {
    model: Option<&'a Arc<CcToolchainFeatureModel>>,
    visits: usize,
    ambiguous: bool,
}
impl<'a> Candidate<'a> {
    pub(super) fn consider(&mut self, model: &'a Arc<CcToolchainFeatureModel>, visits: usize) {
        match self.model {
            None => {
                self.model = Some(model);
                self.visits = visits;
            }
            Some(_) if visits > self.visits => {
                self.model = Some(model);
                self.visits = visits;
                self.ambiguous = false;
            }
            Some(prior) if visits == self.visits && !Arc::ptr_eq(prior, model) => {
                self.ambiguous = true
            }
            Some(_) => {}
        }
    }
    pub(super) fn publish(&self, out: &mut PublicationNativeRetentionCensus) {
        out.feature_selected = self.model.is_some();
        out.feature_maximum_wrapper_visits = self.visits;
        out.feature_ambiguous = self.ambiguous;
    }
    pub(super) fn matches(&self, model: &Arc<CcToolchainFeatureModel>) -> bool {
        self.model.is_some_and(|prior| Arc::ptr_eq(prior, model))
    }
}
