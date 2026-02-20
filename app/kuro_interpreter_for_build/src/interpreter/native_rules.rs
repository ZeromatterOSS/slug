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
use kuro_node::attrs::attr_type::list::ListLiteral;
use kuro_node::attrs::attr_type::string::StringLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_deps_collector::CoercedDeps;
use kuro_node::attrs::coerced_deps_collector::CoercedDepsCollector;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use kuro_node::attrs::spec::AttributeSpec;
use kuro_node::attrs::spec::internal::NAME_ATTRIBUTE;
use kuro_node::attrs::spec::internal::VISIBILITY_ATTRIBUTE;
use kuro_node::attrs::values::AttrValues;
use kuro_node::nodes::unconfigured::RuleKind;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::package::Package;
use kuro_node::provider_id_set::ProviderIdSet;
use kuro_node::rule::Rule;
use kuro_node::rule::RuleIncomingTransition;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use kuro_node::visibility::VisibilityPatternList;
use kuro_node::visibility::VisibilitySpecification;
use kuro_util::arc_str::ArcSlice;
use kuro_util::arc_str::ArcStr;
use once_cell::sync::Lazy;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneType;

use crate::attrs::coerce::attr_type::AttrTypeExt;
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
            is_test: false,
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
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for alias.
    /// alias has a required `actual` attribute pointing to the target it aliases.
    fn alias_attributes() -> AttributeSpec {
        let actual_attr = Attribute::new(
            None, // No default - required attribute
            "The target that this alias points to",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );

        AttributeSpec::from(
            vec![("actual".to_owned(), actual_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("alias attributes should be valid")
    }

    /// The Rule definition for alias.
    pub static ALIAS_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: alias_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::Alias),
            rule_kind: RuleKind::Normal, // Aliases can be used anywhere
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for filegroup.
    /// filegroup has optional `srcs` (list of sources/deps) and `data` attributes.
    fn filegroup_attributes() -> AttributeSpec {
        let srcs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The list of files or targets in this filegroup",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        let data_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The list of data files for this filegroup",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        AttributeSpec::from(
            vec![
                ("srcs".to_owned(), srcs_attr),
                ("data".to_owned(), data_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("filegroup attributes should be valid")
    }

    /// The Rule definition for filegroup.
    pub static FILEGROUP_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: filegroup_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::Filegroup),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for label_flag.
    /// label_flag is a Bazel build setting that holds a label value.
    /// The `build_setting_default` is stored as a STRING (not a dep) because in Bazel,
    /// label_flag targets do NOT create dependency edges to their default value.
    /// The label_flag is a configuration flag; its value is resolved at configuration time,
    /// not at loading time. Treating it as a dep would create false cycles (e.g., in rules_rust
    /// where process_wrapper → import → import_macro_label → import_macro → import_macro_impl
    /// → process_wrapper forms a cycle only if import_macro_label follows its default as a dep).
    fn label_flag_attributes() -> AttributeSpec {
        let default_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "The default label value for this flag (stored as string, not a dep)",
            AttrType::string(),
        );

        AttributeSpec::from(
            vec![("build_setting_default".to_owned(), default_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("label_flag attributes should be valid")
    }

    /// The Rule definition for label_flag.
    pub static LABEL_FLAG_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: label_flag_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::LabelFlag),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for config_setting.
    /// config_setting has a `constraint_values` attribute (list of deps pointing to
    /// constraint_value targets) and a `values` attribute (dict of buckconfig values).
    fn config_setting_attributes() -> AttributeSpec {
        use kuro_node::attrs::attr_type::dict::DictLiteral;
        use kuro_node::attrs::attr_type::list::ListLiteral;
        use kuro_util::arc_str::ArcSlice;

        let constraint_values_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The set of constraint_values that must be satisfied for this config_setting to match",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );

        let values_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Dict(
                DictLiteral(ArcSlice::default()),
            ))),
            "A dictionary of configuration values (section.key to value)",
            AttrType::dict(AttrType::string(), AttrType::string(), false),
        );

        AttributeSpec::from(
            vec![
                ("constraint_values".to_owned(), constraint_values_attr),
                ("values".to_owned(), values_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("config_setting attributes should be valid")
    }

    /// The Rule definition for config_setting.
    pub static CONFIG_SETTING_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: config_setting_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ConfigSetting),
            rule_kind: RuleKind::Configuration,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// The Rule definition for package_group.
    /// package_group defines a set of packages for visibility control.
    pub static PACKAGE_GROUP_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: constraint_setting_attributes(), // Same as constraint_setting - just name + visibility
            rule_type: RuleType::Native(NativeRuleKind::PackageGroup),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for toolchain.
    /// Note: exec_compatible_with and target_compatible_with are internal attributes
    /// and are NOT added here (they're already included automatically).
    fn toolchain_attributes() -> AttributeSpec {
        let toolchain_type_attr = Attribute::new(
            None, // required
            "The toolchain_type this toolchain satisfies",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );
        let toolchain_impl_attr = Attribute::new(
            None, // required
            "The toolchain implementation target",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );

        AttributeSpec::from(
            vec![
                ("toolchain_type".to_owned(), toolchain_type_attr),
                ("toolchain".to_owned(), toolchain_impl_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("toolchain attributes should be valid")
    }

    /// The Rule definition for toolchain.
    /// toolchain registers a toolchain implementation for a toolchain_type + platform.
    pub static TOOLCHAIN_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: toolchain_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::Toolchain),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// The Rule definition for toolchain_type.
    /// toolchain_type is a simple marker target with no special attributes.
    pub static TOOLCHAIN_TYPE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: constraint_setting_attributes(), // Same as constraint_setting - just name + visibility
            rule_type: RuleType::Native(NativeRuleKind::ToolchainType),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for genrule.
    /// genrule has cmd, outs, srcs, tools attributes.
    fn genrule_attributes() -> AttributeSpec {
        let cmd_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "The command to run",
            AttrType::string(),
        );

        let outs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Output files generated by the command",
            AttrType::list(AttrType::string()),
        );

        let srcs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Input files for the command",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        let tools_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Tool dependencies for the command",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        AttributeSpec::from(
            vec![
                ("cmd".to_owned(), cmd_attr),
                ("outs".to_owned(), outs_attr),
                ("srcs".to_owned(), srcs_attr),
                ("tools".to_owned(), tools_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("genrule attributes should be valid")
    }

    /// The Rule definition for genrule.
    pub static GENRULE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: genrule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::Genrule),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for platform.
    fn platform_attributes() -> AttributeSpec {
        let constraint_values_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The constraint values for this platform",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );

        let parents_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Parent platforms to inherit from",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );

        let exec_properties_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Dict({
                use kuro_node::attrs::attr_type::dict::DictLiteral;
                DictLiteral(ArcSlice::default())
            }))),
            "Execution properties for this platform",
            AttrType::dict(AttrType::string(), AttrType::string(), false),
        );

        let flags_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Flags for this platform",
            AttrType::list(AttrType::string()),
        );

        AttributeSpec::from(
            vec![
                ("constraint_values".to_owned(), constraint_values_attr),
                ("parents".to_owned(), parents_attr),
                ("exec_properties".to_owned(), exec_properties_attr),
                ("flags".to_owned(), flags_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("platform attributes should be valid")
    }

    pub static PLATFORM_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: platform_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::Platform),
            rule_kind: RuleKind::Configuration,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for native cc rules (cc_library, cc_binary, cc_test).
    fn cc_rule_attributes() -> AttributeSpec {
        let deps_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Dependencies",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );

        let srcs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Source files",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        let hdrs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Header files",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        let string_list_attr = |doc: &str| {
            Attribute::new(
                Some(Arc::new(CoercedAttr::List(
                    ListLiteral(ArcSlice::default()),
                ))),
                doc,
                AttrType::list(AttrType::string()),
            )
        };

        AttributeSpec::from(
            vec![
                ("deps".to_owned(), deps_attr),
                ("srcs".to_owned(), srcs_attr),
                ("hdrs".to_owned(), hdrs_attr),
                ("copts".to_owned(), string_list_attr("Compiler options")),
                ("linkopts".to_owned(), string_list_attr("Linker options")),
                (
                    "defines".to_owned(),
                    string_list_attr("Preprocessor defines"),
                ),
                (
                    "local_defines".to_owned(),
                    string_list_attr("Local preprocessor defines"),
                ),
                (
                    "includes".to_owned(),
                    string_list_attr("Include directories"),
                ),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("cc rule attributes should be valid")
    }

    pub static CC_LIBRARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcLibrary),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    pub static CC_BINARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcBinary),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    pub static CC_TEST_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcTest),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: true,
        })
    });

    fn test_suite_attributes() -> AttributeSpec {
        // `tests` is an internal attribute (ID 8), so we don't add it as a user attribute.
        AttributeSpec::from(vec![], false, &RuleIncomingTransition::None)
            .expect("test_suite attributes should be valid")
    }

    pub static TEST_SUITE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: test_suite_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::TestSuite),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    /// Creates the AttributeSpec for sh_binary / sh_test / sh_library.
    /// These rules share the same attribute set: srcs, deps, data.
    fn sh_rule_attributes() -> AttributeSpec {
        let srcs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Shell script source files",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        let deps_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Dependencies",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );

        let data_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Data files available at runtime",
            AttrType::list(AttrType::one_of(vec![
                AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        AttributeSpec::from(
            vec![
                ("srcs".to_owned(), srcs_attr),
                ("deps".to_owned(), deps_attr),
                ("data".to_owned(), data_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("sh rule attributes should be valid")
    }

    pub static SH_BINARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: sh_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ShBinary),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });

    pub static SH_TEST_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: sh_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ShTest),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: true,
        })
    });

    pub static SH_LIBRARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: sh_rule_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::ShLibrary),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
        })
    });
}

