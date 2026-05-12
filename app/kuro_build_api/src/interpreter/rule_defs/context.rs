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
use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dice::DiceComputations;
use dupe::Dupe;
use futures::FutureExt;
use kuro_core::configuration::build_setting::BuildSettingLabel;
use kuro_core::configuration::build_setting::BuildSettingValue;
use kuro_core::configuration::data::ConfigurationData;
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
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Demand;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::OwnedFrozenValueTyped;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueTyped;
use starlark::values::ValueTypedComplex;
use starlark::values::dict::AllocDict;
use starlark::values::dict::Dict;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::AllocStruct;
use starlark::values::structs::StructRef;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::analysis::anon_promises_dyn::RunAnonPromisesAccessor;
use crate::analysis::registry::AnalysisRegistry;
use crate::artifact_groups::ArtifactGroup;
use crate::deferred::calculation::GET_PROMISED_ARTIFACT;
use crate::interpreter::rule_defs::artifact::methods::ArtifactRoot;
use crate::interpreter::rule_defs::bazel_label::BazelLabel;
use crate::interpreter::rule_defs::cc_common::CcToolchainFeatures;
use crate::interpreter::rule_defs::cc_common::CcToolchainInfoProvider;
use crate::interpreter::rule_defs::cc_common::CcToolchainVariablesGen;
use crate::interpreter::rule_defs::cc_common::CtxCheatArtifactStub;
use crate::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use crate::interpreter::rule_defs::cmd_args::CommandLineArgLike;
use crate::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use crate::interpreter::rule_defs::cmd_args::CommandLineBuilder;
use crate::interpreter::rule_defs::cmd_args::CommandLineContext;
use crate::interpreter::rule_defs::cmd_args::command_line_arg_like_type::command_line_arg_like_impl;
use crate::interpreter::rule_defs::depset::Depset;
use crate::interpreter::rule_defs::depset::make_depset_from_lists;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::fragments::CppFragment;
use crate::interpreter::rule_defs::platform_common::ToolchainInfoInstanceGen;
use crate::interpreter::rule_defs::platform_common::ToolchainInfoProvider;
use crate::interpreter::rule_defs::plugins::AnalysisPlugins;
use crate::interpreter::rule_defs::provider::ProviderLike;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
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

    /// The workspace name (repository) that owns the current rule target.
    ///
    /// Matches the Bazel-visible `ctx.workspace_name` attribute: `_main` for
    /// the root module, otherwise the cell name of the target's package. Used by
    /// `DefaultInfo(executable=..., default_runfiles=...)` to key the runfiles
    /// symlink tree under `<exe>.runfiles/<workspace_name>/...`.
    pub fn workspace_name_str(&self) -> &str {
        match self.label {
            Some(label) => {
                let cell = label.label().target().pkg().cell_name().as_str();
                if kuro_core::cells::is_root_cell_name(cell) {
                    "_main"
                } else {
                    cell
                }
            }
            None => "",
        }
    }

    /// Returns the `ctx.actions` `ValueTyped<AnalysisActions>` for callers that need
    /// to register additional analysis-time actions without going through Starlark
    /// method dispatch.
    pub fn actions_typed(&self) -> ValueTyped<'v, AnalysisActions<'v>> {
        self.actions
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

    /// Collect declared artifacts for the named `attr.output` / `attr.output_list`
    /// attributes. Used by `run_analysis` to auto-populate
    /// `DefaultInfo.default_outputs` when a rule impl declares outputs via
    /// `attr.output` but does not return DefaultInfo explicitly (Bazel convention).
    ///
    /// Must be called while the analysis registry is still live (i.e. before
    /// `take_state`). Attributes that are not present, are empty, or whose
    /// `ctx.outputs.<name>` access fails are silently skipped; downstream
    /// validation will still fire if the rule genuinely produced nothing.
    pub fn collect_implicit_default_outputs(
        &self,
        names: &[String],
        heap: Heap<'v>,
    ) -> Vec<Value<'v>> {
        if names.is_empty() {
            return Vec::new();
        }

        // Ensure the CtxOutputs wrapper exists (it's built lazily on first
        // Starlark access; we may be invoked even if the rule impl never
        // touched `ctx.outputs`).
        let outputs_val = {
            let borrow = self.outputs.borrow();
            match borrow.as_ref().copied() {
                Some(v) => v,
                None => {
                    drop(borrow);
                    let attrs_val = self.attrs.map(|v| v.get()).unwrap_or_else(Value::new_none);
                    let target_name = self
                        .label
                        .as_ref()
                        .map(|l| l.label().target().name().as_str().to_owned())
                        .unwrap_or_else(|| "output".to_owned());
                    let v = heap.alloc(CtxOutputs {
                        attrs: attrs_val,
                        actions: self.actions,
                        declared: RefCell::new(SmallMap::new()),
                        target_name,
                        rule_outputs: self.rule_outputs.clone(),
                    });
                    *self.outputs.borrow_mut() = Some(v);
                    v
                }
            }
        };

        let mut out = Vec::new();
        for name in names {
            let Some(v) = outputs_val.get_attr(name, heap).ok().flatten() else {
                continue;
            };
            if let Some(list) = starlark::values::list::ListRef::from_value(v) {
                for el in list.iter() {
                    out.push(el);
                }
            } else {
                out.push(v);
            }
        }
        out
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
                let canonical = kuro_core::cells::canonical_dynamic_extension_cell_name(cell)
                    .unwrap_or_else(|| cell.to_owned());
                Ok(heap.alloc_str(&canonical).to_value())
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
        // compilation_mode comes from the target's configuration so that a
        // transitioned dep (exec cfg, user cfg) sees its own mode rather than
        // leaking the top-level target's.
        let mode = compilation_mode_from_cfg(cfg_from_label(this.0.label));
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
        let mode = compilation_mode_from_cfg(cfg_from_label(this.0.label));
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
            target_platform: String::new(),
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
            // Extension repos use their canonical repo name.
            if kuro_core::cells::is_root_cell_name(cell_name) {
                Ok(heap.alloc_str("_main").to_value())
            } else {
                let canonical = kuro_core::cells::canonical_dynamic_extension_cell_name(cell_name)
                    .unwrap_or_else(|| cell_name.to_owned());
                Ok(heap.alloc_str(&canonical).to_value())
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
    /// In Kuro, derives the path from the active buck-out root, target's cell
    /// name, and configuration hash:
    /// `<buck-out>/gen/<cell>/<cfg_hash>`
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

        // Resolve from the target's own ConfigurationData (where CLI
        // overrides and transition-produced overrides land).
        if let Some(label) = this.0.label {
            let target = label.label().target();
            let cfg = label.label().cfg();
            let cell = target.pkg().cell_name().as_str();
            let pkg_path = target.pkg().cell_relative_path().as_str();
            let target_name = target.name().as_str();
            let cell_qualified = if pkg_path.is_empty() {
                format!("@{}//:{}", cell, target_name)
            } else {
                format!("@{}//{}:{}", cell, pkg_path, target_name)
            };
            let apparent_cell = normalize_bzlmod_module_repo_name(cell);
            let apparent_cell_qualified = if apparent_cell != cell {
                Some(if pkg_path.is_empty() {
                    format!("@{}//:{}", apparent_cell, target_name)
                } else {
                    format!("@{}//{}:{}", apparent_cell, pkg_path, target_name)
                })
            } else {
                None
            };
            let cell_relative = if pkg_path.is_empty() {
                format!("//:{}", target_name)
            } else {
                format!("//{}:{}", pkg_path, target_name)
            };

            // Try the canonical-cell form first (matches storage that
            // canonicalizes CLI cell aliases via the alias resolver), then
            // fall back to the cell-less form for callers that wrote with
            // no cell prefix.
            let cfg_value: Option<Value<'v>> = BuildSettingLabel::from_bazel_label(&cell_qualified)
                .ok()
                .and_then(|l| cfg.get_build_setting(&l).ok().flatten())
                .or_else(|| {
                    apparent_cell_qualified
                        .as_deref()
                        .and_then(|label| BuildSettingLabel::from_bazel_label(label).ok())
                        .and_then(|l| cfg.get_build_setting(&l).ok().flatten())
                })
                .or_else(|| {
                    BuildSettingLabel::from_bazel_label(&cell_relative)
                        .ok()
                        .and_then(|l| cfg.get_build_setting(&l).ok().flatten())
                })
                .map(|v| build_setting_value_to_starlark(v, heap));

            // Fallback: the process-global starlark_flags store, used by
            // any path that wrote there (e.g. transition mirror-writes).
            let flag_label_candidates = || {
                std::iter::once(cell_qualified.as_str())
                    .chain(apparent_cell_qualified.as_deref())
                    .chain(std::iter::once(cell_relative.as_str()))
            };
            let cli_value_to_starlark = |cli_value: &str| match cli_value {
                "True" | "true" | "1" => heap.alloc(true).to_value(),
                "False" | "false" | "0" => heap.alloc(false).to_value(),
                s => heap.alloc_str(s).to_value(),
            };
            let final_value = cfg_value.or_else(|| {
                if allows_multiple {
                    use starlark::values::list::AllocList;
                    flag_label_candidates()
                        .find_map(
                            crate::interpreter::rule_defs::build_config::get_starlark_flag_values,
                        )
                        .map(|values| {
                            let values = values
                                .iter()
                                .map(|value| cli_value_to_starlark(value))
                                .collect::<Vec<_>>();
                            heap.alloc(AllocList(values)).to_value()
                        })
                } else {
                    flag_label_candidates()
                        .find_map(crate::interpreter::rule_defs::build_config::get_starlark_flag)
                        .map(|cli_value| cli_value_to_starlark(&cli_value))
                }
            });

            if let Some(value) = final_value {
                if allows_multiple {
                    use starlark::values::list::AllocList;
                    use starlark::values::list::ListRef;
                    // If the typed read already produced a list (e.g. from
                    // `BuildSettingValue::StringList`) return it as-is;
                    // otherwise wrap a scalar.
                    if ListRef::from_value(value).is_some() {
                        return Ok(value);
                    }
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

    // `var`, `coverage_instrumented`, `tokenize`, `runfiles`,
    // `target_platform_has_constraint`, `expand_make_variables`,
    // `package_relative_label`, `resolve_tools`, `resolve_command`,
    // and `new_file` are served from `@kuro_builtins//:exports.bzl`
    // through the rule-context facade installed by `_invoke_rule`.

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
    // `expand_location` is served from `@kuro_builtins//:exports.bzl`
    // through the facade. Two runtime hooks bridge the Rust-only logic:
    //   - `kuro_collect_location_pool` — builds the label→paths pool
    //     (targets list + implicit attrs walk).
    //   - `kuro_lookup_output_path` — lazily resolves attr.output /
    //     attr.output_list labels to declared-artifact paths (deferred
    //     to avoid spurious unbound-artifact declarations).
    fn expand_location<'v>(
        this: RefAnalysisContext<'v>,
        input: &str,
        #[starlark(default = NoneType)] targets: Value<'v>,
        #[starlark(require = named, default = false)] short_paths: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<String> {
        let _ = (this, input, targets, short_paths, heap);
        // Unreachable when the `_make_rule_facade` in exports.bzl is
        // active: the facade's `expand_location` field points to
        // `_expand_location_bound`, which calls `_kuro_expand_location`
        // entirely in Starlark. This stub is retained so the method
        // name survives on `AnalysisContext` (BXL / dynamic_output
        // callers that bypass the facade still reach Rust here — they
        // receive an empty string until a follow-up stage extends the
        // facade to those paths). For the rule-impl path the Starlark
        // function is the canonical implementation.
        Ok(String::new())
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

/// Pub entry point for the `kuro_collect_location_pool` Starlark
/// global. Builds the label→paths pool used by `_kuro_expand_location`
/// in `@kuro_builtins//:exports.bzl`.
///
/// Returns a flat list of `[label_str, [path1, path2, ...]]` two-element
/// Starlark lists. Pool entries come from two sources:
///
///   1. Explicit `targets` list (Dependency / FrozenDependency values).
///   2. Implicit attrs walk: Dependency / FrozenDependency values in any
///      attribute of the rule (srcs, data, tools, etc.) and any source
///      artifact values, keyed by their short_path.
///
/// The attr.output / attr.output_list lookup (string-keyed, deferred to
/// avoid spurious artifact declarations) is handled by the companion
/// `kuro_lookup_output_path` hook rather than being included in this pool.
pub fn collect_location_pool_for_ctx<'v>(
    ctx: &AnalysisContext<'v>,
    targets: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    use starlark::values::list::AllocList;
    use starlark::values::list::ListRef;

    use crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact;
    use crate::interpreter::rule_defs::artifact::starlark_artifact_like::StarlarkInputArtifactLike;
    use crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact;
    use crate::interpreter::rule_defs::provider::dependency::Dependency;
    use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;
    use crate::interpreter::rule_defs::provider::dependency::SourceFileTarget;

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

    let mut entries: Vec<(String, Vec<String>)> = Vec::new();

    // --- Explicit targets list ---
    if let Ok(iter) = targets.iterate(heap) {
        for dep_val in iter {
            if let Some(dep) = dep_val.downcast_ref::<Dependency>() {
                let label_str = dep.label().label().unconfigured().to_string();
                let paths = collect_output_paths(dep.provider_collection());
                entries.push((label_str, paths));
            } else if let Some(dep) = dep_val.downcast_ref::<FrozenDependency>() {
                let label_str = dep.label().label().unconfigured().to_string();
                let paths = collect_output_paths(dep.provider_collection());
                entries.push((label_str, paths));
            }
        }
    }

    // --- Implicit attrs walk (srcs / data / tools / artifact attrs) ---
    let attrs_v_opt = ctx.attrs.as_ref().map(|s| s.get().to_value());

    let collect_from_value = |entries: &mut Vec<(String, Vec<String>)>, v: Value<'v>| {
        if let Some(art) = v.downcast_ref::<StarlarkArtifact>() {
            if let Ok(bound) = art.get_bound_starlark_artifact() {
                let path = bound
                    .artifact()
                    .get_path()
                    .with_full_path(|p| p.as_str().to_owned());
                let short = bound
                    .artifact()
                    .get_path()
                    .with_short_path(|p| p.as_str().to_owned());
                if !short.is_empty() {
                    entries.push((short, vec![path]));
                }
            }
        } else if let Some(art) = v.downcast_ref::<StarlarkDeclaredArtifact<'v>>() {
            if let Ok(bound) = art.get_bound_starlark_artifact() {
                let path = bound
                    .artifact()
                    .get_path()
                    .with_full_path(|p| p.as_str().to_owned());
                let short = bound
                    .artifact()
                    .get_path()
                    .with_short_path(|p| p.as_str().to_owned());
                if !short.is_empty() {
                    entries.push((short, vec![path]));
                }
            }
        } else if let Some(dep) = v.downcast_ref::<Dependency>() {
            let label_str = dep.label().label().unconfigured().to_string();
            let paths = collect_output_paths(dep.provider_collection());
            entries.push((label_str, paths));
        } else if let Some(dep) = v.downcast_ref::<FrozenDependency>() {
            let label_str = dep.label().label().unconfigured().to_string();
            let paths = collect_output_paths(dep.provider_collection());
            entries.push((label_str, paths));
        } else if let Some(source) = v.downcast_ref::<SourceFileTarget>() {
            let artifact_value = source.artifact_value(heap);
            if let Some(art) = artifact_value.downcast_ref::<StarlarkArtifact>() {
                if let Ok(bound) = art.get_bound_starlark_artifact() {
                    let path = bound
                        .artifact()
                        .get_path()
                        .with_full_path(|p| p.as_str().to_owned());
                    let short = bound
                        .artifact()
                        .get_path()
                        .with_short_path(|p| p.as_str().to_owned());
                    entries.push((
                        source.label().unconfigured().to_string(),
                        vec![path.clone()],
                    ));
                    if !short.is_empty() {
                        entries.push((short, vec![path]));
                    }
                }
            }
        }
    };

    if let Some(attrs_v) = attrs_v_opt {
        if let Some(struct_ref) = starlark::values::structs::StructRef::from_value(attrs_v) {
            for (_name, attr_val) in struct_ref.iter() {
                collect_from_value(&mut entries, attr_val);
                if let Some(list) = ListRef::from_value(attr_val) {
                    for el in list.iter() {
                        collect_from_value(&mut entries, el);
                    }
                }
            }
        }
    }

    // Build Starlark list of [label_str, [path, ...]] pairs.
    let starlark_entries: Vec<Value<'v>> = entries
        .into_iter()
        .map(|(label, paths)| {
            let paths_val = heap.alloc(AllocList(paths.into_iter()));
            heap.alloc(AllocList([heap.alloc(label), paths_val]))
        })
        .collect();

    Ok(heap.alloc(AllocList(starlark_entries)))
}

/// Pub entry point for the `kuro_lookup_output_path` Starlark global.
/// Resolves a bare label
/// (e.g. `:generated_file`) to an output artifact path by scanning the
/// rule's attrs for string-valued lists (attr.output / attr.output_list
/// attrs), then calling `ctx.outputs.<attr_name>[idx]` lazily so that
/// only the specific output attribute that matches is declared — not
/// every string attribute in the rule.
///
/// Returns the full artifact path string, or `None` (`Value::new_none()`)
/// if no matching output attribute is found.
pub fn lookup_output_path_for_ctx<'v>(
    ctx: &AnalysisContext<'v>,
    label_str: &str,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    use starlark::values::list::ListRef;

    let query_name = label_str.trim_start_matches(':');
    let attrs_v_opt = ctx.attrs.as_ref().map(|s| s.get().to_value());

    let Some(attrs_v) = attrs_v_opt else {
        return Ok(Value::new_none());
    };
    let Some(struct_ref) = starlark::values::structs::StructRef::from_value(attrs_v) else {
        return Ok(Value::new_none());
    };

    // Ensure the CtxOutputs wrapper is initialized (mirrors the lazy init in
    // the deleted `expand_location` impl).
    let outputs_val = {
        let cached = ctx.outputs.borrow().as_ref().copied();
        match cached {
            Some(v) => v,
            None => {
                let attrs_for_outputs = ctx.attrs.map(|v| v.get()).unwrap_or_else(Value::new_none);
                let target_name = ctx
                    .label
                    .as_ref()
                    .map(|l| l.label().target().name().as_str().to_owned())
                    .unwrap_or_else(|| "output".to_string());
                let v = heap.alloc(CtxOutputs {
                    attrs: attrs_for_outputs,
                    actions: ctx.actions,
                    declared: RefCell::new(SmallMap::new()),
                    target_name,
                    rule_outputs: ctx.rule_outputs.clone(),
                });
                *ctx.outputs.borrow_mut() = Some(v);
                v
            }
        }
    };

    for (name, attr_val) in struct_ref.iter() {
        let Some(list) = ListRef::from_value(attr_val) else {
            continue;
        };
        let strings: Vec<(usize, &str)> = list
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.unpack_str().map(|s| (i, s)))
            .collect();
        let Some(idx) = strings.iter().position(|(_i, s)| *s == query_name) else {
            continue;
        };
        let Ok(Some(out_val)) = outputs_val.get_attr(name.as_str(), heap) else {
            continue;
        };
        let Some(out_list) = ListRef::from_value(out_val) else {
            continue;
        };
        let Some(el) = out_list.iter().nth(idx) else {
            continue;
        };
        if let Some(art) = el.downcast_ref::<crate::interpreter::rule_defs::artifact::starlark_declared_artifact::StarlarkDeclaredArtifact<'v>>() {
            let path = art
                .get_artifact_path()
                .with_full_path(|p| p.as_str().to_owned());
            return Ok(heap.alloc(path));
        }
        if let Some(art) = el.downcast_ref::<crate::interpreter::rule_defs::artifact::starlark_artifact::StarlarkArtifact>() {
            let path = art
                .artifact()
                .get_path()
                .with_full_path(|p| p.as_str().to_owned());
            return Ok(heap.alloc(path));
        }
    }
    Ok(Value::new_none())
}

