/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible cc_common module and CcInfo provider.
//!
//! This provides an implementation of Bazel's cc_common built-in module
//! that rules_cc (0.2.16+) requires for C/C++ compilation support.
//!
//! For Bazel 9.0+, rules_cc is almost entirely pure Starlark. The key native
//! requirement is this cc_common module which provides:
//! - `internal_DO_NOT_USE()` - Returns internal API struct
//! - Public API functions for toolchain/action configuration
//!
//! Reference: thoughts/shared/research/2026-01-26-rules-cc-native-requirements.md

mod actions;
mod ctx_cheat;
mod feature_config;
mod host;
mod msvc_detect;
mod providers;

// Public re-exports: preserve the original
// `use slug_build_api::interpreter::rule_defs::cc_common::X` surface.
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::OnceLock;

pub use actions::CcCommonInternal;
pub use actions::CcCommonModule;
pub(crate) use actions::cc_frozen_list_items;
pub(crate) use actions::cc_toolchain_features_from_config_info;
use allocative::Allocative;
pub use ctx_cheat::CtxCheatActionsStub;
pub use ctx_cheat::CtxCheatArtifactRootStub;
pub use ctx_cheat::CtxCheatArtifactStub;
pub use ctx_cheat::CtxCheatConfigStub;
pub use ctx_cheat::CtxCheatDirStub;
pub use ctx_cheat::CtxCheatLabelDynamic;
pub use ctx_cheat::CtxCheatLabelStub;
pub use ctx_cheat::CtxCheatStub;
pub use ctx_cheat::CtxCheatWithActions;
pub use feature_config::CcExpandIfEqual;
pub use feature_config::CcFeatureFlagSets;
pub use feature_config::CcFlagGroup;
pub use feature_config::CcFlagSet;
pub use feature_config::CcToolchainFeatures;
pub use feature_config::CcWithFeatureSet;
pub use feature_config::FeatureConfiguration;
pub use msvc_detect::get_msvc_include_dirs;
pub use providers::CcCompilationContext;
pub use providers::CcCompilationContextGen;
pub use providers::CcDebugContext;
pub use providers::CcInfoInstanceGen;
pub use providers::CcInfoInstanceStub;
pub use providers::CcInfoProvider;
pub use providers::CcLinkingOutputs;
pub use providers::CcLinkingOutputsGen;
pub use providers::CcSharedLibraryHintInfoInstanceGen;
pub use providers::CcSharedLibraryHintInfoProvider;
pub use providers::CcSharedLibraryInfoInstanceGen;
pub use providers::CcSharedLibraryInfoProvider;
pub use providers::CcToolchainConfigInfoInstanceGen;
pub use providers::CcToolchainConfigInfoProvider;
pub use providers::CcToolchainInfoProvider;
pub use providers::CcToolchainVariables;
pub use providers::CcToolchainVariablesGen;
pub use providers::CompilationOutputs;
pub use providers::CompilationOutputsGen;
pub use providers::DebugPackageInfoInstanceGen;
pub use providers::DebugPackageInfoProvider;
pub use providers::ExecutionInfoInstance;
pub use providers::ExecutionInfoInstanceGen;
pub use providers::ExecutionInfoProvider;
pub use providers::HeaderInfoStub;
pub use providers::LibraryToLink;
pub use providers::LibraryToLinkGen;
pub use providers::LinkerInputStub;
pub use providers::LinkerInputStubGen;
pub use providers::LinkingContextWithInputs;
pub use providers::LinkingContextWithInputsGen;
pub use providers::OutputGroupInfoInstanceGen;
pub use providers::OutputGroupInfoProvider;
pub use providers::PackageSpecificationInfoInstance;
pub use providers::PackageSpecificationInfoInstanceGen;
pub use providers::PackageSpecificationInfoProvider;
pub use providers::RunEnvironmentInfoInstance;
pub use providers::RunEnvironmentInfoInstanceGen;
pub use providers::RunEnvironmentInfoProvider;
use slug_core::provider::id::ProviderId;
use slug_util::late_binding::LateBinding;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

