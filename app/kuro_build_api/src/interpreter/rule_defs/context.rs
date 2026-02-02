/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::cell::RefCell;
use std::cell::RefMut;
use std::convert::Infallible;
use std::fmt;
use std::fmt::Formatter;

use allocative::Allocative;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_execute::digest_config::DigestConfig;
use kuro_interpreter::late_binding_ty::AnalysisContextReprLate;
use kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
use kuro_util::late_binding::LateBinding;
use derive_more::Display;
use dice::DiceComputations;
use futures::FutureExt;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::dict::Dict;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueTyped;
use starlark::values::ValueTypedComplex;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::StructRef;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::analysis::anon_promises_dyn::RunAnonPromisesAccessor;
use crate::analysis::registry::AnalysisRegistry;
use crate::deferred::calculation::GET_PROMISED_ARTIFACT;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::fragments::CppFragment;
use crate::interpreter::rule_defs::plugins::AnalysisPlugins;

/// Functions to allow users to interact with the Actions registry.
///
/// Accessed via `ctx.actions.<function>`
#[derive(ProvidesStaticType, Debug, Display, Trace, NoSerialize, Allocative)]
#[display("<ctx.actions>")]
pub struct AnalysisActions<'v> {
    /// Use a RefCell/Option so when we are done with it, without obtaining exclusive access,
    /// we can take the internal state without having to clone it.
    pub state: RefCell<Option<AnalysisRegistry<'v>>>,
    /// Copies from the ctx, so we can capture them for `dynamic`.
    pub attributes: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
    pub plugins: Option<ValueTypedComplex<'v, AnalysisPlugins<'v>>>,
    /// Digest configuration to use when interpreting digests passed in analysis.
    pub digest_config: DigestConfig,
}

impl<'v> AnalysisActions<'v> {
    pub fn state(&self) -> kuro_error::Result<RefMut<'_, AnalysisRegistry<'v>>> {
        let state = self
            .state
            .try_borrow_mut()
            .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Tier0))
            .internal_error("AnalysisActions.state is already borrowed")?;
        RefMut::filter_map(state, |x| x.as_mut())
            .ok()
            .internal_error("state to be present during execution")
    }

    pub async fn run_promises<'a, 'e: 'a>(
        &self,
        accessor: &mut dyn RunAnonPromisesAccessor<'v, 'a, 'e>,
    ) -> kuro_error::Result<()> {
        // We need to loop here because running the promises evaluates promise.map, which might produce more promises.
        // We keep going until there are no promises left.
        loop {
            let promises = self.state()?.take_promises();
            if let Some(promises) = promises {
                promises.run_promises(accessor).await?;
            } else {
                break;
            }
        }

        accessor
            .with_dice(|dice| self.assert_short_paths_and_resolve(dice).boxed_local())
            .await?;

        Ok(())
    }

    // Called after `run_promises()` to assert short paths and resolve consumer's promise artifacts.
    pub async fn assert_short_paths_and_resolve(
        &self,
        dice: &mut DiceComputations<'_>,
    ) -> kuro_error::Result<()> {
        let (short_path_assertions, content_based_path_assertions, consumer_analysis_artifacts) = {
            let state = self.state()?;
            (
                state.short_path_assertions.clone(),
                state.content_based_path_assertions.clone(),
                state.consumer_analysis_artifacts(),
            )
        };

        for consumer_artifact in consumer_analysis_artifacts {
            let artifact = (GET_PROMISED_ARTIFACT.get()?)(&consumer_artifact, dice).await?;
            let id = consumer_artifact.id();
            let short_path = short_path_assertions.get(id).cloned();
            consumer_artifact.resolve(
                artifact.clone(),
                &short_path,
                content_based_path_assertions.contains(id),
            )?;
        }
        Ok(())
    }
}

#[starlark_value(type = "AnalysisActions", StarlarkTypeRepr, UnpackValue)]
impl<'v> StarlarkValue<'v> for AnalysisActions<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(|builder| {
            (ANALYSIS_ACTIONS_METHODS_ACTIONS.get().unwrap())(builder);
            (ANALYSIS_ACTIONS_METHODS_ANON_TARGET.get().unwrap())(builder);
        })
    }
}

impl<'v> AllocValue<'v> for AnalysisActions<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

