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

use super::args_expansion::ExpansionBudget;
use super::args_expansion::FormatSize;
use super::args_expansion::checked_add;
use super::args_expansion::strings_size;
use super::registry::validate_output;
use super::spec::RetainedCommandLineSegment;
use super::spec::SpawnSpec;
use super::spec::apply_validated_format_sized;
use crate::args_mapper::ArgsDirectoryContext;
use crate::args_mapper::ArgsExpansionError;
use crate::args_mapper::UnsupportedTreeArgsDirectoryContext;

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
    ArgsExpansion(ArgsExpansionError),
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
            Self::ArgsExpansion(error) => error.fmt(f),
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
        self.expand_forced_param_files_with_context(&UnsupportedTreeArgsDirectoryContext)
    }
    pub fn expand_forced_param_files_with_context(
        &self,
        context: &dyn ArgsDirectoryContext,
    ) -> Result<ExpandedSpawnCommandLine, SpawnCommandLineError> {
        let mut output_budget = ExpansionBudget::default();
        let mut temporary_budget = ExpansionBudget::default();
        let mut argv = self
            .invocation()
            .render_prefix_with_budget(&mut output_budget)?;
        // Words are prepared once, including each deferred callback. All file
        // content sizes are checked jointly before any file is serialized.
        let mut prepared_files = Vec::new();
        for segment in self.command_line().segments() {
            match segment {
                RetainedCommandLineSegment::LiteralRun(values) => {
                    let (words, bytes) = strings_size(values.iter().map(|value| value.as_str()))?;
                    output_budget.charge(words, bytes)?;
                    argv.extend(values.iter().map(ToString::to_string));
                }
                RetainedCommandLineSegment::ArgsSnapshot(args) => {
                    let Some(policy) = args.param_file() else {
                        argv.extend(
                            args.recipe()
                                .render_with_budget(context, &mut output_budget)?,
                        );
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
                    let index = prepared_files.len();
                    let index_digits = if index == 0 {
                        1
                    } else {
                        index.ilog10() as usize + 1
                    };
                    let path_size =
                        checked_add(primary.path().len(), checked_add(index_digits, 8)?)?;
                    let flag_size = FormatSize::new(policy.flag_format())?.size(path_size)?;
                    // The retained path and its argv spelling both belong to
                    // the complete expanded consumer output.
                    output_budget.charge(1, checked_add(path_size, flag_size)?)?;
                    let path = format!("{}-{index}.params", primary.path());
                    for output in self.outputs() {
                        if paths_conflict(&path, output.path()) {
                            return Err(SpawnCommandLineError::OutputConflict {
                                param_path: path,
                                output_path: output.path().to_owned(),
                            });
                        }
                    }
                    let (lines, positional) = args
                        .recipe()
                        .prepare_virtual_param_file(context, &mut temporary_budget)?;
                    let (words, bytes) = args.recipe().measure_param_lines(&lines)?;
                    output_budget.charge(words, bytes)?;
                    let (words, bytes) = strings_size(positional.iter().map(String::as_str))?;
                    output_budget.charge(words, bytes)?;
                    argv.push(apply_validated_format_sized(
                        policy.flag_format(),
                        &path,
                        flag_size,
                    ));
                    argv.extend(positional);
                    prepared_files.push((path, args.recipe(), lines));
                }
            }
        }
        let param_files = prepared_files
            .into_iter()
            .map(|(path, recipe, lines)| {
                let (_, bytes) = recipe.measure_param_lines(&lines)?;
                Ok(VirtualParamFile {
                    path,
                    bytes: recipe.serialize_param_lines(&lines, bytes).into_bytes(),
                })
            })
            .collect::<Result<Vec<_>, ArgsExpansionError>>()?;
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

impl From<ArgsExpansionError> for SpawnCommandLineError {
    fn from(error: ArgsExpansionError) -> Self {
        Self::ArgsExpansion(error)
    }
}
