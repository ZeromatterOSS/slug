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
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dice::DiceComputations;
use futures::FutureExt;
use kuro_core::provider::id::ProviderId;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::provider::label::ProvidersName;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_execute::digest_config::DigestConfig;
use kuro_interpreter::late_binding_ty::AnalysisContextReprLate;
use kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel;
use kuro_util::late_binding::LateBinding;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::typing::Ty;
use starlark::values::AllocValue;
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
use starlark::values::dict::Dict;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::StructRef;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::analysis::anon_promises_dyn::RunAnonPromisesAccessor;
use crate::analysis::registry::AnalysisRegistry;
use crate::deferred::calculation::GET_PROMISED_ARTIFACT;
use crate::interpreter::rule_defs::artifact::methods::ArtifactRoot;
use crate::interpreter::rule_defs::bazel_label::BazelLabel;
use crate::interpreter::rule_defs::cc_common::CcToolchainInfoProvider;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::fragments::CppFragment;
use crate::interpreter::rule_defs::plugins::AnalysisPlugins;
use crate::interpreter::rule_defs::provider::ProviderLike;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;

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
    /// Cached outputs for Bazel-compatible ctx.outputs access.
    /// This is computed lazily on first access and cached thereafter.
    outputs: RefCell<Option<Value<'v>>>,
    /// Implicit output patterns from `rule(outputs={...})`.
    /// Each pair is (name, pattern) where pattern may contain `%{name}`.
    rule_outputs: Vec<(String, String)>,
    /// Resolved toolchains from real toolchain resolution.
    /// When Some, ctx.toolchains returns this instead of empty ResolvedToolchains.
    resolved_toolchains: RefCell<Option<Value<'v>>>,
    /// Resolved exec group toolchains from per-group resolution.
    /// When Some, ctx.exec_groups returns real per-group results.
    resolved_exec_groups: RefCell<Option<Value<'v>>>,
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
        rule_outputs: Vec<(String, String)>,
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
            outputs: RefCell::new(None),
            rule_outputs,
            resolved_toolchains: RefCell::new(None),
            resolved_exec_groups: RefCell::new(None),
        }
    }

    pub fn prepare(
        heap: Heap<'v>,
        attrs: Option<ValueOfUnchecked<'v, StructRef<'static>>>,
        label: Option<ConfiguredTargetLabel>,
        plugins: Option<ValueTypedComplex<'v, AnalysisPlugins<'v>>>,
        registry: AnalysisRegistry<'v>,
        digest_config: DigestConfig,
        rule_outputs: Vec<(String, String)>,
    ) -> ValueTyped<'v, AnalysisContext<'v>> {
        let label = label.map(|label| {
            heap.alloc_typed(StarlarkConfiguredProvidersLabel::new(
                ConfiguredProvidersLabel::new(label, ProvidersName::Default),
            ))
        });

        let mut analysis_context = Self::new(
            heap,
            attrs,
            label,
            plugins,
            registry,
            digest_config,
            rule_outputs,
        );

        heap.alloc_typed(analysis_context)
    }

    /// Set the resolved toolchains for this context.
    /// Called from the analysis pipeline after resolution completes.
    pub fn set_resolved_toolchains(&self, toolchains: Value<'v>) {
        *self.resolved_toolchains.borrow_mut() = Some(toolchains);
    }

    /// Set the resolved exec groups for this context.
    /// Called from the analysis pipeline after per-group resolution completes.
    pub fn set_resolved_exec_groups(&self, exec_groups: Value<'v>) {
        *self.resolved_exec_groups.borrow_mut() = Some(exec_groups);
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

    /// Returns true if this target is being built in exec (tool) configuration.
    /// In Bazel, this means the target is a build tool that runs on the host machine.
    fn is_tool_configuration(&self) -> bool {
        self.label
            .as_ref()
            .map(|l| l.inner().target().exec_cfg().is_some())
            .unwrap_or(false)
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

    /// Split configuration attributes.
    ///
    /// In Bazel, `ctx.split_attr.<name>` returns a dict mapping configuration keys
    /// to attribute values when the attribute uses a split transition (cfg parameter).
    /// Since Kuro does not implement split transitions, each attribute value is
    /// wrapped in a single-entry dict with key `"//conditions:default"`.
    #[starlark(attribute)]
    fn split_attr<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match this.0.attrs {
            Some(attrs) => Ok(heap.alloc(CtxSplitAttr::new(attrs))),
            None => Ok(heap.alloc(CtxSplitAttrUnavailable)),
        }
    }

    /// The workspace name (Bazel-compatible).
    ///
    /// In Bazel, `ctx.workspace_name` returns the name of the workspace
    /// (repository) containing the current target. Returns the root cell name
    /// or empty string for the main workspace.
    #[starlark(attribute)]
    fn workspace_name<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if let Some(label) = this.0.label {
            let cell = label.label().target().pkg().cell_name().as_str();
            // In Bazel, the main repo returns "" or the module name
            if kuro_core::cells::is_root_cell_name(cell) {
                Ok(heap.alloc_str("").to_value())
            } else {
                Ok(heap.alloc_str(cell).to_value())
            }
        } else {
            Ok(heap.alloc_str("").to_value())
        }
    }

    /// The path to the BUILD file for the current target (Bazel-compatible).
    ///
    /// In Bazel, `ctx.build_file_path` returns a path like "pkg/BUILD.bazel"
    /// relative to the workspace root.
    #[starlark(attribute)]
    fn build_file_path<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if let Some(label) = this.0.label {
            let pkg = label.label().target().pkg();
            let pkg_path = pkg.cell_relative_path().as_str();
            let path = if pkg_path.is_empty() {
                "BUILD.bazel".to_owned()
            } else {
                format!("{}/BUILD.bazel", pkg_path)
            };
            Ok(heap.alloc_str(&path).to_value())
        } else {
            Ok(heap.alloc_str("BUILD.bazel").to_value())
        }
    }

    /// Configuration fragments for this target.
    ///
    /// Provides access to language-specific configuration like `ctx.fragments.cpp`,
    /// `ctx.fragments.java`, etc.
    #[starlark(attribute)]
    fn fragments<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Build configuration fragments using the global compilation mode
        let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let force_pic = crate::interpreter::rule_defs::build_config::get_force_pic();
        let coverage = crate::interpreter::rule_defs::build_config::get_collect_code_coverage();
        let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
            mode, force_pic, coverage, false,
        );
        Ok(heap.alloc(ConfigurationFragments::new(cpp)))
    }

    /// Host configuration fragments (Bazel-compatible).
    ///
    /// In Bazel, `ctx.host_fragments` provides access to the host platform's
    /// configuration fragments. Since Kuro doesn't distinguish host from target
    /// configurations yet, this returns the same fragments as `ctx.fragments`.
    #[starlark(attribute)]
    fn host_fragments<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let force_pic = crate::interpreter::rule_defs::build_config::get_force_pic();
        let coverage = crate::interpreter::rule_defs::build_config::get_collect_code_coverage();
        let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
            mode, force_pic, coverage, false,
        );
        Ok(heap.alloc(ConfigurationFragments::new(cpp)))
    }

    /// Resolved toolchains for this target (Bazel-compatible).
    ///
    /// In Bazel, rules can declare toolchain dependencies via the `toolchains` attribute
    /// on rule(). During analysis, resolved toolchains are provided via `ctx.toolchains`,
    /// which is a dict-like object mapping toolchain types to toolchain info providers.
    ///
    /// Returns real resolved toolchains if available, or an empty ResolvedToolchains
    /// if no resolution ran for this target.
    #[starlark(attribute)]
    fn toolchains<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Return real resolved toolchains if available
        if let Some(resolved) = *this.0.resolved_toolchains.borrow() {
            return Ok(resolved);
        }
        // Return empty resolved toolchains — no stubs
        Ok(heap.alloc(ResolvedToolchains {
            toolchains: std::collections::HashMap::new(),
            exec_platform: String::new(),
        }))
    }

    /// Predeclared outputs from the rule definition (Bazel-compatible).
    ///
    /// In Bazel, `attr.output()` attributes hold filename strings that the rule can
    /// use to declare output files via `ctx.outputs.<name>`. This implementation
    /// dynamically declares files on first access based on the string values in attrs.
    #[starlark(attribute)]
    fn outputs<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Check if we already have cached outputs
        if let Some(cached) = this.0.outputs.borrow().as_ref() {
            return Ok(*cached);
        }

        let attrs_val = this
            .0
            .attrs
            .map(|v| v.get())
            .unwrap_or_else(Value::new_none);

        let target_name = this
            .0
            .label
            .as_ref()
            .map(|l| l.label().target().name().as_str().to_owned())
            .unwrap_or_else(|| "output".to_string());

        let outputs_value = heap.alloc(CtxOutputs {
            attrs: attrs_val,
            actions: this.0.actions,
            declared: RefCell::new(SmallMap::new()),
            target_name,
            rule_outputs: this.0.rule_outputs.clone(),
        });

        // Cache the result
        *this.0.outputs.borrow_mut() = Some(outputs_value);

        Ok(outputs_value)
    }

    /// Feature flags enabled for this target (Bazel-compatible).
    ///
    /// Returns a list of feature flag names enabled for this target via the
    /// `features` attribute in the BUILD file or command line.
    ///
    /// Returns the list of features enabled for this target via the `features` attribute.
    /// Features with a `-` prefix are excluded (those are "disabled" features).
    #[starlark(attribute)]
    fn features<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        use starlark::values::list::AllocList;
        if let Some(attrs) = this.0.attrs {
            if let Some(features_val) = attrs.get().get_attr("features", heap)? {
                if let Some(list) = starlark::values::list::ListRef::from_value(features_val) {
                    let enabled: Vec<Value<'v>> = list
                        .iter()
                        .filter(|v| v.unpack_str().map(|s| !s.starts_with('-')).unwrap_or(false))
                        .collect();
                    return Ok(heap.alloc(AllocList(enabled)));
                }
            }
        }
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Feature flags disabled for this target (Bazel-compatible).
    ///
    /// Returns a list of feature flag names disabled for this target.
    /// These come from features with a `-` prefix in the `features` attribute.
    #[starlark(attribute)]
    fn disabled_features<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::list::AllocList;
        if let Some(attrs) = this.0.attrs {
            if let Some(features_val) = attrs.get().get_attr("features", heap)? {
                if let Some(list) = starlark::values::list::ListRef::from_value(features_val) {
                    let disabled: Vec<Value<'v>> = list
                        .iter()
                        .filter_map(|v| {
                            v.unpack_str().and_then(|s| {
                                if s.starts_with('-') {
                                    // Return the feature name without the '-' prefix
                                    Some(heap.alloc_str(&s[1..]).to_value())
                                } else {
                                    None
                                }
                            })
                        })
                        .collect();
                    return Ok(heap.alloc(AllocList(disabled)));
                }
            }
        }
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
        let is_tool = this.0.is_tool_configuration();
        let (config_hash, config_label) = match &this.0.label {
            Some(label) => {
                let cfg = label.inner().target().cfg();
                (
                    cfg.output_hash().as_str().to_owned(),
                    cfg.full_name().to_owned(),
                )
            }
            None => (String::new(), "unknown".to_owned()),
        };
        Ok(heap.alloc(BuildConfiguration {
            is_tool,
            config_hash,
            config_label,
        }))
    }

    /// Files from attributes (Bazel-compatible).
    ///
    /// Provides access to files from label/label_list attributes as ctx.files.<attr>.
    /// For example, `ctx.files.srcs` returns a list of files from the `srcs` attribute.
    #[starlark(attribute)]
    fn files<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match this.0.attrs {
            Some(attrs) => Ok(heap.alloc(CtxFiles::new(attrs))),
            None => {
                // Fallback for dynamic_output or BXL contexts
                Ok(heap.alloc(CtxFilesUnavailable))
            }
        }
    }

    /// Single file from attributes (Bazel-compatible).
    ///
    /// Similar to ctx.files but for attrs that expect a single file.
    /// For example, `ctx.file.src` returns a single File from the `src` attribute.
    #[starlark(attribute)]
    fn file<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match this.0.attrs {
            Some(attrs) => Ok(heap.alloc(CtxFile::new(attrs))),
            None => {
                // Fallback for dynamic_output or BXL contexts
                Ok(heap.alloc(CtxFileUnavailable))
            }
        }
    }

    /// Executable from attributes (Bazel-compatible).
    ///
    /// Returns executables from label attributes declared with `executable=True`.
    /// For example, `ctx.executable._compiler` returns the compiler executable.
    #[starlark(attribute)]
    fn executable<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        match this.0.attrs {
            Some(attrs) => Ok(heap.alloc(CtxExecutable::new(attrs))),
            None => {
                // Fallback for dynamic_output or BXL contexts
                Ok(heap.alloc(CtxExecutableUnavailable))
            }
        }
    }

    /// Build file path (Bazel-compatible).
    ///
    /// Returns the path to the BUILD file relative to the workspace root.
    #[starlark(attribute)]
    fn build_file_path<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if let Some(label) = this.0.label {
            let pkg = label.label().target().pkg();
            let pkg_path = pkg.cell_relative_path().as_str();
            let path = if pkg_path.is_empty() {
                "BUILD.bazel".to_owned()
            } else {
                format!("{}/BUILD.bazel", pkg_path)
            };
            Ok(heap.alloc_str(&path).to_value())
        } else {
            Ok(heap.alloc_str("BUILD.bazel").to_value())
        }
    }

    /// Workspace name (Bazel-compatible).
    ///
    /// Returns the name of the current workspace/module from MODULE.bazel.
    ///
    /// In Bazel with bzlmod:
    /// - Root module targets return `"_main"` (Bazel standard for root workspace)
    /// - External module targets return the module's apparent name (cell name)
    ///
    /// Returns an empty string if the workspace name is not available.
    #[starlark(attribute)]
    fn workspace_name<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if let Some(label) = this.0.label {
            let cell_name = label.label().target().pkg().cell_name().as_str();
            // In Bazel with bzlmod, the root module is known as "_main".
            // External modules use their apparent name (cell name).
            if kuro_core::cells::is_root_cell_name(cell_name) {
                Ok(heap.alloc_str("_main").to_value())
            } else {
                Ok(heap.alloc_str(cell_name).to_value())
            }
        } else {
            Ok(heap.alloc_str("").to_value())
        }
    }

    /// Output directory for binary artifacts (Bazel-compatible).
    ///
    /// Returns a root object representing the output tree for binaries.
    /// Access the path via `ctx.bin_dir.path`.
    ///
    /// In Kuro, derives the path from the target's cell name and configuration hash:
    /// `buck-out/v2/gen/<cell>/<cfg_hash>`
    #[starlark(attribute)]
    fn bin_dir<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let path = bin_dir_path_from_label(this.0.label);
        Ok(heap.alloc(CtxDirRoot { path }))
    }

    /// Output directory for generated files (Bazel-compatible).
    ///
    /// Returns a root object representing the output tree for generated files.
    /// Access the path via `ctx.genfiles_dir.path`.
    ///
    /// In Kuro, this returns the same root as `bin_dir` since there is no
    /// separate genfiles directory.
    #[starlark(attribute)]
    fn genfiles_dir<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // In Kuro, genfiles and bin share the same output root
        let path = bin_dir_path_from_label(this.0.label);
        Ok(heap.alloc(CtxDirRoot { path }))
    }

    /// Volatile build status file (Bazel-compatible).
    ///
    /// In Bazel, `ctx.version_file` provides access to the volatile-status.txt file
    /// which contains stamping info like BUILD_TIMESTAMP.
    /// Returns a File object with the standard path attributes.
    #[starlark(attribute)]
    fn version_file<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(StampFile {
            full_path: "bazel-out/volatile-status.txt".to_owned(),
            short_path: "volatile-status.txt".to_owned(),
        }))
    }

    /// Stable build status file (Bazel-compatible).
    ///
    /// In Bazel, `ctx.info_file` provides access to the stable-status.txt file
    /// which contains stamping info like BUILD_EMBED_LABEL.
    /// Returns a File object with the standard path attributes.
    #[starlark(attribute)]
    fn info_file<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(StampFile {
            full_path: "bazel-out/stable-status.txt".to_owned(),
            short_path: "stable-status.txt".to_owned(),
        }))
    }

    /// Execution groups for this rule (Bazel-compatible).
    ///
    /// Returns a dict-like object providing access to execution groups defined
    /// in the rule's `exec_groups` parameter. Each exec group has a `toolchains`
    /// attribute that provides resolved toolchains for that group.
    ///
    /// Example:
    /// ```python
    /// tc = ctx.exec_groups["test"].toolchains["@bazel_tools//tools/cpp:test_runner_toolchain_type"]
    /// ```
    ///
    #[starlark(attribute)]
    fn exec_groups<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        if let Some(resolved) = this.0.resolved_exec_groups.borrow().as_ref() {
            return Ok(*resolved);
        }
        // Fallback: return empty ResolvedExecGroups
        Ok(heap.alloc(ResolvedExecGroups {
            groups: std::collections::HashMap::new(),
            valid_names: Vec::new(),
        }))
    }

    /// Make variable access (Bazel-compatible).
    ///
    /// Returns a dict-like object providing access to Make variables.
    /// Common variables include:
    /// - `BINDIR`: Binary output directory
    /// - `GENDIR`: Generated files directory
    /// - `TARGET_CPU`: Target CPU architecture
    /// - `COMPILATION_MODE`: Current compilation mode (fastbuild, dbg, opt)
    ///
    /// Example:
    /// ```python
    /// bin_dir = ctx.var["BINDIR"]
    /// ```
    #[starlark(attribute)]
    fn var<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Return an actual Dict so that dict(ctx.var) and iteration work correctly.
        // BINDIR and GENDIR are derived from the actual target's cell/configuration.
        let bin_dir = bin_dir_path_from_label(this.0.label);
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        use starlark::values::dict::Dict;
        let entries: &[(&str, &str)] = &[
            ("BINDIR", bin_dir.as_str()),
            ("GENDIR", bin_dir.as_str()),
            ("TARGET_CPU", host_target_cpu()),
            ("COMPILATION_MODE", comp_mode.as_str()),
            ("CC", host_cc_path()),
            ("CC_FLAGS", ""),
            (
                "JAVA",
                if cfg!(windows) {
                    "java.exe"
                } else {
                    "/usr/bin/java"
                },
            ),
            ("JAVA_RUNFILES", ""),
            ("JAVABASE", ""),
            ("ABI_GLIBC_VERSION", "2.17"),
            ("ABI", "local"),
        ];
        let mut map: SmallMap<Value, Value> = SmallMap::new();
        for (k, v) in entries {
            let key = heap.alloc_str(k).to_value();
            let val = heap.alloc_str(v).to_value();
            map.insert_hashed(key.get_hashed().unwrap(), val);
        }
        // Merge --define KEY=VALUE entries into ctx.var
        for (k, v) in crate::interpreter::rule_defs::build_config::get_all_defines() {
            let key = heap.alloc_str(&k).to_value();
            let val = heap.alloc_str(&v).to_value();
            map.insert_hashed(key.get_hashed().unwrap(), val);
        }
        Ok(heap.alloc(Dict::new(map)))
    }

    /// Returns the value of a build setting rule (Bazel-compatible).
    ///
    /// For rules declared with `build_setting = config.string()` or similar,
    /// this returns the current value of the build setting. If the flag was
    /// overridden on the command line via `--//pkg:target=value`, the CLI value
    /// takes precedence over `build_setting_default`.
    #[starlark(attribute)]
    fn build_setting_value<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Check if this build setting allows multiple values
        let allows_multiple = this
            .0
            .attrs
            .and_then(|attrs| {
                attrs
                    .get()
                    .get_attr("_build_setting_allows_multiple", heap)
                    .ok()
                    .flatten()
                    .and_then(|v| v.unpack_bool())
            })
            .unwrap_or(false);

        // Check for command-line override via --//pkg:target=value
        if let Some(label) = this.0.label {
            let target = label.label().target();
            // Build the label string in //pkg:target format
            let pkg_path = target.pkg().cell_relative_path().as_str();
            let target_name = target.name().as_str();
            let label_str = if pkg_path.is_empty() {
                format!("//:{}", target_name)
            } else {
                format!("//{}:{}", pkg_path, target_name)
            };

            if let Some(cli_value) =
                crate::interpreter::rule_defs::build_config::get_starlark_flag(&label_str)
            {
                // Parse the CLI string value into the appropriate type
                // For bool settings, convert "True"/"False"/"true"/"false" to bool
                // For int settings, parse as int
                // For string/string_list, return as-is
                let value = match cli_value.as_str() {
                    "True" | "true" | "1" => heap.alloc(true).to_value(),
                    "False" | "false" | "0" => heap.alloc(false).to_value(),
                    s => heap.alloc_str(s).to_value(),
                };
                if allows_multiple {
                    use starlark::values::list::AllocList;
                    return Ok(heap.alloc(AllocList([value])));
                }
                return Ok(value);
            }
        }

        // Fall back to build_setting_default attribute
        if let Some(attrs) = this.0.attrs {
            if let Some(val) = attrs.get().get_attr("build_setting_default", heap)? {
                if allows_multiple {
                    use starlark::values::list::ListRef;
                    if ListRef::from_value(val).is_some() {
                        return Ok(val);
                    }
                    use starlark::values::list::AllocList;
                    return Ok(heap.alloc(AllocList([val])));
                }
                return Ok(val);
            }
        }
        // Default: return empty list for list settings
        use starlark::values::list::AllocList;
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns whether a target should be instrumented for coverage (Bazel-compatible).
    ///
    /// If dep is provided, returns whether that dependency is instrumented.
    /// If dep is None, returns whether the current rule is instrumented.
    #[allow(unused_variables)]
    fn coverage_instrumented<'v>(
        this: RefAnalysisContext,
        #[starlark(default = NoneType)] dep: Value<'v>,
    ) -> starlark::Result<bool> {
        let _ = (this, dep);
        Ok(crate::interpreter::rule_defs::build_config::get_collect_code_coverage())
    }

    /// Splits a shell command string into a list of tokens.
    ///
    /// Bazel-compatible: splits option_string according to Bourne shell tokenization rules.
    /// Handles single-quoted, double-quoted, and unquoted strings.
    /// Whitespace is used as delimiter outside of quotes.
    #[allow(unused_variables)]
    fn tokenize<'v>(
        this: RefAnalysisContext<'v>,
        option_string: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let tokens = shell_tokenize(option_string);
        Ok(heap.alloc(tokens))
    }

    /// Creates a runfiles object (Bazel-compatible).
    ///
    /// Returns a runfiles object that can be merged with other runfiles.
    #[allow(unused_variables)]
    fn runfiles<'v>(
        this: RefAnalysisContext<'v>,
        #[starlark(default = starlark::values::none::NoneType)] files: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        transitive_files: Value<'v>,
        #[starlark(require = named, default = false)] collect_default: bool,
        #[starlark(require = named, default = false)] collect_data: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)] symlinks: Value<
            'v,
        >,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        root_symlinks: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let mut runfiles =
            crate::interpreter::rule_defs::provider::builtin::default_info::create_runfiles(
                heap,
                files,
                transitive_files,
                symlinks,
                root_symlinks,
            )?;

        if collect_default || collect_data {
            let Some(attrs) = this.0.attrs else {
                return Ok(runfiles);
            };
            let attrs = attrs.get().to_value();

            let mut collect_attr = |name: &str, want_data: bool| -> starlark::Result<()> {
                if let Some(value) = attrs.get_attr(name, heap).ok().flatten() {
                    collect_runfiles_from_value(value, want_data, heap, &mut runfiles)?;
                }
                Ok(())
            };

            if collect_default {
                collect_attr("deps", false)?;
                collect_attr("runtime_deps", false)?;
            }
            if collect_data {
                collect_attr("data", true)?;
            }
        }

        Ok(runfiles)
    }

    /// Whether the target platform has the given constraint value (Bazel-compatible).
    ///
    /// Used by rules to check if target platform matches certain constraints,
    /// e.g., checking if building for Apple platforms.
    ///
    /// This implementation uses the host platform constraints from std::env::consts
    /// since we don't yet have a full platform resolution system. This correctly
    /// handles rules that check for OS/CPU constraints.
    fn target_platform_has_constraint<'v>(
        this: RefAnalysisContext,
        constraint_value: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<bool> {
        let _ = this;

        // Extract the label from the constraint_value (ConstraintValueInfo instance)
        let label = if let Ok(Some(label_val)) = constraint_value.get_attr("label", heap) {
            label_val.to_str()
        } else {
            // Not a ConstraintValueInfo or has no label attribute; return false
            return Ok(false);
        };
        let label_ref: &str = &label;

        // Map the host OS/CPU to @platforms// constraint labels.
        // These are the labels generated by the `platforms` module from BCR.
        let os_constraint = match std::env::consts::OS {
            "linux" => "@platforms//os:linux",
            "macos" => "@platforms//os:macos",
            "windows" => "@platforms//os:windows",
            "freebsd" => "@platforms//os:freebsd",
            "openbsd" => "@platforms//os:openbsd",
            "netbsd" => "@platforms//os:netbsd",
            _ => "",
        };
        let cpu_constraint = match std::env::consts::ARCH {
            "x86_64" => "@platforms//cpu:x86_64",
            "aarch64" => "@platforms//cpu:aarch64",
            "x86" => "@platforms//cpu:x86_32",
            "arm" => "@platforms//cpu:arm",
            "powerpc64" => "@platforms//cpu:ppc",
            "s390x" => "@platforms//cpu:s390x",
            _ => "",
        };

        for candidate in &[os_constraint, cpu_constraint] {
            if candidate.is_empty() {
                continue;
            }
            // Direct match with or without leading '@'
            // e.g., "@platforms//os:linux" == "@platforms//os:linux"
            //   or  "platforms//os:linux"  == "@platforms//os:linux"
            if label_ref == *candidate {
                return Ok(true);
            }
            // Match without the leading '@' (Kuro stores labels without '@' prefix in canonical form)
            // e.g., "platforms//os:linux" matches "@platforms//os:linux"
            let candidate_without_at = candidate.trim_start_matches('@');
            if label_ref == candidate_without_at {
                return Ok(true);
            }
            // Match by path+name suffix for robustness
            // Extract "//os:linux" from "@platforms//os:linux"
            if let Some(path_part) = candidate_without_at
                .find("//")
                .map(|i| &candidate_without_at[i..])
            {
                if label_ref.ends_with(path_part) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Expands Make variables and additional substitutions in a string (Bazel-compatible).
    ///
    /// This is a deprecated Bazel API that expands `$(VAR)` and `$(execpath ...)` patterns.
    /// Still used by some rules (e.g., rules_python for import path expansion).
    ///
    /// Parameters:
    /// - `attribute_name`: Name of the attribute (for error messages only)
    /// - `command`: The string to expand
    /// - `additional_substitutions`: Extra `$(VAR)` substitutions as a dict
    #[allow(unused_variables)]
    fn expand_make_variables<'v>(
        this: RefAnalysisContext<'v>,
        attribute_name: &str,
        command: &str,
        additional_substitutions: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<String> {
        use starlark::values::dict::DictRef;
        let _ = (attribute_name, heap);

        // Build substitution map from additional_substitutions (user-provided overrides)
        let mut substitutions: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        if let Some(subs_dict) = DictRef::from_value(additional_substitutions) {
            for (k, v) in subs_dict.iter() {
                if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                    substitutions.insert(key.to_owned(), val.to_owned());
                }
            }
        }

        // Add built-in Make variables (BINDIR, GENDIR, CC, etc.) as fallbacks.
        // User-provided substitutions take priority.
        let bin_dir = bin_dir_path_from_label(this.0.label);
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let builtins: &[(&str, &str)] = &[
            ("BINDIR", bin_dir.as_str()),
            ("GENDIR", bin_dir.as_str()),
            ("TARGET_CPU", host_target_cpu()),
            ("COMPILATION_MODE", comp_mode.as_str()),
            ("CC", host_cc_path()),
            ("CC_FLAGS", ""),
            (
                "JAVA",
                if cfg!(windows) {
                    "java.exe"
                } else {
                    "/usr/bin/java"
                },
            ),
            ("JAVA_RUNFILES", ""),
            ("JAVABASE", ""),
            ("ABI_GLIBC_VERSION", "2.17"),
            ("ABI", "local"),
        ];
        for (k, v) in builtins {
            substitutions
                .entry(k.to_string())
                .or_insert_with(|| v.to_string());
        }

        // Merge --define KEY=VALUE entries (lowest priority, after user subs and builtins)
        for (k, v) in crate::interpreter::rule_defs::build_config::get_all_defines() {
            substitutions.entry(k).or_insert(v);
        }

        // Expand $(VAR) patterns using the substitutions
        let mut result = String::with_capacity(command.len());
        let mut remaining = command;
        while let Some(start) = remaining.find("$(") {
            result.push_str(&remaining[..start]);
            remaining = &remaining[start..];

            if let Some(end) = remaining.find(')') {
                let inner = remaining[2..end].trim();
                let after = &remaining[end + 1..];

                if let Some(val) = substitutions.get(inner) {
                    result.push_str(val);
                } else {
                    // Leave unresolved $(VAR) patterns as-is
                    result.push_str(&remaining[..end + 1]);
                }
                remaining = after;
            } else {
                // No closing paren - leave as-is
                result.push_str("$(");
                remaining = &remaining[2..];
            }
        }
        result.push_str(remaining);

        Ok(result)
    }

    /// Converts a string to a Label relative to the current package (Bazel-compatible).
    ///
    /// For example, `ctx.package_relative_label(":foo")` returns the Label for `:foo`
    /// relative to the BUILD file's package.
    fn package_relative_label<'v>(
        this: RefAnalysisContext<'v>,
        input: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Get the current target's cell and package from ctx.label
        let (cell_name, pkg_path) = if let Some(label) = this.0.label {
            let tgt = label.label().target();
            let cell = tgt.pkg().cell_name().as_str().to_owned();
            let pkg = tgt.pkg().cell_relative_path().as_str().to_owned();
            (cell, pkg)
        } else {
            // No label context (dynamic_output / BXL) — return input as-is
            return Ok(heap.alloc(BazelLabel::parse(input)));
        };

        let resolved = if input.starts_with('@') {
            // Already fully qualified
            input.to_owned()
        } else if input.starts_with("//") {
            // Absolute within current cell
            format!("@{}{}", cell_name, input)
        } else {
            // Relative (:target or bare target)
            let target = input.strip_prefix(':').unwrap_or(input);
            format!("@{}//{}:{}", cell_name, pkg_path, target)
        };

        Ok(heap.alloc(BazelLabel::parse(&resolved)))
    }

    /// Resolves tools and returns (input files depset, runfiles manifests) tuple (Bazel-compatible).
    ///
    /// This is used by rules to collect all files needed to run specified tools.
    /// Returns a tuple of (depset of files, list of runfiles manifests).
    #[allow(unused_variables)]
    fn resolve_tools<'v>(
        this: RefAnalysisContext<'v>,
        #[starlark(require = named, default = NoneType)] tools: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::list::AllocList;

        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;
        let _ = this;

        // Collect all DefaultInfo default outputs from the tool dependencies.
        // In Bazel, ctx.resolve_tools() returns (depset_of_files, runfiles_manifests).
        // We collect the files from DefaultInfo.default_outputs and
        // FilesToRunProvider (executable + runfiles manifest if present).
        let mut tool_files: Vec<Value<'v>> = Vec::new();

        let collect_tool_files = |pc: starlark::values::FrozenValueTyped<
            '_,
            crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection,
        >,
                                  files: &mut Vec<Value<'v>>| {
            if let Ok(di) = pc.as_ref().default_info() {
                for art in di.default_outputs() {
                    files.push(heap.alloc(art));
                }
            }
        };

        if let Ok(iter) = tools.iterate(heap) {
            for tool_val in iter {
                if let Some(dep) = tool_val.downcast_ref::<Dependency>() {
                    collect_tool_files(dep.provider_collection(), &mut tool_files);
                } else if let Some(dep) = tool_val.downcast_ref::<FrozenDependency>() {
                    collect_tool_files(dep.provider_collection(), &mut tool_files);
                }
            }
        }

        let files_list = heap.alloc(AllocList(tool_files));
        let empty_manifests = heap.alloc(AllocList::EMPTY);
        Ok(heap.alloc((files_list, empty_manifests)))
    }

    /// Resolves a command to be executed (deprecated Bazel API).
    ///
    /// Returns a tuple of `(inputs, command, input_manifests)` where:
    /// - `inputs`: list of input files needed by the command
    /// - `command`: list of resolved command strings
    /// - `input_manifests`: list of runfiles manifests
    ///
    /// This is a deprecated Bazel API. Prefer `ctx.resolve_tools()` + `ctx.expand_location()`.
    #[allow(unused_variables)]
    fn resolve_command<'v>(
        this: RefAnalysisContext<'v>,
        #[starlark(require = named, default = "")] command: &str,
        #[starlark(require = named, default = NoneType)] attribute: Value<'v>,
        #[starlark(require = named, default = false)] expand_locations: bool,
        #[starlark(require = named, default = NoneType)] make_variables: Value<'v>,
        #[starlark(require = named, default = NoneType)] tools: Value<'v>,
        #[starlark(require = named, default = NoneType)] label_dict: Value<'v>,
        #[starlark(require = named, default = NoneType)] execution_requirements: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::list::AllocList;

        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

        let _ = (attribute, execution_requirements);

        // Collect tool files as inputs
        let mut tool_files: Vec<Value<'v>> = Vec::new();
        let collect_files = |pc: starlark::values::FrozenValueTyped<
            '_,
            crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection,
        >,
                             files: &mut Vec<Value<'v>>| {
            if let Ok(di) = pc.as_ref().default_info() {
                for art in di.default_outputs() {
                    files.push(heap.alloc(art));
                }
            }
        };

        // Collect files from tools parameter
        if !tools.is_none() {
            if let Ok(iter) = tools.iterate(heap) {
                for tool_val in iter {
                    if let Some(dep) = tool_val.downcast_ref::<Dependency>() {
                        collect_files(dep.provider_collection(), &mut tool_files);
                    } else if let Some(dep) = tool_val.downcast_ref::<FrozenDependency>() {
                        collect_files(dep.provider_collection(), &mut tool_files);
                    }
                }
            }
        }

        // Collect files from label_dict parameter
        if !label_dict.is_none() {
            if let Ok(iter) = label_dict.iterate(heap) {
                for dep_val in iter {
                    if let Some(dep) = dep_val.downcast_ref::<Dependency>() {
                        collect_files(dep.provider_collection(), &mut tool_files);
                    } else if let Some(dep) = dep_val.downcast_ref::<FrozenDependency>() {
                        collect_files(dep.provider_collection(), &mut tool_files);
                    }
                }
            }
        }

        // Expand $(location ...) in command string if requested
        let mut resolved_command = if expand_locations && !command.is_empty() {
            // Build combined targets list from tools + label_dict for location expansion
            let mut all_targets: Vec<Value<'v>> = Vec::new();
            if !tools.is_none() {
                if let Ok(iter) = tools.iterate(heap) {
                    all_targets.extend(iter);
                }
            }
            if !label_dict.is_none() {
                if let Ok(iter) = label_dict.iterate(heap) {
                    all_targets.extend(iter);
                }
            }
            let targets_list = heap.alloc(all_targets);
            // Use the existing expand_location logic
            expand_location_in_string(command, targets_list, heap)?
        } else {
            command.to_owned()
        };

        // Expand $(VAR) make variables in command string
        if !make_variables.is_none() {
            if let Ok(iter) = make_variables.iterate(heap) {
                for key_val in iter {
                    if let Some(key) = key_val.unpack_str() {
                        if let Ok(val) = make_variables.at(key_val, heap) {
                            if let Some(val_str) = val.unpack_str() {
                                let pattern = format!("$({})", key);
                                resolved_command = resolved_command.replace(&pattern, val_str);
                            }
                        }
                    }
                }
            }
        }

        let inputs = heap.alloc(AllocList(tool_files));
        let cmd_list = heap.alloc(AllocList(vec![
            heap.alloc_str(&resolved_command).to_value(),
        ]));
        let manifests = heap.alloc(AllocList::EMPTY);
        Ok(heap.alloc((inputs, cmd_list, manifests)))
    }

    /// Creates a new file (deprecated Bazel API).
    ///
    /// This is equivalent to `ctx.actions.declare_file()`. The deprecated form is
    /// `ctx.new_file(filename)` or `ctx.new_file(sibling, filename)`.
    #[allow(unused_variables)]
    fn new_file<'v>(
        this: RefAnalysisContext<'v>,
        #[starlark(require = pos)] file_or_sibling: Value<'v>,
        #[starlark(require = pos, default = NoneType)] filename: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        use kuro_core::fs::buck_out_path::BuckOutPathKind;
        use kuro_execute::execute::request::OutputType;

        use crate::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
        use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;

        // If filename is provided, file_or_sibling is a sibling File
        // Otherwise, file_or_sibling is the filename string
        let name = if filename.is_none() {
            file_or_sibling
                .unpack_str()
                .unwrap_or(&file_or_sibling.to_str())
                .to_owned()
        } else {
            filename
                .unpack_str()
                .unwrap_or(&filename.to_str())
                .to_owned()
        };

        // Delegate to ctx.actions.declare_file via the registry
        let to_err = |e: kuro_error::Error| -> starlark::Error {
            starlark::Error::new_other(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        };
        let mut state = this.0.actions.state().map_err(to_err)?;
        let artifact = state
            .declare_output(
                None,
                &name,
                OutputType::File,
                None,
                BuckOutPathKind::default(),
                heap,
            )
            .map_err(to_err)?;
        Ok(heap.alloc(StarlarkDeclaredArtifact::new(
            None,
            artifact,
            AssociatedArtifacts::new(),
        )))
    }

    /// Expands `$(location label)` and `$(locations label)` templates in the input string.
    ///
    /// In Bazel, rules can use these templates to embed file paths of targets into strings.
    /// This is used in genrule commands, args to cc_binary, etc.
    ///
    /// - `$(location :target)` expands to the path of the first default output of :target
    /// - `$(locations :target)` expands to space-separated paths of all default outputs
    ///
    /// The `targets` parameter is a list of Dependency objects (from ctx.attr.* values)
    /// that provide the pool of targets to look up. Labels are matched by their short form.
    #[allow(unused_variables)]
    fn expand_location<'v>(
        this: RefAnalysisContext<'v>,
        input: &str,
        #[starlark(default = NoneType)] targets: Value<'v>,
        #[starlark(require = named, default = false)] short_paths: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<String> {
        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

        // Build a mapping from target label suffixes (short names) to their output paths.
        // We match both the full label form and short forms (":name", "name", "//pkg:name").
        let mut label_to_paths: Vec<(String, Vec<String>)> = vec![];

        // Helper to collect output paths from a FrozenProviderCollection
        let collect_output_paths = |pc: starlark::values::FrozenValueTyped<
            '_,
            crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection,
        >|
         -> Vec<String> {
            if let Ok(di) = pc.as_ref().default_info() {
                di.default_outputs()
                    .into_iter()
                    .map(|art| {
                        art.artifact()
                            .get_path()
                            .with_full_path(|p| p.as_str().to_owned())
                    })
                    .collect()
            } else {
                vec![]
            }
        };

        if let Ok(iter) = targets.iterate(heap) {
            for dep_val in iter {
                let (dep_label, paths) = if let Some(dep) = dep_val.downcast_ref::<Dependency>() {
                    // Use unconfigured label for matching (avoids " (<cfg>)" suffix in comparison)
                    let label_str = dep.label().label().unconfigured().to_string();
                    let paths = collect_output_paths(dep.provider_collection());
                    (label_str, paths)
                } else if let Some(dep) = dep_val.downcast_ref::<FrozenDependency>() {
                    let label_str = dep.label().label().unconfigured().to_string();
                    let paths = collect_output_paths(dep.provider_collection());
                    (label_str, paths)
                } else {
                    continue;
                };
                label_to_paths.push((dep_label, paths));
            }
        }

        // Helper: find paths for a label string from the targets list
        let find_paths = |label: &str| -> Option<Vec<String>> {
            // Try exact match, then suffix match on ":name" part
            for (dep_label, paths) in &label_to_paths {
                // Exact match
                if dep_label == label {
                    return Some(paths.clone());
                }
                // Match on the target name part (after the last ':')
                let dep_name = dep_label.rsplit(':').next().unwrap_or(dep_label.as_str());
                let query_name = label.trim_start_matches(':');
                if dep_name == query_name {
                    return Some(paths.clone());
                }
            }
            None
        };

        // Expand $(location label), $(locations label), $(execpath label), $(execpaths label),
        // $(rootpath label), $(rootpaths label), $(rlocationpath label), $(rlocationpaths label).
        // In Kuro, all variants resolve to the same artifact path.
        let mut result = String::with_capacity(input.len());
        let mut remaining = input;
        while let Some(start) = remaining.find("$(") {
            result.push_str(&remaining[..start]);
            remaining = &remaining[start..];

            // Try each keyword variant: (prefix, is_multi)
            let pattern: Option<(usize, bool)> = if remaining.starts_with("$(locations ") {
                Some(("$(locations ".len(), true))
            } else if remaining.starts_with("$(location ") {
                Some(("$(location ".len(), false))
            } else if remaining.starts_with("$(execpaths ") {
                Some(("$(execpaths ".len(), true))
            } else if remaining.starts_with("$(execpath ") {
                Some(("$(execpath ".len(), false))
            } else if remaining.starts_with("$(rootpaths ") {
                Some(("$(rootpaths ".len(), true))
            } else if remaining.starts_with("$(rootpath ") {
                Some(("$(rootpath ".len(), false))
            } else if remaining.starts_with("$(rlocationpaths ") {
                Some(("$(rlocationpaths ".len(), true))
            } else if remaining.starts_with("$(rlocationpath ") {
                Some(("$(rlocationpath ".len(), false))
            } else {
                None
            };

            if let Some((prefix_len, is_multi)) = pattern {
                if let Some(end) = remaining.find(')') {
                    let label = remaining[prefix_len..end].trim();
                    let after = &remaining[end + 1..];

                    if let Some(paths) = find_paths(label) {
                        if is_multi {
                            result.push_str(&paths.join(" "));
                        } else {
                            result.push_str(paths.first().map(|s| s.as_str()).unwrap_or(""));
                        }
                    } else {
                        // Label not found - leave the template as-is (or error)
                        result.push_str(&remaining[..end + 1]);
                    }
                    remaining = after;
                    continue;
                }
            }

            // Not a location template - advance past "$("
            result.push_str("$(");
            remaining = &remaining[2..];
        }
        result.push_str(remaining);

        Ok(result)
    }
}