#[allow(dead_code)] // field `0` is never read
struct RefAnalysisAction<'v>(&'v AnalysisActions<'v>);

impl<'v> StarlarkTypeRepr for RefAnalysisAction<'v> {
    type Canonical = <AnalysisActions<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AnalysisActions::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefAnalysisAction<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(analysis_actions) = value.downcast_ref::<AnalysisActions>() else {
            return Ok(None);
        };
        Ok(Some(RefAnalysisAction(analysis_actions)))
    }
}

#[derive(ProvidesStaticType, Debug, Trace, NoSerialize, Allocative)]
pub struct AnalysisContext<'v> {
    attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
    pub actions: ValueTyped<'v, AnalysisActions<'v>>,
    /// Only `None` when running a `dynamic_output` action from Bxl.
    label: Option<ValueTyped<'v, StarlarkConfiguredProvidersLabel>>,
    plugins: Option<ValueTypedComplex<'v, AnalysisPlugins<'v>>>,
}

impl<'v> Display for AnalysisContext<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx")?;
        if let Some(label) = &self.label {
            write!(f, " label=\"{label}\"")?;
        }
        write!(f, " attrs=...")?;
        write!(f, " actions=...")?;
        write!(f, ">")?;
        Ok(())
    }
}

impl<'v> AnalysisContext<'v> {
    /// The context that is provided to users' UDR implementation functions. Comprised of things like attribute values, actions, etc
    fn new(
        heap: Heap<'v>,
        attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
        label: Option<ValueTyped<'v, StarlarkConfiguredProvidersLabel>>,
        plugins: Option<ValueTypedComplex<'v, AnalysisPlugins<'v>>>,
        registry: AnalysisRegistry<'v>,
        digest_config: DigestConfig,
    ) -> Self {
        Self {
            attrs,
            actions: heap.alloc_typed(AnalysisActions {
                state: RefCell::new(Some(registry)),
                attributes: attrs,
                plugins,
                digest_config,
            }),
            label,
            plugins,
        }
    }

    pub fn prepare(
        heap: Heap<'v>,
        attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
        label: Option<ConfiguredTargetLabel>,
        plugins: Option<ValueTypedComplex<'v, AnalysisPlugins<'v>>>,
        registry: AnalysisRegistry<'v>,
        digest_config: DigestConfig,
    ) -> ValueTyped<'v, AnalysisContext<'v>> {
        let label = label.map(|label| {
            heap.alloc_typed(StarlarkConfiguredProvidersLabel::new(
                ConfiguredProvidersLabel::new(label, ProvidersName::Default),
            ))
        });

        let analysis_context = Self::new(heap, attrs, label, plugins, registry, digest_config);
        heap.alloc_typed(analysis_context)
    }

    pub fn assert_no_promises(&self) -> kuro_error::Result<()> {
        self.actions.state()?.assert_no_promises()
    }

    /// Must take an `AnalysisContext` which has never had `take_state` called on it before.
    pub fn take_state(&self) -> AnalysisRegistry<'v> {
        self.actions
            .state
            .borrow_mut()
            .take()
            .expect("nothing to have stolen state yet")
    }
}

#[starlark_value(type = "AnalysisContext")]
impl<'v> StarlarkValue<'v> for AnalysisContext<'v> {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(analysis_context_methods)
    }
}

impl<'v> AllocValue<'v> for AnalysisContext<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

struct RefAnalysisContext<'v>(&'v AnalysisContext<'v>);

impl<'v> StarlarkTypeRepr for RefAnalysisContext<'v> {
    type Canonical = <AnalysisContext<'v> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AnalysisContext::starlark_type_repr()
    }
}

impl<'v> UnpackValue<'v> for RefAnalysisContext<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        let Some(analysis_context) = value.downcast_ref::<AnalysisContext>() else {
            return Ok(None);
        };
        Ok(Some(RefAnalysisContext(analysis_context)))
    }
}

