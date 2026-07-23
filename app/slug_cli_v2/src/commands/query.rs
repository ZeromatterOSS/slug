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
use slug_commands_v2::query::QueryRequest;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::evaluate_workspace_query;

pub fn run(argv: Vec<String>) -> i32 {
    if let Some(output_base) = super::build::extract_output_base(&argv) {
        return run_daemon_query(&argv, &output_base);
    }
    let request = match QueryRequest::parse(&argv) {
        Ok(request) => request,
        Err(error) => return super::emit_result(CommandKind::Query, argv, Err(error)),
    };
    let workspace = match std::env::current_dir() {
        Ok(workspace) => workspace,
        Err(error) => return emit_error(7, &error.to_string(), "one-shot"),
    };
    match evaluate_workspace_query(&workspace, &request.expression, request.order) {
        Ok(output) => {
            print!("{}", output.stdout());
            0
        }
        Err(error) => emit_error(error.exit_code, &error.to_string(), "one-shot"),
    }
}

fn run_daemon_query(argv: &[String], output_base: &str) -> i32 {
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
    let request = match QueryRequest::parse(argv) {
        Ok(request) => request,
        Err(error) => {
            return super::emit_result(CommandKind::Query, argv.to_vec(), Err(error));
        }
    };
    let request = slug_server_v2::QueryRequest {
        expression: request.expression,
        order_output: request.order.to_string(),
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
