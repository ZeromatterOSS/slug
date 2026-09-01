/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::attrs::TransitionDefinition;
use slug_loading_v2::attrs::TransitionSetting;
use slug_loading_v2::package::BuildSettingDeclaration;
use slug_loading_v2::package::resolve_rule_definition_label;
use slug_loading_v2::provider::TransitionEvaluationContext;
use slug_loading_v2::provider::alloc_starlark_label;
use slug_loading_v2::provider::starlark_label;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::structs::AllocStruct;
use starlark::values::tuple::TupleRef;
use starlark_map::small_set::SmallSet;

use crate::build_setting;
use crate::configured_attribute::ResolvedRuleAttribute;
use crate::key::ConfigurationKey;

const PLATFORMS: &str = "platforms";

#[derive(Debug, Clone)]
pub(crate) enum PreparedTransitionSetting {
    BuildSetting {
        setting: TransitionSetting,
        declaration: BuildSettingDeclaration,
    },
    Platforms(TransitionSetting),
}

impl PreparedTransitionSetting {
    pub(crate) fn setting(&self) -> &TransitionSetting {
        match self {
            Self::BuildSetting { setting, .. } | Self::Platforms(setting) => setting,
        }
    }
}

fn row<'a>(
    rows: &'a [PreparedTransitionSetting],
    setting: &TransitionSetting,
) -> Result<&'a PreparedTransitionSetting, String> {
    rows.iter()
        .find(|row| row.setting().canonical() == setting.canonical())
        .ok_or_else(|| format!("transition setting {} was not prepared", setting.declared()))
}

fn alloc_raw_attribute<'v>(
    value: &CoercedAttributeValue,
    heap: Heap<'v>,
) -> Result<Value<'v>, String> {
    let label = |label: &CanonicalLabel| alloc_starlark_label(heap, label.clone());
    Ok(match value {
        CoercedAttributeValue::None => Value::new_none(),
        CoercedAttributeValue::Label(value) | CoercedAttributeValue::Output(value) => label(value),
        CoercedAttributeValue::LabelList(values) | CoercedAttributeValue::OutputList(values) => {
            heap.alloc(values.iter().map(label).collect::<Vec<_>>())
        }
        CoercedAttributeValue::String(value) => heap.alloc_str(value).to_value(),
        CoercedAttributeValue::StringList(values) => heap.alloc(
            values
                .iter()
                .map(|value| heap.alloc_str(value).to_value())
                .collect::<Vec<_>>(),
        ),
        CoercedAttributeValue::StringListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    heap.alloc_str(key).to_value(),
                    heap.alloc(
                        values
                            .iter()
                            .map(|value| heap.alloc_str(value).to_value())
                            .collect::<Vec<_>>(),
                    ),
                )
            })))
        }
        CoercedAttributeValue::Boolean(value) => Value::new_bool(*value),
        CoercedAttributeValue::Integer(value) => heap.alloc(*value),
        CoercedAttributeValue::IntegerList(values) => {
            heap.alloc(values.iter().copied().collect::<Vec<_>>())
        }
        CoercedAttributeValue::StringDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (
                    heap.alloc_str(key).to_value(),
                    heap.alloc_str(value).to_value(),
                )
            })))
        }
        CoercedAttributeValue::StringKeyedLabelDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (heap.alloc_str(key).to_value(), label(value))
            })))
        }
        CoercedAttributeValue::LabelKeyedStringDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, value)| {
                (label(key), heap.alloc_str(value).to_value())
            })))
        }
        CoercedAttributeValue::LabelListDict(values) => {
            heap.alloc(AllocDict(values.iter().map(|(key, values)| {
                (
                    heap.alloc_str(key).to_value(),
                    heap.alloc(values.iter().map(label).collect::<Vec<_>>()),
                )
            })))
        }
        CoercedAttributeValue::Selector { .. } | CoercedAttributeValue::Concatenation(_, _) => {
            return Err("transition attr contains an unresolved configurable value".to_owned());
        }
    })
}