/// The type used for defining rules, usually bound as `ctx`.
/// Usually the sole argument to the `impl` argument of the `rule` function.
///
/// ```python
/// def _impl_my_rule(ctx: AnalysisContext) -> ["provider"]:
///     return [DefaultInfo()]
/// my_rule = rule(impl = _impl_my_rule, attrs = {})
/// ```
#[starlark_module]
fn analysis_context_methods(builder: &mut MethodsBuilder) {
    /// Returns the attributes of the target as a Starlark struct with a field for each attribute, which varies per rule.
    /// As an example, given a rule with the `attrs` argument of `{"foo": attrs.string()}`, this field will be
    /// a `struct` containing a field `foo` of type string.
    #[starlark(attribute)]
    fn attrs<'v>(
        this: RefAnalysisContext<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, StructRef<'static>>> {
        Ok(this
            .0
            .attrs
            .buck_error_context("`attrs` is not available for `dynamic_output` or BXL")?)
    }

    /// Returns an `actions` value containing functions to define actual actions that are run.
    /// See the `actions` type for the operations that are available.
    #[starlark(attribute)]
    fn actions<'v>(
        this: RefAnalysisContext<'v>,
    ) -> starlark::Result<ValueTyped<'v, AnalysisActions<'v>>> {
        Ok(this.0.actions)
    }

    /// Returns a `label` representing the target, or `None` if being invoked from a
    /// `dynamic_output` in Bxl.
    #[starlark(attribute)]
    fn label<'v>(
        this: RefAnalysisContext<'v>,
    ) -> starlark::Result<NoneOr<ValueTyped<'v, StarlarkConfiguredProvidersLabel>>> {
        Ok(NoneOr::from_option(this.0.label))
    }

    /// An opaque value that can be indexed with a plugin kind to get a list of the available plugin
    /// deps of that kind. The rule must set an appropriate value on `uses_plugins` in its
    /// declaration.
    #[starlark(attribute)]
    fn plugins<'v>(
        this: RefAnalysisContext<'v>,
    ) -> starlark::Result<ValueTypedComplex<'v, AnalysisPlugins<'v>>> {
        Ok(this
            .0
            .plugins
            .buck_error_context("`plugins` is not available for `dynamic_output` or BXL")?)
    }

    /// Bazel-compatible alias for `attrs`.
    ///
    /// In Bazel, attributes are accessed via `ctx.attr.foo`. In Kuro (Buck2),
    /// they are accessed via `ctx.attrs.foo`. This alias provides Bazel compatibility.
    #[starlark(attribute)]
    fn attr<'v>(
        this: RefAnalysisContext<'v>,
    ) -> starlark::Result<ValueOfUnchecked<'v, StructRef<'static>>> {
        Ok(this
            .0
            .attrs
            .buck_error_context("`attr` is not available for `dynamic_output` or BXL")?)
    }

    /// Configuration fragments for this target.
    ///
    /// Provides access to language-specific configuration like `ctx.fragments.cpp`,
    /// `ctx.fragments.java`, etc.
    #[starlark(attribute)]
    fn fragments<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return default configuration fragments for now
        // TODO(fragments): Pull actual configuration from target configuration
        Ok(heap.alloc(ConfigurationFragments::default()))
    }

    /// Resolved toolchains for this target (Bazel-compatible).
    ///
    /// In Bazel, rules can declare toolchain dependencies via the `toolchains` attribute
    /// on rule(). During analysis, resolved toolchains are provided via `ctx.toolchains`,
    /// which is a dict-like object mapping toolchain types to toolchain info providers.
    ///
    /// Returns a stub that pretends to contain all toolchain types and returns
    /// stub toolchain info when indexed. This allows rules_cc to proceed with
    /// toolchain-based builds.
    ///
    /// TODO(toolchains): Implement proper toolchain resolution.
    #[starlark(attribute)]
    fn toolchains<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(ToolchainsStub))
    }

    /// Feature flags enabled for this target (Bazel-compatible).
    ///
    /// Returns a list of feature flag names enabled for this target via the
    /// `features` attribute in the BUILD file or command line.
    ///
    /// TODO(features): Populate from actual target configuration.
    #[starlark(attribute)]
    fn features<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        use starlark::values::list::AllocList;
        // Return empty list for now
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Feature flags disabled for this target (Bazel-compatible).
    ///
    /// Returns a list of feature flag names disabled for this target.
    ///
    /// TODO(features): Populate from actual target configuration.
    #[starlark(attribute)]
    fn disabled_features<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        use starlark::values::list::AllocList;
        // Return empty list for now
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Build configuration for this target (Bazel-compatible).
    ///
    /// Provides access to build configuration settings like coverage_enabled.
    #[starlark(attribute)]
    fn configuration<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(BuildConfigurationStub))
    }

    /// Files from attributes (Bazel-compatible).
    ///
    /// Provides access to files from label/label_list attributes as ctx.files.<attr>.
    /// Returns a struct-like object that returns empty lists for any attribute.
    #[starlark(attribute)]
    fn files<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CtxFilesStub))
    }

    /// Single file from attributes (Bazel-compatible).
    ///
    /// Similar to ctx.files but for attrs that expect a single file.
    #[starlark(attribute)]
    fn file<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CtxFileStub))
    }

    /// Executable from attributes (Bazel-compatible).
    ///
    /// Returns executables from label attributes.
    #[starlark(attribute)]
    fn executable<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CtxExecutableStub))
    }

    /// Build file path (Bazel-compatible).
    #[starlark(attribute)]
    fn build_file_path<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc_str("BUILD.bazel").to_value())
    }

    /// Returns whether this target should be instrumented for coverage (Bazel-compatible).
    fn coverage_instrumented(this: RefAnalysisContext) -> starlark::Result<bool> {
        let _ = this;
        // For now, coverage is not enabled
        Ok(false)
    }

    /// Creates a runfiles object (Bazel-compatible).
    ///
    /// Returns a runfiles object that can be merged with other runfiles.
    #[allow(unused_variables)]
    fn runfiles<'v>(
        this: RefAnalysisContext<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] files: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] transitive_files: Value<'v>,
        #[starlark(require = named, default = false)] collect_default: bool,
        #[starlark(require = named, default = false)] collect_data: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)] symlinks: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] root_symlinks: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty runfiles stub
        Ok(heap.alloc(crate::interpreter::rule_defs::provider::builtin::default_info::RunfilesStub))
    }
}

