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
mod resource_storage;

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

#[derive(Debug, Allocative)]
pub(in crate::starlark_rule) struct PendingSpawn {
    header: ActionSpec,
    command_line: PendingCommandLine,
    inputs: Option<inputs::PendingSpawnInputs>,
    resource_set: Option<slug_loading_v2::resource_set::ResourceSetRegistration>,
}

#[derive(Debug, Clone, Allocative)]
pub(in crate::starlark_rule) struct PendingArgsWrite {
    header: ActionSpec,
    registration: ArgsSnapshotRegistration,
}

#[derive(Debug, Allocative)]
pub(in crate::starlark_rule) enum PendingRegisteredAction {
    Ready(ActionSpec),
    Spawn(Box<PendingSpawn>),
    ArgsWrite(PendingArgsWrite),
}

impl From<ActionSpec> for PendingRegisteredAction {
    fn from(action: ActionSpec) -> Self {
        Self::Ready(action)
    }
}

impl PendingRegisteredAction {
    pub(in crate::starlark_rule) fn spawn(
        module: &Module,
        header: ActionSpec,
        command_line: PendingCommandLine,
    ) -> anyhow::Result<Self> {
        resource_storage::pending(module)?;
        let spawn = header
            .spawn_spec()
            .ok_or_else(|| anyhow::anyhow!("pending Spawn requires a typed native header"))?;
        anyhow::ensure!(
            spawn.command_line().segments().is_empty(),
            "pending Spawn header must not contain a command-line recipe"
        );
        Ok(Self::Spawn(Box::new(PendingSpawn {
            header,
            command_line,
            inputs: None,
            resource_set: None,
        })))
    }

    pub(in crate::starlark_rule) fn with_resource_set<'v>(
        mut self,
        module: &'v Module,
        value: Option<Value<'v>>,
    ) -> anyhow::Result<Self> {
        let Self::Spawn(action) = &mut self else {
            anyhow::bail!("only Spawn accepts resource_set");
        };
        if let Some(value) = value {
            action.resource_set = Some(slug_loading_v2::resource_set::register_resource_set(
                module, value,
            )?);
        }
        Ok(self)
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

    #[cfg(test)]
    pub(in crate::starlark_rule) fn prepare<'v, R: Clone>(
        &self,
        module: &'v Module,
        publish: &mut impl FnMut(EvaluatorArgsSnapshot<'v>) -> anyhow::Result<R>,
        admit_spawn: &mut impl FnMut(usize, usize) -> Result<(), String>,
    ) -> anyhow::Result<PreparedAction<R>> {
        self.prepare_with_inputs(
            module,
            None,
            publish,
            &mut |_| anyhow::bail!("resource publisher required"),
            admit_spawn,
        )
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
        publish_resource: &mut impl FnMut(
            &slug_loading_v2::resource_set::ResourceSetRegistration,
        ) -> anyhow::Result<
            super::super::resource_set::DraftResourceSet,
        >,
        admit_spawn: &mut impl FnMut(usize, usize) -> Result<(), String>,
    ) -> anyhow::Result<PreparedAction<R>> {
        match self {
            Self::Ready(action) => Ok(PreparedAction::Ready(action.clone())),
            Self::Spawn(action) => {
                anyhow::ensure!(
                    action.inputs.is_none() || lowered_inputs.is_some(),
                    "pending Spawn inputs must be lowered with the shared publication owner"
                );
                resource_storage::prepared::<R>(admit_spawn).map_err(anyhow::Error::msg)?;
                Ok(PreparedAction::Spawn(Box::new(PreparedSpawn {
                    header: action.header.clone(),
                    command_line: action.command_line.prepare(module, publish)?,
                    inputs: lowered_inputs,
                    resource_set: action
                        .resource_set
                        .as_ref()
                        .map(publish_resource)
                        .transpose()?,
                })))
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

#[derive(Debug)]
pub(in crate::starlark_rule) struct PreparedSpawn<R> {
    pub(in crate::starlark_rule) header: ActionSpec,
    pub(in crate::starlark_rule) command_line: PreparedCommandLine<R>,
    pub(in crate::starlark_rule) inputs: Option<(ArtifactInputs, ArtifactInputs)>,
    pub(in crate::starlark_rule) resource_set: Option<super::super::resource_set::DraftResourceSet>,
}
#[derive(Debug)]
pub(in crate::starlark_rule) enum PreparedAction<R> {
    Ready(ActionSpec),
    Spawn(Box<PreparedSpawn<R>>),
    ArgsWrite { header: ActionSpec, recipe: R },
}

fn decorations(header: &ActionSpec, action: ActionSpec) -> ActionSpec {
    let action = action.with_exec_properties(header.exec_properties().clone());
    match header.exec_group() {
        Some(group) => action.with_exec_group(group),
        None => action,
    }
}

impl<R> PreparedAction<R> {
    #[cfg(test)]
    pub(in crate::starlark_rule) fn complete(
        self,
        publish: &mut impl FnMut(
            R,
        ) -> anyhow::Result<(
            RetainedArgsRecipe,
            Option<RetainedSpawnParamFilePolicy>,
        )>,
    ) -> anyhow::Result<ActionSpec> {
        self.complete_with_resource_sets(publish, &mut |_| {
            anyhow::bail!("resource completion required")
        })
    }
    pub(in crate::starlark_rule) fn complete_with_resource_sets(
        self,
        publish: &mut impl FnMut(
            R,
        ) -> anyhow::Result<(
            RetainedArgsRecipe,
            Option<RetainedSpawnParamFilePolicy>,
        )>,
        publish_resource: &mut impl FnMut(
            super::super::resource_set::DraftResourceSet,
        ) -> anyhow::Result<
            slug_build_api_v2::actions::RetainedSpawnResourceSet,
        >,
    ) -> anyhow::Result<ActionSpec> {
        match self {
            Self::Ready(action) => Ok(action),
            Self::Spawn(spawn) => {
                let PreparedSpawn {
                    header,
                    command_line,
                    inputs,
                    resource_set,
                } = *spawn;
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
                if let Some(resource_set) = resource_set {
                    spawn = spawn.with_resource_set(publish_resource(resource_set)?);
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
