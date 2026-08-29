/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_commands_v2::CommandKind;
use slug_commands_v2::CommandParseError;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::BuildCommandEvaluation;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_repository_environment;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

pub fn run(argv: Vec<String>) -> i32 {
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
                json_escape(&error.to_string())
            );
            return 2;
        }
    };
    let request = match BuildRequest::parse_at_workspace(&argv, &workspace) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Build, argv, Err(error)),
    };
    let repository_environment = match super::repository_environment::capture_repository_environment(
        &workspace,
        &request.repository_environment_overrides,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => return super::emit_result(CommandKind::Build, argv, Err(error)),
    };
    let environment_value = match capture_bzlmod_allow_yanked_versions() {
        Ok(value) => value,
        Err(error) => return super::emit_result(CommandKind::Build, argv, Err(error)),
    };
    let environment_policy = match normalize_bzlmod_environment_value(environment_value.as_deref())
    {
        Ok(policy) => policy,
        Err(error) => return super::emit_result(CommandKind::Build, argv, Err(error)),
    };

    // Daemon mode: when --output_base is set, route through the persistent
    // daemon so DICE state survives across builds (gate clause 5).
    if let Some(output_base) = extract_output_base(&argv) {
        let bzlmod = slug_server_v2::BzlmodRequestInputs::from_normalized_with_registry_urls(
            &request.bzlmod_policy,
            &environment_policy,
            &request.lockfile_mode,
            &request.registry_urls,
        );
        let repository_environment =
            slug_server_v2::RepositoryEnvironmentRequestInputs::from_normalized(
                &repository_environment,
            );
        return run_daemon_build(&argv, &output_base, request, bzlmod, repository_environment);
    }

    let accepted = match evaluate_workspace_build_command_with_repository_environment(
        &workspace,
        &request.targets,
        request.bzlmod_policy.clone(),
        environment_policy,
        request.lockfile_mode.clone(),
        &request.registry_urls,
        repository_environment,
        request.configuration_overlay.clone(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => {
            eprint!(
                "{}",
                build_error_json("build_runtime_error", &error.to_string(), "one-shot")
            );
            return 2;
        }
    };
    let published = accepted
        .project(|terminal| match terminal.as_ref() {
            Err(error) => {
                let (kind, exit_code) = error.terminal_error();
                TerminalOutput::new(
                    exit_code,
                    String::new(),
                    build_error_json(kind, &error.to_string(), "one-shot"),
                )
            }
            Ok(evaluation) => {
            if evaluation.is_observed_exported_source() {
                return TerminalOutput::new(
                    0,
                    String::new(),
                    "{\"success\":true,\"command\":\"build\",\"target_count\":1,\"loaded_package_count\":1,\"analyzed_target_count\":0,\"declared_action_count\":0,\"runtime_mode\":\"one-shot\",\"completed_boundary\":\"dice_exported_source_file\"}\n".to_owned(),
                );
            }
            let argv_json = super::repository_environment::redacted_repository_environment_argv(&argv)
                .into_iter()
                .map(|arg| format!("\"{}\"", json_escape(arg)))
                .collect::<Vec<_>>()
                .join(",");
            let analyzed_target_count = evaluation.analyzed_target_count();
            let declared_action_count = evaluation.declared_action_count();
            let completed_boundary = if analyzed_target_count == 0 {
                "dice_starlark_package_loading"
            } else {
                "dice_starlark_rule_analysis"
            };
            let remote_args = argv.iter().map(String::as_str).collect::<Vec<_>>();
            let remote = match RemoteConfig::from_args(&remote_args) {
                Ok(remote) => remote,
                Err(error) => {
                    return TerminalOutput::new(
                        2,
                        String::new(),
                        build_error_json("build_runtime_error", &error.to_string(), "one-shot"),
                    );
                }
            };
            if remote.mode() == RemoteMode::Execute {
                run_reapi_build(
                    &workspace,
                    evaluation,
                    analyzed_target_count,
                    declared_action_count,
                    &remote,
                )
            } else {
                TerminalOutput::new(
                    2,
                    String::new(),
                    format!(
                        "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"argv\":[{}],\"target_count\":{},\"loaded_package_count\":{},\"analyzed_target_count\":{},\"declared_action_count\":{},\"runtime_mode\":\"one-shot\",\"completed_boundary\":\"{}\"}}\n",
                        argv_json,
                        request.targets.len(),
                        evaluation.loaded_package_count(),
                        analyzed_target_count,
                        declared_action_count,
                        completed_boundary,
                    ),
                )
            }
            }
        })
        .publish();
    let (_terminal, exit_code, stdout, stderr) = published.into_parts();
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    exit_code
}

fn build_error_json(kind: &str, message: &str, runtime_mode: &str) -> String {
    format!(
        "{{\"error\":\"{}\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"{}\"}}\n",
        kind,
        json_escape(message),
        runtime_mode,
    )
}