/// Bazel-style visibility constants
const BAZEL_VISIBILITY_PUBLIC: &str = "//visibility:public";

/// Parses a visibility list and returns a VisibilitySpecification.
/// Supports both Kuro-style ("PUBLIC") and Bazel-style ("//visibility:public").
/// Returns None if no visibility was explicitly specified (to use default).
fn parse_explicit_visibility(visibility: &[String]) -> Option<VisibilitySpecification> {
    if visibility.is_empty() {
        return None; // No explicit visibility, use default
    }
    for item in visibility {
        // Check for public visibility patterns
        if item == "PUBLIC"
            || item == BAZEL_VISIBILITY_PUBLIC
            || item.contains("__subpackages__")
            || item.contains("__pkg__")
        {
            // TODO(bazel-compat): Implement proper Bazel visibility patterns.
            // For now, treat __subpackages__ and __pkg__ as public.
            return Some(VisibilitySpecification(VisibilityPatternList::Public));
        }
    }
    // Explicit visibility but not public - default to private
    Some(VisibilitySpecification::DEFAULT)
}

/// Extract visibility strings from a Starlark value.
/// Handles None (returns empty), list of strings, or tuple of strings.
fn extract_visibility_strings(value: starlark::values::Value) -> Vec<String> {
    if value.is_none() {
        return Vec::new();
    }
    if let Some(list) = starlark::values::list::ListRef::from_value(value) {
        return list
            .iter()
            .filter_map(|v| v.unpack_str().map(|s| s.to_owned()))
            .collect();
    }
    if let Some(tuple) = starlark::values::tuple::TupleRef::from_value(value) {
        return tuple
            .iter()
            .filter_map(|v| v.unpack_str().map(|s| s.to_owned()))
            .collect();
    }
    Vec::new()
}

