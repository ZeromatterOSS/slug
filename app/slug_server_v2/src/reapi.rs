/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

//! Legacy Run execution shared by the daemon and one-shot CLI. It
//! embeds `runtime_mode = "daemon"` and the invalidated-file count.

use std::path::Path;

use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::BuildCommandEvaluation;
use slug_core_v2::runtime::ConfiguredNodeKind;
use slug_reapi_v2::RemoteConfig;

/// Execute the existing Run action closure through REAPI and return the
/// JSON evidence line plus the exit code.
fn run_reapi_run_build(
    workspace: &Path,
    evaluation: &BuildCommandEvaluation,
    analyzed_target_count: usize,
    declared_action_count: usize,
    remote: &RemoteConfig,
    runtime_mode: &str,
    invalidated_files: usize,
) -> ReapiBuildOutcome {
    if evaluation.requested_target_kinds().any(|kind| {
        matches!(
            kind,
            ConfiguredNodeKind::Alias | ConfiguredNodeKind::GeneratedFile
        )
    }) {
        return ReapiBuildOutcome::error(
            runtime_mode,
            invalidated_files,
            "REAPI execution for requested aliases and generated files is unsupported",
        );
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return ReapiBuildOutcome::error(runtime_mode, invalidated_files, &error.to_string());
        }
    };
    let execution = runtime.block_on(async {
        let mut reapi_actions = 0_u64;
        let mut direct_local_actions = 0_u64;
        let mut ac_hits = 0_u64;
        let mut ac_misses = 0_u64;
        let mut action_digests: Vec<String> = Vec::new();
        let mut uploaded_digests: Vec<String> = Vec::new();
        let mut materialized_outputs: Vec<String> = Vec::new();
        let mut platform_properties = std::collections::BTreeMap::new();
        let mut record = |output_root: &std::path::Path,
                          result: slug_reapi_v2::RemoteExecutionResult|
         -> Result<(), String> {
            slug_reapi_v2::materialize_outputs(output_root, &result)
                .map_err(|error| error.to_string())?;
            reapi_actions += result.evidence.reapi_actions;
            direct_local_actions += result.evidence.direct_local_actions;
            ac_hits += result.evidence.ac_hits;
            ac_misses += result.evidence.ac_misses;
            action_digests.push(result.action_digest.to_string());
            uploaded_digests.extend(
                result
                    .evidence
                    .uploaded_digests
                    .iter()
                    .map(|digest| digest.to_string()),
            );
            materialized_outputs.extend(
                result
                    .evidence
                    .materialized_outputs
                    .iter()
                    .map(|digest| digest.to_string()),
            );
            platform_properties.extend(result.platform_properties);
            Ok(())
        };

        let mut file_write_actions = 0_usize;
        let mut other_actions = 0_usize;
        for action in evaluation
            .analyses()
            .flat_map(|analysis| analysis.actions())
        {
            if slug_reapi_v2::is_file_write_action(action) {
                file_write_actions += 1;
            } else {
                other_actions += 1;
            }
        }
        if file_write_actions > 0 && other_actions > 0 {
            return Err(
                "mixed FileWrite and non-FileWrite REAPI closures are unsupported".to_owned(),
            );
        }

        if file_write_actions > 0 {
            let views = evaluation
                .resolved_file_write_execution_views_in_closure()
                .map_err(str::to_owned)?;
            for view in views {
                let configuration = view
                    .action()
                    .owner()
                    .configuration()
                    .slug_configuration()
                    .ok_or_else(|| {
                        "production FileWrite owner has an opaque configuration".to_owned()
                    })?;
                let output_root =
                    slug_core_v2::runtime::configured_output_root(workspace, configuration);
                let result = slug_reapi_v2::execute_file_write(remote, &view)
                    .await
                    .map_err(|error| error.to_string())?;
                record(&output_root, result)?;
            }
        } else {
            for analysis in evaluation.analyses() {
                let configuration = analysis
                    .configured_target_key()
                    .ok_or_else(|| "production rule analysis returned a null node".to_owned())?
                    .configuration()
                    .slug_configuration()
                    .ok_or_else(|| {
                        "production analysis returned an opaque configuration".to_owned()
                    })?;
                let output_root =
                    slug_core_v2::runtime::configured_output_root(workspace, configuration);
                for action in analysis.actions() {
                    let result = slug_reapi_v2::execute_action(remote, action)
                        .await
                        .map_err(|error| error.to_string())?;
                    record(&output_root, result)?;
                }
            }
        }
        Ok::<_, String>((
            reapi_actions,
            direct_local_actions,
            ac_hits,
            ac_misses,
            action_digests,
            uploaded_digests,
            materialized_outputs,
            platform_properties,
        ))
    });
    match execution {
        Ok((
            reapi_actions,
            direct_local_actions,
            ac_hits,
            ac_misses,
            action_digests,
            uploaded_digests,
            materialized_outputs,
            platform_properties,
        )) if reapi_actions > 0 => {
            let action_digests_json = action_digests
                .iter()
                .map(|digest| format!("\"{}\"", json_escape(digest)))
                .collect::<Vec<_>>()
                .join(",");
            let uploaded_digests_json = uploaded_digests
                .iter()
                .map(|digest| format!("\"{}\"", json_escape(digest)))
                .collect::<Vec<_>>()
                .join(",");
            let materialized_outputs_json = materialized_outputs
                .iter()
                .map(|digest| format!("\"{}\"", json_escape(digest)))
                .collect::<Vec<_>>()
                .join(",");
            let platform_properties_json = platform_properties
                .iter()
                .map(|(key, value)| format!("\"{}\":\"{}\"", json_escape(key), json_escape(value)))
                .collect::<Vec<_>>()
                .join(",");
            let stderr = format!(
                "{{\"success\":true,\"command\":\"build\",\"analyzed_target_count\":{},\"declared_action_count\":{},\"reapi_actions\":{},\"direct_local_actions\":{},\"ac_hits\":{},\"ac_misses\":{},\"action_digests\":[{}],\"uploaded_digests\":[{}],\"materialized_outputs\":[{}],\"platform_properties\":{{{}}},\"runtime_mode\":\"{}\",\"invalidated_files\":{},\"completed_boundary\":\"reapi_native_execution\"}}\n",
                analyzed_target_count,
                declared_action_count,
                reapi_actions,
                direct_local_actions,
                ac_hits,
                ac_misses,
                action_digests_json,
                uploaded_digests_json,
                materialized_outputs_json,
                platform_properties_json,
                runtime_mode,
                invalidated_files,
            );
            ReapiBuildOutcome {
                exit_code: 0,
                stderr,
            }
        }
        Ok(_) => ReapiBuildOutcome {
            exit_code: 2,
            stderr: format!(
                "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"message\":\"no executable actions were declared\",\"runtime_mode\":\"{}\",\"invalidated_files\":{}}}\n",
                runtime_mode, invalidated_files,
            ),
        },
        Err(error) => ReapiBuildOutcome::error(runtime_mode, invalidated_files, &error),
    }
}

