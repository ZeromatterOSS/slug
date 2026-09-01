/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::SpawnExecutable;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::NormalizedBazelPath;
use slug_loading_v2::SubruleIdentity;
use slug_loading_v2::subrule_invocation::AnalysisActionCallScope;
use slug_loading_v2::subrule_invocation::AnalysisArtifactValue;
use slug_loading_v2::subrule_invocation::AnalysisSpawnInvocation;
use starlark::values::Value;
use starlark_map::small_map::SmallMap;

use crate::analysis_value::AnalysisValueLowerer;
use crate::analysis_value::files_to_run_provider as starlark_files_to_run_provider;
use crate::starlark_rule::PreparedConfiguredAttribute;
use crate::starlark_rule::PreparedDependency;

#[derive(Debug, Default)]
pub(crate) struct ExecutableArtifactProvenance {
    root: SmallMap<AnalysisArtifact, FilesToRunProvider>,
    subrules: SmallMap<Arc<SubruleIdentity>, SmallMap<AnalysisArtifact, FilesToRunProvider>>,
}

enum Association<'a> {
    Root(&'a FilesToRunProvider),
    Subrule,
}

impl ExecutableArtifactProvenance {
    fn association(
        &self,
        scope: &AnalysisActionCallScope,
        artifact: &AnalysisArtifact,
    ) -> Option<Association<'_>> {
        match scope {
            AnalysisActionCallScope::Root => self.root.get(artifact).map(Association::Root),
            AnalysisActionCallScope::Subrule(identity) => self
                .subrules
                .get(identity)
                .and_then(|artifacts| artifacts.get(artifact))
                .map(|_| Association::Subrule),
        }
    }
}

fn retained_files_to_run_provider(value: &AnalysisValue) -> Option<FilesToRunProvider> {
    let AnalysisValueKind::Provider(provider) = value.kind() else {
        return None;
    };
    FilesToRunProvider::from_occurrence(provider)
}

pub(crate) fn executable_artifact_provenance(
    dependencies: &[PreparedDependency],
    configured_attributes: &[PreparedConfiguredAttribute],
) -> ExecutableArtifactProvenance {
    let mut provenance = ExecutableArtifactProvenance::default();
    for provider in dependencies
        .iter()
        .filter_map(|dependency| dependency.executable.clone())
    {
        if let Some(executable) = provider.executable.clone() {
            provenance.root.insert(executable, provider);
        }
    }
    for attribute in configured_attributes {
        let (Some(owner), Some(provider)) = (
            &attribute.owner,
            retained_files_to_run_provider(&attribute.value),
        ) else {
            continue;
        };
        let Some(executable) = provider.executable.clone() else {
            continue;
        };
        provenance
            .subrules
            .entry(owner.clone())
            .or_default()
            .insert(executable, provider);
    }
    provenance
}

fn reject_directory_artifact(artifact: &AnalysisArtifact, name: &str) -> anyhow::Result<()> {
    if matches!(
        artifact,
        AnalysisArtifact::Derived { output, .. } if output.kind() == ActionOutputKind::Directory
    ) {
        anyhow::bail!("{name} must contain only regular Files")
    }
    Ok(())
}

fn subrule_association_error(name: &str) -> anyhow::Error {
    anyhow::anyhow!("{name}: expected FilesToRunProvider, got File")
}

pub(crate) fn retained_invocation(
    invocation: AnalysisSpawnInvocation<'_>,
    scope: &AnalysisActionCallScope,
    provenance: &ExecutableArtifactProvenance,
    path_flavor: HostPathFlavor,
    pad_dollar_zero: bool,
) -> anyhow::Result<RetainedSpawnInvocation> {
    match invocation {
        AnalysisSpawnInvocation::Executable(value) => {
            let executable = if let Some(file) = AnalysisArtifactValue::from_starlark(value) {
                reject_directory_artifact(file.artifact(), "ctx.actions.run executable")?;
                match provenance.association(scope, file.artifact()) {
                    Some(Association::Root(provider)) => {
                        SpawnExecutable::FilesToRun(provider.clone())
                    }
                    Some(Association::Subrule) => {
                        return Err(subrule_association_error("ctx.actions.run executable"));
                    }
                    None => SpawnExecutable::Artifact(file.artifact().clone()),
                }
            } else if let Some(provider) = starlark_files_to_run_provider(value) {
                let Some(executable) = &provider.executable else {
                    anyhow::bail!("ctx.actions.run FilesToRunProvider has no executable")
                };
                reject_directory_artifact(executable, "ctx.actions.run executable")?;
                SpawnExecutable::FilesToRun(provider.clone())
            } else if let Some(path) = value.unpack_str() {
                SpawnExecutable::Path(
                    NormalizedBazelPath::new(path_flavor, path)
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?,
                )
            } else {
                anyhow::bail!(
                    "ctx.actions.run executable must be a File, string, or FilesToRunProvider"
                )
            };
            Ok(RetainedSpawnInvocation::Executable(executable))
        }
        AnalysisSpawnInvocation::Shell(value) => value
            .unpack_str()
            .map(|command| RetainedSpawnInvocation::Shell {
                command: command.into(),
                pad_dollar_zero,
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ctx.actions.run_shell command must be a string under Bazel 9 defaults"
                )
            }),
    }
}

