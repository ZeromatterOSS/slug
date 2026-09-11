use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use slug_loading_v2::ModuleRegistrationExpansionError;

use super::AnalysisError;
use super::AnalysisErrorKind;

/// Shared semantic cause. Presentation never participates in equality.
#[derive(Clone, Eq, PartialEq, Allocative)]
pub struct RegistrationAnalysisError(Arc<ModuleRegistrationExpansionError>);

impl RegistrationAnalysisError {
    pub fn error(&self) -> &ModuleRegistrationExpansionError {
        &self.0
    }
}

impl AnalysisError {
    pub(super) fn from_registration(error: &Arc<ModuleRegistrationExpansionError>) -> Self {
        Self::from(Arc::clone(error))
    }
}

impl From<Arc<ModuleRegistrationExpansionError>> for AnalysisError {
    fn from(error: Arc<ModuleRegistrationExpansionError>) -> Self {
        Self {
            kind: AnalysisErrorKind::Registration(RegistrationAnalysisError(error)),
        }
    }
}

impl fmt::Display for RegistrationAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.diagnostic().fmt(f)
    }
}

impl fmt::Debug for RegistrationAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
