//! Attempt-local generated input binding. Core owns ordering and freshness gates.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ExpandedSpawnCommandLine;
use slug_core_v2::runtime::ActionChainTransport;
use slug_core_v2::runtime::PreparedActionChainInputs;
use slug_core_v2::runtime::PreparedRunfilesAction;
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::TransferPolicy;

use crate::executor::cache_error;
use crate::executor::execute_staged_metadata;
use crate::executor::tonic_endpoint;
use crate::source_spawn::spawn_command;
use crate::*;

mod binding;
use binding::Binding;
use binding::BoundInputs;
use binding::validate_paths;

mod output_staging;
mod result;
pub use result::ActionChainStepResult;
pub use result::LocalManifestResult;

/// Immutable remote policy. Each Core attempt creates an independent session.
pub struct ActionChainReapiTransport {
    config: RemoteConfig,
}

/// Remote and local outcomes in plan order, exposed only after native acceptance.
#[derive(Debug)]
pub struct ActionChainRemoteResult {
    results: Vec<ActionChainStepResult>,
    selected: Option<usize>,
}
impl ActionChainRemoteResult {
    pub fn results(&self) -> &[ActionChainStepResult] {
        &self.results
    }
    /// Requested forests have an ordered result table, but no single selected action.
    pub fn selected(&self) -> Option<&ActionChainStepResult> {
        self.selected.map(|index| &self.results[index])
    }
}

/// Opaque: neither old results nor caller-selected endpoints can be substituted.
pub struct ActionChainReapiSession {
    inputs: Arc<PreparedActionChainInputs>,
    config: RemoteConfig,
    templates: Vec<Template>,
    results: Vec<ActionChainStepResult>,
    cache: CacheClient,
    channel: tonic::transport::Channel,
    identity: Arc<()>,
}
pub struct StagedChainAction {
    session: Arc<()>,
    index: usize,
    payload: StagedPayload,
}
enum StagedPayload {
    Remote {
        command: ReapiCommand,
        identity: ReapiActionIdentity,
        uploaded: Vec<ReapiDigest>,
    },
    Runfiles(ActionChainStepResult),
}

enum Template {
    Runfiles(ActionChainStepResult),
    Write(FileWriteReapiPlan),
    Spawn {
        command: ReapiCommand,
        expanded: ExpandedSpawnCommandLine,
        inputs: Vec<Binding>,
    },
}
struct BoundAction {
    command: ReapiCommand,
    tree: ReapiInputTree,
    sources: BTreeMap<ReapiDigest, usize>,
    generated: BTreeSet<ReapiDigest>,
    local: BTreeMap<ReapiDigest, Arc<[u8]>>,
}
fn command_error(value: impl ToString) -> RemoteExecutionError {
    RemoteExecutionError::Command(value.to_string())
}
fn protocol(value: impl Into<String>) -> RemoteExecutionError {
    RemoteExecutionError::Protocol(value.into())
}

impl ActionChainReapiTransport {
    pub fn new(config: RemoteConfig) -> Result<Self, RemoteExecutionError> {
        // Share the established policy allowlist, without opening a connection.
        SourceReapiTransport::new(config.clone())?;
        Ok(Self { config })
    }
}

