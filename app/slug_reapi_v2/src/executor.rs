/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use prost::Message;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;

use crate::action_cache::ActionResult;
use crate::cas::GeneratedOutput;
use crate::command::ReapiActionIdentity;
use crate::command::ReapiCommand;
use crate::command::digest_to_proto;
use crate::config::RemoteConfig;
use crate::digest::ReapiDigest;
use crate::evidence::ExecutionEvidence;
use crate::input_tree::ReapiBlob;
use crate::input_tree::ReapiInputTree;
use crate::proto;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RemoteExecutionResult {
    pub action_digest: ReapiDigest,
    pub result: ActionResult,
    pub output_blobs: BTreeMap<String, Vec<u8>>,
    pub evidence: ExecutionEvidence,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RemoteExecutionError {
    MissingExecutor,
    InputTree(String),
    Command(String),
    Transport(String),
    Protocol(String),
    MissingBlobData {
        digest: ReapiDigest,
    },
    OutputPath {
        path: String,
    },
    OutputDigest {
        path: String,
        expected: ReapiDigest,
        actual: ReapiDigest,
    },
    Io {
        path: String,
        error: String,
    },
}

impl std::fmt::Display for RemoteExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingExecutor => write!(f, "--remote_executor is required for REAPI execution"),
            Self::InputTree(error) => write!(f, "invalid REAPI input tree: {error}"),
            Self::Command(error) => write!(f, "cannot lower action for REAPI: {error}"),
            Self::Transport(error) => write!(f, "REAPI transport error: {error}"),
            Self::Protocol(error) => write!(f, "REAPI protocol error: {error}"),
            Self::MissingBlobData { digest } => {
                write!(
                    f,
                    "REAPI CAS is missing input {digest}, but Slug has no bytes to upload"
                )
            }
            Self::OutputPath { path } => write!(f, "unsafe REAPI output path: {path}"),
            Self::OutputDigest {
                path,
                expected,
                actual,
            } => write!(
                f,
                "REAPI output digest mismatch for {path}: expected {expected}, got {actual}"
            ),
            Self::Io { path, error } => write!(f, "cannot materialize {path}: {error}"),
        }
    }
}

impl std::error::Error for RemoteExecutionError {}