/// Standalone helper that expands `$(location ...)` patterns in a command string.
///
/// Used by both `ctx.expand_location()` and `ctx.resolve_command()`.
fn expand_location_in_string<'v>(
    input: &str,
    targets: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<String> {
    use crate::interpreter::rule_defs::provider::dependency::Dependency;
    use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

    let mut label_to_paths: Vec<(String, Vec<String>)> = vec![];

    let collect_output_paths = |pc: starlark::values::FrozenValueTyped<
        '_,
        crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection,
    >|
     -> Vec<String> {
        if let Ok(di) = pc.as_ref().default_info() {
            di.default_outputs()
                .into_iter()
                .map(|art| {
                    art.artifact()
                        .get_path()
                        .with_full_path(|p| p.as_str().to_owned())
                })
                .collect()
        } else {
            vec![]
        }
    };

    if let Ok(iter) = targets.iterate(heap) {
        for dep_val in iter {
            let (dep_label, paths) = if let Some(dep) = dep_val.downcast_ref::<Dependency>() {
                let label_str = dep.label().label().unconfigured().to_string();
                let paths = collect_output_paths(dep.provider_collection());
                (label_str, paths)
            } else if let Some(dep) = dep_val.downcast_ref::<FrozenDependency>() {
                let label_str = dep.label().label().unconfigured().to_string();
                let paths = collect_output_paths(dep.provider_collection());
                (label_str, paths)
            } else {
                continue;
            };
            label_to_paths.push((dep_label, paths));
        }
    }

    let find_paths = |label: &str| -> Option<Vec<String>> {
        for (dep_label, paths) in &label_to_paths {
            if dep_label == label {
                return Some(paths.clone());
            }
            let dep_name = dep_label.rsplit(':').next().unwrap_or(dep_label.as_str());
            let query_name = label.trim_start_matches(':');
            if dep_name == query_name {
                return Some(paths.clone());
            }
        }
        None
    };

    let mut result = String::with_capacity(input.len());
    let mut remaining = input;
    while let Some(start) = remaining.find("$(") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];

        let pattern: Option<(usize, bool)> = if remaining.starts_with("$(locations ") {
            Some(("$(locations ".len(), true))
        } else if remaining.starts_with("$(location ") {
            Some(("$(location ".len(), false))
        } else if remaining.starts_with("$(execpaths ") {
            Some(("$(execpaths ".len(), true))
        } else if remaining.starts_with("$(execpath ") {
            Some(("$(execpath ".len(), false))
        } else if remaining.starts_with("$(rootpaths ") {
            Some(("$(rootpaths ".len(), true))
        } else if remaining.starts_with("$(rootpath ") {
            Some(("$(rootpath ".len(), false))
        } else if remaining.starts_with("$(rlocationpaths ") {
            Some(("$(rlocationpaths ".len(), true))
        } else if remaining.starts_with("$(rlocationpath ") {
            Some(("$(rlocationpath ".len(), false))
        } else {
            None
        };

        if let Some((prefix_len, is_multi)) = pattern {
            if let Some(end) = remaining.find(')') {
                let label = remaining[prefix_len..end].trim();
                let after = &remaining[end + 1..];

                if let Some(paths) = find_paths(label) {
                    if is_multi {
                        result.push_str(&paths.join(" "));
                    } else {
                        result.push_str(paths.first().map(|s| s.as_str()).unwrap_or(""));
                    }
                } else {
                    result.push_str(&remaining[..end + 1]);
                }
                remaining = after;
                continue;
            }
        }

        result.push_str("$(");
        remaining = &remaining[2..];
    }
    result.push_str(remaining);

    Ok(result)
}