/// Creates a TargetNode for a native rule.
fn create_native_target_node(
    rule: Arc<Rule>,
    package: Arc<Package>,
    target_name: &str,
    attrs: Vec<(String, CoercedAttr)>,
    visibility: &[String],
    default_visibility: &VisibilitySpecification,
) -> kuro_error::Result<TargetNode> {
    let target_label = TargetLabel::new(
        package.buildfile_path.package().dupe(),
        TargetNameRef::new(target_name)?,
    );

    // Build attribute values
    let mut attr_values = AttrValues::with_capacity(attrs.len() + 2);

    // Add the required name attribute first (it has AttributeId(0))
    attr_values.push_sorted(
        NAME_ATTRIBUTE.id,
        CoercedAttr::String(StringLiteral(ArcStr::from(target_name))),
    );

    // Add visibility attribute (AttributeId(5))
    // Use explicit visibility if provided, otherwise fall back to package default
    let visibility_spec =
        parse_explicit_visibility(visibility).unwrap_or_else(|| default_visibility.dupe());
    attr_values.push_sorted(
        VISIBILITY_ATTRIBUTE.id,
        CoercedAttr::Visibility(visibility_spec),
    );

    // Get the attribute IDs from the spec and add user-provided attrs
    for (attr_name, coerced_value) in attrs {
        if let Some((_, attr_id, _)) = rule
            .attributes
            .attr_specs()
            .find(|(name, _, _)| *name == attr_name)
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
        None,  // No call stack for native rules
        None,  // No package cfg modifiers
        false, // test_config_unification_rollout
    ))
}

