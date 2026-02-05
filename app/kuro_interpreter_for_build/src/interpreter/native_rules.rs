/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Native rule implementations for Bazel compatibility.
//!
//! This module provides native rule functions (constraint_setting, constraint_value)
//! that are required for loading BCR packages like @platforms which expect these
//! rules to be available without loading from .bzl files.

use std::sync::Arc;

use dupe::Dupe;
use kuro_core::plugins::PluginKindSet;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::string::StringLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_deps_collector::CoercedDeps;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::spec::internal::NAME_ATTRIBUTE;
use kuro_util::arc_str::ArcStr;
use kuro_node::attrs::coerced_deps_collector::CoercedDepsCollector;
use kuro_node::attrs::spec::AttributeSpec;
use kuro_node::attrs::values::AttrValues;
use kuro_node::nodes::unconfigured::RuleKind;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::package::Package;
use kuro_node::provider_id_set::ProviderIdSet;
use kuro_node::rule::Rule;
use kuro_node::rule::RuleIncomingTransition;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use once_cell::sync::Lazy;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;

use crate::interpreter::module_internals::ModuleInternals;

/// Pre-built Rule definitions for native rules.
/// These are created once and reused across all invocations.
mod rule_defs {
    use super::*;

    /// Creates the AttributeSpec for constraint_setting.
    /// constraint_setting only has the standard internal attributes (name, visibility, etc.)
    fn constraint_setting_attributes() -> AttributeSpec {
        // Configuration rules don't need any user-defined attributes beyond the standard ones
        // (name, visibility, etc. are added automatically by AttributeSpec::from)
        AttributeSpec::from(
            vec![], // No user-defined attributes
            false,  // Not an anonymous target
            &RuleIncomingTransition::None,
        )
        .expect("constraint_setting attributes should be valid")
    }

    /// Creates the AttributeSpec for constraint_value.
    /// constraint_value has a required `constraint_setting` attribute.
    fn constraint_value_attributes() -> AttributeSpec {
        // The constraint_setting attribute is a dep attribute.
        // Similar to what prelude uses: attrs.configuration_label() which maps to
        // AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)
        let constraint_setting_attr = Attribute::new(
            None, // No default - required attribute
            "The constraint_setting this value belongs to",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );

        AttributeSpec::from(
            vec![("constraint_setting".to_owned(), constraint_setting_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("constraint_value attributes should be valid")
    }

    /// The Rule definition for constraint_setting.
    pub static CONSTRAINT_SETTING_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: constraint_setting_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ConstraintSetting),
            rule_kind: RuleKind::Configuration,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
        })
    });

    /// The Rule definition for constraint_value.
    pub static CONSTRAINT_VALUE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: constraint_value_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ConstraintValue),
            rule_kind: RuleKind::Configuration,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
        })
    });
}

/// Creates a TargetNode for a native rule.
fn create_native_target_node(
    rule: Arc<Rule>,
    package: Arc<Package>,
    target_name: &str,
    attrs: Vec<(String, CoercedAttr)>,
) -> kuro_error::Result<TargetNode> {
    let target_label = TargetLabel::new(
        package.buildfile_path.package().dupe(),
        TargetNameRef::new(target_name)?,
    );

    // Build attribute values
    let mut attr_values = AttrValues::with_capacity(attrs.len() + 1);

    // Add the required name attribute first (it has AttributeId(0))
    attr_values.push_sorted(
        NAME_ATTRIBUTE.id,
        CoercedAttr::String(StringLiteral(ArcStr::from(target_name))),
    );

    // Get the attribute IDs from the spec and add user-provided attrs
    for (attr_name, coerced_value) in attrs {
        if let Some((_, attr_id, _)) =
            rule.attributes.attr_specs().find(|(name, _, _)| *name == attr_name)
        {
            attr_values.push_sorted(attr_id, coerced_value);
        }
    }

    // Collect dependencies for caching
    let mut deps_collector = CoercedDepsCollector::new();
    let pkg_label = package.buildfile_path.package();
    for (_name, attr_id, attr) in rule.attributes.attr_specs() {
        let value = attr_values
            .get(attr_id)
            .or_else(|| attr.default().map(|d| d.as_ref()));
        if let Some(v) = value {
            v.traverse(attr.coercer(), pkg_label, &mut deps_collector)?;
        }
    }
    let deps_cache = CoercedDeps::from(deps_collector);

    Ok(TargetNode::new(
        rule,
        package,
        target_label,
        attr_values,
        deps_cache,
        None, // No call stack for native rules
        None, // No package cfg modifiers
        false, // test_config_unification_rollout
    ))
}

/// Register native rule functions for Bazel compatibility.
///
/// These functions are available at the top level of BUILD files and provide
/// native implementations for configuration rules required by BCR packages
/// like @platforms.
#[starlark_module]
pub fn register_native_rules(globals: &mut GlobalsBuilder) {
    /// Defines a constraint setting (a dimension for platform configuration).
    ///
    /// A constraint setting defines a category of constraints like "os" or "cpu".
    /// Each constraint setting can have multiple constraint values (like "linux", "windows"
    /// for os, or "x86_64", "arm64" for cpu).
    ///
    /// Example:
    /// ```python
    /// constraint_setting(name = "os")
    /// constraint_value(name = "linux", constraint_setting = ":os")
    /// constraint_value(name = "windows", constraint_setting = ":os")
    /// ```
    ///
    /// See: https://bazel.build/reference/be/platforms-and-toolchains#constraint_setting
    fn constraint_setting<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "constraint_setting")?;

        let target_node = create_native_target_node(
            rule_defs::CONSTRAINT_SETTING_RULE.clone(),
            internals.package(),
            name,
            vec![], // No attributes beyond name
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Defines a constraint value for a constraint setting.
    ///
    /// A constraint value is a specific value within a constraint setting.
    /// For example, "linux" and "windows" are constraint values for the "os"
    /// constraint setting.
    ///
    /// Example:
    /// ```python
    /// constraint_setting(name = "os")
    /// constraint_value(
    ///     name = "linux",
    ///     constraint_setting = ":os",
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/platforms-and-toolchains#constraint_value
    fn constraint_value<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] constraint_setting: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "constraint_value")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce the constraint_setting label to a dep attribute
        let label = coercion_ctx.coerce_providers_label(constraint_setting)?;
        let coerced_constraint_setting = CoercedAttr::Dep(label);

        let target_node = create_native_target_node(
            rule_defs::CONSTRAINT_VALUE_RULE.clone(),
            internals.package(),
            name,
            vec![("constraint_setting".to_owned(), coerced_constraint_setting)],
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }
}