/// Derives the output directory path from a configured target label.
///
/// In Kuro, the output path is `buck-out/v2/gen/<cell>/<cfg_hash>`.
/// Falls back to `buck-out/v2/gen` if no label is available.
pub fn bin_dir_path_from_label(
    label: Option<
        starlark::values::ValueTyped<
            '_,
            kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel,
        >,
    >,
) -> String {
    if let Some(label) = label {
        let target = label.label().target();
        let cell_name = target.pkg().cell_name().as_str();
        let cfg_hash = label.label().cfg().output_hash().as_str();
        format!("buck-out/v2/gen/{}/{}", cell_name, cfg_hash)
    } else {
        "buck-out/v2/gen".to_owned()
    }
}

/// Returns the Bazel CPU identifier for the host platform.
/// See https://bazel.build/concepts/platforms#cpu
pub fn host_target_cpu() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "k8",
        ("linux", "aarch64") => "aarch64",
        ("macos", "x86_64") => "darwin_x86_64",
        ("macos", "aarch64") => "darwin_arm64",
        ("windows", "x86_64") => "x64_windows",
        ("windows", "aarch64") => "arm64_windows",
        _ => "k8",
    }
}

/// Returns the default C compiler path for the host platform.
pub fn host_cc_path() -> &'static str {
    match std::env::consts::OS {
        "windows" => "cl.exe",
        "macos" => "/usr/bin/clang",
        _ => "/usr/bin/gcc",
    }
}

