//! Bazel 9.2 CcToolchainFeatures expansion with NOOP path mapping and no
//! execution-time artifact metadata. Live lookup/cache ownership stays in the
//! adapter; this layer never retains evaluator values or expansion scratch.
use super::*;

/// Access to either evaluator-live or retained variables. Lookup includes the
/// native direct-name/longest-prefix and structured-cache semantics. `None`
/// means unavailable, including a local None shadowing a parent binding.
pub trait CcVariableAccess: Sized {
    type Value: Clone;
    type Sequence: Iterator<Item = Result<Self::Value, CcFeatureError>>;

    fn lookup(
        &mut self,
        name: &str,
        throw_on_missing: bool,
    ) -> Result<Option<Self::Value>, CcFeatureError>;
    fn string(&mut self, value: &Self::Value, name: &str) -> Result<CompactString, CcFeatureError>;
    fn sequence(
        &mut self,
        value: &Self::Value,
        name: &str,
    ) -> Result<Self::Sequence, CcFeatureError>;
    fn truthy(&mut self, value: &Self::Value) -> Result<bool, CcFeatureError>;

    /// A fresh native Single scope, including its own dotted lookup cache.
    /// The binding is already an adapted value, not a raw sequence container.
    fn with_binding(&self, name: &str, value: Self::Value) -> Self;
}

impl CcFeatureConfiguration {
    pub fn action_is_enabled(&self, action: &str) -> bool {
        self.model.enabled_action_configs.contains(action)
    }

    fn selected_tool(&self, action: &str) -> Result<&CcTool, CcFeatureError> {
        // Native actionConfigByActionName contains all declared actions, even
        // disabled ones; only command-line action flags use the enabled gate.
        let config = self
            .model
            .model
            .action_configs
            .iter()
            .find(|config| config.action_name == action)
            .ok_or_else(|| {
                invalid(format!(
                    "Action {action} does not have an enabled configuration in the toolchain."
                ))
            })?;
        config
            .tools
            .iter()
            .find(|tool| self.predicates_match(&tool.with_features))
            .ok_or_else(|| {
                invalid(format!(
                    "Matching tool for action {action} not found for given feature configuration"
                ))
            })
    }

    pub fn get_tool_for_action(&self, action: &str) -> Result<CompactString, CcFeatureError> {
        let tool = self.selected_tool(action)?;
        let path = match tool.origin() {
            CcToolPathOrigin::CrosstoolPackage => {
                let directory = &self.model.model.tools_directory;
                if directory.is_empty() {
                    normalize_posix_path(tool.path())?
                } else {
                    normalize_posix_path(&format!("{directory}/{}", tool.path()))?
                }
            }
            CcToolPathOrigin::FilesystemRoot | CcToolPathOrigin::WorkspaceRoot => {
                tool.path().into()
            }
        };
        // PathFragment.getSafePathString spells the empty fragment as '.'.
        Ok(if path.is_empty() { ".".into() } else { path })
    }

    pub fn get_execution_requirements(
        &self,
        action: &str,
    ) -> Result<Vec<CompactString>, CcFeatureError> {
        Ok(self
            .selected_tool(action)?
            .execution_requirements
            .iter()
            .cloned()
            .collect())
    }

    pub fn get_command_line(
        &self,
        action: &str,
        variables: &mut impl CcVariableAccess,
    ) -> Result<Vec<CompactString>, CcFeatureError> {
        let mut output = Vec::new();
        if self.action_is_enabled(action) {
            let config = self
                .model
                .model
                .action_configs
                .iter()
                .find(|config| config.action_name == action)
                .expect("enabled action was validated during selection");
            self.expand_flag_sets(&config.flag_sets, action, variables, &mut output)?;
        }
        for feature in &self.model.model.features {
            if self.is_enabled(&feature.name) {
                self.expand_flag_sets(&feature.flag_sets, action, variables, &mut output)?;
            }
        }
        Ok(output)
    }

