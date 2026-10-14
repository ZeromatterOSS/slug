//! Borrowed configured-input families, before any snapshot is retained.
use super::*;

impl Audit {
    pub(super) fn dependency_inputs(
        &mut self,
        dependencies: &[PreparedDependency],
        configured_attributes: &[PreparedConfiguredAttribute],
        resolved_attributes: &[ResolvedRuleAttribute],
    ) -> Result<(), String> {
        for dependency in dependencies {
            self.edge()?;
            self.row::<DependencyMetadata>()?;
            self.node_key(&dependency.key)?;
            // The native target also owns a serialized complete key alongside the
            // metadata key. Forecast it independently before constructing either.
            self.node_key(&dependency.key)?;
            self.row::<ConfiguredTargetValue>()?;
            self.row::<AnalysisValue>()?;
            self.row::<AnalysisValue>()?;
            self.text(&dependency.attribute)?;
            for (_, value) in dependency.providers.iter() {
                self.edge()?;
                self.native
                    .audit_owner_provider_value(value)
                    .map_err(|e| e.to_string())?;
            }
            if let Some(executable) = &dependency.executable {
                self.native
                    .audit_owner_files_to_run(executable)
                    .map_err(|e| e.to_string())?;
            }
        }
        for attribute in configured_attributes {
            self.edge()?;
            self.row::<ConfiguredAttributeMetadata>()?;
            if let Some(owner) = &attribute.owner {
                self.row::<SubruleIdentity>()?;
                self.label(owner.defining_label())?;
                self.text(owner.exported_name())?;
            }
            if let Some(name) = &attribute.user_name {
                self.text(name)?;
            }
            self.native
                .audit_owner_native(&attribute.value)
                .map_err(|e| e.to_string())?;
        }
        for attribute in resolved_attributes {
            self.edge()?;
            self.row::<ResolvedAttributeMetadata>()?;
            self.text(&attribute.declaration_name)?;
            self.coerced(&attribute.value, 0)?;
        }
        Ok(())
    }
    pub(super) fn execution_inputs(
        &mut self,
        exec_groups: &ConfiguredExecGroupCollection,
    ) -> Result<(), String> {
        self.row::<ConfiguredExecGroupCollection>()?;
        for row in exec_groups.rows() {
            self.edge()?;
            self.row::<crate::execution_groups::ConfiguredExecGroupRow>()?;
            self.group(row.identity())?;
            for requirement in row.requirements() {
                self.edge()?;
                self.label(requirement.label())?;
            }
            for label in row.constraints() {
                self.edge()?;
                self.label(label)?;
            }
            self.action_context(row.action_context())?;
            for closure in row.owner_runfiles_packages() {
                self.edge()?;
                self.row::<crate::result::RunfilesPackageClosureRow>()?;
                self.node_key(closure.owner_key())?;
                self.runfiles(closure.owner_packages(), 0)?;
            }
        }
        for requirement in exec_groups.default_declared_requirements() {
            self.edge()?;
            self.label(requirement.label())?;
        }
        let topology = exec_groups.toolchain_topology();
        self.row::<crate::result::ToolchainTopology>()?;
        for platform in topology.candidate_execution_platforms() {
            self.edge()?;
            self.key(platform)?;
        }
        if let Some(platform) = topology.known_preferred_execution_platform() {
            self.key(platform)?;
        }
        if let Some(toolchain) = topology.toolchain() {
            self.toolchain(toolchain)?;
        }
        Ok(())
    }
    pub(super) fn configuration_inputs(
        &mut self,
        platform_cpu: &str,
        workspace_status: &WorkspaceStatusArtifacts,
        runfiles_packages: &RunfilesPackageDepset,
        cc_context: &CcContextProjection,
    ) -> Result<(), String> {
        self.text(platform_cpu)?;
        self.row::<WorkspaceStatusArtifacts>()?;
        for artifact in [workspace_status.stable(), workspace_status.volatile()] {
            self.bytes(
                artifact
                    .root()
                    .workspace()
                    .as_path()
                    .as_os_str()
                    .as_encoded_bytes()
                    .len(),
            )?;
        }
        self.runfiles(runfiles_packages, 0)?;
        self.row::<CcContextProjection>()?;
        if let Some(config) = cc_context.owner_configuration() {
            self.bytes(config.canonical_bytes().len())?;
        }
        if let Some(features) = cc_context.owner_features() {
            for value in features.enabled().iter().chain(features.disabled()) {
                self.edge()?;
                self.text(value)?;
            }
        }
        self.action_context(cc_context.owner_default_execution())?;
        Ok(())
    }
}
