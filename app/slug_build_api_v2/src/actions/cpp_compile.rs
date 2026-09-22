//! Immutable C++ compile construction recipe. Expansion and header discovery
//! are deliberately not a Spawn projection. Live Values are lowered at the
//! analysis publication checkpoint before this owner can be constructed.
use std::sync::Arc;

use allocative::Allocative;
use compact_str::CompactString;
use dupe::Dupe;
use slug_configuration_v2::CanonicalStringMap;
use slug_configuration_v2::RetainedActionEnvironment;

use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::AnalysisArtifact;
use crate::AnalysisValue;
use crate::AnalysisValueKind;
use crate::ArtifactInputs;
use crate::CcFeatureConfiguration;
use crate::RetainedCppFragment;
use crate::actions::registry::validate_output;
use crate::analysis_value::PublicationEqState;
use crate::cc_variables::CcToolchainVariables;

mod dependencies;
mod expansion;
mod input_preparation;
mod input_sets;

pub use dependencies::CppCompileDependencies;
pub use input_preparation::CppCompileInputPreparation;
pub use input_sets::CppCompileInputSets;

/// Immutable eagerly consumed constructor inputs, not an action or a recipe.
#[derive(Debug, Clone, Allocative)]
pub struct CppCompileConstruction {
    pub source: AnalysisArtifact,
    /// Object first, followed by the required dotd. Roles borrow this collection.
    pub outputs: Box<[ActionOutput]>,
    pub action_name: CompactString,
    pub feature_configuration: CcFeatureConfiguration,
    pub mandatory_spawn_inputs: ArtifactInputs,
    pub cache_key_inputs: ArtifactInputs,
    pub declared_headers: ArtifactInputs,
    pub builtin_inputs: ArtifactInputs,
    pub additional_prunable_inputs: ArtifactInputs,
    pub environment: RetainedActionEnvironment,
    pub execution_requirements: CanonicalStringMap,
    pub selected_tool: CompactString,
    pub toolchain_cpp: RetainedCppFragment,
    pub builtin_include_directories: Box<[CompactString]>,
    pub use_pic: bool,
    pub needs_include_validation: bool,
}

/// Final, fully owned construction arguments. Both live values have been lowered
/// together with returned providers using the same publication lowerer.
#[derive(Debug, Clone, Allocative)]
pub struct CppCompileData {
    pub construction: CppCompileConstruction,
    pub variables: CcToolchainVariables,
    pub compilation_context: AnalysisValue,
}

impl std::ops::Deref for CppCompileData {
    type Target = CppCompileConstruction;
    fn deref(&self) -> &Self::Target {
        &self.construction
    }
}

#[derive(Debug, Clone, Dupe, Allocative)]
pub struct CppCompileSpec(Arc<CppCompileData>);

