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

use prost::Message;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ReapiCommandProjection;

use crate::digest::ReapiDigest;
use crate::proto;

pub fn is_file_write_action(action: &ActionSpec) -> bool {
    matches!(action.kind(), ActionKind::Write { .. })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiCommand {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub output_files: Vec<String>,
    pub output_directories: Vec<String>,
    pub platform_properties: BTreeMap<String, String>,
}

impl ReapiCommand {
    pub fn from_action(action: &ActionSpec) -> Result<Self, String> {
        let projection = ReapiCommandProjection::from_action(action).map_err(str::to_owned)?;
        Ok(Self {
            argv: projection.argv,
            env: projection.env,
            output_files: projection.output_files,
            output_directories: projection.output_directories,
            platform_properties: projection.platform_properties,
        })
    }
    pub(crate) fn file_write(
        output: &str,
        is_executable: bool,
        platform_properties: BTreeMap<String, String>,
    ) -> Self {
        let mode = if is_executable { "0555" } else { "0444" };
        Self {
            argv: vec![
                "sh".to_owned(),
                "-c".to_owned(),
                format!("cp -- \"$1\" \"$2\" && chmod {mode} -- \"$2\""),
                "slug-filewrite".to_owned(),
                "__slug_filewrite__/content".to_owned(),
                output.to_owned(),
            ],
            env: BTreeMap::new(),
            output_files: vec![output.to_owned()],
            output_directories: Vec::new(),
            platform_properties,
        }
    }

    /// Convert an action into a worker-executable command. The Stage 6 write
    /// actions are still declarative, so this is the first V2-owned lowering
    /// point rather than a direct-local shortcut.
    pub fn for_execution(action: &ActionSpec) -> Result<Self, String> {
        let mut command = Self::from_action(action)?;
        if !command.argv.is_empty() {
            return Ok(command);
        }

        let output = action
            .outputs()
            .first()
            .ok_or_else(|| format!("{} declares no output", action.mnemonic()))?;
        let parent = output
            .path()
            .rsplit_once('/')
            .map_or(".", |(parent, _)| parent);
        match action.kind() {
            ActionKind::Write { .. } => Err("raw FileWrite REAPI lowering is forbidden".to_owned()),
            ActionKind::WriteJson { content } => {
                command.argv = vec![
                    "sh".to_owned(),
                    "-c".to_owned(),
                    format!(
                        "mkdir -p -- {} && printf %s {} > {}",
                        shell_quote(parent),
                        shell_quote(content),
                        shell_quote(output.path())
                    ),
                ];
                Ok(command)
            }
            kind => Err(format!(
                "REAPI execution lowering is not implemented for {kind:?}"
            )),
        }
    }

    pub fn serialized(&self) -> Vec<u8> {
        self.to_proto().encode_to_vec()
    }

    pub fn digest(&self) -> ReapiDigest {
        ReapiDigest::of_bytes(&self.serialized())
    }

    fn to_proto(&self) -> proto::Command {
        proto::Command {
            arguments: self.argv.clone(),
            environment_variables: self
                .env
                .iter()
                .map(|(name, value)| proto::command::EnvironmentVariable {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect(),
            output_files: self.output_files.clone(),
            output_directories: self.output_directories.clone(),
            // The v2.2 Action field is authoritative, but the protocol asks
            // clients to also populate this retained Command field.
            platform: Some(platform_from_properties(&self.platform_properties)),
            output_paths: self
                .output_files
                .iter()
                .chain(&self.output_directories)
                .cloned()
                .collect(),
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReapiActionIdentity {
    pub action_digest: ReapiDigest,
    pub command_digest: ReapiDigest,
    pub input_root_digest: ReapiDigest,
    pub timeout_seconds: Option<u64>,
    action_bytes: Vec<u8>,
}

impl ReapiActionIdentity {
    pub fn new(
        command: &ReapiCommand,
        input_root_digest: ReapiDigest,
        timeout_seconds: Option<u64>,
    ) -> Self {
        let command_digest = command.digest();
        let action = proto::Action {
            command_digest: Some(digest_to_proto(&command_digest)),
            input_root_digest: Some(digest_to_proto(&input_root_digest)),
            timeout: timeout_seconds.map(|seconds| prost_types::Duration {
                seconds: seconds.try_into().expect("timeout fits in i64"),
                nanos: 0,
            }),
            do_not_cache: false,
            salt: Vec::new(),
            platform: Some(platform_from_properties(&command.platform_properties)),
        };
        let action_bytes = action.encode_to_vec();
        Self {
            action_digest: ReapiDigest::of_bytes(&action_bytes),
            command_digest,
            input_root_digest,
            timeout_seconds,
            action_bytes,
        }
    }

    pub fn action_bytes(&self) -> &[u8] {
        &self.action_bytes
    }
}

pub(crate) fn digest_to_proto(digest: &ReapiDigest) -> proto::Digest {
    proto::Digest {
        hash: digest.hash().to_owned(),
        size_bytes: digest
            .size_bytes()
            .try_into()
            .expect("REAPI digest size fits in i64"),
    }
}

fn platform_from_properties(properties: &BTreeMap<String, String>) -> proto::Platform {
    proto::Platform {
        properties: properties
            .iter()
            .map(|(name, value)| proto::platform::Property {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
    }
}
