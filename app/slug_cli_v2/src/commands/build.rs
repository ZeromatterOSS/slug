/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_commands_v2::CommandParseError;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceRuntime;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_repository_environment;
use slug_reapi_v2::ActionChainReapiTransport;
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
        Err(error) => return emit_parse_error(&argv, error),
    };
    let repository_environment = match super::repository_environment::capture_repository_environment(
        &workspace,
        &request.repository_environment_overrides,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => return emit_parse_error(&argv, error),
    };
    let environment_value = match capture_bzlmod_allow_yanked_versions() {
        Ok(value) => value,
        Err(error) => return emit_parse_error(&argv, error),
    };
    let environment_policy = match normalize_bzlmod_environment_value(environment_value.as_deref())
    {
        Ok(policy) => policy,
        Err(error) => return emit_parse_error(&argv, error),
    };

    let output_base = extract_output_base(&argv);
    let runtime_mode = if output_base.is_some() {
        "daemon"
    } else {
        "one-shot"
    };
    let remote = match RemoteConfig::from_args(&argv.iter().map(String::as_str).collect::<Vec<_>>())
    {
        Ok(remote) => remote,
        Err(error) => {
            eprint!(
                "{}",
                build_error_json("build_runtime_error", &error.to_string(), runtime_mode)
            );
            return 2;
        }
    };
    // Validate the entire immutable Execute policy before daemon startup or dispatch.
    let transport = if remote.mode() == RemoteMode::Execute {
        match ActionChainReapiTransport::new(remote.clone()) {
            Ok(transport) => Some(transport),
            Err(error) => {
                eprint!(
                    "{}",
                    build_error_json("build_runtime_error", &error.to_string(), runtime_mode)
                );
                return 2;
            }
        }
    } else {
        None
    };

    // Daemon mode: when --output_base is set, route through the persistent
    // daemon so DICE state survives across builds (gate clause 5).
    if let Some(output_base) = output_base {
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
        return run_daemon_build(
            &output_base,
            request,
            bzlmod,
            repository_environment,
            &remote,
        );
    }

    if let Some(transport) = transport {
        let accepted = WorkspaceRuntime::new(&workspace, ProcessHostOwner::native())
            .map_err(|error| error.to_string())
            .and_then(|runtime| {
                runtime
                    .execute_and_publish_requested_actions_with_repository_environment(
                        &request.targets,
                        request.bzlmod_policy,
                        environment_policy,
                        request.lockfile_mode,
                        &request.registry_urls,
                        repository_environment,
                        request.configuration_overlay,
                        &transport,
                    )
                    .map_err(|error| error.to_string())
            });
        let accepted = match accepted {
            Ok(accepted) => accepted,
            Err(error) => {
                eprint!(
                    "{}",
                    build_error_json("build_runtime_error", &error, "one-shot")
                );
                return 2;
            }
        };
        let published = accepted
            .project(|result| match result {
                Err(error) => {
                    let (kind, code) = error.terminal_error();
                    TerminalOutput::new(
                        code,
                        String::new(),
                        build_error_json(kind, &error.to_string(), "one-shot"),
                    )
                }
                Ok(result) => {
                    match slug_reapi_v2::requested_build_success_json(result, "one-shot", None) {
                        Ok(json) => TerminalOutput::new(0, String::new(), json),
                        Err(error) => TerminalOutput::new(
                            2,
                            String::new(),
                            build_error_json("build_runtime_error", &error.to_string(), "one-shot"),
                        ),
                    }
                }
            })
            .publish();
        let (_, code, stdout, stderr) = published.into_parts();
        return emit_output(code, stdout, stderr);
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
            let argv_json = redacted_build_argv(&argv)
                .into_iter()
                .map(|arg| format!("\"{}\"", json_escape(&arg)))
                .collect::<Vec<_>>()
                .join(",");
            let analyzed_target_count = evaluation.analyzed_target_count();
            let declared_action_count = evaluation.declared_action_count();
            let completed_boundary = if analyzed_target_count == 0 {
                "dice_starlark_package_loading"
            } else {
                "dice_starlark_rule_analysis"
            };
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
        })
        .publish();
    let (_, exit_code, stdout, stderr) = published.into_parts();
    emit_output(exit_code, stdout, stderr)
}

fn emit_output(code: i32, stdout: String, stderr: String) -> i32 {
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    code
}

fn redacted_build_argv(argv: &[String]) -> Vec<String> {
    let mut value = false;
    super::repository_environment::redacted_repository_environment_argv(argv)
        .into_iter()
        .map(|arg| {
            if value {
                value = false;
                "<redacted>".to_owned()
            } else if arg == "--remote_header" {
                value = true;
                arg.to_owned()
            } else if arg.starts_with("--remote_header=") {
                "--remote_header=<redacted>".to_owned()
            } else {
                arg.to_owned()
            }
        })
        .collect()
}

