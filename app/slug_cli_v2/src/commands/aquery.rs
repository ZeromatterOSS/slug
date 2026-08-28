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
use slug_commands_v2::aquery::AqueryRequest;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::evaluate_workspace_build_command_with_bzlmod_inputs;
use slug_core_v2::runtime::format_file_write_aquery_text_output_for_scope;
use slug_server_v2::AqueryRequest as DaemonAqueryRequest;

pub fn run(argv: Vec<String>) -> i32 {
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => return emit_error(2, &error.to_string(), "one-shot"),
    };
    let request = match AqueryRequest::parse_at_workspace(&argv, &workspace) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Aquery, argv, Err(error)),
    };
    let environment_value = match super::build::capture_bzlmod_allow_yanked_versions() {
        Ok(value) => value,
        Err(error) => return super::emit_result(CommandKind::Aquery, Vec::new(), Err(error)),
    };
    let environment_policy = match normalize_bzlmod_environment_value(environment_value.as_deref())
    {
        Ok(policy) => policy,
        Err(error) => return super::emit_result(CommandKind::Aquery, Vec::new(), Err(error)),
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

    let accepted = match evaluate_workspace_build_command_with_bzlmod_inputs(
        &workspace,
        std::slice::from_ref(&request.target),
        request.bzlmod_policy,
        environment_policy,
        request.lockfile_mode,
        &request.registry_urls,
        Default::default(),
    ) {
        Ok(accepted) => accepted,
        Err(error) => {
            let (_, exit_code) = error.terminal_error();
            return emit_error(exit_code, &error.to_string(), "one-shot");
        }
    };
    let published = accepted
        .project(|terminal| match terminal.as_ref() {
            Ok(evaluation) => {
                match format_file_write_aquery_text_output_for_scope(evaluation, request.scope) {
                    Ok(stdout) => TerminalOutput::new(0, stdout, String::new()),
                    Err(error) => {
                        TerminalOutput::new(2, String::new(), error_json(error, "one-shot"))
                    }
                }
            }
            Err(error) => {
                let (_, exit_code) = error.terminal_error();
                TerminalOutput::new(
                    exit_code,
                    String::new(),
                    error_json(&error.to_string(), "one-shot"),
                )
            }
        })
        .publish();
    let (_terminal, exit_code, stdout, stderr) = published.into_parts();
    print!("{stdout}");
    eprint!("{stderr}");
    exit_code
}

fn run_daemon(
    output_base: &str,
    request: AqueryRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
) -> i32 {
    let output_base = std::path::Path::new(output_base);
    if let Err(error) = std::fs::create_dir_all(output_base) {
        return emit_error(2, &error.to_string(), "daemon");
    }
    let socket = slug_server_v2::socket_path(output_base);
    if std::os::unix::net::UnixStream::connect(&socket).is_err()
        && let Err(error) = super::build::start_daemon(output_base)
    {
        return emit_error(2, &error.to_string(), "daemon");
    }
    match slug_server_v2::send_aquery_request(
        &socket,
        &DaemonAqueryRequest {
            expression: request.expression,
            bzlmod,
        },
    ) {
        Ok(response) => {
            print!("{}", response.stdout);
            eprint!("{}", response.stderr);
            response.exit_code
        }
        Err(error) => emit_error(2, &error.to_string(), "daemon"),
    }
}

fn emit_error(exit_code: i32, message: &str, runtime_mode: &str) -> i32 {
    eprint!("{}", error_json(message, runtime_mode));
    exit_code
}

fn error_json(message: &str, runtime_mode: &str) -> String {
    format!(
        "{{\"error\":\"aquery_runtime_error\",\"command\":\"aquery\",\"message\":\"{}\",\"runtime_mode\":\"{runtime_mode}\"}}\n",
        json_escape(message)
    )
}
