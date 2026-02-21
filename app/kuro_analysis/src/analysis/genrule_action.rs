/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Native genrule action implementation.
//!
//! This module provides a simple shell command action for native genrule targets.
//! It is used by `analyze_genrule()` in `native_rule_analysis.rs` to register
//! a real action that executes the genrule's shell command.

use std::borrow::Cow;

use allocative::Allocative;
use async_trait::async_trait;
use dupe::Dupe;
use indexmap::IndexSet;
use kuro_artifact::artifact::build_artifact::BuildArtifact;
use kuro_build_api::actions::Action;
use kuro_build_api::actions::ActionExecutionCtx;
use kuro_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use kuro_build_api::actions::execute::action_executor::ActionOutputs;
use kuro_build_api::actions::execute::error::ExecuteError;
use kuro_build_api::artifact_groups::ArtifactGroup;
use kuro_build_signals::env::WaitingData;
use kuro_core::category::CategoryRef;
use kuro_core::content_hash::ContentBasedPathHash;
use kuro_execute::artifact::artifact_dyn::ArtifactDyn;
use kuro_execute::execute::request::CommandExecutionInput;
use kuro_execute::execute::request::CommandExecutionOutput;
use kuro_execute::execute::request::CommandExecutionPaths;
use kuro_execute::execute::request::CommandExecutionRequest;
use kuro_execute::execute::request::ExecutorPreference;
use sorted_vector_map::SortedVectorMap;

/// A simple shell command action for native genrule targets.
///
/// This action runs a bash command to produce genrule output files.
/// Make variables like `$(SRCS)`, `$(OUTS)`, `$@`, `$<`, `$^` are expanded
/// at execution time using the actual artifact paths.
/// `$(location label)`, `$(locations label)`, `$(execpath label)`, and
/// `$(execpaths label)` are also expanded using dep artifact paths.
#[derive(Debug, Allocative)]
pub struct GenruleAction {
    /// The shell command template with unexpanded make variables.
    cmd: String,
    /// Input artifact groups (from srcs and tools deps).
    inputs: Vec<ArtifactGroup>,
    /// Output build artifacts (one per entry in genrule's `outs`).
    outputs: Vec<BuildArtifact>,
    /// Location mappings: (normalized_label, artifacts) for $(location label) expansion.
    /// The normalized_label is a string key used to match $(location <key>) patterns.
    /// Each entry stores the artifact groups for the matching dependency.
    location_mappings: Vec<(String, Vec<ArtifactGroup>)>,
}

impl GenruleAction {
    pub fn new(
        cmd: String,
        inputs: Vec<ArtifactGroup>,
        outputs: Vec<BuildArtifact>,
        location_mappings: Vec<(String, Vec<ArtifactGroup>)>,
    ) -> Self {
        Self {
            cmd,
            inputs,
            outputs,
            location_mappings,
        }
    }
}

/// Expand genrule make variables using resolved absolute paths.
///
/// Handles:
/// - `$(SRCS)` / `$(OUTS)` / `$@` / `$<` / `$^` / `$(@D)` - standard Make variables
/// - `$(location label)` - first output path of label
/// - `$(locations label)` - space-separated output paths of label
/// - `$(execpath label)` - alias for $(location label)
/// - `$(execpaths label)` - alias for $(locations label)
fn expand_genrule_cmd(
    cmd: &str,
    srcs: &[String],
    outs: &[String],
    locations: &[(String, Vec<String>)],
) -> String {
    let srcs_str = srcs.join(" ");
    let outs_str = outs.join(" ");
    let first_out = outs.first().map(|s| s.as_str()).unwrap_or("");
    let first_src = srcs.first().map(|s| s.as_str()).unwrap_or("");

    // Compute output directory (dirname of first output)
    let out_dir = std::path::Path::new(first_out)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    // Expand $(VARNAME) style make variables
    let mut cmd = cmd
        .replace("$(SRCS)", &srcs_str)
        .replace("$(OUTS)", &outs_str)
        .replace("$(@D)", out_dir)
        .replace("$(RULEDIR)", out_dir)
        .replace("$(GENDIR)", out_dir)
        .replace("$(BINDIR)", out_dir)
        .replace("$(TARGET)", ""); // Not easily available at exec time, skip

    // Expand single-char $ substitutions
    if outs.len() == 1 {
        cmd = cmd.replace("$@", first_out);
    } else {
        cmd = cmd.replace("$@", out_dir);
    }
    cmd = cmd.replace("$<", first_src);
    cmd = cmd.replace("$^", &srcs_str);

    // Expand $(location ...) and related patterns.
    // We do a single-pass scan to handle all variants consistently.
    cmd = expand_location_patterns(&cmd, locations);

    cmd
}

