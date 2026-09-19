//! Verified selected-output transfer into Core-owned, unpublished staging.

use std::io::Write;

use slug_core_v2::runtime::ActionChainOutputTransport;
use slug_core_v2::runtime::ActionOutputStaging;
use slug_core_v2::runtime::PlannedActionOutputStaging;
use slug_reapi_cache_v2::CacheError;

use super::*;

#[derive(Clone, Copy)]
enum SelectedOutput<'a> {
    File(&'a GeneratedOutput),
    Directory(&'a GeneratedDirectory),
    Local(&'a LocalManifestResult),
    RunfilesTree,
    ArtifactSymlink,
}

/// Reconcile the entire batch before creating even the first staging file.
fn selected_outputs<'a>(
    session: &'a ActionChainReapiSession,
    stages: &[PlannedActionOutputStaging],
) -> Result<Vec<Vec<SelectedOutput<'a>>>, RemoteExecutionError> {
    project_outputs(
        session,
        stages
            .iter()
            .map(|stage| (stage.action_index(), stage.staging().outputs())),
    )
}

fn project_outputs<'a, 's>(
    session: &'a ActionChainReapiSession,
    selections: impl IntoIterator<Item = (usize, &'s [ActionOutput])>,
) -> Result<Vec<Vec<SelectedOutput<'a>>>, RemoteExecutionError> {
    let plan = session.inputs.plan().map_err(command_error)?;
    if session.results.len() != session.templates.len()
        || session.results.len() != plan.actions().len()
        || session.results.is_empty()
    {
        return Err(protocol("output staging requires a completed action plan"));
    }
    let mut producers = BTreeSet::new();
    selections
        .into_iter()
        .map(|(index, selected)| {
            let action = plan
                .actions()
                .get(index)
                .ok_or_else(|| protocol("output staging producer index is outside the plan"))?
                .action();
            if !producers.insert(index) {
                return Err(protocol("duplicate output staging producer"));
            }
            // An unselected cooutput is still part of the producer's contract.
            let complete = reconcile_step_outputs(action.outputs(), &session.results[index])?;
            let mut outputs = action
                .outputs()
                .iter()
                .zip(complete)
                .collect::<BTreeMap<_, _>>();
            selected
                .iter()
                .map(|output| {
                    outputs.remove(output).ok_or_else(|| {
                        protocol("staged output subset differs from producer declaration")
                    })
                })
                .collect()
        })
        .collect()
}

fn reconcile_step_outputs<'a>(
    declared: &[ActionOutput],
    result: &'a ActionChainStepResult,
) -> Result<Vec<SelectedOutput<'a>>, RemoteExecutionError> {
    let (output, selected) = match result {
        ActionChainStepResult::Remote(remote) => {
            return reconcile_outputs(declared, &remote.result);
        }
        ActionChainStepResult::Manifest(file) | ActionChainStepResult::SymlinkTree(file) => {
            if file.output().kind() != ActionOutputKind::File {
                return Err(protocol("local manifest has a non-file output"));
            }
            (file.output(), SelectedOutput::Local(file))
        }
        ActionChainStepResult::ArtifactSymlink(alias) => {
            if alias.output().kind() != ActionOutputKind::File {
                return Err(protocol("artifact symlink result has wrong output kind"));
            }
            (alias.output(), SelectedOutput::ArtifactSymlink)
        }
        ActionChainStepResult::RunfilesTree { output } => {
            if output.kind() != ActionOutputKind::RunfilesTree {
                return Err(protocol("virtual runfiles result has wrong output kind"));
            }
            (output, SelectedOutput::RunfilesTree)
        }
    };
    if declared != [output.clone()] {
        return Err(protocol(
            "local output differs from complete producer declaration",
        ));
    }
    Ok(vec![selected])
}

fn reconcile_outputs<'a>(
    declared: &[ActionOutput],
    result: &'a ActionResult,
) -> Result<Vec<SelectedOutput<'a>>, RemoteExecutionError> {
    let mut outputs = BTreeMap::new();
    for file in result.output_files() {
        if outputs
            .insert(
                (file.path(), ActionOutputKind::File),
                SelectedOutput::File(file),
            )
            .is_some()
        {
            return Err(protocol("duplicate selected output file"));
        }
    }
    for directory in result.output_directories() {
        if outputs
            .insert(
                (directory.path(), ActionOutputKind::Directory),
                SelectedOutput::Directory(directory),
            )
            .is_some()
        {
            return Err(protocol("duplicate selected output directory"));
        }
    }
    let selected = declared
        .iter()
        .map(|output| {
            outputs
                .remove(&(output.path(), output.kind()))
                .ok_or_else(|| protocol("selected output paths or kinds differ from staging owner"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !outputs.is_empty() {
        return Err(protocol("selected result has undeclared staging outputs"));
    }
    Ok(selected)
}

async fn stage_output(
    cache: &CacheClient,
    staging: &ActionOutputStaging,
    index: usize,
    output: SelectedOutput<'_>,
) -> Result<(), RemoteExecutionError> {
    match output {
        SelectedOutput::Local(file) => {
            let mut destination = staging
                .create_file(index, "")
                .map_err(|error| protocol(format!("creating staged manifest: {error}")))?;
            destination
                .write_all(file.bytes())
                .map_err(|error| protocol(format!("writing staged manifest: {error}")))?;
        }
        // Core owns the confined writer and couples the physical MANIFEST link.
        SelectedOutput::RunfilesTree | SelectedOutput::ArtifactSymlink => {}
        SelectedOutput::File(file) => {
            stage_file(cache, staging, index, "", file.digest()).await?;
        }
        SelectedOutput::Directory(directory) => {
            for path in directory.directories() {
                staging.create_directory(index, path).map_err(|error| {
                    protocol(format!("creating staged output directory: {error}"))
                })?;
            }
            for file in directory.files() {
                stage_file(cache, staging, index, file.path(), file.digest()).await?;
            }
        }
    }
    Ok(())
}

async fn stage_file(
    cache: &CacheClient,
    staging: &ActionOutputStaging,
    index: usize,
    relative: &str,
    digest: &ReapiDigest,
) -> Result<(), RemoteExecutionError> {
    let mut file = staging
        .create_file(index, relative)
        .map_err(|error| protocol(format!("creating staged output file: {error}")))?;
    cache
        .read_blob_verified(digest, |chunk| {
            file.write_all(chunk)
                .map_err(|error| CacheError::Sink(format!("writing staged output file: {error}")))
        })
        .await
        .map_err(cache_error)?;
    // The file handle closes here. Core seals permissions and publishes only
    // after the entire set and the request's full source frontier are validated.
    Ok(())
}

impl ActionChainOutputTransport for ActionChainReapiTransport {
    async fn stage_outputs(
        &self,
        session: &mut Self::Session,
        stages: &[PlannedActionOutputStaging],
    ) -> Result<(), Self::Error> {
        let selected = selected_outputs(session, stages)?;
        for (stage, selected) in stages.iter().zip(selected) {
            for (index, output) in selected.into_iter().enumerate() {
                stage_output(&session.cache, stage.staging(), index, output).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod requested_publication_tests;
