/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use compact_str::CompactString;
use num_bigint::BigInt;
use slug_configuration_v2::StarlarkOption;
use slug_configuration_v2::StarlarkOptionScope;
use slug_configuration_v2::StarlarkOptionValue;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::package::BuildSettingDeclaration;
use slug_loading_v2::package::BuildSettingDefault;
use slug_loading_v2::package::BuildSettingDefinition;
use slug_loading_v2::package::BuildSettingScope;
use starlark::environment::GlobalsStatic;
use starlark::environment::LibraryExtension;
use starlark::eval::Evaluator;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::set::SetRef;

pub(crate) fn effective_default(declaration: &BuildSettingDeclaration) -> StarlarkOptionValue {
    match (declaration.definition(), declaration.default()) {
        (BuildSettingDefinition::Integer { .. }, BuildSettingDefault::Integer(value)) => {
            StarlarkOptionValue::Integer(BigInt::from(*value))
        }
        (BuildSettingDefinition::Boolean { .. }, BuildSettingDefault::Boolean(value)) => {
            StarlarkOptionValue::Boolean(*value)
        }
        (
            BuildSettingDefinition::String {
                allow_multiple: true,
                ..
            },
            BuildSettingDefault::String(value),
        ) => StarlarkOptionValue::string_list([value.clone()]),
        (BuildSettingDefinition::String { .. }, BuildSettingDefault::String(value)) => {
            StarlarkOptionValue::String(value.clone())
        }
        (BuildSettingDefinition::StringList { .. }, BuildSettingDefault::StringList(values)) => {
            StarlarkOptionValue::StringList(values.clone())
        }
        (BuildSettingDefinition::StringSet { .. }, BuildSettingDefault::StringSet(values)) => {
            StarlarkOptionValue::string_set(values.iter().cloned())
        }
        _ => unreachable!("loading validates build-setting default kinds"),
    }
}

fn resolved_scope(declaration: &BuildSettingDeclaration) -> StarlarkOptionScope {
    match declaration.scope() {
        BuildSettingScope::Default => StarlarkOptionScope::Default,
        BuildSettingScope::Universal => StarlarkOptionScope::Universal,
        BuildSettingScope::Target => StarlarkOptionScope::Target,
        BuildSettingScope::Project => StarlarkOptionScope::Project,
    }
}

fn expected_kind(declaration: &BuildSettingDeclaration) -> &'static str {
    match declaration.definition() {
        BuildSettingDefinition::Integer { .. } => "integer",
        BuildSettingDefinition::Boolean { .. } => "Boolean",
        BuildSettingDefinition::String {
            allow_multiple: false,
            ..
        } => "string",
        BuildSettingDefinition::String {
            allow_multiple: true,
            ..
        } => "list of strings",
        BuildSettingDefinition::StringList { .. } => "list of strings",
        BuildSettingDefinition::StringSet { .. } => "set or list of strings",
    }
}

fn value_kind(value: &StarlarkOptionValue) -> &'static str {
    match value {
        StarlarkOptionValue::Integer(_) => "integer",
        StarlarkOptionValue::Boolean(_) => "Boolean",
        StarlarkOptionValue::String(_) => "string",
        StarlarkOptionValue::StringList(_) => "list of strings",
        StarlarkOptionValue::StringSet(_) => "set of strings",
    }
}

fn value_matches_declaration(
    declaration: &BuildSettingDeclaration,
    value: &StarlarkOptionValue,
) -> bool {
    matches!(
        (declaration.definition(), value),
        (
            BuildSettingDefinition::Integer { .. },
            StarlarkOptionValue::Integer(_)
        ) | (
            BuildSettingDefinition::Boolean { .. },
            StarlarkOptionValue::Boolean(_)
        ) | (
            BuildSettingDefinition::String {
                allow_multiple: false,
                ..
            },
            StarlarkOptionValue::String(_)
        ) | (
            BuildSettingDefinition::String {
                allow_multiple: true,
                ..
            },
            StarlarkOptionValue::StringList(_)
        ) | (
            BuildSettingDefinition::StringList { .. },
            StarlarkOptionValue::StringList(_)
        ) | (
            BuildSettingDefinition::StringSet { .. },
            StarlarkOptionValue::StringSet(_)
        )
    )
}

