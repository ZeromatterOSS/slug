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
use dupe::Dupe;
use pagable::Pagable;
use strong_hash::StrongHash;

use crate::bzl_or_bxl_path::BzlOrBxlPath;

/// The identifier used to find the implementation function for this rule. Should point at the output of `rule()`
#[derive(
    Debug,
    Clone,
    derive_more::Display,
    Eq,
    PartialEq,
    Hash,
    StrongHash,
    Pagable,
    Allocative
)]
#[display("{}:{}", path, name)]
pub struct StarlarkRuleType {
    /// The cell, package, and file that contains the output of `rule()`
    pub path: BzlOrBxlPath,
    /// The name of the symbol that is bound to the output of `rule()`, e.g. `cxx_binary`
    pub name: String,
}

/// A Bazel-9-removed native rule that kuro accepts at load time but rejects
/// during analysis with a Bazel-shaped diagnostic. See
/// `thoughts/shared/plans/kuro-bazel-subplans/27-native-language-rule-removal.md`.
#[derive(
    Debug,
    Clone,
    Copy,
    Dupe,
    derive_more::Display,
    Eq,
    PartialEq,
    Hash,
    Pagable,
    Allocative
)]
pub enum RemovedNativeRule {
    /// Bazel's environment-based constraint system, deprecated in Bazel 6
    /// and removed entirely in Bazel 9. The replacement is the
    /// platform/constraint/toolchain system (`constraint_setting`,
    /// `constraint_value`, `platform`).
    #[display("environment_group")]
    EnvironmentGroup,
    /// Buck2-only rule with no Bazel form. Bazel registers execution
    /// platforms via `register_execution_platforms(...)` in
    /// `MODULE.bazel` (or `--extra_execution_platforms` on the CLI).
    #[display("execution_platform")]
    ExecutionPlatform,
    /// Buck2-only rule with no Bazel form. Same migration path as
    /// `execution_platform`: use `register_execution_platforms(...)`.
    #[display("execution_platforms")]
    ExecutionPlatforms,
    /// Bazel-removed shell rule. Replacement is `@rules_shell//shell:sh_binary.bzl`.
    /// Kuro tests use the nano_prelude Starlark `sh_binary` instead.
    #[display("sh_binary")]
    ShBinary,
    /// Bazel-removed shell rule. Replacement is `@rules_shell//shell:sh_test.bzl`.
    /// Kuro tests use the nano_prelude Starlark `sh_test` instead.
    #[display("sh_test")]
    ShTest,
    /// Bazel-removed shell rule. Replacement is `@rules_shell//shell:sh_library.bzl`.
    /// Kuro tests use the nano_prelude Starlark `sh_library` instead.
    #[display("sh_library")]
    ShLibrary,
}

impl RemovedNativeRule {
    pub fn rule_name(&self) -> &'static str {
        match self {
            RemovedNativeRule::EnvironmentGroup => "environment_group",
            RemovedNativeRule::ExecutionPlatform => "execution_platform",
            RemovedNativeRule::ExecutionPlatforms => "execution_platforms",
            RemovedNativeRule::ShBinary => "sh_binary",
            RemovedNativeRule::ShTest => "sh_test",
            RemovedNativeRule::ShLibrary => "sh_library",
        }
    }

    /// Bazel-shaped diagnostic for a removed rule. Callers append target
    /// context using kuro's normal analysis-error formatting.
    pub fn diagnostic_message(&self) -> String {
        match self {
            RemovedNativeRule::EnvironmentGroup => format!(
                "The {rule} rule has been removed in Bazel 9. The \
                 environment-based constraint system was deprecated in \
                 favor of platforms and toolchains. Migrate to \
                 constraint_setting() / constraint_value() and \
                 target_compatible_with.",
                rule = self.rule_name()
            ),
            RemovedNativeRule::ExecutionPlatform | RemovedNativeRule::ExecutionPlatforms => {
                format!(
                    "The {rule} rule is Buck2-specific and has been removed. \
                     Bazel does not have this rule. Define platforms with \
                     platform(...) and register them via \
                     register_execution_platforms(\"//path:platform\") in \
                     MODULE.bazel, or pass --extra_execution_platforms on \
                     the command line.",
                    rule = self.rule_name()
                )
            }
            RemovedNativeRule::ShBinary => format!(
                "The sh_binary rule has been removed in Bazel 9, add the \
                 following to your BUILD/bzl file:\n    \
                 load(\"@rules_shell//shell:sh_binary.bzl\", \"sh_binary\")"
            ),
            RemovedNativeRule::ShTest => format!(
                "The sh_test rule has been removed in Bazel 9, add the \
                 following to your BUILD/bzl file:\n    \
                 load(\"@rules_shell//shell:sh_test.bzl\", \"sh_test\")"
            ),
            RemovedNativeRule::ShLibrary => format!(
                "The sh_library rule has been removed in Bazel 9, add the \
                 following to your BUILD/bzl file:\n    \
                 load(\"@rules_shell//shell:sh_library.bzl\", \"sh_library\")"
            ),
        }
    }
}