// ============================================================================
// ToolchainsStub - Stub for ctx.toolchains
// ============================================================================

/// A stub for ctx.toolchains that pretends to contain all toolchain types.
///
/// This allows rules_cc and other Bazel rules to proceed with toolchain-based builds
/// without implementing full toolchain resolution. The stub returns a CcToolchainInfoStub
/// when indexed, which provides minimal toolchain info.
///
/// TODO(toolchains): Replace with proper toolchain resolution.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ToolchainsStub;

impl std::fmt::Display for ToolchainsStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<toolchains>")
    }
}

starlark::starlark_simple_value!(ToolchainsStub);

#[starlark::values::starlark_value(type = "toolchains")]
impl<'v> StarlarkValue<'v> for ToolchainsStub {
    /// Returns True for any key - pretends to contain all toolchain types.
    fn is_in(&self, _other: Value<'v>) -> starlark::Result<bool> {
        // Return True for any toolchain type check
        Ok(true)
    }

    /// Returns a CcToolchainInfoStub when indexed with any key.
    fn at(&self, _index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcToolchainInfoStub))
    }
}

/// A stub for CcToolchainInfo that provides minimal toolchain info.
///
/// This provides the attributes that rules_cc expects from a CcToolchainInfo provider.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainInfoStub;

impl std::fmt::Display for CcToolchainInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CcToolchainInfo>")
    }
}

starlark::starlark_simple_value!(CcToolchainInfoStub);

