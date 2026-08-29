/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the above-listed
 * licenses.
 */

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;

use slug_commands_v2::CommandKind;
use slug_commands_v2::CommandParseError;
use slug_commands_v2::build::BuildRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_commands_v2::run::RunRequest;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_repository_environment;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;
use slug_server_v2::RUN_ENVIRONMENT_TO_CLEAR;
use slug_server_v2::RunLaunchPlan;

pub fn run(argv: Vec<String>) -> i32 {
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => return run_error(&error.to_string(), "one-shot", 2),
    };
    let request = match parse_run_at_workspace(&argv, &workspace) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Run, argv, Err(error)),
    };
    let repository_environment = match super::repository_environment::capture_repository_environment(
        &workspace,
        &request.repository_environment_overrides,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => return super::emit_result(CommandKind::Run, argv, Err(error)),
    };
    let environment = match super::build::capture_bzlmod_allow_yanked_versions()
        .and_then(|value| normalize_bzlmod_environment_value(value.as_deref()))
    {
        Ok(environment) => environment,
        Err(error) => return super::emit_result(CommandKind::Run, argv, Err(error)),
    };
    let remote_args = request
        .flags
        .iter()
        .map(|flag| flag.raw.as_str())
        .collect::<Vec<_>>();
    let remote = match RemoteConfig::from_args(&remote_args) {
        Ok(remote) if remote.mode() == RemoteMode::Execute => remote,
        Ok(_) => return run_error("run requires --remote_executor", "one-shot", 2),
        Err(error) => return run_error(&error.to_string(), "one-shot", 2),
    };
    if let Some(output_base) = super::build::extract_output_base(&argv) {
        let bzlmod = slug_server_v2::BzlmodRequestInputs::from_normalized(
            &request.bzlmod_policy,
            &environment,
            &request.lockfile_mode,
        );
        let repository_environment =
            slug_server_v2::RepositoryEnvironmentRequestInputs::from_normalized(
                &repository_environment,
            );
        return run_daemon(
            &output_base,
            request,
            bzlmod,
            repository_environment,
            remote,
        );
    }
    let accepted = match evaluate_workspace_build_command_with_repository_environment(
        &workspace,
        std::slice::from_ref(&request.target),
        request.bzlmod_policy,
        environment,
        request.lockfile_mode,
        &[],
        repository_environment,
        Default::default(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => return run_error(&error.to_string(), "one-shot", 2),
    };
    let launch_plan = std::cell::RefCell::new(None);
    let published = accepted
        .project(|terminal| match terminal.as_ref() {
            Err(error) => {
                let (kind, exit_code) = error.terminal_error();
                TerminalOutput::new(
                    exit_code,
                    String::new(),
                    run_error_json(kind, &error.to_string(), "one-shot"),
                )
            }
            Ok(evaluation) => {
                let (outcome, executable) = slug_server_v2::run_reapi_executable(
                    &workspace, evaluation, &remote, "one-shot", 0,
                );
                if let Some(executable) = executable {
                    launch_plan.replace(Some(RunLaunchPlan {
                        executable_path: executable.display().to_string(),
                        working_directory: workspace.display().to_string(),
                        environment_to_clear: expected_environment_clear(),
                    }));
                }
                TerminalOutput::new(outcome.exit_code, String::new(), outcome.stderr)
            }
        })
        .publish();
    let (_terminal, exit_code, stdout, stderr) = published.into_parts();
    finish_build(
        exit_code,
        stdout,
        stderr,
        launch_plan.into_inner(),
        &request.program_args,
        "one-shot",
    )
}

fn run_daemon(
    output_base: &str,
    request: RunRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
    repository_environment: slug_server_v2::RepositoryEnvironmentRequestInputs,
    remote: RemoteConfig,
) -> i32 {
    let output_base = Path::new(output_base);
    if let Err(error) = std::fs::create_dir_all(output_base) {
        return run_error(&error.to_string(), "daemon", 2);
    }
    let socket = slug_server_v2::socket_path(output_base);
    if std::os::unix::net::UnixStream::connect(&socket).is_err()
        && let Err(error) = super::build::start_daemon(output_base)
    {
        return run_error(&error.to_string(), "daemon", 2);
    }
    let daemon_request = slug_server_v2::BuildRequest {
        targets: vec![request.target.to_string()],
        configuration_overlay: Default::default(),
        executor: remote.executor,
        default_exec_properties: remote.default_exec_properties.into_iter().collect(),
        bzlmod,
        repository_environment,
    };
    match slug_server_v2::send_run_request(&socket, &daemon_request) {
        Ok(response) => finish_build(
            response.exit_code,
            response.stdout,
            response.stderr,
            response.run_launch_plan,
            &request.program_args,
            "daemon",
        ),
        Err(error) => run_error(&error.to_string(), "daemon", 2),
    }
}

