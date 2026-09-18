//! Closure-owned typed Spawn serialization. This plan cannot authorize Execute.

use std::collections::BTreeMap;
use std::sync::Arc;

use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::actions::registry::validate_output;
use slug_core_v2::runtime::PreparedSourceActionInputs;

use crate::ReapiActionIdentity;
use crate::ReapiCommand;
use crate::SourceInputReapiPlan;

#[derive(Debug, Clone)]
pub struct SourceSpawnReapiPlan {
    inputs: SourceInputReapiPlan,
    command: ReapiCommand,
    identity: ReapiActionIdentity,
}

impl SourceSpawnReapiPlan {
    /// Serialize the selected producer and its complete source-only input tree.
    /// Before future execution, the request must select an execution representative,
    /// establish completed verified staging and revalidate its full source frontier.
    pub fn from_prepared(
        prepared: Arc<PreparedSourceActionInputs>,
        remote_defaults: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let action = prepared.configured_action();
        let context = action.context();
        if context.execution_platform().is_none() {
            return Err("Spawn REAPI plan requires a selected execution platform".to_owned());
        }
        let raw = context
            .raw_platform_fact()
            .ok_or("Spawn REAPI plan requires the raw platform fact")?;
        let effective = context
            .platform_fact()
            .ok_or("Spawn REAPI plan requires the effective platform fact")?;
        if !action.exec_properties().is_empty() {
            return Err("Spawn REAPI plan rejects legacy action exec_properties".to_owned());
        }
        let spawn = prepared.spawn();
        if spawn.environment().inherited().iter().next().is_some() {
            return Err("Spawn REAPI plan requires resolved inherited environment".to_owned());
        }
        if spawn.execution_requirements().iter().next().is_some() {
            return Err("Spawn REAPI execution requirements are not admitted".to_owned());
        }
        let mut platform_properties = if raw.exec_properties.is_empty() {
            remote_defaults.clone()
        } else {
            BTreeMap::new()
        };
        // PlatformUtils falls back to the raw platform when the combined map
        // is empty (for example, all raw properties belong to another group).
        let properties = if effective.exec_properties.is_empty() {
            &raw.exec_properties
        } else {
            &effective.exec_properties
        };
        platform_properties.extend(
            properties
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string())),
        );
        let env = spawn
            .environment()
            .fixed()
            .iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        let output_files = regular_output_paths(spawn.outputs())?;
        let mut inputs = SourceInputReapiPlan::from_prepared(prepared)?;
        for output in &output_files {
            for input in inputs.input_tree().entries() {
                if paths_conflict(output, input.path()) {
                    return Err(format!(
                        "Spawn REAPI output {output} conflicts with input {}",
                        input.path()
                    ));
                }
            }
        }
        let command = ReapiCommand {
            argv: inputs.take_expanded_argv(),
            env,
            output_files,
            output_directories: Vec::new(),
            platform_properties,
        };
        let identity =
            ReapiActionIdentity::new(&command, inputs.input_tree().root_digest().clone(), None);
        Ok(Self {
            inputs,
            command,
            identity,
        })
    }

    pub fn inputs(&self) -> &SourceInputReapiPlan {
        &self.inputs
    }

    pub fn command(&self) -> &ReapiCommand {
        &self.command
    }

    pub fn identity(&self) -> &ReapiActionIdentity {
        &self.identity
    }
}

fn regular_output_paths(outputs: &[ActionOutput]) -> Result<Vec<String>, String> {
    if outputs.is_empty() {
        return Err("Spawn REAPI plan requires declared file outputs".to_owned());
    }
    let mut paths = Vec::with_capacity(outputs.len());
    for output in outputs {
        if output.kind() != ActionOutputKind::File {
            return Err("Spawn REAPI plan requires regular file outputs".to_owned());
        }
        validate_output(output).map_err(|error| error.to_string())?;
        paths.push(output.path().to_owned());
    }
    paths.sort();
    Ok(paths)
}

fn paths_conflict(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|tail| tail.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|tail| tail.starts_with('/'))
}

#[cfg(test)]
mod tests;