#[starlark::values::starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_toolchain_info_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        // Report which attributes exist for hasattr() checks
        matches!(
            attribute,
            "cc" | "cc_provider_in_toolchain" | "toolchain_id" | "compiler"
                | "cpu" | "target_gnu_system_name" | "dynamic_runtime_lib"
                | "static_runtime_lib" | "sysroot" | "all_files"
                | "compiler_files" | "linker_files" | "ar_files"
                | "objcopy_files" | "strip_files" | "gcov_files"
                | "_supports_header_parsing" | "_needs_pic_for_dynamic_libraries"
                | "_use_pic_for_dynamic_libraries_not_for_binaries"
                | "_supports_start_end_lib" | "_feature_configuration"
                | "_tool_paths" | "libc" | "_abi_glibc_version" | "_abi"
                | "_crosstool_top_path" | "_legacy_cc_flags_make_variable"
                | "_build_variables" | "_coverage_files" | "_strip_files"
                | "_cpp_configuration" | "_if_so_builder" | "_solib_dir"
                | "_build_variables_dict" | "_ar_files" | "_linker_files"
                | "_supports_param_files"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "cc_provider_in_toolchain" => Some(Value::new_bool(false)),
            "toolchain_id" => Some(heap.alloc_str("local_cc_toolchain").to_value()),
            "compiler" => Some(heap.alloc_str("gcc").to_value()),
            "cpu" => Some(heap.alloc_str("k8").to_value()),
            "target_gnu_system_name" => Some(heap.alloc_str("x86_64-linux-gnu").to_value()),
            "sysroot" => Some(Value::new_none()),
            "_supports_header_parsing" => Some(Value::new_bool(true)),
            "_needs_pic_for_dynamic_libraries" => Some(Value::new_bool(true)),
            "_use_pic_for_dynamic_libraries_not_for_binaries" => Some(Value::new_bool(false)),
            "_supports_start_end_lib" => Some(Value::new_bool(false)),
            "_cc_info" => Some(heap.alloc(CcInfoStub)),
            "_tool_paths" => Some(heap.alloc(ToolPathsStub)),
            "_toolchain_features" => Some(heap.alloc(ToolchainFeaturesStub)),
            "_is_tool_configuration" => Some(Value::new_bool(false)),
            "_fdo_context" => Some(Value::new_none()),
            "libc" => Some(heap.alloc_str("glibc").to_value()),
            "_abi_glibc_version" => Some(heap.alloc_str("2.17").to_value()),
            "_abi" => Some(heap.alloc_str("local").to_value()),
            "_crosstool_top_path" => Some(heap.alloc_str("external/local_config_cc").to_value()),
            "_legacy_cc_flags_make_variable" => Some(heap.alloc_str("").to_value()),
            "_build_variables" => Some(heap.alloc(DepsetStub)),
            "_coverage_files" => Some(heap.alloc(DepsetStub)),
            "_strip_files" => Some(heap.alloc(DepsetStub)),
            "_cpp_configuration" => Some(heap.alloc(CppFragment::default())),
            "_if_so_builder" => Some(Value::new_none()),
            "_solib_dir" => Some(heap.alloc_str("_solib_k8").to_value()),
            "_build_variables_dict" => {
                let map: SmallMap<Value, Value> = SmallMap::new();
                Some(heap.alloc(Dict::new(map)))
            }
            "_ar_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_linker_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_supports_param_files" => Some(Value::new_bool(true)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "cc".to_owned(),
            "cc_provider_in_toolchain".to_owned(),
            "toolchain_id".to_owned(),
            "compiler".to_owned(),
            "cpu".to_owned(),
            "target_gnu_system_name".to_owned(),
            "dynamic_runtime_lib".to_owned(),
            "static_runtime_lib".to_owned(),
            "sysroot".to_owned(),
            "all_files".to_owned(),
            "compiler_files".to_owned(),
            "linker_files".to_owned(),
            "ar_files".to_owned(),
            "objcopy_files".to_owned(),
            "strip_files".to_owned(),
            "gcov_files".to_owned(),
            "libc".to_owned(),
        ]
    }
}

#[starlark_module]
fn cc_toolchain_info_stub_methods(builder: &mut MethodsBuilder) {
    /// The C++ toolchain provider itself (for cc_provider_in_toolchain pattern).
    #[starlark(attribute)]
    fn cc<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CcToolchainInfoStub))
    }

    /// Toolchain identifier.
    #[starlark(attribute)]
    fn toolchain_id(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("local_cc_toolchain")
    }

    /// Whether the toolchain is for tool execution.
    #[starlark(attribute)]
    fn is_tool_configuration(this: &CcToolchainInfoStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// Compiler type.
    #[starlark(attribute)]
    fn compiler(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("gcc")
    }

    /// Target CPU architecture.
    #[starlark(attribute)]
    fn cpu(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("k8")
    }

    /// GNU system name for the target.
    #[starlark(attribute)]
    fn target_gnu_system_name(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        Ok("x86_64-linux-gnu")
    }

    /// All input files for the toolchain.
    #[starlark(attribute)]
    fn all_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Compiler input files.
    #[starlark(attribute)]
    fn compiler_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Linker input files.
    #[starlark(attribute)]
    fn linker_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Archiver input files.
    #[starlark(attribute)]
    fn ar_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Strip input files.
    #[starlark(attribute)]
    fn strip_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Objcopy input files.
    #[starlark(attribute)]
    fn objcopy_files<'v>(
        this: &CcToolchainInfoStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Returns the dynamic runtime libraries for the given feature configuration.
    #[allow(unused_variables)]
    fn dynamic_runtime_lib<'v>(
        this: &CcToolchainInfoStub,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return empty depset - no dynamic runtime libraries
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Returns the static runtime libraries for the given feature configuration.
    #[allow(unused_variables)]
    fn static_runtime_lib<'v>(
        this: &CcToolchainInfoStub,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return empty depset - no static runtime libraries
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Returns whether PIC is required for dynamic libraries.
    #[allow(unused_variables)]
    fn needs_pic_for_dynamic_libraries<'v>(
        this: &CcToolchainInfoStub,
        #[starlark(require = named)] feature_configuration: Value<'v>,
    ) -> starlark::Result<bool> {
        // Return true - PIC is typically needed for dynamic libraries
        Ok(true)
    }

    /// Returns whether PIC is used only for dynamic libraries.
    #[allow(unused_variables)]
    fn use_pic_for_dynamic_libraries_not_for_binaries<'v>(
        this: &CcToolchainInfoStub,
        #[starlark(require = named)] feature_configuration: Value<'v>,
    ) -> starlark::Result<bool> {
        // Return false - use PIC everywhere
        Ok(false)
    }
}