fn unpack_platform<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
    transition: &TransitionDefinition,
) -> Result<Option<CanonicalLabel>, String> {
    let resolve = |value: &str| {
        if value.starts_with("@@") {
            CanonicalLabel::parse(value).map_err(|error| error.to_string())
        } else if let Some(value) = value.strip_prefix("@//") {
            CanonicalLabel::parse(&format!("@@//{value}")).map_err(|error| error.to_string())
        } else {
            resolve_rule_definition_label(value, transition.definition_source())
                .map_err(|error| error.to_string())
        }
    };
    if starlark_label(value).is_some() {
        return Err(
            "transition output for //command_line_option:platforms cannot be a scalar Label"
                .to_owned(),
        );
    }
    let mut values = if let Some(value) = value.unpack_str() {
        value
            .split(',')
            .filter(|value| !value.is_empty())
            .map(resolve)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        if ListRef::from_value(value).is_none() && TupleRef::from_value(value).is_none() {
            return Err(
                "platforms output must be a string or sequence of strings/Labels".to_owned(),
            );
        }
        let mut converted = Vec::new();
        for value in value.iterate(heap).map_err(|_| {
            "platforms output must be a string or sequence of strings/Labels".to_owned()
        })? {
            if let Some(label) = starlark_label(value) {
                converted.push(label);
                continue;
            }
            let value = value
                .unpack_str()
                .ok_or_else(|| "platforms sequence members must be strings or Labels".to_owned())?;
            for value in value.split(',').filter(|value| !value.is_empty()) {
                converted.push(resolve(value)?);
            }
        }
        converted
    };
    Ok(values.drain(..).next())
}

fn apply_patch<'v>(
    transition: &TransitionDefinition,
    rows: &[PreparedTransitionSetting],
    configuration: &ConfigurationKey,
    patch: DictRef<'v>,
    heap: Heap<'v>,
) -> Result<ConfigurationKey, String> {
    if patch.is_empty() {
        return Ok(configuration.clone());
    }
    let mut returned = SmallSet::with_capacity(patch.len());
    let mut result = configuration.clone();
    for (key, value) in patch.iter() {
        let declared = key
            .unpack_str()
            .ok_or_else(|| "transition output keys must be strings".to_owned())?;
        let setting = transition
            .outputs()
            .iter()
            .find(|setting| setting.declared() == declared)
            .ok_or_else(|| format!("transition returned undeclared output {declared}"))?;
        if !returned.insert(setting.canonical().clone()) {
            return Err(format!("transition returned duplicate output {declared}"));
        }
        match row(rows, setting)? {
            PreparedTransitionSetting::BuildSetting { declaration, .. } => {
                let candidate = build_setting::unpack_transition_value(
                    setting.canonical(),
                    declaration,
                    value,
                    heap,
                )?;
                match build_setting::resolve_candidate(
                    setting.canonical().clone(),
                    declaration,
                    candidate,
                )? {
                    Some(value) => result = result.with_starlark_option(value),
                    None => result = result.without_starlark_option(setting.canonical()),
                }
            }
            PreparedTransitionSetting::Platforms(_) => {
                let structural = result
                    .slug_configuration()
                    .ok_or("platform transition requires structural configuration")?;
                let platform = unpack_platform(value, heap, transition)?;
                result = ConfigurationKey::from_slug(
                    structural
                        .with_transition_target_platform(platform.as_ref())
                        .map_err(|error| error.to_string())?,
                );
            }
        }
    }
    let missing = transition
        .outputs()
        .iter()
        .filter(|setting| !returned.contains(setting.canonical()))
        .map(TransitionSetting::declared)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "transition did not return declared outputs: {}",
            missing.join(", ")
        ));
    }
    Ok(result)
}