impl Template {
    /// Pure whole-chain preflight, before a channel or action can be started.
    fn prepare(
        inputs: &PreparedActionChainInputs,
        defaults: &BTreeMap<String, String>,
    ) -> Result<Vec<Self>, RemoteExecutionError> {
        let sources = inputs
            .sources()
            .enumerate()
            .map(|(index, source)| {
                let hash = source
                    .digest()
                    .sha256()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                let digest =
                    ReapiDigest::new(hash, source.digest().size_bytes()).map_err(command_error)?;
                Ok((
                    AnalysisArtifact::Source(source.label().clone()),
                    (index, digest),
                ))
            })
            .collect::<Result<std::collections::HashMap<_, _>, RemoteExecutionError>>()?;
        let plan = inputs.plan().map_err(command_error)?;
        plan.actions()
            .iter()
            .map(|step| {
                let action = step.action();
                if action.runfiles_support_spec().is_some()
                    && let Some(runfiles) = inputs
                        .prepare_runfiles_action(action)
                        .map_err(command_error)?
                {
                    return Ok(Self::Runfiles(ActionChainStepResult::from_runfiles(
                        runfiles,
                    )));
                }
                let Some(spawn) = action.spawn_spec() else {
                    return FileWriteReapiPlan::from_chain_action(action, defaults)
                        .map(Self::Write)
                        .map_err(command_error);
                };
                let mut command = spawn_command(action, defaults).map_err(command_error)?;
                let expanded = spawn.expand_forced_param_files().map_err(command_error)?;
                command.argv = expanded.argv().to_vec();
                let bindings = step
                    .inputs()
                    .iter()
                    .map(|input| Binding::prepare(input, &sources, &plan))
                    .collect::<Result<Vec<_>, RemoteExecutionError>>()?;
                validate_namespaces(&bindings, &expanded, &command)?;
                // Validate canonical input/param paths before any upstream action runs.
                let mut entries = Vec::new();
                let mut directories = command.output_directories.clone();
                for binding in &bindings {
                    binding.preflight(&mut entries, &mut directories);
                }
                ReapiInputTree::from_entries_and_directories(entries, directories)
                    .and_then(|tree| tree.with_spawn_param_files_executable(&expanded, true))
                    .map_err(command_error)?;
                Ok(Self::Spawn {
                    command,
                    expanded,
                    inputs: bindings,
                })
            })
            .collect()
    }

    fn bind(&self, results: &[ActionChainStepResult]) -> Result<BoundAction, RemoteExecutionError> {
        let Self::Spawn {
            command,
            expanded,
            inputs,
        } = self
        else {
            let Self::Write(plan) = self else {
                return Err(protocol("runfiles step has no remote input tree"));
            };
            return Ok(BoundAction {
                command: plan.command().clone(),
                tree: plan.input_tree().clone(),
                sources: BTreeMap::new(),
                generated: BTreeSet::new(),
                local: BTreeMap::new(),
            });
        };
        let mut bound = BoundInputs {
            directories: command.output_directories.clone(),
            ..Default::default()
        };
        for binding in inputs {
            bound.add(binding, results)?;
        }
        let tree = ReapiInputTree::from_entries_and_directories(bound.files, bound.directories)
            .and_then(|tree| tree.with_spawn_param_files_executable(expanded, true))
            .map_err(command_error)?;
        Ok(BoundAction {
            command: command.clone(),
            tree,
            sources: bound.sources,
            generated: bound.generated,
            local: bound.local,
        })
    }
}

fn input_entry(path: &str, digest: ReapiDigest) -> ReapiInputTreeEntry {
    ReapiInputTreeEntry::new(path, digest, InputTreeEntryKind::Input).with_executable(true)
}

fn validate_namespaces(
    inputs: &[Binding],
    expanded: &ExpandedSpawnCommandLine,
    command: &ReapiCommand,
) -> Result<(), RemoteExecutionError> {
    let paths = inputs
        .iter()
        .map(Binding::path)
        .chain(expanded.param_files().iter().map(|file| file.path()))
        .chain(
            command
                .output_files
                .iter()
                .chain(&command.output_directories)
                .map(String::as_str),
        );
    // Reserve whole-tree namespaces before generated children are known.
    validate_paths(paths)
}

impl ActionChainTransport for ActionChainReapiTransport {
    type Session = ActionChainReapiSession;
    type Staged = StagedChainAction;
    type Output = ActionChainRemoteResult;
    type Error = RemoteExecutionError;

    async fn start(
        &self,
        inputs: Arc<PreparedActionChainInputs>,
    ) -> Result<Self::Session, Self::Error> {
        let templates = Template::prepare(&inputs, &self.config.default_exec_properties)?;
        let endpoint =
            tonic_endpoint(self.config.executor.as_deref().expect("validated executor"))?;
        let endpoint = tonic::transport::Endpoint::from_shared(endpoint)
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
        let remote = templates
            .iter()
            .any(|template| !matches!(template, Template::Runfiles(_)));
        let channel = if remote {
            endpoint
                .connect()
                .await
                .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
        } else {
            endpoint.connect_lazy()
        };
        let mut cache = CacheClient::new(
            channel.clone(),
            self.config.instance_name.clone().unwrap_or_default(),
            TransferPolicy::default(),
        )
        .map_err(cache_error)?;
        if remote {
            cache
                .apply_server_batch_limit()
                .await
                .map_err(cache_error)?;
        }
        Ok(ActionChainReapiSession {
            inputs,
            config: self.config.clone(),
            templates,
            results: Vec::new(),
            cache,
            channel,
            identity: Arc::new(()),
        })
    }

