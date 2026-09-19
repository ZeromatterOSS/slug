//! Session-local logical input projection; the native forest owns producer identity.

use std::collections::HashMap;

use slug_build_api_v2::RunfilesLayoutTarget;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_core_v2::runtime::PlannedActionInput;
use slug_core_v2::runtime::PreparedActionPlan;

use super::*;

pub(super) enum Binding {
    Source {
        path: String,
        digest: ReapiDigest,
        index: usize,
    },
    Generated {
        path: String,
        producer: usize,
        output: ActionOutput,
    },
    Runfiles {
        producer: usize,
        output: ActionOutput,
        entries: Vec<Binding>,
    },
    Empty {
        path: String,
    },
}

impl Binding {
    pub(super) fn prepare(
        input: &PlannedActionInput,
        sources: &HashMap<AnalysisArtifact, (usize, ReapiDigest)>,
        plan: &PreparedActionPlan<'_>,
    ) -> Result<Self, RemoteExecutionError> {
        if let AnalysisArtifact::Derived { output, .. } = input.artifact()
            && output.kind() == ActionOutputKind::RunfilesTree
        {
            let producer = input
                .producer()
                .ok_or_else(|| protocol("runfiles input has no planned producer"))?;
            let step = plan
                .actions()
                .get(producer)
                .ok_or_else(|| protocol("runfiles producer is outside the plan"))?;
            let Some(RunfilesSupportActionSpec::RunfilesTree { support, .. }) =
                step.action().runfiles_support_spec()
            else {
                return Err(protocol("runfiles input has the wrong producer family"));
            };
            if &support.tree != input.artifact() {
                return Err(protocol("runfiles input differs from its retained support"));
            }
            let layout = support.layout().map_err(command_error)?;
            // Preserve the exact input artifact's canonical producer ordinal, including
            // shared FileWrite representatives. Never recover ownership from a path.
            let targets = step
                .inputs()
                .iter()
                .map(|input| (input.artifact(), input))
                .collect::<HashMap<_, _>>();
            let entries = layout
                .entries()
                .iter()
                .map(|entry| {
                    let path = format!("{}/{}", output.path(), entry.path());
                    match entry.target() {
                        RunfilesLayoutTarget::Empty => Ok(Self::Empty { path }),
                        RunfilesLayoutTarget::Artifact(artifact) => {
                            let input = targets.get(artifact).ok_or_else(|| {
                                protocol("runfiles target has no exact declared input")
                            })?;
                            Self::leaf(input, path, sources)
                        }
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            validate_paths(entries.iter().map(Self::path))?;
            return Ok(Self::Runfiles {
                producer,
                output: output.clone(),
                entries,
            });
        }
        Self::leaf(input, input.artifact().path().into_owned(), sources)
    }

    fn leaf(
        input: &PlannedActionInput,
        path: String,
        sources: &HashMap<AnalysisArtifact, (usize, ReapiDigest)>,
    ) -> Result<Self, RemoteExecutionError> {
        Ok(match input.artifact() {
            AnalysisArtifact::Source(_) => {
                let (index, digest) = sources
                    .get(input.artifact())
                    .ok_or_else(|| protocol("unobserved chain source"))?;
                Self::Source {
                    path,
                    digest: digest.clone(),
                    index: *index,
                }
            }
            AnalysisArtifact::Derived { output, .. }
                if matches!(
                    output.kind(),
                    ActionOutputKind::File | ActionOutputKind::Directory
                ) =>
            {
                Self::Generated {
                    path,
                    producer: input
                        .producer()
                        .ok_or_else(|| protocol("generated input has no planned producer"))?,
                    output: output.clone(),
                }
            }
            _ => return Err(command_error("unsupported runfiles input target kind")),
        })
    }

    pub(super) fn path(&self) -> &str {
        match self {
            Self::Source { path, .. } | Self::Generated { path, .. } | Self::Empty { path } => path,
            Self::Runfiles { output, .. } => output.path(),
        }
    }

    pub(super) fn preflight(
        &self,
        files: &mut Vec<ReapiInputTreeEntry>,
        directories: &mut Vec<String>,
    ) {
        match self {
            Self::Runfiles {
                output, entries, ..
            } => {
                directories.push(output.path().to_owned());
                for entry in entries {
                    entry.preflight(files, directories);
                }
            }
            Self::Generated { output, path, .. }
                if output.kind() == ActionOutputKind::Directory =>
            {
                directories.push(path.clone());
            }
            _ => files.push(input_entry(self.path(), ReapiDigest::of_bytes(b""))),
        }
    }
}

#[derive(Default)]
pub(super) struct BoundInputs {
    pub files: Vec<ReapiInputTreeEntry>,
    pub directories: Vec<String>,
    pub sources: BTreeMap<ReapiDigest, usize>,
    pub generated: BTreeSet<ReapiDigest>,
    pub local: BTreeMap<ReapiDigest, Arc<[u8]>>,
}

impl BoundInputs {
    pub(super) fn add(
        &mut self,
        binding: &Binding,
        results: &[ActionChainStepResult],
    ) -> Result<(), RemoteExecutionError> {
        match binding {
            Binding::Source {
                path,
                digest,
                index,
            } => {
                self.files.push(input_entry(path, digest.clone()));
                self.sources.insert(digest.clone(), *index);
            }
            Binding::Empty { path } => {
                let digest = ReapiDigest::of_bytes(b"");
                self.files.push(input_entry(path, digest.clone()));
                self.local.insert(digest, Arc::from([]));
            }
            Binding::Runfiles {
                producer,
                output,
                entries,
            } => {
                if !matches!(results.get(*producer),
                    Some(ActionChainStepResult::RunfilesTree { output: completed }) if completed == output)
                {
                    return Err(protocol(
                        "runfiles producer has not completed its exact tree",
                    ));
                }
                self.directories.push(output.path().to_owned());
                for entry in entries {
                    self.add(entry, results)?;
                }
            }
            Binding::Generated {
                path,
                producer,
                output,
            } => {
                let completed = results
                    .get(*producer)
                    .ok_or_else(|| protocol("producer has not completed in this attempt"))?;
                if let ActionChainStepResult::ArtifactSymlink(alias) = completed {
                    if alias.output() != output {
                        return Err(protocol("artifact alias differs from declared input"));
                    }
                    self.files.push(input_entry(path, alias.digest().clone()));
                    match alias.backing() {
                        artifact_symlink::FileBacking::Source(index) => {
                            self.sources.insert(alias.digest().clone(), *index);
                        }
                        artifact_symlink::FileBacking::GeneratedCas => {
                            self.generated.insert(alias.digest().clone());
                        }
                        artifact_symlink::FileBacking::Local(bytes) => {
                            self.local.insert(alias.digest().clone(), bytes.clone());
                        }
                    }
                    return Ok(());
                }
                if let Some(file) = completed.local_file() {
                    if file.output() != output {
                        return Err(protocol("local producer differs from declared input"));
                    }
                    self.files.push(input_entry(path, file.digest().clone()));
                    self.local.insert(file.digest().clone(), file.bytes.clone());
                    return Ok(());
                }
                let result = &completed
                    .remote()
                    .ok_or_else(|| protocol("virtual runfiles tree cannot bind a file input"))?
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
                        self.generated.insert(file.digest().clone());
                        self.files.push(input_entry(path, file.digest().clone()));
                    }
                    ActionOutputKind::Directory => {
                        let tree = result
                            .output_directories()
                            .iter()
                            .find(|tree| tree.path() == output.path())
                            .ok_or_else(|| {
                                protocol("verified producer is missing its declared directory")
                            })?;
                        // File children only; an empty tree retains its root. Producer
                        // modes and nested empty directories do not enter this projection.
                        self.directories.push(path.clone());
                        for file in tree.files() {
                            self.generated.insert(file.digest().clone());
                            self.files.push(input_entry(
                                &format!("{path}/{}", file.path()),
                                file.digest().clone(),
                            ));
                        }
                    }
                    _ => return Err(protocol("unsupported generated input kind")),
                }
            }
        }
        Ok(())
    }
}

pub(super) fn validate_paths<'a>(
    paths: impl Iterator<Item = &'a str>,
) -> Result<(), RemoteExecutionError> {
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

#[cfg(test)]
mod tests;
