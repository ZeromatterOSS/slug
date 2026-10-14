//! Private ordered action rows retain only native headers and traced-snapshot tokens.
//! Publication has a module-borrowing prepare phase and a module-free finish phase.
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use slug_build_api_v2::ActionError;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ArgsWriteSpec;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::RegisterableAction;
use slug_build_api_v2::RetainedArgsRecipe;
use slug_build_api_v2::RetainedCommandLine;
use slug_build_api_v2::RetainedCommandLineSegment;
use slug_build_api_v2::RetainedParamFileFormat;
use slug_build_api_v2::RetainedSpawnArgsSnapshot;
use slug_build_api_v2::RetainedSpawnParamFilePolicy;
use slug_build_api_v2::SpawnSpec;
use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
use slug_loading_v2::subrule_invocation::ArgsSnapshotRegistration;
use slug_loading_v2::subrule_invocation::EvaluatorArgsSnapshot;
use slug_loading_v2::subrule_invocation::StarlarkArgs;
use slug_loading_v2::subrule_invocation::args_snapshot;
use slug_loading_v2::subrule_invocation::register_args_snapshot;
use starlark::environment::Module;
use starlark::values::Value;

use crate::analysis_value::AnalysisValueLowerer;
use crate::files_to_run_spawn::ExecutableArtifactProvenance;

mod inputs;

#[derive(Debug, Clone, Allocative)]
enum PendingCommandLineSegment {
    LiteralRun(Arc<[CompactString]>),
    ArgsSnapshot(ArgsSnapshotRegistration),
}

#[derive(Debug, Clone, Default, Allocative)]
pub(in crate::starlark_rule) struct PendingCommandLine {
    segments: Vec<PendingCommandLineSegment>,
}

