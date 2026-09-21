//! Finite source-authenticated CcToolchainInfo methods. This model owns captures,
//! never Starlark functions, evaluator frames or code. Runtime tokens authenticate
//! occurrence only; Loading separately authenticates the pinned initializer.
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;

use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum CcToolchainCallableKind {
    SupportsPic,
    AlwaysPic,
    StaticRuntime,
    DynamicRuntime,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct CcToolchainCallableSource {
    pub label: CanonicalLabel,
    pub digest: [u8; 32],
}

#[derive(Debug, Allocative)]
pub struct CcToolchainCallableModel {
    pub kind: CcToolchainCallableKind,
    pub source: CcToolchainCallableSource,
    pub identity: Arc<()>,
    /// Cached string-name hash, not the completed function hash.
    pub name_hash: u32,
    pub capture: AnalysisValue,
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CcToolchainCallable(Arc<CcToolchainCallableModel>);

impl CcToolchainCallable {
    pub fn new(model: CcToolchainCallableModel) -> Result<Self, String> {
        let valid = match model.kind {
            CcToolchainCallableKind::SupportsPic | CcToolchainCallableKind::AlwaysPic => {
                matches!(model.capture.kind(), AnalysisValueKind::None)
            }
            CcToolchainCallableKind::StaticRuntime | CcToolchainCallableKind::DynamicRuntime => {
                matches!(
                    model.capture.kind(),
                    AnalysisValueKind::None | AnalysisValueKind::Depset(_)
                )
            }
        };
        if !valid {
            return Err("CcToolchainInfo runtime capture requires None or a depset".to_owned());
        }
        Ok(Self(Arc::new(model)))
    }

    pub fn model(&self) -> &CcToolchainCallableModel {
        &self.0
    }

    pub(crate) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let left = self.model();
        let right = other.model();
        if left.kind != right.kind
            || left.source != right.source
            || left.name_hash != right.name_hash
        {
            return false;
        }
        // A genuine module-global Def outside this graph can observe its identity.
        if left.kind == CcToolchainCallableKind::SupportsPic
            && !Arc::ptr_eq(&left.identity, &right.identity)
        {
            return false;
        }
        state.enter_cc_callable_pair(
            Arc::as_ptr(&left.identity) as usize,
            Arc::as_ptr(&right.identity) as usize,
        ) && left.capture.publication_eq_with(&right.capture, state)
    }
}

impl PartialEq for CcToolchainCallable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.identity, &other.0.identity)
    }
}
impl Eq for CcToolchainCallable {}
impl Hash for CcToolchainCallable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0.identity) as usize).hash(state);
    }
}

#[cfg(test)]
mod tests;