// (Plan 29) — the previous `EXTERNAL_INCLUDE_DIRS` process-global registry
// has been retired. Include directories now flow exclusively through
// `CcCompilationContext.{includes, system_includes, quote_includes,
// external_includes}` providers, matching what Bazel and Bonanza do. See
// `thoughts/shared/plans/slug-bazel-subplans/29-cc-include-dir-determinism.md`
// for the rationale and the audit of every former call site. If you find
// yourself wanting a "shared registry" again, you almost certainly want
// to add the dir to this target's own `CcCompilationContext.includes`
// depset — that's how Bazel propagates it to dependents.
// ============================================================================
// Registration
// ============================================================================

/// Register the cc_common global and related providers.
///
/// Note: Per Bazel's CcRules.java, some providers are set to None because
/// they are defined in Starlark by rules_cc.
#[starlark_module]
pub fn register_cc_common(globals: &mut GlobalsBuilder) {
    /// The cc_common module provides C/C++ compilation support.
    const cc_common: CcCommonModule = CcCommonModule;

    /// CcInfo provider for C++ compilation/linking information.
    const CcInfo: CcInfoProvider = CcInfoProvider;

    /// CcToolchainInfo provider for C++ toolchain information.
    const CcToolchainInfo: CcToolchainInfoProvider = CcToolchainInfoProvider;

    /// CcToolchainConfigInfo provider for toolchain configuration.
    /// Used by cc_common.create_cc_toolchain_config_info().
    const CcToolchainConfigInfo: CcToolchainConfigInfoProvider = CcToolchainConfigInfoProvider;

    /// DebugPackageInfo provider for debug/symbol information.
    const DebugPackageInfo: DebugPackageInfoProvider = DebugPackageInfoProvider;

    /// CcSharedLibraryInfo provider for shared library information.
    const CcSharedLibraryInfo: CcSharedLibraryInfoProvider = CcSharedLibraryInfoProvider;

    /// OutputGroupInfo - provider for grouping outputs.
    /// This is callable to create instances.
    const OutputGroupInfo: OutputGroupInfoProvider = OutputGroupInfoProvider;

    /// PackageSpecificationInfo - provider for package visibility/allowlisting.
    /// Used by cc_toolchain.bzl for visibility_public_presubmit attribute.
    /// See: https://bazel.build/rules/lib/providers/PackageSpecificationInfo
    const PackageSpecificationInfo: PackageSpecificationInfoProvider =
        PackageSpecificationInfoProvider;

    /// RunEnvironmentInfo - Provider for specifying environment variables
    /// that should be set when running binaries or tests.
    ///
    /// Usage in rules:
    /// ```python
    /// return [RunEnvironmentInfo(environment = {"FOO": "bar"}, inherited_environment = ["PATH"])]
    /// ```
    const RunEnvironmentInfo: RunEnvironmentInfoProvider = RunEnvironmentInfoProvider;

    /// testing module constant for Bazel-compatible testing utilities.
    /// Currently a stub that provides TestEnvironment.
    const testing: TestingModule = TestingModule;
}
// ============================================================================
// TestingModule - Bazel's testing module
// ============================================================================

/// Stub for the testing module.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TestingModule;

impl Display for TestingModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<module: testing>")
    }
}

starlark_simple_value!(TestingModule);

#[starlark_value(type = "testing")]
impl<'v> StarlarkValue<'v> for TestingModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(testing_methods)
    }
}