impl PendingCommandLine {
    pub(in crate::starlark_rule) fn capture<'v>(
        module: &'v Module,
        arguments: Option<Value<'v>>,
    ) -> anyhow::Result<(Self, bool)> {
        let Some(arguments) = arguments else {
            return Ok((Self::default(), false));
        };
        let arguments = super::super::sequence_values(arguments)
            .ok_or_else(|| anyhow::anyhow!("ctx.actions.run arguments must be a sequence"))?;
        let has_arguments = !arguments.is_empty();
        let mut segments = Vec::new();
        let mut literals = Vec::new();
        for argument in arguments {
            if let Some(snapshot) = StarlarkArgs::snapshot(argument) {
                if !literals.is_empty() {
                    segments.push(PendingCommandLineSegment::LiteralRun(
                        std::mem::take(&mut literals).into(),
                    ));
                }
                segments.push(PendingCommandLineSegment::ArgsSnapshot(
                    register_args_snapshot(module, snapshot)?,
                ));
            } else if let Some(literal) = argument.unpack_str() {
                literals.push(CompactString::new(literal));
            } else {
                anyhow::bail!("ctx.actions.run arguments entries must be strings or Args");
            }
        }
        if !literals.is_empty() {
            segments.push(PendingCommandLineSegment::LiteralRun(literals.into()));
        }
        Ok((Self { segments }, has_arguments))
    }

    fn prepare<'v, R: Clone>(
        &self,
        module: &'v Module,
        publish: &mut impl FnMut(EvaluatorArgsSnapshot<'v>) -> anyhow::Result<R>,
    ) -> anyhow::Result<PreparedCommandLine<R>> {
        let segments = self
            .segments
            .iter()
            .map(|segment| match segment {
                PendingCommandLineSegment::LiteralRun(values) => {
                    Ok(PreparedCommandLineSegment::LiteralRun(values.clone()))
                }
                PendingCommandLineSegment::ArgsSnapshot(registration) => {
                    let draft = publish(args_snapshot(module, registration)?)?;
                    Ok(PreparedCommandLineSegment::ArgsSnapshot(draft))
                }
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(PreparedCommandLine { segments })
    }
}

#[derive(Debug, Clone)]
enum PreparedCommandLineSegment<R> {
    LiteralRun(Arc<[CompactString]>),
    ArgsSnapshot(R),
}

#[derive(Debug, Clone)]
pub(in crate::starlark_rule) struct PreparedCommandLine<R> {
    segments: Vec<PreparedCommandLineSegment<R>>,
}

impl<R> PreparedCommandLine<R> {
    fn complete(
        self,
        publish: &mut impl FnMut(
            R,
        ) -> anyhow::Result<(
            RetainedArgsRecipe,
            Option<RetainedSpawnParamFilePolicy>,
        )>,
    ) -> anyhow::Result<RetainedCommandLine> {
        let segments = self
            .segments
            .into_iter()
            .map(|segment| match segment {
                PreparedCommandLineSegment::LiteralRun(values) => {
                    Ok(RetainedCommandLineSegment::LiteralRun(values))
                }
                PreparedCommandLineSegment::ArgsSnapshot(draft) => {
                    let (recipe, policy) = publish(draft)?;
                    Ok(RetainedCommandLineSegment::ArgsSnapshot(
                        RetainedSpawnArgsSnapshot::new(recipe, policy),
                    ))
                }
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(RetainedCommandLine::new(segments))
    }
}

#[derive(Debug, Clone, Allocative)]
pub(in crate::starlark_rule) struct PendingSpawn {
    header: ActionSpec,
    command_line: PendingCommandLine,
    inputs: Option<inputs::PendingSpawnInputs>,
}

#[derive(Debug, Clone, Allocative)]
pub(in crate::starlark_rule) struct PendingArgsWrite {
    header: ActionSpec,
    registration: ArgsSnapshotRegistration,
}

#[derive(Debug, Clone, Allocative)]
pub(in crate::starlark_rule) enum PendingRegisteredAction {
    Ready(ActionSpec),
    Spawn(PendingSpawn),
    ArgsWrite(PendingArgsWrite),
}

impl From<ActionSpec> for PendingRegisteredAction {
    fn from(action: ActionSpec) -> Self {
        Self::Ready(action)
    }
}

impl PendingRegisteredAction {
    pub(in crate::starlark_rule) fn spawn(
        header: ActionSpec,
        command_line: PendingCommandLine,
    ) -> anyhow::Result<Self> {
        let spawn = header
            .spawn_spec()
            .ok_or_else(|| anyhow::anyhow!("pending Spawn requires a typed native header"))?;
        anyhow::ensure!(
            spawn.command_line().segments().is_empty(),
            "pending Spawn header must not contain a command-line recipe"
        );
        Ok(Self::Spawn(PendingSpawn {
            header,
            command_line,
            inputs: None,
        }))
    }

    pub(in crate::starlark_rule) fn args_write(
        output: ActionOutput,
        registration: ArgsSnapshotRegistration,
        is_executable: bool,
    ) -> Self {
        Self::ArgsWrite(PendingArgsWrite {
            header: ActionSpec::args_write(ArgsWriteSpec::new(
                output,
                RetainedArgsRecipe::new(Vec::new(), RetainedParamFileFormat::Multiline),
                is_executable,
            )),
            registration,
        })
    }

    fn header(&self) -> &ActionSpec {
        match self {
            Self::Ready(action) => action,
            Self::Spawn(action) => &action.header,
            Self::ArgsWrite(action) => &action.header,
        }
    }

    pub(in crate::starlark_rule) fn prepare<'v, R: Clone>(
        &self,
        module: &'v Module,
        publish: &mut impl FnMut(EvaluatorArgsSnapshot<'v>) -> anyhow::Result<R>,
    ) -> anyhow::Result<PreparedAction<R>> {
        self.prepare_with_inputs(module, None, publish)
    }

    pub(in crate::starlark_rule) fn with_inputs<'v>(
        mut self,
        module: &'v Module,
        inputs: Option<Value<'v>>,
        tools: Option<Value<'v>>,
        scope: AnalysisActionCallScope,
    ) -> anyhow::Result<Self> {
        let Self::Spawn(action) = &mut self else {
            anyhow::bail!("only pending Spawn rows accept input snapshots");
        };
        anyhow::ensure!(
            action.inputs.is_none(),
            "pending Spawn input snapshots were already captured"
        );
        action.inputs = Some(inputs::PendingSpawnInputs::capture(
            module, inputs, tools, scope,
        )?);
        Ok(self)
    }

    pub(in crate::starlark_rule) fn lower_inputs<'v>(
        &self,
        module: &'v Module,
        lowerer: &mut AnalysisValueLowerer<'v>,
        provenance: &ExecutableArtifactProvenance,
    ) -> anyhow::Result<Option<(ArtifactInputs, ArtifactInputs)>> {
        match self {
            Self::Spawn(action) => action
                .inputs
                .as_ref()
                .map(|inputs| inputs.lower(module, lowerer, provenance))
                .transpose(),
            _ => Ok(None),
        }
    }

    pub(in crate::starlark_rule) fn prepare_with_inputs<'v, R: Clone>(
        &self,
        module: &'v Module,
        lowered_inputs: Option<(ArtifactInputs, ArtifactInputs)>,
        publish: &mut impl FnMut(EvaluatorArgsSnapshot<'v>) -> anyhow::Result<R>,
    ) -> anyhow::Result<PreparedAction<R>> {
        match self {
            Self::Ready(action) => Ok(PreparedAction::Ready(action.clone())),
            Self::Spawn(action) => {
                anyhow::ensure!(
                    action.inputs.is_none() || lowered_inputs.is_some(),
                    "pending Spawn inputs must be lowered with the shared publication owner"
                );
                Ok(PreparedAction::Spawn {
                    header: action.header.clone(),
                    command_line: action.command_line.prepare(module, publish)?,
                    inputs: lowered_inputs,
                })
            }
            Self::ArgsWrite(action) => Ok(PreparedAction::ArgsWrite {
                header: action.header.clone(),
                recipe: publish(args_snapshot(module, &action.registration)?)?,
            }),
        }
    }
}

impl RegisterableAction for PendingRegisteredAction {
    fn outputs(&self) -> &[ActionOutput] {
        self.header().outputs()
    }
    fn mnemonic(&self) -> &str {
        self.header().mnemonic()
    }
    fn validate(&self) -> Result<(), ActionError> {
        RegisterableAction::validate(self.header())
    }
    fn as_action_spec(&self) -> Option<&ActionSpec> {
        match self {
            Self::Ready(action) => Some(action),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::starlark_rule) enum PreparedAction<R> {
    Ready(ActionSpec),
    Spawn {
        header: ActionSpec,
        command_line: PreparedCommandLine<R>,
        inputs: Option<(ArtifactInputs, ArtifactInputs)>,
    },
    ArgsWrite {
        header: ActionSpec,
        recipe: R,
    },
}

fn decorations(header: &ActionSpec, action: ActionSpec) -> ActionSpec {
    let action = action.with_exec_properties(header.exec_properties().clone());
    match header.exec_group() {
        Some(group) => action.with_exec_group(group),
        None => action,
    }
}

impl<R> PreparedAction<R> {
    pub(in crate::starlark_rule) fn complete(
        self,
        publish: &mut impl FnMut(
            R,
        ) -> anyhow::Result<(
            RetainedArgsRecipe,
            Option<RetainedSpawnParamFilePolicy>,
        )>,
    ) -> anyhow::Result<ActionSpec> {
        match self {
            Self::Ready(action) => Ok(action),
            Self::Spawn {
                header,
                command_line,
                inputs,
            } => {
                let prototype = header
                    .spawn_spec()
                    .expect("pending header was checked as Spawn");
                let (inputs, tools) = inputs
                    .unwrap_or_else(|| (prototype.inputs().clone(), prototype.tools().clone()));
                let mut spawn = SpawnSpec::new(
                    prototype.invocation().clone(),
                    command_line.complete(publish)?,
                    inputs,
                    tools,
                    prototype.outputs().to_vec(),
                    prototype.unused_inputs_list().cloned(),
                    prototype.environment().clone(),
                    prototype.execution_requirements().clone(),
                    prototype.mnemonic(),
                    prototype.progress_message(),
                );
                if let Some(resource_set) = prototype.resource_set() {
                    spawn = spawn.with_resource_set(resource_set);
                }
                Ok(decorations(&header, ActionSpec::spawn(spawn)))
            }
            Self::ArgsWrite { header, recipe } => {
                let prototype = header
                    .args_write_spec()
                    .expect("pending header was built as ArgsWrite");
                let (recipe, _) = publish(recipe)?;
                let action = ActionSpec::args_write(ArgsWriteSpec::new(
                    prototype.output().clone(),
                    recipe,
                    prototype.is_executable(),
                ));
                Ok(decorations(&header, action))
            }
        }
    }
}

#[cfg(test)]
mod tests;
