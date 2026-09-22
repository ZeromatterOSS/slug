//! Pure NOOP-mapped command observations. These are not execution projections.
use compact_str::CompactString;
use slug_configuration_v2::CanonicalStringMap;

use super::CppCompileSpec;
use crate::cc_features::CcFeatureError;
use crate::cc_variables::OwnedCcVariableAccess;

impl CppCompileSpec {
    /// Expand the admitted no-paramfile, always-pass-filter command recipe.
    /// Does not discover inputs, authorize execution or mutate retained Variables.
    pub fn expand_command_line(&self) -> Result<Vec<CompactString>, CcFeatureError> {
        let data = self.data();
        // CppCompileAction.getOverwrittenVariables returns an empty Variables
        // even without modules. CompileCommandLine consequently creates a fresh
        // child scope: direct bindings are inherited, dotted cache seeds are not.
        let mut variables = OwnedCcVariableAccess::fresh_overlay(&data.variables);
        let options = data
            .feature_configuration
            .get_command_line(&data.action_name, &mut variables)?;
        let mut arguments = Vec::with_capacity(options.len() + 1);
        arguments.push(data.selected_tool.clone());
        arguments.extend(options);
        Ok(arguments)
    }

    /// Resolve effective environment key/value pairs using only explicit client
    /// inputs. This is not Starlark Action.env or an execution admission.
    pub fn effective_environment(
        &self,
        client_env: &CanonicalStringMap,
    ) -> Result<CanonicalStringMap, CcFeatureError> {
        let data = self.data();
        // Unlike argv, CompileCommandLine.getEnvironment uses the original
        // Variables and its cache seeds. New lookup state remains call-local.
        let mut variables = OwnedCcVariableAccess::original(&data.variables);
        let feature_env = data
            .feature_configuration
            .get_environment_variables(&data.action_name, &mut variables)?;
        let inherited = data
            .environment
            .inherited()
            .iter()
            .filter_map(|name| client_env.get(name).map(|value| (name, value)));
        let pwd = data
            .execution_requirements
            .get("requires-darwin")
            .is_none()
            .then_some(("PWD", "/proc/self/cwd"));
        Ok(CanonicalStringMap::from_pairs(
            data.environment
                .fixed()
                .iter()
                .chain(inherited)
                .chain(pwd)
                .map(|(key, value)| (CompactString::from(key), CompactString::from(value)))
                .chain(feature_env),
        ))
    }
}

#[cfg(test)]
mod tests;