#[starlark_module]
fn testing_methods(builder: &mut MethodsBuilder) {
    /// TestEnvironment provider for specifying test environment variables.
    /// This is an alias for RunEnvironmentInfo (deprecated in Bazel).
    #[starlark(attribute)]
    fn TestEnvironment<'v>(this: &TestingModule) -> starlark::Result<RunEnvironmentInfoProvider> {
        let _ = this;
        Ok(RunEnvironmentInfoProvider)
    }

    /// ExecutionInfo provider for specifying execution requirements.
    ///
    /// `testing.ExecutionInfo` is a provider callable. Usage:
    /// ```python
    /// return [testing.ExecutionInfo(requirements = {"no-remote": "1"})]
    /// ```
    /// See: https://bazel.build/rules/lib/providers/ExecutionInfo
    #[starlark(attribute)]
    fn ExecutionInfo(this: &TestingModule) -> starlark::Result<ExecutionInfoProvider> {
        let _ = this;
        Ok(ExecutionInfoProvider)
    }

    /// Creates an analysis test rule or registers an analysis test target.
    ///
    /// In Bazel, `testing.analysis_test(implementation, attrs, ...)` creates a
    /// rule for analysis-time tests. When called with `name` and `attr_values`,
    /// it also registers the target.
    ///
    /// Typical usage (bazel_skylib analysistest.make pattern):
    /// ```python
    /// # In .bzl file - returns a callable rule:
    /// my_test = testing.analysis_test(implementation = _impl, attrs = {...})
    ///
    /// # In BUILD file - registers a test target:
    /// my_test(name = "my_test", target_under_test = ":some_target")
    /// ```
    ///
    /// See: https://bazel.build/rules/lib/builtins/testing#analysis_test
    fn analysis_test<'v>(
        this: &TestingModule,
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = NoneType)] name: Value<'v>,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        if let Some(name_str) = name.unpack_str() {
            // Called with name= directly - register the target
            if let Ok(register) = ANALYSIS_TEST_REGISTER.get() {
                register(eval, name_str)?;
            }
            return Ok(Value::new_none());
        }
        // No name provided - return a callable that registers a target when called
        Ok(eval.heap().alloc(AnalysisTestCallable { implementation }))
    }
}

/// Signature: (eval, target_name) -> starlark::Result<NoneType>
pub static ANALYSIS_TEST_REGISTER: LateBinding<
    for<'v, 'a, 'e> fn(&mut Evaluator<'v, 'a, 'e>, &str) -> starlark::Result<NoneType>,
> = LateBinding::new("ANALYSIS_TEST_REGISTER");

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum AnalysisTestError {
    #[error("analysis_test_rule can only be invoked after the module is frozen")]
    InvokedBeforeFreezing,
    #[error("analysis_test requires a 'name' argument")]
    MissingName,
    #[error("analysis_test 'name' argument must be a string")]
    NameNotString,
}

// ============================================================================
// AnalysisTestCallable - Returned by testing.analysis_test() when no name given
// ============================================================================

/// A callable that, when invoked with name=..., registers a native analysis test target.
/// Returned by testing.analysis_test() when no `name` argument is provided.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct AnalysisTestCallable<'v> {
    /// The Starlark implementation function (stored for potential future use).
    implementation: Value<'v>,
}

impl<'v> Display for AnalysisTestCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<analysis_test_rule>")
    }
}

impl<'v> starlark::values::AllocValue<'v> for AnalysisTestCallable<'v> {
    fn alloc_value(self, heap: starlark::values::Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> Freeze for AnalysisTestCallable<'v> {
    type Frozen = FrozenAnalysisTestCallable;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenAnalysisTestCallable> {
        Ok(FrozenAnalysisTestCallable {
            _implementation: self.implementation.freeze(freezer)?,
        })
    }
}

#[starlark_value(type = "analysis_test_rule")]
impl<'v> StarlarkValue<'v> for AnalysisTestCallable<'v> {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(slug_error::Error::from(AnalysisTestError::InvokedBeforeFreezing).into())
    }
}

/// Frozen version of AnalysisTestCallable.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenAnalysisTestCallable {
    /// The frozen Starlark implementation function.
    _implementation: FrozenValue,
}

impl Display for FrozenAnalysisTestCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<analysis_test_rule>")
    }
}

starlark_simple_value!(FrozenAnalysisTestCallable);

#[starlark_value(type = "analysis_test_rule")]
impl<'v> StarlarkValue<'v> for FrozenAnalysisTestCallable {
    type Canonical = AnalysisTestCallable<'v>;

