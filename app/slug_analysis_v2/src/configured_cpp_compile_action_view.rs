/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionOutputMetadata;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::ArtifactInputSource;
use slug_build_api_v2::ArtifactInputs;
use slug_build_api_v2::CppCompileData;
use slug_build_api_v2::CppCompileSpec;
use slug_build_api_v2::DepsetOrder;
use slug_configuration_v2::HostPathFlavor;

use crate::ConfiguredAction;
use crate::ConfiguredActionAspectProvenance;
use crate::ConfiguredActionOwnerContext;
use crate::ConfiguredExecGroup;
use crate::key::ConfiguredTargetKey;

/// Borrowed pre-discovery regular C/C++ query metadata, not a Spawn projection.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ConfiguredCppCompileActionView<'a>(&'a ConfiguredAction);

fn require_regular(artifact: &AnalysisArtifact) -> Result<(), &'static str> {
    match artifact {
        AnalysisArtifact::Source(_) => Ok(()),
        AnalysisArtifact::Derived { owner, output } if output.kind() == ActionOutputKind::File => {
            // Input configuration belongs to its own owner, not the consumer.
            // Constant regular inputs are admitted; outputs require Ordinary.
            if output.root().configuration().canonical_bytes() != owner.configuration() {
                return Err("configured CppCompile input root differs from its own owner");
            }
            Ok(())
        }
        _ => Err("configured CppCompile query requires Source or Derived regular File inputs"),
    }
}

fn require_inputs(inputs: &ArtifactInputs) -> Result<(), &'static str> {
    let mut error = None;
    inputs
        .visit(|artifact| {
            if error.is_none() {
                error = require_regular(artifact).err();
            }
        })
        .map_err(|_| "configured CppCompile query requires complete artifact inputs")?;
    error.map_or(Ok(()), Err)
}

impl<'a> ConfiguredCppCompileActionView<'a> {
    pub fn from_configured_action(action: &'a ConfiguredAction) -> Result<Self, &'static str> {
        let spec = action
            .cpp_compile_spec()
            .ok_or("configured CppCompile view requires a typed compile")?;
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("configured CppCompile action requires a selected execution platform");
        }
        if context.raw_platform_fact().is_none() || context.platform_fact().is_none() {
            return Err("configured CppCompile action requires raw and effective platform facts");
        }
        if context.aspect() != ConfiguredActionAspectProvenance::Absent {
            return Err("configured CppCompile query does not support aspect provenance");
        }
        if context.exec_group() != &ConfiguredExecGroup::Default
            || action.exec_group().is_some()
            || !action.exec_properties().is_empty()
        {
            return Err("configured CppCompile query requires default action group and properties");
        }
        let configuration = context
            .owner()
            .configuration()
            .slug_configuration()
            .ok_or("configured CppCompile query requires structural configuration")?;
        if configuration
            .configured_action_path_flavor()
            .map_err(|_| "configured CppCompile query requires retained Host path facts")?
            != HostPathFlavor::Unix
        {
            return Err("configured CppCompile query does not support Windows");
        }
        let data = spec.data();
        if configuration
            .cpp_compile_include_scanning()
            .map_err(|_| "configured CppCompile query requires include-scanning policy")?
            || data
                .feature_configuration
                .is_requested("cc_include_scanning")
            || !configuration
                .cpp_compile_execution_modifiers_empty()
                .map_err(|_| "configured CppCompile query requires execution-modifier policy")?
        {
            return Err(
                "configured CppCompile query requires default no-scanning execution policy",
            );
        }
        for feature in [
            "header_modules",
            "cpp_modules",
            "use_header_modules",
            "parse_showincludes",
            "compiler_param_file",
        ] {
            if data.feature_configuration.is_enabled(feature) {
                return Err(
                    "configured CppCompile query does not support modules or parameter files",
                );
            }
        }
        require_regular(spec.source())?;
        CppCompileSpec::default_action_name(spec.source())
            .map_err(|_| "configured CppCompile query requires a regular C/C++ source")?;
        if !matches!(data.action_name.as_str(), "c-compile" | "c++-compile") {
            return Err("configured CppCompile query requires a C/C++ action name");
        }
        if action.outputs() != spec.outputs()
            || spec.outputs().iter().any(|output| {
                output.kind() != ActionOutputKind::File
                    || output.metadata() != ActionOutputMetadata::Ordinary
                    || output.root().configuration() != configuration
            })
        {
            return Err("configured CppCompile query requires its ordinary object and dotd roots");
        }
        let mandatory = spec.mandatory_spawn_inputs();
        match mandatory.sources() {
            [ArtifactInputSource::Depset(inputs)]
                if matches!(
                    inputs.order(),
                    DepsetOrder::Default | DepsetOrder::Postorder
                ) => {}
            sources
                if sources
                    .iter()
                    .all(|source| matches!(source, ArtifactInputSource::Direct(_))) => {}
            _ => return Err("configured CppCompile mandatory input order is not admitted"),
        }
        let mut has_source = false;
        let mut error = None;
        mandatory
            .visit(|artifact| {
                has_source |= artifact == spec.source();
                if error.is_none() {
                    error = require_regular(artifact).err();
                }
            })
            .map_err(|_| "configured CppCompile query requires complete mandatory inputs")?;
        if let Some(error) = error {
            return Err(error);
        }
        if !has_source {
            return Err("configured CppCompile mandatory inputs must contain its source");
        }
        require_inputs(spec.cache_key_inputs())?;
        require_inputs(spec.declared_headers())?;
        Ok(Self(action))
    }

    pub fn action(&self) -> &'a ConfiguredAction {
        self.0
    }

    pub fn spec(&self) -> &'a CppCompileSpec {
        self.0.cpp_compile_spec().expect("checked typed CppCompile")
    }

    pub fn data(&self) -> &'a CppCompileData {
        self.spec().data()
    }

    pub fn context(&self) -> &'a ConfiguredActionOwnerContext {
        self.0.context().as_ref()
    }

    pub fn owner(&self) -> &'a ConfiguredTargetKey {
        self.context().owner()
    }

    pub fn execution_platform(&self) -> &'a ConfiguredTargetKey {
        self.context()
            .execution_platform()
            .expect("checked selected platform")
    }

    pub fn source(&self) -> &'a AnalysisArtifact {
        self.spec().source()
    }

    pub fn object_output(&self) -> &'a ActionOutput {
        self.spec().object_output()
    }

    pub fn dotd_output(&self) -> &'a ActionOutput {
        self.spec().dotd_output()
    }

    pub fn mandatory_spawn_inputs(&self) -> &'a ArtifactInputs {
        self.spec().mandatory_spawn_inputs()
    }

    pub fn cache_key_inputs(&self) -> &'a ArtifactInputs {
        self.spec().cache_key_inputs()
    }

    pub fn declared_headers(&self) -> &'a ArtifactInputs {
        self.spec().declared_headers()
    }
}
