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
use kuro_core::package::package_relative_path::PackageRelativePathBuf;
use kuro_core::plugins::PluginKindSet;
use kuro_core::provider::label::ProvidersLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_core::target::name::TargetNameRef;
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::bool::BoolLiteral;
use kuro_node::attrs::attr_type::list::ListLiteral;
use kuro_node::attrs::attr_type::string::StringLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::coerced_deps_collector::CoercedDeps;
use kuro_node::attrs::coerced_deps_collector::CoercedDepsCollector;
use kuro_node::attrs::coerced_path::CoercedPath;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use kuro_node::attrs::spec::AttributeId;
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
use kuro_node::visibility::VisibilitySpecification;
use kuro_node::visibility::VisibilityWithinViewBuilder;
use kuro_util::arc_str::ArcSlice;
use kuro_util::arc_str::ArcStr;
use once_cell::sync::Lazy;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::Value;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;

use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::attr_type::visibility::parse_visibility_from_strs;
use crate::interpreter::module_internals::ModuleInternals;

/// Pre-built Rule definitions for native rules.
/// These are created once and reused across all invocations.
pub(crate) mod rule_defs {
    use super::*;

    /// Helper to construct a native `Rule` with the common sentinel defaults
    /// (no incoming transition, no plugins/providers/toolchains/exec groups/fragments,
    /// not a build setting). All native rules in this file share those defaults; they
    /// vary only in `attributes`, `rule_type`, `rule_kind`, `is_test`, and `is_executable`.
    fn make_native_rule(
        attributes: AttributeSpec,
        kind: NativeRuleKind,
        rule_kind: RuleKind,
        is_test: bool,
        is_executable: bool,
    ) -> Arc<Rule> {
        Arc::new(Rule {
            attributes,
            rule_type: RuleType::Native(kind),
            rule_kind,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test,
            is_executable,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    }

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
        make_native_rule(
            constraint_setting_attributes(),
            NativeRuleKind::ConstraintSetting,
            RuleKind::Configuration,
            false,
            false,
        )
    });

