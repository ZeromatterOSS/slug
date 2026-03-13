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
use kuro_error::BuckErrorContext;
use sorted_vector_map::SortedVectorMap;

/// Which shell interpreter to use for executing the genrule command.
#[derive(Debug, Clone, Allocative)]
pub enum GenruleShell {
    /// Run via `bash -c "cmd"` (default, works everywhere)
    Bash,
    /// Run via `powershell.exe -Command "cmd"` (Windows only, selected by `cmd_ps` attr)
    PowerShell,
    /// Run via `cmd.exe /c "cmd"` (Windows only, selected by `cmd_bat` attr)
    CmdExe,
}

/// A simple shell command action for native genrule targets.
///
/// This action runs a shell command to produce genrule output files.
/// Make variables like `$(SRCS)`, `$(OUTS)`, `$@`, `$<`, `$^` are expanded
/// at execution time using the actual artifact paths.
/// `$(location label)`, `$(locations label)`, `$(execpath label)`, and
/// `$(execpaths label)` are also expanded using dep artifact paths.
///
/// On Windows, `GenruleShell::PowerShell` or `GenruleShell::CmdExe` can be used
/// when the genrule specifies `cmd_ps` or `cmd_bat` respectively.
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
    /// The shell interpreter to use when executing the command.
    shell: GenruleShell,
}

impl GenruleAction {
    pub fn new(
        cmd: String,
        inputs: Vec<ArtifactGroup>,
        outputs: Vec<BuildArtifact>,
        location_mappings: Vec<(String, Vec<ArtifactGroup>)>,
        shell: GenruleShell,
    ) -> Self {
        Self {
            cmd,
            inputs,
            outputs,
            location_mappings,
            shell,
        }
    }
}

/// Shell-quote a path for bash compatibility.
///
/// On Windows, converts the path to MSYS2 POSIX format (`/c/path/to/file`) and
/// backslash-escapes any spaces or shell-special characters. Double-quoting is
/// intentionally avoided: when bash is spawned via CreateProcess (Rust `Command`
/// or Python subprocess), MSYS2 path translation does not apply to double-quoted
/// Windows paths containing spaces, causing "No such file or directory" even when
/// the directory exists. The `/drive/path` format with backslash-escaped spaces
/// works correctly in that context.
///
/// On Unix, backslash-escapes special characters.
fn shell_quote_path(path: &str) -> String {
    if path.is_empty() {
        return path.to_owned();
    }
    // On Windows, convert C:\... to /c/... (MSYS2 POSIX drive format).
    let normalized = if cfg!(windows) {
        let bytes = path.as_bytes();
        if bytes.len() >= 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
            let drive = (bytes[0] as char).to_ascii_lowercase();
            let rest = path[3..].replace('\\', "/");
            format!("/{}/{}", drive, rest)
        } else {
            path.replace('\\', "/")
        }
    } else {
        path.to_owned()
    };
    // Backslash-escape shell-special characters (including spaces).
    // Using backslash escapes rather than double-quoting sidesteps the MSYS2
    // path-translation issue described in the doc comment above.
    let needs_escape = normalized
        .contains(|c: char| matches!(c, ' ' | '\'' | '"' | '(' | ')' | '&' | ';' | '|' | '<' | '>'));
    if needs_escape {
        let mut result = String::with_capacity(normalized.len() + 8);
        for c in normalized.chars() {
            if matches!(c, ' ' | '\'' | '"' | '(' | ')' | '&' | ';' | '|' | '<' | '>') {
                result.push('\\');
            }
            result.push(c);
        }
        result
    } else {
        normalized
    }
}