/// Helper to create a native cc target node (cc_library, cc_binary, cc_test).
fn create_native_cc_target<'v>(
    rule: Arc<Rule>,
    name: &str,
    srcs: Value<'v>,
    hdrs: Value<'v>,
    deps: Value<'v>,
    visibility: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<NoneType> {
    let internals = ModuleInternals::from_context(eval, "cc_rule")?;
    let coercion_ctx = internals.attr_coercion_context();

    let src_type = AttrType::list(AttrType::one_of(vec![
        AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        AttrType::source(false),
    ]));
    let dep_type = AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));

    let empty_list = eval.heap().alloc(starlark::values::list::AllocList::EMPTY);
    let srcs_val = if srcs.is_none() { empty_list } else { srcs };
    let hdrs_val = if hdrs.is_none() { empty_list } else { hdrs };
    let deps_val = if deps.is_none() { empty_list } else { deps };

    let coerced_srcs = src_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, srcs_val)?;
    let coerced_hdrs = src_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, hdrs_val)?;
    let coerced_deps = dep_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, deps_val)?;

    let empty_string_list = CoercedAttr::List(ListLiteral(kuro_util::arc_str::ArcSlice::default()));

    let target_node = create_native_target_node(
        rule,
        internals.package(),
        name,
        vec![
            ("deps".to_owned(), coerced_deps),
            ("srcs".to_owned(), coerced_srcs),
            ("hdrs".to_owned(), coerced_hdrs),
            ("copts".to_owned(), empty_string_list.clone()),
            ("linkopts".to_owned(), empty_string_list.clone()),
            ("defines".to_owned(), empty_string_list.clone()),
            ("local_defines".to_owned(), empty_string_list.clone()),
            ("includes".to_owned(), empty_string_list),
        ],
        &extract_visibility_strings(visibility),
        &internals.default_visibility(),
    )?;

    internals.record(target_node)?;
    Ok(NoneType)
}

