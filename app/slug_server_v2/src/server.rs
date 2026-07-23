/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

//! Unix-socket server for the slug daemon. The CLI connects, sends a JSON
//! build request, and receives a JSON build response. The daemon process
//! persists across requests so DICE state survives between builds.

use std::io::BufRead;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;
use slug_identity_v2::TargetPattern;
use slug_reapi_v2::RemoteConfig;

use crate::Daemon;

/// A build request sent by the CLI client over the socket.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildRequest {
    pub targets: Vec<String>,
    pub executor: Option<String>,
    pub default_exec_properties: Vec<(String, String)>,
}

/// A loading query request sent over the same daemon protocol.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub expression: String,
    pub order_output: String,
}

/// Tagged command request. The envelope is deliberately small; query syntax
/// remains raw until the retained runtime parses it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum DaemonRequest {
    Build(BuildRequest),
    Query(QueryRequest),
}

/// Common response envelope for all daemon commands.
#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub invalidated_files: usize,
}

pub type BuildResponse = DaemonResponse;

/// Run the daemon server on the given Unix socket path. Blocks until a
/// `shutdown` request is received or the process is killed.
pub fn serve(socket_path: impl AsRef<Path>, workspace: impl AsRef<Path>) -> anyhow::Result<()> {
    let socket_path = socket_path.as_ref();
    let workspace = workspace.as_ref();
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("binding daemon socket {}", socket_path.display()))?;
    let mut daemon = Daemon::new(workspace)?;
    eprintln!(
        "{{\"daemon\":\"started\",\"socket\":\"{}\"}}",
        socket_path.display()
    );
    for stream in listener.incoming() {
        let mut stream = stream.context("accepting daemon connection")?;
        let line = match read_line(&mut stream) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("{{\"daemon\":\"read_error\",\"message\":\"{error}\"}}");
                continue;
            }
        };
        if line == "shutdown" {
            let _ = std::fs::remove_file(socket_path);
            eprintln!("{{\"daemon\":\"shutdown\"}}");
            return Ok(());
        }
        let response = handle_request(&mut daemon, &line);
        let json = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"exit_code":2,"stdout":"","stderr":"{\"error\":\"daemon_serialize_error\"}","invalidated_files":0}"#.to_string()
        });
        if write!(stream, "{json}\n").is_err() {
            // Client disconnected; ignore.
        }
    }
    Ok(())
}

fn handle_request(daemon: &mut Daemon, request_json: &str) -> DaemonResponse {
    let request: DaemonRequest = match serde_json::from_str(request_json) {
        Ok(req) => req,
        Err(error) => {
            return DaemonResponse {
                exit_code: 2,
                stdout: String::new(),
                stderr: format!(
                    "{{\"error\":\"daemon_parse_error\",\"message\":\"{}\"}}",
                    error
                ),
                invalidated_files: 0,
            };
        }
    };
    match request {
        DaemonRequest::Build(request) => {
            let targets: Vec<TargetPattern> = request
                .targets
                .iter()
                .filter_map(|t| TargetPattern::parse(t).ok())
                .collect();
            let remote = build_remote_config(&request);
            let result = daemon.build(&targets, &remote, &[]);
            DaemonResponse {
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                invalidated_files: result.invalidated_files,
            }
        }
        DaemonRequest::Query(request) => {
            let order = match slug_query_v2::QueryOrder::parse(&request.order_output) {
                Ok(order) => order,
                Err(error) => {
                    return DaemonResponse {
                        exit_code: error.exit_code,
                        stdout: String::new(),
                        stderr: format!("{{\"error\":\"query_error\",\"message\":\"{}\"}}", error),
                        invalidated_files: 0,
                    };
                }
            };
            let result = daemon.query(&request.expression, order);
            DaemonResponse {
                exit_code: result.exit_code,
                stdout: result.stdout,
                stderr: result.stderr,
                invalidated_files: result.invalidated_files,
            }
        }
    }
}

fn build_remote_config(request: &BuildRequest) -> RemoteConfig {
    let mut config = RemoteConfig {
        executor: None,
        cache: None,
        instance_name: None,
        headers: std::collections::BTreeMap::new(),
        timeout_seconds: None,
        retry_attempts: None,
        default_exec_properties: std::collections::BTreeMap::new(),
    };
    if let Some(executor) = &request.executor {
        config.executor = Some(executor.clone());
    }
    for (key, value) in &request.default_exec_properties {
        config
            .default_exec_properties
            .insert(key.clone(), value.clone());
    }
    config
}

fn read_line(stream: &mut UnixStream) -> anyhow::Result<String> {
    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("reading daemon request line")?;
    Ok(line.trim().to_string())
}

/// Send a build request to a running daemon and return the response.
pub fn send_build_request(
    socket_path: &Path,
    request: &BuildRequest,
) -> anyhow::Result<BuildResponse> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connecting to daemon socket {}", socket_path.display()))?;
    let json = serde_json::to_string(&DaemonRequest::Build(BuildRequest {
        targets: request.targets.clone(),
        executor: request.executor.clone(),
        default_exec_properties: request.default_exec_properties.clone(),
    }))
    .context("serializing build request for daemon")?;
    write!(stream, "{json}\n").context("sending build request to daemon")?;
    let line = read_line(&mut stream)?;
    let response: BuildResponse =
        serde_json::from_str(&line).context("deserializing daemon build response")?;
    Ok(response)
}

/// Send a raw query request to a running daemon.
pub fn send_query_request(
    socket_path: &Path,
    request: &QueryRequest,
) -> anyhow::Result<DaemonResponse> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connecting to daemon socket {}", socket_path.display()))?;
    let json = serde_json::to_string(&DaemonRequest::Query(QueryRequest {
        expression: request.expression.clone(),
        order_output: request.order_output.clone(),
    }))
    .context("serializing query request for daemon")?;
    write!(stream, "{json}\n").context("sending query request to daemon")?;
    let line = read_line(&mut stream)?;
    serde_json::from_str(&line).context("deserializing daemon query response")
}

/// Send a shutdown command to a running daemon.
pub fn send_shutdown(socket_path: &Path) -> anyhow::Result<()> {
    let mut stream = UnixStream::connect(socket_path)
        .with_context(|| format!("connecting to daemon socket {}", socket_path.display()))?;
    write!(stream, "shutdown\n").context("sending shutdown to daemon")?;
    Ok(())
}

/// The canonical socket path for a given output base directory.
pub fn socket_path(output_base: &Path) -> PathBuf {
    output_base.join("slugd.sock")
}

/// The canonical PID file path for a given output base directory.
pub fn pid_path(output_base: &Path) -> PathBuf {
    output_base.join("slugd.pid")
}
