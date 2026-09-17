/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::error::Error;
use std::fmt;

use super::registry::validate_output;
use super::spec::RetainedCommandLineSegment;
use super::spec::SpawnSpec;
use super::spec::apply_validated_format;

/// Action-local expansion: replacement argv and its virtual inputs are produced together.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExpandedSpawnCommandLine {
    argv: Vec<String>,
    param_files: Vec<VirtualParamFile>,
}

impl ExpandedSpawnCommandLine {
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    pub fn param_files(&self) -> &[VirtualParamFile] {
        &self.param_files
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VirtualParamFile {
    path: String,
    bytes: Vec<u8>,
}

impl VirtualParamFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SpawnCommandLineError {
    ConditionalParamFileUnsupported,
    MissingPrimaryOutput,
    InvalidPrimaryOutput {
        path: String,
    },
    OutputConflict {
        param_path: String,
        output_path: String,
    },
}

impl fmt::Display for SpawnCommandLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConditionalParamFileUnsupported => {
                f.write_str("conditional parameter-file expansion is not admitted")
            }
            Self::MissingPrimaryOutput => {
                f.write_str("parameter-file expansion requires a primary output")
            }
            Self::InvalidPrimaryOutput { path } => write!(
                f,
                "parameter-file primary output requires a normal relative path: {path}"
            ),
            Self::OutputConflict {
                param_path,
                output_path,
            } => write!(
                f,
                "virtual parameter file {param_path} conflicts with output {output_path}"
            ),
        }
    }
}

impl Error for SpawnCommandLineError {}

impl SpawnSpec {
    /// Expand forced parameter files using Bazel 9.2 CommandLines.expand ordering.
    /// Paths retain Slug-native output spelling. This does not admit execution.
    pub fn expand_forced_param_files(
        &self,
    ) -> Result<ExpandedSpawnCommandLine, SpawnCommandLineError> {
        let mut argv = self.invocation().render_prefix();
        let mut param_files = Vec::new();
        for segment in self.command_line().segments() {
            match segment {
                RetainedCommandLineSegment::LiteralRun(values) => {
                    argv.extend(values.iter().map(ToString::to_string));
                }
                RetainedCommandLineSegment::ArgsSnapshot(args) => {
                    let Some(policy) = args.param_file() else {
                        argv.extend(args.recipe().render());
                        continue;
                    };
                    if !policy.use_always() {
                        return Err(SpawnCommandLineError::ConditionalParamFileUnsupported);
                    }
                    let primary = self
                        .outputs()
                        .first()
                        .ok_or(SpawnCommandLineError::MissingPrimaryOutput)?;
                    if validate_output(primary).is_err() {
                        return Err(SpawnCommandLineError::InvalidPrimaryOutput {
                            path: primary.path().to_owned(),
                        });
                    }
                    let path = format!("{}-{}.params", primary.path(), param_files.len());
                    for output in self.outputs() {
                        if paths_conflict(&path, output.path()) {
                            return Err(SpawnCommandLineError::OutputConflict {
                                param_path: path,
                                output_path: output.path().to_owned(),
                            });
                        }
                    }
                    let (content, positional) = args.recipe().render_virtual_param_file();
                    argv.push(apply_validated_format(policy.flag_format(), &path));
                    argv.extend(positional);
                    param_files.push(VirtualParamFile {
                        path,
                        bytes: content.into_bytes(),
                    });
                }
            }
        }
        Ok(ExpandedSpawnCommandLine { argv, param_files })
    }
}

fn paths_conflict(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|tail| tail.starts_with('/'))
}