/// The type of native rule (built into Kuro, not defined in Starlark).
#[derive(
    Debug,
    Clone,
    Dupe,
    derive_more::Display,
    Eq,
    PartialEq,
    Hash,
    Pagable,
    Allocative
)]
pub enum NativeRuleKind {
    #[display("filegroup")]
    Filegroup,
    #[display("constraint_setting")]
    ConstraintSetting,
    #[display("constraint_value")]
    ConstraintValue,
    #[display("alias")]
    Alias,
    #[display("label_flag")]
    LabelFlag,
    #[display("config_setting")]
    ConfigSetting,
    #[display("toolchain_type")]
    ToolchainType,
    #[display("package_group")]
    PackageGroup,
    #[display("genrule")]
    Genrule,
    #[display("platform")]
    Platform,
    #[display("cc_library")]
    CcLibrary,
    #[display("cc_binary")]
    CcBinary,
    #[display("cc_test")]
    CcTest,
    #[display("test_suite")]
    TestSuite,
    #[display("toolchain")]
    Toolchain,
    #[display("cc_libc_top_alias")]
    CcLibcTopAlias,
    #[display("analysis_test")]
    AnalysisTest,
    #[display("genquery")]
    Genquery,
    #[display("starlark_doc_extract")]
    StarlarkDocExtract,
    #[display("cc_toolchain")]
    CcToolchain,
    #[display("cc_toolchain_suite")]
    CcToolchainSuite,
    #[display("cc_import")]
    CcImport,
    #[display("cc_shared_library")]
    CcSharedLibrary,
    #[display("xcode_config")]
    XcodeConfig,
    /// A Bazel-9-removed rule. Loaded as a stub target and rejected during
    /// analysis with a Bazel-shaped diagnostic.
    #[display("{}", _0)]
    Removed(RemovedNativeRule),
}

impl NativeRuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NativeRuleKind::Filegroup => "filegroup",
            NativeRuleKind::ConstraintSetting => "constraint_setting",
            NativeRuleKind::ConstraintValue => "constraint_value",
            NativeRuleKind::Alias => "alias",
            NativeRuleKind::LabelFlag => "label_flag",
            NativeRuleKind::ConfigSetting => "config_setting",
            NativeRuleKind::ToolchainType => "toolchain_type",
            NativeRuleKind::PackageGroup => "package_group",
            NativeRuleKind::Genrule => "genrule",
            NativeRuleKind::Platform => "platform",
            NativeRuleKind::CcLibrary => "cc_library",
            NativeRuleKind::CcBinary => "cc_binary",
            NativeRuleKind::CcTest => "cc_test",
            NativeRuleKind::TestSuite => "test_suite",
            NativeRuleKind::Toolchain => "toolchain",
            NativeRuleKind::CcLibcTopAlias => "cc_libc_top_alias",
            NativeRuleKind::AnalysisTest => "analysis_test",
            NativeRuleKind::Genquery => "genquery",
            NativeRuleKind::StarlarkDocExtract => "starlark_doc_extract",
            NativeRuleKind::CcToolchain => "cc_toolchain",
            NativeRuleKind::CcToolchainSuite => "cc_toolchain_suite",
            NativeRuleKind::CcImport => "cc_import",
            NativeRuleKind::CcSharedLibrary => "cc_shared_library",
            NativeRuleKind::XcodeConfig => "xcode_config",
            NativeRuleKind::Removed(removed) => removed.rule_name(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Dupe,
    derive_more::Display,
    Eq,
    PartialEq,
    Hash,
    Pagable,
    Allocative
)]
pub enum RuleType {
    Starlark(Arc<StarlarkRuleType>),
    #[display("forward")]
    Forward,
    #[display("native:{_0}")]
    Native(NativeRuleKind),
}

