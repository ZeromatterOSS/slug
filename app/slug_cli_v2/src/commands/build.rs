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
use slug_core_v2::runtime::evaluate_workspace_targets_with_bzlmod_inputs;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

pub fn run(argv: Vec<String>) -> i32 {
    let request = match BuildRequest::parse(&argv) {
        Ok(request) => request,
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
        let bzlmod = slug_server_v2::BzlmodRequestInputs::from_normalized(
            &request.bzlmod_policy,
            &environment_policy,
            &request.lockfile_mode,
        );
        return run_daemon_build(&argv, &output_base, request, bzlmod);
    }

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

    match evaluate_workspace_targets_with_bzlmod_inputs(
        &workspace,
        &request.targets,
        request.bzlmod_policy.clone(),
        environment_policy,
        request.lockfile_mode.clone(),
    ) {
        Ok(evaluation) => {
            let argv_json = argv
                .iter()
                .map(|arg| format!("\"{}\"", json_escape(arg)))
                .collect::<Vec<_>>()
                .join(",");
            let analyzed_target_count = evaluation
                .packages
                .iter()
                .filter(|package| package.analysis.is_some())
                .count();
            let declared_action_count = evaluation
                .packages
                .iter()
                .filter_map(|package| package.analysis.as_ref())
                .map(|analysis| analysis.actions().len())
                .sum::<usize>();
            let completed_boundary = if analyzed_target_count == 0 {
                "dice_starlark_package_loading"
            } else {
                "dice_starlark_rule_analysis"
            };
            let remote_args = argv.iter().map(String::as_str).collect::<Vec<_>>();
            let remote = match RemoteConfig::from_args(&remote_args) {
                Ok(remote) => remote,
                Err(error) => {
                    eprintln!(
                        "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
                        json_escape(&error.to_string())
                    );
                    return 2;
                }
            };
            if remote.mode() == RemoteMode::Execute {
                return run_reapi_build(
                    &workspace,
                    &evaluation,
                    analyzed_target_count,
                    declared_action_count,
                    &remote,
                );
            }
            eprintln!(
                "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"argv\":[{}],\"target_count\":{},\"loaded_package_count\":{},\"analyzed_target_count\":{},\"declared_action_count\":{},\"runtime_mode\":\"one-shot\",\"completed_boundary\":\"{}\"}}",
                argv_json,
                request.targets.len(),
                evaluation.packages.len(),
                analyzed_target_count,
                declared_action_count,
                completed_boundary,
            );
            2
        }
        Err(error) => {
            eprintln!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
                json_escape(&error.to_string())
            );
            2
        }
    }
}

fn run_reapi_build(
    workspace: &std::path::Path,
    evaluation: &slug_core_v2::runtime::WorkspaceBuildEvaluation,
    analyzed_target_count: usize,
    declared_action_count: usize,
    remote: &RemoteConfig,
) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
                json_escape(&error.to_string())
            );
            return 2;
        }
    };
    let output_root = workspace.join("bazel-bin");
    let execution = runtime.block_on(async {
        let mut reapi_actions = 0_u64;
        let mut direct_local_actions = 0_u64;
        let mut ac_hits = 0_u64;
        let mut ac_misses = 0_u64;
        let mut action_digests: Vec<String> = Vec::new();
        let mut uploaded_digests: Vec<String> = Vec::new();
        let mut materialized_outputs: Vec<String> = Vec::new();
        let mut platform_properties: Vec<(String, String)> = remote
            .default_exec_properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        platform_properties.sort();
        for package in &evaluation.packages {
            let Some(analysis) = &package.analysis else {
                continue;
            };
            for action in analysis.actions() {
                let result = slug_reapi_v2::execute_action(remote, action)
                    .await
                    .map_err(|error| error.to_string())?;
                slug_reapi_v2::materialize_outputs(&output_root, &result)
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
            eprintln!(
                "{{\"success\":true,\"command\":\"build\",\"analyzed_target_count\":{},\"declared_action_count\":{},\"reapi_actions\":{},\"direct_local_actions\":{},\"ac_hits\":{},\"ac_misses\":{},\"action_digests\":[{}],\"uploaded_digests\":[{}],\"materialized_outputs\":[{}],\"platform_properties\":{{{}}},\"runtime_mode\":\"one-shot\",\"completed_boundary\":\"reapi_native_execution\"}}",
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
            );
            0
        }
        Ok(_) => {
            eprintln!(
                "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"message\":\"no executable actions were declared\",\"runtime_mode\":\"one-shot\"}}"
            );
            2
        }
        Err(error) => {
            eprintln!(
                "{{\"error\":\"build_runtime_error\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"one-shot\"}}",
                json_escape(&error)
            );
            2
        }
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
        executor: remote.executor.clone(),
        default_exec_properties: remote
            .default_exec_properties
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        bzlmod,
    };

    match slug_server_v2::send_build_request(&socket, &daemon_request) {
        Ok(response) => {
            if !response.stdout.is_empty() {
                print!("{}", response.stdout);
            }
            if !response.stderr.is_empty() {
                eprintln!("{}", response.stderr);
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