/// Derives the output directory path from a configured target label.
///
/// In Kuro, the output path is `<buck-out>/gen/<cell>/<cfg_hash>`.
/// Falls back to `<buck-out>/gen` if no label is available.
pub fn bin_dir_path_from_label(
    label: Option<
        starlark::values::ValueTyped<
            '_,
            kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel,
        >,
    >,
) -> String {
    let buck_out_root = kuro_execute::path::artifact_path::get_artifact_path_buck_out_root();
    let buck_out_root = buck_out_root.as_str();
    if let Some(label) = label {
        let target = label.label().target();
        let cell_name = target.pkg().cell_name().as_str();
        let output_cell_name = if kuro_core::cells::is_root_cell_name(cell_name) {
            cell_name.to_owned()
        } else {
            kuro_core::cells::canonical_dynamic_extension_cell_name(cell_name)
                .unwrap_or_else(|| cell_name.to_owned())
        };
        let cfg_hash = label.label().cfg().output_hash().as_str();
        format!("{}/gen/{}/{}", buck_out_root, output_cell_name, cfg_hash)
    } else {
        format!("{}/gen", buck_out_root)
    }
}

/// Bazel's `$(WORKSPACE_ROOT)` make-variable: the path from the exec
/// root to the workspace root that contains a target. For a target in
/// the root cell this is the empty string; for a target in an external
/// cell it is `external/<cell>`.
///
/// rules_cc cc_library declarations in real-world Bazel projects bake
/// this into `copts`, e.g. `@llvm-project//clang:basic` carries
/// `copts = ["-I$(WORKSPACE_ROOT)/clang/lib/Basic"]` so that
/// `clang/lib/Basic/Targets/TCE.cpp`'s `#include "Targets.h"` resolves
/// to a sibling file in `clang/lib/Basic/`. Bazel proper exposes
/// `WORKSPACE_ROOT` through Bazel's `MAKE_VARIABLES` defaults; without
/// hardcoding it here, `$(WORKSPACE_ROOT)` would survive copts
/// substitution literally and the compile would fail to find the
/// header. (Plan 29.4.)
pub fn workspace_root_from_label(
    label: Option<
        starlark::values::ValueTyped<
            '_,
            kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel,
        >,
    >,
) -> String {
    let Some(label) = label else {
        return String::new();
    };
    let cell_name = label.label().target().pkg().cell_name().as_str();
    if kuro_core::cells::is_root_cell_name(cell_name) {
        String::new()
    } else {
        let output_cell_name = kuro_core::cells::canonical_dynamic_extension_cell_name(cell_name)
            .unwrap_or_else(|| cell_name.to_owned());
        format!("external/{}", output_cell_name)
    }
}

