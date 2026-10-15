//! Borrow complete rule metadata before retaining its native equality snapshot.
use super::*;
use crate::attrs::owner_metadata::*;
use crate::subrule::SubruleAttributeDefault;
use crate::subrule::SubruleIdentity;
fn subrule_identity(value: &SubruleIdentity, visit: &mut Visitor<'_>) -> Result<(), String> {
    row::<SubruleIdentity>(visit)?;
    label(&value.defining_label, visit)?;
    text(&value.exported_name, visit)
}
fn configuration_field(
    value: &slug_configuration_v2::ConfigurationFieldIdentity,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    row::<slug_configuration_v2::ConfigurationFieldIdentity>(visit)?;
    text(value.tools_repository().as_str(), visit)
}
fn toolchain(value: &ToolchainTypeRequirement, visit: &mut Visitor<'_>) -> Result<(), String> {
    row::<ToolchainTypeRequirement>(visit)?;
    label(&value.label, visit)
}
impl StarlarkRuleImplementation {
    /// Exhaustive borrowed native metadata audit. The callback performs checked
    /// typed work/storage accounting only; executable FrozenValues are audited separately
    /// by the callable witness and retained original-heap ownership proof.
    pub fn visit_owner_metadata(
        &self,
        visit: &mut dyn FnMut(slug_build_api_v2::DepsetStorageCharge) -> Result<(), String>,
    ) -> Result<(), String> {
        self.visit_metadata(visit, true)
    }
    #[doc(hidden)]
    pub fn visit_owner_intrinsic_metadata(
        &self,
        visit: &mut dyn FnMut(slug_build_api_v2::DepsetStorageCharge) -> Result<(), String>,
    ) -> Result<(), String> {
        self.visit_metadata(visit, false)
    }
    fn visit_metadata(&self, visit: &mut Visitor<'_>, full_table: bool) -> Result<(), String> {
        let Self {
            implementation: _,
            definition_source,
            source_identities_by_filename,
            dependencies,
            required_toolchains,
            declared_exec_groups,
            advertised_providers,
            required_fragments,
            attached_subrules,
            subrule_callables,
            late_bound_attributes,
            attached_aspects,
            schema,
            values,
            capability,
            build_setting_definition: _,
            incoming_transition,
            predeclared_outputs,
            output_to_genfiles: _,
        } = self;
        row::<Self>(visit)?;
        source(definition_source, visit)?;
        if full_table {
            complete_sources(source_identities_by_filename, visit)?;
        } else {
            // The exact declared table body is separate; local pair/guard fields
            // and declaration reference storage remain positive intrinsic data.
            row::<crate::source_table::CertifiedSourceTable>(visit)?;
            row::<slug_build_api_v2::published_value::SharedPublicationInput>(visit)?;
        }
        for value in &**dependencies {
            label(value, visit)?;
        }
        for value in &**required_toolchains {
            toolchain(value, visit)?;
        }
        for (name, group) in &**declared_exec_groups {
            row::<(CompactString, DeclaredExecGroup)>(visit)?;
            text(name, visit)?;
            for value in &*group.toolchains {
                toolchain(value, visit)?;
            }
            for value in &*group.exec_compatible_with {
                label(value, visit)?;
            }
        }
        for value in &**advertised_providers {
            provider(value, visit)?;
        }
        for value in &**required_fragments {
            row::<CompactString>(visit)?;
            text(value, visit)?;
        }
        for value in &*attached_subrules.direct {
            subrule_identity(value, visit)?;
        }
        for value in &*attached_subrules.definitions {
            row::<crate::subrule::SubruleSemanticDefinition>(visit)?;
            subrule_identity(&value.identity, visit)?;
            for value in &*value.direct_subrules {
                subrule_identity(value, visit)?;
            }
            for value in value.fragments.iter() {
                row::<CompactString>(visit)?;
                text(value, visit)?;
            }
            for value in &*value.toolchains {
                toolchain(value, visit)?;
            }
        }
        for value in &*attached_subrules.lifted_attributes {
            row::<crate::subrule::LiftedSubruleAttribute>(visit)?;
            subrule_identity(&value.owner, visit)?;
            text(&value.user_name, visit)?;
            text(&value.rule_name, visit)?;
            match &value.default {
                SubruleAttributeDefault::Literal(value) => coerced(value, visit, 0)?,
                SubruleAttributeDefault::ConfigurationField(value) => {
                    configuration_field(value, visit)?
                }
            }
            files(&value.file_admissibility, visit)?;
            classes(&value.rule_class_admissibility, visit)?;
            allowed(&value.allowed_values, visit)?;
            required(&value.required_providers, visit)?;
        }
        for value in &*attached_subrules.spans {
            row::<crate::subrule::SubruleAttributeSpan>(visit)?;
            subrule_identity(&value.owner, visit)?;
        }
        for _ in &**subrule_callables {
            row::<FrozenValue>(visit)?;
        }
        for value in &**late_bound_attributes {
            row::<LateBoundRuleAttribute>(visit)?;
            configuration_field(&value.identity, visit)?;
            required(&value.required_providers, visit)?;
        }
        for value in &**attached_aspects {
            row::<RetainedAttachedAspect>(visit)?;
            label(&value.defining_label, visit)?;
            text(&value.exported_name, visit)?;
        }
        for value in &**schema {
            value.visit_owner_metadata(visit)?;
        }
        for value in &**values {
            row::<AttributeValue>(visit)?;
            text(&value.declaration_name, visit)?;
            coerced(&value.value, visit, 0)?;
        }
        row::<RuleCapability>(visit)?;
        text(&capability.rule_class, visit)?;
        if let Some(value) = incoming_transition {
            transition(value, visit)?;
        }
        for value in &**predeclared_outputs {
            row::<PredeclaredOutput>(visit)?;
            text(&value.key, visit)?;
            label(&value.label, visit)?;
        }
        Ok(())
    }
}
