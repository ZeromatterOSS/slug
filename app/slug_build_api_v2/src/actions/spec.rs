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
use fxhash::FxHashSet;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::NormalizedAbsoluteBazelPath;
use slug_configuration_v2::NormalizedBazelPath;
use slug_configuration_v2::RetainedActionEnvironment;

use crate::actions::runfiles_support::RunfilesSupportActionSpec;
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
    RunfilesTree,
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
    pub fn render(&self) -> String {
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

    fn render_group(&self) -> Vec<String> {
        let mut argv = Vec::with_capacity(2);
        if let Some(name) = &self.arg_name {
            argv.push(name.to_string());
        }
        let value = self.value.render();
        argv.push(match &self.format {
            Some(format) => apply_validated_format(format, &value),
            None => value,
        });
        argv
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct RetainedArgsDepset(AnalysisDepset);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RetainedArgsDepsetError {
    ValueType(AnalysisValueType),
    Directory,
}

impl fmt::Display for RetainedArgsDepsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueType(value_type) => write!(
                f,
                "Args vector requires strings, integers, or regular Files, got depset of {value_type}"
            ),
            Self::Directory => write!(f, "Args vector directory expansion is not supported"),
        }
    }
}

impl Error for RetainedArgsDepsetError {}

impl RetainedArgsDepset {
    pub fn new(depset: AnalysisDepset) -> Result<Self, RetainedArgsDepsetError> {
        match depset.element_type() {
            AnalysisValueType::Empty | AnalysisValueType::String | AnalysisValueType::Integer => {
                Ok(Self(depset))
            }
            AnalysisValueType::Artifact => {
                let mut directory = false;
                depset
                    .visit(|value| {
                        if matches!(
                            value.kind(),
                            AnalysisValueKind::Artifact(AnalysisArtifact::Derived { output, .. })
                                if output.kind() == ActionOutputKind::Directory
                        ) {
                            directory = true;
                        }
                        Ok::<_, Infallible>(())
                    })
                    .unwrap_or_else(|never| match never {});
                if directory {
                    Err(RetainedArgsDepsetError::Directory)
                } else {
                    Ok(Self(depset))
                }
            }
            value_type => Err(RetainedArgsDepsetError::ValueType(value_type)),
        }
    }