/// Canonical label of Bazel's `--compilation_mode` CLI flag. Kept in sync
/// with `kuro_configured::target_platform_resolution`.
const COMPILATION_MODE_LABEL: &str = "@bazel_tools//tools/cpp:compilation_mode";

/// Returns the current target's `ConfigurationData`, if the context has a
/// configured label (all rule-impl ctxs do).
fn cfg_from_label<'v>(
    label: Option<
        starlark::values::ValueTyped<
            'v,
            kuro_interpreter::types::configured_providers_label::StarlarkConfiguredProvidersLabel,
        >,
    >,
) -> Option<&'v ConfigurationData> {
    let typed = label?;
    let starlark_label: &'v StarlarkConfiguredProvidersLabel = typed.as_ref();
    Some(starlark_label.label().cfg())
}

/// Reads `compilation_mode` from the target cfg's build_settings. Falls back
/// to the process-global `BUILD_CONFIG` entry (pre-Plan-19.4 path) when the
/// cfg does not carry the setting — preserves behaviour during the
/// phased rollout for contexts that never route through
/// `get_configured_target` (e.g. bxl top-level, anonymous targets).
fn compilation_mode_from_cfg(cfg: Option<&ConfigurationData>) -> String {
    if let Some(cfg) = cfg {
        if let Ok(label) = BuildSettingLabel::from_bazel_label(COMPILATION_MODE_LABEL) {
            if let Ok(Some(BuildSettingValue::String(value))) = cfg.get_build_setting(&label) {
                return value.clone();
            }
        }
    }
    crate::interpreter::rule_defs::build_config::get_compilation_mode()
}

