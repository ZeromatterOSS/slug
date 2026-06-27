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

use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ReapiCommandProjection;

use crate::digest::ReapiDigest;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiCommand {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub output_files: Vec<String>,
    pub output_directories: Vec<String>,
    pub platform_properties: BTreeMap<String, String>,
}

impl ReapiCommand {
    pub fn from_action(action: &ActionSpec) -> Self {
        let projection = ReapiCommandProjection::from_action(action);
        Self {
            argv: projection.argv,
            env: projection.env,
            output_files: projection.output_files,
            output_directories: projection.output_directories,
            platform_properties: projection.platform_properties,
        }
    }

    pub fn digest(&self) -> ReapiDigest {
        ReapiDigest::of_bytes(self.stable_serialize().as_bytes())
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "argv={:?};env={:?};files={:?};dirs={:?};platform={:?}",
            self.argv,
            self.env,
            self.output_files,
            self.output_directories,
            self.platform_properties
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiActionIdentity {
    pub command_digest: ReapiDigest,
    pub input_root_digest: ReapiDigest,
    pub platform_digest: ReapiDigest,
    pub timeout_seconds: Option<u64>,
}

impl ReapiActionIdentity {
    pub fn new(
        command: &ReapiCommand,
        input_root_digest: ReapiDigest,
        timeout_seconds: Option<u64>,
    ) -> Self {
        let platform_digest =
            ReapiDigest::of_bytes(format!("{:?}", command.platform_properties).as_bytes());
        Self {
            command_digest: command.digest(),
            input_root_digest,
            platform_digest,
            timeout_seconds,
        }
    }

    pub fn stable_serialize(&self) -> String {
        format!(
            "command={};input_root={};platform={};timeout={:?}",
            self.command_digest, self.input_root_digest, self.platform_digest, self.timeout_seconds
        )
    }
}