    /// The Rule definition for constraint_value.
    pub static CONSTRAINT_VALUE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            constraint_value_attributes(),
            NativeRuleKind::ConstraintValue,
            RuleKind::Configuration,
            false,
            false,
        )
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
        make_native_rule(
            alias_attributes(),
            NativeRuleKind::Alias,
            RuleKind::Normal, // Aliases can be used anywhere
            false,
            false,
        )
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
        make_native_rule(
            filegroup_attributes(),
            NativeRuleKind::Filegroup,
            RuleKind::Normal,
            false,
            false,
        )
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
        make_native_rule(
            label_flag_attributes(),
            NativeRuleKind::LabelFlag,
            RuleKind::Normal,
            false,
            false,
        )
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

        // flag_values: dict mapping build setting label strings to expected string values.
        // Keys are stored as strings (not deps) to avoid hard errors when the referenced
        // flag target doesn't exist (e.g., bazel_tools//tools/cpp:compiler). The
        // check_config_setting_flag_values function gracefully handles missing targets.
        let flag_values_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Dict(
                DictLiteral(ArcSlice::default()),
            ))),
            "A dictionary of build setting labels to expected values",
            AttrType::dict(AttrType::string(), AttrType::string(), false),
        );

        // define_values: dict mapping --define key names to expected values.
        // config_setting(define_values = {"FOO": "bar"}) matches when --define FOO=bar.
        let define_values_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Dict(
                DictLiteral(ArcSlice::default()),
            ))),
            "A dictionary of --define flag keys to expected values",
            AttrType::dict(AttrType::string(), AttrType::string(), false),
        );

        AttributeSpec::from(
            vec![
                ("constraint_values".to_owned(), constraint_values_attr),
                ("values".to_owned(), values_attr),
                ("flag_values".to_owned(), flag_values_attr),
                ("define_values".to_owned(), define_values_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("config_setting attributes should be valid")
    }

    /// The Rule definition for config_setting.
    pub static CONFIG_SETTING_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            config_setting_attributes(),
            NativeRuleKind::ConfigSetting,
            RuleKind::Configuration,
            false,
            false,
        )
    });

    /// The Rule definition for package_group.
    /// package_group defines a set of packages for visibility control.
    pub static PACKAGE_GROUP_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            constraint_setting_attributes(), // Same as constraint_setting - just name + visibility
            NativeRuleKind::PackageGroup,
            RuleKind::Normal,
            false,
            false,
        )
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
        make_native_rule(
            toolchain_attributes(),
            NativeRuleKind::Toolchain,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// The Rule definition for toolchain_type.
    /// toolchain_type is a simple marker target with no special attributes.
    pub static TOOLCHAIN_TYPE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            constraint_setting_attributes(), // Same as constraint_setting - just name + visibility
            NativeRuleKind::ToolchainType,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// Creates the AttributeSpec for genrule.
    /// genrule has cmd, cmd_bash, cmd_bat, cmd_ps, outs, srcs, tools attributes.
    fn genrule_attributes() -> AttributeSpec {
        let cmd_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "The command to run",
            AttrType::string(),
        );

        let cmd_bash_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "Bash-specific command (preferred over cmd on Unix)",
            AttrType::string(),
        );

        let cmd_bat_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "Windows CMD.exe command (takes priority over cmd on Windows when cmd_ps is absent)",
            AttrType::string(),
        );

        let cmd_ps_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                "",
            ))))),
            "Windows PowerShell command (takes highest priority on Windows)",
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

        // `tools` is Bazel-compatible `cfg="exec"`: the referenced binaries are
        // built in the execution configuration so they run on the build host
        // regardless of the target platform. Plan 19.3 routes the resulting
        // exec cfg through `exec_properties` → `build_settings`, giving tools
        // like llvm-tblgen an opt compilation mode by default. Plan 20.1
        // restored this after an earlier attempt exposed a siphash
        // `_virtual_includes` regression — see commit history.
        let tools_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Tool dependencies for the command (exec-configured)",
            AttrType::list(AttrType::one_of(vec![
                AttrType::exec_dep(ProviderIdSet::EMPTY),
                AttrType::source(false),
            ])),
        );

        AttributeSpec::from(
            vec![
                ("cmd".to_owned(), cmd_attr),
                ("cmd_bash".to_owned(), cmd_bash_attr),
                ("cmd_bat".to_owned(), cmd_bat_attr),
                ("cmd_ps".to_owned(), cmd_ps_attr),
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
        make_native_rule(
            genrule_attributes(),
            NativeRuleKind::Genrule,
            RuleKind::Normal,
            false,
            false,
        )
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

        // `exec_properties` lives as an internal attribute (Plan 24 Phase 2)
        // because Bazel exposes it as a common attribute on every rule —
        // both `platform()` (where the dict drives RE worker selection)
        // and ordinary rules (where the dict overrides keys on the
        // resolved exec platform's properties at action time). Unifying
        // them avoids two competing attribute definitions; the analysis
        // path reads via `configured_node.get("exec_properties", ...)`.

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
                ("flags".to_owned(), flags_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("platform attributes should be valid")
    }

    pub static PLATFORM_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            platform_attributes(),
            NativeRuleKind::Platform,
            RuleKind::Configuration,
            false,
            false,
        )
    });

    pub static EXECUTION_PLATFORM_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        let platform_attr = Attribute::new(
            None, // required, no default
            "The platform target for this execution platform",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );
        let attributes = AttributeSpec::from(
            vec![("platform".to_owned(), platform_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("execution_platform attributes should be valid");
        make_native_rule(
            attributes,
            NativeRuleKind::ExecutionPlatform,
            RuleKind::Configuration,
            false,
            false,
        )
    });

    pub static EXECUTION_PLATFORMS_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        let platforms_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The list of execution platform targets",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let attributes = AttributeSpec::from(
            vec![("platforms".to_owned(), platforms_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("execution_platforms attributes should be valid");
        make_native_rule(
            attributes,
            NativeRuleKind::ExecutionPlatforms,
            RuleKind::Configuration,
            false,
            false,
        )
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
        make_native_rule(
            cc_rule_attributes(),
            NativeRuleKind::CcLibrary,
            RuleKind::Normal,
            false,
            false,
        )
    });

    pub static CC_BINARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            cc_rule_attributes(),
            NativeRuleKind::CcBinary,
            RuleKind::Normal,
            false,
            true,
        )
    });

    pub static CC_TEST_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            cc_rule_attributes(),
            NativeRuleKind::CcTest,
            RuleKind::Normal,
            true,
            false,
        )
    });

    fn test_suite_attributes() -> AttributeSpec {
        // `tests` is an internal attribute (ID 8) with type AttrType::list(AttrType::label()).
        // We populate it by passing ("tests", coerced_label_list) to create_native_target_node,
        // which finds "tests" via attr_specs() (internal attrs are always included) and stores it.
        // node.tests() then reads TESTS_ATTRIBUTE (ID 8) and returns the test labels for expansion.
        AttributeSpec::from(vec![], false, &RuleIncomingTransition::None)
            .expect("test_suite attributes should be valid")
    }

    pub static TEST_SUITE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            test_suite_attributes(),
            NativeRuleKind::TestSuite,
            RuleKind::Normal,
            false,
            false,
        )
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
        make_native_rule(
            sh_rule_attributes(),
            NativeRuleKind::ShBinary,
            RuleKind::Normal,
            false,
            true,
        )
    });

    pub static SH_TEST_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            sh_rule_attributes(),
            NativeRuleKind::ShTest,
            RuleKind::Normal,
            true,
            false,
        )
    });

    pub static SH_LIBRARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            sh_rule_attributes(),
            NativeRuleKind::ShLibrary,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// The Rule definition for cc_libc_top_alias.
    /// This is a Bazel internal rule used by rules_cc to alias the libc top directory.
    /// It's a simple stub with no extra attributes beyond name/visibility.
    pub static CC_LIBC_TOP_ALIAS_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            constraint_setting_attributes(), // Same as constraint_setting - just name + visibility
            NativeRuleKind::CcLibcTopAlias,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// The Rule definition for analysis_test.
    /// Created by `testing.analysis_test()` - an analysis-time test with no build actions.
    pub static ANALYSIS_TEST_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            constraint_setting_attributes(), // Minimal: just name + visibility
            NativeRuleKind::AnalysisTest,
            RuleKind::Normal,
            true,
            false,
        )
    });

    /// Attributes for the genquery rule.
    fn genquery_attributes() -> AttributeSpec {
        let expression_attr = Attribute::new(None, "Query expression to run", AttrType::string());
        let scope_attr = Attribute::new(
            None,
            "Labels of targets that bound the universe of targets the query is allowed to access",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let opts_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Query options (e.g. --output=label)",
            AttrType::list(AttrType::string()),
        );
        let strict_attr = {
            let v = Arc::new(CoercedAttr::Bool(BoolLiteral(true)));
            Attribute::new(
                Some(v),
                "Fail if targets outside scope are referenced",
                AttrType::bool(),
            )
        };

        AttributeSpec::from(
            vec![
                ("expression".to_owned(), expression_attr),
                ("scope".to_owned(), scope_attr),
                ("opts".to_owned(), opts_attr),
                ("strict".to_owned(), strict_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("genquery attributes should be valid")
    }

    /// The Rule definition for genquery.
    pub static GENQUERY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            genquery_attributes(),
            NativeRuleKind::Genquery,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// Attributes for starlark_doc_extract rule.
    fn starlark_doc_extract_attributes() -> AttributeSpec {
        let src_attr = Attribute::new(
            None,
            "src",
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        );
        let symbol_names_attr = Attribute::new(
            None,
            "The symbol names to extract documentation for",
            AttrType::list(AttrType::string()),
        );

        AttributeSpec::from(
            vec![
                ("src".to_owned(), src_attr),
                ("symbol_names".to_owned(), symbol_names_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("starlark_doc_extract attributes should be valid")
    }

    /// The Rule definition for starlark_doc_extract.
    pub static STARLARK_DOC_EXTRACT_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        make_native_rule(
            starlark_doc_extract_attributes(),
            NativeRuleKind::StarlarkDocExtract,
            RuleKind::Normal,
            false,
            false,
        )
    });

    /// Attributes for cc_toolchain rule.
    /// Accepts all_files (label dep), toolchain_config (label dep), toolchain_identifier (string),
    /// and common attributes. Extra kwargs are silently accepted for forward compat.
    fn cc_toolchain_attributes() -> AttributeSpec {
        let all_files_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "all_files",
            AttrType::option(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let toolchain_config_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "toolchain_config",
            AttrType::option(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let toolchain_identifier_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::String(StringLiteral::default()))),
            "toolchain_identifier",
            AttrType::string(),
        );

        AttributeSpec::from(
            vec![
                ("all_files".to_owned(), all_files_attr),
                ("toolchain_config".to_owned(), toolchain_config_attr),
                ("toolchain_identifier".to_owned(), toolchain_identifier_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("cc_toolchain attributes should be valid")
    }

    /// The Rule definition for cc_toolchain.
    pub static CC_TOOLCHAIN_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_toolchain_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcToolchain),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });

    /// Attributes for cc_toolchain_suite rule.
    /// Accepts toolchains (dict of string to label).
    fn cc_toolchain_suite_attributes() -> AttributeSpec {
        let toolchains_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "toolchains",
            AttrType::option(AttrType::dict(
                AttrType::string(),
                AttrType::string(),
                false,
            )),
        );

        AttributeSpec::from(
            vec![("toolchains".to_owned(), toolchains_attr)],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("cc_toolchain_suite attributes should be valid")
    }

    /// The Rule definition for cc_toolchain_suite.
    pub static CC_TOOLCHAIN_SUITE_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_toolchain_suite_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcToolchainSuite),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });

    /// Attributes for cc_import rule.
    /// cc_import provides a way to import prebuilt C/C++ libraries.
    fn cc_import_attributes() -> AttributeSpec {
        let static_library_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "A prebuilt static library (.a or .lib)",
            AttrType::option(AttrType::source(false)),
        );
        let shared_library_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "A prebuilt shared library (.so, .dylib, or .dll)",
            AttrType::option(AttrType::source(false)),
        );
        let interface_library_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "An interface library for linking (.ifso or import .lib)",
            AttrType::option(AttrType::source(false)),
        );
        let hdrs_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Header files for this library",
            AttrType::list(AttrType::source(false)),
        );
        let system_provided_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Bool(BoolLiteral(false)))),
            "If true, the shared library is provided by the system",
            AttrType::bool(),
        );
        let alwayslink_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::Bool(BoolLiteral(false)))),
            "If true, always link the static library",
            AttrType::bool(),
        );

        AttributeSpec::from(
            vec![
                ("static_library".to_owned(), static_library_attr),
                ("shared_library".to_owned(), shared_library_attr),
                ("interface_library".to_owned(), interface_library_attr),
                ("hdrs".to_owned(), hdrs_attr),
                ("system_provided".to_owned(), system_provided_attr),
                ("alwayslink".to_owned(), alwayslink_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("cc_import attributes should be valid")
    }

    /// The Rule definition for cc_import.
    pub static CC_IMPORT_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_import_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcImport),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });

    /// Attributes for cc_shared_library rule.
    /// cc_shared_library produces a shared library from cc_library dependencies.
    fn cc_shared_library_attributes() -> AttributeSpec {
        let deps_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Direct dependencies to include in the shared library",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let exports_filter_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Labels of targets whose symbols should be exported",
            AttrType::list(AttrType::string()),
        );
        let dynamic_deps_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Other cc_shared_library dependencies",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let roots_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Root targets from which to start dependency collection",
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY)),
        );
        let shared_lib_name_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::None)),
            "Override output shared library name",
            AttrType::option(AttrType::string()),
        );
        let user_link_flags_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "Additional linker flags",
            AttrType::list(AttrType::string()),
        );

        AttributeSpec::from(
            vec![
                ("deps".to_owned(), deps_attr),
                ("exports_filter".to_owned(), exports_filter_attr),
                ("dynamic_deps".to_owned(), dynamic_deps_attr),
                ("roots".to_owned(), roots_attr),
                ("shared_lib_name".to_owned(), shared_lib_name_attr),
                ("user_link_flags".to_owned(), user_link_flags_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("cc_shared_library attributes should be valid")
    }

    /// The Rule definition for cc_shared_library.
    pub static CC_SHARED_LIBRARY_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: cc_shared_library_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::CcSharedLibrary),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });

    fn environment_group_attributes() -> AttributeSpec {
        let environments_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The environments in this group",
            AttrType::list(AttrType::string()),
        );
        let defaults_attr = Attribute::new(
            Some(Arc::new(CoercedAttr::List(
                ListLiteral(ArcSlice::default()),
            ))),
            "The default environments from this group",
            AttrType::list(AttrType::string()),
        );
        AttributeSpec::from(
            vec![
                ("environments".to_owned(), environments_attr),
                ("defaults".to_owned(), defaults_attr),
            ],
            false,
            &RuleIncomingTransition::None,
        )
        .expect("environment_group attributes should be valid")
    }

    pub static ENVIRONMENT_GROUP_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: environment_group_attributes(),
            rule_type: RuleType::Native(NativeRuleKind::EnvironmentGroup),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });

    /// Stub xcode_config rule for non-Apple platforms.
    /// Provides XcodeVersionConfig with dummy values so cc_toolchain_config.bzl
    /// can execute without crashing on Linux.
    pub static XCODE_CONFIG_RULE: Lazy<Arc<Rule>> = Lazy::new(|| {
        Arc::new(Rule {
            attributes: constraint_setting_attributes(), // No user-defined attrs needed
            rule_type: RuleType::Native(NativeRuleKind::XcodeConfig),
            rule_kind: RuleKind::Normal,
            cfg: RuleIncomingTransition::None,
            uses_plugins: vec![],
            is_test: false,
            is_executable: false,
            provides: vec![],
            toolchain_types: vec![],
            exec_group_defs: vec![],
            fragments: vec![],
            build_setting_type: None,
            build_setting_is_flag: false,
        })
    });
}

