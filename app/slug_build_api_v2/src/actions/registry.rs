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

use crate::actions::spec::ActionOutput;
use crate::actions::spec::ActionSpec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActionError {
    InvalidOutputPath { path: String },
    ConflictingOutput { path: String },
    MissingOutputs { mnemonic: String },
    MissingArgv { mnemonic: String },
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            if self.output_owners.contains_key(output.path()) {
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

    pub fn actions(&self) -> &[ActionSpec] {
        &self.actions
    }

    pub fn output_owner(&self, output_path: &str) -> Option<usize> {
        self.output_owners.get(output_path).copied()
    }
}

fn validate_action(action: &ActionSpec) -> Result<(), ActionError> {
    if action.outputs().is_empty() {
        return Err(ActionError::MissingOutputs {
            mnemonic: action.mnemonic().to_owned(),
        });
    }
    if matches!(
        action.kind(),
        crate::actions::spec::ActionKind::Run | crate::actions::spec::ActionKind::RunShell { .. }
    ) && action.argv().is_empty()
    {
        return Err(ActionError::MissingArgv {
            mnemonic: action.mnemonic().to_owned(),
        });
    }
    Ok(())
}

pub fn validate_output(output: &ActionOutput) -> Result<(), ActionError> {
    let path = output.path();
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
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
