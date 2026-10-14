//! DICE-owned actual module evaluation, independent of observed attempt metadata.
use std::hash::Hash;
use std::hash::Hasher;

use super::*;

mod budget;
mod evaluate;
mod events;
mod facts;
mod inputs;
pub(super) use inputs::PreparedSemanticBzlEvaluation;
pub(super) use inputs::prepare;

#[derive(Debug, Clone, Allocative)]
pub(super) struct SemanticBzlEvaluationKey(Arc<PreparedSemanticBzlEvaluation>);
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
pub(super) enum SemanticBzlEvaluationError {
    Parse(Arc<str>),
    Evaluation(Arc<str>),
    Freeze(Arc<str>),
    Resource(Arc<str>),
}
#[derive(Debug, Allocative)]
pub(super) struct SemanticBzlEvaluationResult {
    inputs: Arc<PreparedSemanticBzlEvaluation>,
    result: Result<FrozenBzlModule, SemanticBzlEvaluationError>,
    events: Arc<EventBatch>,
}
impl SemanticBzlEvaluationResult {
    pub(super) fn result(&self) -> Result<&FrozenBzlModule, &SemanticBzlEvaluationError> {
        self.result.as_ref()
    }
    pub(super) fn events(&self) -> &Arc<EventBatch> {
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
        Arc::new(evaluate::evaluate(self.0.clone(), direct))
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