fn collect_runfiles_from_value<'v>(
    value: Value<'v>,
    want_data: bool,
    heap: Heap<'v>,
    runfiles: &mut Value<'v>,
) -> starlark::Result<()> {
    use starlark::values::list::ListRef;

    use crate::interpreter::rule_defs::provider::builtin::default_info::merge_runfiles_values;
    use crate::interpreter::rule_defs::provider::dependency::Dependency;
    use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

    if value.is_none() {
        return Ok(());
    }

    if let Some(list) = ListRef::from_value(value) {
        for item in list.iter() {
            collect_runfiles_from_value(item, want_data, heap, runfiles)?;
        }
        return Ok(());
    }

    let pc = if let Some(dep) = value.downcast_ref::<Dependency>() {
        Some(dep.provider_collection())
    } else if let Some(dep) = value.downcast_ref::<FrozenDependency>() {
        Some(dep.provider_collection())
    } else {
        None
    };

    if let Some(pc) = pc {
        if let Ok(di) = pc.default_info() {
            let dep_runfiles = if want_data {
                di.data_runfiles_raw().to_value()
            } else {
                di.default_runfiles_raw().to_value()
            };
            *runfiles = merge_runfiles_values(heap, *runfiles, dep_runfiles)?;
        }
    }

    Ok(())
}