fn run_reapi_build(
    workspace: &std::path::Path,
    evaluation: &BuildCommandEvaluation,
    analyzed_target_count: usize,
    declared_action_count: usize,
    remote: &RemoteConfig,
) -> TerminalOutput {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return TerminalOutput::new(
                2,
                String::new(),
                build_error_json("build_runtime_error", &error.to_string(), "one-shot"),
            );
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
                .resolved_file_write_semantic_views_in_closure()
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
            TerminalOutput::new(
                0,
                String::new(),
                format!(
                    "{{\"success\":true,\"command\":\"build\",\"analyzed_target_count\":{},\"declared_action_count\":{},\"reapi_actions\":{},\"direct_local_actions\":{},\"ac_hits\":{},\"ac_misses\":{},\"action_digests\":[{}],\"uploaded_digests\":[{}],\"materialized_outputs\":[{}],\"platform_properties\":{{{}}},\"runtime_mode\":\"one-shot\",\"completed_boundary\":\"reapi_native_execution\"}}\n",
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
                ),
            )
        }
        Ok(_) => TerminalOutput::new(
            2,
            String::new(),
            "{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"message\":\"no executable actions were declared\",\"runtime_mode\":\"one-shot\"}\n"
                .to_owned(),
        ),
        Err(error) => TerminalOutput::new(
            2,
            String::new(),
            build_error_json("build_runtime_error", &error, "one-shot"),
        ),
    }
}

/// Extract `--output_base=PATH` or `--output_base PATH` from the argv.
pub(super) fn extract_output_base(argv: &[String]) -> Option<String> {
    let mut iter = argv.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--output_base=") {
            return Some(value.to_string());
        }
        if arg == "--output_base" {
            if let Some(next) = iter.next() {
                return Some(next.clone());
            }
        }
    }
    None
}

/// Run a build through the persistent daemon. If the daemon is not running,
/// start it as a background process first. The daemon holds DICE state across
/// builds so `.bzl` edits are invalidated and replayed in the same process.
fn run_daemon_build(
    argv: &[String],
    output_base: &str,
    request: BuildRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
    repository_environment: slug_server_v2::RepositoryEnvironmentRequestInputs,
) -> i32 {
    let output_base_path = std::path::Path::new(output_base);
    let _ = std::fs::create_dir_all(output_base_path);
    let socket = slug_server_v2::socket_path(output_base_path);

    // Try to connect; if the daemon isn't running, start it.
    if std::os::unix::net::UnixStream::connect(&socket).is_err() {
        if let Err(error) = start_daemon(output_base_path) {
            eprintln!(
                "{{\"error\":\"daemon_start_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\"}}",
                json_escape(&error.to_string())
            );
            return 2;
        }
    }

    let remote_args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let remote = match RemoteConfig::from_args(&remote_args) {
        Ok(remote) => remote,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\"}}",
                json_escape(&error.to_string())
            );
            return 2;
        }
    };

    let daemon_request = slug_server_v2::BuildRequest {
        targets: request.targets.iter().map(|t| t.to_string()).collect(),
        configuration_overlay: request.configuration_overlay,
        executor: remote.executor.clone(),
        default_exec_properties: remote
            .default_exec_properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        bzlmod,
        repository_environment,
    };

    match slug_server_v2::send_build_request(&socket, &daemon_request) {
        Ok(response) => {
            if !response.stdout.is_empty() {
                print!("{}", response.stdout);
            }
            if !response.stderr.is_empty() {
                eprint!("{}", response.stderr);
            }
            response.exit_code
        }
        Err(error) => {
            eprintln!(
                "{{\"error\":\"daemon_connect_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\"}}",
                json_escape(&error.to_string())
            );
            2
        }
    }
}

pub(super) fn capture_bzlmod_allow_yanked_versions() -> Result<Option<String>, CommandParseError> {
    std::env::var_os("BZLMOD_ALLOW_YANKED_VERSIONS")
        .map(|value| {
            value
                .into_string()
                .map_err(|_| CommandParseError::InvalidFlagValue {
                    flag: "BZLMOD_ALLOW_YANKED_VERSIONS".to_owned(),
                    message: "environment value is not valid Unicode".to_owned(),
                })
        })
        .transpose()
}

/// Start the daemon as a background process. The current binary re-execs
/// itself with `--serve` to enter server mode.
pub(super) fn start_daemon(output_base: &std::path::Path) -> anyhow::Result<()> {
    let socket = slug_server_v2::socket_path(output_base);
    let pid_file = slug_server_v2::pid_path(output_base);
    let workspace = std::env::current_dir()
        .map_err(|e| anyhow::anyhow!("getting current dir for daemon: {e}"))?;
    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("getting current exe for daemon: {e}"))?;
    let _ = std::fs::remove_file(&socket);
    let child = std::process::Command::new(&exe)
        .arg("--serve")
        .arg("--socket")
        .arg(&socket)
        .arg("--workspace")
        .arg(&workspace)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawning daemon: {e}"))?;
    std::fs::write(&pid_file, child.id().to_string())
        .map_err(|e| anyhow::anyhow!("writing pid file: {e}"))?;
    // Wait for the socket to become connectable.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    anyhow::bail!(
        "daemon did not become ready within 10s (socket: {})",
        socket.display()
    )
}