pub fn run_reapi_executable(
    workspace: &Path,
    evaluation: &BuildCommandEvaluation,
    remote: &RemoteConfig,
    runtime_mode: &str,
    invalidated_files: usize,
) -> (ReapiBuildOutcome, Option<std::path::PathBuf>) {
    let view = match evaluation.resolved_run_semantic_view() {
        Ok(view) => view,
        Err(error) => {
            return (
                ReapiBuildOutcome::error(runtime_mode, invalidated_files, error),
                None,
            );
        }
    };
    let outcome = run_reapi_run_build(
        workspace,
        evaluation,
        evaluation.analyzed_target_count(),
        evaluation.declared_action_count(),
        remote,
        runtime_mode,
        invalidated_files,
    );
    if outcome.exit_code != 0 {
        return (outcome, None);
    }
    match slug_reapi_v2::verify_materialized_run_executable(workspace, &view) {
        Ok(path) => (outcome, Some(path)),
        Err(error) => (
            ReapiBuildOutcome::error(runtime_mode, invalidated_files, &error.to_string()),
            None,
        ),
    }
}
/// The outcome of a REAPI build: exit code and the stderr JSON line.
#[derive(Debug, Clone)]
pub struct ReapiBuildOutcome {
    pub exit_code: i32,
    pub stderr: String,
}

impl ReapiBuildOutcome {
    fn error(runtime_mode: &str, invalidated_files: usize, message: &str) -> Self {
        Self {
            exit_code: 2,
            stderr: format!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"{}\",\"invalidated_files\":{}}}\n",
                json_escape(message),
                runtime_mode,
                invalidated_files,
            ),
        }
    }
}

#[cfg(test)]
#[path = "../../slug_core_v2/src/runtime/source_staging/test_workspace.rs"]
mod requested_root_guard_fixture;

#[cfg(test)]
mod requested_root_guard_tests {
    use slug_core_v2::runtime::BzlmodCommandPolicyKey;
    use slug_core_v2::runtime::BzlmodEnvironmentPolicyKey;
    use slug_core_v2::runtime::LockfileMode;
    use slug_core_v2::runtime::ProcessHostOwner;
    use slug_core_v2::runtime::TargetPattern;
    use slug_core_v2::runtime::WorkspaceRuntime;

