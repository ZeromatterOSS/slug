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
use slug_reapi_cache_v2::CacheClient;
use slug_reapi_cache_v2::TransferPolicy;

use crate::executor::cache_error;
use crate::executor::execute_staged_metadata;
use crate::executor::tonic_endpoint;
use crate::source_spawn::spawn_command;
use crate::*;

mod output_staging;

/// Immutable remote policy. Each Core attempt creates an independent session.
pub struct ActionChainReapiTransport {
    config: RemoteConfig,
}

/// Verified metadata in prerequisite order, exposed only after native acceptance.
#[derive(Debug)]
pub struct ActionChainRemoteResult {
    results: Vec<RemoteExecutionResult>,
}
impl ActionChainRemoteResult {
    pub fn results(&self) -> &[RemoteExecutionResult] {
        &self.results
    }
    pub fn selected(&self) -> &RemoteExecutionResult {
        self.results
            .last()
            .expect("nonempty selected prerequisite chain")
    }
}

/// Opaque: neither old results nor caller-selected endpoints can be substituted.
pub struct ActionChainReapiSession {
    inputs: Arc<PreparedActionChainInputs>,
    config: RemoteConfig,
    templates: Vec<Template>,
    results: Vec<RemoteExecutionResult>,
    cache: CacheClient,
    channel: tonic::transport::Channel,
    identity: Arc<()>,
}
pub struct StagedChainAction {
    session: Arc<()>,
    index: usize,
    command: ReapiCommand,
    identity: ReapiActionIdentity,
    uploaded: Vec<ReapiDigest>,
}

