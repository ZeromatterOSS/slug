//! Pure declared Spawn input traversal; no staging or execution authority.

use std::sync::Arc;

use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::FilesToRunProvider;
use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RetainedSpawnInvocation;
use slug_build_api_v2::SpawnExecutable;
use slug_build_api_v2::SpawnSpec;

fn error(value: impl std::fmt::Display) -> Arc<str> {
    Arc::from(value.to_string())
}

/// Visit retained artifact leaves synchronously, keeping visitor loans local.
/// Virtual parameter files and File-valued Args are not declared input leaves.
pub(in crate::runtime) fn visit_declared_artifacts(
    spawn: &SpawnSpec,
    mut visitor: impl FnMut(&AnalysisArtifact),
) -> Result<(), Arc<str>> {
    fn provider(
        provider: &FilesToRunProvider,
        visitor: &mut impl FnMut(&AnalysisArtifact),
    ) -> Result<(), Arc<str>> {
        let mut has_executable = provider.executable.is_none();
        let mut has_tree = provider.support.is_none();
        RetainedArtifactInputs::new(provider.files().clone())
            .map_err(error)?
            .visit(|artifact| {
                has_executable |= provider.executable.as_ref() == Some(artifact);
                has_tree |= provider
                    .support
                    .as_ref()
                    .is_some_and(|support| &support.tree == artifact);
                visitor(artifact);
            })
            .map_err(error)?;
        if !has_executable || !has_tree {
            return Err(error(
                "FilesToRun files omit its executable or runfiles tree",
            ));
        }
        if provider.support.is_some() && provider.executable.is_none() {
            return Err(error("FilesToRun runfiles support has no executable"));
        }
        Ok(())
    }

    fn inputs(
        inputs: &ArtifactInputs,
        visitor: &mut impl FnMut(&AnalysisArtifact),
    ) -> Result<(), Arc<str>> {
        for input in inputs.sources() {
            if let ArtifactInputSource::FilesToRun(value) = input {
                provider(value, visitor)?;
            }
        }
        // Provider leaves were validated and visited above. Repeating them
        // cannot add a distinct declared artifact and repeats path rendering.
        for input in inputs.sources() {
            match input {
                ArtifactInputSource::Direct(artifact) => visitor(artifact),
                ArtifactInputSource::Depset(values) => {
                    values.visit(&mut *visitor).map_err(error)?;
                }
                ArtifactInputSource::FilesToRun(_) => {}
            }
        }
        Ok(())
    }

    inputs(spawn.inputs(), &mut visitor)?;
    inputs(spawn.tools(), &mut visitor)?;
    match spawn.invocation() {
        RetainedSpawnInvocation::Executable(SpawnExecutable::Artifact(artifact)) => {
            visitor(artifact);
        }
        RetainedSpawnInvocation::Executable(SpawnExecutable::FilesToRun(value)) => {
            provider(value, &mut visitor)?;
            if value.executable.is_none() {
                return Err(error("source staging requires a declared executable"));
            }
        }
        // Selected-platform paths and Shell executables are not artifact leaves.
        RetainedSpawnInvocation::Executable(SpawnExecutable::Path(_))
        | RetainedSpawnInvocation::Shell { .. } => {}
    }
    Ok(())
}