// ============================================================================
// CtxOutputs - Provides output files declared via attr.output()
// ============================================================================

/// Holds output files for ctx.outputs, supporting both `attr.output()` and `rule(outputs={...})`.
///
/// In Bazel:
/// - `attr.output()` attributes store filename strings in `ctx.attrs.<name>`. Accessing
///   `ctx.outputs.<name>` declares a file with that name.
/// - `rule(outputs={...})` defines pattern-based outputs like `"%{name}.stripped"`. These
///   are expanded using the target name and made available via `ctx.outputs.<name>`.
///
/// This implementation handles both cases:
/// 1. Dynamic: look up `ctx.attrs.<name>` as a string and declare a file with that filename
/// 2. Pattern fallback: for well-known `rule(outputs={...})` patterns, generate from target name
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxOutputs<'v> {
    /// The attrs struct - used to look up attr.output() string values
    attrs: Value<'v>,
    /// Analysis actions - used to declare artifacts on demand
    actions: ValueTyped<'v, AnalysisActions<'v>>,
    /// Cache of already-declared artifacts: attr_name → StarlarkDeclaredArtifact
    declared: RefCell<SmallMap<String, Value<'v>>>,
    /// Target name for pattern-based output expansion (e.g., "hello_bin" → "hello_bin.stripped")
    target_name: String,
    /// Implicit output patterns from `rule(outputs={...})`.
    /// Each pair is (name, pattern) where `%{name}` is substituted with `target_name`.
    rule_outputs: Vec<(String, String)>,
}

impl<'v> std::fmt::Display for CtxOutputs<'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.outputs>")
    }
}

impl<'v> AllocValue<'v> for CtxOutputs<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

#[starlark::values::starlark_value(type = "ctx_outputs")]
impl<'v> StarlarkValue<'v> for CtxOutputs<'v> {
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        // Check cache
        if self.declared.borrow().contains_key(attribute) {
            return true;
        }
        // Check if attrs has a string or list value for this attribute
        // (i.e. it's an attr.output() or attr.output_list())
        if let Ok(Some(v)) = self.attrs.get_attr(attribute, heap) {
            if v.unpack_str().is_some() {
                return true;
            }
            if starlark::values::list::ListRef::from_value(v).is_some() {
                return true;
            }
        }
        // Check rule(outputs={...}) pattern names from the rule definition
        if self.rule_outputs.iter().any(|(k, _)| k == attribute) {
            return true;
        }
        // Check well-known fallback pattern names
        matches!(attribute, "stripped_binary" | "dwp_file" | "executable")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // Check cache first
        if let Some(v) = self.declared.borrow().get(attribute) {
            return Some(*v);
        }

        use kuro_core::fs::buck_out_path::BuckOutPathKind;
        use kuro_execute::execute::request::OutputType;

        use crate::interpreter::rule_defs::artifact::associated::AssociatedArtifacts;
        use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;

        let declare_file = |filename: &str| -> Option<Value<'v>> {
            if filename.is_empty() {
                return None;
            }
            let artifact = self
                .actions
                .state()
                .ok()?
                .declare_output(
                    None,
                    filename,
                    OutputType::File,
                    None,
                    BuckOutPathKind::default(),
                    heap,
                )
                .ok()?;
            Some(heap.alloc(StarlarkDeclaredArtifact::new(
                None,
                artifact,
                AssociatedArtifacts::new(),
            )))
        };

        // Check if the attr is a list (attr.output_list()) - returns a list of declared artifacts
        let raw_attr_val = self.attrs.get_attr(attribute, heap).ok().flatten();
        if let Some(list_ref) = raw_attr_val
            .as_ref()
            .and_then(|v| starlark::values::list::ListRef::from_value(*v))
        {
            let filenames: Vec<String> = list_ref
                .iter()
                .filter_map(|elem| elem.unpack_str().map(|s| s.to_owned()))
                .collect();
            if !filenames.is_empty() || list_ref.is_empty() {
                let artifacts: Vec<Value<'v>> = filenames
                    .iter()
                    .filter_map(|f| declare_file(f.as_str()))
                    .collect();
                let result = heap.alloc(artifacts);
                self.declared
                    .borrow_mut()
                    .insert(attribute.to_owned(), result);
                return Some(result);
            }
        }

        // Determine the filename for this output attribute.
        // Case 1: attr.output() - the attribute value in attrs is the filename string
        // Case 2: rule(outputs={...}) pattern - expand using target name
        // Case 3: well-known fallback patterns
        let filename_owned: String;
        let filename: &str = if let Some(v) =
            raw_attr_val.and_then(|v| v.unpack_str().map(|s| s.to_owned()))
        {
            filename_owned = v;
            &filename_owned
        } else if let Some((_, pattern)) = self.rule_outputs.iter().find(|(k, _)| k == attribute) {
            // Expand %{name} substitution pattern from rule(outputs={...})
            filename_owned = pattern.replace("%{name}", &self.target_name);
            &filename_owned
        } else {
            // Fallback: expand well-known patterns using target name.
            filename_owned = match attribute {
                "stripped_binary" => format!("{}.stripped", self.target_name),
                "dwp_file" => format!("{}.dwp", self.target_name),
                "executable" => self.target_name.clone(),
                _ => return None,
            };
            &filename_owned
        };

        if filename.is_empty() {
            return None;
        }

        let val = declare_file(filename)?;
        self.declared.borrow_mut().insert(attribute.to_owned(), val);
        Some(val)
    }

    fn dir_attr(&self) -> Vec<String> {
        // Return currently cached outputs; we can't enumerate all possible attr.output() names
        self.declared.borrow().keys().map(|k| k.clone()).collect()
    }
}

// ============================================================================
// ResolvedToolchains - Real toolchain resolution results for ctx.toolchains
// ============================================================================

