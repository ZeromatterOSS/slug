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
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_core_v2::runtime::ResolvedFileWriteSemanticView;

use crate::action_cache::ActionResult;
use crate::cas::GeneratedOutput;
use crate::command::ReapiActionIdentity;
use crate::command::ReapiCommand;
use crate::command::digest_to_proto;
use crate::config::RemoteConfig;
use crate::digest::ReapiDigest;
use crate::evidence::ExecutionEvidence;
use crate::input_tree::InputTreeEntryKind;
use crate::input_tree::ReapiBlob;
use crate::input_tree::ReapiInputTree;
use crate::proto;

const FILE_WRITE_CONTENT_PATH: &str = "__slug_filewrite__/content";
const FILE_WRITE_RESERVED_SEGMENT: &str = "__slug_filewrite__";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FileWriteReapiPlan {
    command: ReapiCommand,
    input_tree: ReapiInputTree,
    identity: ReapiActionIdentity,
}

impl FileWriteReapiPlan {
    pub fn from_resolved(
        view: &ResolvedFileWriteSemanticView<'_>,
        remote_defaults: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let selected_properties = view
            .platform_fact()
            .exec_properties
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>();
        let platform_properties =
            effective_platform_properties(selected_properties, remote_defaults);
        Self::from_action(view.action().spec(), platform_properties)
    }

    fn from_action(
        action: &ActionSpec,
        platform_properties: BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let (content, is_executable) = match action.kind() {
            ActionKind::Write {
                content,
                is_executable,
            } => (content.as_bytes(), *is_executable),
            _ => return Err("FileWrite REAPI plan requires a FileWrite action".to_owned()),
        };
        let [output] = action.outputs() else {
            return Err("FileWrite REAPI plan requires exactly one output".to_owned());
        };
        if output.kind() != ActionOutputKind::File {
            return Err("FileWrite REAPI plan requires one file output".to_owned());
        }
        if output.path().split('/').next() == Some(FILE_WRITE_RESERVED_SEGMENT) {
            return Err(format!(
                "FileWrite output uses reserved REAPI input namespace: {}",
                output.path()
            ));
        }
        let input_tree = ReapiInputTree::from_inline_file(
            FILE_WRITE_CONTENT_PATH,
            content,
            InputTreeEntryKind::FileWriteContent,
        )
        .map_err(|error| error.to_string())?;
        let command = ReapiCommand::file_write(output.path(), is_executable, platform_properties);
        let identity = ReapiActionIdentity::new(&command, input_tree.root_digest().clone(), None);
        Ok(Self {
            command,
            input_tree,
            identity,
        })
    }

    pub fn command(&self) -> &ReapiCommand {
        &self.command
    }

    pub fn input_tree(&self) -> &ReapiInputTree {
        &self.input_tree
    }

    pub fn identity(&self) -> &ReapiActionIdentity {
        &self.identity
    }
}