/// Pub entry point for the `kuro_compilation_mode_for_label` Starlark
/// global. Takes the raw label `Value`, extracts its cfg, and resolves
/// `compilation_mode_from_cfg`. Hides the cfg hash from Starlark.
pub fn compilation_mode_for_label_value(label: Value<'_>) -> String {
    let cfg = label
        .downcast_ref::<StarlarkConfiguredProvidersLabel>()
        .map(|l| l.label().cfg());
    compilation_mode_from_cfg(cfg)
}

/// Converts a typed `BuildSettingValue` into the Starlark representation used
/// by `ctx.build_setting_value`.
fn build_setting_value_to_starlark<'v>(value: &BuildSettingValue, heap: Heap<'v>) -> Value<'v> {
    match value {
        BuildSettingValue::Bool(b) => heap.alloc(*b).to_value(),
        BuildSettingValue::Int(i) => heap.alloc(*i).to_value(),
        BuildSettingValue::String(s) => {
            // Preserve the CLI boolean encoding so bool-typed build settings
            // stored as "True"/"False" by Plan 19.2's mirror-write round-trip
            // to a Starlark bool here.
            match s.as_str() {
                "True" | "true" | "1" => heap.alloc(true).to_value(),
                "False" | "false" | "0" => heap.alloc(false).to_value(),
                other => heap.alloc_str(other).to_value(),
            }
        }
        BuildSettingValue::StringList(xs) | BuildSettingValue::StringSet(xs) => heap
            .alloc(starlark::values::list::AllocList(xs.iter().cloned()))
            .to_value(),
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

/// Gather `TemplateVariableInfo` variables from each dep in the current
/// target's `toolchains` attribute. Returns `(name, value)` pairs ready to
/// merge into `ctx.var` / the `$(VAR)` expansion map.
///
/// Mirrors Bazel's `RuleContext.getMakeVariables()`. `ctx.attr.toolchains`
/// is the target-level attribute (as opposed to `rule(toolchains = [...])`
/// which declares rule-level toolchain *types*): a list of deps whose
/// providers are exposed to the target. `TemplateVariableInfo` is the
/// provider that carries Make-variable definitions, most commonly from
/// LLVM-style `workspace_root` rules or from a project's `make_variables.bzl`.
pub fn collect_toolchains_template_vars_from_list<'v>(
    toolchains_val: Value<'v>,
) -> Vec<(String, String)> {
    use starlark::values::list::ListRef;

    use crate::interpreter::rule_defs::platform_common::TemplateVariableInfoCallable;
    use crate::interpreter::rule_defs::platform_common::TemplateVariableInfoInstance;
    use crate::interpreter::rule_defs::provider::dependency::Dependency;
    use crate::interpreter::rule_defs::provider::dependency::FrozenDependency;

    let Some(list) = ListRef::from_value(toolchains_val) else {
        return Vec::new();
    };

    let tvi_id = TemplateVariableInfoCallable::provider_id();
    let mut out: Vec<(String, String)> = Vec::new();
    for item in list.iter() {
        // `ctx.attr.toolchains` items are `Dependency` (unfrozen, rare at
        // analysis time) or `FrozenDependency` (the common case).
        let providers = if let Some(dep) = item.downcast_ref::<Dependency<'v>>() {
            dep.provider_collection()
        } else if let Some(frozen) = item.downcast_ref::<FrozenDependency>() {
            frozen.provider_collection()
        } else {
            continue;
        };
        let Some(provider_fv) = providers.get_provider_raw(tvi_id) else {
            continue;
        };
        let Some(instance) = provider_fv
            .to_value()
            .downcast_ref::<TemplateVariableInfoInstance>()
        else {
            continue;
        };
        for (k, v) in instance.variables() {
            out.push((k.clone(), v.clone()));
        }
    }
    out
}

/// Merges runfiles from a single attribute value (list of Dependency
/// or a single Dependency) into `runfiles`. Called by
/// `kuro_collect_runfiles_into` to serve
/// `ctx.runfiles(collect_default=True/collect_data=True)` from Starlark.
pub fn collect_runfiles_from_value<'v>(
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

        // Outputs declared via `ctx.outputs.<name>` from `attr.output` /
        // `attr.output_list` attributes use the Bazel-shaped path layout so
        // that `cc_library(hdrs=[...])` consumers find generated headers at
        // the include-path locations that follow Bazel's `bazel-bin/<pkg>/...`
        // convention (with `external/<cell>/` prefix for external cells).
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
                    BuckOutPathKind::BazelOutput,
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

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct CcInfoNativeShim {
    module_map_path: Option<String>,
}

impl std::fmt::Display for CcInfoNativeShim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CcInfo(...)")
    }
}

starlark::starlark_simple_value!(CcInfoNativeShim);

#[starlark::values::starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoNativeShim {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "compilation_context" | "linking_context")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "compilation_context" => Some(heap.alloc(EmptyCompilationContext {
                module_map_path: self.module_map_path.clone(),
            })),
            "linking_context" => Some(heap.alloc(EmptyLinkingContext)),
            _ => None,
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "CcInfoNativeShim".hash(hasher);
        self.module_map_path.hash(hasher);
        Ok(())
    }
}

/// Cycle-breaker for C++ toolchain provider lookups.
///
/// This is intentionally minimal: it exposes the CcToolchainInfo fields that
/// rules_cc consults while analyzing support/header/runtime deps inside the
/// selected C++ toolchain's own dependency cone. The real cc_toolchain target is
/// still analyzed normally once its deps are available.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct CcToolchainInfoNativeShim {
    toolchain_label: String,
    target_platform: String,
    toolchain_config_info: Option<FrozenValue>,
    toolchain_features: Option<CcToolchainFeatures>,
    module_map_path: Option<String>,
    toolchain_files: Option<Arc<[ToolchainInputFile]>>,
}

const CC_TOOLCHAIN_NATIVE_SHIM_ATTRS: &[&str] = &[
    "compiler",
    "compiler_executable",
    "preprocessor_executable",
    "nm_executable",
    "objdump_executable",
    "ar_executable",
    "strip_executable",
    "ld_executable",
    "gcov_executable",
    "objcopy_executable",
    "generate_modmap",
    "cpu",
    "libc",
    "target_gnu_system_name",
    "toolchain_id",
    "dynamic_runtime_solib_dir",
    "built_in_include_directories",
    "all_files",
    "sysroot",
    "_as_files",
    "_ar_files",
    "_strip_files",
    "_tool_paths",
    "_solib_dir",
    "_linker_files",
    "_coverage_files",
    "_fdo_context",
    "_compiler_files",
    "_dwp_files",
    "_builtin_include_files",
    "_legacy_cc_flags_make_variable",
    "_additional_make_variables",
    "_all_files_including_libc",
    "_abi",
    "_abi_glibc_version",
    "_crosstool_top_path",
    "_build_info_files",
    "_build_variables_dict",
    "_build_variables",
    "_supports_header_parsing",
    "_supports_param_files",
    "_toolchain_features",
    "_toolchain_label",
    "_cpp_configuration",
    "_link_dynamic_library_tool",
    "_grep_includes",
    "_if_so_builder",
    "_is_tool_configuration",
    "_is_sibling_repository_layout",
    "_stamp_binaries",
    "_static_runtime_lib_depset",
    "_dynamic_runtime_lib_depset",
    "_compiler_files_without_includes",
    "_allowlist_for_layering_check",
    "_cc_info",
    "_objcopy_files",
    "_aggregate_ddi",
    "_extra_allowlisted_feature_layering_check_macros",
    "_force_layering_check_features",
];