pub(crate) fn resolve_candidate(
    label: CanonicalLabel,
    declaration: &BuildSettingDeclaration,
    candidate: StarlarkOptionValue,
) -> Result<Option<StarlarkOption>, String> {
    let candidate = match candidate {
        StarlarkOptionValue::StringSet(values) => {
            StarlarkOptionValue::string_set(values.iter().cloned())
        }
        candidate => candidate,
    };
    if !value_matches_declaration(declaration, &candidate) {
        return Err(format!(
            "build setting {label} expects {}, not {}",
            expected_kind(declaration),
            value_kind(&candidate),
        ));
    }
    if candidate == effective_default(declaration) {
        Ok(None)
    } else {
        Ok(Some(StarlarkOption::new(
            label,
            candidate,
            resolved_scope(declaration),
        )))
    }
}

pub(crate) fn effective_value(
    label: &CanonicalLabel,
    declaration: &BuildSettingDeclaration,
    configured: Option<&StarlarkOption>,
) -> Result<StarlarkOptionValue, String> {
    let Some(configured) = configured else {
        return Ok(effective_default(declaration));
    };
    if !value_matches_declaration(declaration, configured.value()) {
        return Err(format!(
            "configured build setting {label} expects {}, not {}",
            expected_kind(declaration),
            value_kind(configured.value()),
        ));
    }
    if configured.scope() != resolved_scope(declaration) {
        return Err(format!(
            "configured build setting {label} carries {:?} scope instead of declaration scope {:?}",
            configured.scope(),
            resolved_scope(declaration),
        ));
    }
    Ok(configured.value().clone())
}

fn unpack_string_values<'v>(
    value: Value<'v>,
    heap: Heap<'v>,
) -> Result<Vec<CompactString>, String> {
    value
        .iterate(heap)
        .map_err(|error| error.to_string())?
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::from)
                .ok_or_else(|| "build-setting collection members must be strings".to_owned())
        })
        .collect()
}

pub(crate) fn unpack_transition_value<'v>(
    label: &CanonicalLabel,
    declaration: &BuildSettingDeclaration,
    value: Value<'v>,
    heap: Heap<'v>,
) -> Result<StarlarkOptionValue, String> {
    let converted = match declaration.definition() {
        BuildSettingDefinition::Integer { .. } => BigInt::unpack_value(value)
            .map_err(|error| error.to_string())?
            .map(StarlarkOptionValue::Integer),
        BuildSettingDefinition::Boolean { .. } => {
            value.unpack_bool().map(StarlarkOptionValue::Boolean)
        }
        BuildSettingDefinition::String {
            allow_multiple: false,
            ..
        } => value
            .unpack_str()
            .map(|value| StarlarkOptionValue::String(value.into())),
        BuildSettingDefinition::String {
            allow_multiple: true,
            ..
        }
        | BuildSettingDefinition::StringList { .. }
            if ListRef::from_value(value).is_some() =>
        {
            Some(StarlarkOptionValue::string_list(unpack_string_values(
                value, heap,
            )?))
        }
        BuildSettingDefinition::StringSet { .. }
            if ListRef::from_value(value).is_some()
                || SetRef::unpack_value_opt(value).is_some() =>
        {
            Some(StarlarkOptionValue::string_set(unpack_string_values(
                value, heap,
            )?))
        }
        _ => None,
    };
    converted.ok_or_else(|| {
        format!(
            "transition output for {label} must be {}",
            expected_kind(declaration)
        )
    })
}

fn set_function() -> FrozenValue {
    static SET: GlobalsStatic = GlobalsStatic::new();
    SET.function(|globals| LibraryExtension::SetType.add(globals))
}

pub(crate) fn alloc_value<'v>(
    value: &StarlarkOptionValue,
    heap: Heap<'v>,
    evaluator: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    Ok(match value {
        StarlarkOptionValue::Integer(value) => heap.alloc(value.clone()),
        StarlarkOptionValue::Boolean(value) => Value::new_bool(*value),
        StarlarkOptionValue::String(value) => heap.alloc_str(value).to_value(),
        StarlarkOptionValue::StringList(values) => heap.alloc(
            values
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>(),
        ),
        StarlarkOptionValue::StringSet(values) => {
            let members = heap.alloc(
                values
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>(),
            );
            evaluator.eval_function(set_function().to_value(), &[members], &[])?
        }
    })
}
