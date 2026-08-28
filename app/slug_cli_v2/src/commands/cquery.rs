/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 * You may select, at your option, one of the above-listed licenses.
 */

use slug_commands_v2::CommandKind;
use slug_commands_v2::cquery::CqueryOutputMode;
use slug_commands_v2::cquery::CqueryRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::CqueryCommandError;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_cquery_command_with_bzlmod_inputs;

pub fn run(argv: Vec<String>) -> i32 {
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => return emit_error(&error.to_string(), "one-shot"),
    };
    let request = match CqueryRequest::parse_at_workspace(&argv, &workspace) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Cquery, argv, Err(error)),
    };
    let environment_value = match super::build::capture_bzlmod_allow_yanked_versions() {
        Ok(value) => value,
        Err(error) => return super::emit_result(CommandKind::Cquery, Vec::new(), Err(error)),
    };
    let environment_policy = match normalize_bzlmod_environment_value(environment_value.as_deref())
    {
        Ok(policy) => policy,
        Err(error) => return super::emit_result(CommandKind::Cquery, Vec::new(), Err(error)),
    };
    if let Some(output_base) = request.output_base.clone() {
        let bzlmod = slug_server_v2::BzlmodRequestInputs::from_normalized_with_registry_urls(
            &request.bzlmod_policy,
            &environment_policy,
            &request.lockfile_mode,
            &request.registry_urls,
        );
        return run_daemon(&output_base, request, bzlmod);
    }
    let output_mode = request.output_mode;
    let accepted = match evaluate_workspace_cquery_command_with_bzlmod_inputs(
        &workspace,
        &request.expression,
        request.include_implicit,
        request.include_tool,
        request.bzlmod_policy,
        environment_policy,
        request.lockfile_mode,
        &request.registry_urls,
        request.configuration_overlay.clone(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => {
            if let Some(stderr) = error.missing_stderr() {
                eprint!("{stderr}");
                return error.exit_code();
            }
            eprint!("{}", cquery_error_json(&error, "one-shot"));
            return error.exit_code();
        }
    };
    let published = accepted
        .project(|terminal| match terminal.as_ref() {
            Ok(evaluation) => match output_mode {
                CqueryOutputMode::Label => {
                    TerminalOutput::new(0, evaluation.label_stdout(), String::new())
                }
                CqueryOutputMode::LabelKind => match evaluation.label_kind_stdout() {
                    Ok(stdout) => TerminalOutput::new(0, stdout, String::new()),
                    Err(error) => terminal_error(&error, "one-shot"),
                },
                CqueryOutputMode::StarlarkLabel => {
                    TerminalOutput::new(0, evaluation.starlark_label_stdout(), String::new())
                }
                CqueryOutputMode::Graph => {
                    TerminalOutput::new(0, evaluation.graph_stdout(), String::new())
                }
            },
            Err(error) => terminal_error(error, "one-shot"),
        })
        .publish();
    let (_terminal, exit_code, stdout, stderr) = published.into_parts();
    print!("{stdout}");
    eprint!("{stderr}");
    exit_code
}

fn run_daemon(
    output_base: &str,
    request: CqueryRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
) -> i32 {
    let output_base = std::path::Path::new(output_base);
    if let Err(error) = std::fs::create_dir_all(output_base) {
        return emit_error(&error.to_string(), "daemon");
    }
    let socket = slug_server_v2::socket_path(output_base);
    if std::os::unix::net::UnixStream::connect(&socket).is_err()
        && let Err(error) = super::build::start_daemon(output_base)
    {
        return emit_error(&error.to_string(), "daemon");
    }
    match slug_server_v2::send_cquery_request(
        &socket,
        &slug_server_v2::CqueryRequest {
            expression: request.expression,
            include_implicit: request.include_implicit,
            include_tool: request.include_tool,
            output: match request.output_mode {
                CqueryOutputMode::Label => slug_server_v2::CqueryOutput::Label,
                CqueryOutputMode::LabelKind => slug_server_v2::CqueryOutput::LabelKind,
                CqueryOutputMode::StarlarkLabel => slug_server_v2::CqueryOutput::StarlarkLabel,
                CqueryOutputMode::Graph => slug_server_v2::CqueryOutput::Graph,
            },
            configuration_overlay: request.configuration_overlay,
            bzlmod,
        },
    ) {
        Ok(response) => {
            print!("{}", response.stdout);
            eprint!("{}", response.stderr);
            response.exit_code
        }
        Err(error) => emit_error(&error.to_string(), "daemon"),
    }
}

fn terminal_error(error: &CqueryCommandError, runtime_mode: &str) -> TerminalOutput {
    match error.missing_stderr() {
        Some(stderr) => TerminalOutput::new(error.exit_code(), String::new(), stderr),
        None => TerminalOutput::new(
            error.exit_code(),
            String::new(),
            cquery_error_json(error, runtime_mode),
        ),
    }
}

fn emit_error(message: &str, runtime_mode: &str) -> i32 {
    eprint!(
        "{{\"error\":\"cquery_runtime_error\",\"command\":\"cquery\",\"message\":\"{}\",\"runtime_mode\":\"{runtime_mode}\"}}\n",
        json_escape(message)
    );
    2
}

fn cquery_error_json(error: &CqueryCommandError, runtime_mode: &str) -> String {
    format!(
        "{{\"error\":\"cquery_runtime_error\",\"command\":\"cquery\",\"message\":\"{}\",\"runtime_mode\":\"{runtime_mode}\"}}\n",
        json_escape(&error.to_string())
    )
}