    fn render(&self) -> Vec<String> {
        let mut values = Vec::new();
        self.0
            .visit(|value| {
                values.push(match value.kind() {
                    AnalysisValueKind::String(value) => value.to_owned(),
                    AnalysisValueKind::Number(value) => {
                        render_analysis_integer(value.as_integer().expect("validated Args integer"))
                    }
                    AnalysisValueKind::Artifact(artifact) => artifact.path().into_owned(),
                    _ => unreachable!("validated Args depset element type"),
                });
                Ok::<_, Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
        values
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.publication_eq_with(&other.0, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedVectorSource {
    Sequence(Arc<[RetainedScalarValue]>),
    Depset(RetainedArgsDepset),
}

impl RetainedVectorSource {
    fn render(&self) -> Vec<String> {
        match self {
            Self::Sequence(values) => values.iter().map(RetainedScalarValue::render).collect(),
            Self::Depset(values) => values.render(),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Sequence(left), Self::Sequence(right)) => left == right,
            (Self::Depset(left), Self::Depset(right)) => left.publication_eq_with(right, state),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedVectorOptions {
    pub arg_name: Option<CompactString>,
    pub format_each: Option<CompactString>,
    pub before_each: Option<CompactString>,
    pub join_with: Option<CompactString>,
    pub format_joined: Option<CompactString>,
    pub omit_if_empty: bool,
    pub uniquify: bool,
    pub expand_directories: bool,
    pub terminate_with: Option<CompactString>,
}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedVectorArg {
    source: RetainedVectorSource,
    options: RetainedVectorOptions,
}

impl RetainedVectorArg {
    pub fn new(source: RetainedVectorSource, options: RetainedVectorOptions) -> Self {
        Self { source, options }
    }

    fn render_values(&self) -> Vec<String> {
        let mut values = self.source.render();
        if let Some(format) = &self.options.format_each {
            for value in &mut values {
                *value = apply_validated_format(format, value);
            }
        }
        if self.options.uniquify {
            let mut seen = FxHashSet::default();
            values.retain(|value| seen.insert(value.clone()));
        }
        values
    }

    fn render_add_all(&self) -> Vec<String> {
        let values = self.render_values();
        if values.is_empty() && self.options.omit_if_empty {
            return Vec::new();
        }
        let mut group = Vec::new();
        if let Some(name) = &self.options.arg_name {
            group.push(name.to_string());
        }
        for value in values {
            if let Some(before) = &self.options.before_each {
                group.push(before.to_string());
            }
            group.push(value);
        }
        if let Some(terminate) = &self.options.terminate_with {
            group.push(terminate.to_string());
        }
        group
    }

    fn render_add_joined(&self) -> Vec<String> {
        let values = self.render_values();
        if values.is_empty() && self.options.omit_if_empty {
            return Vec::new();
        }
        let mut group = Vec::with_capacity(2);
        if let Some(name) = &self.options.arg_name {
            group.push(name.to_string());
        }
        let joined = values.join(self.options.join_with.as_deref().unwrap_or_default());
        group.push(match &self.options.format_joined {
            Some(format) => apply_validated_format(format, &joined),
            None => joined,
        });
        group
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.options == other.options && self.source.publication_eq_with(&other.source, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedArgCall {
    Scalar(RetainedScalarArg),
    AddAll(RetainedVectorArg),
    AddJoined(RetainedVectorArg),
}

impl RetainedArgCall {
    fn render_group(&self) -> Vec<String> {
        match self {
            Self::Scalar(value) => value.render_group(),
            Self::AddAll(value) => value.render_add_all(),
            Self::AddJoined(value) => value.render_add_joined(),
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        match (self, other) {
            (Self::Scalar(left), Self::Scalar(right)) => left == right,
            (Self::AddAll(left), Self::AddAll(right))
            | (Self::AddJoined(left), Self::AddJoined(right)) => {
                left.publication_eq_with(right, state)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Allocative, Dupe)]
pub enum RetainedParamFileFormat {
    Shell,
    Multiline,
    FlagPerLine,
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct RetainedArgsRecipe {
    calls: Arc<[RetainedArgCall]>,
    write_format: RetainedParamFileFormat,
}

impl RetainedArgsRecipe {
    pub fn new(
        calls: impl Into<Arc<[RetainedArgCall]>>,
        write_format: RetainedParamFileFormat,
    ) -> Self {
        Self {
            calls: calls.into(),
            write_format,
        }
    }

    pub fn render(&self) -> Vec<String> {
        let mut argv = Vec::new();
        for call in self.calls.iter() {
            let group = call.render_group();
            if self.write_format == RetainedParamFileFormat::FlagPerLine {
                argv.extend(flag_per_line_group(group));
            } else {
                argv.extend(group);
            }
        }
        argv
    }

    pub fn render_write_content(&self) -> String {
        let mut lines = Vec::new();
        for call in self.calls.iter() {
            let group = call.render_group();
            match self.write_format {
                RetainedParamFileFormat::FlagPerLine => lines.extend(flag_per_line_group(group)),
                RetainedParamFileFormat::Shell => {
                    lines.extend(group.iter().map(|value| shell_escape(value)));
                }
                RetainedParamFileFormat::Multiline => {
                    lines.extend(group);
                }
            }
        }
        if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        }
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.write_format == other.write_format
            && self.calls.len() == other.calls.len()
            && self
                .calls
                .iter()
                .zip(other.calls.iter())
                .all(|(left, right)| left.publication_eq_with(right, state))
    }
}

fn flag_per_line_group(group: Vec<String>) -> Vec<String> {
    if group.len() < 2 {
        return group;
    }
    let first = &group[0];
    let rest = group[1..].join(" ");
    vec![if first.is_empty() {
        rest
    } else {
        format!("{first}={rest}")
    }]
}

impl PartialEq for RetainedArgsRecipe {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RetainedArgsRecipe {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct RetainedSpawnParamFilePolicy {
    flag_format: CompactString,
    use_always: bool,
}

impl RetainedSpawnParamFilePolicy {
    pub fn new(flag_format: impl Into<CompactString>, use_always: bool) -> Self {
        Self {
            flag_format: flag_format.into(),
            use_always,
        }
    }

    pub fn flag_format(&self) -> &str {
        &self.flag_format
    }

    pub fn use_always(&self) -> bool {
        self.use_always
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct RetainedSpawnArgsSnapshot {
    recipe: RetainedArgsRecipe,
    param_file: Option<RetainedSpawnParamFilePolicy>,
}

impl RetainedSpawnArgsSnapshot {
    pub fn new(
        recipe: RetainedArgsRecipe,
        param_file: Option<RetainedSpawnParamFilePolicy>,
    ) -> Self {
        Self { recipe, param_file }
    }

    pub fn recipe(&self) -> &RetainedArgsRecipe {
        &self.recipe
    }

    pub fn param_file(&self) -> Option<&RetainedSpawnParamFilePolicy> {
        self.param_file.as_ref()
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.param_file == other.param_file && self.recipe.publication_eq_with(&other.recipe, state)
    }
}

#[derive(Debug, Clone, Allocative)]
pub enum RetainedCommandLineSegment {
    LiteralRun(Arc<[CompactString]>),
    ArgsSnapshot(RetainedSpawnArgsSnapshot),
}

#[derive(Debug, Clone, Allocative, Dupe)]
pub struct RetainedCommandLine(Arc<[RetainedCommandLineSegment]>);

impl RetainedCommandLine {
    pub fn new(segments: impl Into<Arc<[RetainedCommandLineSegment]>>) -> Self {
        Self(segments.into())
    }

    pub fn segments(&self) -> &[RetainedCommandLineSegment] {
        &self.0
    }

    pub fn render(&self) -> Vec<String> {
        let mut argv = Vec::new();
        for segment in self.0.iter() {
            match segment {
                RetainedCommandLineSegment::LiteralRun(values) => {
                    argv.extend(values.iter().map(ToString::to_string));
                }
                RetainedCommandLineSegment::ArgsSnapshot(snapshot) => {
                    argv.extend(snapshot.recipe.render());
                }
            }
        }
        argv
    }

    fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(left, right)| match (left, right) {
                    (
                        RetainedCommandLineSegment::LiteralRun(left),
                        RetainedCommandLineSegment::LiteralRun(right),
                    ) => left == right,
                    (
                        RetainedCommandLineSegment::ArgsSnapshot(left),
                        RetainedCommandLineSegment::ArgsSnapshot(right),
                    ) => left.publication_eq_with(right, state),
                    _ => false,
                })
    }
}

impl PartialEq for RetainedCommandLine {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}

impl Eq for RetainedCommandLine {}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum SpawnExecutable {
    Path(NormalizedBazelPath),
    Artifact(AnalysisArtifact),
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub enum RetainedSpawnInvocation {
    Executable(SpawnExecutable),
    Shell {
        command: CompactString,
        pad_dollar_zero: bool,
    },
}

impl RetainedSpawnInvocation {
    pub fn render_prefix(&self) -> Vec<String> {
        match self {
            Self::Executable(executable) => vec![executable.render()],
            Self::Shell {
                command,
                pad_dollar_zero,
            } => {
                let mut prefix = vec![command.to_string()];
                if *pad_dollar_zero {
                    prefix.push(String::new());
                }
                prefix
            }
        }
    }
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
    invocation: RetainedSpawnInvocation,
    command_line: RetainedCommandLine,
    inputs: ArtifactInputs,
    tools: ArtifactInputs,
    outputs: Arc<[ActionOutput]>,
    unused_inputs_list: Option<AnalysisArtifact>,
    environment: RetainedActionEnvironment,
    execution_requirements: CanonicalStringMap,
    mnemonic: CompactString,
    progress_message: Option<CompactString>,
}

impl SpawnSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        invocation: RetainedSpawnInvocation,
        command_line: RetainedCommandLine,
        inputs: ArtifactInputs,
        tools: ArtifactInputs,
        outputs: impl Into<Arc<[ActionOutput]>>,
        unused_inputs_list: Option<AnalysisArtifact>,
        environment: RetainedActionEnvironment,
        execution_requirements: CanonicalStringMap,
        mnemonic: impl Into<CompactString>,
        progress_message: Option<impl Into<CompactString>>,
    ) -> Self {
        Self {
            invocation,
            command_line,
            inputs,
            tools,
            outputs: outputs.into(),
            unused_inputs_list,
            environment,
            execution_requirements,
            mnemonic: mnemonic.into(),
            progress_message: progress_message.map(Into::into),
        }
    }

    pub fn invocation(&self) -> &RetainedSpawnInvocation {
        &self.invocation
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
    pub fn unused_inputs_list(&self) -> Option<&AnalysisArtifact> {
        self.unused_inputs_list.as_ref()
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
        let mut argv = self.invocation.render_prefix();
        argv.extend(self.command_line.render());
        argv
    }

    fn publication_eq(&self, other: &Self) -> bool {
        let mut state = PublicationEqState::default();
        self.invocation == other.invocation
            && self.outputs == other.outputs
            && self.unused_inputs_list == other.unused_inputs_list
            && self.environment == other.environment
            && self.execution_requirements == other.execution_requirements
            && self.mnemonic == other.mnemonic
            && self.progress_message == other.progress_message
            && self
                .command_line
                .publication_eq_with(&other.command_line, &mut state)
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

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ArgsWriteSpec {
    output: ActionOutput,
    recipe: RetainedArgsRecipe,
    is_executable: bool,
    execution_requirements: CanonicalStringMap,
}

impl ArgsWriteSpec {
    pub fn new(output: ActionOutput, recipe: RetainedArgsRecipe, is_executable: bool) -> Self {
        Self {
            output,
            recipe,
            is_executable,
            execution_requirements: CanonicalStringMap::default(),
        }
    }

    pub fn output(&self) -> &ActionOutput {
        &self.output
    }

    pub fn recipe(&self) -> &RetainedArgsRecipe {
        &self.recipe
    }

    pub fn is_executable(&self) -> bool {
        self.is_executable
    }

    pub fn execution_requirements(&self) -> &CanonicalStringMap {
        &self.execution_requirements
    }

    pub fn render_content(&self) -> String {
        self.recipe.render_write_content()
    }
}

fn render_analysis_integer(value: &crate::analysis_value::AnalysisInteger) -> String {
    if value.magnitude().is_empty() {
        return "0".to_owned();
    }
    let mut digits = vec![0u8];
    for byte in value.magnitude() {
        let mut carry = u16::from(*byte);
        for digit in &mut digits {
            let next = u16::from(*digit) * 256 + carry;
            *digit = (next % 10) as u8;
            carry = next / 10;
        }
        while carry != 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    let mut result = String::with_capacity(digits.len() + usize::from(value.is_negative()));
    if value.is_negative() {
        result.push('-');
    }
    result.extend(digits.iter().rev().map(|digit| char::from(b'0' + *digit)));
    result
}

fn shell_escape(value: &str) -> String {
    let safe =
        |character: char| character.is_ascii_alphanumeric() || "@%-_+:,./".contains(character);
    if !value.is_empty()
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| safe(character) || (character == '~' && index != 0))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
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
    ArgsWrite,
    RepoMappingManifest,
    SourceSymlinkManifest,
    SymlinkTree,
    RunfilesTree,
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
    ArgsWrite(ArgsWriteSpec),
    RunfilesSupport(RunfilesSupportActionSpec),
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
static ARGS_WRITE_KIND: ActionKind = ActionKind::ArgsWrite;
static REPO_MAPPING_MANIFEST_KIND: ActionKind = ActionKind::RepoMappingManifest;
static SOURCE_SYMLINK_MANIFEST_KIND: ActionKind = ActionKind::SourceSymlinkManifest;
static SYMLINK_TREE_KIND: ActionKind = ActionKind::SymlinkTree;
static RUNFILES_TREE_KIND: ActionKind = ActionKind::RunfilesTree;
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

    pub fn args_write(spec: ArgsWriteSpec) -> Self {
        Self::typed(ActionPayload::ArgsWrite(spec))
    }

    pub fn runfiles_support(spec: RunfilesSupportActionSpec) -> Self {
        Self::typed(ActionPayload::RunfilesSupport(spec))
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
            ActionPayload::ArgsWrite(_) => &ARGS_WRITE_KIND,
            ActionPayload::RunfilesSupport(spec) => match spec {
                RunfilesSupportActionSpec::RepoMappingManifest { .. } => {
                    &REPO_MAPPING_MANIFEST_KIND
                }
                RunfilesSupportActionSpec::SourceSymlinkManifest { .. } => {
                    &SOURCE_SYMLINK_MANIFEST_KIND
                }
                RunfilesSupportActionSpec::SymlinkTree { .. } => &SYMLINK_TREE_KIND,
                RunfilesSupportActionSpec::RunfilesTree { .. } => &RUNFILES_TREE_KIND,
            },
        }
    }

    pub fn mnemonic(&self) -> &str {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.mnemonic,
            ActionPayload::Spawn(spec) => spec.mnemonic(),
            ActionPayload::Symlink(_) => "Symlink",
            ActionPayload::ArgsWrite(_) => "FileWrite",
            ActionPayload::RunfilesSupport(spec) => spec.mnemonic(),
        }
    }

    pub fn argv(&self) -> &[String] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.argv,
            _ => &[],
        }
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.env,
            _ => &EMPTY_STRING_MAP,
        }
    }

    pub fn execution_requirements(&self) -> &BTreeMap<String, String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.execution_requirements,
            _ => &EMPTY_STRING_MAP,
        }
    }

    pub fn inputs(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.inputs,
            _ => &[],
        }
    }

    pub fn tools(&self) -> &[ActionInput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.tools,
            _ => &[],
        }
    }

    pub fn outputs(&self) -> &[ActionOutput] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.outputs,
            ActionPayload::Spawn(spec) => spec.outputs(),
            ActionPayload::Symlink(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::ArgsWrite(spec) => std::slice::from_ref(spec.output()),
            ActionPayload::RunfilesSupport(spec) => std::slice::from_ref(spec.output()),
        }
    }

    pub fn param_files(&self) -> &[ParamFile] {
        match &self.payload {
            ActionPayload::Legacy(spec) => &spec.param_files,
            _ => &[],
        }
    }

    pub fn progress_message(&self) -> Option<&str> {
        match &self.payload {
            ActionPayload::Legacy(spec) => spec.progress_message.as_deref(),
            ActionPayload::Spawn(spec) => spec.progress_message(),
            ActionPayload::Symlink(spec) => spec.progress_message(),
            ActionPayload::ArgsWrite(_) => None,
            ActionPayload::RunfilesSupport(_) => None,
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
            _ => None,
        }
    }

    pub fn symlink_spec(&self) -> Option<&SymlinkSpec> {
        match &self.payload {
            ActionPayload::Symlink(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn args_write_spec(&self) -> Option<&ArgsWriteSpec> {
        match &self.payload {
            ActionPayload::ArgsWrite(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn runfiles_support_spec(&self) -> Option<&RunfilesSupportActionSpec> {
        match &self.payload {
            ActionPayload::RunfilesSupport(spec) => Some(spec),
            _ => None,
        }
    }

    pub fn is_typed_payload(&self) -> bool {
        !matches!(&self.payload, ActionPayload::Legacy(_))
    }

    pub fn render_argv(&self) -> Vec<String> {
        match &self.payload {
            ActionPayload::Legacy(spec) => spec.argv.clone(),
            ActionPayload::Spawn(spec) => spec.render_argv(),
            _ => Vec::new(),
        }
    }

    fn legacy_mut(&mut self) -> &mut LegacyActionSpec {
        match &mut self.payload {
            ActionPayload::Legacy(spec) => spec,
            _ => panic!("legacy action builders cannot mutate typed action payloads"),
        }
    }
}