fn effective_platform_properties(
    selected: BTreeMap<String, String>,
    remote_defaults: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    if selected.is_empty() {
        remote_defaults.clone()
    } else {
        selected
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RemoteExecutionResult {
    pub action_digest: ReapiDigest,
    pub platform_properties: BTreeMap<String, String>,
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
    let inline_output_files = action
        .outputs()
        .iter()
        .filter(|output| output.kind() == ActionOutputKind::File)
        .map(|output| output.path().to_owned())
        .collect();
    execute_prepared(config, command, input_tree, identity, inline_output_files).await
}

pub async fn execute_file_write(
    config: &RemoteConfig,
    view: &ResolvedFileWriteSemanticView<'_>,
) -> Result<RemoteExecutionResult, RemoteExecutionError> {
    let plan = FileWriteReapiPlan::from_resolved(view, &config.default_exec_properties)
        .map_err(RemoteExecutionError::Command)?;
    let inline_output_files = plan.command.output_files.clone();
    execute_prepared(
        config,
        plan.command,
        plan.input_tree,
        plan.identity,
        inline_output_files,
    )
    .await
}

async fn execute_prepared(
    config: &RemoteConfig,
    command: ReapiCommand,
    input_tree: ReapiInputTree,
    identity: ReapiActionIdentity,
    inline_output_files: Vec<String>,
) -> Result<RemoteExecutionResult, RemoteExecutionError> {
    let executor = config
        .executor
        .as_deref()
        .ok_or(RemoteExecutionError::MissingExecutor)?;
    let endpoint = tonic_endpoint(executor)?;
    let platform_properties = command.platform_properties.clone();
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

    let mut execution = proto::execution_client::ExecutionClient::connect(endpoint.clone())
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;

    let mut ac_client = proto::action_cache_client::ActionCacheClient::connect(endpoint)
        .await
        .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
    let ac_result = ac_client
        .get_action_result(proto::GetActionResultRequest {
            instance_name: config.instance_name.clone().unwrap_or_default(),
            action_digest: Some(digest_to_proto(&identity.action_digest)),
            inline_stdout: true,
            inline_stderr: true,
            inline_output_files: inline_output_files.clone(),
        })
        .await;
    let (result, cached_result) = match ac_result {
        Ok(cached) => (cached.into_inner(), true),
        Err(_) => {
            // AC miss: proceed to Execute.
            let result =
                execute_through_server(&mut execution, config, &identity, inline_output_files)
                    .await?;
            (result.0, result.1)
        }
    };
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
    evidence = if cached_result {
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
        platform_properties,
        evidence,
    })
}

/// Execute an action through the Execution server (AC miss path). Returns the
/// ActionResult and whether the server reported a cached result.
async fn execute_through_server(
    execution: &mut proto::execution_client::ExecutionClient<tonic::transport::Channel>,
    config: &RemoteConfig,
    identity: &ReapiActionIdentity,
    inline_output_files: Vec<String>,
) -> Result<(proto::ActionResult, bool), RemoteExecutionError> {
    let mut operations = execution
        .execute(proto::ExecuteRequest {
            instance_name: config.instance_name.clone().unwrap_or_default(),
            skip_cache_lookup: false,
            action_digest: Some(digest_to_proto(&identity.action_digest)),
            inline_stdout: true,
            inline_stderr: true,
            inline_output_files,
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
    Ok((result, response.cached_result))
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

pub fn verify_materialized_run_executable(
    workspace: &Path,
    view: &slug_core_v2::runtime::ResolvedRunSemanticView<'_>,
) -> Result<std::path::PathBuf, RemoteExecutionError> {
    let configuration = view
        .file_write()
        .action()
        .owner()
        .configuration()
        .slug_configuration()
        .ok_or_else(|| {
            RemoteExecutionError::Command(
                "run FileWrite owner has an opaque configuration".to_owned(),
            )
        })?;
    let root = slug_core_v2::runtime::configured_output_root(workspace, configuration);
    if !root.is_absolute() {
        return Err(RemoteExecutionError::OutputPath {
            path: root.display().to_string(),
        });
    }
    let root_metadata =
        std::fs::symlink_metadata(&root).map_err(|error| RemoteExecutionError::Io {
            path: root.display().to_string(),
            error: error.to_string(),
        })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(RemoteExecutionError::OutputPath {
            path: root.display().to_string(),
        });
    }
    let path = safe_output_path(&root, view.executable())?;
    let segments = view.executable().split('/').collect::<Vec<_>>();
    let mut current = root;
    for (index, segment) in segments.iter().enumerate() {
        current.push(segment);
        let metadata =
            std::fs::symlink_metadata(&current).map_err(|error| RemoteExecutionError::Io {
                path: current.display().to_string(),
                error: error.to_string(),
            })?;
        if metadata.file_type().is_symlink() {
            return Err(RemoteExecutionError::OutputPath {
                path: current.display().to_string(),
            });
        }
        let final_entry = index + 1 == segments.len();
        if (!final_entry && !metadata.is_dir()) || (final_entry && !metadata.is_file()) {
            return Err(RemoteExecutionError::OutputPath {
                path: current.display().to_string(),
            });
        }
        #[cfg(unix)]
        if final_entry {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(RemoteExecutionError::OutputPath {
                    path: current.display().to_string(),
                });
            }
        }
    }
    Ok(path)
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

#[cfg(test)]
mod tests {
    use prost::Message;
    use slug_build_api_v2::ActionOutput;

    use super::*;

    fn write_action(content: &str, executable: bool, output: &str) -> ActionSpec {
        ActionSpec::new(
            ActionKind::Write {
                content: content.to_owned(),
                is_executable: executable,
            },
            "FileWrite",
            vec![ActionOutput::new(output, ActionOutputKind::File)],
        )
    }

    fn plan(
        content: &str,
        executable: bool,
        output: &str,
        properties: &[(&str, &str)],
    ) -> FileWriteReapiPlan {
        FileWriteReapiPlan::from_action(
            &write_action(content, executable, output),
            properties
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn file_write_plan_owns_canonical_nul_safe_reapi_objects() {
        let content = "quote'\n\0tail";
        let plan = plan(
            content,
            false,
            "pkg/out.txt",
            &[("container-image", "selected:v1")],
        );

        assert_eq!(
            plan.command.argv,
            [
                "sh",
                "-c",
                "cp -- \"$1\" \"$2\" && chmod 0444 -- \"$2\"",
                "slug-filewrite",
                FILE_WRITE_CONTENT_PATH,
                "pkg/out.txt",
            ]
        );
        assert!(plan.command.env.is_empty());
        assert!(!plan.command.argv.iter().any(|argument| argument == content));
        assert_eq!(plan.input_tree.entries().len(), 1);
        assert_eq!(
            plan.input_tree.entries()[0].kind(),
            InputTreeEntryKind::FileWriteContent
        );
        assert_eq!(plan.input_tree.inline_blobs()[0].data(), content.as_bytes());

        let command = proto::Command::decode(plan.command.serialized().as_slice()).unwrap();
        assert_eq!(command.output_files, ["pkg/out.txt"]);
        assert_eq!(command.output_paths, ["pkg/out.txt"]);
        assert!(command.output_directories.is_empty());
        assert_eq!(
            command.platform.unwrap().properties[0],
            proto::platform::Property {
                name: "container-image".to_owned(),
                value: "selected:v1".to_owned(),
            }
        );

        let directories = plan.input_tree.directory_blobs();
        let content_directory = proto::Directory::decode(directories[0].data()).unwrap();
        assert_eq!(content_directory.files[0].name, "content");
        assert!(!content_directory.files[0].is_executable);
        let root = proto::Directory::decode(directories.last().unwrap().data()).unwrap();
        assert_eq!(root.directories[0].name, FILE_WRITE_RESERVED_SEGMENT);
        assert_eq!(
            directories.last().unwrap().digest(),
            plan.input_tree.root_digest()
        );

        let action = proto::Action::decode(plan.identity.action_bytes()).unwrap();
        assert!(action.timeout.is_none());
        assert!(!action.do_not_cache);
        assert!(action.salt.is_empty());
        assert_eq!(
            action.command_digest.unwrap().hash,
            plan.identity.command_digest.hash()
        );
        assert_eq!(
            action.input_root_digest.unwrap().hash,
            plan.identity.input_root_digest.hash()
        );
    }

    #[test]
    fn file_write_plan_identity_discriminates_and_restores() {
        let baseline = plan("a", false, "pkg/out.txt", &[("cpu", "x86_64")]);
        for changed in [
            plan("b", false, "pkg/out.txt", &[("cpu", "x86_64")]),
            plan("a", true, "pkg/out.txt", &[("cpu", "x86_64")]),
            plan("a", false, "pkg/other.txt", &[("cpu", "x86_64")]),
            plan("a", false, "pkg/out.txt", &[("cpu", "arm64")]),
        ] {
            assert_ne!(
                changed.identity.action_digest,
                baseline.identity.action_digest
            );
        }
        let restored = plan("a", false, "pkg/out.txt", &[("cpu", "x86_64")]);
        assert_eq!(restored, baseline);
        assert!(restored.command.argv[2].contains("chmod 0444"));
        assert!(plan("a", true, "pkg/out.txt", &[]).command.argv[2].contains("chmod 0555"));
    }

    #[test]
    fn selected_platform_properties_replace_defaults_as_a_whole() {
        let defaults = BTreeMap::from([
            ("container-image".to_owned(), "default:v1".to_owned()),
            ("default-only".to_owned(), "ignored".to_owned()),
        ]);
        let selected = BTreeMap::from([("container-image".to_owned(), "selected:v1".to_owned())]);
        assert_eq!(
            effective_platform_properties(selected.clone(), &defaults),
            selected
        );
        assert_eq!(
            effective_platform_properties(BTreeMap::new(), &defaults),
            defaults
        );
    }

    #[test]
    fn reserved_namespace_and_raw_file_write_fail_closed() {
        let action = write_action("content", false, "__slug_filewrite__/out.txt");
        assert!(
            FileWriteReapiPlan::from_action(&action, BTreeMap::new())
                .unwrap_err()
                .contains("reserved REAPI input namespace")
        );
        assert_eq!(
            ReapiCommand::for_execution(&write_action("content", false, "out.txt")).unwrap_err(),
            "raw FileWrite REAPI lowering is forbidden"
        );
    }

    #[test]
    fn transport_timeout_cannot_salt_file_write_action() {
        let first = RemoteConfig {
            executor: None,
            cache: None,
            instance_name: None,
            headers: BTreeMap::new(),
            timeout_seconds: Some(1),
            retry_attempts: None,
            default_exec_properties: BTreeMap::new(),
        };
        let second = RemoteConfig {
            timeout_seconds: Some(99),
            ..first.clone()
        };
        let action = write_action("content", false, "out.txt");
        let first_plan =
            FileWriteReapiPlan::from_action(&action, first.default_exec_properties).unwrap();
        let second_plan =
            FileWriteReapiPlan::from_action(&action, second.default_exec_properties).unwrap();
        assert_eq!(
            first_plan.identity.action_bytes(),
            second_plan.identity.action_bytes()
        );
        assert_eq!(
            first_plan.identity.action_digest,
            second_plan.identity.action_digest
        );
    }

    #[tokio::test]
    #[ignore = "requires SLUG_V2_NATIVELINK_ENDPOINT"]
    async fn nativelink_file_write_bytes_digest_and_materialized_mode_match_oracle() {
        let endpoint = std::env::var("SLUG_V2_NATIVELINK_ENDPOINT")
            .expect("SLUG_V2_NATIVELINK_ENDPOINT points at a local NativeLink server");
        let config = RemoteConfig {
            executor: Some(endpoint),
            cache: None,
            instance_name: None,
            headers: BTreeMap::new(),
            timeout_seconds: Some(30),
            retry_attempts: None,
            default_exec_properties: BTreeMap::new(),
        };
        let content = b"hello from an action\n";
        let plan = plan(
            std::str::from_utf8(content).unwrap(),
            false,
            "pkg/write_file.txt",
            &[("container-image", "selected:v1")],
        );
        let inline_output_files = plan.command().output_files.clone();
        let execution = execute_prepared(
            &config,
            plan.command().clone(),
            plan.input_tree().clone(),
            plan.identity().clone(),
            inline_output_files,
        )
        .await
        .unwrap();

        assert_eq!(execution.output_blobs["pkg/write_file.txt"], content);
        let expected_digest = ReapiDigest::of_bytes(content);
        assert_eq!(
            execution.result.output_files()[0].digest(),
            &expected_digest
        );
        assert_eq!(execution.evidence.materialized_outputs, [expected_digest]);

        let root =
            std::env::temp_dir().join(format!("slug-filewrite-nativelink-{}", std::process::id()));
        materialize_outputs(&root, &execution).unwrap();
        let output = root.join("pkg/write_file.txt");
        assert_eq!(std::fs::read(&output).unwrap(), content);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o555
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
