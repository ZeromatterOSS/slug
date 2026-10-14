//! Allocation-free forecasts of complete immutable Loading metadata.
use super::*;
mod charge;
#[cfg(test)]
mod charge_tests;
pub(crate) type Visitor<'a> =
    dyn FnMut(slug_build_api_v2::DepsetStorageCharge) -> Result<(), String> + 'a;
pub(crate) fn row<T>(visit: &mut Visitor<'_>) -> Result<(), String> {
    charge::normal(std::mem::size_of::<T>(), visit)
}
pub(crate) fn text(value: &str, visit: &mut Visitor<'_>) -> Result<(), String> {
    charge::normal(value.len(), visit)
}
pub(crate) fn label(value: &CanonicalLabel, visit: &mut Visitor<'_>) -> Result<(), String> {
    row::<CanonicalLabel>(visit)?;
    text(value.package().repo().as_str(), visit)?;
    text(value.package().package().as_str(), visit)?;
    text(value.target().as_str(), visit)?;
    if let Some(mapping) = value.mapping_id() {
        text(mapping.as_str(), visit)?;
    }
    Ok(())
}
pub(crate) fn provider(value: &ProviderIdentity, visit: &mut Visitor<'_>) -> Result<(), String> {
    row::<ProviderIdentity>(visit)?;
    match value {
        ProviderIdentity::Builtin(name) => text(name, visit),
        ProviderIdentity::User(id) => {
            text(id.source_label(), visit)?;
            text(id.exported_name(), visit)
        }
    }
}
pub(crate) fn required(
    values: &[Arc<[ProviderIdentity]>],
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    for group in values {
        row::<Arc<[ProviderIdentity]>>(visit)?;
        for value in &**group {
            provider(value, visit)?;
        }
    }
    Ok(())
}
pub(crate) fn source(value: &BzlModuleIdentity, visit: &mut Visitor<'_>) -> Result<(), String> {
    charge::source_prefix(value, visit)?;
    for (apparent, canonical) in &*value.repository_mapping {
        charge::mapping_entry(apparent.as_str(), canonical.as_str(), visit)?;
    }
    Ok(())
}
pub(crate) fn sources(
    values: &[(CompactString, BzlModuleSourceProvenance)],
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    for (name, value) in values {
        row::<(CompactString, BzlModuleSourceProvenance)>(visit)?;
        text(name, visit)?;
        source(&value.identity, visit)?;
    }
    Ok(())
}
pub(crate) fn files(value: &FileAdmissibility, visit: &mut Visitor<'_>) -> Result<(), String> {
    row::<FileAdmissibility>(visit)?;
    match &value.file_types {
        FileTypes::NoFiles | FileTypes::AnyFile => {}
        FileTypes::Suffixes(values) => {
            for value in &**values {
                row::<CompactString>(visit)?;
                text(value, visit)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn classes(
    value: &RuleClassAdmissibility,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    row::<RuleClassAdmissibility>(visit)?;
    if let RuleClassAdmissibility::Only(values) = value {
        for value in &**values {
            row::<CompactString>(visit)?;
            text(value, visit)?;
        }
    }
    Ok(())
}
pub(crate) fn allowed(
    value: &AllowedAttributeValues,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    row::<AllowedAttributeValues>(visit)?;
    match value {
        AllowedAttributeValues::None => {}
        AllowedAttributeValues::Integer(values) => {
            for _ in &**values {
                row::<i32>(visit)?;
            }
        }
        AllowedAttributeValues::String(values) => {
            for value in &**values {
                row::<CompactString>(visit)?;
                text(value, visit)?;
            }
        }
    }
    Ok(())
}
pub(crate) fn transition(
    value: &TransitionDefinition,
    visit: &mut Visitor<'_>,
) -> Result<(), String> {
    row::<TransitionDefinition>(visit)?;
    for setting in value.inputs.iter().chain(value.outputs.iter()) {
        row::<TransitionSetting>(visit)?;
        label(&setting.canonical, visit)?;
        text(&setting.declared, visit)?;
    }
    source(&value.definition_source, visit)?;
    sources(&value.source_identities_by_filename, visit)
}
pub(crate) fn coerced(
    value: &CoercedAttributeValue,
    visit: &mut Visitor<'_>,
    depth: usize,
) -> Result<(), String> {
    if depth > 256 {
        return Err("rule owner metadata depth limit".into());
    }
    row::<CoercedAttributeValue>(visit)?;
    match value {
        CoercedAttributeValue::None
        | CoercedAttributeValue::Boolean(_)
        | CoercedAttributeValue::Integer(_) => {}
        CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => {
            label(value, visit)?
        }
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            for value in &**values {
                label(value, visit)?;
            }
        }
        CoercedAttributeValue::String(value) => text(value, visit)?,
        CoercedAttributeValue::StringList(values) => {
            for value in &**values {
                row::<CompactString>(visit)?;
                text(value, visit)?;
            }
        }
        CoercedAttributeValue::IntegerList(values) => {
            for _ in &**values {
                row::<i32>(visit)?;
            }
        }
        CoercedAttributeValue::StringDict(values) => {
            for (key, value) in &**values {
                row::<(CompactString, CompactString)>(visit)?;
                text(key, visit)?;
                text(value, visit)?;
            }
        }
        CoercedAttributeValue::StringListDict(values) => {
            for (key, values) in &**values {
                row::<(CompactString, Arc<[CompactString]>)>(visit)?;
                text(key, visit)?;
                for value in &**values {
                    row::<CompactString>(visit)?;
                    text(value, visit)?;
                }
            }
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            for (key, value) in &**values {
                row::<(CompactString, CanonicalLabel)>(visit)?;
                text(key, visit)?;
                label(value, visit)?;
            }
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            for (key, value) in &**values {
                row::<(CanonicalLabel, CompactString)>(visit)?;
                label(key, visit)?;
                text(value, visit)?;
            }
        }
        CoercedAttributeValue::LabelListDict(values) => {
            for (key, values) in &**values {
                row::<(CompactString, Arc<[CanonicalLabel]>)>(visit)?;
                text(key, visit)?;
                for value in &**values {
                    label(value, visit)?;
                }
            }
        }
        CoercedAttributeValue::Selector { branches, default } => {
            for (key, value) in &**branches {
                row::<(CanonicalLabel, Arc<CoercedAttributeValue>)>(visit)?;
                label(key, visit)?;
                coerced(value, visit, depth + 1)?;
            }
            if let Some(value) = default {
                coerced(value, visit, depth + 1)?;
            }
        }
        CoercedAttributeValue::Concatenation(left, right) => {
            coerced(left, visit, depth + 1)?;
            coerced(right, visit, depth + 1)?;
        }
    }
    Ok(())
}
impl AttributeSchema {
    pub(crate) fn visit_owner_metadata(&self, visit: &mut Visitor<'_>) -> Result<(), String> {
        let Self {
            declaration_name,
            query_name,
            kind: _,
            mandatory: _,
            configurable: _,
            label_reachable: _,
            order_independent: _,
            ordinary_dependency: _,
            builtin: _,
            file_admissibility,
            flags: _,
            rule_class_admissibility,
            allowed_values,
            allow_empty: _,
            required_providers,
            default,
            dependency_configuration,
            executable: _,
        } = self;
        row::<Self>(visit)?;
        text(declaration_name, visit)?;
        text(query_name, visit)?;
        files(file_admissibility, visit)?;
        classes(rule_class_admissibility, visit)?;
        allowed(allowed_values, visit)?;
        required(required_providers, visit)?;
        if let Some(value) = default {
            coerced(value, visit, 0)?;
        }
        match dependency_configuration {
            AttributeDependencyConfiguration::Target | AttributeDependencyConfiguration::Exec => {}
            AttributeDependencyConfiguration::ExecGroup(name) => text(name, visit)?,
            AttributeDependencyConfiguration::Starlark(value) => transition(value, visit)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn totals(schema: &AttributeSchema) -> (usize, usize) {
        let mut visits = 0;
        let mut bytes = 0;
        schema
            .visit_owner_metadata(&mut |n| {
                visits += n.visits;
                bytes += n.bytes;
                Ok(())
            })
            .unwrap();
        (visits, bytes)
    }
    #[test]
    fn schema_owner_forecast_covers_private_allowed_values_and_file_policies_before_retention() {
        let mut schema = AttributeSchema::new(
            "field",
            AttributeKind::String,
            false,
            true,
            Some(CoercedAttributeValue::String("default".into())),
        );
        let baseline = totals(&schema);
        schema.allowed_values = AllowedAttributeValues::String(Arc::from([
            CompactString::new("long_allowed_value"),
            CompactString::new("other"),
        ]));
        schema.file_admissibility =
            FileAdmissibility::ordered_suffixes(Arc::from([CompactString::new(".longsuffix")]));
        schema.rule_class_admissibility =
            RuleClassAdmissibility::only(vec!["private_ruleclass".into()]);
        schema.required_providers = Arc::from([Arc::from([ProviderIdentity::User(
            slug_build_api_v2::ProviderId::new("//very/long/source:defs.bzl", "Required").unwrap(),
        )])]);
        let expanded = totals(&schema);
        assert!(expanded.0 > baseline.0);
        assert!(expanded.1>=baseline.1+"long_allowed_valueother.longsuffixprivate_ruleclass//very/long/source:defs.bzlRequired".len());
        let mut visits = 0;
        let error = schema
            .visit_owner_metadata(&mut |_| {
                visits += 1;
                Err("budget before clone".into())
            })
            .unwrap_err();
        assert_eq!(visits, 1);
        assert_eq!(error, "budget before clone");
    }
    #[test]
    fn owner_metadata_coerced_recursion_obeys_shared_visit_failure_and_depth_boundary() {
        let mut value = Arc::new(CoercedAttributeValue::String("leaf".into()));
        for _ in 0..258 {
            value = Arc::new(CoercedAttributeValue::Concatenation(
                value,
                Arc::new(CoercedAttributeValue::None),
            ));
        }
        assert!(
            coerced(&value, &mut |_| Ok(()), 0)
                .unwrap_err()
                .contains("depth limit")
        );
        let mut visits = 0;
        assert_eq!(
            coerced(
                &value,
                &mut |_| {
                    visits += 1;
                    Err("visit budget".into())
                },
                0
            )
            .unwrap_err(),
            "visit budget"
        );
        assert_eq!(visits, 1);
    }
}