    /// Called when this analysis_test_rule callable is invoked in a BUILD file.
    /// Parses the `name` argument and delegates to ANALYSIS_TEST_REGISTER.
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Find `name` in the named arguments
        let named = args.names_map()?;
        let name_str = named
            .iter()
            .find_map(|(k, v)| if k.as_str() == "name" { Some(*v) } else { None })
            .ok_or_else(|| slug_error::Error::from(AnalysisTestError::MissingName))?
            .unpack_str()
            .ok_or_else(|| slug_error::Error::from(AnalysisTestError::NameNotString))?;
        if let Ok(register) = ANALYSIS_TEST_REGISTER.get() {
            register(eval, name_str)?;
        }
        Ok(Value::new_none())
    }
}

// ============================================================================
// Top-level Bazel globals registration
// ============================================================================

/// Register Bazel provider globals that should be available at the top level.
///
/// In Bazel 6+, these providers are available as top-level globals in .bzl files.
/// The `bazel_features` package generates `globals.bzl` referencing them by name.
#[starlark_module]
pub fn register_bazel_provider_globals(globals: &mut GlobalsBuilder) {
    const CcSharedLibraryInfo: CcSharedLibraryInfoProvider = CcSharedLibraryInfoProvider;
    const CcSharedLibraryHintInfo: CcSharedLibraryHintInfoProvider =
        CcSharedLibraryHintInfoProvider;
    const PackageSpecificationInfo: PackageSpecificationInfoProvider =
        PackageSpecificationInfoProvider;
    const RunEnvironmentInfo: RunEnvironmentInfoProvider = RunEnvironmentInfoProvider;
    const py_internal: PyInternalStub = PyInternalStub;
}
// ============================================================================
// PyInternal stub - Bazel's internal Python helper
// ============================================================================

/// Stub for Bazel's `py_internal` native global.
///
/// `py_internal` is an internal API that rules_python uses for accessing
/// Bazel-private Python rule implementation details. We provide a stub
/// so that .bzl files can load without errors.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone, Copy)]
pub struct PyInternalStub;

impl std::fmt::Display for PyInternalStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "py_internal")
    }
}

starlark_simple_value!(PyInternalStub);

#[starlark_value(type = "py_internal")]
impl<'v> StarlarkValue<'v> for PyInternalStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "cc_toolchain_build_info_files"
                | "copy_without_caching"
                | "create_repo_mapping_manifest"
                | "declare_constant_metadata_file"
                | "declare_shareable_artifact"
                | "expand_location_and_make_variables"
                | "get_label_repo_runfiles_path"
                | "get_legacy_external_runfiles"
                | "is_bzlmod_enabled"
                | "is_singleton_depset"
                | "is_tool_configuration"
                | "link"
                | "linkstamp_file"
                | "make_runfiles_respect_legacy_external_runfiles"
                | "merge_runfiles_with_generated_inits_empty_files_supplier"
                | "regex_match"
                | "runfiles_enabled"
                | "share_native_deps"
                | "stamp_binaries"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if !self.has_attr(attribute, heap) {
            return None;
        }
        // rules_python 1.9+ calls several of these attributes directly,
        // e.g. `py_internal.get_label_repo_runfiles_path(ctx.label)`, so the
        // stub attribute needs to be a callable (not None). Return a lambda
        // via the starlark evaluator that always produces an empty string.
        // The booleans (is_bzlmod_enabled, stamp_binaries, …) are sometimes
        // used as values rather than called, but starlark allows unused
        // callables there too, so returning a no-arg lambda is uniformly safe.
        match attribute {
            "get_label_repo_runfiles_path"
            | "get_legacy_external_runfiles"
            | "copy_without_caching"
            | "create_repo_mapping_manifest"
            | "cc_toolchain_build_info_files"
            | "declare_constant_metadata_file"
            | "declare_shareable_artifact"
            | "expand_location_and_make_variables"
            | "is_bzlmod_enabled"
            | "is_singleton_depset"
            | "is_tool_configuration"
            | "link"
            | "linkstamp_file"
            | "make_runfiles_respect_legacy_external_runfiles"
            | "merge_runfiles_with_generated_inits_empty_files_supplier"
            | "regex_match"
            | "runfiles_enabled"
            | "share_native_deps"
            | "stamp_binaries" => Some(heap.alloc(PyInternalStubCall {
                name: attribute.to_owned(),
            })),
            _ => Some(Value::new_none()),
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "cc_toolchain_build_info_files".to_owned(),
            "copy_without_caching".to_owned(),
            "create_repo_mapping_manifest".to_owned(),
            "declare_constant_metadata_file".to_owned(),
            "declare_shareable_artifact".to_owned(),
            "expand_location_and_make_variables".to_owned(),
            "get_label_repo_runfiles_path".to_owned(),
            "get_legacy_external_runfiles".to_owned(),
            "is_bzlmod_enabled".to_owned(),
            "is_singleton_depset".to_owned(),
            "is_tool_configuration".to_owned(),
            "link".to_owned(),
            "linkstamp_file".to_owned(),
            "make_runfiles_respect_legacy_external_runfiles".to_owned(),
            "merge_runfiles_with_generated_inits_empty_files_supplier".to_owned(),
            "regex_match".to_owned(),
            "runfiles_enabled".to_owned(),
            "share_native_deps".to_owned(),
            "stamp_binaries".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> starlark::typing::Ty {
        starlark::typing::Ty::any()
    }
}