enum Binding {
    Source {
        path: String,
        digest: ReapiDigest,
        index: usize,
    },
    Generated {
        producer: usize,
        output: ActionOutput,
    },
}
enum Template {
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
                Ok((source.label().clone(), (index, digest)))
            })
            .collect::<Result<BTreeMap<_, _>, RemoteExecutionError>>()?;
        let plan = inputs.plan().map_err(command_error)?;
        plan.actions()
            .iter()
            .map(|step| {
                let action = step.action();
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
                    .map(|input| {
                        Ok(match input.artifact() {
                            AnalysisArtifact::Source(label) => {
                                let (index, digest) = sources
                                    .get(label)
                                    .ok_or_else(|| protocol("unobserved chain source"))?;
                                Binding::Source {
                                    path: input.artifact().path().into_owned(),
                                    digest: digest.clone(),
                                    index: *index,
                                }
                            }
                            AnalysisArtifact::Derived { output, .. } => Binding::Generated {
                                producer: input.producer().ok_or_else(|| {
                                    protocol("generated input has no planned producer")
                                })?,
                                output: output.clone(),
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, RemoteExecutionError>>()?;
                validate_namespaces(&bindings, &expanded, &command)?;
                // Validate canonical input/param paths before any upstream action runs.
                let mut entries = Vec::new();
                let mut directories = command.output_directories.clone();
                for binding in &bindings {
                    match binding {
                        Binding::Source { path, digest, .. } => {
                            entries.push(input_entry(path, digest.clone()))
                        }
                        Binding::Generated { output, .. }
                            if output.kind() == ActionOutputKind::Directory =>
                        {
                            directories.push(output.path().to_owned())
                        }
                        Binding::Generated { output, .. } => entries.push(input_entry(
                            output.path(),
                            ReapiBlob::from_bytes(Vec::new()).digest().clone(),
                        )),
                    }
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

    fn bind(&self, results: &[RemoteExecutionResult]) -> Result<BoundAction, RemoteExecutionError> {
        let Self::Spawn {
            command,
            expanded,
            inputs,
        } = self
        else {
            let Self::Write(plan) = self else {
                unreachable!()
            };
            return Ok(BoundAction {
                command: plan.command().clone(),
                tree: plan.input_tree().clone(),
                sources: BTreeMap::new(),
                generated: BTreeSet::new(),
            });
        };
        let mut entries = Vec::new();
        let mut directories = command.output_directories.clone();
        let mut sources = BTreeMap::new();
        let mut generated = BTreeSet::new();
        for binding in inputs {
            match binding {
                Binding::Source {
                    path,
                    digest,
                    index,
                } => {
                    entries.push(input_entry(path, digest.clone()));
                    sources.insert(digest.clone(), *index);
                }
                Binding::Generated { producer, output } => {
                    let result = &results
                        .get(*producer)
                        .ok_or_else(|| protocol("producer has not completed in this attempt"))?
                        .result;
                    match output.kind() {
                        ActionOutputKind::File => {
                            let file = result
                                .output_files()
                                .iter()
                                .find(|file| file.path() == output.path())
                                .ok_or_else(|| {
                                    protocol("verified producer is missing its declared file")
                                })?;
                            generated.insert(file.digest().clone());
                            entries.push(input_entry(output.path(), file.digest().clone()));
                        }
                        ActionOutputKind::Directory => {
                            let tree = result
                                .output_directories()
                                .iter()
                                .find(|tree| tree.path() == output.path())
                                .ok_or_else(|| {
                                    protocol("verified producer is missing its declared directory")
                                })?;
                            // Bazel projects file children; do not graft truthful producer modes
                            // or nested empty directories into the consumer's input root.
                            directories.push(output.path().to_owned());
                            for file in tree.files() {
                                generated.insert(file.digest().clone());
                                entries.push(input_entry(
                                    &format!("{}/{}", output.path(), file.path()),
                                    file.digest().clone(),
                                ));
                            }
                        }
                        _ => return Err(protocol("unsupported generated input kind")),
                    }
                }
            }
        }
        let tree = ReapiInputTree::from_entries_and_directories(entries, directories)
            .and_then(|tree| tree.with_spawn_param_files_executable(expanded, true))
            .map_err(command_error)?;
        Ok(BoundAction {
            command: command.clone(),
            tree,
            sources,
            generated,
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
        .map(|input| match input {
            Binding::Source { path, .. } => path.as_str(),
            Binding::Generated { output, .. } => output.path(),
        })
        .chain(expanded.param_files().iter().map(|file| file.path()))
        .chain(
            command
                .output_files
                .iter()
                .chain(&command.output_directories)
                .map(String::as_str),
        );
    // Check whole-tree namespaces before their children are known.
    let mut paths = paths.collect::<Vec<_>>();
    paths.sort_unstable();
    // A lexical neighbor alone is insufficient when punctuation sorts before '/'.
    let mut seen = BTreeSet::new();
    for path in paths {
        if !seen.insert(path)
            || path
                .match_indices('/')
                .any(|(end, _)| seen.contains(&path[..end]))
        {
            return Err(command_error(format!(
                "chain input/param/output namespace conflict at {path}"
            )));
        }
    }
    Ok(())
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
        let channel = tonic::transport::Endpoint::from_shared(endpoint)
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?
            .connect()
            .await
            .map_err(|error| RemoteExecutionError::Transport(error.to_string()))?;
        let mut cache = CacheClient::new(
            channel.clone(),
            self.config.instance_name.clone().unwrap_or_default(),
            TransferPolicy::default(),
        )
        .map_err(cache_error)?;
        cache
            .apply_server_batch_limit()
            .await
            .map_err(cache_error)?;
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
        let bound = session
            .templates
            .get(index)
            .ok_or_else(|| protocol("unknown chain step"))?
            .bind(&session.results)?;
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
            command: bound.command,
            identity,
            uploaded: missing.into_iter().collect(),
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
        let result = execute_staged_metadata(
            &session.config,
            &staged.command,
            &staged.identity,
            &session.cache,
            session.channel.clone(),
            staged.uploaded,
        )
        .await?;
        session.results.push(result);
        Ok(())
    }

    async fn finish(&self, session: Self::Session) -> Result<Self::Output, Self::Error> {
        if session.results.len() != session.templates.len() || session.results.is_empty() {
            return Err(protocol("chain did not complete every prerequisite"));
        }
        Ok(ActionChainRemoteResult {
            results: session.results,
        })
    }
}

#[cfg(test)]
mod tests;
