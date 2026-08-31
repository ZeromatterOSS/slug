/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::NormalizedAbsoluteBazelPath;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;

use crate::analysis_value::AnalysisArtifact;
use crate::analysis_value::AnalysisDepset;
use crate::analysis_value::AnalysisValueKind;
use crate::analysis_value::AnalysisValueType;
use crate::analysis_value::PublicationEqState;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ActionOutputKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ActionOutput {
    path: String,
    kind: ActionOutputKind,
}

impl ActionOutput {
    pub fn new(path: impl Into<String>, kind: ActionOutputKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn kind(&self) -> ActionOutputKind {
        self.kind
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub struct ActionInput {
    path: String,
    digest: Option<String>,
}

impl ActionInput {
    pub fn new(path: impl Into<String>, digest: Option<String>) -> Self {
        Self {
            path: path.into(),
            digest,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedArtifactInputs(AnalysisDepset);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RetainedArtifactInputsError {
    value_type: AnalysisValueType,
}

impl RetainedArtifactInputsError {
    pub fn value_type(&self) -> AnalysisValueType {
        self.value_type
    }
}

impl fmt::Display for RetainedArtifactInputsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "action inputs require a depset of File, got depset of {}",
            self.value_type
        )
    }
}

impl Error for RetainedArtifactInputsError {}

impl RetainedArtifactInputs {
    pub fn new(depset: AnalysisDepset) -> Result<Self, RetainedArtifactInputsError> {
        match depset.element_type() {
            AnalysisValueType::Empty | AnalysisValueType::Artifact => Ok(Self(depset)),
            value_type => Err(RetainedArtifactInputsError { value_type }),
        }
    }

    pub fn visit(
        &self,
        mut visitor: impl FnMut(&AnalysisArtifact),
    ) -> Result<(), RetainedArtifactInputsError> {
        let mut invalid = None;
        self.0
            .visit(|value| {
                match value.kind() {
                    AnalysisValueKind::Artifact(artifact) => visitor(artifact),
                    _ => invalid = Some(value.value_type()),
                }
                Ok::<_, Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        match invalid {
            Some(value_type) => Err(RetainedArtifactInputsError { value_type }),
            None => Ok(()),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.publication_eq_with(&other.0, state)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum RetainedScalarValue {
    String(CompactString),
    Integer(CompactString),
    Artifact(AnalysisArtifact),
}

impl RetainedScalarValue {
    fn render(&self) -> String {
        match self {
            Self::String(value) | Self::Integer(value) => value.to_string(),
            Self::Artifact(artifact) => artifact.path().into_owned(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedScalarArg {
    arg_name: Option<CompactString>,
    value: RetainedScalarValue,
    format: Option<CompactString>,
}

impl RetainedScalarArg {
    pub fn new(
        arg_name: Option<impl Into<CompactString>>,
        value: RetainedScalarValue,
        format: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            arg_name: arg_name.map(Into::into),
            value,
            format: format.map(Into::into),
        }
    }

    fn append_rendered(&self, argv: &mut Vec<String>) {
        if let Some(name) = &self.arg_name {
            argv.push(name.to_string());
        }
        let value = self.value.render();
        argv.push(match &self.format {
            Some(format) => apply_validated_format(format, &value),
            None => value,
        });
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum RetainedCommandLineSegment {
    LiteralRun(Arc<[CompactString]>),
    ArgsSnapshot(Arc<[RetainedScalarArg]>),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct RetainedCommandLine(Arc<[RetainedCommandLineSegment]>);

impl RetainedCommandLine {
    pub fn new(segments: impl Into<Arc<[RetainedCommandLineSegment]>>) -> Self {
        Self(segments.into())
    }

    pub fn render(&self) -> Vec<String> {
        let mut argv = Vec::new();
        for segment in self.0.iter() {
            match segment {
                RetainedCommandLineSegment::LiteralRun(values) => {
                    argv.extend(values.iter().map(ToString::to_string));
                }
                RetainedCommandLineSegment::ArgsSnapshot(values) => {
                    for value in values.iter() {
                        value.append_rendered(&mut argv);
                    }
                }
            }
        }
        argv
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum SpawnExecutable {
    Path(NormalizedBazelPath),
    Artifact(AnalysisArtifact),
}

impl SpawnExecutable {
    pub fn render(&self) -> String {
        match self {
            Self::Path(path) => path.as_str().to_owned(),
            Self::Artifact(artifact) => artifact.path().into_owned(),
        }
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum ArtifactInputSource {
    Direct(AnalysisArtifact),
    Depset(RetainedArtifactInputs),
}

impl ArtifactInputSource {
    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Direct(left), Self::Direct(right)) => left == right,
            (Self::Depset(left), Self::Depset(right)) => left.publication_eq_with(right, state),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct ArtifactInputs(Arc<[ArtifactInputSource]>);

impl ArtifactInputs {
    pub fn new(inputs: impl Into<Arc<[ArtifactInputSource]>>) -> Self {
        Self(inputs.into())
    }

    pub fn sources(&self) -> &[ArtifactInputSource] {
        &self.0
    }

    pub fn visit(
        &self,
        mut visitor: impl FnMut(&AnalysisArtifact),
    ) -> Result<(), RetainedArtifactInputsError> {
        for source in self.0.iter() {
            match source {
                ArtifactInputSource::Direct(artifact) => visitor(artifact),
                ArtifactInputSource::Depset(inputs) => inputs.visit(&mut visitor)?,
            }
        }
        Ok(())
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(left, right)| left.publication_eq_with(right, state))
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct SpawnSpec {
    executable: SpawnExecutable,
    command_line: RetainedCommandLine,
    inputs: ArtifactInputs,
    tools: ArtifactInputs,
    outputs: Arc<[ActionOutput]>,
    environment: RetainedActionEnvironment,
    execution_requirements: CanonicalStringMap,
    mnemonic: CompactString,
    progress_message: Option<CompactString>,
}

impl SpawnSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executable: SpawnExecutable,
        command_line: RetainedCommandLine,
        inputs: ArtifactInputs,
        tools: ArtifactInputs,
        outputs: impl Into<Arc<[ActionOutput]>>,
        environment: RetainedActionEnvironment,
        execution_requirements: CanonicalStringMap,
        mnemonic: impl Into<CompactString>,
        progress_message: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            executable,
            command_line,
            inputs,
            tools,
            outputs: outputs.into(),
            environment,
            execution_requirements,
            mnemonic: mnemonic.into(),
            progress_message: progress_message.map(Into::into),
        }
    }

    pub fn executable(&self) -> &SpawnExecutable {
        &self.executable
    }
    pub fn command_line(&self) -> &RetainedCommandLine {
        &self.command_line
    }
    pub fn inputs(&self) -> &ArtifactInputs {
        &self.inputs
    }
    pub fn tools(&self) -> &ArtifactInputs {
        &self.tools
    }
    pub fn outputs(&self) -> &[ActionOutput] {
        &self.outputs
    }
    pub fn environment(&self) -> &RetainedActionEnvironment {
        &self.environment
    }
    pub fn execution_requirements(&self) -> &CanonicalStringMap {
        &self.execution_requirements
    }
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }
    pub fn progress_message(&self) -> Option<&str> {
        self.progress_message.as_deref()
    }
    pub fn render_argv(&self) -> Vec<String> {
        let mut argv = vec![self.executable.render()];
        argv.extend(self.command_line.render());
        argv
    }

    fn publication_eq(&self, other: &Self) -> bool {
        let mut state = PublicationEqState::default();
        self.executable == other.executable
            && self.command_line == other.command_line
            && self.outputs == other.outputs
            && self.environment == other.environment
            && self.execution_requirements == other.execution_requirements
            && self.mnemonic == other.mnemonic
            && self.progress_message == other.progress_message
            && self.inputs.publication_eq_with(&other.inputs, &mut state)
            && self.tools.publication_eq_with(&other.tools, &mut state)
    }
}

impl PartialEq for SpawnSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq(other)
    }
}

impl Eq for SpawnSpec {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum SymlinkTarget {
    Artifact {
        input: AnalysisArtifact,
        require_executable: bool,
        use_exec_root_for_source: bool,
    },
    AbsolutePath {
        target: NormalizedAbsoluteBazelPath,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct SymlinkSpec {
    output: ActionOutput,
    target: SymlinkTarget,
    progress_message: Option<CompactString>,
}

impl SymlinkSpec {
    pub fn new(
        output: ActionOutput,
        target: SymlinkTarget,
        progress_message: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            output,
            target,
            progress_message: progress_message.map(Into::into),
        }
    }
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn target(&self) -> &SymlinkTarget {
        &self.target
    }
    pub fn progress_message(&self) -> Option<&str> {
        self.progress_message.as_deref()
    }
}

fn apply_validated_format(format: &str, value: &str) -> String {
    let mut result = String::with_capacity(format.len() + value.len());
    let mut chars = format.chars();
    while let Some(character) = chars.next() {
        if character != '%' {
            result.push(character);
            continue;
        }
        match chars.next() {
            Some('%') => result.push('%'),
            Some('s') => result.push_str(value),
            _ => unreachable!("format strings are validated before retention"),
        }
    }
    result
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Allocative)]
pub enum ParamFileFormat {
    Multiline,
    ShellQuoted,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ParamFile {
    path: String,
    args: Vec<String>,
    format: ParamFileFormat,
}

impl ParamFile {
    pub fn new(path: impl Into<String>, args: Vec<String>, format: ParamFileFormat) -> Self {
        Self {
            path: path.into(),
            args,
            format,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn format(&self) -> ParamFileFormat {
        self.format
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum ActionKind {
    Write {
        content: String,
        is_executable: bool,
    },
    WriteJson {
        content: String,
    },
    ExpandTemplate {
        template: ActionInput,
        substitutions: BTreeMap<String, String>,
    },
    Run,
    RunShell {
        command: String,
    },
    Symlink {
        target_path: String,
    },
    Spawn,
    ArtifactSymlink,
    AbsoluteSymlink,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct LegacyActionSpec {
    kind: ActionKind,
    mnemonic: String,
    argv: Vec<String>,
    env: BTreeMap<String, String>,
    execution_requirements: BTreeMap<String, String>,
    inputs: Vec<ActionInput>,
    tools: Vec<ActionInput>,
    outputs: Vec<ActionOutput>,
    param_files: Vec<ParamFile>,
    progress_message: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
enum ActionPayload {
    Legacy(LegacyActionSpec),
    Spawn(SpawnSpec),
    Symlink(SymlinkSpec),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ActionSpec {
    payload: ActionPayload,
    exec_properties: BTreeMap<String, String>,
    exec_group: Option<String>,
}

static SPAWN_KIND: ActionKind = ActionKind::Spawn;
static ARTIFACT_SYMLINK_KIND: ActionKind = ActionKind::ArtifactSymlink;
static ABSOLUTE_SYMLINK_KIND: ActionKind = ActionKind::AbsoluteSymlink;
static EMPTY_STRING_MAP: std::sync::LazyLock<BTreeMap<String, String>> =
    std::sync::LazyLock::new(BTreeMap::new);

impl ActionSpec {
    pub fn new(kind: ActionKind, mnemonic: impl Into<String>, outputs: Vec<ActionOutput>) -> Self {
        Self {
            payload: ActionPayload::Legacy(LegacyActionSpec {
                kind,
                mnemonic: mnemonic.into(),
                argv: Vec::new(),
                env: BTreeMap::new(),
                execution_requirements: BTreeMap::new(),
                inputs: Vec::new(),
                tools: Vec::new(),
                outputs,
                param_files: Vec::new(),
                progress_message: None,
            }),
            exec_properties: BTreeMap::new(),
            exec_group: None,
        }
    }

    pub fn spawn(spec: SpawnSpec) -> Self {
        Self::typed(ActionPayload::Spawn(spec))
    }

    pub fn symlink(spec: SymlinkSpec) -> Self {
        Self::typed(ActionPayload::Symlink(spec))
    }

    fn typed(payload: ActionPayload) -> Self {
        Self {
            payload,
            exec_properties: BTreeMap::new(),
            exec_group: None,
        }
    }

    pub fn kind(&self) -> &ActionKind {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.kind,
            ActionPayload::Spawn(_) => &SPAWN_KIND,
            ActionPayload::Symlink(spec) => match spec.target() {
                SymlinkTarget::Artifact { .. } => &ARTIFACT_SYMLINK_KIND,
                SymlinkTarget::AbsolutePath { .. } => &ABSOLUTE_SYMLINK_KIND,
            },
        }
    }

    pub fn mnemonic(&self) -> &str {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.mnemonic,
            ActionPayload::Spawn(spec) => spec.mnemonic(),
            ActionPayload::Symlink(_) => "Symlink",
        }
    }

    pub fn argv(&self) -> &[String] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.argv,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &[],
        }
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.env,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &EMPTY_STRING_MAP,
        }
    }

    pub fn execution_requirements(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.execution_requirements,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &EMPTY_STRING_MAP,
        }
    }

    pub fn inputs(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.inputs,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &[],
        }
    }

    pub fn tools(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.tools,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &[],
        }
    }

    pub fn outputs(&self) -> &[ActionOutput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.outputs,
            ActionPayload::Spawn(spec) => spec.outputs(),
            ActionPayload::Symlink(spec) => std::slice::from_ref(spec.output()),
        }
    }

    pub fn param_files(&self) -> &[ParamFile] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.param_files,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => &[],
        }
    }

    pub fn progress_message(&self) -> Option<&str> {
        match &self.payload {
            ActionPayload::Legacy(spec) => spec.progress_message.as_deref(),
            ActionPayload::Spawn(spec) => spec.progress_message(),
            ActionPayload::Symlink(spec) => spec.progress_message(),
        }
    }

    pub fn exec_properties(&self) -> &BTreeMap<String, String> {
        &self.exec_properties
    }

    pub fn exec_group(&self) -> Option<&str> {
        self.exec_group.as_deref()
    }

    pub fn with_argv(mut self, argv: Vec<String>) -> Self {
        self.legacy_mut().argv = argv;
        self
    }

    pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
        self.legacy_mut().env = env;
        self
    }

    pub fn with_execution_requirements(
        mut self,
        execution_requirements: BTreeMap<String, String>,
    ) -> Self {
        self.legacy_mut().execution_requirements = execution_requirements;
        self
    }

    pub fn with_inputs(mut self, inputs: Vec<ActionInput>) -> Self {
        self.legacy_mut().inputs = inputs;
        self
    }

    pub fn with_tools(mut self, tools: Vec<ActionInput>) -> Self {
        self.legacy_mut().tools = tools;
        self
    }

    pub fn with_param_files(mut self, param_files: Vec<ParamFile>) -> Self {
        self.legacy_mut().param_files = param_files;
        self
    }

    pub fn with_progress_message(mut self, progress_message: impl Into<String>) -> Self {
        self.legacy_mut().progress_message = Some(progress_message.into());
        self
    }

    pub fn with_exec_properties(mut self, exec_properties: BTreeMap<String, String>) -> Self {
        self.exec_properties = exec_properties;
        self
    }

    pub fn with_exec_group(mut self, exec_group: impl Into<String>) -> Self {
        self.exec_group = Some(exec_group.into());
        self
    }

    pub fn spawn_spec(&self) -> Option<&SpawnSpec> {
        match &self.payload {
            ActionPayload::Spawn(spec) => Some(spec),
            ActionPayload::Legacy(_) | ActionPayload::Symlink(_) => None,
        }
    }

    pub fn symlink_spec(&self) -> Option<&SymlinkSpec> {
        match &self.payload {
            ActionPayload::Symlink(spec) => Some(spec),
            ActionPayload::Legacy(_) | ActionPayload::Spawn(_) => None,
        }
    }

    pub fn is_typed_payload(&self) -> bool {
        !matches!(&self.payload, ActionPayload::Legacy(_))
    }

    pub fn render_argv(&self) -> Vec<String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => spec.argv.clone(),
            ActionPayload::Spawn(spec) => spec.render_argv(),
            ActionPayload::Symlink(_) => Vec::new(),
        }
    }

    fn legacy_mut(&mut self) -> &mut LegacyActionSpec {
        match &mut self.payload {
            ActionPayload::Legacy(spec) => spec,
            ActionPayload::Spawn(_) | ActionPayload::Symlink(_) => {
                panic!("legacy action builders cannot mutate typed action payloads")
            }
        }
    }
}