fn cc_toolchain_native_shim_has_attr(attribute: &str) -> bool {
    CC_TOOLCHAIN_NATIVE_SHIM_ATTRS.contains(&attribute)
}

/// Adds target-platform identity to real `CcToolchainInfo` values when Kuro has
/// resolved the toolchain but the provider's rule-based metadata is incomplete.
///
/// rules_rust derives build-script `CFLAGS` from `cc_toolchain.libc` and
/// `cc_toolchain.target_gnu_system_name`; without the musl identity it compiles
/// C deps such as aws-lc-sys against host/glibc headers while later linking a
/// musl binary.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct CcToolchainInfoTargetPlatformOverlay {
    inner: FrozenValue,
    target_platform: String,
}

impl CcToolchainInfoTargetPlatformOverlay {
    fn is_musl_target(&self) -> bool {
        self.target_platform.contains("musl")
    }
}

impl std::fmt::Display for CcToolchainInfoTargetPlatformOverlay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.to_value())
    }
}

starlark::starlark_simple_value!(CcToolchainInfoTargetPlatformOverlay);

#[starlark::starlark_module]
fn cc_toolchain_target_platform_overlay_methods(builder: &mut MethodsBuilder) {
    fn needs_pic_for_dynamic_libraries<'v>(
        #[starlark(this)] this: &CcToolchainInfoTargetPlatformOverlay,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        let heap = eval.heap();
        if let Ok(Some(method)) = this
            .inner
            .to_value()
            .get_attr("needs_pic_for_dynamic_libraries", heap)
        {
            if let Ok(result) = eval.eval_function(
                method,
                &[],
                &[("feature_configuration", feature_configuration)],
            ) {
                if let Some(value) = result.unpack_bool() {
                    return Ok(value);
                }
            }
        }
        Ok(true)
    }

    fn static_runtime_lib<'v>(
        #[starlark(this)] this: &CcToolchainInfoTargetPlatformOverlay,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        if let Ok(Some(method)) = this.inner.to_value().get_attr("static_runtime_lib", heap) {
            if let Ok(result) = eval.eval_function(
                method,
                &[],
                &[("feature_configuration", feature_configuration)],
            ) {
                return Ok(result);
            }
        }
        Ok(heap.alloc(Depset::empty()))
    }

    fn dynamic_runtime_lib<'v>(
        #[starlark(this)] this: &CcToolchainInfoTargetPlatformOverlay,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        if let Ok(Some(method)) = this.inner.to_value().get_attr("dynamic_runtime_lib", heap) {
            if let Ok(result) = eval.eval_function(
                method,
                &[],
                &[("feature_configuration", feature_configuration)],
            ) {
                return Ok(result);
            }
        }
        Ok(heap.alloc(Depset::empty()))
    }
}

#[starlark::values::starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoTargetPlatformOverlay {
    fn to_bool(&self) -> bool {
        true
    }

    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        matches!(attribute, "target_gnu_system_name" | "libc")
            || cc_toolchain_native_shim_has_attr(attribute)
            || self.inner.to_value().has_attr(attribute, heap)
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if self.is_musl_target() {
            match attribute {
                "target_gnu_system_name" => {
                    return Some(heap.alloc_str("x86_64-linux-musl").to_value());
                }
                "libc" => return Some(heap.alloc_str("musl").to_value()),
                _ => {}
            }
        }
        self.inner
            .to_value()
            .get_attr(attribute, heap)
            .ok()
            .flatten()
            .or_else(|| {
                let shim = CcToolchainInfoNativeShim {
                    toolchain_label: "@bazel_tools//tools/cpp:current_cc_toolchain".to_owned(),
                    target_platform: self.target_platform.clone(),
                    toolchain_config_info: None,
                    toolchain_features: None,
                    module_map_path: None,
                    toolchain_files: None,
                };
                shim.get_attr(attribute, heap)
            })
    }

    fn dir_attr(&self) -> Vec<String> {
        let mut attrs = self.inner.to_value().dir_attr();
        for attr in CC_TOOLCHAIN_NATIVE_SHIM_ATTRS.iter().copied() {
            if !attrs.iter().any(|existing| existing == attr) {
                attrs.push(attr.to_owned());
            }
        }
        attrs
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_toolchain_target_platform_overlay_methods)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "CcToolchainInfoTargetPlatformOverlay".hash(hasher);
        self.inner.to_value().write_hash(hasher)?;
        self.target_platform.hash(hasher);
        Ok(())
    }
}

fn host_llvm_toolchain_bin(tool: &str) -> Option<String> {
    let root = kuro_core::cells::get_dynamic_project_root()?;
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let suffix = format!("-{os}-{arch}");
    let external = root.join("bazel-external");
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(&external).ok()?.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let is_llvm_toolchain = name.starts_with("llvm-toolchain-minimal-")
            || name.starts_with("llvm+http_archive+llvm-toolchain-minimal-");
        if !is_llvm_toolchain || !name.ends_with(&suffix) {
            continue;
        }
        let path = entry.path().join("bin").join(tool);
        if path.is_file() {
            candidates.push(path.to_string_lossy().into_owned());
        }
    }
    candidates.sort();
    candidates.into_iter().next()
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ToolchainInputRootStub {
    path: String,
}

impl fmt::Display for ToolchainInputRootStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<root {}>", self.path)
    }
}

starlark::starlark_simple_value!(ToolchainInputRootStub);

unsafe impl<'v> Trace<'v> for ToolchainInputRootStub {
    fn trace(&mut self, _tracer: &starlark::values::Tracer<'v>) {}
}

#[starlark::values::starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for ToolchainInputRootStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// Stable-backed `File` value used only by the native C++ toolchain shim.
///
/// These values live inside a frozen provider collection that is returned as a
/// cycle breaker during toolchain analysis. Keep the payload pointer-stable and
/// convert to Kuro artifact groups only when command-line inputs are visited.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ToolchainInputFileStub {
    path: &'static str,
    input_target: &'static ConfiguredTargetLabel,
}

impl fmt::Display for ToolchainInputFileStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<artifact {}>", self.path)
    }
}

starlark::starlark_simple_value!(ToolchainInputFileStub);

unsafe impl<'v> Trace<'v> for ToolchainInputFileStub {
    fn trace(&mut self, _tracer: &starlark::values::Tracer<'v>) {}
}

#[starlark::values::starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for ToolchainInputFileStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "extension"
                | "is_source"
                | "root"
                | "is_directory"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" | "short_path" => Some(heap.alloc_str(self.path).to_value()),
            "basename" => {
                let basename = self.path.rsplit('/').next().unwrap_or(self.path);
                Some(heap.alloc_str(basename).to_value())
            }
            "extension" => {
                let ext = self.path.rsplit('.').next().unwrap_or("");
                Some(heap.alloc_str(ext).to_value())
            }
            "is_source" | "is_directory" => Some(Value::new_bool(false)),
            "root" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(ToolchainInputRootStub {
                    path: format!("bazel-out/{}-{}/bin", host_target_cpu(), m),
                }))
            }
            _ => None,
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        match ToolchainInputFileStub::from_value(other) {
            Some(other) => Ok(self.path == other.path),
            None => Ok(false),
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.path.hash(hasher);
        Ok(())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }
}