/// A stub for CcInfo with compilation_context and linking_context.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoStub;

impl std::fmt::Display for CcInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CcInfo>")
    }
}

starlark::starlark_simple_value!(CcInfoStub);

#[starlark::values::starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "compilation_context" | "linking_context")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "compilation_context" => Some(heap.alloc(CompilationContextStub)),
            "linking_context" => Some(heap.alloc(LinkingContextStub)),
            _ => None,
        }
    }
}

/// A stub for CompilationContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CompilationContextStub;

impl std::fmt::Display for CompilationContextStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CompilationContext>")
    }
}

starlark::starlark_simple_value!(CompilationContextStub);

#[starlark::values::starlark_value(type = "CompilationContext")]
impl<'v> StarlarkValue<'v> for CompilationContextStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "_module_map" | "headers" | "system_includes" | "includes"
                | "quote_includes" | "defines" | "local_defines" | "framework_includes"
                | "_exporting_module_maps" | "direct_headers" | "direct_public_headers"
                | "direct_private_headers" | "direct_textual_headers" | "_header_info"
                | "external_includes" | "_non_code_inputs" | "_virtual_to_original_headers"
                | "validation_artifacts" | "_transitive_modules" | "_transitive_pic_modules"
                | "_modules_info_files" | "_pic_modules_info_files" | "_module_files"
                | "_pic_module_files" | "_direct_module_maps"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "_module_map" => Some(Value::new_none()),
            "headers" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "system_includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "quote_includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "defines" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "local_defines" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "framework_includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "external_includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_non_code_inputs" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_virtual_to_original_headers" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "validation_artifacts" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_transitive_modules" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_transitive_pic_modules" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_modules_info_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_pic_modules_info_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_module_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_pic_module_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_direct_module_maps" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_exporting_module_maps" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_public_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_private_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_textual_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "_header_info" => Some(heap.alloc(HeaderInfoStubSimple)),
            _ => None,
        }
    }
}

/// A stub for HeaderInfo for CompilationContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct HeaderInfoStubSimple;

impl std::fmt::Display for HeaderInfoStubSimple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<HeaderInfo>")
    }
}

starlark::starlark_simple_value!(HeaderInfoStubSimple);

#[starlark::values::starlark_value(type = "HeaderInfo")]
impl<'v> StarlarkValue<'v> for HeaderInfoStubSimple {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "modular_public_headers"
                | "modular_private_headers"
                | "textual_headers"
                | "separate_module_headers"
                | "header_module"
                | "pic_header_module"
                | "separate_module"
                | "separate_pic_module"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "modular_public_headers" | "modular_private_headers" | "textual_headers"
            | "separate_module_headers" => {
                // Return empty list
                Some(heap.alloc(Vec::<Value>::new()))
            }
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                // Return None
                Some(Value::new_none())
            }
            _ => None,
        }
    }
}

/// A stub for LinkingContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct LinkingContextStub;

impl std::fmt::Display for LinkingContextStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<LinkingContext>")
    }
}