    async fn stage(
        &self,
        session: &mut Self::Session,
        index: usize,
    ) -> Result<Self::Staged, Self::Error> {
        if index != session.results.len() {
            return Err(protocol("chain staging is out of order"));
        }
        let template = session
            .templates
            .get(index)
            .ok_or_else(|| protocol("unknown chain step"))?;
        if let Template::Runfiles(result) = template {
            return Ok(StagedChainAction {
                session: session.identity.clone(),
                index,
                payload: StagedPayload::Runfiles(result.clone()),
            });
        }
        let bound = template.bind(&session.results)?;
        let identity =
            ReapiActionIdentity::new(&bound.command, bound.tree.root_digest().clone(), None);
        let blobs = bound
            .tree
            .directory_blobs()
            .iter()
            .chain(bound.tree.inline_blobs())
            .cloned()
            .chain([
                ReapiBlob::from_bytes(bound.command.serialized()),
                ReapiBlob::from_bytes(identity.action_bytes().to_vec()),
            ])
            .map(|blob| (blob.digest().clone(), blob))
            .collect::<BTreeMap<_, _>>();
        let required = bound
            .tree
            .entries()
            .iter()
            .map(|entry| entry.digest().clone())
            .chain(blobs.keys().cloned())
            .collect();
        let missing = session
            .cache
            .find_missing(&required)
            .await
            .map_err(cache_error)?;
        // Check CAS-only provenance first, even when the digest also has local bytes.
        for digest in &missing {
            if bound.generated.contains(digest) {
                return Err(RemoteExecutionError::MissingBlobData {
                    digest: digest.clone(),
                });
            }
        }
        for digest in &missing {
            if let Some(blob) = blobs.get(digest) {
                session
                    .cache
                    .upload_missing(&[blob.clone()])
                    .await
                    .map_err(cache_error)?;
            } else if let Some(bytes) = bound.local.get(digest) {
                session
                    .cache
                    .upload_reader_verified(digest, std::io::Cursor::new(bytes.clone()))
                    .await
                    .map_err(cache_error)?;
            } else if let Some(index) = bound.sources.get(digest) {
                let file = session
                    .inputs
                    .open_source(*index)
                    .map_err(|error| protocol(error.to_string()))?;
                session
                    .cache
                    .upload_reader_verified(digest, tokio::fs::File::from_std(file))
                    .await
                    .map_err(cache_error)?;
            } else {
                return Err(RemoteExecutionError::MissingBlobData {
                    digest: digest.clone(),
                });
            }
        }
        for digest in &required {
            session
                .cache
                .read_blob_verified(digest, |_| Ok(()))
                .await
                .map_err(cache_error)?;
        }
        Ok(StagedChainAction {
            session: session.identity.clone(),
            index,
            payload: StagedPayload::Remote {
                command: bound.command,
                identity,
                uploaded: missing.into_iter().collect(),
            },
        })
    }

    async fn execute(
        &self,
        session: &mut Self::Session,
        index: usize,
        staged: Self::Staged,
    ) -> Result<(), Self::Error> {
        if index != session.results.len()
            || index != staged.index
            || !Arc::ptr_eq(&session.identity, &staged.session)
        {
            return Err(protocol(
                "staged action belongs to another chain attempt or step",
            ));
        }
        let result = match staged.payload {
            StagedPayload::Runfiles(result) => result,
            StagedPayload::Remote {
                command,
                identity,
                uploaded,
            } => ActionChainStepResult::Remote(
                execute_staged_metadata(
                    &session.config,
                    &command,
                    &identity,
                    &session.cache,
                    session.channel.clone(),
                    uploaded,
                )
                .await?,
            ),
        };
        session.results.push(result);
        Ok(())
    }

    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        if session.results.len() != session.templates.len() || session.results.is_empty() {
            return Err(protocol("chain did not complete every prerequisite"));
        }
        Ok(ActionChainRemoteResult {
            selected: session
                .inputs
                .plan()
                .map_err(command_error)?
                .selected_action()
                .map(|_| session.results.len() - 1),
            results: session.results,
        })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod requested_tests;

#[cfg(test)]
mod local_results_tests;

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod runfiles_tests;

#[cfg(all(test, target_os = "linux", target_env = "gnu"))]
mod files_to_run_tests;