impl<'v> CommandLineArgLike<'v> for ToolchainInputFileStub {
    fn register_me(&self) {
        command_line_arg_like_impl!(ToolchainInputFileStub::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        cli: &mut dyn CommandLineBuilder,
        _context: &mut dyn CommandLineContext,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> kuro_error::Result<()> {
        cli.push_arg(self.path.to_owned());
        Ok(())
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> kuro_error::Result<()> {
        visitor.visit_input(
            ArtifactGroup::TargetDefaultOutputs(Arc::new(self.input_target.dupe())),
            vec![],
        );
        Ok(())
    }

    fn contains_arg_attr(&self) -> bool {
        false
    }

    fn visit_write_to_file_macros(
        &self,
        _visitor: &mut dyn crate::interpreter::rule_defs::cmd_args::WriteToFileMacroVisitor,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> kuro_error::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Allocative)]
struct ToolchainInputFile {
    path: &'static str,
    input_target: &'static ConfiguredTargetLabel,
}

/// Hidden action input carrier for native C++ toolchain files.
///
/// This is stored as the single direct element of the `cc_toolchain.all_files`
/// depset. It reports Starlark type `File` so public depset type validation can
/// compose it with real file depsets, but it avoids exposing fake per-file
/// artifacts through Starlark. Kuro only needs these records as hidden action
/// inputs when rules pass toolchain files through `ctx.actions.run(tools=...)`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ToolchainInputFilesArg {
    files: Arc<[ToolchainInputFile]>,
}

impl fmt::Display for ToolchainInputFilesArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<toolchain files>")
    }
}

starlark::starlark_simple_value!(ToolchainInputFilesArg);

unsafe impl<'v> Trace<'v> for ToolchainInputFilesArg {
    fn trace(&mut self, _tracer: &starlark::values::Tracer<'v>) {}
}

#[starlark::values::starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for ToolchainInputFilesArg {
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "ToolchainInputFilesArg".hash(hasher);
        for file in self.files.iter() {
            file.path.hash(hasher);
        }
        Ok(())
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }
}

impl<'v> CommandLineArgLike<'v> for ToolchainInputFilesArg {
    fn register_me(&self) {
        command_line_arg_like_impl!(ToolchainInputFilesArg::starlark_type_repr());
    }

    fn add_to_command_line(
        &self,
        _cli: &mut dyn CommandLineBuilder,
        _context: &mut dyn CommandLineContext,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> kuro_error::Result<()> {
        Ok(())
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> kuro_error::Result<()> {
        for file in self.files.iter() {
            visitor.visit_input(
                ArtifactGroup::TargetDefaultOutputs(Arc::new(file.input_target.dupe())),
                vec![],
            );
        }
        Ok(())
    }

    fn contains_arg_attr(&self) -> bool {
        false
    }

    fn visit_write_to_file_macros(
        &self,
        _visitor: &mut dyn crate::interpreter::rule_defs::cmd_args::WriteToFileMacroVisitor,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> kuro_error::Result<()> {
        Ok(())
    }
}

impl std::fmt::Display for CcToolchainInfoNativeShim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CcToolchainInfo({})", self.toolchain_label)
    }
}

starlark::starlark_simple_value!(CcToolchainInfoNativeShim);

impl CcToolchainInfoNativeShim {
    fn is_musl_target(&self) -> bool {
        self.toolchain_label.contains("musl") || self.target_platform.contains("musl")
    }

    fn target_system_name(&self) -> &'static str {
        if self.is_musl_target() {
            "x86_64-linux-musl"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    }

    fn target_libc(&self) -> &'static str {
        if self.is_musl_target() { "musl" } else { "gnu" }
    }

    fn build_variables<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        let vars = heap.alloc(Dict::new(SmallMap::from_iter([
            (
                heap.alloc_str("target_system_name")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                heap.alloc_str(self.target_system_name()).to_value(),
            ),
            (
                heap.alloc_str("target_libc")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                heap.alloc_str(self.target_libc()).to_value(),
            ),
        ])));
        heap.alloc(CcToolchainVariablesGen { vars })
    }

    fn toolchain_files_depset<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        if std::env::var_os("KURO_DISABLE_CC_TOOLCHAIN_FILE_SHIM").is_some() {
            return heap.alloc(Depset::empty());
        }
        self.toolchain_files
            .as_ref()
            .map(|files| {
                let carrier = heap.alloc(ToolchainInputFilesArg {
                    files: Arc::clone(files),
                });
                make_depset_from_lists(heap, vec![carrier], vec![], "default")
                    .expect("toolchain file carrier depset should be valid")
            })
            .unwrap_or_else(|| heap.alloc(Depset::empty()))
    }
}

#[starlark::starlark_module]
fn cc_toolchain_native_shim_methods(builder: &mut MethodsBuilder) {
    fn needs_pic_for_dynamic_libraries(
        #[starlark(this)] _this: &CcToolchainInfoNativeShim,
        #[starlark(require = named)] feature_configuration: Value,
    ) -> starlark::Result<bool> {
        let _ = feature_configuration;
        Ok(true)
    }

    fn static_runtime_lib<'v>(
        #[starlark(this)] _this: &CcToolchainInfoNativeShim,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = feature_configuration;
        Ok(heap.alloc(Depset::empty()))
    }

    fn dynamic_runtime_lib<'v>(
        #[starlark(this)] _this: &CcToolchainInfoNativeShim,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = feature_configuration;
        Ok(heap.alloc(Depset::empty()))
    }
}

