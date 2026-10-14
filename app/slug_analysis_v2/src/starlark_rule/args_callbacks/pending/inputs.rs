//! Preserve registration-time membership, then lower input/tool graphs together.
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::RetainedVectorOptions;
use slug_loading_v2::provider::StarlarkDepset;
use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
use slug_loading_v2::subrule_invocation::EvaluatorArgCallGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorArgGen;
use slug_loading_v2::subrule_invocation::EvaluatorVectorSourceGen;
use starlark::values::list::AllocList;

use super::*;
use crate::analysis_value::AnalysisValueLowerer;
use crate::files_to_run_spawn::ExecutableArtifactProvenance;

#[derive(Debug, Clone, Allocative)]
enum PendingInputValue {
    Absent,
    Sequence(ArgsSnapshotRegistration),
    Depset(ArgsSnapshotRegistration),
}

impl PendingInputValue {
    fn capture<'v>(
        module: &'v Module,
        value: Option<Value<'v>>,
        name: &str,
    ) -> anyhow::Result<Self> {
        let Some(value) = value else {
            return Ok(Self::Absent);
        };
        let (source, is_depset) =
            if StarlarkDepset::visit_metadata_from_value(value, &mut |_, _, _| Ok(()))
                .map_err(anyhow::Error::msg)?
            {
                (EvaluatorVectorSourceGen::Depset(value), true)
            } else {
                let values = super::super::super::sequence_values(value).ok_or_else(|| {
                    anyhow::anyhow!("ctx.actions.run {name} must be a sequence or depset")
                })?;
                (EvaluatorVectorSourceGen::Sequence(values), false)
            };
        let registration = register_args_snapshot(
            module,
            EvaluatorArgsSnapshot {
                calls: vec![EvaluatorArgCallGen::AddAll(EvaluatorVectorArgGen {
                    source,
                    options: RetainedVectorOptions {
                        arg_name: None,
                        format_each: None,
                        before_each: None,
                        join_with: None,
                        format_joined: None,
                        omit_if_empty: true,
                        uniquify: false,
                        expand_directories: false,
                        terminate_with: None,
                    },
                    map_each: None,
                    registration_location: None,
                })],
                format: RetainedParamFileFormat::Multiline,
                param_file: None,
            },
        )?;
        Ok(if is_depset {
            Self::Depset(registration)
        } else {
            Self::Sequence(registration)
        })
    }

    fn resolve<'v>(&self, module: &'v Module) -> anyhow::Result<Option<Value<'v>>> {
        let registration = match self {
            Self::Absent => return Ok(None),
            Self::Sequence(registration) | Self::Depset(registration) => registration,
        };
        let snapshot = args_snapshot(module, registration)?;
        let mut calls = snapshot.calls.into_iter();
        let Some(EvaluatorArgCallGen::AddAll(vector)) = calls.next() else {
            anyhow::bail!("invalid pending input membership snapshot");
        };
        anyhow::ensure!(
            calls.next().is_none() && vector.map_each.is_none(),
            "invalid pending input membership snapshot"
        );
        match (self, vector.source) {
            (Self::Sequence(_), EvaluatorVectorSourceGen::Sequence(values)) => {
                Ok(Some(module.heap().alloc(AllocList(values))))
            }
            (Self::Depset(_), EvaluatorVectorSourceGen::Depset(value)) => Ok(Some(value)),
            _ => anyhow::bail!("pending input membership snapshot shape changed"),
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub(super) struct PendingSpawnInputs {
    inputs: PendingInputValue,
    tools: PendingInputValue,
    scope: AnalysisActionCallScope,
}

impl PendingSpawnInputs {
    pub(super) fn capture<'v>(
        module: &'v Module,
        inputs: Option<Value<'v>>,
        tools: Option<Value<'v>>,
        scope: AnalysisActionCallScope,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            inputs: PendingInputValue::capture(module, inputs, "inputs")?,
            tools: PendingInputValue::capture(module, tools, "tools")?,
            scope,
        })
    }

    pub(super) fn lower<'v>(
        &self,
        module: &'v Module,
        lowerer: &mut AnalysisValueLowerer<'v>,
        provenance: &ExecutableArtifactProvenance,
    ) -> anyhow::Result<(ArtifactInputs, ArtifactInputs)> {
        let inputs = super::super::super::retained_artifact_inputs(
            self.inputs.resolve(module)?,
            false,
            true,
            "inputs",
            lowerer,
        )?;
        let tools = crate::files_to_run_spawn::retained_tools(
            self.tools.resolve(module)?,
            &self.scope,
            provenance,
            lowerer,
        )?;
        Ok((inputs, tools))
    }
}