/// Extract visibility strings from a Starlark value.
/// Handles None (returns empty), list of strings, or tuple of strings.
pub(crate) fn extract_visibility_strings(value: starlark::values::Value) -> Vec<String> {
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
pub(crate) fn create_native_target_node(
    rule: Arc<Rule>,
    package: Arc<Package>,
    target_name: &str,
    attrs: Vec<(String, CoercedAttr)>,
    visibility: &[String],
    coercion_ctx: &dyn AttrCoercionContext,
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
    let visibility_spec = if visibility.is_empty() {
        default_visibility.dupe()
    } else {
        parse_visibility_from_strs(coercion_ctx, visibility)
            .unwrap_or_else(|_| VisibilityWithinViewBuilder::with_capacity(0))
            .build_visibility()
    };
    attr_values.push_sorted(
        VISIBILITY_ATTRIBUTE.id,
        CoercedAttr::Visibility(visibility_spec),
    );

    // Resolve attribute IDs from the spec, then insert in id order so
    // `push_sorted`'s monotonic-id invariant holds. Without sorting,
    // a user-provided attribute whose id falls between internal
    // attribute ids (e.g. `exec_properties` at id 16, internal,
    // alongside rule-specific attrs that occupy higher ids) would
    // panic with "attributes must be sorted".
    let mut resolved: Vec<(AttributeId, CoercedAttr)> = attrs
        .into_iter()
        .filter_map(|(attr_name, coerced_value)| {
            rule.attributes
                .attr_specs()
                .find(|(name, _, _)| *name == attr_name)
                .map(|(_, attr_id, _)| (attr_id, coerced_value))
        })
        .collect();
    resolved.sort_by_key(|(id, _)| *id);
    for (attr_id, coerced_value) in resolved {
        attr_values.push_sorted(attr_id, coerced_value);
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
        internals.attr_coercion_context(),
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
        internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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
        #[starlark(require = named)] build_setting_default: Value<'v>,
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "label_flag")?;

        // Accept both string and Label types for build_setting_default.
        // In Bazel, label_flag accepts Label("//pkg:target") or "//pkg:target" string.
        let default_str = if let Some(s) = build_setting_default.unpack_str() {
            s.to_owned()
        } else {
            // For Label or other types, use str() representation
            format!("{}", build_setting_default)
        };
        let coerced_default =
            CoercedAttr::String(StringLiteral(ArcStr::from(default_str.as_str())));

        let target_node = create_native_target_node(
            rule_defs::LABEL_FLAG_RULE.clone(),
            internals.package(),
            name,
            vec![("build_setting_default".to_owned(), coerced_default)],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
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
        // Matching implemented in calculation.rs resolve_bazel_config_value().
        // Accept Value<'v> to handle both string and Label() keys.
        #[starlark(require = named, default = starlark::values::none::NoneType)] values: Value<'v>,
        // Bazel: dict for --define flag values
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        define_values: Value<'v>,
        // Bazel: dict mapping build setting labels to expected values
        // Keys may be Label() objects (e.g., flag_values = {Label("//..."): "val"})
        #[starlark(require = named, default = starlark::values::none::NoneType)] flag_values: Value<
            'v,
        >,
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (tags, testonly, deprecation, features);
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

        // Coerce the flag_values dict: keys are build setting labels (coerced as deps),
        // values are expected string values. Keys may be Label() objects or label strings.
        let coerced_flag_values = if let Some(dict_ref) = DictRef::from_value(flag_values) {
            let pairs: Vec<(CoercedAttr, CoercedAttr)> = dict_ref
                .iter()
                .map(|(k, v)| {
                    // Key is a label (possibly a Label() object or a string).
                    // Store as a string to avoid hard errors when the flag target
                    // doesn't exist (e.g., bazel_tools//tools/cpp:compiler).
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
                    Ok((
                        CoercedAttr::String(StringLiteral(ArcStr::from(k_str.as_str()))),
                        CoercedAttr::String(StringLiteral(ArcStr::from(v_str.as_str()))),
                    ))
                })
                .collect::<kuro_error::Result<Vec<_>>>()?;
            CoercedAttr::Dict(DictLiteral(ArcSlice::from_iter(pairs)))
        } else {
            CoercedAttr::Dict(DictLiteral(ArcSlice::default()))
        };

        // Coerce define_values dict the same way as values dict
        let coerced_define_values = if let Some(dict_ref) = DictRef::from_value(define_values) {
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
                ("flag_values".to_owned(), coerced_flag_values),
                ("define_values".to_owned(), coerced_define_values),
            ],
            &vis_strings,
            internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (tags, testonly, deprecation, features);
        let internals = ModuleInternals::from_context(eval, "package_group")?;

        // Register the package_group in the global registry for visibility resolution
        let pkg_label = internals.package().buildfile_path.package();
        let pkg_path = pkg_label.cell_relative_path().as_str();
        let group_label = format!("//{}:{}", pkg_path, name);
        kuro_node::visibility::register_package_group(&group_label, packages.items, includes.items);

        let target_node = create_native_target_node(
            rule_defs::PACKAGE_GROUP_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &["//visibility:public".to_owned()], // package_group is always public
            internals.attr_coercion_context(),
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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
        #[starlark(require = named, default = starlark::values::none::NoneType)] cmd: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] cmd_bash: Value<
            'v,
        >,
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
        // Bazel accepts int (0/1) or bool here; upstream .bazel files use both
        // forms. Take `Value` and let it be coerced downstream — we currently
        // ignore the actual value.
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        output_to_bindir: Value<'v>,
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _unused = (
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

        // Coerce cmd, cmd_bash, cmd_bat, cmd_ps as configurable strings (support select())
        let cmd_attr_type = AttrType::string();
        let cmd_value = if cmd.is_none() {
            eval.heap().alloc("")
        } else {
            cmd
        };
        let coerced_cmd = cmd_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, cmd_value)?;

        let cmd_bash_value = if cmd_bash.is_none() {
            eval.heap().alloc("")
        } else {
            cmd_bash
        };
        let coerced_cmd_bash =
            cmd_attr_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, cmd_bash_value)?;

        let coerced_cmd_bat = cmd_attr_type.coerce(
            AttrIsConfigurable::Yes,
            coercion_ctx,
            eval.heap().alloc(cmd_bat),
        )?;

        let coerced_cmd_ps = cmd_attr_type.coerce(
            AttrIsConfigurable::Yes,
            coercion_ctx,
            eval.heap().alloc(cmd_ps),
        )?;

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
                ("cmd_bash".to_owned(), coerced_cmd_bash),
                ("cmd_bat".to_owned(), coerced_cmd_bat),
                ("cmd_ps".to_owned(), coerced_cmd_ps),
                ("outs".to_owned(), coerced_outs),
                ("srcs".to_owned(), coerced_srcs),
                ("tools".to_owned(), coerced_tools),
            ],
            &vis_strings,
            coercion_ctx,
            &default_vis,
        )?;

        internals.record(target_node)?;

        // In Bazel, each genrule output in `outs` is a separate target.
        // Register each output as a filegroup target that forwards to the genrule.
        // This allows other rules to depend on specific genrule outputs by name,
        // and the filegroup's DefaultInfo.files will contain the genrule's artifacts.
        let genrule_label = ProvidersLabel::default_for(TargetLabel::new(
            internals.package().buildfile_path.package().dupe(),
            TargetNameRef::new(name)?,
        ));
        // The filegroup srcs attr is `list(one_of(dep(), source()))`.
        // When manually creating a CoercedAttr::Dep and putting it in a one_of list,
        // we must wrap it in CoercedAttr::OneOf(value, index) where index=0 means
        // the first one_of variant (dep()). Otherwise pack() fails with type mismatch.
        let genrule_dep_attr = CoercedAttr::OneOf(Box::new(CoercedAttr::Dep(genrule_label)), 0);
        let srcs_attr = CoercedAttr::List(ListLiteral(ArcSlice::from_iter(std::iter::once(
            genrule_dep_attr,
        ))));
        for out_name in &outs.items {
            let out_target = create_native_target_node(
                rule_defs::FILEGROUP_RULE.clone(),
                internals.package(),
                out_name,
                vec![("srcs".to_owned(), srcs_attr.clone())],
                &vis_strings,
                coercion_ctx,
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
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Native execution_platform rule.
    fn execution_platform<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] platform: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "execution_platform")?;
        let coercion_ctx = internals.attr_coercion_context();
        let mut attrs = vec![];
        if !platform.is_none() {
            let dep_type = AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY);
            let coerced = dep_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, platform)?;
            attrs.push(("platform".to_owned(), coerced));
        }
        let target_node = create_native_target_node(
            rule_defs::EXECUTION_PLATFORM_RULE.clone(),
            internals.package(),
            name,
            attrs,
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Native execution_platforms rule.
    fn execution_platforms<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        platforms: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "execution_platforms")?;
        let coercion_ctx = internals.attr_coercion_context();
        let platforms_val = eval.heap().alloc(platforms.items);
        let platforms_type =
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let coerced_platforms =
            platforms_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, platforms_val)?;
        let target_node = create_native_target_node(
            rule_defs::EXECUTION_PLATFORMS_RULE.clone(),
            internals.package(),
            name,
            vec![("platforms".to_owned(), coerced_platforms)],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
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

        for src in srcs.items {
            let file_str = match src.unpack_str() {
                Some(s) => s,
                None => continue, // Skip non-string values (e.g., from glob())
            };

            // Construct the srcs list manually to avoid self-referential cycles.
            // exports_files creates a filegroup named "X" with srcs=["X"]; if coerced
            // via one_of(dep, source), "X" resolves as dep to the same target, creating
            // a cycle. Instead, construct the CoercedAttr directly as a source file,
            // using OneOf index 1 (source) to match filegroup's srcs type definition.
            let file_path = PackageRelativePathBuf::unchecked_new(file_str.to_owned());
            let source_file =
                CoercedAttr::SourceFile(CoercedPath::File(file_path.as_path().to_arc()));
            let one_of = CoercedAttr::OneOf(Box::new(source_file), 1);
            let srcs_list = CoercedAttr::List(ListLiteral(ArcSlice::new([one_of])));

            let target_node = create_native_target_node(
                rule_defs::FILEGROUP_RULE.clone(),
                internals.package(),
                file_str,
                vec![
                    ("srcs".to_owned(), srcs_list),
                    (
                        "data".to_owned(),
                        CoercedAttr::List(ListLiteral(ArcSlice::default())),
                    ),
                ],
                &vis_strings,
                coercion_ctx,
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
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
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
            internals.attr_coercion_context(),
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

        // Coerce tests as a label list (matching TESTS_ATTRIBUTE type: AttrType::list(AttrType::label())).
        // This stores the tests in the internal TESTS_ATTRIBUTE (ID 8) via create_native_target_node,
        // which makes node.tests() return them correctly for test suite expansion in the test runner.
        let label_list_type = AttrType::list(AttrType::label());
        let tests_val = eval.heap().alloc(tests.items);
        let coerced_tests =
            label_list_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, tests_val)?;

        let target_node = create_native_target_node(
            rule_defs::TEST_SUITE_RULE.clone(),
            internals.package(),
            name,
            vec![("tests".to_owned(), coerced_tests)],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Stub for Bazel's cc_libc_top_alias rule used internally by rules_cc.
    ///
    /// `cc_libc_top_alias` creates an alias to the libc top directory for toolchain
    /// configuration. In practice it's a no-op stub that produces empty DefaultInfo.
    ///
    /// See: rules_cc/cc/BUILD for usage context.
    fn cc_libc_top_alias<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        let internals = ModuleInternals::from_context(eval, "cc_libc_top_alias")?;

        let target_node = create_native_target_node(
            rule_defs::CC_LIBC_TOP_ALIAS_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Bazel's genquery rule: runs a query and writes results to an output file.
    ///
    /// This is a stub implementation that creates an empty output file. Full query
    /// execution would require integrating with the Kuro query engine at build time.
    ///
    /// See: https://bazel.build/reference/be/general#genquery
    fn genquery<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] expression: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] scope: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] opts: Value<'v>,
        #[starlark(require = named, default = true)] strict: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        let internals = ModuleInternals::from_context(eval, "genquery")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce expression (required string)
        let coerced_expression = CoercedAttr::String(StringLiteral(ArcStr::from(expression)));

        // Coerce scope (list of dep labels, optional - default to empty list)
        let scope_attr_type =
            AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let scope_value = if scope.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            scope
        };
        let coerced_scope =
            scope_attr_type.coerce(AttrIsConfigurable::No, coercion_ctx, scope_value)?;

        // Coerce opts (list of strings, optional - default to empty list)
        let opts_attr_type = AttrType::list(AttrType::string());
        let opts_value = if opts.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            opts
        };
        let coerced_opts =
            opts_attr_type.coerce(AttrIsConfigurable::No, coercion_ctx, opts_value)?;

        // Coerce strict (bool)
        let coerced_strict = CoercedAttr::Bool(BoolLiteral(strict));

        let target_node = create_native_target_node(
            rule_defs::GENQUERY_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("expression".to_owned(), coerced_expression),
                ("scope".to_owned(), coerced_scope),
                ("opts".to_owned(), coerced_opts),
                ("strict".to_owned(), coerced_strict),
            ],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Bazel's starlark_doc_extract rule: extracts documentation from .bzl files.
    ///
    /// This is a stub implementation that creates an empty output file.
    /// Its primary purpose is to make `hasattr(native, "starlark_doc_extract")` return
    /// True, which rules_python uses as a Bazel 7+ feature detection signal.
    ///
    /// See: https://bazel.build/reference/be/general#starlark_doc_extract
    fn starlark_doc_extract<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] src: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        symbol_names: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = extra_kwargs;
        let internals = ModuleInternals::from_context(eval, "starlark_doc_extract")?;
        let coercion_ctx = internals.attr_coercion_context();

        // Coerce src (required dep label)
        let src_attr_type = AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY);
        let coerced_src = src_attr_type.coerce(AttrIsConfigurable::No, coercion_ctx, src)?;

        // Coerce symbol_names (list of strings, optional)
        let names_attr_type = AttrType::list(AttrType::string());
        let names_value = if symbol_names.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            symbol_names
        };
        let coerced_names =
            names_attr_type.coerce(AttrIsConfigurable::No, coercion_ctx, names_value)?;

        let target_node = create_native_target_node(
            rule_defs::STARLARK_DOC_EXTRACT_RULE.clone(),
            internals.package(),
            name,
            vec![
                ("src".to_owned(), coerced_src),
                ("symbol_names".to_owned(), coerced_names),
            ],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Bazel's cc_import rule: imports a prebuilt C/C++ library.
    ///
    /// Creates a target that provides CcInfo from a prebuilt static or shared library.
    /// Used for integrating pre-compiled libraries into the build graph.
    ///
    /// See: https://bazel.build/reference/be/c-cpp#cc_import
    fn cc_import<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        static_library: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        shared_library: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        interface_library: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] hdrs: Value<'v>,
        #[starlark(require = named, default = false)] system_provided: bool,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "cc_import")?;
        let coercion_ctx = internals.attr_coercion_context();

        let mut attrs = Vec::new();

        // Coerce optional source attributes
        let opt_source_type = AttrType::option(AttrType::source(false));

        if !static_library.is_none() {
            let coerced =
                opt_source_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, static_library)?;
            attrs.push(("static_library".to_owned(), coerced));
        }
        if !shared_library.is_none() {
            let coerced =
                opt_source_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, shared_library)?;
            attrs.push(("shared_library".to_owned(), coerced));
        }
        if !interface_library.is_none() {
            let coerced =
                opt_source_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, interface_library)?;
            attrs.push(("interface_library".to_owned(), coerced));
        }

        // Coerce hdrs (list of sources)
        let hdrs_type = AttrType::list(AttrType::source(false));
        let hdrs_value = if hdrs.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            hdrs
        };
        let coerced_hdrs = hdrs_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, hdrs_value)?;
        attrs.push(("hdrs".to_owned(), coerced_hdrs));

        // Bool attributes
        attrs.push((
            "system_provided".to_owned(),
            CoercedAttr::Bool(BoolLiteral(system_provided)),
        ));
        attrs.push((
            "alwayslink".to_owned(),
            CoercedAttr::Bool(BoolLiteral(alwayslink)),
        ));

        let target_node = create_native_target_node(
            rule_defs::CC_IMPORT_RULE.clone(),
            internals.package(),
            name,
            attrs,
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Creates a shared library from cc_library dependencies.
    ///
    /// `cc_shared_library` produces a shared library (.so, .dylib, .dll) that includes
    /// all of its `deps` and their transitive dependencies, minus any dependencies
    /// already provided by `dynamic_deps`.
    ///
    /// Example:
    /// ```python
    /// cc_shared_library(
    ///     name = "my_shared_lib",
    ///     deps = [":my_lib"],
    ///     exports_filter = ["//my_package:__subpackages__"],
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/c-cpp#cc_shared_library
    fn cc_shared_library<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        exports_filter: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        dynamic_deps: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] roots: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] shared_lib_name: NoneOr<&str>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        user_link_flags: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let _ = (exports_filter, shared_lib_name, user_link_flags);
        let internals = ModuleInternals::from_context(eval, "cc_shared_library")?;
        let coercion_ctx = internals.attr_coercion_context();

        let mut attrs = Vec::new();

        // Coerce deps
        let deps_type = AttrType::list(AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY));
        let deps_value = if deps.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            deps
        };
        let coerced_deps = deps_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, deps_value)?;
        attrs.push(("deps".to_owned(), coerced_deps));

        // Coerce dynamic_deps
        let dynamic_deps_value = if dynamic_deps.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            dynamic_deps
        };
        let coerced_dynamic_deps =
            deps_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, dynamic_deps_value)?;
        attrs.push(("dynamic_deps".to_owned(), coerced_dynamic_deps));

        // Coerce roots
        let roots_value = if roots.is_none() {
            eval.heap().alloc(Vec::<Value>::new())
        } else {
            roots
        };
        let coerced_roots = deps_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, roots_value)?;
        attrs.push(("roots".to_owned(), coerced_roots));

        let target_node = create_native_target_node(
            rule_defs::CC_SHARED_LIBRARY_RULE.clone(),
            internals.package(),
            name,
            attrs,
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Declares a group of target environments and their defaults.
    ///
    /// Used by Bazel's environment-based constraint system (predates platforms).
    ///
    /// Example:
    /// ```python
    /// environment_group(
    ///     name = "jdk",
    ///     environments = [":jdk8", ":jdk11", ":jdk17"],
    ///     defaults = [":jdk17"],
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/general#environment_group
    fn environment_group<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        environments: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        defaults: UnpackListOrTuple<String>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "environment_group")?;
        let coercion_ctx = internals.attr_coercion_context();

        let mut attrs = Vec::new();

        // Coerce environments
        let list_type = AttrType::list(AttrType::string());
        let envs_value = eval.heap().alloc(environments.items);
        let coerced_envs = list_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, envs_value)?;
        attrs.push(("environments".to_owned(), coerced_envs));

        // Coerce defaults
        let defaults_value = eval.heap().alloc(defaults.items);
        let coerced_defaults =
            list_type.coerce(AttrIsConfigurable::Yes, coercion_ctx, defaults_value)?;
        attrs.push(("defaults".to_owned(), coerced_defaults));

        let target_node = create_native_target_node(
            rule_defs::ENVIRONMENT_GROUP_RULE.clone(),
            internals.package(),
            name,
            attrs,
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;

        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Stub xcode_config rule for non-Apple platforms.
    ///
    /// In Bazel, xcode_config selects an Xcode version for Apple builds.
    /// On non-Apple platforms, this provides a stub XcodeVersionConfig with dummy values.
    fn xcode_config<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "xcode_config")?;
        let target_node = create_native_target_node(
            rule_defs::XCODE_CONFIG_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
        Ok(NoneType)
    }
}

/// Initialize the ANALYSIS_TEST_REGISTER late binding.
///
/// Called from `kuro_interpreter_for_build::init_late_bindings()`.
/// This bridges the circular dependency: `kuro_build_api` defines `ANALYSIS_TEST_REGISTER`
/// but can't depend on `kuro_interpreter_for_build` (which has `ModuleInternals`).
/// We initialize the binding here instead.
pub fn init_analysis_test_register() {
    use kuro_build_api::interpreter::rule_defs::cc_common::ANALYSIS_TEST_REGISTER;
    ANALYSIS_TEST_REGISTER.init(|eval, name| {
        let internals = ModuleInternals::from_context(eval, "testing.analysis_test")?;
        let target_node = create_native_target_node(
            rule_defs::ANALYSIS_TEST_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &[],
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
        Ok(NoneType)
    });
}