pub async fn execute_action(
    config: &RemoteConfig,
    action: &ActionSpec,
) -> Result<RemoteExecutionResult, RemoteExecutionError> {
    let executor = config
        .executor
        .as_deref()
        .ok_or(RemoteExecutionError::MissingExecutor)?;
    let endpoint = tonic_endpoint(executor)?;
    let mut command = ReapiCommand::for_execution(action).map_err(RemoteExecutionError::Command)?;
    for (name, value) in &config.default_exec_properties {
        command
            .platform_properties
            .entry(name.clone())
            .or_insert_with(|| value.clone());
    }
    let input_tree = ReapiInputTree::from_action(action)
        .map_err(|error| RemoteExecutionError::InputTree(error.to_string()))?;
    let identity = ReapiActionIdentity::new(
        &command,
        input_tree.root_digest().clone(),
        config.timeout_seconds,
    );
    let owned_blobs = owned_blobs(&command, &identity, &input_tree);

    let mut cas =
        proto::content_addressable_storage_client::ContentAddressableStorageClient::connect(
            endpoint.clone(),
        )
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
    let requested = required_digests(&input_tree, &owned_blobs);
    let missing = cas
        .find_missing_blobs(proto::FindMissingBlobsRequest {
            instance_name: config.instance_name.clone().unwrap_or_default(),
            blob_digests: requested.iter().map(digest_to_proto).collect(),
        })
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
        .into_inner()
        .missing_blob_digests
        .into_iter()
        .map(|digest| digest_from_proto(&digest))
        .collect::<Result<BTreeSet<_>, _>>()?;

    let blob_by_digest = owned_blobs
        .iter()
        .map(|blob| (blob.digest().clone(), blob))
        .collect::<BTreeMap<_, _>>();
    for digest in &missing {
        if !blob_by_digest.contains_key(digest) {
            return Err(RemoteExecutionError::MissingBlobData {
                digest: digest.clone(),
            });
        }
    }
    let uploads = missing
        .iter()
        .filter_map(|digest| blob_by_digest.get(digest).copied())
        .collect::<Vec<_>>();
    if !uploads.is_empty() {
        let response = cas
            .batch_update_blobs(proto::BatchUpdateBlobsRequest {
                instance_name: config.instance_name.clone().unwrap_or_default(),
                requests: uploads
                    .iter()
                    .map(|blob| proto::batch_update_blobs_request::Request {
                        digest: Some(digest_to_proto(blob.digest())),
                        data: blob.data().to_vec(),
                    })
                    .collect(),
            })
            .await
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
            .into_inner();
        for response in response.responses {
            if let Some(status) = response.status
                && status.code != 0
            {
                return Err(RemoteExecutionError::Protocol(status.message));
            }
        }
    }

    let mut execution = proto::execution_client::ExecutionClient::connect(endpoint)
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
    let mut operations = execution
        .execute(proto::ExecuteRequest {
            instance_name: config.instance_name.clone().unwrap_or_default(),
            skip_cache_lookup: false,
            action_digest: Some(digest_to_proto(&identity.action_digest)),
            inline_stdout: true,
            inline_stderr: true,
            inline_output_files: action
                .outputs()
                .iter()
                .filter(|output| output.kind() == ActionOutputKind::File)
                .map(|output| output.path().to_owned())
                .collect(),
        })
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
        .into_inner();
    let operation = loop {
        let operation = operations
            .message()
            .await
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
            .ok_or_else(|| {
                RemoteExecutionError::Protocol(
                    "Execution stream ended before completion".to_owned(),
                )
            })?;
        if operation.done {
            break operation;
        }
    };
    let response = decode_execute_response(operation)?;
    if let Some(status) = &response.status
        && status.code != 0
    {
        return Err(RemoteExecutionError::Protocol(status.message.clone()));
    }
    let result = response.result.ok_or_else(|| {
        RemoteExecutionError::Protocol("completed Execute response omitted ActionResult".to_owned())
    })?;
    if result.exit_code != 0 {
        return Err(RemoteExecutionError::Protocol(format!(
            "remote action exited {}: {}",
            result.exit_code,
            String::from_utf8_lossy(&result.stderr_raw)
        )));
    }

    let output_blobs = fetch_outputs(&mut cas, config, &result).await?;
    let outputs = result
        .output_files
        .iter()
        .map(|file| {
            let digest = file
                .digest
                .as_ref()
                .ok_or_else(|| {
                    RemoteExecutionError::Protocol(format!("output {} has no digest", file.path))
                })
                .and_then(digest_from_proto)?;
            Ok(GeneratedOutput::new(file.path.clone(), digest))
        })
        .collect::<Result<Vec<_>, RemoteExecutionError>>()?;
    let mut action_result = ActionResult::new(outputs);
    if let Some(digest) = result.stdout_digest.as_ref() {
        action_result = action_result.with_stdout_digest(digest_from_proto(digest)?);
    }
    if let Some(digest) = result.stderr_digest.as_ref() {
        action_result = action_result.with_stderr_digest(digest_from_proto(digest)?);
    }
    let mut evidence = ExecutionEvidence::reapi("nativelink").record_action();
    evidence = if response.cached_result {
        evidence.record_ac_hit()
    } else {
        evidence.record_ac_miss()
    };
    for blob in uploads {
        evidence = evidence.record_upload(blob.digest().clone());
    }
    for output in action_result.output_files() {
        evidence = evidence.record_materialized_output(output.digest().clone());
    }
    Ok(RemoteExecutionResult {
        action_digest: identity.action_digest,
        result: action_result,
        output_blobs,
        evidence,
    })
}

pub fn materialize_outputs(
    output_root: &Path,
    execution: &RemoteExecutionResult,
) -> Result<(), RemoteExecutionError> {
    for output in execution.result.output_files() {
        let path = safe_output_path(output_root, output.path())?;
        let data = execution.output_blobs.get(output.path()).ok_or_else(|| {
            RemoteExecutionError::Protocol(format!("missing downloaded output {}", output.path()))
        })?;
        let actual = ReapiDigest::of_bytes(data);
        if actual != *output.digest() {
            return Err(RemoteExecutionError::OutputDigest {
                path: output.path().to_owned(),
                expected: output.digest().clone(),
                actual,
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| RemoteExecutionError::Io {
                path: parent.display().to_string(),
                error: error.to_string(),
            })?;
        }
        // Remove any stale output from a previous build. The previous build
        // may have left the file read-only (mode 0o555), which would cause
        // the write below to fail with EACCES on a same-daemon rebuild.
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, data).map_err(|error| RemoteExecutionError::Io {
            path: path.display().to_string(),
            error: error.to_string(),
        })?;
        // Bazel marks action outputs read-only with mode 0555; match that so
        // manifest comparison against the Bazel oracle is byte-for-byte.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, PermissionsExt::from_mode(0o555)).map_err(|error| {
                RemoteExecutionError::Io {
                    path: path.display().to_string(),
                    error: error.to_string(),
                }
            })?;
        }
    }
    Ok(())
}

