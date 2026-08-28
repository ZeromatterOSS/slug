/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Scratch-only configured attribute selection.
//!
//! Loading owns the retained typed expression. DICE owns condition truth and
//! supplies the declarations used for Bazel's specialization relation. This
//! module only combines those borrowed inputs for one analysis attempt.

use std::fmt;

use allocative::Allocative;
use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use slug_loading_v2::AttributeKind;
use slug_loading_v2::CoercedAttributeValue;
use slug_loading_v2::package::ConfigSettingTarget;

#[derive(Debug, Clone)]
pub(crate) struct ConfiguredAttributeCondition {
    pub(crate) label: CanonicalLabel,
    pub(crate) declaration: ConfigSettingTarget,
    pub(crate) matches: bool,
}

/// One request-local value aligned with the loading declaration. It may be
/// moved into a synchronous evaluator but never enters a configured result.
#[derive(Debug, Clone, Allocative)]
pub(crate) struct ResolvedRuleAttribute {
    pub(crate) declaration_name: CompactString,
    pub(crate) kind: AttributeKind,
    pub(crate) sequence: bool,
    pub(crate) value: CoercedAttributeValue,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ConfiguredAttributeError {
    MissingCondition(CanonicalLabel),
    NoMatchingCondition,
    Ambiguous {
        first: CanonicalLabel,
        second: CanonicalLabel,
    },
    Concatenation(String),
}

impl fmt::Display for ConfiguredAttributeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCondition(label) => {
                write!(f, "configured selector condition was not prepared: {label}")
            }
            Self::NoMatchingCondition => f.write_str(
                "configurable attribute has no matching condition and no default condition",
            ),
            Self::Ambiguous { first, second } => write!(
                f,
                "configurable attribute has ambiguous matching conditions {first} and {second}"
            ),
            Self::Concatenation(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for ConfiguredAttributeError {}

/// Resolves one retained attribute without retaining a configured copy in
/// DICE. Only the selected branch is recursively materialized.
pub(crate) fn resolve_configured_attribute(
    value: &CoercedAttributeValue,
    conditions: &[ConfiguredAttributeCondition],
) -> Result<CoercedAttributeValue, ConfiguredAttributeError> {
    match value {
        CoercedAttributeValue::Selector { branches, default } => {
            let mut matching = Vec::new();
            for (label, branch) in branches.iter() {
                let condition = conditions
                    .iter()
                    .find(|condition| &condition.label == label)
                    .ok_or_else(|| ConfiguredAttributeError::MissingCondition(label.clone()))?;
                if condition.matches {
                    matching.push((condition, branch));
                }
            }
            if matching.is_empty() {
                return default
                    .as_deref()
                    .ok_or(ConfiguredAttributeError::NoMatchingCondition)
                    .and_then(|value| resolve_configured_attribute(value, conditions));
            }

            let maximal = matching
                .iter()
                .enumerate()
                .filter(|(candidate_index, (candidate, _))| {
                    !matching
                        .iter()
                        .enumerate()
                        .any(|(other_index, (other, _))| {
                            candidate_index != &other_index
                                && config_setting_refines(
                                    &other.declaration,
                                    &candidate.declaration,
                                )
                        })
                })
                .map(|(_, value)| *value)
                .collect::<Vec<_>>();
            let resolved = maximal
                .into_iter()
                .map(|(condition, value)| {
                    resolve_configured_attribute(value, conditions).map(|value| (condition, value))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (selected, selected_value) = &resolved[0];
            if let Some((other, _)) = resolved
                .iter()
                .skip(1)
                .find(|(_, value)| value != selected_value)
            {
                return Err(ConfiguredAttributeError::Ambiguous {
                    first: selected.label.clone(),
                    second: other.label.clone(),
                });
            }
            Ok(selected_value.clone())
        }
        CoercedAttributeValue::Concatenation(left, right) => {
            let left = resolve_configured_attribute(left, conditions)?;
            let right = resolve_configured_attribute(right, conditions)?;
            left.concatenate_resolved(&right)
                .map_err(|error| ConfiguredAttributeError::Concatenation(error.to_string()))
        }
        value => Ok(value.clone()),
    }
}

/// Bazel's ConfigMatchingProvider specialization relation for the admitted
/// native/define/Starlark-flag predicate slice. Constraint predicates remain a
/// separately deferred configured-platform category.
pub(crate) fn config_setting_refines(
    candidate: &ConfigSettingTarget,
    other: &ConfigSettingTarget,
) -> bool {
    fn native_contains(declaration: &ConfigSettingTarget, key: &str, value: &str) -> bool {
        declaration
            .values()
            .value()
            .iter()
            .any(|(candidate_key, candidate_value)| {
                candidate_key == key && candidate_value == value
            })
            || (key == "define"
                && declaration
                    .define_values()
                    .value()
                    .iter()
                    .any(|(define_key, define_value)| {
                        value == format!("{define_key}={define_value}")
                    }))
    }
    fn contains_define(declaration: &ConfigSettingTarget, key: &str, value: &str) -> bool {
        declaration
            .define_values()
            .value()
            .iter()
            .any(|(candidate_key, candidate_value)| {
                candidate_key == key && candidate_value == value
            })
            || native_contains(declaration, "define", &format!("{key}={value}"))
    }

    let native_subset = other
        .values()
        .value()
        .iter()
        .all(|(key, value)| native_contains(candidate, key, value));
    let define_subset = other
        .define_values()
        .value()
        .iter()
        .all(|(key, value)| contains_define(candidate, key, value));
    let flags_subset = other
        .flag_values()
        .value()
        .iter()
        .all(|pair| candidate.flag_values().value().contains(pair));
    if !(native_subset && define_subset && flags_subset) {
        return false;
    }

    candidate
        .values()
        .value()
        .iter()
        .any(|(key, value)| !native_contains(other, key, value))
        || candidate
            .define_values()
            .value()
            .iter()
            .any(|(key, value)| !contains_define(other, key, value))
        || candidate
            .flag_values()
            .value()
            .iter()
            .any(|pair| !other.flag_values().value().contains(pair))
}