/// Callable stub returned by `py_internal.<name>` access for methods that
/// rules_python calls. Accepts any args and returns an empty string (a
/// reasonable harmless default; all current callers ignore return value
/// when it's non-meaningful in slug).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct PyInternalStubCall {
    pub name: String,
}

impl std::fmt::Display for PyInternalStubCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "py_internal.{}(stub)", self.name)
    }
}

starlark_simple_value!(PyInternalStubCall);

#[starlark_value(type = "py_internal_stub_fn")]
impl<'v> StarlarkValue<'v> for PyInternalStubCall {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Most py_internal methods have unused return values in slug; returning
        // an empty string is a safe default. A few methods feed their return
        // value back into downstream APIs that type-check it — for those,
        // pass through the runfiles arg that rules_python expects to round-trip.
        let kwargs = args.names_map()?;
        let kwarg = |name: &str| {
            kwargs
                .iter()
                .find_map(|(k, v)| if k.as_str() == name { Some(*v) } else { None })
        };
        let positional: Vec<Value<'v>> = args.positions(eval.heap())?.collect();
        match self.name.as_str() {
            // Both methods pass a runfiles object through. Called either as
            //   make_runfiles_respect_legacy_external_runfiles(ctx, runfiles)   # positional
            //   merge_runfiles_with_generated_inits_empty_files_supplier(ctx=, runfiles=)  # kwarg
            // With no real merge, passing the input runfiles back is a
            // correct no-op (downstream appends it to a RunfilesBuilder or
            // feeds it to DefaultInfo.default_runfiles).
            "merge_runfiles_with_generated_inits_empty_files_supplier"
            | "make_runfiles_respect_legacy_external_runfiles" => {
                if let Some(rf) = kwarg("runfiles").or_else(|| positional.get(1).copied()) {
                    return Ok(rf);
                }
            }
            // declare_constant_metadata_file(ctx=, name=, root=) is called by
            // rules_python's _write_build_data to create the build-data output
            // File. The downstream code reads `.path` on the return value, so
            // we must return a real File, not a string. Delegate to
            // ctx.actions.declare_file(name).
            "declare_constant_metadata_file" => {
                if let (Some(ctx), Some(name)) = (kwarg("ctx"), kwarg("name")) {
                    if let (Ok(Some(actions)), Some(_name_str)) =
                        (ctx.get_attr("actions", eval.heap()), name.unpack_str())
                    {
                        if let Ok(Some(declare_file)) =
                            actions.get_attr("declare_file", eval.heap())
                        {
                            return eval.eval_function(declare_file, &[name], &[]);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(eval.heap().alloc_str("").to_value())
    }

    fn get_type_starlark_repr() -> starlark::typing::Ty {
        starlark::typing::Ty::any()
    }
}