/// Expand $(location label), $(locations label), $(execpath label), $(execpaths label).
///
/// For each `$(location key)` pattern, looks up `key` in the `locations` mapping
/// and replaces with the first resolved path.
/// For `$(locations key)`, replaces with space-separated list of all paths.
fn expand_location_patterns(cmd: &str, locations: &[(String, Vec<String>)]) -> String {
    if locations.is_empty() || !cmd.contains("$(location") && !cmd.contains("$(execpath") {
        return cmd.to_owned();
    }

    let mut result = String::with_capacity(cmd.len());
    let mut remaining = cmd;

    while let Some(start) = remaining.find("$(") {
        result.push_str(&remaining[..start]);
        let after_paren = &remaining[start + 2..]; // after "$("

        // Try each keyword variant
        let (keyword, multi) = if after_paren.starts_with("locations ") {
            ("locations ", true)
        } else if after_paren.starts_with("location ") {
            ("location ", false)
        } else if after_paren.starts_with("execpaths ") {
            ("execpaths ", true)
        } else if after_paren.starts_with("execpath ") {
            ("execpath ", false)
        } else {
            // Not a location pattern - keep the "$(" and continue
            result.push_str("$(");
            remaining = &remaining[start + 2..];
            continue;
        };

        let label_start = keyword.len();
        let label_rest = &after_paren[label_start..];

        if let Some(end) = label_rest.find(')') {
            let label = label_rest[..end].trim();
            let expansion = resolve_location(label, multi, locations);
            result.push_str(&expansion);
            // Advance past "$(" + keyword + label + ")"
            remaining = &remaining[start + 2 + label_start + end + 1..];
        } else {
            // Malformed pattern — keep as-is and skip past "$("
            result.push_str("$(");
            remaining = &remaining[start + 2..];
        }
    }

    result.push_str(remaining);
    result
}

/// Resolve a label to its file path(s) from the location mappings.
///
/// Matches the label against each key in the mappings using:
/// 1. Exact match
/// 2. Target-name match (after the last ':')
fn resolve_location(label: &str, multi: bool, locations: &[(String, Vec<String>)]) -> String {
    // Normalize: strip leading "@" repo prefix for simple matching,
    // or use suffix after ':'
    let label_name = label.rsplit(':').next().unwrap_or(label);

    for (key, paths) in locations {
        // Exact match
        if key == label {
            return if multi {
                paths.join(" ")
            } else {
                paths.first().cloned().unwrap_or_default()
            };
        }
        // Name-only match (key name suffix == label name suffix)
        let key_name = key.rsplit(':').next().unwrap_or(key.as_str());
        if key_name == label_name {
            return if multi {
                paths.join(" ")
            } else {
                paths.first().cloned().unwrap_or_default()
            };
        }
    }

    // No match found — leave unexpanded (emit a warning via the original text)
    // This is better than silently producing wrong output.
    if multi {
        format!("$(locations {})", label)
    } else {
        format!("$(location {})", label)
    }
}

#[async_trait]
impl Action for GenruleAction {
    fn kind(&self) -> kuro_data::ActionKind {
        kuro_data::ActionKind::Run
    }

