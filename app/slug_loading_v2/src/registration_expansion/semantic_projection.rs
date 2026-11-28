use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use dice::DiceComputations;
use dice::DiceProjectionComputations;
use dice::Key;
use dice::ProjectionKey;
use dupe::Dupe;
use slug_bzlmod_v2::SourcePreparationOutcome;

use super::CommandRegistrationExpansionObservationKey;
use super::ModuleRegistrationExpansion;
use super::ModuleRegistrationExpansionObservationError;
use super::ModuleRegistrationExpansionObservationKey;

type SemanticExpansion = SourcePreparationOutcome<
    Result<Arc<ModuleRegistrationExpansion>, ModuleRegistrationExpansionObservationError>,
>;

fn project<T>(
    observed: &SourcePreparationOutcome<Result<T, ModuleRegistrationExpansionObservationError>>,
    result: impl FnOnce(&T) -> &Arc<ModuleRegistrationExpansion>,
) -> SemanticExpansion {
    match observed {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need.dupe()),
        SourcePreparationOutcome::Complete(value) => SourcePreparationOutcome::Complete(
            value
                .as_ref()
                .map(|value| result(value).dupe())
                .map_err(Dupe::dupe),
        ),
    }
}

impl ModuleRegistrationExpansionObservationKey {
    /// Read the expansion result while its current observed producer stays in
    /// the activation closure for path selection and registration warnings.
    #[doc(hidden)]
    pub async fn compute_semantic(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> anyhow::Result<SemanticExpansion> {
        let observed = ctx.compute_opaque(self).await?;
        Ok(ctx.projection(&observed, &ModuleResultProjection)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
struct ModuleResultProjection;

impl fmt::Display for ModuleResultProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("module-registration-result")
    }
}

impl ProjectionKey for ModuleResultProjection {
    type DeriveFromKey = ModuleRegistrationExpansionObservationKey;
    type Value = SemanticExpansion;

    fn compute(
        &self,
        base: &<Self::DeriveFromKey as Key>::Value,
        _: &DiceProjectionComputations<'_>,
    ) -> Self::Value {
        project(base, |observed| observed.result())
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.complete_eq(right)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

impl CommandRegistrationExpansionObservationKey {
    /// Read the expansion result while its current observed producer stays in
    /// the activation closure for path selection and registration warnings.
    #[doc(hidden)]
    pub async fn compute_semantic(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> anyhow::Result<SemanticExpansion> {
        let observed = ctx.compute_opaque(self).await?;
        Ok(ctx.projection(&observed, &CommandResultProjection)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative)]
struct CommandResultProjection;

impl fmt::Display for CommandResultProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("command-registration-result")
    }
}

impl ProjectionKey for CommandResultProjection {
    type DeriveFromKey = CommandRegistrationExpansionObservationKey;
    type Value = SemanticExpansion;

    fn compute(
        &self,
        base: &<Self::DeriveFromKey as Key>::Value,
        _: &DiceProjectionComputations<'_>,
    ) -> Self::Value {
        project(base, |observed| observed.result())
    }

    fn equality(left: &Self::Value, right: &Self::Value) -> bool {
        left.complete_eq(right)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}
