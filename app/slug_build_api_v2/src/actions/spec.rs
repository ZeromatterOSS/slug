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

use allocative::Allocative;
use dupe::Dupe;

use crate::analysis_value::AnalysisArtifact;
use crate::analysis_value::AnalysisDepset;
use crate::analysis_value::AnalysisValueKind;
use crate::analysis_value::AnalysisValueType;

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

    pub fn depset(&self) -> &AnalysisDepset {
        &self.0
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
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct ActionSpec {
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
    exec_properties: BTreeMap<String, String>,
    exec_group: Option<String>,
}

impl ActionSpec {
    pub fn new(kind: ActionKind, mnemonic: impl Into<String>, outputs: Vec<ActionOutput>) -> Self {
        Self {
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
            exec_properties: BTreeMap::new(),
            exec_group: None,
        }
    }

    pub fn kind(&self) -> &ActionKind {
        &self.kind
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub fn execution_requirements(&self) -> &BTreeMap<String, String> {
        &self.execution_requirements
    }

    pub fn inputs(&self) -> &[ActionInput] {
        &self.inputs
    }

    pub fn tools(&self) -> &[ActionInput] {
        &self.tools
    }

    pub fn outputs(&self) -> &[ActionOutput] {
        &self.outputs
    }

    pub fn param_files(&self) -> &[ParamFile] {
        &self.param_files
    }

    pub fn progress_message(&self) -> Option<&str> {
        self.progress_message.as_deref()
    }

    pub fn exec_properties(&self) -> &BTreeMap<String, String> {
        &self.exec_properties
    }

    pub fn exec_group(&self) -> Option<&str> {
        self.exec_group.as_deref()
    }

    pub fn with_argv(mut self, argv: Vec<String>) -> Self {
        self.argv = argv;
        self
    }

    pub fn with_env(mut self, env: BTreeMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn with_execution_requirements(
        mut self,
        execution_requirements: BTreeMap<String, String>,
    ) -> Self {
        self.execution_requirements = execution_requirements;
        self
    }

    pub fn with_inputs(mut self, inputs: Vec<ActionInput>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_tools(mut self, tools: Vec<ActionInput>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_param_files(mut self, param_files: Vec<ParamFile>) -> Self {
        self.param_files = param_files;
        self
    }

    pub fn with_progress_message(mut self, progress_message: impl Into<String>) -> Self {
        self.progress_message = Some(progress_message.into());
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
}