fn finish_build(
    exit_code: i32,
    stdout: String,
    stderr: String,
    plan: Option<RunLaunchPlan>,
    program_args: &[String],
    runtime_mode: &str,
) -> i32 {
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    match (exit_code, plan) {
        (0, Some(plan)) => launch(plan, program_args, runtime_mode),
        (0, None) => run_error(
            "successful run build omitted launch authorization",
            runtime_mode,
            1,
        ),
        (code, None) => code,
        (_, Some(_)) => run_error(
            "failed run build returned launch authorization",
            runtime_mode,
            1,
        ),
    }
}

fn launch(plan: RunLaunchPlan, program_args: &[String], runtime_mode: &str) -> i32 {
    let executable = Path::new(&plan.executable_path);
    let working_directory = Path::new(&plan.working_directory);
    if !executable.is_absolute()
        || !working_directory.is_absolute()
        || plan.environment_to_clear != expected_environment_clear()
    {
        return run_error("invalid run launch authorization", runtime_mode, 1);
    }
    match std::fs::symlink_metadata(executable) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.permissions().mode() & 0o111 != 0 => {}
        Ok(_) => return run_error("authorized executable is not executable", runtime_mode, 1),
        Err(error) => return run_error(&error.to_string(), runtime_mode, 1),
    }
    let mut command = Command::new(executable);
    command.args(program_args).current_dir(working_directory);
    for name in RUN_ENVIRONMENT_TO_CLEAR {
        command.env_remove(name);
    }
    match command.status() {
        Ok(status) => status
            .code()
            .or_else(|| status.signal().map(|signal| 128 + signal))
            .unwrap_or(1),
        Err(error) => run_error(&error.to_string(), runtime_mode, 1),
    }
}

fn expected_environment_clear() -> Vec<String> {
    RUN_ENVIRONMENT_TO_CLEAR
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

fn run_error(message: &str, runtime_mode: &str, exit_code: i32) -> i32 {
    eprint!(
        "{}",
        run_error_json("run_runtime_error", message, runtime_mode)
    );
    exit_code
}

fn run_error_json(kind: &str, message: &str, runtime_mode: &str) -> String {
    format!(
        "{{\"error\":\"{}\",\"command\":\"run\",\"message\":\"{}\",\"runtime_mode\":\"{}\"}}\n",
        kind,
        json_escape(message),
        runtime_mode,
    )
}

fn parse_run_at_workspace(
    argv: &[String],
    workspace: &Path,
) -> Result<RunRequest, CommandParseError> {
    let has_override = argv
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| matches!(flag_name(arg), Some("override_module")));
    if !has_override {
        return RunRequest::parse(argv);
    }

    let mut policy_args = Vec::new();
    let mut run_args = Vec::with_capacity(argv.len());
    let mut passthrough = false;
    for arg in argv {
        if arg == "--" {
            passthrough = true;
            run_args.push(arg.clone());
            continue;
        }
        let name = (!passthrough).then(|| flag_name(arg)).flatten();
        if matches!(
            name,
            Some(
                "allow_yanked_versions"
                    | "ignore_dev_dependency"
                    | "noignore_dev_dependency"
                    | "override_module"
            )
        ) {
            policy_args.push(arg.clone());
        }
        if name != Some("override_module") {
            run_args.push(arg.clone());
        }
    }
    policy_args.push("//:__slug_override_policy__".to_owned());
    let normalized = BuildRequest::parse_at_workspace(&policy_args, workspace)?;
    let mut request = RunRequest::parse(&run_args)?;
    request.bzlmod_policy = normalized.bzlmod_policy;
    Ok(request)
}

fn flag_name(arg: &str) -> Option<&str> {
    arg.strip_prefix("--")
        .map(|flag| flag.split_once('=').map_or(flag, |(name, _)| name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_or_unreviewed_launch_authorization() {
        let relative = RunLaunchPlan {
            executable_path: "out/hello".to_owned(),
            working_directory: "/workspace".to_owned(),
            environment_to_clear: expected_environment_clear(),
        };
        assert_eq!(launch(relative, &[], "one-shot"), 1);

        let mut unreviewed = expected_environment_clear();
        unreviewed.push("CLIENT_SECRET".to_owned());
        let unreviewed = RunLaunchPlan {
            executable_path: "/workspace/out/hello".to_owned(),
            working_directory: "/workspace".to_owned(),
            environment_to_clear: unreviewed,
        };
        assert_eq!(launch(unreviewed, &[], "daemon"), 1);
    }

    #[test]
    fn launch_checks_mode_and_preserves_program_arguments_and_exit() {
        let directory =
            std::env::temp_dir().join(format!("slug-run-launch-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let executable = directory.join("probe.sh");
        std::fs::write(
            &executable,
            "#!/bin/sh\n[ \"$1\" = alpha ] || exit 9\nexit 7\n",
        )
        .unwrap();
        let plan = RunLaunchPlan {
            executable_path: executable.display().to_string(),
            working_directory: directory.display().to_string(),
            environment_to_clear: expected_environment_clear(),
        };
        assert_eq!(launch(plan.clone(), &["alpha".to_owned()], "one-shot"), 1);

        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        assert_eq!(launch(plan, &["alpha".to_owned()], "one-shot"), 7);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
