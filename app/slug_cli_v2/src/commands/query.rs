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
use slug_commands_v2::QueryOutputFormat;
use slug_commands_v2::normalize_bzlmod_environment_value;
use slug_commands_v2::query::QueryRequest;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::QueryOutputCompletion;
use slug_core_v2::runtime::evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion;

pub fn run(argv: Vec<String>) -> i32 {
    let request = match QueryRequest::parse(&argv) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Query, argv, Err(error)),
    };
    let environment_value = match super::build::capture_bzlmod_allow_yanked_versions() {
        Ok(value) => value,
        Err(error) => return super::emit_result(CommandKind::Query, argv, Err(error)),
    };
    let environment_policy = match normalize_bzlmod_environment_value(environment_value.as_deref())
    {
        Ok(policy) => policy,
        Err(error) => return super::emit_result(CommandKind::Query, argv, Err(error)),
    };
    if let Some(output_base) = super::build::extract_output_base(&argv) {
        let bzlmod = slug_server_v2::BzlmodRequestInputs::from_normalized_with_registry_urls(
            &request.bzlmod_policy,
            &environment_policy,
            &request.lockfile_mode,
            &request.registry_urls,
        );
        return run_daemon_query(&output_base, request, bzlmod);
    }
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => return emit_error(7, &error.to_string(), "one-shot"),
    };
    let completion = if request.output == QueryOutputFormat::LabelKind {
        QueryOutputCompletion::LabelKind
    } else {
        QueryOutputCompletion::Standard
    };
    match evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion(
        &workspace,
        &request.expression,
        request.order,
        request.policy,
        request.bzlmod_policy,
        environment_policy,
        request.lockfile_mode,
        &request.registry_urls,
        completion,
    ) {
        Ok(output) => {
            let stdout = match request.output {
                QueryOutputFormat::Text => output.stdout(),
                QueryOutputFormat::Graph => {
                    output.graph_stdout(request.graph_factored, request.order.is_full())
                }
                QueryOutputFormat::LabelKind => output.label_kind_stdout(),
                QueryOutputFormat::Package => output.package_stdout(),
                _ => unreachable!("query parser rejects deferred output formats"),
            };
            print!("{stdout}");
            0
        }
        Err(error) => {
            let mut message = error.to_string();
            if error.needs_evaluation_context() {
                message.push_str("\nEvaluation of query");
            }
            emit_error(error.exit_code, &message, "one-shot")
        }
    }
}

fn run_daemon_query(
    output_base: &str,
    request: QueryRequest,
    bzlmod: slug_server_v2::BzlmodRequestInputs,
) -> i32 {
    let output_base = std::path::Path::new(output_base);
    if let Err(error) = std::fs::create_dir_all(output_base) {
        return emit_error(7, &error.to_string(), "daemon");
    }
    let socket = slug_server_v2::socket_path(output_base);
    if std::os::unix::net::UnixStream::connect(&socket).is_err() {
        if let Err(error) = super::build::start_daemon(output_base) {
            return emit_error(7, &error.to_string(), "daemon");
        }
    }
    let request = slug_server_v2::QueryRequest {
        expression: request.expression,
        order_output: request.order.to_string(),
        output: request.output.to_string(),
        graph_factored: request.graph_factored,
        strict_test_suite: request.policy.strict_test_suite,
        bzlmod,
    };
    match slug_server_v2::send_query_request(&socket, &request) {
        Ok(response) => {
            if !response.stdout.is_empty() {
                print!("{}", response.stdout);
            }
            if !response.stderr.is_empty() {
                eprintln!("{}", response.stderr);
            }
            response.exit_code
        }
        Err(error) => emit_error(7, &error.to_string(), "daemon"),
    }
}

fn emit_error(exit_code: i32, message: &str, runtime_mode: &str) -> i32 {
    eprintln!(
        "{{\"error\":\"query_error\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"{}\"}}",
        json_escape(message),
        runtime_mode,
    );
    exit_code
}
