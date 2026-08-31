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
    pub fn from_action(action: &ActionSpec) -> Result<Self, &'static str> {
        if action.is_typed_payload() {
            return Err("typed Spawn/Symlink/ArgsWrite REAPI projection is not admitted");
        }
        let mut output_files = Vec::new();
        let mut output_directories = Vec::new();
        for output in action.outputs() {
            match output.kind() {
                ActionOutputKind::File | ActionOutputKind::Symlink => {
                    output_files.push(output.path().to_owned())
                }
                ActionOutputKind::Directory => output_directories.push(output.path().to_owned()),
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