/// Real resolved toolchains from the toolchain resolution algorithm.
///
/// Real resolved toolchains from the toolchain resolution algorithm.
/// Maps toolchain type labels to their resolved `ToolchainInfo` provider
/// values (or None for optional unmatched types).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ResolvedToolchains {
    /// Map from toolchain_type label → resolved provider collection.
    /// Value is None for toolchain types that resolved but whose impl target
    /// could not be analyzed (errors on access).
    pub toolchains: std::collections::HashMap<String, Option<FrozenProviderCollectionValue>>,
    /// The selected execution platform label.
    pub exec_platform: String,
}

impl std::fmt::Display for ResolvedToolchains {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<resolved_toolchains({} types)>", self.toolchains.len())
    }
}

starlark::starlark_simple_value!(ResolvedToolchains);

#[starlark::values::starlark_value(type = "resolved_toolchains")]
impl<'v> StarlarkValue<'v> for ResolvedToolchains {
    /// Check if a toolchain type is available.
    /// Returns true for any type. Real Bazel would only return true for types
    /// that resolved, but many rules rely on the check always succeeding.
    fn is_in(&self, _other: Value<'v>) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Index by toolchain type to get the ToolchainInfo provider.
    /// If a real provider collection was resolved via DICE analysis, returns it.
    /// Otherwise, returns an error for unresolved toolchain types.
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let key = if let Some(s) = index.unpack_str() {
            s.to_owned()
        } else {
            format!("{}", index)
        };
        let normalized = normalize_toolchain_type_label(&key);

        // Look for a resolved provider collection (exact match first, then normalized)
        let provider_collection = self
            .toolchains
            .get(&normalized)
            .or_else(|| {
                self.toolchains
                    .iter()
                    .find(|(k, _)| normalize_toolchain_type_label(k) == normalized)
                    .map(|(_, v)| v)
            })
            .and_then(|opt| opt.as_ref());

        if let Some(providers) = provider_collection {
            // In Bazel, ctx.toolchains[TYPE] returns the ToolchainInfo provider
            // (platform_common.ToolchainInfo), NOT the entire provider collection.
            // Look for ToolchainInfo in the resolved provider collection.
            let toolchain_info_id =
                crate::interpreter::rule_defs::platform_common::ToolchainInfoProvider::provider_id(
                );
            if let Some(fv) = providers
                .provider_collection()
                .get_provider_raw(toolchain_info_id)
            {
                return Ok(fv.to_value());
            }

            // If no ToolchainInfo found, return the full collection as fallback.
            let fv = unsafe { providers.value().to_frozen_value() };
            return Ok(fv.to_value());
        }

        // No resolved provider for this toolchain type.
        // Return None for exec-group toolchains (empty map = no per-group resolution),
        // which lets rules fall back to their legacy code paths.
        if self.toolchains.is_empty() {
            return Ok(Value::new_none());
        }

        Err(starlark::Error::new_other(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "Toolchain type '{}' was not resolved. Ensure the toolchain is registered \
                 via register_toolchains() and the rule declares it in toolchains=[...]",
                key
            ),
        )))
    }
}

/// Normalize a toolchain type label for matching.
/// Strips @@ prefix, handles common variations.
fn normalize_toolchain_type_label(label: &str) -> String {
    let label = label.trim_start_matches('@');
    label.to_owned()
}

// ============================================================================
// CompilationContext / LinkingContext stubs for CcInfo
// ============================================================================

/// A stub for CompilationContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct EmptyCompilationContext;

impl std::fmt::Display for EmptyCompilationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CompilationContext>")
    }
}

starlark::starlark_simple_value!(EmptyCompilationContext);

#[starlark::values::starlark_value(type = "CompilationContext")]
impl<'v> StarlarkValue<'v> for EmptyCompilationContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "_module_map"
                | "headers"
                | "system_includes"
                | "includes"
                | "quote_includes"
                | "defines"
                | "local_defines"
                | "framework_includes"
                | "_exporting_module_maps"
                | "direct_headers"
                | "direct_public_headers"
                | "direct_private_headers"
                | "direct_textual_headers"
                | "_header_info"
                | "external_includes"
                | "_non_code_inputs"
                | "_virtual_to_original_headers"
                | "validation_artifacts"
                | "_transitive_modules"
                | "_transitive_pic_modules"
                | "_modules_info_files"
                | "_pic_modules_info_files"
                | "_module_files"
                | "_pic_module_files"
                | "_direct_module_maps"
                | "_exporting_module_map_files"
                | "loose_hdrs_dirs"
                | "purpose"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "_module_map" => Some(Value::new_none()),
            "headers" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "system_includes" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "includes" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "quote_includes" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "defines" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "local_defines" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "framework_includes" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "external_includes" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_non_code_inputs" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_virtual_to_original_headers" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "validation_artifacts" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_transitive_modules" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_transitive_pic_modules" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_modules_info_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_pic_modules_info_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_module_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_pic_module_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_direct_module_maps" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_exporting_module_maps" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_public_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_private_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "direct_textual_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "_header_info" => Some(heap.alloc(EmptyHeaderInfo)),
            "_exporting_module_map_files" | "loose_hdrs_dirs" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "purpose" => Some(Value::new_none()),
            _ => None,
        }
    }
}

/// A stub for HeaderInfo for CompilationContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct EmptyHeaderInfo;

impl std::fmt::Display for EmptyHeaderInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<HeaderInfo>")
    }
}

starlark::starlark_simple_value!(EmptyHeaderInfo);

#[starlark::values::starlark_value(type = "HeaderInfo")]
impl<'v> StarlarkValue<'v> for EmptyHeaderInfo {
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
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
            | "separate_module_headers" => Some(heap.alloc(Vec::<Value>::new())),
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                Some(Value::new_none())
            }
            _ => None,
        }
    }
}

/// A stub for LinkingContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct EmptyLinkingContext;

impl std::fmt::Display for EmptyLinkingContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<LinkingContext>")
    }
}

starlark::starlark_simple_value!(EmptyLinkingContext);

#[starlark::values::starlark_value(type = "LinkingContext")]
impl<'v> StarlarkValue<'v> for EmptyLinkingContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "linker_inputs" | "_extra_link_time_libraries")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "linker_inputs" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_extra_link_time_libraries" => Some(Value::new_none()),
            _ => None,
        }
    }
}

// ============================================================================
// CtxFiles / CtxFile / CtxExecutable — ctx.files, ctx.file, ctx.executable
// ============================================================================

/// Provides access to files from label/label_list attributes as ctx.files.<attr>.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxFiles<'v> {
    attrs: Value<'v>,
}

impl<'v> CtxFiles<'v> {
    pub fn new(attrs: ValueOfUnchecked<'v, StructRef<'static>>) -> Self {
        Self {
            attrs: attrs.get().to_value(),
        }
    }
}

impl<'v> std::fmt::Display for CtxFiles<'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.files>")
    }
}

#[starlark::values::starlark_value(type = "ctx_files")]
impl<'v> StarlarkValue<'v> for CtxFiles<'v> {
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        self.attrs.has_attr(attribute, heap)
    }

    fn dir_attr(&self) -> Vec<String> {
        self.attrs.dir_attr()
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::AllocList;
        use starlark::values::list::ListRef;

        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;

        if attr_value.is_none() {
            return Some(heap.alloc(AllocList::EMPTY));
        }

        let extract_files = |v: Value<'v>, files: &mut Vec<Value<'v>>| {
            let pc = if let Some(dep) = v.downcast_ref::<Dependency>() {
                Some(dep.provider_collection())
            } else if let Some(dep) = v.downcast_ref::<FrozenDependency>() {
                Some(dep.provider_collection())
            } else {
                None
            };
            if let Some(pc) = pc {
                if let Ok(di) = pc.default_info() {
                    let raw = di.default_outputs_raw();
                    if let Some(list) = ListRef::from_frozen_value(raw) {
                        for item in list.iter() {
                            files.push(item);
                        }
                    }
                }
            } else {
                files.push(v);
            }
        };

        let result = if let Some(list) = ListRef::from_value(attr_value) {
            let mut files = Vec::new();
            for item in list.iter() {
                extract_files(item, &mut files);
            }
            Some(heap.alloc(files))
        } else {
            let mut files = Vec::new();
            extract_files(attr_value, &mut files);
            Some(heap.alloc(files))
        };
        result
    }
}

impl<'v> AllocValue<'v> for CtxFiles<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Provides access to single files from label attributes as ctx.file.<attr>.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxFile<'v> {
    attrs: Value<'v>,
}

impl<'v> CtxFile<'v> {
    pub fn new(attrs: ValueOfUnchecked<'v, StructRef<'static>>) -> Self {
        Self {
            attrs: attrs.get().to_value(),
        }
    }
}

impl<'v> std::fmt::Display for CtxFile<'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.file>")
    }
}

#[starlark::values::starlark_value(type = "ctx_file")]
impl<'v> StarlarkValue<'v> for CtxFile<'v> {
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        self.attrs.has_attr(attribute, heap)
    }

    fn dir_attr(&self) -> Vec<String> {
        self.attrs.dir_attr()
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::ListRef;

        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;

        if attr_value.is_none() {
            return Some(Value::new_none());
        }

        let extract_first_file = |v: Value<'v>| -> Value<'v> {
            let pc = if let Some(dep) = v.downcast_ref::<Dependency>() {
                Some(dep.provider_collection())
            } else if let Some(dep) = v.downcast_ref::<FrozenDependency>() {
                Some(dep.provider_collection())
            } else {
                None
            };
            if let Some(pc) = pc {
                if let Ok(di) = pc.default_info() {
                    let raw = di.default_outputs_raw();
                    if let Some(list) = ListRef::from_frozen_value(raw) {
                        if !list.is_empty() {
                            return list.content()[0];
                        }
                    }
                }
                Value::new_none()
            } else {
                v
            }
        };

        if let Some(list) = ListRef::from_value(attr_value) {
            if list.is_empty() {
                return Some(Value::new_none());
            }
            return Some(extract_first_file(list.content()[0]));
        }

        Some(extract_first_file(attr_value))
    }
}

impl<'v> AllocValue<'v> for CtxFile<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Provides access to executable files from label attributes as ctx.executable.<attr>.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxExecutable<'v> {
    attrs: Value<'v>,
}

impl<'v> CtxExecutable<'v> {
    pub fn new(attrs: ValueOfUnchecked<'v, StructRef<'static>>) -> Self {
        Self {
            attrs: attrs.get().to_value(),
        }
    }
}

impl<'v> std::fmt::Display for CtxExecutable<'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.executable>")
    }
}

#[starlark::values::starlark_value(type = "ctx_executable")]
impl<'v> StarlarkValue<'v> for CtxExecutable<'v> {
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        self.attrs.has_attr(attribute, heap)
    }

    fn dir_attr(&self) -> Vec<String> {
        self.attrs.dir_attr()
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::ListRef;

        use crate::interpreter::rule_defs::provider::dependency::Dependency;
        use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;

        if attr_value.is_none() {
            return Some(Value::new_none());
        }

        let extract_executable = |v: Value<'v>| -> Option<Value<'v>> {
            let pc = if let Some(dep) = v.downcast_ref::<Dependency>() {
                Some(dep.provider_collection())
            } else if let Some(dep) = v.downcast_ref::<FrozenDependency>() {
                Some(dep.provider_collection())
            } else {
                None
            };
            if let Some(pc) = pc {
                if let Ok(di) = pc.default_info() {
                    if let Some(exe) = di.executable() {
                        return Some(heap.alloc(exe));
                    }
                    let raw = di.default_outputs_raw();
                    if let Some(list) = ListRef::from_frozen_value(raw) {
                        if !list.is_empty() {
                            return Some(list.content()[0]);
                        }
                    }
                }
                return Some(Value::new_none());
            }
            Some(v)
        };

        if let Some(list) = ListRef::from_value(attr_value) {
            if list.is_empty() {
                return Some(Value::new_none());
            }
            return extract_executable(list.content()[0]);
        }

        extract_executable(attr_value)
    }
}

