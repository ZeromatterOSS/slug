//! Pure command evidence from native accepted execution and publication metadata.

use std::collections::BTreeMap;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::RequestedActionResult;

use crate::ActionChainRemoteResult;
use crate::ReapiDigest;
use crate::RemoteExecutionError;
use crate::RemoteExecutionResult;

/// Render accepted Build evidence without executing, reading or materializing anything.
/// Publication groups, rather than producer cooutputs, determine materialized digests.
pub fn requested_build_success_json(
    accepted: &RequestedActionResult<ActionChainRemoteResult>,
    runtime_mode: &str,
    invalidated_files: Option<usize>,
) -> Result<String, RemoteExecutionError> {
    let evaluation = accepted.inputs().evaluation().map_err(|error| {
        RemoteExecutionError::Protocol(format!(
            "successful requested result retained an error: {error}"
        ))
    })?;
    let results = accepted
        .output()
        .map_or(&[][..], ActionChainRemoteResult::results);
    let materialized = materialized_digests(
        results,
        accepted
            .published_outputs()
            .iter()
            .map(|group| (group.action_index(), group.outputs().outputs())),
    )?;
    Ok(success_json(
        evaluation.analyzed_target_count(),
        evaluation.declared_action_count(),
        results,
        &materialized,
        runtime_mode,
        invalidated_files,
    ))
}

fn materialized_digests<'a>(
    results: &'a [RemoteExecutionResult],
    groups: impl IntoIterator<Item = (usize, &'a [ActionOutput])>,
) -> Result<Vec<&'a ReapiDigest>, RemoteExecutionError> {
    let mut digests = Vec::new();
    for (index, outputs) in groups {
        let result = &results
            .get(index)
            .ok_or_else(|| {
                RemoteExecutionError::Protocol("published producer has no result".into())
            })?
            .result;
        for output in outputs {
            match output.kind() {
                ActionOutputKind::File => {
                    let file = result
                        .output_files()
                        .iter()
                        .find(|file| file.path() == output.path())
                        .ok_or_else(|| {
                            RemoteExecutionError::Protocol(
                                "published file has no verified result".into(),
                            )
                        })?;
                    digests.push(file.digest());
                }
                ActionOutputKind::Directory => {
                    let tree = result
                        .output_directories()
                        .iter()
                        .find(|tree| tree.path() == output.path())
                        .ok_or_else(|| {
                            RemoteExecutionError::Protocol(
                                "published directory has no verified result".into(),
                            )
                        })?;
                    digests.extend(tree.files().iter().map(|file| file.digest()));
                }
                _ => {
                    return Err(RemoteExecutionError::Protocol(
                        "unsupported published output kind".into(),
                    ));
                }
            }
        }
    }
    Ok(digests)
}

fn digest_json<'a>(digests: impl IntoIterator<Item = &'a ReapiDigest>) -> String {
    digests
        .into_iter()
        .map(|digest| format!("\"{}\"", json_escape(&digest.to_string())))
        .collect::<Vec<_>>()
        .join(",")
}

fn success_json(
    analyzed_target_count: usize,
    declared_action_count: usize,
    results: &[RemoteExecutionResult],
    materialized: &[&ReapiDigest],
    runtime_mode: &str,
    invalidated_files: Option<usize>,
) -> String {
    let mut reapi_actions = 0;
    let mut direct_local_actions = 0;
    let mut ac_hits = 0;
    let mut ac_misses = 0;
    let mut platform_properties = BTreeMap::new();
    for result in results {
        reapi_actions += result.evidence.reapi_actions;
        direct_local_actions += result.evidence.direct_local_actions;
        ac_hits += result.evidence.ac_hits;
        ac_misses += result.evidence.ac_misses;
        platform_properties.extend(result.platform_properties.iter());
    }
    let action_digests = digest_json(results.iter().map(|result| &result.action_digest));
    let uploaded_digests = digest_json(
        results
            .iter()
            .flat_map(|result| &result.evidence.uploaded_digests),
    );
    let materialized_outputs = digest_json(materialized.iter().copied());
    let platform_properties = platform_properties
        .iter()
        .map(|(key, value)| format!("\"{}\":\"{}\"", json_escape(key), json_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    let invalidated = invalidated_files.map_or_else(String::new, |count| {
        format!(",\"invalidated_files\":{count}")
    });
    format!(
        "{{\"success\":true,\"command\":\"build\",\"analyzed_target_count\":{analyzed_target_count},\"declared_action_count\":{declared_action_count},\"reapi_actions\":{reapi_actions},\"direct_local_actions\":{direct_local_actions},\"ac_hits\":{ac_hits},\"ac_misses\":{ac_misses},\"action_digests\":[{action_digests}],\"uploaded_digests\":[{uploaded_digests}],\"materialized_outputs\":[{materialized_outputs}],\"platform_properties\":{{{platform_properties}}},\"runtime_mode\":\"{}\"{invalidated},\"completed_boundary\":\"reapi_native_execution\"}}\n",
        json_escape(runtime_mode),
    )
}

#[cfg(test)]
mod tests;
