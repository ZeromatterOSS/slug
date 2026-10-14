//! Complete borrowed owner input frontier and phase-local quota accounting.

use super::*;

#[cfg(test)]
mod tests;

/// A phase-local borrowed audit. No semantic identity uses its accounting or
/// pointer memo; they only prevent retaining inputs beyond publication limits.
#[derive(Default)]
pub(super) struct Audit {
    pub(super) native: PublishedValueArenaBuilder,
    pub(super) runfiles_stores:
        slug_build_api_v2::DepsetStorageVisitSet<Arc<RunfilesPackageMetadata>>,
}

impl Audit {
    pub(super) fn bytes(&mut self, bytes: usize) -> Result<(), String> {
        self.native.charge_visit().map_err(|e| e.to_string())?;
        self.native.charge_bytes(bytes).map_err(|e| e.to_string())
    }
    pub(super) fn row<T>(&mut self) -> Result<(), String> {
        self.bytes(size_of::<T>())
    }
    pub(super) fn text(&mut self, value: &str) -> Result<(), String> {
        self.bytes(value.len().saturating_add(size_of::<CompactString>()))
    }
    pub(super) fn depth(&mut self, depth: usize) -> Result<(), String> {
        self.native.observe_depth(depth).map_err(|e| e.to_string())
    }
    pub(super) fn edge(&mut self) -> Result<(), String> {
        self.native.charge_owner_edge().map_err(|e| e.to_string())?;
        self.row::<usize>()
    }
    pub(super) fn label(&mut self, label: &slug_identity_v2::CanonicalLabel) -> Result<(), String> {
        self.row::<slug_identity_v2::CanonicalLabel>()?;
        self.text(label.package().repo().as_str())?;
        self.text(label.package().package().as_str())?;
        self.text(label.target().as_str())?;
        if let Some(mapping) = label.mapping_id() {
            self.text(mapping.as_str())?;
        }
        Ok(())
    }
    pub(super) fn key(&mut self, key: &ConfiguredTargetKey) -> Result<(), String> {
        self.row::<ConfiguredTargetKey>()?;
        self.label(key.label())?;
        if let Some(config) = key.configuration().slug_configuration() {
            self.bytes(config.canonical_bytes().len())?;
        } else {
            // The legacy key remains structural too: budget its complete typed
            // option data without constructing its serialization during audit.
            self.text(
                key.configuration()
                    .checksum()
                    .expect("legacy checksum")
                    .as_str(),
            )?;
            for option in key.configuration().starlark_options().iter() {
                use slug_configuration_v2::StarlarkOptionValue as V;
                self.row::<slug_configuration_v2::StarlarkOption>()?;
                self.label(option.label())?;
                match option.value() {
                    V::Label(value) => self.label(value)?,
                    V::Integer(value) => {
                        self.bytes((value.bits() as usize).saturating_add(7) / 8 + 1)?
                    }
                    V::Boolean(_) => {}
                    V::String(value) => self.text(value)?,
                    V::StringList(values) | V::StringSet(values) => {
                        for value in &**values {
                            self.text(value)?;
                        }
                    }
                }
            }
        }
        if let Some(platform) = key.toolchain_execution_platform() {
            self.label(platform)?;
        }
        Ok(())
    }
    pub(super) fn node_key(&mut self, key: &ConfiguredNodeKey) -> Result<(), String> {
        match key {
            ConfiguredNodeKey::Configured(key) => self.key(key),
            ConfiguredNodeKey::Null(label) => self.label(label),
        }
    }
    pub(super) fn callable(&mut self, value: FrozenValue) -> Result<(), String> {
        self.row::<CallableWitness>()?;
        let inspection = inspect_function_symbol(value.to_value()).ok_or_else(|| {
            "configured implementation requires an actual Starlark Def".to_owned()
        })?;
        if let FunctionSymbolStateRef::Global { name } = inspection.state() {
            self.text(name)?;
        }
        let owner = slug_loading_v2::function_owner::function_symbol_owner_ref(value.to_value())
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                "configured implementation requires a structural function owner".to_owned()
            })?;
        self.native
            .audit_owner_shape_at_depth(owner, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(super) fn coerced(
        &mut self,
        value: &CoercedAttributeValue,
        depth: usize,
    ) -> Result<(), String> {
        use CoercedAttributeValue as V;
        self.depth(depth)?;
        self.row::<CoercedAttributeValue>()?;
        match value {
            V::None | V::Boolean(_) | V::Integer(_) => {}
            V::Label(value) | V::Output(value) => self.label(value)?,
            V::LabelList(values) | V::OutputList(values) => {
                for value in &**values {
                    self.edge()?;
                    self.label(value)?;
                }
            }
            V::String(value) => self.text(value)?,
            V::StringList(values) => {
                for value in &**values {
                    self.edge()?;
                    self.text(value)?;
                }
            }
            V::IntegerList(values) => {
                for _ in &**values {
                    self.edge()?;
                    self.row::<i32>()?;
                }
            }
            V::StringDict(values) => {
                for (key, value) in &**values {
                    self.edge()?;
                    self.text(key)?;
                    self.text(value)?;
                }
            }
            V::StringListDict(values) => {
                for (key, values) in &**values {
                    self.edge()?;
                    self.text(key)?;
                    for value in &**values {
                        self.edge()?;
                        self.text(value)?;
                    }
                }
            }
            V::StringKeyedLabelDict(values) => {
                for (key, value) in &**values {
                    self.edge()?;
                    self.text(key)?;
                    self.label(value)?;
                }
            }
            V::LabelKeyedStringDict(values) => {
                for (key, value) in &**values {
                    self.edge()?;
                    self.label(key)?;
                    self.text(value)?;
                }
            }
            V::LabelListDict(values) => {
                for (key, values) in &**values {
                    self.edge()?;
                    self.text(key)?;
                    for value in &**values {
                        self.edge()?;
                        self.label(value)?;
                    }
                }
            }
            V::Selector { branches, default } => {
                for (key, value) in &**branches {
                    self.edge()?;
                    self.label(key)?;
                    self.coerced(value, depth + 1)?;
                }
                if let Some(value) = default {
                    self.edge()?;
                    self.coerced(value, depth + 1)?;
                }
            }
            V::Concatenation(left, right) => {
                self.edge()?;
                self.coerced(left, depth + 1)?;
                self.edge()?;
                self.coerced(right, depth + 1)?;
            }
        }
        Ok(())
    }
    pub(super) fn group(&mut self, group: &ConfiguredExecGroup) -> Result<(), String> {
        self.row::<ConfiguredExecGroup>()?;
        match group {
            ConfiguredExecGroup::Default => Ok(()),
            ConfiguredExecGroup::Named(name) => self.text(name),
            ConfiguredExecGroup::Automatic(label) => self.label(label),
        }
    }
    pub(super) fn toolchain(
        &mut self,
        context: &crate::result::ConfiguredActionToolchainContext,
    ) -> Result<(), String> {
        self.row::<crate::result::ConfiguredActionToolchainContext>()?;
        self.key(context.execution_platform())?;
        for row in context.rows() {
            self.edge()?;
            self.row::<crate::result::ConfiguredToolchainContextRow>()?;
            self.key(row.requested())?;
            self.key(row.actual())?;
            if let Some(selected) = row.selected() {
                self.row::<crate::result::ConfiguredToolchainSelection>()?;
                self.label(selected.declaration())?;
                self.key(selected.implementation())?;
                self.key(selected.actual_implementation())?;
                self.row::<AnalysisValue>()?;
                self.native
                    .audit_owner_provider_occurrence(selected.info())
                    .map_err(|e| e.to_string())?;
                if let Some(info) = selected.template_variable_info() {
                    self.row::<AnalysisValue>()?;
                    self.row::<slug_build_api_v2::TemplateVariableInfo>()?;
                    for (key, value) in info.variables().iter() {
                        self.edge()?;
                        self.text(key)?;
                        self.text(value)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn action_context(
        &mut self,
        context: &ConfiguredActionOwnerContext,
    ) -> Result<(), String> {
        self.row::<ConfiguredActionOwnerContext>()?;
        self.key(context.owner())?;
        self.group(context.exec_group())?;
        if let Some(platform) = context.execution_platform() {
            self.key(platform)?;
        }
        for fact in [context.platform_fact(), context.raw_platform_fact()]
            .into_iter()
            .flatten()
        {
            self.row::<crate::result::PlatformSemanticFact>()?;
            for (key, value) in &*fact.exec_properties {
                self.edge()?;
                self.text(key)?;
                self.text(value)?;
            }
            if let Some(error) = &fact.missing_toolchain_error {
                self.text(error)?;
            }
        }
        for constraint in context.platform_constraints() {
            self.edge()?;
            self.row::<crate::result::ConfiguredActionPlatformConstraint>()?;
            self.key(constraint.constraint_value())?;
            self.key(constraint.constraint_setting())?;
            if let Some(label) = constraint.default_constraint_value() {
                self.label(label)?;
            }
        }
        if let Some(toolchain) = context.toolchain() {
            self.toolchain(toolchain)?;
        }
        Ok(())
    }
    pub(super) fn runfiles(
        &mut self,
        packages: &RunfilesPackageDepset,
        depth: usize,
    ) -> Result<(), String> {
        self.depth(depth)?;
        self.edge()?;
        // One owning memo spans every package root in this owner. The physical
        // visitor charges before retaining a store guard or growing traversal
        // scratch, and sees hidden rows/leaves as well as the selected root.
        let mut stores = std::mem::take(&mut self.runfiles_stores);
        let result = packages.visit_storage_bounded_with(&mut stores, depth, |event| match event {
            slug_build_api_v2::DepsetStorageEvent::Charge(charge) => self
                .native
                .charge_storage(charge)
                .map_err(|e| e.to_string()),
            slug_build_api_v2::DepsetStorageEvent::Node { depth, .. } => self.depth(depth),
            slug_build_api_v2::DepsetStorageEvent::Value { value, depth } => {
                self.depth(depth)?;
                self.row::<RunfilesPackageMetadata>()?;
                self.text(value.package().repo().as_str())?;
                self.text(value.package().package().as_str())?;
                for (apparent, canonical) in value.mapping().entries() {
                    self.edge()?;
                    self.text(apparent.as_str())?;
                    self.text(canonical.as_str())?;
                }
                if let Some(group) = value.mapping().compact_group() {
                    self.text(group)?;
                }
                Ok(())
            }
        });
        // Keep the poisoned memo too: partial audits must never become reusable
        // successes, and every retained address continues to have its guard.
        self.runfiles_stores = stores;
        result.map_err(|e| e.to_string())
    }
}