impl<'v> AllocValue<'v> for CtxExecutable<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// A stub for ctx.files that returns empty lists for all attributes.
/// Used as fallback when attrs is not available (e.g., dynamic_output, BXL).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxFilesUnavailable;

impl std::fmt::Display for CtxFilesUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.files>")
    }
}

starlark::starlark_simple_value!(CtxFilesUnavailable);

#[starlark::values::starlark_value(type = "ctx_files")]
impl<'v> StarlarkValue<'v> for CtxFilesUnavailable {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::AllocList;
        Some(heap.alloc(AllocList::EMPTY))
    }
}

// ============================================================================
// CtxSplitAttr - Wraps attribute values in single-entry config dicts
// ============================================================================

/// Provides access to split-configuration attributes as `ctx.split_attr.<name>`.
///
/// In Bazel, when an attribute uses `cfg = some_transition`, `ctx.split_attr.<name>`
/// returns a dict mapping configuration keys to the attribute values in those configs.
/// Since Kuro doesn't implement split transitions, each value is wrapped in
/// `{"//conditions:default": <original_value>}`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxSplitAttr<'v> {
    attrs: Value<'v>,
}

impl<'v> CtxSplitAttr<'v> {
    pub fn new(attrs: ValueOfUnchecked<'v, StructRef<'static>>) -> Self {
        Self {
            attrs: attrs.get().to_value(),
        }
    }
}

impl<'v> std::fmt::Display for CtxSplitAttr<'v> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.split_attr>")
    }
}

impl<'v> AllocValue<'v> for CtxSplitAttr<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

#[starlark::values::starlark_value(type = "ctx_split_attr")]
impl<'v> StarlarkValue<'v> for CtxSplitAttr<'v> {
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        self.attrs.has_attr(attribute, heap)
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::AllocDict;

        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;
        // Wrap in {"//conditions:default": value}
        let key = heap.alloc_str("//conditions:default").to_value();
        Some(heap.alloc(AllocDict(vec![(key, attr_value)])))
    }

    fn dir_attr(&self) -> Vec<String> {
        self.attrs.dir_attr()
    }
}

/// A stub for ctx.split_attr when attrs are not available.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxSplitAttrUnavailable;

impl std::fmt::Display for CtxSplitAttrUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.split_attr>")
    }
}

starlark::starlark_simple_value!(CtxSplitAttrUnavailable);

#[starlark::values::starlark_value(type = "ctx_split_attr_stub")]
impl<'v> StarlarkValue<'v> for CtxSplitAttrUnavailable {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::AllocDict;
        let key = heap.alloc_str("//conditions:default").to_value();
        Some(heap.alloc(AllocDict(vec![(key, Value::new_none())])))
    }
}

/// A stub for ctx.file that returns None for all attributes.
/// Used as fallback when attrs is not available.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxFileUnavailable;

impl std::fmt::Display for CtxFileUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.file>")
    }
}

starlark::starlark_simple_value!(CtxFileUnavailable);

#[starlark::values::starlark_value(type = "ctx_file")]
impl<'v> StarlarkValue<'v> for CtxFileUnavailable {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        Some(Value::new_none())
    }
}

/// A stub for ctx.executable that returns None for all attributes.
/// Used as fallback when attrs is not available.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxExecutableUnavailable;

impl std::fmt::Display for CtxExecutableUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.executable>")
    }
}

starlark::starlark_simple_value!(CtxExecutableUnavailable);

#[starlark::values::starlark_value(type = "ctx_executable")]
impl<'v> StarlarkValue<'v> for CtxExecutableUnavailable {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, _attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        Some(Value::new_none())
    }
}

/// Output directory root for ctx.bin_dir and ctx.genfiles_dir (Bazel-compatible).
///
/// In Bazel, `ctx.bin_dir` and `ctx.genfiles_dir` are root objects that provide
/// a `path` attribute containing the directory path string.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxDirRoot {
    pub path: String,
}

impl std::fmt::Display for CtxDirRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<root: {}>", self.path)
    }
}

starlark::starlark_simple_value!(CtxDirRoot);

#[starlark::values::starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxDirRoot {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_dir_root_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "path")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_dir_root_methods(builder: &mut MethodsBuilder) {
    /// The path to this output directory.
    #[starlark(attribute)]
    fn path<'v>(this: &CtxDirRoot, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc_str(&this.path).to_value())
    }
}

/// A dict-like object for ctx.var providing access to Make variables (Bazel-compatible).
///
/// In Bazel, `ctx.var` is a dictionary containing Make variables that can be
/// accessed using `ctx.var["VAR_NAME"]` or `ctx.var.get("VAR_NAME")`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxVarDict;

impl std::fmt::Display for CtxVarDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.var>")
    }
}

starlark::starlark_simple_value!(CtxVarDict);

#[starlark::values::starlark_value(type = "dict")]
impl<'v> StarlarkValue<'v> for CtxVarDict {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_var_dict_methods)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let key = index.unpack_str().unwrap_or("");
        let cpu = host_target_cpu();
        let cc = host_cc_path();
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        // Return common Make variables
        let value: String = match key {
            "BINDIR" => format!("bazel-out/{cpu}-{comp_mode}/bin"),
            "GENDIR" => format!("bazel-out/{cpu}-{comp_mode}/genfiles"),
            "TARGET_CPU" => cpu.to_owned(),
            "COMPILATION_MODE" => comp_mode,
            "CC" => cc.to_owned(),
            "CC_FLAGS" => String::new(),
            "JAVA" => if cfg!(windows) {
                "java.exe"
            } else {
                "/usr/bin/java"
            }
            .to_owned(),
            "JAVA_RUNFILES" => String::new(),
            "JAVABASE" => String::new(),
            "ABI_GLIBC_VERSION" => "2.17".to_owned(),
            "ABI" => "local".to_owned(),
            _ => {
                // Check --define values for unknown keys
                crate::interpreter::rule_defs::build_config::get_define(key).unwrap_or_default()
            }
        };
        Ok(heap.alloc_str(&value).to_value())
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // Return true for known Make variables or --define keys
        if let Some(key) = other.unpack_str() {
            if crate::interpreter::rule_defs::build_config::get_define(key).is_some() {
                return Ok(true);
            }
            Ok(matches!(
                key,
                "BINDIR"
                    | "GENDIR"
                    | "TARGET_CPU"
                    | "COMPILATION_MODE"
                    | "CC"
                    | "CC_FLAGS"
                    | "JAVA"
                    | "JAVA_RUNFILES"
                    | "JAVABASE"
                    | "ABI_GLIBC_VERSION"
                    | "ABI"
            ))
        } else {
            Ok(false)
        }
    }
}

#[starlark_module]
fn ctx_var_dict_methods(builder: &mut MethodsBuilder) {
    /// Get a Make variable by name, with optional default.
    fn get<'v>(
        this: &CtxVarDict,
        #[starlark(require = pos)] key: &str,
        #[starlark(default = NoneType)] default: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        let cpu = host_target_cpu();
        let cc = host_cc_path();
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let value: Option<String> = match key {
            "BINDIR" => Some(format!("bazel-out/{cpu}-{comp_mode}/bin")),
            "GENDIR" => Some(format!("bazel-out/{cpu}-{comp_mode}/genfiles")),
            "TARGET_CPU" => Some(cpu.to_owned()),
            "COMPILATION_MODE" => Some(comp_mode),
            "CC" => Some(cc.to_owned()),
            "CC_FLAGS" => Some(String::new()),
            "JAVA" => Some(
                if cfg!(windows) {
                    "java.exe"
                } else {
                    "/usr/bin/java"
                }
                .to_owned(),
            ),
            "JAVA_RUNFILES" => Some(String::new()),
            "JAVABASE" => Some(String::new()),
            "ABI_GLIBC_VERSION" => Some("2.17".to_owned()),
            "ABI" => Some("local".to_owned()),
            _ => crate::interpreter::rule_defs::build_config::get_define(key),
        };
        match value {
            Some(v) => Ok(heap.alloc_str(&v).to_value()),
            None => Ok(default),
        }
    }

    /// Get all keys in the Make variables dict.
    fn keys<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let mut keys: Vec<String> = vec![
            "BINDIR",
            "GENDIR",
            "TARGET_CPU",
            "COMPILATION_MODE",
            "CC",
            "CC_FLAGS",
            "JAVA",
            "JAVA_RUNFILES",
            "JAVABASE",
            "ABI_GLIBC_VERSION",
            "ABI",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        // Include --define keys
        for key in crate::interpreter::rule_defs::build_config::get_all_defines().keys() {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        let values: Vec<Value> = keys.iter().map(|k| heap.alloc_str(k).to_value()).collect();
        Ok(heap.alloc(values))
    }

    /// Get all values in the Make variables dict.
    fn values<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let cpu = host_target_cpu();
        let cc = host_cc_path();
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let mut result: Vec<Value> = vec![
            heap.alloc_str(&format!("bazel-out/{cpu}-{comp_mode}/bin"))
                .to_value(),
            heap.alloc_str(&format!("bazel-out/{cpu}-{comp_mode}/genfiles"))
                .to_value(),
            heap.alloc_str(cpu).to_value(),
            heap.alloc_str(&comp_mode).to_value(),
            heap.alloc_str(cc).to_value(),
            heap.alloc_str("").to_value(), // CC_FLAGS
            heap.alloc_str(if cfg!(windows) {
                "java.exe"
            } else {
                "/usr/bin/java"
            })
            .to_value(),
            heap.alloc_str("").to_value(),      // JAVA_RUNFILES
            heap.alloc_str("").to_value(),      // JAVABASE
            heap.alloc_str("2.17").to_value(),  // ABI_GLIBC_VERSION
            heap.alloc_str("local").to_value(), // ABI
        ];
        for (_, v) in crate::interpreter::rule_defs::build_config::get_all_defines() {
            result.push(heap.alloc_str(&v).to_value());
        }
        Ok(heap.alloc(result))
    }

    /// Get all key-value pairs as a list of tuples.
    fn items<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let cpu = host_target_cpu();
        let cc = host_cc_path();
        let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        let mut result: Vec<Value> = Vec::new();
        let entries: Vec<(&str, String)> = vec![
            ("BINDIR", format!("bazel-out/{cpu}-{comp_mode}/bin")),
            ("GENDIR", format!("bazel-out/{cpu}-{comp_mode}/genfiles")),
            ("TARGET_CPU", cpu.to_owned()),
            ("COMPILATION_MODE", comp_mode),
            ("CC", cc.to_owned()),
            ("CC_FLAGS", String::new()),
            (
                "JAVA",
                if cfg!(windows) {
                    "java.exe"
                } else {
                    "/usr/bin/java"
                }
                .to_owned(),
            ),
            ("JAVA_RUNFILES", String::new()),
            ("JAVABASE", String::new()),
            ("ABI_GLIBC_VERSION", "2.17".to_owned()),
            ("ABI", "local".to_owned()),
        ];
        for (k, v) in &entries {
            let tuple = heap.alloc((heap.alloc_str(k).to_value(), heap.alloc_str(v).to_value()));
            result.push(tuple);
        }
        for (k, v) in crate::interpreter::rule_defs::build_config::get_all_defines() {
            let tuple = heap.alloc((heap.alloc_str(&k).to_value(), heap.alloc_str(&v).to_value()));
            result.push(tuple);
        }
        Ok(heap.alloc(result))
    }
}

