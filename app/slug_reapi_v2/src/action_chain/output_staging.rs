//! Verified selected-output transfer into Core-owned, unpublished staging.

use std::io::Write;

use slug_core_v2::runtime::ActionChainOutputTransport;
use slug_core_v2::runtime::ActionOutputStaging;
use slug_reapi_cache_v2::CacheError;

use super::*;

#[derive(Clone, Copy)]
enum SelectedOutput<'a> {
    File(&'a GeneratedOutput),
    Directory(&'a GeneratedDirectory),
}

/// Reconcile the entire declaration before creating even the first staging file.
fn selected_outputs<'a>(
    session: &'a ActionChainReapiSession,
    staging: &ActionOutputStaging,
) -> Result<Vec<SelectedOutput<'a>>, RemoteExecutionError> {
    if session.results.len() != session.templates.len() || session.results.is_empty() {
        return Err(protocol(
            "output staging requires a completed selected chain",
        ));
    }
    reconcile_outputs(staging.outputs(), &session.results.last().unwrap().result)
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
        staging: &ActionOutputStaging,
    ) -> Result<(), Self::Error> {
        let selected = selected_outputs(session, staging)?;
        for (index, output) in selected.into_iter().enumerate() {
            stage_output(&session.cache, staging, index, output).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