/// Helper to create a native sh_* target node (sh_binary, sh_test, sh_library).
fn create_sh_target<'v>(
    rule: Arc<Rule>,
    name: &str,
    srcs: Value<'v>,
    deps: Value<'v>,
    visibility: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<NoneType> {
    let internals = ModuleInternals::from_context(eval, "sh_rule")?;
    let coercion_ctx = internals.attr_coercion_context();

    let src_type = AttrType::list(AttrType::one_of(vec![
        AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        AttrType::source(false),
    ]));
    let dep_type = AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));

    let empty_list = eval.heap().alloc(starlark::values::list::AllocList::EMPTY);
    let srcs_val = if srcs.is_none() { empty_list } else { srcs };
    let deps_val = if deps.is_none() { empty_list } else { deps };

    let coerced_srcs = src_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, srcs_val)?;
    let coerced_deps = dep_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, deps_val)?;

    let target_node = create_native_target_node(
        rule,
        internals.package(),
        name,
        vec![
            ("srcs".to_owned(), coerced_srcs),
            ("deps".to_owned(), coerced_deps),
            (
                "data".to_owned(),
                CoercedAttr::List(ListLiteral(kuro_util::arc_str::ArcSlice::default())),
            ),
        ],
        &extract_visibility_strings(visibility),
        &internals.default_visibility(),
    )?;

    internals.record(target_node)?;
    Ok(NoneType)
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
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "constraint_setting")?;

        let target_node = create_native_target_node(
            rule_defs::CONSTRAINT_SETTING_RULE.clone(),
            internals.package(),
            name,
            vec![], // No attributes beyond name
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
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
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "constraint_setting")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce the constraint_setting label to a dep attribute
        let label = coercion_ctx.coerce_providers_label(constraint_setting)?;
        let coerced_constraint_setting = CoercedAttr::Dep(label);

        let target_node = create_native_target_node(
            rule_defs::CONSTRAINT_VALUE_RULE.clone(),
            internals.package(),
            name,
            vec![("constraint_setting".to_owned(), coerced_constraint_setting)],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Creates an alias to another target.
    ///
    /// An alias is a target that forwards all requests to another target.
    /// When you build an alias, you actually build its `actual` target.
    /// When you depend on an alias, you depend on its `actual` target.
    ///
    /// Example:
    /// ```python
    /// alias(
    ///     name = "macos",
    ///     actual = ":osx",
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/general#alias
    fn alias<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] actual: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "alias")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce the actual target - supports both plain strings and select() expressions
        let dep_attr_type = AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY);
        let coerced_actual = dep_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, actual)?;

        let target_node = create_native_target_node(
            rule_defs::ALIAS_RULE.clone(),
            internals.package(),
            name,
            vec![("actual".to_owned(), coerced_actual)],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Groups a set of files under a single name for convenience.
    ///
    /// This is a Bazel built-in rule that creates a named reference to a set of files.
    /// Other rules can depend on a filegroup instead of listing individual files.
    ///
    /// See: https://bazel.build/reference/be/general#filegroup
    fn filegroup<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] data: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "filegroup")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce srcs and data - accept both lists and select() expressions
        let srcs_attr_type = AttrType::list(AttrType::one_of(vec![
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
            AttrType::source(false),
        ]));
        let data_attr_type = srcs_attr_type.clone();

        let srcs_value = if srcs.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            srcs
        };
        let coerced_srcs =
            srcs_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, srcs_value)?;

        let data_value = if data.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            data
        };
        let coerced_data =
            data_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, data_value)?;

        let target_node = create_native_target_node(
            rule_defs::FILEGROUP_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("srcs".to_owned(), coerced_srcs),
                ("data".to_owned(), coerced_data),
            ],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Defines a label-typed build setting (label_flag).
    ///
    /// A label_flag is a build setting that holds a label value and can be
    /// overridden on the command line. The target acts as a forwarding dependency
    /// to its `build_setting_default` target.
    ///
    /// Example:
    /// ```python
    /// label_flag(
    ///     name = "link_extra_libs",
    ///     build_setting_default = ":empty_lib",
    /// )
    /// ```
    ///
    /// See: https://bazel.build/rules/lib/toplevel/label_flag
    fn label_flag<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] build_setting_default: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "label_flag")?;

        // Store build_setting_default as a plain string - NOT as a dep.
        // In Bazel, label_flag is a build setting (configuration flag) whose value is resolved
        // at configuration time. The build_setting_default is just the DEFAULT VALUE of the flag,
        // not a regular dep edge. If we stored it as a dep, we'd create false cycles in the
        // dependency graph (e.g., rules_rust's import macro bootstrapping cycle).
        let coerced_default =
            CoercedAttr::String(StringLiteral(ArcStr::from(build_setting_default)));

        let target_node = create_native_target_node(
            rule_defs::LABEL_FLAG_RULE.clone(),
            internals.package(),
            name,
            vec![("build_setting_default".to_owned(), coerced_default)],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Defines a config_setting for use as a key in `select()` statements.
    ///
    /// A config_setting matches when all specified constraint values are present
    /// in the current platform configuration.
    ///
    /// Example:
    /// ```python
    /// config_setting(
    ///     name = "linux_x86_64",
    ///     constraint_values = [
    ///         "@platforms//os:linux",
    ///         "@platforms//cpu:x86_64",
    ///     ],
    /// )
    ///
    /// cc_library(
    ///     name = "mylib",
    ///     srcs = select({
    ///         ":linux_x86_64": ["linux_x86_64_impl.c"],
    ///         "//conditions:default": ["generic_impl.c"],
    ///     }),
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/general#config_setting
    fn config_setting<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        constraint_values: UnpackListOrTuple<String>,
        // Bazel: dict of configuration values (e.g., {"compilation_mode": "opt"})
        // TODO(bazel): Implement values-based config_setting matching
        // Accept Value<'v> to handle both string and Label() keys.
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        values: Value<'v>,
        // Bazel: dict for --define flag values
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        define_values: Value<'v>,
        // Bazel: dict mapping build setting labels to expected values
        // Keys may be Label() objects (e.g., flag_values = {Label("//..."): "val"})
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        flag_values: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (
            define_values,
            flag_values,
            tags,
            testonly,
            deprecation,
            features,
        );
        let internals = ModuleInternals::from_context(eval, "config_setting")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce each constraint_value label to a dep attribute
        let coerced_cvs: Vec<CoercedAttr> = constraint_values
            .items
            .iter()
            .map(|cv| {
                let label = coercion_ctx.coerce_providers_label(cv)?;
                Ok(CoercedAttr::Dep(label))
            })
            .collect::<kuro_error::Result<Vec<_>>>()?;

        let coerced_list = CoercedAttr::List(ListLiteral(kuro_util::arc_str::ArcSlice::from_iter(
            coerced_cvs,
        )));

        // Coerce the values dict (Bazel native flag values)
        use kuro_node::attrs::attr_type::dict::DictLiteral;
        use starlark::values::dict::DictRef;
        let coerced_values = if let Some(dict_ref) = DictRef::from_value(values) {
            CoercedAttr::Dict(DictLiteral(ArcSlice::from_iter(dict_ref.iter().map(
                |(k, v)| {
                    let k_str = if let Some(s) = k.unpack_str() {
                        s.to_owned()
                    } else {
                        format!("{}", k)
                    };
                    let v_str = if let Some(s) = v.unpack_str() {
                        s.to_owned()
                    } else {
                        format!("{}", v)
                    };
                    (
                        CoercedAttr::String(StringLiteral(ArcStr::from(k_str.as_str()))),
                        CoercedAttr::String(StringLiteral(ArcStr::from(v_str.as_str()))),
                    )
                },
            ))))
        } else {
            CoercedAttr::Dict(DictLiteral(ArcSlice::default()))
        };

        let vis_strings = extract_visibility_strings(visibility);
        let target_node = create_native_target_node(
            rule_defs::CONFIG_SETTING_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("constraint_values".to_owned(), coerced_list),
                ("values".to_owned(), coerced_values),
            ],
            &vis_strings,
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Defines a package group for visibility control.
    ///
    /// A package_group defines a set of packages that can be used in
    /// visibility specifications.
    ///
    /// See: https://bazel.build/reference/be/functions#package_group
    fn package_group<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        packages: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        includes: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (packages, includes, tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "package_group")?;

        let target_node = create_native_target_node(
            rule_defs::PACKAGE_GROUP_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &["//visibility:public".to_owned()], // package_group is always public
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Defines a toolchain type (a marker for a category of toolchains).
    ///
    /// A toolchain_type is used by the toolchain resolution system to match
    /// toolchain implementations to rules that need them.
    ///
    /// Example:
    /// ```python
    /// toolchain_type(name = "toolchain_type")
    /// ```
    ///
    /// See: https://bazel.build/reference/be/platforms-and-toolchains#toolchain_type
    fn toolchain_type<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        features: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "toolchain_type")?;

        let target_node = create_native_target_node(
            rule_defs::TOOLCHAIN_TYPE_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Generates one or more files using a shell command.
    ///
    /// genrule creates a target that runs a shell command to produce output files.
    /// It is the most general rule available, as it allows arbitrary commands.
    ///
    /// Example:
    /// ```python
    /// genrule(
    ///     name = "gen_header",
    ///     srcs = ["input.txt"],
    ///     outs = ["output.h"],
    ///     cmd = "cp $(location input.txt) $@",
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/general#genrule
    fn genrule<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = "")] cmd: &str,
        #[starlark(require = named, default = "")] cmd_bash: &str,
        #[starlark(require = named, default = "")] cmd_bat: &str,
        #[starlark(require = named, default = "")] cmd_ps: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        outs: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] tools: Value<'v>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        toolchains: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = false)] local: bool,
        #[starlark(require = named, default = "")] message: &str,
        #[starlark(require = named, default = "")] output_to_bindir: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)] tags: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] features: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        exec_compatible_with: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        target_compatible_with: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (
            cmd_bash,
            cmd_bat,
            cmd_ps,
            toolchains,
            executable,
            local,
            message,
            output_to_bindir,
            tags,
            testonly,
            deprecation,
            features,
            exec_compatible_with,
            target_compatible_with,
        );
        let internals = ModuleInternals::from_context(eval, "genrule")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce cmd
        let coerced_cmd = CoercedAttr::String(StringLiteral(ArcStr::from(cmd)));

        // Coerce outs
        let coerced_outs = CoercedAttr::List(ListLiteral(ArcSlice::from_iter(
            outs.items
                .iter()
                .map(|s| CoercedAttr::String(StringLiteral(ArcStr::from(s.as_str())))),
        )));

        // Coerce srcs - accept both lists and select() expressions
        let srcs_attr_type = AttrType::list(AttrType::one_of(vec![
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
            AttrType::source(false),
        ]));
        let srcs_value = if srcs.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            srcs
        };
        let coerced_srcs =
            srcs_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, srcs_value)?;

        // Coerce tools - accept both lists and select() expressions
        let tools_attr_type = AttrType::list(AttrType::one_of(vec![
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
            AttrType::source(false),
        ]));
        let tools_value = if tools.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            tools
        };
        let coerced_tools =
            tools_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, tools_value)?;

        let vis_strings = extract_visibility_strings(visibility);
        let default_vis = internals.default_visibility();

        let target_node = create_native_target_node(
            rule_defs::GENRULE_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("cmd".to_owned(), coerced_cmd),
                ("outs".to_owned(), coerced_outs),
                ("srcs".to_owned(), coerced_srcs),
                ("tools".to_owned(), coerced_tools),
            ],
            &vis_strings,
            &default_vis,
        )?;

        internals.record(target_node)?;

        // In Bazel, each genrule output in `outs` is a separate target.
        // Register each output as a filegroup target so other rules can reference them.
        for out_name in &outs.items {
            let out_target = create_native_target_node(
                rule_defs::FILEGROUP_RULE.clone(),
                internals.package(),
                out_name,
                vec![],
                &vis_strings,
                &default_vis,
            )?;
            internals.record(out_target)?;
        }

        Ok(NoneType)
    }

    /// Defines a platform with constraint values.
    ///
    /// Bazel's `platform()` rule defines a named collection of constraint values
    /// that can be used as a target platform or execution platform.
    fn platform<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        constraint_values: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        parents: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        exec_properties: UnpackDictEntries<&'v str, &'v str>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        flags: UnpackListOrTuple<&str>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)] tags: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        exec_compatible_with: Value<'v>,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (
            flags,
            tags,
            testonly,
            deprecation,
            exec_compatible_with,
            extra_kwargs,
        );
        let internals = ModuleInternals::from_context(eval, "platform")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce constraint_values
        let cv_attr_type =
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let cv_value = eval.heap().alloc(constraint_values.items);
        let coerced_cv = cv_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, cv_value)?;

        // Coerce parents
        let parents_attr_type =
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let parents_value = eval.heap().alloc(parents.items);
        let coerced_parents =
            parents_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, parents_value)?;

        // Coerce exec_properties
        let ep_attr_type = AttrType::dict(AttrType::string(), AttrType::string(), false);
        let ep_value = eval
            .heap()
            .alloc(starlark::values::dict::AllocDict(exec_properties.entries));
        let coerced_ep = ep_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, ep_value)?;

        // Coerce flags
        let flags_coerced = CoercedAttr::List(ListLiteral(ArcSlice::default()));

        let target_node = create_native_target_node(
            rule_defs::PLATFORM_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("constraint_values".to_owned(), coerced_cv),
                ("parents".to_owned(), coerced_parents),
                ("exec_properties".to_owned(), coerced_ep),
                ("flags".to_owned(), flags_coerced),
            ],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Native cc_library rule stub for Bazel compatibility.
    fn cc_library<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] hdrs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_native_cc_target(
            rule_defs::CC_LIBRARY_RULE.clone(),
            name,
            srcs,
            hdrs,
            deps,
            visibility,
            eval,
        )
    }

    /// Native cc_binary rule stub for Bazel compatibility.
    fn cc_binary<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] hdrs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_native_cc_target(
            rule_defs::CC_BINARY_RULE.clone(),
            name,
            srcs,
            hdrs,
            deps,
            visibility,
            eval,
        )
    }

    /// Native cc_test rule stub for Bazel compatibility.
    fn cc_test<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] hdrs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_native_cc_target(
            rule_defs::CC_TEST_RULE.clone(),
            name,
            srcs,
            hdrs,
            deps,
            visibility,
            eval,
        )
    }

    /// Exports source files from this package for use by other packages.
    ///
    /// In Bazel, `exports_files` makes source files accessible as targets so that
    /// other packages can reference them (e.g., via `attr.label(default = "file.txt")`).
    ///
    /// This implementation registers each file as a filegroup target so that
    /// `ctx.file._attr` and `ctx.files._attr` work correctly.
    ///
    /// See: https://bazel.build/reference/be/functions#exports_files
    fn exports_files<'v>(
        #[starlark(default = UnpackListOrTuple::default())] srcs: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        licenses: UnpackListOrTuple<String>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = licenses;
        let internals = ModuleInternals::from_context(eval, "exports_files")?;
        let coercion_ctx = internals.attr_coercion_context();

        let vis_strings = extract_visibility_strings(visibility);
        let default_vis = internals.default_visibility();

        let src_list_type = AttrType::list(AttrType::one_of(vec![
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
            AttrType::source(false),
        ]));

        for src in srcs.items {
            let file_str = match src.unpack_str() {
                Some(s) => s,
                None => continue, // Skip non-string values (e.g., from glob())
            };

            // Create srcs list with just this single source file
            let src_list_value = eval.heap().alloc(vec![src]);
            let coerced_srcs =
                match src_list_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, src_list_value) {
                    Ok(v) => v,
                    Err(_) => continue, // Skip files that can't be coerced
                };

            let target_node = create_native_target_node(
                rule_defs::FILEGROUP_RULE.clone(),
                internals.package(),
                file_str,
                vec![
                    ("srcs".to_owned(), coerced_srcs),
                    (
                        "data".to_owned(),
                        CoercedAttr::List(ListLiteral(ArcSlice::default())),
                    ),
                ],
                &vis_strings,
                &default_vis,
            )?;

            // Silently skip if target already exists (duplicate exports_files entries
            // or conflicts with rule targets are allowed in some Bazel setups).
            let _ = internals.record(target_node);
        }

        Ok(NoneType)
    }

    /// Registers a toolchain implementation.
    ///
    /// A toolchain() target registers a specific toolchain for a toolchain_type.
    /// The toolchain resolution system selects the appropriate toolchain based on
    /// exec_compatible_with and target_compatible_with platform constraints.
    ///
    /// Example:
    /// ```python
    /// toolchain(
    ///     name = "cc_toolchain_linux",
    ///     toolchain_type = "@rules_cc//cc:toolchain_type",
    ///     toolchain = ":cc_toolchain",
    ///     exec_compatible_with = ["@platforms//os:linux"],
    ///     target_compatible_with = ["@platforms//os:linux"],
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/platforms-and-toolchains#toolchain
    fn toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] toolchain_type: Value<'v>,
        #[starlark(require = named)] toolchain: Value<'v>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        exec_compatible_with: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        target_compatible_with: UnpackListOrTuple<Value<'v>>,
        // target_settings: config_setting labels that must match for this toolchain to be selected
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        target_settings: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] testonly: Value<
            'v,
        >,
        #[starlark(require = named, default = "")] deprecation: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (
            tags,
            testonly,
            deprecation,
            target_settings,
            exec_compatible_with,
            target_compatible_with,
        );
        let internals = ModuleInternals::from_context(eval, "toolchain")?;
        let coercion_ctx = internals.attr_coercion_context();

        let dep_type = AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY);
        let coerced_toolchain_type =
            dep_type.coerce(AttrIsConfigurable::No, coercion_ctx, toolchain_type)?;
        let coerced_toolchain = dep_type.coerce(AttrIsConfigurable::No, coercion_ctx, toolchain)?;

        // Note: exec_compatible_with and target_compatible_with are internal attributes
        // already present in the rule's AttributeSpec. We do not pass them as user attrs here;
        // they use their empty-list defaults. Toolchain resolution is not yet implemented.
        let target_node = create_native_target_node(
            rule_defs::TOOLCHAIN_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("toolchain_type".to_owned(), coerced_toolchain_type),
                ("toolchain".to_owned(), coerced_toolchain),
            ],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Bazel's sh_binary rule: creates a binary from shell scripts.
    ///
    /// See: https://bazel.build/reference/be/shell#sh_binary
    fn sh_binary<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_sh_target(
            rule_defs::SH_BINARY_RULE.clone(),
            name,
            srcs,
            deps,
            visibility,
            eval,
        )
    }

    /// Bazel's sh_test rule: creates a test target from shell scripts.
    ///
    /// See: https://bazel.build/reference/be/shell#sh_test
    fn sh_test<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_sh_target(
            rule_defs::SH_TEST_RULE.clone(),
            name,
            srcs,
            deps,
            visibility,
            eval,
        )
    }

    /// Bazel's sh_library rule: creates a library from shell scripts.
    ///
    /// See: https://bazel.build/reference/be/shell#sh_library
    fn sh_library<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        create_sh_target(
            rule_defs::SH_LIBRARY_RULE.clone(),
            name,
            srcs,
            deps,
            visibility,
            eval,
        )
    }

    /// Native test_suite rule stub for Bazel compatibility.
    fn test_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        tests: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        let internals = ModuleInternals::from_context(eval, "test_suite")?;
        let coercion_ctx = internals.attr_coercion_context();

        let dep_type = AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let tests_val = eval.heap().alloc(tests.items);
        let coerced_tests = dep_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, tests_val)?;

        let target_node = create_native_target_node(
            rule_defs::TEST_SUITE_RULE.clone(),
            internals.package(),
            name,
            vec![("tests".to_owned(), coerced_tests)],
            &extract_visibility_strings(visibility),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }
}