/// Bazel-compatible ctx.configuration object.
///
/// Provides build configuration attributes:
/// - `coverage_enabled`: Whether --collect_code_coverage is active
/// - `host_path_separator`: ":" on Unix, ";" on Windows
/// - `default_shell_env`: Dict from --action_env flags
/// - `stamp_binaries`: Whether build stamping is enabled
/// - `short_id`: Opaque configuration fingerprint (cpu-hash)
/// - `test_env`: Dict from --test_env flags
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BuildConfiguration {
    pub is_tool: bool,
    /// The configuration hash from ConfigurationData (16-char hex, e.g. "6770d7f2ebfc0845")
    pub config_hash: String,
    /// The full configuration label (e.g. "@local_config_platform//:host#6770d7f2ebfc0845")
    pub config_label: String,
}

impl std::fmt::Display for BuildConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<configuration>")
    }
}

starlark::starlark_simple_value!(BuildConfiguration);

#[starlark::values::starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for BuildConfiguration {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(build_configuration_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "coverage_enabled"
                | "host_path_separator"
                | "default_shell_env"
                | "stamp_binaries"
                | "short_id"
                | "test_env"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "coverage_enabled" => Some(Value::new_bool(
                crate::interpreter::rule_defs::build_config::get_collect_code_coverage(),
            )),
            "stamp_binaries" => Some(Value::new_bool(
                crate::interpreter::rule_defs::build_config::get_stamp(),
            )),
            "host_path_separator" => {
                let sep = if cfg!(windows) { ";" } else { ":" };
                Some(heap.alloc_str(sep).to_value())
            }
            "default_shell_env" => {
                // Return --action_env values from build config
                let env_map = crate::interpreter::rule_defs::build_config::get_action_env();
                let dict = starlark::values::dict::Dict::new(
                    env_map
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                heap.alloc_str(&k).to_value().get_hashed().unwrap(),
                                heap.alloc_str(&v).to_value(),
                            )
                        })
                        .collect(),
                );
                Some(heap.alloc(dict))
            }
            "short_id" => {
                // Opaque configuration fingerprint using real config hash
                if self.config_hash.is_empty() {
                    // Fallback for BXL/dynamic_output contexts without config data
                    let comp_mode =
                        crate::interpreter::rule_defs::build_config::get_compilation_mode();
                    let cpu = host_target_cpu();
                    Some(heap.alloc_str(&format!("{cpu}-{comp_mode}")).to_value())
                } else {
                    let cpu = host_target_cpu();
                    Some(
                        heap.alloc_str(&format!("{cpu}-{}", self.config_hash))
                            .to_value(),
                    )
                }
            }
            "test_env" => {
                // Return --test_env values from build config
                let env_map = crate::interpreter::rule_defs::build_config::get_test_env();
                let dict = starlark::values::dict::Dict::new(
                    env_map
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                heap.alloc_str(&k).to_value().get_hashed().unwrap(),
                                heap.alloc_str(&v).to_value(),
                            )
                        })
                        .collect(),
                );
                Some(heap.alloc(dict))
            }
            _ => None,
        }
    }
}

#[starlark_module]
fn build_configuration_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &BuildConfiguration) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// Returns whether this is a tool configuration (exec configuration).
    /// Tool configurations are used for build tools that run on the host machine.
    fn is_tool_configuration(this: &BuildConfiguration) -> starlark::Result<bool> {
        Ok(this.is_tool)
    }

    /// Returns whether this configuration has a separate genfiles directory.
    /// In modern Bazel, this is always false (genfiles merged with bin directory).
    fn has_separate_genfiles_directory(this: &BuildConfiguration) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// Returns whether build stamping is enabled.
    fn stamp_binaries(this: &BuildConfiguration) -> starlark::Result<bool> {
        let _ = this;
        Ok(crate::interpreter::rule_defs::build_config::get_stamp())
    }
}

// ============================================================================
// ResolvedExecGroups - ctx.exec_groups
// ============================================================================

/// Real exec group collection backed by per-group toolchain resolution.
///
/// `ctx.exec_groups["name"]` returns a `ResolvedExecGroupContext` for the named
/// exec group, which provides `.toolchains` for per-group toolchain access.
///
/// Groups that weren't declared in `rule(exec_groups={...})` produce an error
/// listing valid group names.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ResolvedExecGroups {
    /// Map of group name -> per-group resolved toolchains.
    pub groups: std::collections::HashMap<
        String,
        std::collections::HashMap<String, Option<FrozenProviderCollectionValue>>,
    >,
    /// Valid group names for error messages.
    pub valid_names: Vec<String>,
}

impl std::fmt::Display for ResolvedExecGroups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<exec_groups({} groups)>", self.groups.len())
    }
}

starlark::starlark_simple_value!(ResolvedExecGroups);

#[starlark::values::starlark_value(type = "exec_groups")]
impl<'v> StarlarkValue<'v> for ResolvedExecGroups {
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let key = match index.unpack_str() {
            Some(s) => s,
            None => {
                return Ok(heap.alloc(ResolvedExecGroupContext {
                    toolchains: ResolvedToolchains {
                        toolchains: std::collections::HashMap::new(),
                        exec_platform: String::new(),
                    },
                }));
            }
        };

        if let Some(toolchains) = self.groups.get(key) {
            Ok(heap.alloc(ResolvedExecGroupContext {
                toolchains: ResolvedToolchains {
                    toolchains: toolchains.clone(),
                    exec_platform: String::new(),
                },
            }))
        } else {
            // Fallback: return an empty exec group context for any key.
            // Many rules access exec groups that may not have been resolved yet,
            // so we return an empty context rather than erroring.
            Ok(heap.alloc(ResolvedExecGroupContext {
                toolchains: ResolvedToolchains {
                    toolchains: std::collections::HashMap::new(),
                    exec_platform: String::new(),
                },
            }))
        }
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        if let Some(key) = other.unpack_str() {
            Ok(self.groups.contains_key(key) || self.valid_names.contains(&key.to_owned()))
        } else {
            Ok(false)
        }
    }
}

/// A single resolved exec group, returned from `ctx.exec_groups["name"]`.
///
/// Exposes `.toolchains` attribute for per-group toolchain access.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ResolvedExecGroupContext {
    toolchains: ResolvedToolchains,
}

impl std::fmt::Display for ResolvedExecGroupContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<exec_group_context>")
    }
}

starlark::starlark_simple_value!(ResolvedExecGroupContext);

#[starlark::values::starlark_value(type = "exec_group_info")]
impl<'v> StarlarkValue<'v> for ResolvedExecGroupContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "toolchains" | "exec_compatible_with")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "toolchains" => Some(heap.alloc(ResolvedToolchains {
                toolchains: self.toolchains.toolchains.clone(),
                exec_platform: self.toolchains.exec_platform.clone(),
            })),
            "exec_compatible_with" => {
                use starlark::values::list::AllocList;
                Some(heap.alloc(AllocList::EMPTY))
            }
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["toolchains".to_owned(), "exec_compatible_with".to_owned()]
    }
}

/// A Bazel-compatible stamp file (stable-status.txt or volatile-status.txt).
///
/// In Bazel, `ctx.info_file` and `ctx.version_file` return File objects
/// representing build stamping status files. Rules add these as inputs
/// to actions when stamping is enabled (ctx.attr.stamp != 0).
///
/// This provides a File-like object with the correct type and attributes.
/// TODO(stamping): Generate real stamp file content during execution.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StampFile {
    /// Full path (e.g. "bazel-out/stable-status.txt")
    full_path: String,
    /// Short path / filename (e.g. "stable-status.txt")
    short_path: String,
}

impl std::fmt::Display for StampFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<stamp file {}>", self.short_path)
    }
}

starlark::starlark_simple_value!(StampFile);

// Use "File" type for Bazel compatibility
#[starlark::values::starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for StampFile {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "dirname"
                | "extension"
                | "is_source"
                | "is_directory"
                | "owner"
                | "root"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.full_path).to_value()),
            "short_path" => Some(heap.alloc_str(&self.short_path).to_value()),
            "basename" => {
                let basename = std::path::Path::new(&self.short_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&self.short_path);
                Some(heap.alloc_str(basename).to_value())
            }
            "dirname" => {
                let path_str = &self.full_path;
                match path_str.rfind('/') {
                    Some(pos) => Some(heap.alloc_str(&path_str[..pos]).to_value()),
                    None => Some(heap.alloc_str("").to_value()),
                }
            }
            "extension" => Some(heap.alloc_str("txt").to_value()),
            "is_source" => Some(Value::new_bool(false)),
            "is_directory" => Some(Value::new_bool(false)),
            "owner" => Some(Value::new_none()),
            "root" => {
                // Root is the prefix before the short_path
                let root_path = if let Some(prefix) = self.full_path.strip_suffix(&self.short_path)
                {
                    prefix.trim_end_matches('/').to_owned()
                } else {
                    "bazel-out".to_owned()
                };
                Some(heap.alloc(ArtifactRoot { path: root_path }))
            }
            _ => None,
        }
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

/// Shell-tokenize a string following Bourne shell rules.
///
/// Handles single-quoted strings (no escape), double-quoted strings (\\ and \"
/// escapes), and unquoted strings (whitespace is delimiter).
fn shell_tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut in_token = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                // Single-quoted: everything until closing quote, no escapes
                in_token = true;
                while let Some(c2) = chars.next() {
                    if c2 == '\'' {
                        break;
                    }
                    current.push(c2);
                }
            }
            '"' => {
                // Double-quoted: handle \\ and \" escapes
                in_token = true;
                while let Some(c2) = chars.next() {
                    if c2 == '"' {
                        break;
                    } else if c2 == '\\' {
                        if let Some(&next) = chars.peek() {
                            if next == '"' || next == '\\' || next == '$' || next == '`' {
                                current.push(chars.next().unwrap());
                            } else {
                                current.push('\\');
                            }
                        }
                    } else {
                        current.push(c2);
                    }
                }
            }
            '\\' => {
                // Backslash escape outside quotes
                in_token = true;
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_ascii_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            _ => {
                in_token = true;
                current.push(c);
            }
        }
    }

    if in_token || !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