starlark::starlark_simple_value!(LinkingContextStub);

#[starlark::values::starlark_value(type = "LinkingContext")]
impl<'v> StarlarkValue<'v> for LinkingContextStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "linker_inputs")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "linker_inputs" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            _ => None,
        }
    }
}

/// A stub for ctx.files that returns empty lists for all attributes.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxFilesStub;

impl std::fmt::Display for CtxFilesStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.files>")
    }
}

starlark::starlark_simple_value!(CtxFilesStub);

#[starlark::values::starlark_value(type = "ctx_files")]
impl<'v> StarlarkValue<'v> for CtxFilesStub {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // All attributes exist but return empty lists
        true
    }

    fn get_attr(&self, _attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // Return empty list for any attribute
        use starlark::values::list::AllocList;
        Some(heap.alloc(AllocList::EMPTY))
    }
}

/// A stub for ctx.file that returns None for all attributes.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxFileStub;

impl std::fmt::Display for CtxFileStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.file>")
    }
}

starlark::starlark_simple_value!(CtxFileStub);

#[starlark::values::starlark_value(type = "ctx_file")]
impl<'v> StarlarkValue<'v> for CtxFileStub {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        Some(Value::new_none())
    }
}

/// A stub for ctx.executable that returns None for all attributes.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxExecutableStub;

impl std::fmt::Display for CtxExecutableStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.executable>")
    }
}

starlark::starlark_simple_value!(CtxExecutableStub);

#[starlark::values::starlark_value(type = "ctx_executable")]
impl<'v> StarlarkValue<'v> for CtxExecutableStub {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        Some(Value::new_none())
    }
}

/// A stub for ctx.configuration.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BuildConfigurationStub;

impl std::fmt::Display for BuildConfigurationStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<configuration>")
    }
}

starlark::starlark_simple_value!(BuildConfigurationStub);

#[starlark::values::starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for BuildConfigurationStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(build_configuration_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "coverage_enabled" | "host_path_separator")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "coverage_enabled" => Some(Value::new_bool(false)),
            "host_path_separator" => Some(heap.alloc_str(":").to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn build_configuration_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &BuildConfigurationStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }
}

/// A stub for empty depset values.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct DepsetStub;

impl std::fmt::Display for DepsetStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "depset([])")
    }
}

starlark::starlark_simple_value!(DepsetStub);

#[starlark::values::starlark_value(type = "depset")]
impl<'v> StarlarkValue<'v> for DepsetStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(depset_stub_methods)
    }
}

#[starlark_module]
fn depset_stub_methods(builder: &mut MethodsBuilder) {
    /// Convert to list (returns empty list).
    fn to_list<'v>(this: &DepsetStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(Vec::<Value>::new()))
    }
}

/// A stub for _toolchain_features that provides feature configuration.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ToolchainFeaturesStub;

impl std::fmt::Display for ToolchainFeaturesStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<toolchain_features>")
    }
}

starlark::starlark_simple_value!(ToolchainFeaturesStub);

#[starlark::values::starlark_value(type = "CcToolchainFeatures")]
impl<'v> StarlarkValue<'v> for ToolchainFeaturesStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(toolchain_features_stub_methods)
    }
}

#[starlark_module]
fn toolchain_features_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns default features and action configs.
    fn default_features_and_action_configs<'v>(
        this: &ToolchainFeaturesStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        use starlark::values::list::AllocList;
        // Return empty list of features
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns the set of all feature names.
    fn feature_names<'v>(
        this: &ToolchainFeaturesStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        use starlark::values::list::AllocList;
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns the set of all action config names.
    fn action_config_names<'v>(
        this: &ToolchainFeaturesStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        use starlark::values::list::AllocList;
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Configures features based on requested features list.
    fn configure_features<'v>(
        this: &ToolchainFeaturesStub,
        #[starlark(require = named)] requested_features: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (this, requested_features);
        Ok(heap.alloc(FeatureConfigurationStub))
    }
}

/// A stub for FeatureConfiguration returned by configure_features.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FeatureConfigurationStub;

impl std::fmt::Display for FeatureConfigurationStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<FeatureConfiguration>")
    }
}

starlark::starlark_simple_value!(FeatureConfigurationStub);

