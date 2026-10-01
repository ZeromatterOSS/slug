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
use std::ops::Range;

use crate::actions::registry::ActionError;
use crate::actions::registry::ActionRegistry;
use crate::actions::registry::validate_output;
use crate::actions::solib_symlink::SolibSymlinkSpec;
use crate::actions::spec::ActionInput;
use crate::actions::spec::ActionKind;
use crate::actions::spec::ActionOutput;
use crate::actions::spec::ActionOutputKind;
use crate::actions::spec::ActionSpec;
use crate::actions::spec::ArgsWriteSpec;
use crate::actions::spec::SpawnSpec;
use crate::actions::spec::SymlinkSpec;
use crate::actions::spec::SymlinkTarget;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CtxActions {
    root: crate::DerivedArtifactRoot,
    registry: ActionRegistry,
}

impl CtxActions {
    /// End-of-analysis publication checkpoint. No pending recipes enter this
    /// owner: validate the complete, ordered action batch before replacing state.
    pub fn replace_actions(&mut self, actions: Vec<ActionSpec>) -> Result<(), ActionError> {
        let mut registry = ActionRegistry::new();
        registry.register_batch(actions)?;
        self.registry = registry;
        Ok(())
    }

    pub fn new(root: crate::DerivedArtifactRoot) -> Self {
        Self {
            root,
            registry: ActionRegistry::new(),
        }
    }

    pub fn declare_file(&self, path: impl Into<String>) -> Result<ActionOutput, ActionError> {
        self.declare_output(path, ActionOutputKind::File)
    }

    pub fn declare_directory(&self, path: impl Into<String>) -> Result<ActionOutput, ActionError> {
        self.declare_output(path, ActionOutputKind::Directory)
    }

    pub fn declare_symlink(&self, path: impl Into<String>) -> Result<ActionOutput, ActionError> {
        self.declare_output(path, ActionOutputKind::Symlink)
    }

    pub fn write(
        &mut self,
        output: ActionOutput,
        content: impl Into<String>,
        is_executable: bool,
    ) -> Result<usize, ActionError> {
        self.registry.register(ActionSpec::new(
            ActionKind::Write {
                content: content.into(),
                is_executable,
            },
            "FileWrite",
            vec![output],
        ))
    }

    pub fn write_json(
        &mut self,
        output: ActionOutput,
        content: impl Into<String>,
    ) -> Result<usize, ActionError> {
        self.registry.register(ActionSpec::new(
            ActionKind::WriteJson {
                content: content.into(),
            },
            "JsonWrite",
            vec![output],
        ))
    }

    pub fn expand_template(
        &mut self,
        output: ActionOutput,
        template: ActionInput,
        substitutions: BTreeMap<String, String>,
    ) -> Result<usize, ActionError> {
        let action = ActionSpec::new(
            ActionKind::ExpandTemplate {
                template: template.clone(),
                substitutions,
            },
            "TemplateExpand",
            vec![output],
        )
        .with_inputs(vec![template]);
        self.registry.register(action)
    }

    pub fn run(
        &mut self,
        output: ActionOutput,
        executable: impl Into<String>,
        args: Vec<String>,
        inputs: Vec<ActionInput>,
        tools: Vec<ActionInput>,
    ) -> Result<usize, ActionError> {
        let executable = executable.into();
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push(executable);
        argv.extend(args);
        let action = ActionSpec::new(ActionKind::Run, "Spawn", vec![output])
            .with_argv(argv)
            .with_inputs(inputs)
            .with_tools(tools);
        self.registry.register(action)
    }

    pub fn run_shell(
        &mut self,
        output: ActionOutput,
        command: impl Into<String>,
        args: Vec<String>,
        inputs: Vec<ActionInput>,
    ) -> Result<usize, ActionError> {
        let command = command.into();
        // Match Bazel's ShellCommand pad behavior (StarlarkActionFactory.java:
        // "add an empty argument before other arguments"): when arguments are
        // present, an empty $0 is inserted so the first user argument is $1.
        let pad = !args.is_empty();
        let mut argv = vec!["sh".to_owned(), "-c".to_owned(), command.clone()];
        if pad {
            argv.push(String::new());
        }
        argv.extend(args);
        let action = ActionSpec::new(ActionKind::RunShell { command }, "Shell", vec![output])
            .with_argv(argv)
            .with_inputs(inputs);
        self.registry.register(action)
    }

    pub fn symlink(
        &mut self,
        output: ActionOutput,
        target_path: impl Into<String>,
    ) -> Result<usize, ActionError> {
        self.registry.register(ActionSpec::new(
            ActionKind::Symlink {
                target_path: target_path.into(),
            },
            "Symlink",
            vec![output],
        ))
    }

    pub fn register_spawn(&mut self, spec: SpawnSpec) -> Result<usize, ActionError> {
        self.registry.register(ActionSpec::spawn(spec))
    }
    pub fn register_build_info_transform(
        &mut self,
        spec: crate::BuildInfoTransformSpec,
    ) -> Result<usize, ActionError> {
        self.registry
            .register(ActionSpec::build_info_transform(spec))
    }

    pub fn register_args_write(&mut self, spec: ArgsWriteSpec) -> Result<usize, ActionError> {
        self.registry.register(ActionSpec::args_write(spec))
    }

    pub fn register_symlink(&mut self, spec: SymlinkSpec) -> Result<usize, ActionError> {
        if matches!(spec.target(), SymlinkTarget::Artifact { .. })
            && let Some(index) = self.registry.output_owner(spec.output().path())
        {
            let candidate = &self.registry.actions()[index];
            if candidate.exec_properties().is_empty()
                && candidate.exec_group().is_none()
                && let Some(existing) = candidate.symlink_spec()
                && existing.output() == spec.output()
                && existing.target() == spec.target()
            {
                // Bazel SymlinkAction keys exclude progress; retain the first action.
                return Ok(index);
            }
        }
        self.registry.register(ActionSpec::symlink(spec))
    }

    pub fn register_solib_symlink(&mut self, spec: SolibSymlinkSpec) -> Result<usize, ActionError> {
        let action = ActionSpec::solib_symlink(spec);
        if let Some(index) = self.registry.output_owner(action.outputs()[0].path())
            && self.registry.actions()[index] == action
        {
            return Ok(index);
        }
        self.registry.register(action)
    }

    pub fn register_batch(
        &mut self,
        actions: impl IntoIterator<Item = ActionSpec>,
    ) -> Result<Range<usize>, ActionError> {
        self.registry.register_batch(actions)
    }

    pub fn registry(&self) -> &ActionRegistry {
        &self.registry
    }

    pub fn into_registry(self) -> ActionRegistry {
        self.registry
    }

    fn declare_output(
        &self,
        path: impl Into<String>,
        kind: ActionOutputKind,
    ) -> Result<ActionOutput, ActionError> {
        let output = ActionOutput::new(self.root.clone(), path, kind);
        validate_output(&output)?;
        Ok(output)
    }
}