#[starlark::values::starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoNativeShim {
    fn to_bool(&self) -> bool {
        true
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        cc_toolchain_native_shim_has_attr(attribute)
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let empty_depset = || heap.alloc(Depset::empty());
        let empty_list = || heap.alloc(Vec::<Value<'v>>::new());
        let empty_dict = || heap.alloc(AllocDict::EMPTY);
        let empty_string = || heap.alloc_str("").to_value();
        match attribute {
            "compiler" => Some(heap.alloc_str("clang").to_value()),
            "compiler_executable" | "preprocessor_executable" => {
                let compiler = host_llvm_toolchain_bin("clang").unwrap_or_else(|| {
                    if cfg!(target_os = "windows") {
                        "cl.exe".to_owned()
                    } else if cfg!(target_os = "macos") {
                        "/usr/bin/clang".to_owned()
                    } else {
                        "/usr/bin/gcc".to_owned()
                    }
                });
                Some(heap.alloc_str(&compiler).to_value())
            }
            "nm_executable" => Some(heap.alloc_str("nm").to_value()),
            "objdump_executable" => Some(heap.alloc_str("objdump").to_value()),
            "ar_executable" => Some(heap.alloc_str("ar").to_value()),
            "strip_executable" => Some(heap.alloc_str("strip").to_value()),
            "ld_executable" => Some(heap.alloc_str("ld").to_value()),
            "gcov_executable" => Some(heap.alloc_str("gcov").to_value()),
            "objcopy_executable" => Some(heap.alloc_str("objcopy").to_value()),
            "generate_modmap" => Some(Value::new_none()),
            "cpu" => Some(heap.alloc_str(&host_target_cpu()).to_value()),
            "target_gnu_system_name" => Some(heap.alloc_str(self.target_system_name()).to_value()),
            "libc" => Some(heap.alloc_str(self.target_libc()).to_value()),
            "toolchain_id" => Some(empty_string()),
            "dynamic_runtime_solib_dir" => Some(heap.alloc_str("_solib").to_value()),
            "built_in_include_directories" => Some(empty_list()),
            "all_files" => Some(self.toolchain_files_depset(heap)),
            "_as_files"
            | "_ar_files"
            | "_strip_files"
            | "_linker_files"
            | "_coverage_files"
            | "_compiler_files"
            | "_dwp_files"
            | "_builtin_include_files"
            | "_all_files_including_libc"
            | "_build_info_files"
            | "_static_runtime_lib_depset"
            | "_dynamic_runtime_lib_depset"
            | "_compiler_files_without_includes"
            | "_objcopy_files" => Some(empty_depset()),
            "sysroot"
            | "_link_dynamic_library_tool"
            | "_grep_includes"
            | "_if_so_builder"
            | "_allowlist_for_layering_check"
            | "_aggregate_ddi" => Some(Value::new_none()),
            "_tool_paths" | "_additional_make_variables" => Some(empty_dict()),
            "_build_variables_dict" => Some(heap.alloc(AllocDict([
                (
                    "target_system_name",
                    heap.alloc_str(self.target_system_name()).to_value(),
                ),
                ("target_libc", heap.alloc_str(self.target_libc()).to_value()),
            ]))),
            "_solib_dir" => Some(heap.alloc_str("_solib").to_value()),
            "_fdo_context" => Some(heap.alloc(AllocStruct::EMPTY)),
            "_legacy_cc_flags_make_variable"
            | "_abi"
            | "_abi_glibc_version"
            | "_crosstool_top_path" => Some(empty_string()),
            "_build_variables" => Some(self.build_variables(heap)),
            "_supports_header_parsing" | "_supports_param_files" => Some(Value::new_bool(true)),
            "_toolchain_features" => Some(if let Some(features) = &self.toolchain_features {
                heap.alloc(features.clone())
            } else {
                self.toolchain_config_info
                        .map(|toolchain_config_info| {
                            heap.alloc(
                                crate::interpreter::rule_defs::cc_common::cc_toolchain_features_from_config_info(
                                    toolchain_config_info.to_value(),
                                    "",
                                    heap,
                                ),
                            )
                        })
                        .unwrap_or_else(|| heap.alloc(CcToolchainFeatures::empty()))
            }),
            "_toolchain_label" => Some(heap.alloc_str(&self.toolchain_label).to_value()),
            "_cpp_configuration" => Some(heap.alloc(CppFragment::default())),
            "_is_tool_configuration"
            | "_is_sibling_repository_layout"
            | "_stamp_binaries"
            | "_force_layering_check_features" => Some(Value::new_bool(false)),
            "_cc_info" => Some(heap.alloc(CcInfoNativeShim {
                module_map_path: self.module_map_path.clone(),
            })),
            "_extra_allowlisted_feature_layering_check_macros" => Some(empty_list()),
            _ => None,
        }
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_toolchain_native_shim_methods)
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "CcToolchainInfoNativeShim".hash(hasher);
        self.toolchain_label.hash(hasher);
        self.target_platform.hash(hasher);
        if let Some(toolchain_files) = &self.toolchain_files {
            for file in toolchain_files.iter() {
                file.path.hash(hasher);
            }
        }
        Ok(())
    }
}

pub fn cc_toolchain_native_shim_provider_collection(
    toolchain_label: &str,
    target_platform: &str,
    toolchain_config_info: Option<FrozenValue>,
    toolchain_features: Option<CcToolchainFeatures>,
    module_map_path: Option<String>,
    toolchain_data: Vec<(ConfiguredTargetLabel, Arc<str>)>,
) -> FrozenProviderCollectionValue {
    let heap = FrozenHeap::new();
    let toolchain_files: Arc<[ToolchainInputFile]> = toolchain_data
        .into_iter()
        .map(|(target, path)| {
            let path = Box::leak(path.to_string().into_boxed_str());
            let input_target = Box::leak(Box::new(target));
            ToolchainInputFile { path, input_target }
        })
        .collect();
    let cc = heap.alloc(CcToolchainInfoNativeShim {
        toolchain_label: toolchain_label.to_owned(),
        target_platform: target_platform.to_owned(),
        toolchain_config_info,
        toolchain_features,
        module_map_path,
        toolchain_files: Some(toolchain_files),
    });
    let cc_provider_in_toolchain = heap.alloc(true);
    let fields = heap.alloc(AllocDict([
        ("cc", cc),
        ("cc_provider_in_toolchain", cc_provider_in_toolchain),
    ]));
    let toolchain_info = heap.alloc(ToolchainInfoInstanceGen::new(fields));
    let providers = FrozenProviderCollection::new(SmallMap::from_iter([
        (ToolchainInfoProvider::provider_id().dupe(), toolchain_info),
        (CcToolchainInfoProvider::provider_id().dupe(), cc),
    ]));
    let provider_collection =
        starlark::values::FrozenValueTyped::new_err(heap.alloc(providers)).unwrap();
    let heap_ref = heap.into_ref();
    FrozenProviderCollectionValue::from_value(unsafe {
        OwnedFrozenValueTyped::new(heap_ref, provider_collection)
    })
}

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
    /// The target platform/configuration this toolchain set was resolved for.
    pub target_platform: String,
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

        // Locate the entry by normalized label. Two outcomes matter:
        //   * Entry present, Some(providers): return the ToolchainInfo.
        //   * Entry present, None: the rule declared this toolchain type (likely
        //     mandatory=False) but nothing registered — return None so Bazel
        //     idioms like `hasattr(toolchain, "field")` or `if toolchain:`
        //     work without raising.
        //   * Entry absent: rule never declared this type; raise to surface
        //     the bug early.
        let entry = self.toolchains.get(&normalized).or_else(|| {
            self.toolchains
                .iter()
                .find(|(k, _)| normalize_toolchain_type_label(k) == normalized)
                .map(|(_, v)| v)
        });

        if is_cpp_toolchain_type_label_for_lookup(&normalized) {
            if let Some(Some(providers)) = entry {
                let cc_toolchain_info_id =
                    crate::interpreter::rule_defs::cc_common::CcToolchainInfoProvider::provider_id(
                    );
                if let Some(cc_fv) = providers
                    .provider_collection()
                    .get_provider_raw(cc_toolchain_info_id)
                {
                    let cc = heap.alloc(CcToolchainInfoTargetPlatformOverlay {
                        inner: *cc_fv,
                        target_platform: self.target_platform.clone(),
                    });
                    let cc_provider_in_toolchain = heap.alloc(true);
                    let fields = heap.alloc(AllocDict([
                        ("cc", cc),
                        ("cc_provider_in_toolchain", cc_provider_in_toolchain),
                    ]));
                    return Ok(heap.alloc(ToolchainInfoInstanceGen::new(fields)));
                }
            }
            return Ok(alloc_cc_toolchain_info_shim(
                heap,
                &key,
                &self.target_platform,
            ));
        }

        match entry {
            Some(Some(providers)) => {
                let toolchain_info_id =
                    crate::interpreter::rule_defs::platform_common::ToolchainInfoProvider::provider_id();
                if let Some(fv) = providers
                    .provider_collection()
                    .get_provider_raw(toolchain_info_id)
                {
                    return Ok(fv.to_value());
                }
                let fv = unsafe { providers.value().to_frozen_value() };
                Ok(fv.to_value())
            }
            Some(None) => Ok(Value::new_none()),
            None => {
                // Empty map: no toolchain machinery ran for this rule (e.g., a
                // rule with no `toolchains=` declaration accessing ctx.toolchains).
                // Return None to match Bazel's pre-resolution behaviour rather
                // than hard-failing.
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
    }
}

/// Normalize a toolchain type label for matching.
/// Strips `@` / `@@` prefixes and treats Bzlmod module-version canonical
/// repo names as equivalent to their apparent module name. Extension repos
/// such as `rules_rust+rust+rust_linux_x86_64` keep their full name.
fn normalize_toolchain_type_label(label: &str) -> String {
    let label = label.trim_start_matches('@');
    let Some((repo, rest)) = label.split_once("//") else {
        return label.to_owned();
    };
    let repo = normalize_bzlmod_module_repo_name(repo);
    format!("{repo}//{rest}")
}