fn associated_tool_sources(
    provenance: &ExecutableArtifactProvenance,
    scope: &AnalysisActionCallScope,
    artifact: &AnalysisArtifact,
    name: &str,
) -> anyhow::Result<Option<ArtifactInputSource>> {
    match provenance.association(scope, artifact) {
        Some(Association::Root(provider)) => {
            Ok(Some(ArtifactInputSource::FilesToRun(provider.clone())))
        }
        Some(Association::Subrule) => Err(subrule_association_error(name)),
        None => Ok(None),
    }
}

fn validate_tool_depset(
    tools: &RetainedArtifactInputs,
    scope: &AnalysisActionCallScope,
    provenance: &ExecutableArtifactProvenance,
    collect_associations: bool,
) -> anyhow::Result<Vec<ArtifactInputSource>> {
    let mut sources = Vec::new();
    let mut error = None;
    tools
        .visit(|artifact| {
            if error.is_some() {
                return;
            }
            error = reject_directory_artifact(artifact, "ctx.actions.run tools").err();
            if error.is_none() && collect_associations {
                match associated_tool_sources(
                    provenance,
                    scope,
                    artifact,
                    "ctx.actions.run top-level depset tool",
                ) {
                    Ok(Some(source)) => sources.push(source),
                    Ok(None) => {}
                    Err(next) => error = Some(next),
                }
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    error.map_or(Ok(sources), Err)
}

pub(crate) fn retained_tools<'v>(
    value: Option<Value<'v>>,
    scope: &AnalysisActionCallScope,
    provenance: &ExecutableArtifactProvenance,
    lowerer: &mut AnalysisValueLowerer<'v>,
) -> anyhow::Result<ArtifactInputs> {
    let Some(value) = value else {
        return Ok(ArtifactInputs::new(Vec::new()));
    };
    if value.is_none() {
        anyhow::bail!("ctx.actions.run tools must be a sequence or depset");
    }
    let lowered = lowerer
        .lower(value, "ctx.actions.run tools")
        .map_err(anyhow::Error::msg)?;
    if let AnalysisValueKind::Depset(depset) = lowered.kind() {
        let retained = RetainedArtifactInputs::new(depset.clone())
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let mut sources = vec![ArtifactInputSource::Depset(retained.clone())];
        sources.extend(validate_tool_depset(&retained, scope, provenance, true)?);
        return Ok(ArtifactInputs::new(sources));
    }
    let (AnalysisValueKind::List(values) | AnalysisValueKind::Tuple(values)) = lowered.kind()
    else {
        anyhow::bail!(
            "ctx.actions.run tools must contain Files, FilesToRunProviders, or depsets of Files"
        )
    };
    let mut sources = Vec::with_capacity(values.len());
    for value in values.iter() {
        match value.kind() {
            AnalysisValueKind::Artifact(artifact) => {
                reject_directory_artifact(artifact, "ctx.actions.run tools")?;
                sources.push(ArtifactInputSource::Direct(artifact.clone()));
                if let Some(source) = associated_tool_sources(
                    provenance,
                    scope,
                    artifact,
                    "ctx.actions.run direct tool",
                )? {
                    sources.push(source);
                }
            }
            AnalysisValueKind::Depset(depset) => {
                let retained = RetainedArtifactInputs::new(depset.clone())
                    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                validate_tool_depset(&retained, scope, provenance, false)?;
                sources.push(ArtifactInputSource::Depset(retained));
            }
            _ => {
                if let Some(provider) = retained_files_to_run_provider(value) {
                    sources.push(ArtifactInputSource::FilesToRun(provider));
                } else {
                    anyhow::bail!(
                        "ctx.actions.run tools entries must be Files, FilesToRunProviders, or depsets of Files"
                    )
                }
            }
        }
    }
    Ok(ArtifactInputs::new(sources))
}
