//! C++ variable publication uses the ordinary memo, cycle, and depset bridge.
use slug_build_api_v2::cc_variables::CcToolchainVariables;
use slug_build_api_v2::cc_variables::CcVariablesModel;

use super::*;

impl AnalysisValueMaterializer<'_> {
    pub(super) fn cc_variables(
        &mut self,
        value: &CcToolchainVariables,
    ) -> Result<FrozenValue, String> {
        if let Some(frozen) = self.cc_variables.get(value.occurrence()) {
            return Ok(*frozen);
        }
        let model = value.model();
        let parent = model
            .parent
            .as_ref()
            .map(|parent| self.cc_variables(parent))
            .transpose()?;
        let locals = model
            .locals
            .iter()
            .map(|(name, value)| Ok((name.clone(), self.value(value)?)))
            .collect::<Result<Vec<_>, String>>()?;
        let cache = model
            .cache
            .iter()
            .map(|(name, entry)| Ok((name.clone(), entry.try_map(|value| self.value(value))?)))
            .collect::<Result<Vec<_>, String>>()?;
        let frozen = slug_loading_v2::alloc_frozen_cc_variables(
            self.heap,
            model.kind,
            model.occurrence.clone(),
            parent,
            locals,
            cache,
        );
        self.cc_variables.insert(model.occurrence.clone(), frozen);
        Ok(frozen)
    }
}

impl<'v> AnalysisValueLowerer<'v> {
    pub(super) fn lower_cc_variables(
        &mut self,
        variables: slug_loading_v2::CcVariablesView<'v>,
        path: &str,
    ) -> Result<AnalysisValue, String> {
        let parent = variables
            .parent
            .map(|parent| {
                let parent = self.lower(parent, &format!("{path}.parent"))?;
                match parent.kind() {
                    AnalysisValueKind::CcToolchainVariables(value) => Ok(value.clone()),
                    _ => Err(format!(
                        "{path}: C++ variables parent is not CcToolchainVariables"
                    )),
                }
            })
            .transpose()?;
        let locals = variables
            .locals
            .into_iter()
            .map(|(name, value)| {
                let value = self.lower(value, &format!("{path}.vars[{name}]"))?;
                Ok((name, value))
            })
            .collect::<Result<Box<[_]>, String>>()?;
        let cache = variables
            .cache
            .into_iter()
            .map(|(name, entry)| {
                let entry =
                    entry.try_map(|value| self.lower(*value, &format!("{path}.cache[{name}]")))?;
                Ok((name, entry))
            })
            .collect::<Result<Box<[_]>, String>>()?;
        Ok(AnalysisValue::cc_toolchain_variables(
            CcToolchainVariables::new(CcVariablesModel {
                kind: variables.kind,
                occurrence: variables.occurrence,
                parent,
                locals,
                cache,
            }),
        ))
    }
}

#[cfg(test)]
#[path = "cc_variables/tests.rs"]
mod tests;