fn emit_parse_error(argv: &[String], error: CommandParseError) -> i32 {
    let mut message = error.to_string();
    let mut value = false;
    for arg in argv {
        let header = if value {
            value = false;
            Some(arg.as_str())
        } else if arg == "--remote_header" {
            value = true;
            None
        } else {
            arg.strip_prefix("--remote_header=")
        };
        if let Some(header) = header {
            for sensitive in [
                header,
                header.split_once('=').map_or(header, |(_, value)| value),
            ] {
                if !sensitive.is_empty() && message.contains(sensitive) {
                    message = "invalid build arguments (remote header redacted)".into();
                }
            }
        }
    }
    eprint!(
        "{}",
        build_error_json("command_parse_error", &message, "one-shot")
    );
    2
}

fn build_error_json(kind: &str, message: &str, runtime_mode: &str) -> String {
    format!(
        "{{\"error\":\"{}\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"{}\"}}\n",
        kind,
        json_escape(message),
        runtime_mode,
    )
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
    output_base: &str,
    request: BuildRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
    repository_environment: slug_server_v2::RepositoryEnvironmentRequestInputs,
    remote: &RemoteConfig,
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

    let daemon_request = slug_server_v2::BuildRequest {
        targets: request.targets.iter().map(|t| t.to_string()).collect(),
        configuration_overlay: request.configuration_overlay,
        remote: remote.into(),
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
    let mut child = std::process::Command::new(&exe)
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
    if let Err(error) = std::fs::write(&pid_file, child.id().to_string()) {
        return Err(terminate_and_reap(
            &mut child,
            anyhow::anyhow!("writing pid file: {error}"),
        ));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    wait_for_daemon_ready(&socket, &mut child, deadline)
}

fn wait_for_daemon_ready(
    socket: &std::path::Path,
    child: &mut std::process::Child,
    deadline: std::time::Instant,
) -> anyhow::Result<()> {
    while std::time::Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                anyhow::bail!("daemon exited before becoming ready (status: {status})")
            }
            Ok(None) => {}
            Err(error) => {
                return Err(terminate_and_reap(
                    child,
                    anyhow::anyhow!(
                        "checking daemon process status while waiting for readiness: {error}"
                    ),
                ));
            }
        }
        if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err(terminate_and_reap(
        child,
        anyhow::anyhow!(
            "daemon did not become ready within 10s (socket: {})",
            socket.display()
        ),
    ))
}

fn terminate_and_reap(child: &mut std::process::Child, primary: anyhow::Error) -> anyhow::Error {
    let kill_result = child.kill();
    daemon_cleanup_error(primary, kill_result, child.wait())
}

fn daemon_cleanup_error(
    primary: anyhow::Error,
    kill_result: std::io::Result<()>,
    wait_result: std::io::Result<std::process::ExitStatus>,
) -> anyhow::Error {
    // A failed kill can race with child exit; only a successful reap proves cleanup.
    match wait_result {
        Ok(_) => primary,
        Err(wait_error) => {
            let cleanup = match kill_result {
                Ok(()) => format!("waiting for daemon: {wait_error}"),
                Err(kill_error) => {
                    format!("killing daemon: {kill_error}; waiting for daemon: {wait_error}")
                }
            };
            let message = format!("{primary}; daemon cleanup failed: {cleanup}");
            primary.context(message)
        }
    }
}

#[cfg(test)]
mod daemon_readiness_tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    #[test]
    fn controlled_child_exit_is_reported_before_the_deadline() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let socket = std::env::temp_dir().join(format!("slug-no-socket-{}", child.id()));
        let error = wait_for_daemon_ready(
            &socket,
            &mut child,
            std::time::Instant::now() + std::time::Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "daemon exited before becoming ready (status: exit status: 0)"
        );
    }

    #[test]
    fn readiness_timeout_kills_and_reaps_a_live_child() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("5")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let socket = std::env::temp_dir().join(format!("slug-no-socket-{}", child.id()));
        let result = wait_for_daemon_ready(&socket, &mut child, std::time::Instant::now());
        // Check before try_wait so the test cannot reap a zombie left by cleanup.
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new(&format!("/proc/{}", child.id())).exists());
        assert_eq!(child.try_wait().unwrap().unwrap().signal(), Some(9));
        assert_eq!(
            result.unwrap_err().to_string(),
            format!(
                "daemon did not become ready within 10s (socket: {})",
                socket.display()
            )
        );
    }

    #[test]
    fn cleanup_failures_append_to_the_primary_error_and_reap_resolves_kill_races() {
        for (kill_failed, wait_failed, suffix) in [
            (false, false, ""),
            (true, false, ""),
            (
                false,
                true,
                "; daemon cleanup failed: waiting for daemon: wait failed",
            ),
            (
                true,
                true,
                "; daemon cleanup failed: killing daemon: kill failed; waiting for daemon: wait failed",
            ),
        ] {
            let primary = std::io::Error::other("startup failed");
            let kill_result = if kill_failed {
                Err(std::io::Error::other("kill failed"))
            } else {
                Ok(())
            };
            let wait_result = if wait_failed {
                Err(std::io::Error::other("wait failed"))
            } else {
                Ok(std::process::ExitStatus::from_raw(0))
            };
            let error = daemon_cleanup_error(primary.into(), kill_result, wait_result);
            assert_eq!(error.to_string(), format!("startup failed{suffix}"));
            assert_eq!(error.root_cause().to_string(), "startup failed");
        }
    }
}
