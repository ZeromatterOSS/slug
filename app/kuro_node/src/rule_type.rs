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
    #[display("sh_binary")]
    ShBinary,
    #[display("sh_test")]
    ShTest,
    #[display("sh_library")]
    ShLibrary,
    #[display("cc_libc_top_alias")]
    CcLibcTopAlias,
    #[display("analysis_test")]
    AnalysisTest,
    #[display("genquery")]
    Genquery,
    #[display("execution_platform")]
    ExecutionPlatform,
    #[display("execution_platforms")]
    ExecutionPlatforms,
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
    #[display("environment_group")]
    EnvironmentGroup,
    #[display("proto_library")]
    ProtoLibrary,
    #[display("java_library")]
    JavaLibrary,
    #[display("java_binary")]
    JavaBinary,
    #[display("java_test")]
    JavaTest,
    #[display("java_import")]
    JavaImport,
    #[display("py_library")]
    PyLibrary,
    #[display("py_binary")]
    PyBinary,
    #[display("py_test")]
    PyTest,
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
            NativeRuleKind::ShBinary => "sh_binary",
            NativeRuleKind::ShTest => "sh_test",
            NativeRuleKind::ShLibrary => "sh_library",
            NativeRuleKind::CcLibcTopAlias => "cc_libc_top_alias",
            NativeRuleKind::AnalysisTest => "analysis_test",
            NativeRuleKind::Genquery => "genquery",
            NativeRuleKind::ExecutionPlatform => "execution_platform",
            NativeRuleKind::ExecutionPlatforms => "execution_platforms",
            NativeRuleKind::StarlarkDocExtract => "starlark_doc_extract",
            NativeRuleKind::CcToolchain => "cc_toolchain",
            NativeRuleKind::CcToolchainSuite => "cc_toolchain_suite",
            NativeRuleKind::CcImport => "cc_import",
            NativeRuleKind::CcSharedLibrary => "cc_shared_library",
            NativeRuleKind::EnvironmentGroup => "environment_group",
            NativeRuleKind::ProtoLibrary => "proto_library",
            NativeRuleKind::JavaLibrary => "java_library",
            NativeRuleKind::JavaBinary => "java_binary",
            NativeRuleKind::JavaTest => "java_test",
            NativeRuleKind::JavaImport => "java_import",
            NativeRuleKind::PyLibrary => "py_library",
            NativeRuleKind::PyBinary => "py_binary",
            NativeRuleKind::PyTest => "py_test",
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
}
