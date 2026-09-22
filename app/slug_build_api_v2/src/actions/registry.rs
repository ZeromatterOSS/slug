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
use std::error::Error;
use std::fmt;
use std::ops::Range;

use crate::actions::spec::ActionOutput;
use crate::actions::spec::ActionSpec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActionError {
    InvalidActionPayload { mnemonic: String },
    InvalidOutputPath { path: String },
    ConflictingOutput { path: String },
    MissingOutputs { mnemonic: String },
    MissingArgv { mnemonic: String },
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidActionPayload { mnemonic } => {
                write!(f, "action {mnemonic} requires its native typed payload")
            }
            Self::InvalidOutputPath { path } => write!(f, "invalid action output path: {path}"),
            Self::ConflictingOutput { path } => {
                write!(f, "action output declared more than once: {path}")
            }
            Self::MissingOutputs { mnemonic } => {
                write!(f, "action {mnemonic} must declare at least one output")
            }
            Self::MissingArgv { mnemonic } => {
                write!(f, "action {mnemonic} must declare a non-empty argv")
            }
        }
    }
}

impl Error for ActionError {}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ActionRegistry {
    actions: Vec<ActionSpec>,
    // Candidate index and projection collision guard, never structural identity.
    output_owners: BTreeMap<String, usize>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, action: ActionSpec) -> Result<usize, ActionError> {
        validate_action(&action)?;
        for output in action.outputs() {
            validate_output(output)?;
            if self.structural_output_owner(output).is_some()
                || self.output_owners.contains_key(output.path())
            {
                return Err(ActionError::ConflictingOutput {
                    path: output.path().to_owned(),
                });
            }
        }

        let index = self.actions.len();
        for output in action.outputs() {
            self.output_owners.insert(output.path().to_owned(), index);
        }
        self.actions.push(action);
        Ok(index)
    }

    pub fn register_batch(
        &mut self,
        actions: impl IntoIterator<Item = ActionSpec>,
    ) -> Result<Range<usize>, ActionError> {
        let actions = actions.into_iter().collect::<Vec<_>>();
        let mut pending = BTreeMap::new();
        for action in &actions {
            validate_action(action)?;
            for output in action.outputs() {
                validate_output(output)?;
                if self.structural_output_owner(output).is_some()
                    || self.output_owners.contains_key(output.path())
                    || pending.insert(output.path().to_owned(), ()).is_some()
                {
                    return Err(ActionError::ConflictingOutput {
                        path: output.path().to_owned(),
                    });
                }
            }
        }

        let start = self.actions.len();
        for (offset, action) in actions.into_iter().enumerate() {
            let index = start + offset;
            for output in action.outputs() {
                self.output_owners.insert(output.path().to_owned(), index);
            }
            self.actions.push(action);
        }
        Ok(start..self.actions.len())
    }

    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    pub fn output_owner(&self, output_path: &str) -> Option<usize> {
        self.output_owners.get(output_path).copied()
    }

    pub fn structural_output_owner(&self, output: &ActionOutput) -> Option<usize> {
        // Equal structural coordinates have the same deterministic projection.
        // Verify the candidate structurally: a projected-path collision must not
        // silently identify a different root, and kind does not distinguish owners.
        let candidate = self.output_owner(output.path())?;
        self.actions[candidate]
            .outputs()
            .iter()
            .any(|declared| {
                declared.root() == output.root()
                    && declared.root_relative_path() == output.root_relative_path()
            })
            .then_some(candidate)
    }
}

fn validate_action(action: &ActionSpec) -> Result<(), ActionError> {
    if action.kind() == &crate::ActionKind::CppCompile && action.cpp_compile_spec().is_none() {
        return Err(ActionError::InvalidActionPayload {
            mnemonic: action.mnemonic().to_owned(),
        });
    }
    if action.outputs().is_empty() {
        return Err(ActionError::MissingOutputs {
            mnemonic: action.mnemonic().to_owned(),
        });
    }
    if matches!(
        action.kind(),
        crate::actions::spec::ActionKind::Run
            | crate::actions::spec::ActionKind::RunShell { .. }
            | crate::actions::spec::ActionKind::Spawn
    ) && action.argv().is_empty()
        && action
            .render_argv()
            .expect("Run/RunShell/Spawn command projection is admitted")
            .is_empty()
    {
        return Err(ActionError::MissingArgv {
            mnemonic: action.mnemonic().to_owned(),
        });
    }
    Ok(())
}

pub fn validate_output(output: &ActionOutput) -> Result<(), ActionError> {
    validate_output_path(output.root_relative_path())
}

pub(crate) fn validate_output_path(path: &str) -> Result<(), ActionError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ActionError::InvalidOutputPath {
            path: path.to_owned(),
        });
    }
    Ok(())
}