impl CppCompileSpec {
    /// Pinned CppFileTypes.C_SOURCE/CPP_SOURCE use case-sensitive suffixes,
    /// including on Windows. Other native source families remain unadmitted.
    pub fn default_action_name(source: &AnalysisArtifact) -> Result<&'static str, String> {
        if let AnalysisArtifact::Derived { output, .. } = source {
            if output.kind() != ActionOutputKind::File {
                return Err("CppCompile requires a regular source File".into());
            }
        }
        let path = source.path();
        if path.ends_with(".c") {
            Ok("c-compile")
        } else if [".cc", ".cpp", ".cxx", ".c++", ".C", ".cu", ".cl"]
            .iter()
            .any(|suffix| path.ends_with(suffix))
        {
            Ok("c++-compile")
        } else {
            Err("CppCompile admits only regular C/C++ source Files".into())
        }
    }

    pub fn new(data: CppCompileData) -> Result<Self, String> {
        Self::validate_construction(&data.construction)?;
        if !matches!(
            data.compilation_context.kind(),
            AnalysisValueKind::Provider(_)
        ) {
            return Err("CppCompile requires an owned compilation context provider".into());
        }
        Ok(Self(Arc::new(data)))
    }

    /// Shared eager admission checks for the live constructor and the final
    /// owned recipe. No Variables expansion or mutable context read occurs here.
    pub fn validate_construction(data: &CppCompileConstruction) -> Result<(), String> {
        Self::default_action_name(&data.source)?;
        if !matches!(data.action_name.as_str(), "c-compile" | "c++-compile") {
            return Err(
                "CppCompile action_name is outside the admitted C/C++ source family".into(),
            );
        }
        if data.outputs.len() != 2 {
            return Err("CppCompile requires an object and dotd output".into());
        }
        for output in &data.outputs {
            validate_output(output).map_err(|error| error.to_string())?;
            if output.kind() != ActionOutputKind::File || output.root() != data.outputs[0].root() {
                return Err("CppCompile outputs require regular Files in one root".into());
            }
        }
        if data.outputs[0] == data.outputs[1] {
            return Err("CppCompile object and dotd must be distinct outputs".into());
        }
        for deferred in [
            "parse_showincludes",
            "use_header_modules",
            "compiler_param_file",
        ] {
            if data.feature_configuration.is_enabled(deferred) {
                return Err(format!("CppCompile feature '{deferred}' is not admitted"));
            }
        }
        if !data
            .feature_configuration
            .action_is_enabled(&data.action_name)
        {
            return Err(format!(
                "Expected action_config for '{}' to be configured",
                data.action_name
            ));
        }
        let selected = data
            .feature_configuration
            .get_tool_for_action(&data.action_name)
            .map_err(|error| error.to_string())?;
        if selected != data.selected_tool {
            return Err("CppCompile selected tool disagrees with feature configuration".into());
        }
        Ok(())
    }

    pub fn data(&self) -> &CppCompileData {
        &self.0
    }
    pub fn source(&self) -> &AnalysisArtifact {
        &self.0.source
    }
    pub fn outputs(&self) -> &[ActionOutput] {
        &self.0.outputs
    }
    pub fn object_output(&self) -> &ActionOutput {
        &self.0.outputs[0]
    }
    pub fn dotd_output(&self) -> &ActionOutput {
        &self.0.outputs[1]
    }
    pub fn mandatory_spawn_inputs(&self) -> &ArtifactInputs {
        &self.0.mandatory_spawn_inputs
    }
    pub fn cache_key_inputs(&self) -> &ArtifactInputs {
        &self.0.cache_key_inputs
    }
    pub fn declared_headers(&self) -> &ArtifactInputs {
        &self.0.declared_headers
    }

    pub(super) fn publication_eq_with(&self, other: &Self, state: &mut PublicationEqState) -> bool {
        let (left, right) = (self.data(), other.data());
        left.source == right.source
            && left.outputs == right.outputs
            && left.action_name == right.action_name
            // Compare complete metadata, not the native occurrence-only Eq.
            && left.feature_configuration.model() == right.feature_configuration.model()
            && state.enter_cc_feature_pair(
                left.feature_configuration.occurrence().pointer(),
                right.feature_configuration.occurrence().pointer(),
            )
            && left.variables.publication_eq_with(&right.variables, state)
            && left.compilation_context.publication_eq_with(&right.compilation_context, state)
            && left.mandatory_spawn_inputs.publication_eq_with(&right.mandatory_spawn_inputs, state)
            && left.cache_key_inputs.publication_eq_with(&right.cache_key_inputs, state)
            && left.declared_headers.publication_eq_with(&right.declared_headers, state)
            && left.builtin_inputs.publication_eq_with(&right.builtin_inputs, state)
            && left.additional_prunable_inputs.publication_eq_with(&right.additional_prunable_inputs, state)
            && left.environment == right.environment
            && left.execution_requirements == right.execution_requirements
            && left.selected_tool == right.selected_tool
            && left.toolchain_cpp.publication_eq_with(&right.toolchain_cpp, state)
            && left.builtin_include_directories == right.builtin_include_directories
            && left.use_pic == right.use_pic
            && left.needs_include_validation == right.needs_include_validation
    }
}

impl PartialEq for CppCompileSpec {
    fn eq(&self, other: &Self) -> bool {
        self.publication_eq_with(other, &mut PublicationEqState::default())
    }
}
impl Eq for CppCompileSpec {}

#[cfg(test)]
mod tests;