    use super::requested_root_guard_fixture as fixture;
    use super::*;

    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn requested_alias_and_generated_roots_reject_before_execution_setup() {
        let parent =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/wp741/guards");
        std::fs::create_dir_all(&parent).unwrap();
        let root = Scratch(parent.canonicalize().unwrap().join(format!(
            "server-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        )));
        std::fs::create_dir(&root.0).unwrap();
        fixture::write(&root.0);
        std::fs::write(
            root.0.join("defs.bzl"),
            r#"
def _generate(ctx):
    ctx.actions.write(ctx.outputs.out, "bytes")
    ctx.actions.write(ctx.actions.declare_file("unrelated.out"), "unselected")
    return [DefaultInfo(files = depset([ctx.outputs.out]))]
generate = rule(implementation = _generate, attrs = {"out": attr.output(mandatory = True)})
def _empty(ctx): return []
empty = rule(implementation = _empty)
"#,
        )
        .unwrap();
        std::fs::write(
            root.0.join("BUILD.bazel"),
            r#"
load(":defs.bzl", "generate", "empty")
platform(name = "platform")
exports_files(["input"])
generate(name = "producer", out = "generated.out")
alias(name = "rule_alias", actual = ":producer")
alias(name = "generated_alias", actual = ":generated.out")
empty(name = "empty")
"#,
        )
        .unwrap();
        let workspace = WorkspaceRuntime::new(&root.0, ProcessHostOwner::native()).unwrap();
        let remote = RemoteConfig::from_args(&["--remote_executor=http://[invalid"]).unwrap();
        for (targets, rejected) in [
            (vec!["//:rule_alias"], true),
            (vec!["//:generated_alias"], true),
            (vec!["//:generated.out"], true),
            (vec!["//:input", "//:empty"], false),
        ] {
            let accepted = workspace
                .build_command_with_repository_environment(
                    &targets
                        .iter()
                        .map(|target| TargetPattern::parse(target).unwrap())
                        .collect::<Vec<_>>(),
                    BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                    LockfileMode::Update,
                    &[format!("file://{}/empty-registry", root.0.display())],
                    Default::default(),
                    Default::default(),
                )
                .unwrap();
            let published = accepted
                .project(|terminal| {
                    let evaluation = terminal.as_ref().as_ref().unwrap();
                    assert_eq!(
                        evaluation.requested_target_kinds().any(|kind| matches!(
                            kind,
                            ConfiguredNodeKind::Alias | ConfiguredNodeKind::GeneratedFile
                        )),
                        rejected
                    );
                    let mut output_paths = Vec::new();
                    for analysis in evaluation.analyses() {
                        let configuration = analysis
                            .configured_target_key()
                            .unwrap()
                            .configuration()
                            .slug_configuration()
                            .unwrap();
                        let output_root =
                            slug_core_v2::runtime::configured_output_root(&root.0, configuration);
                        for action in analysis.actions() {
                            for output in action.outputs() {
                                output_paths.push(output_root.join(output.path()));
                            }
                        }
                    }
                    assert_eq!(output_paths.len(), if rejected { 2 } else { 0 });
                    assert!(output_paths.iter().all(|path| !path.exists()));
                    let execute = || {
                        let outcome = run_reapi_run_build(
                            &root.0,
                            evaluation,
                            evaluation.analyzed_target_count(),
                            evaluation.declared_action_count(),
                            &remote,
                            "daemon",
                            0,
                        );
                        slug_core_v2::runtime::TerminalOutput::new(
                            outcome.exit_code,
                            String::new(),
                            outcome.stderr,
                        )
                    };
                    let output = if rejected {
                        // An unguarded path panics if it reaches nested block_on.
                        tokio::runtime::Builder::new_current_thread()
                            .build()
                            .unwrap()
                            .block_on(async { execute() })
                    } else {
                        execute()
                    };
                    assert!(output_paths.iter().all(|path| !path.exists()));
                    output
                })
                .publish();
            let (_, code, _, stderr) = published.into_parts();
            assert_eq!(code, 2);
            if rejected {
                assert!(
                    stderr.contains(
                        "REAPI execution for requested aliases and generated files is unsupported"
                    ),
                    "{stderr}"
                );
            } else {
                assert!(
                    stderr.contains("no executable actions were declared"),
                    "{stderr}"
                );
            }
            assert!(!root.0.join("generated.out").exists());
        }
    }
}