#[starlark::values::starlark_value(type = "FeatureConfiguration")]
impl<'v> StarlarkValue<'v> for FeatureConfigurationStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(feature_configuration_stub_methods)
    }
}

#[starlark_module]
fn feature_configuration_stub_methods(builder: &mut MethodsBuilder) {
    /// Check if a feature is enabled.
    fn is_enabled<'v>(
        this: &FeatureConfigurationStub,
        #[starlark(require = pos)] feature_name: &str,
    ) -> starlark::Result<bool> {
        let _ = this;
        // Return false for most features
        Ok(matches!(feature_name, "supports_dynamic_linker" | "static_linking_mode"))
    }

    /// Check if a feature was requested by the user.
    fn is_requested<'v>(
        this: &FeatureConfigurationStub,
        #[starlark(require = pos)] feature_name: &str,
    ) -> starlark::Result<bool> {
        let _ = this;
        // Return false - no features explicitly requested
        Ok(false)
    }
}

/// A stub for _tool_paths dict that returns tool paths.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ToolPathsStub;

impl std::fmt::Display for ToolPathsStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<tool_paths>")
    }
}

starlark::starlark_simple_value!(ToolPathsStub);

#[starlark::values::starlark_value(type = "dict")]
impl<'v> StarlarkValue<'v> for ToolPathsStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(tool_paths_stub_methods)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let key = index.unpack_str().unwrap_or("unknown");
        let path = match key {
            "gcc" | "g++" | "cpp" => "/usr/bin/gcc",
            "ar" => "/usr/bin/ar",
            "ld" => "/usr/bin/ld",
            "nm" => "/usr/bin/nm",
            "objcopy" => "/usr/bin/objcopy",
            "objdump" => "/usr/bin/objdump",
            "strip" => "/usr/bin/strip",
            "gcov" => "/usr/bin/gcov",
            "dwp" => "/usr/bin/dwp",
            "llvm-profdata" => "/usr/bin/llvm-profdata",
            _ => "",
        };
        Ok(heap.alloc_str(path).to_value())
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // Return true if the key is a known tool
        if let Some(key) = other.unpack_str() {
            Ok(matches!(
                key,
                "gcc" | "g++" | "cpp" | "ar" | "ld" | "nm" | "objcopy"
                    | "objdump" | "strip" | "gcov" | "dwp" | "llvm-profdata"
            ))
        } else {
            Ok(false)
        }
    }
}

#[starlark_module]
fn tool_paths_stub_methods(builder: &mut MethodsBuilder) {
    /// Get a tool path by name, or return the default if not found.
    fn get<'v>(
        this: &ToolPathsStub,
        #[starlark(require = pos)] key: &str,
        #[starlark(default = NoneType)] default: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        let path = match key {
            "gcc" | "g++" | "cpp" => "/usr/bin/gcc",
            "ar" => "/usr/bin/ar",
            "ld" => "/usr/bin/ld",
            "nm" => "/usr/bin/nm",
            "objcopy" => "/usr/bin/objcopy",
            "objdump" => "/usr/bin/objdump",
            "strip" => "/usr/bin/strip",
            "gcov" => "/usr/bin/gcov",
            "dwp" => "/usr/bin/dwp",
            "llvm-profdata" => "/usr/bin/llvm-profdata",
            _ => return Ok(default),
        };
        Ok(heap.alloc_str(path).to_value())
    }
}

#[starlark_module]
pub(crate) fn register_analysis_context(builder: &mut GlobalsBuilder) {
    const AnalysisContext: StarlarkValueAsType<AnalysisContext> = StarlarkValueAsType::new();
    const AnalysisActions: StarlarkValueAsType<AnalysisActions> = StarlarkValueAsType::new();
}

pub static ANALYSIS_ACTIONS_METHODS_ACTIONS: LateBinding<fn(&mut MethodsBuilder)> =
    LateBinding::new("ANALYSIS_ACTIONS_METHODS_ACTIONS");
pub static ANALYSIS_ACTIONS_METHODS_ANON_TARGET: LateBinding<fn(&mut MethodsBuilder)> =
    LateBinding::new("ANALYSIS_ACTIONS_METHODS_ANON_TARGET");

pub(crate) fn init_analysis_context_ty() {
    AnalysisContextReprLate::init(AnalysisContext::starlark_type_repr());
}
