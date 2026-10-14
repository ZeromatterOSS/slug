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

use crate::ArgsExpansionError;
use crate::actions::spec::ActionOutputKind;
use crate::actions::spec::ActionSpec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiCommandProjection {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub output_files: Vec<String>,
    pub output_directories: Vec<String>,
    pub platform_properties: BTreeMap<String, String>,
}

impl ReapiCommandProjection {
    pub fn from_action(action: &ActionSpec) -> Result<Self, ArgsExpansionError> {
        if action.kind() == &crate::ActionKind::BuildInfoTransform
            || action
                .outputs()
                .iter()
                .any(|output| output.metadata() == crate::ActionOutputMetadata::Constant)
        {
            return Err(ArgsExpansionError::Unsupported(
                "BuildInfoTransform/constant-metadata REAPI projection is not admitted".into(),
            ));
        }
        if action.kind() == &crate::ActionKind::CppCompile {
            return Err(ArgsExpansionError::Unsupported(
                "CppCompile REAPI projection is not admitted".into(),
            ));
        }
        if action.solib_symlink_spec().is_some() {
            return Err(ArgsExpansionError::Unsupported(
                "native SolibSymlink REAPI projection is not admitted".into(),
            ));
        }
        if action.is_typed_payload() {
            return Err(ArgsExpansionError::Unsupported(
                "typed Spawn/Symlink/ArgsWrite REAPI projection is not admitted".into(),
            ));
        }
        let mut output_files = Vec::new();
        let mut output_directories = Vec::new();
        for output in action.outputs() {
            match output.kind() {
                ActionOutputKind::File | ActionOutputKind::Symlink => {
                    output_files.push(output.path().to_owned())
                }
                ActionOutputKind::Directory => output_directories.push(output.path().to_owned()),
                ActionOutputKind::RunfilesTree => {
                    return Err(ArgsExpansionError::Unsupported(
                        "runfiles tree REAPI projection is not admitted".into(),
                    ));
                }
            }
        }

        Ok(Self {
            argv: action.argv().to_vec(),
            env: action.env().clone(),
            output_files,
            output_directories,
            platform_properties: action.exec_properties().clone(),
        })
    }
}