/// Invoke one transition with declarations and attributes already prepared by
/// DICE. The returned vector preserves Bazel's split-shaped surface; callers
/// own their zero/one/many topology boundary.
pub(crate) fn evaluate(
    transition: &TransitionDefinition,
    rows: &[PreparedTransitionSetting],
    attributes: &[ResolvedRuleAttribute],
    configuration: &ConfigurationKey,
) -> Result<Vec<ConfigurationKey>, String> {
    let module = Module::new();
    let context = TransitionEvaluationContext::new(
        transition.definition_source().as_ref().clone(),
        transition.source_identities_by_filename().clone(),
    );
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    let settings = transition
        .inputs()
        .iter()
        .map(|setting| {
            let value = match row(rows, setting)? {
                PreparedTransitionSetting::BuildSetting { declaration, .. } => {
                    let value = build_setting::effective_value(
                        setting.canonical(),
                        declaration,
                        configuration.starlark_option(setting.canonical()),
                    )?;
                    build_setting::alloc_value(&value, module.heap(), &mut evaluator)
                        .map_err(|error| error.to_string())?
                }
                PreparedTransitionSetting::Platforms(_) => {
                    let structural = configuration
                        .slug_configuration()
                        .ok_or("platform transition requires structural configuration")?;
                    module.heap().alloc(
                        structural
                            .transition_target_platforms()
                            .map_err(|error| error.to_string())?
                            .iter()
                            .cloned()
                            .map(|label| alloc_starlark_label(module.heap(), label))
                            .collect::<Vec<_>>(),
                    )
                }
            };
            Ok((
                module.heap().alloc_str(setting.declared()).to_value(),
                value,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let attributes = attributes
        .iter()
        .map(|attribute| {
            Ok((
                attribute.declaration_name.to_string(),
                alloc_raw_attribute(&attribute.value, module.heap())?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let returned = evaluator
        .eval_function(
            transition.implementation().to_value(),
            &[
                module.heap().alloc(AllocDict(settings)),
                module.heap().alloc(AllocStruct(attributes)),
            ],
            &[],
        )
        .map_err(|error| error.to_string())?;
    if returned.is_none() {
        return Ok(vec![configuration.clone()]);
    }
    if let Some(dictionary) = DictRef::from_value(returned) {
        if dictionary.is_empty() {
            return Ok(vec![configuration.clone()]);
        }
        if dictionary
            .iter()
            .all(|(_, value)| DictRef::from_value(value).is_some())
        {
            return dictionary
                .iter()
                .map(|(key, value)| {
                    key.unpack_str().ok_or_else(|| {
                        "split transition dictionary keys must be strings".to_owned()
                    })?;
                    apply_patch(
                        transition,
                        rows,
                        configuration,
                        DictRef::from_value(value).expect("checked split patch"),
                        module.heap(),
                    )
                })
                .collect();
        }
        return Ok(vec![apply_patch(
            transition,
            rows,
            configuration,
            dictionary,
            module.heap(),
        )?]);
    }
    if ListRef::from_value(returned).is_none() && TupleRef::from_value(returned).is_none() {
        return Err(
            "transition must return a dictionary, sequence of dictionaries, or None".to_owned(),
        );
    }
    let patches = returned
        .iterate(module.heap())
        .map_err(|_| {
            "transition must return a dictionary, sequence of dictionaries, or None".to_owned()
        })?
        .collect::<Vec<_>>();
    if patches.is_empty() {
        return Ok(vec![configuration.clone()]);
    }
    patches
        .into_iter()
        .map(|patch| {
            let patch = DictRef::from_value(patch)
                .ok_or_else(|| "transition sequence members must be dictionaries".to_owned())?;
            apply_patch(transition, rows, configuration, patch, module.heap())
        })
        .collect()
}

pub(crate) fn is_platforms(setting: &TransitionSetting) -> bool {
    setting.is_native_option() && setting.canonical().target().as_str() == PLATFORMS
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn transition_attribute_projects_signed_integer_list_in_order() {
        let module = Module::new();
        let value = alloc_raw_attribute(
            &CoercedAttributeValue::IntegerList(Arc::from([1, -2, 3])),
            module.heap(),
        )
        .unwrap();
        assert_eq!(value.to_repr(), "[1, -2, 3]");
    }
}
