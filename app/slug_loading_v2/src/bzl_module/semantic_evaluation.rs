//! DICE-owned actual module evaluation, independent of observed attempt metadata.
use std::hash::Hash;
use std::hash::Hasher;

use super::*;

pub(crate) mod budget;
pub(super) mod environment;
mod evaluate;
mod events;
pub(crate) mod facts;
mod inputs;
pub(crate) use inputs::PreparedSemanticBzlEvaluation;
pub(crate) use inputs::prepare;

#[derive(Debug, Clone, Allocative)]
pub(crate) struct SemanticBzlEvaluationKey(Arc<PreparedSemanticBzlEvaluation>);
impl PartialEq for SemanticBzlEvaluationKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for SemanticBzlEvaluationKey {}
impl Hash for SemanticBzlEvaluationKey {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}
impl fmt::Display for SemanticBzlEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "semantic-bzl-evaluation:{}",
            self.0.rows[0].key.identity.label
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum SemanticBzlEvaluationError {
    Parse(Arc<str>),
    Evaluation(Arc<str>),
    Freeze(Arc<str>),
    Resource(Arc<str>),
}
#[derive(Debug, Allocative)]
pub(crate) struct SemanticBzlEvaluationResult {
    inputs: Arc<PreparedSemanticBzlEvaluation>,
    result: Result<FrozenBzlModule, SemanticBzlEvaluationError>,
    events: Arc<EventBatch>,
}
impl SemanticBzlEvaluationResult {
    pub(crate) fn result(&self) -> Result<&FrozenBzlModule, &SemanticBzlEvaluationError> {
        self.result.as_ref()
    }
    pub(crate) fn events(&self) -> &Arc<EventBatch> {
        &self.events
    }
}
impl PartialEq for SemanticBzlEvaluationResult {
    fn eq(&self, other: &Self) -> bool {
        self.inputs == other.inputs && self.result == other.result && self.events == other.events
    }
}
impl Eq for SemanticBzlEvaluationResult {}
#[async_trait]
impl Key for SemanticBzlEvaluationKey {
    type Value = Arc<SemanticBzlEvaluationResult>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let direct = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_err();
        let result = Arc::new(evaluate::evaluate(self.0.clone(), direct));
        // Builtins have no Host/external outer wrapper. Their actual semantic
        // producer activation owns only its own local events; reuse has none.
        if !direct
            && self.0.environment.mode() == crate::bzl_environment::BzlEnvironmentMode::Builtin
        {
            ctx.store_evaluation_data(result.events().as_ref().clone())
                .expect("builtin semantic producer stores one local event batch");
        }
        result
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
    fn validity(value: &Self::Value) -> bool {
        // Admission/work failures must not canonize caller-dependent physical
        // allocation history. Complete source/evaluation terminals stay cached.
        !matches!(value.result, Err(SemanticBzlEvaluationError::Resource(_)))
    }
}

#[cfg(test)]
mod inputs_tests;

#[cfg(test)]
mod parser_copy_tests;

#[cfg(test)]
fn test_environment() -> crate::bzl_environment::PreparedBzlEnvironment {
    crate::bzl_environment::PreparedBzlEnvironment::uninjected(
        crate::bzl_environment::BzlEnvironmentMode::BzlmodBootstrap,
    )
    .unwrap()
}