    pub fn get_environment_variables(
        &self,
        action: &str,
        variables: &mut impl CcVariableAccess,
    ) -> Result<Vec<(CompactString, CompactString)>, CcFeatureError> {
        let mut output = Vec::new();
        for feature in &self.model.model.features {
            if !self.is_enabled(&feature.name) {
                continue;
            }
            for set in &feature.env_sets {
                if !set.actions.contains(action) || !self.predicates_match(&set.with_features) {
                    continue;
                }
                for entry in &set.env_entries {
                    if let Some(name) = &entry.expand_if_available {
                        if variables.lookup(name, false)?.is_none() {
                            continue;
                        }
                    }
                    output.push((entry.key.clone(), expand_chunks(&entry.value, variables)?));
                }
            }
        }
        // Native ImmutableMap.Builder detects conflicts at buildOrThrow, after
        // every included environment value has been expanded.
        let mut keys = SmallSet::new();
        for (key, _) in &output {
            if !keys.insert(key.as_str()) {
                return Err(invalid(format!("Multiple entries with same key: {key}")));
            }
        }
        drop(keys);
        Ok(output)
    }

    fn predicates_match(&self, predicates: &CcSet<CcWithFeatureSet>) -> bool {
        predicates.is_empty()
            || predicates.iter().any(|predicate| {
                predicate.features.iter().all(|name| self.is_enabled(name))
                    && predicate
                        .not_features
                        .iter()
                        .all(|name| !self.is_enabled(name))
            })
    }

    fn expand_flag_sets(
        &self,
        sets: &[CcFlagSet],
        action: &str,
        variables: &mut impl CcVariableAccess,
        output: &mut Vec<CompactString>,
    ) -> Result<(), CcFeatureError> {
        for set in sets {
            if !self.predicates_match(&set.with_features) || !set.actions.contains(action) {
                continue;
            }
            for group in &set.flag_groups {
                expand_group(group, variables, output)?;
            }
        }
        Ok(())
    }
}

fn required<A: CcVariableAccess>(
    variables: &mut A,
    name: &str,
) -> Result<A::Value, CcFeatureError> {
    variables
        .lookup(name, true)?
        .ok_or_else(|| invalid(format!("Cannot find variable named '{name}'.")))
}

fn expand_chunks(
    chunks: &[CcStringChunk],
    variables: &mut impl CcVariableAccess,
) -> Result<CompactString, CcFeatureError> {
    let mut output = String::new();
    for chunk in chunks {
        match chunk {
            CcStringChunk::Literal(value) | CcStringChunk::RelativePath(value) => {
                // RelativePathChunk uses PathMapper.NOOP, not tools_directory.
                output.push_str(value);
            }
            CcStringChunk::Variable(name) => {
                let value = required(variables, name)?;
                output.push_str(&variables.string(&value, name)?);
            }
        }
    }
    Ok(output.into())
}

fn expand_group(
    group: &CcFlagGroup,
    variables: &mut impl CcVariableAccess,
    output: &mut Vec<CompactString>,
) -> Result<(), CcFeatureError> {
    if let Some(name) = &group.expand_if_available {
        if variables.lookup(name, false)?.is_none() {
            return Ok(());
        }
    }
    if let Some(name) = &group.expand_if_not_available {
        if variables.lookup(name, false)?.is_some() {
            return Ok(());
        }
    }
    if let Some(name) = &group.expand_if_true {
        if variables.lookup(name, false)?.is_none() {
            return Ok(());
        }
        let value = required(variables, name)?;
        if !variables.truthy(&value)? {
            return Ok(());
        }
    }
    if let Some(name) = &group.expand_if_false {
        if variables.lookup(name, false)?.is_none() {
            return Ok(());
        }
        let value = required(variables, name)?;
        if variables.truthy(&value)? {
            return Ok(());
        }
    }
    if let Some(equal) = &group.expand_if_equal {
        if variables.lookup(&equal.name, false)?.is_none() {
            return Ok(());
        }
        let value = required(variables, &equal.name)?;
        if variables.string(&value, &equal.name)? != equal.value {
            return Ok(());
        }
    }
    if let Some(name) = &group.iterate_over {
        let value = required(variables, name)?;
        for value in variables.sequence(&value, name)? {
            // Native Sequence adapts each raw element when its iterator advances,
            // before this element's child guards, not before earlier expansions.
            let mut nested = variables.with_binding(name, value?);
            expand_children(&group.expandables, &mut nested, output)?;
        }
    } else {
        expand_children(&group.expandables, variables, output)?;
    }
    Ok(())
}

fn expand_children(
    children: &[CcExpandable],
    variables: &mut impl CcVariableAccess,
    output: &mut Vec<CompactString>,
) -> Result<(), CcFeatureError> {
    for child in children {
        match child {
            CcExpandable::Flag(chunks) => output.push(expand_chunks(chunks, variables)?),
            CcExpandable::Group(group) => expand_group(group, variables, output)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
