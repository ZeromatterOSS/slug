//! Exact support validation at the existing prerequisite producer boundary.

use slug_build_api_v2::RetainedArtifactInputs;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_build_api_v2::SpawnSpec;

use super::*;
use crate::runtime::dice::source_staging::visit_files_to_run;

pub(super) fn require_tree_producer<'a>(
    artifact: &AnalysisArtifact,
    action: &'a ConfiguredAction,
) -> Result<&'a Arc<RunfilesSupport>, Arc<str>> {
    let Some(RunfilesSupportActionSpec::RunfilesTree { support, output }) =
        action.runfiles_support_spec()
    else {
        return Err("unsupported runfiles tree producer family".into());
    };
    if support.manifest.is_none() || support.repo_mapping_manifest.is_none() {
        return Err(
            "runfiles tree support requires public and repository mapping manifests".into(),
        );
    }
    let AnalysisArtifact::Derived {
        owner,
        output: declared,
    } = artifact
    else {
        return Err("runfiles tree requires an exact generated artifact".into());
    };
    if artifact != &support.tree
        || declared != output
        || declared.kind() != ActionOutputKind::RunfilesTree
        || owner != &action.context().owner().artifact_owner()
        || action.outputs() != [output.clone()]
    {
        return Err("runfiles tree differs from its exact producer support".into());
    }
    Ok(support)
}

pub(super) fn validate_providers<'a>(
    spawn: &SpawnSpec,
    resolve: impl Fn(&AnalysisArtifact) -> Result<&'a ConfiguredAction, Arc<str>>,
) -> Result<(), Arc<str>> {
    visit_files_to_run(spawn, |provider| {
        let Some(support) = &provider.support else {
            return Ok(());
        };
        let retained = require_tree_producer(&support.tree, resolve(&support.tree)?)?;
        if !Arc::ptr_eq(support, retained) && support != retained {
            return Err("FilesToRun support differs from its exact runfiles producer".into());
        }
        let executable = provider
            .executable
            .as_ref()
            .ok_or_else(|| Arc::<str>::from("FilesToRun runfiles support has no executable"))?;
        let mut found = false;
        RetainedArtifactInputs::new(support.runfiles.files.clone())
            .map_err(|error| Arc::<str>::from(error.to_string()))?
            .visit(|artifact| {
                found |= artifact == executable;
            })
            .map_err(|error| Arc::<str>::from(error.to_string()))?;
        if !found {
            return Err("FilesToRun executable is absent from its runfiles support".into());
        }
        Ok(())
    })
}