    fn inputs(&self) -> kuro_error::Result<Cow<'_, [ArtifactGroup]>> {
        // Include both regular inputs and location mapping artifacts.
        // If there are no location mappings, return a borrowed slice for efficiency.
        if self.location_mappings.is_empty() {
            return Ok(Cow::Borrowed(&self.inputs));
        }
        let mut all = self.inputs.clone();
        for (_, ags) in &self.location_mappings {
            all.extend_from_slice(ags);
        }
        Ok(Cow::Owned(all))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(&self.outputs)
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.outputs[0]
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("genrule")
    }

    fn identifier(&self) -> Option<&str> {
        None
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        // Resolve input artifact paths for make-variable expansion
        let mut srcs_abs_paths: Vec<String> = Vec::new();
        for ag in &self.inputs {
            let values = ctx.artifact_values(ag);
            for (artifact, value) in values.iter() {
                let content_hash: Option<ContentBasedPathHash> =
                    if artifact.has_content_based_path() {
                        Some(value.content_based_path_hash())
                    } else {
                        None
                    };
                let proj_rel = artifact.resolve_path(ctx.fs(), content_hash.as_ref())?;
                let abs = ctx.fs().fs().resolve(&proj_rel);
                srcs_abs_paths.push(abs.to_string());
            }
        }

        // Resolve output artifact paths for make-variable expansion
        let mut outs_abs_paths: Vec<String> = Vec::new();
        for ba in &self.outputs {
            let proj_rel = ctx.fs().resolve_build(
                ba.get_path(),
                Some(&ContentBasedPathHash::for_output_artifact()),
            )?;
            let abs = ctx.fs().fs().resolve(&proj_rel);
            outs_abs_paths.push(abs.to_string());
        }

        // Resolve location mapping artifact paths for $(location ...) expansion
        let mut location_resolved: Vec<(String, Vec<String>)> =
            Vec::with_capacity(self.location_mappings.len());
        for (label_key, ags) in &self.location_mappings {
            let mut paths: Vec<String> = Vec::new();
            for ag in ags {
                let values = ctx.artifact_values(ag);
                for (artifact, value) in values.iter() {
                    let content_hash: Option<ContentBasedPathHash> =
                        if artifact.has_content_based_path() {
                            Some(value.content_based_path_hash())
                        } else {
                            None
                        };
                    let proj_rel = artifact.resolve_path(ctx.fs(), content_hash.as_ref())?;
                    let abs = ctx.fs().fs().resolve(&proj_rel);
                    paths.push(abs.to_string());
                }
            }
            location_resolved.push((label_key.clone(), paths));
        }

        // Expand make variables in the command
        let expanded_cmd =
            expand_genrule_cmd(&self.cmd, &srcs_abs_paths, &outs_abs_paths, &location_resolved);

        // Build CommandExecutionPaths for the action executor
        let ce_inputs: Vec<CommandExecutionInput> = self
            .inputs
            .iter()
            .map(|ag| CommandExecutionInput::Artifact(Box::new(ctx.artifact_values(ag).dupe())))
            .collect();

        let ce_outputs: IndexSet<CommandExecutionOutput> = self
            .outputs
            .iter()
            .map(|ba| CommandExecutionOutput::BuildArtifact {
                path: ba.get_path().dupe(),
                output_type: ba.output_type(),
                supports_incremental_remote: false,
            })
            .collect();

        let paths =
            CommandExecutionPaths::new(ce_inputs, ce_outputs, ctx.fs(), ctx.digest_config(), None)?;

        // Run bash -c "expanded_cmd"
        let req = CommandExecutionRequest::new(
            vec!["bash".to_owned()],
            vec!["-c".to_owned(), expanded_cmd],
            paths,
            SortedVectorMap::default(),
        );

        let prepared = ctx.prepare_action(&req, false)?;
        let manager = ctx.command_execution_manager(waiting_data);
        let result = ctx.exec_cmd(manager, &req, &prepared).await;

        ctx.unpack_command_execution_result(
            ExecutorPreference::Default,
            result,
            false, // allow_cache_upload
            false, // allow_dep_file_cache_upload
            None,  // input_files_bytes
            kuro_data::IncrementalKind::NonIncremental,
        )
    }
}