fn tonic_endpoint(value: &str) -> Result<String, RemoteExecutionError> {
    let endpoint = value
        .strip_prefix("grpc://")
        .map(|rest| format!("http://{rest}"))
        .or_else(|| {
            value
                .strip_prefix("grpcs://")
                .map(|rest| format!("https://{rest}"))
        })
        .unwrap_or_else(|| value.to_owned());
    tonic::transport::Endpoint::from_shared(endpoint.clone())
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
    Ok(endpoint)
}

fn owned_blobs(
    command: &ReapiCommand,
    identity: &ReapiActionIdentity,
    input_tree: &ReapiInputTree,
) -> Vec<ReapiBlob> {
    let mut blobs = input_tree.directory_blobs().to_vec();
    blobs.extend_from_slice(input_tree.inline_blobs());
    blobs.push(ReapiBlob::from_bytes(command.serialized()));
    blobs.push(ReapiBlob::from_bytes(identity.action_bytes().to_vec()));
    blobs
}

fn required_digests(input_tree: &ReapiInputTree, owned: &[ReapiBlob]) -> BTreeSet<ReapiDigest> {
    let mut digests = owned
        .iter()
        .map(|blob| blob.digest().clone())
        .collect::<BTreeSet<_>>();
    digests.extend(
        input_tree
            .entries()
            .iter()
            .map(|entry| entry.digest().clone()),
    );
    digests
}

fn digest_from_proto(digest: &proto::Digest) -> Result<ReapiDigest, RemoteExecutionError> {
    let size_bytes: u64 = digest.size_bytes.try_into().map_err(|_| {
        RemoteExecutionError::Protocol(format!("negative digest size for {}", digest.hash))
    })?;
    ReapiDigest::new(digest.hash.clone(), size_bytes).map_err(RemoteExecutionError::Protocol)
}

fn decode_execute_response(
    operation: proto::google::longrunning::Operation,
) -> Result<proto::ExecuteResponse, RemoteExecutionError> {
    match operation.result {
        Some(proto::google::longrunning::operation::Result::Error(status)) => {
            Err(RemoteExecutionError::Protocol(status.message))
        }
        Some(proto::google::longrunning::operation::Result::Response(response)) => {
            proto::ExecuteResponse::decode(response.value.as_slice())
                .map_err(|error| RemoteExecutionError::Protocol(error.to_string()))
        }
        None => Err(RemoteExecutionError::Protocol(
            "completed Execute operation has no response".to_owned(),
        )),
    }
}

async fn fetch_outputs(
    cas: &mut proto::content_addressable_storage_client::ContentAddressableStorageClient<
        tonic::transport::Channel,
    >,
    config: &RemoteConfig,
    result: &proto::ActionResult,
) -> Result<BTreeMap<String, Vec<u8>>, RemoteExecutionError> {
    let mut output_blobs = BTreeMap::new();
    let mut missing = Vec::new();
    for output in &result.output_files {
        let digest = output.digest.as_ref().ok_or_else(|| {
            RemoteExecutionError::Protocol(format!("output {} has no digest", output.path))
        })?;
        if !output.contents.is_empty() {
            output_blobs.insert(output.path.clone(), output.contents.clone());
        } else {
            missing.push(digest.clone());
        }
    }
    if missing.is_empty() {
        return Ok(output_blobs);
    }
    let response = cas
        .batch_read_blobs(proto::BatchReadBlobsRequest {
            instance_name: config.instance_name.clone().unwrap_or_default(),
            digests: missing,
        })
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
        .into_inner();
    let by_digest = result
        .output_files
        .iter()
        .filter_map(|output| {
            output
                .digest
                .as_ref()
                .map(|digest| (digest.hash.clone(), output.path.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for response in response.responses {
        if let Some(status) = response.status
            && status.code != 0
        {
            return Err(RemoteExecutionError::Protocol(status.message));
        }
        let digest = response.digest.ok_or_else(|| {
            RemoteExecutionError::Protocol("BatchReadBlobs response has no digest".to_owned())
        })?;
        let path = by_digest.get(&digest.hash).ok_or_else(|| {
            RemoteExecutionError::Protocol(format!("unexpected output digest {}", digest.hash))
        })?;
        output_blobs.insert(path.clone(), response.data);
    }
    Ok(output_blobs)
}

fn safe_output_path(
    root: &Path,
    relative: &str,
) -> Result<std::path::PathBuf, RemoteExecutionError> {
    if relative.is_empty()
        || relative.starts_with('/')
        || relative.ends_with('/')
        || relative
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
    {
        return Err(RemoteExecutionError::OutputPath {
            path: relative.to_owned(),
        });
    }
    Ok(root.join(relative))
}
