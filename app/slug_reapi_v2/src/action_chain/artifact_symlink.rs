//! Resolved artifact aliases retain their terminal byte authority within one session.

use super::*;

#[derive(Debug, Clone)]
pub(super) enum FileBacking {
    Source(usize),
    GeneratedCas,
    Local(Arc<[u8]>),
}

/// A local artifact-alias completion, never a remote action or unresolved link.
#[derive(Debug, Clone)]
pub struct ArtifactSymlinkResult {
    output: ActionOutput,
    digest: ReapiDigest,
    executable: bool,
    backing: FileBacking,
}
impl ArtifactSymlinkResult {
    pub fn output(&self) -> &ActionOutput {
        &self.output
    }
    pub fn digest(&self) -> &ReapiDigest {
        &self.digest
    }
    /// Effective target status, distinct from the consumer's executable FileNode policy.
    pub fn is_executable(&self) -> bool {
        self.executable
    }
    pub(super) fn backing(&self) -> &FileBacking {
        &self.backing
    }
}

pub(super) struct ArtifactSymlinkTemplate {
    output: ActionOutput,
    input: Binding,
    require_executable: bool,
    source_executable: Option<bool>,
}
impl ArtifactSymlinkTemplate {
    pub(super) fn new(
        prepared: slug_core_v2::runtime::PreparedArtifactSymlink,
        input: Binding,
    ) -> Self {
        Self {
            output: prepared.output().clone(),
            input,
            require_executable: prepared.require_executable(),
            source_executable: prepared.source_executable(),
        }
    }

    pub(super) fn resolve(
        &self,
        results: &[ActionChainStepResult],
    ) -> Result<ArtifactSymlinkResult, RemoteExecutionError> {
        let (digest, executable, backing) = match &self.input {
            Binding::Source { digest, index, .. } => (
                digest.clone(),
                self.source_executable
                    .ok_or_else(|| protocol("alias source lacks observed permissions"))?,
                FileBacking::Source(*index),
            ),
            Binding::Generated {
                producer, output, ..
            } if output.kind() == ActionOutputKind::File => {
                let result = results
                    .get(*producer)
                    .ok_or_else(|| protocol("alias target has not completed in this attempt"))?;
                if let ActionChainStepResult::ArtifactSymlink(alias) = result {
                    if alias.output() != output {
                        return Err(protocol("alias target differs from declared producer"));
                    }
                    (
                        alias.digest.clone(),
                        alias.executable,
                        alias.backing.clone(),
                    )
                } else if let Some(file) = result.local_file() {
                    if file.output() != output {
                        return Err(protocol(
                            "alias manifest target differs from declared producer",
                        ));
                    }
                    // Generated local files follow the existing 0555 output policy.
                    (
                        file.digest().clone(),
                        true,
                        FileBacking::Local(file.bytes.clone()),
                    )
                } else {
                    let remote = result
                        .remote()
                        .ok_or_else(|| protocol("alias target is not a completed regular file"))?;
                    let file = remote
                        .result
                        .output_files()
                        .iter()
                        .find(|file| file.path() == output.path())
                        .ok_or_else(|| protocol("alias target is absent from verified producer"))?;
                    // Bazel RemoteActionFileSystem treats remote-only files executable;
                    // its raw output mode is not an executable-alias rejection gate.
                    (file.digest().clone(), true, FileBacking::GeneratedCas)
                }
            }
            _ => return Err(protocol("unsupported artifact alias input kind")),
        };
        if self.require_executable && !executable {
            return Err(command_error("artifact symlink target is not executable"));
        }
        Ok(ArtifactSymlinkResult {
            output: self.output.clone(),
            digest,
            executable,
            backing,
        })
    }
}

pub(super) async fn stage(
    template: &ArtifactSymlinkTemplate,
    session: &ActionChainReapiSession,
) -> Result<ActionChainStepResult, RemoteExecutionError> {
    let result = template.resolve(&session.results)?;
    if matches!(result.backing, FileBacking::GeneratedCas) {
        let required = BTreeSet::from([result.digest.clone()]);
        if !session
            .cache
            .find_missing(&required)
            .await
            .map_err(cache_error)?
            .is_empty()
        {
            return Err(RemoteExecutionError::MissingBlobData {
                digest: result.digest,
            });
        }
        session
            .cache
            .read_blob_verified(&result.digest, |_| Ok(()))
            .await
            .map_err(cache_error)?;
    }
    Ok(ActionChainStepResult::ArtifactSymlink(result))
}

#[cfg(test)]
mod tests;
