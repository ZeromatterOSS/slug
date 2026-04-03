/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use kuro_core::configuration::transition::id::TransitionId;
use kuro_core::plugins::PluginKind;
#[allow(unused_imports)]
use kuro_util::hash::BuckHasher;
use pagable::Pagable;
use static_interner::interner;

use crate::attrs::spec::AttributeSpec;
use crate::nodes::unconfigured::RuleKind;
use crate::rule_type::RuleType;

#[derive(Debug, Eq, PartialEq, Hash, Pagable, Allocative, Clone, dupe::Dupe)]
pub enum RuleIncomingTransition {
    None,
    Fixed(Arc<TransitionId>),
    /// This rule has an `incoming_transition` attribute
    FromAttribute,
}

/// Stored definition of an exec group from `rule(exec_groups={...})`.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Pagable, Allocative)]
pub struct ExecGroupDef {
    /// Toolchain type labels this group requires.
    pub toolchain_types: Vec<String>,
    /// Exec-compatible-with constraint labels.
    pub exec_compatible_with: Vec<String>,
}

/// Common rule data needed in `TargetNode`.
#[derive(Debug, Eq, PartialEq, Hash, Pagable, Allocative)]
pub struct Rule {
    /// The attribute spec. This holds the attribute name -> index mapping and the default values
    /// (for those attributes without explicit values).
    pub attributes: AttributeSpec,
    /// The 'type', used to find the implementation function from the graph
    pub rule_type: RuleType,
    /// The kind of rule, e.g. configuration or otherwise.
    pub rule_kind: RuleKind,
    /// Transition to apply to the target.
    pub cfg: RuleIncomingTransition,
    /// The plugin kinds that are used by the target
    pub uses_plugins: Vec<PluginKind>,
    /// Whether this rule is a Bazel test rule (created with `rule(test=True)`).
    /// Test rules auto-generate `ExternalRunnerTestInfo` from `DefaultInfo.executable`
    /// during analysis if no explicit `ExternalRunnerTestInfo` is provided.
    pub is_test: bool,
    /// Whether this rule produces an executable (created with `rule(executable=True)`).
    /// Executable rules have `ctx.outputs.executable` auto-declared and can be used
    /// with `kuro run`.
    pub is_executable: bool,
    /// Provider type names declared via `rule(provides=[CcInfo, ...])`.
    /// After analysis, the returned provider collection is validated to contain
    /// all declared providers. Empty means no validation.
    pub provides: Vec<String>,
    /// Toolchain type labels declared via `rule(toolchains=[...])`.
    /// Used during analysis to populate `ctx.toolchains`.
    pub toolchain_types: Vec<String>,
    /// Execution group definitions declared via `rule(exec_groups={...})`.
    /// Each entry is `(group_name, definition)` with toolchain types and exec constraints.
    pub exec_group_defs: Vec<(String, ExecGroupDef)>,
    /// Configuration fragment names declared via `rule(fragments=["cpp", "java", ...])`.
    /// In Bazel, this declares which configuration fragments a rule requires.
    /// Currently stored as metadata; fragment access is handled via `ctx.fragments`.
    pub fragments: Vec<String>,
    /// Build setting type from `rule(build_setting=config.bool(flag=True))`.
    /// When set, this rule defines a user-configurable build flag.
    /// The string is the setting type: "bool", "string", "int", "string_list", "string_set".
    /// None means this is a regular rule, not a build setting.
    pub build_setting_type: Option<String>,
    /// Whether this build setting is a command-line flag (settable via --//pkg:target=value).
    pub build_setting_is_flag: bool,
}

impl Rule {
    /// Returns true if this rule is a build setting (user-configurable build flag).
    pub fn is_build_setting(&self) -> bool {
        self.build_setting_type.is_some()
    }

    /// Convenience: returns just the exec group names (derived from exec_group_defs).
    pub fn exec_group_names(&self) -> Vec<String> {
        self.exec_group_defs
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }
}

interner!(INTERNER, BuckHasher, Rule);
