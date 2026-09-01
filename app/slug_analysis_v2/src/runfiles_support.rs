/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file.
 */

use std::sync::Arc;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;
use slug_build_api_v2::CtxActions;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_build_api_v2::RunfilesSupport;
use slug_build_api_v2::RunfilesSupportActionSpec;
use slug_configuration_v2::HostPathFlavor;
use slug_configuration_v2::RetainedActionEnvironment;

pub(crate) fn complete_runfiles_support(
    providers: ProviderCollection,
    actions: &mut CtxActions,
    owner: &AnalysisConfiguredTargetKey,
    packages: &RunfilesPackageDepset,
    configuration: Result<(HostPathFlavor, RetainedActionEnvironment), String>,
) -> Result<ProviderCollection, String> {
    let info = providers
        .default_info()
        .expect("ProviderCollection requires DefaultInfo");
    let Some(executable) = &info.executable else {
        return Ok(providers);
    };
    let AnalysisArtifact::Derived {
        owner: executable_owner,
        output,
    } = executable
    else {
        return Err("DefaultInfo executable must be a derived File".to_owned());
    };
    if executable_owner != owner || output.kind() != ActionOutputKind::File {
        return Err("DefaultInfo executable must be an owner-created regular File".to_owned());
    }
    if info.default_runfiles.is_empty() {
        return Err("executable or test rule must define nonempty runfiles".to_owned());
    }
    let (path_flavor, environment) = configuration?;
    if path_flavor == HostPathFlavor::Windows {
        return Err("runfiles support is unsupported on Windows".to_owned());
    }

    let executable_path = output.path();
    let artifact = |path: String, kind| AnalysisArtifact::Derived {
        owner: owner.clone(),
        output: ActionOutput::new(path, kind),
    };
    let support = Arc::new(RunfilesSupport {
        runfiles: info.default_runfiles.clone(),
        tree: artifact(
            format!("{executable_path}.runfiles"),
            ActionOutputKind::RunfilesTree,
        ),
        input_manifest: artifact(
            format!("{executable_path}.runfiles_manifest"),
            ActionOutputKind::File,
        ),
        manifest: Some(artifact(
            format!("{executable_path}.runfiles/MANIFEST"),
            ActionOutputKind::File,
        )),
        repo_mapping_manifest: Some(artifact(
            format!("{executable_path}.repo_mapping"),
            ActionOutputKind::File,
        )),
    });
    let completed = info
        .with_runfiles_support(support.clone())
        .map_err(|error| error.to_string())?;
    let specs = RunfilesSupportActionSpec::default_actions(
        support,
        packages.clone(),
        path_flavor,
        environment,
    )?
    .map(ActionSpec::runfiles_support);
    actions
        .register_batch(specs)
        .map_err(|error| error.to_string())?;
    Ok(providers.with_default_info(completed))
}