impl RuleType {
    pub fn name(&self) -> &str {
        match self {
            RuleType::Starlark(rule_type) => rule_type.name.as_str(),
            RuleType::Forward => "forward",
            RuleType::Native(kind) => kind.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_core::bzl::ImportPath;

    use crate::bzl_or_bxl_path::BzlOrBxlPath;
    use crate::rule_type::NativeRuleKind;
    use crate::rule_type::RemovedNativeRule;
    use crate::rule_type::StarlarkRuleType;

    #[test]
    fn function_id_has_useful_string() {
        let import_path = ImportPath::testing_new("root//some/subdir:foo.bzl");
        let name = "foo_binary".to_owned();

        assert_eq!(
            "root//some/subdir/foo.bzl:foo_binary",
            &StarlarkRuleType {
                path: BzlOrBxlPath::Bzl(import_path),
                name
            }
            .to_string()
        );
    }

    /// Plan 27.6 guardrail: every `NativeRuleKind` variant must have a
    /// known parity category. The exhaustive match below forces anyone
    /// adding a new variant to declare whether it's a true Bazel 9 native
    /// rule, a removed-rule stub, a kuro-internal helper, or pending
    /// migration. Removing a `pending_removal_*` entry requires either
    /// converting the rule to a `Removed(...)` stub (Phase 27.2 pattern)
    /// or proving with a Bazel 9 source citation that the rule is a true
    /// native rule.
    fn parity_category(kind: NativeRuleKind) -> &'static str {
        match kind {
            // True Bazel 9 native rules — keep as native.
            NativeRuleKind::Filegroup
            | NativeRuleKind::ConstraintSetting
            | NativeRuleKind::ConstraintValue
            | NativeRuleKind::Alias
            | NativeRuleKind::LabelFlag
            | NativeRuleKind::ConfigSetting
            | NativeRuleKind::ToolchainType
            | NativeRuleKind::PackageGroup
            | NativeRuleKind::Genrule
            | NativeRuleKind::Platform
            | NativeRuleKind::Toolchain
            | NativeRuleKind::TestSuite
            | NativeRuleKind::Genquery
            | NativeRuleKind::CcLibcTopAlias => "true_native",

            // Bazel-9-removed; stub records the diagnostic at analysis time.
            NativeRuleKind::Removed(_) => "removed_stub",

            // Kuro-internal helpers; not exposed as Bazel parity surface.
            NativeRuleKind::AnalysisTest | NativeRuleKind::StarlarkDocExtract => "kuro_internal",

            // Apple-specific, tracked under a separate parity initiative.
            NativeRuleKind::XcodeConfig => "kuro_internal_apple",

            // Bazel-9-removed but not yet converted to `Removed(...)`. Each
            // entry here is a Plan 27.2 follow-up. Removing one without
            // converting the rule to a stub will fail this test.
            NativeRuleKind::CcLibrary
            | NativeRuleKind::CcBinary
            | NativeRuleKind::CcTest
            | NativeRuleKind::CcImport
            | NativeRuleKind::CcSharedLibrary
            | NativeRuleKind::CcToolchain
            | NativeRuleKind::CcToolchainSuite => "pending_removal_cc",
        }
    }

    #[test]
    fn native_rule_kinds_have_parity_category() {
        // Spot-check categorizations. The real guardrail is the exhaustive
        // match in `parity_category`: a new variant fails to compile until
        // a category is assigned.
        assert_eq!(parity_category(NativeRuleKind::Filegroup), "true_native");
        assert_eq!(
            parity_category(NativeRuleKind::Removed(RemovedNativeRule::EnvironmentGroup)),
            "removed_stub"
        );
        assert_eq!(
            parity_category(NativeRuleKind::Removed(
                RemovedNativeRule::ExecutionPlatform
            )),
            "removed_stub"
        );
        assert_eq!(
            parity_category(NativeRuleKind::CcLibrary),
            "pending_removal_cc"
        );
        assert_eq!(
            parity_category(NativeRuleKind::Removed(RemovedNativeRule::ShBinary)),
            "removed_stub"
        );
        assert_eq!(
            parity_category(NativeRuleKind::AnalysisTest),
            "kuro_internal"
        );
    }

    #[test]
    fn removed_native_rule_diagnostics_mention_rule_name() {
        // Every removed-rule diagnostic must mention the rule name so the
        // user can locate the call site.
        for kind in [
            RemovedNativeRule::EnvironmentGroup,
            RemovedNativeRule::ExecutionPlatform,
            RemovedNativeRule::ExecutionPlatforms,
            RemovedNativeRule::ShBinary,
            RemovedNativeRule::ShTest,
            RemovedNativeRule::ShLibrary,
        ] {
            let msg = kind.diagnostic_message();
            assert!(
                msg.contains(kind.rule_name()),
                "diagnostic for {} missing rule name: {}",
                kind.rule_name(),
                msg
            );
        }
    }
}