/// Quote a path for PowerShell or CMD.exe (Windows native shells).
///
/// Unlike bash, PowerShell and CMD.exe can use forward-slash paths directly.
/// We wrap in double quotes only when the path contains spaces.
fn windows_quote_path(path: &str) -> String {
    if path.is_empty() {
        return path.to_owned();
    }
    if path.contains(' ') {
        format!("\"{}\"", path)
    } else {
        path.to_owned()
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
///
/// Paths are shell-quoted to handle spaces and special characters (important on
/// Windows where user home directories may contain spaces).
fn expand_genrule_cmd(
    cmd: &str,
    srcs: &[String],
    outs: &[String],
    locations: &[(String, Vec<String>)],
    quote_fn: fn(&str) -> String,
) -> String {
    let srcs_str = srcs.iter().map(|s| quote_fn(s)).collect::<Vec<_>>().join(" ");
    let outs_str = outs.iter().map(|s| quote_fn(s)).collect::<Vec<_>>().join(" ");
    let first_out_raw = outs.first().map(|s| s.as_str()).unwrap_or("");
    let first_src_raw = srcs.first().map(|s| s.as_str()).unwrap_or("");
    let first_out = quote_fn(first_out_raw);
    let first_src = quote_fn(first_src_raw);

    // Compute output directory (dirname of first output) from the raw path,
    // then apply shell quoting to the result.
    let out_dir_raw = std::path::Path::new(first_out_raw)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    let out_dir = quote_fn(out_dir_raw);

    // Expand $(VARNAME) style make variables
    let mut cmd = cmd
        .replace("$(SRCS)", &srcs_str)
        .replace("$(OUTS)", &outs_str)
        .replace("$(@D)", &out_dir)
        .replace("$(RULEDIR)", &out_dir)
        .replace("$(GENDIR)", &out_dir)
        .replace("$(BINDIR)", &out_dir)
        .replace("$(TARGET)", ""); // Not easily available at exec time, skip

    // Expand single-char $ substitutions
    if outs.len() == 1 {
        cmd = cmd.replace("$@", &first_out);
    } else {
        cmd = cmd.replace("$@", &out_dir);
    }
    cmd = cmd.replace("$<", &first_src);
    cmd = cmd.replace("$^", &srcs_str);

    // Expand $(location ...) and related patterns.
    // We do a single-pass scan to handle all variants consistently.
    cmd = expand_location_patterns(&cmd, locations, quote_fn);

    cmd
}

/// Expand $(location label), $(locations label), $(execpath label), $(execpaths label),
/// $(rootpath label), $(rootpaths label), $(rlocationpath label), $(rlocationpaths label).
///
/// For each `$(location key)` pattern, looks up `key` in the `locations` mapping
/// and replaces with the first resolved path.
/// For `$(locations key)`, replaces with space-separated list of all paths.
///
/// In Bazel, `execpath` returns exec-root-relative paths, `rootpath` returns
/// runfiles-tree-relative paths, and `rlocationpath` returns rlocation paths.
/// In Kuro, all three resolve to the same artifact path.
fn expand_location_patterns(
    cmd: &str,
    locations: &[(String, Vec<String>)],
    quote_fn: fn(&str) -> String,
) -> String {
    if locations.is_empty()
        || !cmd.contains("$(location")
            && !cmd.contains("$(execpath")
            && !cmd.contains("$(rootpath")
            && !cmd.contains("$(rlocationpath")
    {
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
        } else if after_paren.starts_with("rootpaths ") {
            ("rootpaths ", true)
        } else if after_paren.starts_with("rootpath ") {
            ("rootpath ", false)
        } else if after_paren.starts_with("rlocationpaths ") {
            ("rlocationpaths ", true)
        } else if after_paren.starts_with("rlocationpath ") {
            ("rlocationpath ", false)
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
            let expansion = resolve_location(label, multi, locations, quote_fn);
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
fn resolve_location(
    label: &str,
    multi: bool,
    locations: &[(String, Vec<String>)],
    quote_fn: fn(&str) -> String,
) -> String {
    // Normalize: strip leading "@" repo prefix for simple matching,
    // or use suffix after ':'
    let label_name = label.rsplit(':').next().unwrap_or(label);

    for (key, paths) in locations {
        // Exact match
        if key == label {
            return if multi {
                paths.iter().map(|p| quote_fn(p)).collect::<Vec<_>>().join(" ")
            } else {
                paths.first().map(|p| quote_fn(p)).unwrap_or_default()
            };
        }
        // Name-only match (key name suffix == label name suffix)
        let key_name = key.rsplit(':').next().unwrap_or(key.as_str());
        if key_name == label_name {
            return if multi {
                paths.iter().map(|p| quote_fn(p)).collect::<Vec<_>>().join(" ")
            } else {
                paths.first().map(|p| quote_fn(p)).unwrap_or_default()
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
        // Resolve input artifact paths for make-variable expansion.
        // We use project-relative paths (forward-slash, never containing spaces) so that
        // the generated bash command works on Windows without MSYS2 path-translation issues.
        // Bash runs with CWD = project root (via `env --chdir`), so relative paths resolve
        // correctly to the same files.
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
                srcs_abs_paths.push(proj_rel.to_string());
            }
        }

        // Resolve output artifact paths for make-variable expansion and pre-create output dirs.
        // Genrule commands (like `echo foo > $@`) require the parent directory to exist.
        // Use project-relative paths for the bash command (no spaces, no Windows drive issues).
        let mut outs_abs_paths: Vec<String> = Vec::new();
        for ba in &self.outputs {
            let proj_rel = ctx.fs().resolve_build(
                ba.get_path(),
                Some(&ContentBasedPathHash::for_output_artifact()),
            )?;
            // Ensure the parent directory exists so the command can write to the output file.
            // Genrule semantics (matching Bazel) require output dirs to be pre-created.
            if let Some(parent) = proj_rel.parent() {
                let abs_parent = ctx.fs().fs().resolve(parent);
                std::fs::create_dir_all(abs_parent.as_path())
                    .buck_error_context("Failed to create genrule output directory")?;
            }
            outs_abs_paths.push(proj_rel.to_string());
        }

        // Resolve location mapping artifact paths for $(location ...) expansion.
        // Again use project-relative paths.
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
                    paths.push(proj_rel.to_string());
                }
            }
            location_resolved.push((label_key.clone(), paths));
        }

        // Select path-quoting function based on shell type.
        // Bash requires POSIX path conversion on Windows; PowerShell and CMD.exe use native paths.
        let quote_fn: fn(&str) -> String = match &self.shell {
            GenruleShell::Bash => shell_quote_path,
            GenruleShell::PowerShell | GenruleShell::CmdExe => windows_quote_path,
        };

        // Expand make variables in the command
        let expanded_cmd = expand_genrule_cmd(
            &self.cmd,
            &srcs_abs_paths,
            &outs_abs_paths,
            &location_resolved,
            quote_fn,
        );

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

        // Build the execution request using the appropriate shell interpreter.
        let req = match &self.shell {
            GenruleShell::Bash => CommandExecutionRequest::new(
                vec![kuro_execute::shell::find_bash().to_owned()],
                vec!["-c".to_owned(), expanded_cmd],
                paths,
                SortedVectorMap::default(),
            ),
            GenruleShell::PowerShell => CommandExecutionRequest::new(
                vec!["powershell.exe".to_owned()],
                vec!["-Command".to_owned(), expanded_cmd],
                paths,
                SortedVectorMap::default(),
            ),
            GenruleShell::CmdExe => CommandExecutionRequest::new(
                vec!["cmd.exe".to_owned()],
                vec!["/c".to_owned(), expanded_cmd],
                paths,
                SortedVectorMap::default(),
            ),
        };

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
