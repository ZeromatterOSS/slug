//! Ordinary top-level artifact selection from producer-owned configured providers.

use std::sync::Arc;

use slug_analysis_v2::ConfiguredNodeKey;
use slug_analysis_v2::ConfiguredNodeKind;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderCollection;
use starlark_map::small_map::SmallMap;

use super::BuildCommandEvaluation;
use super::BuildTargetCompletion;

// OutputGroupInfo.determineOutputGroups returns a sorted set. Ordinary builds
// enable validations and do not enable the experimental validation aspect.
const ORDINARY_GROUPS: [&str; 4] = [
    "_hidden_top_level_INTERNAL_",
    "_validation",
    "default",
    "temp_files_INTERNAL_",
];

/// Request-local metadata borrowed from a build evaluation. This view neither
/// plans producers nor authorizes source reads, execution, or publication.
#[derive(Debug)]
pub struct RequestedArtifacts<'a> {
    targets: Vec<RequestedTargetArtifacts<'a>>,
    artifacts: Vec<AnalysisArtifact>,
}

#[derive(Debug)]
pub struct RequestedTargetArtifacts<'a> {
    pattern: &'a str,
    analysis: &'a ConfiguredNodeResult,
    groups: Vec<RequestedOutputGroup>,
}

#[derive(Debug)]
pub struct RequestedOutputGroup {
    name: &'static str,
    artifact_indices: Vec<usize>,
}

impl RequestedArtifacts<'_> {
    /// Requested targets in command order, including repeated requests.
    pub fn targets(&self) -> &[RequestedTargetArtifacts<'_>] {
        &self.targets
    }

    /// Structurally distinct artifacts in first request/group occurrence order.
    pub fn artifacts(&self) -> &[AnalysisArtifact] {
        &self.artifacts
    }
}

impl RequestedTargetArtifacts<'_> {
    pub fn pattern(&self) -> &str {
        self.pattern
    }

    pub fn requested_target(&self) -> &ConfiguredNodeKey {
        self.analysis.key()
    }

    pub fn actual_target(&self) -> &ConfiguredNodeKey {
        self.analysis.actual_target()
    }

    /// Nonempty groups in ordinary output-group order.
    pub fn groups(&self) -> &[RequestedOutputGroup] {
        &self.groups
    }
}

impl RequestedOutputGroup {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn is_important(&self) -> bool {
        !self.name.starts_with('_')
    }

    /// Membership in RequestedArtifacts::artifacts(), in stable depset order.
    pub fn artifact_indices(&self) -> &[usize] {
        &self.artifact_indices
    }
}

impl BuildCommandEvaluation {
    /// Select the complete ordinary build output groups of every admitted root.
    /// Loading-only results are not empty analyzed targets. Unsupported artifact
    /// kinds remain selected: the later execution planner must admit or reject
    /// them explicitly, rather than silently dropping hidden requirements.
    pub fn requested_artifacts(&self) -> Result<RequestedArtifacts<'_>, Arc<str>> {
        let mut artifacts = SmallMap::new();
        let mut targets = Vec::with_capacity(self.targets.len());
        for target in &*self.targets {
            let analysis = target
                .analysis
                .as_deref()
                .filter(|_| target.completion != BuildTargetCompletion::LoadedOnly)
                .filter(|analysis| {
                    matches!(
                        analysis.kind(),
                        ConfiguredNodeKind::Rule
                            | ConfiguredNodeKind::SourceFile
                            | ConfiguredNodeKind::GeneratedFile
                            | ConfiguredNodeKind::Alias
                    )
                })
                .ok_or_else(|| {
                    Arc::from(format!(
                        "requested artifact selection is unsupported for unactivated target {}",
                        target.pattern
                    ))
                })?;
            targets.push(RequestedTargetArtifacts {
                pattern: &target.pattern,
                analysis,
                groups: select_groups(analysis.providers(), &mut artifacts)?,
            });
        }
        Ok(RequestedArtifacts {
            targets,
            artifacts: artifacts
                .into_iter()
                .map(|(artifact, _)| artifact)
                .collect(),
        })
    }
}

fn select_groups(
    providers: &ProviderCollection,
    artifacts: &mut SmallMap<AnalysisArtifact, usize>,
) -> Result<Vec<RequestedOutputGroup>, Arc<str>> {
    let mut groups = Vec::new();
    for name in ORDINARY_GROUPS {
        let mut children = Vec::with_capacity(2);
        if name == "default"
            && let Some(default) = providers.default_info()
        {
            children.push(default.files().clone());
        }
        if let Some(files) = providers
            .output_group_info()
            .and_then(|info| info.groups().get(name))
        {
            children.push(files.clone());
        }
        // Keep the retained children intact. Flatten only this command view,
        // after the same stable union used by TopLevelArtifactHelper.
        let files = AnalysisDepset::new(DepsetOrder::Default, Vec::new(), children)
            .map_err(|error| Arc::from(error.to_string()))?;
        if files.is_empty() {
            continue;
        }
        let mut artifact_indices = Vec::new();
        for value in files.to_list() {
            let AnalysisValueKind::Artifact(artifact) = value.kind() else {
                unreachable!("checked file/output-group providers contain only artifacts")
            };
            let next = artifacts.len();
            artifact_indices.push(*artifacts.entry(artifact.clone()).or_insert(next));
        }
        groups.push(RequestedOutputGroup {
            name,
            artifact_indices,
        });
    }
    Ok(groups)
}

#[cfg(test)]
#[path = "requested_artifacts/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "requested_artifacts/unit_tests.rs"]
mod unit_tests;