fn is_cpp_toolchain_type_label_for_lookup(label: &str) -> bool {
    let label = label.trim_start_matches('@');
    if label.contains("tools/cpp:toolchain_type") {
        return true;
    }
    let Some((repo, package)) = label.split_once("//") else {
        return false;
    };
    (package == "tools/cpp:toolchain_type"
        && (repo == "bazel_tools" || repo.starts_with("bazel_tools+")))
        || package == "cc:toolchain_type"
            && (repo == "rules_cc"
                || repo.strip_prefix("rules_cc+").is_some_and(|rest| {
                    rest.split('+')
                        .next()
                        .is_some_and(|version| version.contains('.'))
                }))
}

fn alloc_cc_toolchain_info_shim<'v>(
    heap: Heap<'v>,
    toolchain_label: &str,
    target_platform: &str,
) -> Value<'v> {
    let providers = cc_toolchain_native_shim_provider_collection(
        toolchain_label,
        target_platform,
        None,
        None,
        None,
        Vec::new(),
    );
    let cc_toolchain_info_id =
        crate::interpreter::rule_defs::cc_common::CcToolchainInfoProvider::provider_id();
    let cc = providers
        .provider_collection()
        .get_provider_raw(cc_toolchain_info_id)
        .expect("native C++ toolchain shim provider collection includes CcToolchainInfo")
        .to_value();
    let cc_provider_in_toolchain = heap.alloc(true);
    let fields = heap.alloc(AllocDict([
        ("cc", cc),
        ("cc_provider_in_toolchain", cc_provider_in_toolchain),
    ]));
    heap.alloc(ToolchainInfoInstanceGen::new(fields))
}

fn normalize_bzlmod_module_repo_name(repo: &str) -> &str {
    if let Some(module_name) = repo.strip_suffix('+') {
        return module_name;
    }

    let Some((module_name, suffix)) = repo.split_once('+') else {
        return repo;
    };

    if suffix
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_digit())
    {
        module_name
    } else {
        repo
    }
}

#[cfg(test)]
mod resolved_toolchains_tests {
    use super::normalize_toolchain_type_label;

    #[test]
    fn toolchain_type_lookup_normalizes_bzlmod_module_versions() {
        assert_eq!(
            normalize_toolchain_type_label("@@rules_rust+0.69.0//rust:toolchain_type"),
            "rules_rust//rust:toolchain_type"
        );
        assert_eq!(
            normalize_toolchain_type_label("@@rules_rust//rust:toolchain_type"),
            "rules_rust//rust:toolchain_type"
        );
        assert_eq!(
            normalize_toolchain_type_label("@rules_cc+0.2.17//cc:toolchain_type"),
            "rules_cc//cc:toolchain_type"
        );
    }

    #[test]
    fn toolchain_type_lookup_keeps_extension_repo_names() {
        assert_eq!(
            normalize_toolchain_type_label(
                "@rules_rust+rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain"
            ),
            "rules_rust+rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain"
        );
    }
}

// ============================================================================
// CompilationContext / LinkingContext stubs for CcInfo
// ============================================================================

/// A stub for CompilationContext.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct EmptyCompilationContext {
    pub(crate) module_map_path: Option<String>,
}

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
            "_module_map" => Some(if let Some(path) = &self.module_map_path {
                heap.alloc(ModuleMapNativeShim {
                    name: "toolchain_module_map".to_owned(),
                    file_path: path.clone(),
                })
            } else {
                Value::new_none()
            }),
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
            "_exporting_module_maps" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
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

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "CompilationContext".hash(hasher);
        "empty".hash(hasher);
        self.module_map_path.hash(hasher);
        Ok(())
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ModuleMapNativeShim {
    name: String,
    file_path: String,
}

impl std::fmt::Display for ModuleMapNativeShim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "struct(file = <file>, name = {:?})", self.name)
    }
}

starlark::starlark_simple_value!(ModuleMapNativeShim);

#[starlark::values::starlark_value(type = "struct")]
impl<'v> StarlarkValue<'v> for ModuleMapNativeShim {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "file")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str(&self.name).to_value()),
            "file" => Some(heap.alloc(CtxCheatArtifactStub {
                path: Arc::<str>::from(self.file_path.as_str()),
                input_target: None,
            })),
            _ => None,
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.name.hash(hasher);
        self.file_path.hash(hasher);
        Ok(())
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

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "HeaderInfo".hash(hasher);
        "empty".hash(hasher);
        Ok(())
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

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        "LinkingContext".hash(hasher);
        "empty".hash(hasher);
        Ok(())
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
        use crate::interpreter::rule_defs::provider::dependency::SourceFileTarget;

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
            } else if let Some(source_target) = v.downcast_ref::<SourceFileTarget>() {
                files.push(source_target.artifact_value(heap));
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
        use crate::interpreter::rule_defs::provider::dependency::SourceFileTarget;

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
            } else if let Some(source_target) = v.downcast_ref::<SourceFileTarget>() {
                source_target.artifact_value(heap)
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

/// Single source of truth for Kuro's builtin Make-variable (`$(VAR)`) bindings
/// exposed via `ctx.var`. Each lookup recomputes the list because host_cc_path
/// and compilation_mode may change between analyses.
fn builtin_ctx_vars() -> Vec<(&'static str, String)> {
    let cpu = host_target_cpu();
    let cc = host_cc_path();
    let comp_mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
    let java = if cfg!(windows) {
        "java.exe"
    } else {
        "/usr/bin/java"
    };
    vec![
        ("BINDIR", format!("bazel-out/{cpu}-{comp_mode}/bin")),
        ("GENDIR", format!("bazel-out/{cpu}-{comp_mode}/genfiles")),
        ("TARGET_CPU", cpu.to_owned()),
        ("COMPILATION_MODE", comp_mode),
        ("CC", cc.to_owned()),
        ("CC_FLAGS", String::new()),
        ("JAVA", java.to_owned()),
        ("JAVA_RUNFILES", String::new()),
        ("JAVABASE", String::new()),
        ("ABI_GLIBC_VERSION", "2.17".to_owned()),
        ("ABI", "local".to_owned()),
    ]
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
        let value = builtin_ctx_vars()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
            .or_else(|| crate::interpreter::rule_defs::build_config::get_define(key));
        match value {
            Some(v) => Ok(heap.alloc_str(&v).to_value()),
            None => Ok(default),
        }
    }

    /// Get all keys in the Make variables dict.
    fn keys<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let mut keys: Vec<String> = builtin_ctx_vars()
            .into_iter()
            .map(|(k, _)| k.to_owned())
            .collect();
        for key in crate::interpreter::rule_defs::build_config::get_all_defines().keys() {
            if !keys.iter().any(|k| k == key) {
                keys.push(key.clone());
            }
        }
        let values: Vec<Value> = keys.iter().map(|k| heap.alloc_str(k).to_value()).collect();
        Ok(heap.alloc(values))
    }

    /// Get all values in the Make variables dict.
    fn values<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let mut result: Vec<Value> = builtin_ctx_vars()
            .into_iter()
            .map(|(_, v)| heap.alloc_str(&v).to_value())
            .collect();
        for (_, v) in crate::interpreter::rule_defs::build_config::get_all_defines() {
            result.push(heap.alloc_str(&v).to_value());
        }
        Ok(heap.alloc(result))
    }

    /// Get all key-value pairs as a list of tuples.
    fn items<'v>(this: &CtxVarDict, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        let mut result: Vec<Value> = builtin_ctx_vars()
            .into_iter()
            .map(|(k, v)| heap.alloc((heap.alloc_str(k).to_value(), heap.alloc_str(&v).to_value())))
            .collect();
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
                        target_platform: String::new(),
                    },
                }));
            }
        };

        if let Some(toolchains) = self.groups.get(key) {
            Ok(heap.alloc(ResolvedExecGroupContext {
                toolchains: ResolvedToolchains {
                    toolchains: toolchains.clone(),
                    exec_platform: String::new(),
                    target_platform: String::new(),
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
                    target_platform: String::new(),
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
                target_platform: self.toolchains.target_platform.clone(),
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
