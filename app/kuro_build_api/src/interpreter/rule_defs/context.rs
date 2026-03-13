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
use crate::interpreter::rule_defs::artifact::methods::ArtifactRootStub;
use crate::interpreter::rule_defs::bazel_label::BazelLabel;
use crate::interpreter::rule_defs::cc_common::CcToolchainInfoProvider;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::fragments::CppFragment;
use crate::interpreter::rule_defs::plugins::AnalysisPlugins;
use crate::interpreter::rule_defs::provider::ProviderLike;

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

        let analysis_context = Self::new(
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
    fn split_attr<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        match this.0.attrs {
            Some(attrs) => Ok(heap.alloc(CtxSplitAttr::new(attrs))),
            None => Ok(heap.alloc(CtxSplitAttrStub)),
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
    /// Returns a stub that pretends to contain all toolchain types and returns
    /// stub toolchain info when indexed. This allows rules_cc to proceed with
    /// toolchain-based builds.
    ///
    /// TODO(toolchains): Implement proper toolchain resolution.
    #[starlark(attribute)]
    fn toolchains<'v>(this: RefAnalysisContext<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let is_tool = this.0.is_tool_configuration();
        Ok(heap.alloc(ToolchainsStub { is_tool }))
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
        Ok(heap.alloc(BuildConfigurationStub { is_tool }))
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
                Ok(heap.alloc(CtxFilesStub))
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
                Ok(heap.alloc(CtxFileStub))
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
                Ok(heap.alloc(CtxExecutableStub))
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
    /// TODO(bazel): Implement proper toolchain resolution for exec groups.
    #[starlark(attribute)]
    fn exec_groups<'v>(
        this: RefAnalysisContext<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(ExecGroupsDict))
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
            ("JAVA", if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }),
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
        let allows_multiple = this.0.attrs
            .and_then(|attrs| {
                attrs.get()
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

            if let Some(cli_value) = crate::interpreter::rule_defs::build_config::get_starlark_flag(&label_str) {
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
            ("JAVA", if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }),
            ("JAVA_RUNFILES", ""),
            ("JAVABASE", ""),
            ("ABI_GLIBC_VERSION", "2.17"),
            ("ABI", "local"),
        ];
        for (k, v) in builtins {
            substitutions.entry(k.to_string()).or_insert_with(|| v.to_string());
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
        let cmd_list = heap.alloc(AllocList(vec![heap.alloc_str(&resolved_command).to_value()]));
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
            starlark::Error::new_other(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
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
        #[starlark(require = named, default = NoneType)] targets: Value<'v>,
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
            let pattern: Option<(usize, bool)> =
                if remaining.starts_with("$(locations ") {
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

/// Returns the tool path for a given tool name, appropriate for the host platform.
///
/// On Windows with MSVC, tools like strip/objcopy/gcov don't have MSVC equivalents,
/// so we fall back to the bare tool name (works if LLVM tools are on PATH).
/// On Unix, paths default to /usr/bin/<tool>.
pub fn host_tool_path(tool_name: &str) -> &'static str {
    match std::env::consts::OS {
        "windows" => match tool_name {
            "gcc" | "g++" | "cpp" => "cl.exe",
            "ar" => "lib.exe",
            "ld" => "link.exe",
            "nm" | "objdump" => "dumpbin.exe",
            // These tools don't have MSVC equivalents. Return the bare name
            // so they can be found on PATH (e.g., if LLVM tools are installed).
            "objcopy" => "llvm-objcopy",
            "strip" => "strip",
            "gcov" => "gcov",
            "dwp" => "dwp",
            "llvm-profdata" => "llvm-profdata",
            "llvm-cov" => "llvm-cov",
            _ => "", // Unknown tools
        },
        "macos" => match tool_name {
            "gcc" | "g++" | "cpp" => "/usr/bin/clang",
            "ar" => "/usr/bin/ar",
            "ld" => "/usr/bin/ld",
            "nm" => "/usr/bin/nm",
            "objcopy" => "/usr/bin/objcopy",
            "objdump" => "/usr/bin/objdump",
            "strip" => "/usr/bin/strip",
            "gcov" => "/usr/bin/gcov",
            "dwp" => "/usr/bin/dwp",
            "llvm-profdata" => "/usr/bin/llvm-profdata",
            "llvm-cov" => "/usr/bin/llvm-cov",
            _ => "", // Unknown tools
        },
        _ => match tool_name {
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
            "llvm-cov" => "/usr/bin/llvm-cov",
            _ => "", // Unknown tools
        },
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
                self.declared.borrow_mut().insert(attribute.to_owned(), result);
                return Some(result);
            }
        }

        // Determine the filename for this output attribute.
        // Case 1: attr.output() - the attribute value in attrs is the filename string
        // Case 2: rule(outputs={...}) pattern - expand using target name
        // Case 3: well-known fallback patterns
        let filename_owned: String;
        let filename: &str = if let Some(v) = raw_attr_val
            .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
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
pub struct ToolchainsStub {
    pub is_tool: bool,
}

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

    /// Returns a toolchain stub when indexed.
    /// Checks the key to determine which kind of toolchain to return.
    /// Returns None for unrecognized toolchain types.
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Get the key string - handle both str and Label objects.
        // Label("//rust:toolchain_type") is a BazelLabel, not a str, so unpack_str() returns None.
        // Using format!("{}", index) gives the Display representation:
        //   - for str: includes quotes (e.g., `"@rules_rust//rust:toolchain_type"`) but we use unpack_str first
        //   - for BazelLabel: returns the raw label string (e.g., `@rules_rust//rust:toolchain_type`)
        let owned_str;
        let key_str = if let Some(s) = index.unpack_str() {
            s
        } else {
            owned_str = format!("{}", index);
            &owned_str
        };
        // Extract target name (after :) and package path (before :) for matching
        let (pkg_path, target_name) = key_str.rsplit_once(':').unwrap_or(("", key_str));
        // Extract the last package segment (e.g., "cpp" from "//tools/cpp")
        let pkg_segment = pkg_path
            .rsplit_once('/')
            .map(|(_, seg)| seg)
            .unwrap_or(pkg_path);
        if target_name == "rust_toolchain"
            || target_name == "rustfmt_toolchain"
            || (target_name == "toolchain_type" && pkg_segment == "rust")
        {
            Ok(heap.alloc(RustToolchainInfoStub))
        } else if target_name == "toolchain_type" && (pkg_segment == "cc" || pkg_segment == "cpp") {
            Ok(heap.alloc(CcToolchainInfoStub { is_tool: self.is_tool }))
        } else if target_name == "toolchain_type" && (pkg_segment == "java" || key_str.contains("rules_java")) {
            // Java toolchain: ctx.toolchains["@rules_java//java:toolchain_type"].java
            Ok(heap.alloc(JavaToolchainWrapperStub))
        } else if target_name == "runtime_toolchain_type" && (pkg_segment == "java" || key_str.contains("rules_java")) {
            // Java runtime toolchain: ctx.toolchains["@rules_java//java:runtime_toolchain_type"].java_runtime
            Ok(heap.alloc(JavaRuntimeToolchainWrapperStub))
        } else if target_name == "toolchain_type"
            && key_str.contains("python")
            && !key_str.contains("exec_tools")
        {
            // Python target toolchain - provide a stub with py3_runtime
            Ok(heap.alloc(PyToolchainInfoStub))
        } else if target_name == "crane_toolchain_type" {
            // OCI crane toolchain
            let crane_path = detect_crane_path();
            Ok(heap.alloc(OciCraneToolchainStub { crane_path }))
        } else if target_name == "registry_toolchain_type" {
            // OCI registry toolchain (uses crane registry serve)
            let crane_path = detect_crane_path();
            let launcher_path = get_or_create_oci_launcher();
            Ok(heap.alloc(OciRegistryToolchainStub {
                launcher_path,
                registry_path: crane_path,
            }))
        } else if target_name == "jq_toolchain_type" {
            // jq toolchain
            let jq_path = detect_jq_path();
            Ok(heap.alloc(JqToolchainStub { jq_path }))
        } else if target_name == "test_runner_toolchain_type"
            || (target_name == "toolchain_type" && pkg_segment == "test_runner")
        {
            // Test runner toolchain - return a generic toolchain with test runner info
            Ok(heap.alloc(GenericToolchainStub))
        } else if key_str.contains("exec_tools") {
            // exec_tools toolchains (e.g., rules_python exec_tools, rules_rust exec_tools)
            // These provide tools for execution, return a generic toolchain
            Ok(heap.alloc(GenericToolchainStub))
        } else if target_name == "toolchain_type"
            || target_name.ends_with("_toolchain_type")
            || target_name.ends_with("_toolchain")
        {
            // Generic toolchain type - return a stub that won't crash on attribute access
            Ok(heap.alloc(GenericToolchainStub))
        } else {
            // For unrecognized toolchain keys, return a generic stub rather than None.
            // This prevents AttributeError crashes when rules access toolchain attributes.
            tracing::warn!(
                "ctx.toolchains[{:?}]: unrecognized toolchain type, returning stub. \
                 Attribute accesses on this toolchain will return None.",
                key_str
            );
            Ok(heap.alloc(GenericToolchainStub))
        }
    }
}

/// A stub for CcToolchainInfo that provides minimal toolchain info.
///
/// This provides the attributes that rules_cc expects from a CcToolchainInfo provider.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainInfoStub {
    is_tool: bool,
}

impl std::fmt::Display for CcToolchainInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CcToolchainInfo>")
    }
}

starlark::starlark_simple_value!(CcToolchainInfoStub);

impl<'v> ProviderLike<'v> for CcToolchainInfoStub {
    fn id(&self) -> &Arc<ProviderId> {
        CcToolchainInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, starlark::values::Value<'v>)> {
        vec![]
    }
}

#[starlark::values::starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_toolchain_info_stub_methods)
    }

    fn provide(&'v self, demand: &mut starlark::values::Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        // Report which attributes exist for hasattr() checks
        matches!(
            attribute,
            "cc" | "cc_provider_in_toolchain"
                | "toolchain_id"
                | "compiler"
                | "cpu"
                | "target_gnu_system_name"
                | "dynamic_runtime_lib"
                | "static_runtime_lib"
                | "sysroot"
                | "all_files"
                | "compiler_files"
                | "linker_files"
                | "ar_files"
                | "objcopy_files"
                | "strip_files"
                | "gcov_files"
                | "_supports_header_parsing"
                | "_needs_pic_for_dynamic_libraries"
                | "_use_pic_for_dynamic_libraries_not_for_binaries"
                | "_supports_start_end_lib"
                | "_feature_configuration"
                | "_tool_paths"
                | "libc"
                | "_abi_glibc_version"
                | "_abi"
                | "_crosstool_top_path"
                | "_legacy_cc_flags_make_variable"
                | "_build_variables"
                | "_coverage_files"
                | "_strip_files"
                | "_cpp_configuration"
                | "_if_so_builder"
                | "_solib_dir"
                | "_build_variables_dict"
                | "_ar_files"
                | "_linker_files"
                | "_supports_param_files"
                | "_stamp_binaries"
                | "_is_tool_configuration"
                | "_is_sibling_repository_layout"
                | "_static_runtime_lib_depset"
                | "_dynamic_runtime_lib_depset"
                | "_compiler_files_without_includes"
                | "_as_files"
                | "_dwp_files"
                | "_builtin_include_files"
                | "_additional_make_variables"
                | "_all_files_including_libc"
                | "_build_info_files"
                | "_allowlist_for_layering_check"
                | "_cc_info"
                | "_objcopy_files"
                | "_aggregate_ddi"
                | "_toolchain_label"
                | "_link_dynamic_library_tool"
                | "_grep_includes"
                | "_compiler_files"
                | "built_in_include_directories"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "cc_provider_in_toolchain" => Some(Value::new_bool(false)),
            "toolchain_id" => Some(heap.alloc_str("local_cc_toolchain").to_value()),
            "compiler" => {
                let compiler = if cfg!(windows) { "msvc" } else if cfg!(target_os = "macos") { "clang" } else { "gcc" };
                Some(heap.alloc_str(compiler).to_value())
            }
            "cpu" => Some(heap.alloc_str(host_target_cpu()).to_value()),
            "target_gnu_system_name" => {
                let name = if cfg!(windows) {
                    if cfg!(target_arch = "aarch64") { "aarch64-w64-windows-msvc" } else { "x86_64-w64-windows-msvc" }
                } else if cfg!(target_os = "macos") {
                    if cfg!(target_arch = "aarch64") { "aarch64-apple-darwin" } else { "x86_64-apple-darwin" }
                } else {
                    if cfg!(target_arch = "aarch64") { "aarch64-linux-gnu" } else { "x86_64-linux-gnu" }
                };
                Some(heap.alloc_str(name).to_value())
            }
            "sysroot" => Some(Value::new_none()),
            "_supports_header_parsing" => Some(Value::new_bool(true)),
            "_needs_pic_for_dynamic_libraries" => Some(Value::new_bool(!cfg!(windows))),
            "_use_pic_for_dynamic_libraries_not_for_binaries" => Some(Value::new_bool(false)),
            "_supports_start_end_lib" => Some(Value::new_bool(false)),
            "_cc_info" => Some(heap.alloc(CcInfoStub)),
            "_tool_paths" => Some(heap.alloc(ToolPathsStub)),
            "_toolchain_features" => Some(heap.alloc(ToolchainFeaturesStub)),
            "_is_tool_configuration" => Some(Value::new_bool(false)),
            "_fdo_context" => Some(Value::new_none()),
            "libc" => {
                let libc = if cfg!(windows) { "msvcrt" } else if cfg!(target_os = "macos") { "macosx" } else { "glibc" };
                Some(heap.alloc_str(libc).to_value())
            }
            "_abi_glibc_version" => Some(heap.alloc_str("2.17").to_value()),
            "_abi" => Some(heap.alloc_str("local").to_value()),
            "_crosstool_top_path" => Some(heap.alloc_str("external/local_config_cc").to_value()),
            "_legacy_cc_flags_make_variable" => Some(heap.alloc_str("").to_value()),
            "_build_variables" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_coverage_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_strip_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_cpp_configuration" => Some(heap.alloc(CppFragment::default())),
            "_if_so_builder" => Some(Value::new_none()),
            "_solib_dir" => {
                let solib = if cfg!(target_arch = "aarch64") { "_solib_aarch64" } else { "_solib_k8" };
                Some(heap.alloc_str(solib).to_value())
            }
            "_build_variables_dict" => {
                let map: SmallMap<Value, Value> = SmallMap::new();
                Some(heap.alloc(Dict::new(map)))
            }
            "_ar_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_linker_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_supports_param_files" => Some(Value::new_bool(true)),
            "_stamp_binaries" => Some(Value::new_bool(
                crate::interpreter::rule_defs::build_config::get_stamp(),
            )),
            "_is_sibling_repository_layout" => Some(Value::new_bool(false)),
            "_static_runtime_lib_depset" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_dynamic_runtime_lib_depset" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_compiler_files_without_includes" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_as_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "_dwp_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_builtin_include_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_additional_make_variables" => {
                let map: SmallMap<Value, Value> = SmallMap::new();
                Some(heap.alloc(Dict::new(map)))
            }
            "_all_files_including_libc" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_build_info_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_allowlist_for_layering_check" => Some(Value::new_none()),
            "_objcopy_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_aggregate_ddi" => Some(Value::new_none()),
            "_toolchain_label" => Some(
                heap.alloc_str("@bazel_tools//tools/cpp:toolchain")
                    .to_value(),
            ),
            "_link_dynamic_library_tool" => Some(Value::new_none()),
            "_grep_includes" => Some(Value::new_none()),
            "_compiler_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "gcov_files" | "_gcov_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "built_in_include_directories" => {
                // Return list of built-in include directories from the system compiler.
                // On Windows (MSVC), return MSVC and Windows SDK include paths.
                // On Unix, return standard system include paths.
                let dirs: Vec<Value<'v>> = if cfg!(windows) {
                    crate::interpreter::rule_defs::cc_common::get_msvc_include_dirs()
                        .iter()
                        .map(|d| heap.alloc_str(d).to_value())
                        .collect()
                } else {
                    ["/usr/include", "/usr/local/include"]
                        .iter()
                        .map(|d| heap.alloc_str(d).to_value())
                        .collect()
                };
                Some(heap.alloc(dirs))
            }
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
            "built_in_include_directories".to_owned(),
        ]
    }
}

#[starlark_module]
fn cc_toolchain_info_stub_methods(builder: &mut MethodsBuilder) {
    /// The C++ toolchain provider itself (for cc_provider_in_toolchain pattern).
    #[starlark(attribute)]
    fn cc<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcToolchainInfoStub { is_tool: this.is_tool }))
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
        Ok(this.is_tool)
    }

    /// Compiler type.
    #[starlark(attribute)]
    fn compiler(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        if cfg!(windows) {
            Ok("msvc")
        } else if cfg!(target_os = "macos") {
            Ok("clang")
        } else {
            Ok("gcc")
        }
    }

    /// Target CPU architecture.
    #[starlark(attribute)]
    fn cpu(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        Ok(host_target_cpu())
    }

    /// GNU system name for the target.
    #[starlark(attribute)]
    fn target_gnu_system_name(this: &CcToolchainInfoStub) -> starlark::Result<&'static str> {
        let _ = this;
        if cfg!(windows) {
            if cfg!(target_arch = "aarch64") {
                Ok("aarch64-w64-windows-msvc")
            } else {
                Ok("x86_64-w64-windows-msvc")
            }
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                Ok("aarch64-apple-darwin")
            } else {
                Ok("x86_64-apple-darwin")
            }
        } else {
            if cfg!(target_arch = "aarch64") {
                Ok("aarch64-linux-gnu")
            } else {
                Ok("x86_64-linux-gnu")
            }
        }
    }

    /// All input files for the toolchain.
    #[starlark(attribute)]
    fn all_files<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
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
    fn linker_files<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Archiver input files.
    #[starlark(attribute)]
    fn ar_files<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Strip input files.
    #[starlark(attribute)]
    fn strip_files<'v>(this: &CcToolchainInfoStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
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
        let _ = (this, feature_configuration);
        // MSVC doesn't use PIC; Unix does
        Ok(!cfg!(windows))
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
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
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

/// Provides access to files from label/label_list attributes as ctx.files.<attr>.
///
/// In Bazel, `ctx.files.foo` returns a list of Files from attribute `foo`.
/// This implementation extracts files from the resolved attribute values.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxFiles<'v> {
    /// The resolved attributes struct as a Value
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
        // Check if the attribute exists in attrs using StarlarkValue method
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

        // Get the attribute value from attrs
        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;

        if attr_value.is_none() {
            return Some(heap.alloc(AllocList::EMPTY));
        }

        // Helper: extract default output files from a dependency value
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
                // Not a dependency - include as-is (already an artifact)
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
            // Single value (e.g. single label attribute)
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
///
/// In Bazel, `ctx.file.foo` returns a single File from attribute `foo`.
/// If the attribute contains multiple files, this is an error in Bazel,
/// but for compatibility we return the first file.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxFile<'v> {
    /// The resolved attributes struct as a Value
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

        // Extract the first file from a dependency or artifact value
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

        // If it's a list, get first element and extract file from it
        if let Some(list) = ListRef::from_value(attr_value) {
            if list.is_empty() {
                return Some(Value::new_none());
            }
            return Some(extract_first_file(list.content()[0]));
        }

        // Single value - extract file from it
        Some(extract_first_file(attr_value))
    }
}

impl<'v> AllocValue<'v> for CtxFile<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// Provides access to executable files from label attributes as ctx.executable.<attr>.
///
/// In Bazel, `ctx.executable.foo` returns the executable File from attribute `foo`
/// (which must be declared with `executable=True`).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxExecutable<'v> {
    /// The resolved attributes struct as a Value
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

        // Get the attribute value from attrs
        // Value::get_attr returns Result<Option<Value>, Error>, we ignore errors
        let attr_value = self.attrs.get_attr(attribute, heap).ok().flatten()?;

        // If it's None, return None
        if attr_value.is_none() {
            return Some(Value::new_none());
        }

        // Helper: extract first executable file from a dependency's DefaultInfo
        let extract_executable = |v: Value<'v>| -> Option<Value<'v>> {
            let pc = if let Some(dep) = v.downcast_ref::<Dependency>() {
                Some(dep.provider_collection())
            } else if let Some(dep) = v.downcast_ref::<FrozenDependency>() {
                Some(dep.provider_collection())
            } else {
                None
            };
            if let Some(pc) = pc {
                // Try to get the executable from DefaultInfo
                if let Ok(di) = pc.default_info() {
                    // Check DefaultInfo.executable first
                    if let Some(exe) = di.executable() {
                        return Some(heap.alloc(exe));
                    }
                    // Fall back to first default output
                    let raw = di.default_outputs_raw();
                    if let Some(list) = ListRef::from_frozen_value(raw) {
                        if !list.is_empty() {
                            return Some(list.content()[0]);
                        }
                    }
                }
                return Some(Value::new_none());
            }
            // Not a dependency - return as-is (already an artifact)
            Some(v)
        };

        // Handle list attributes (list of deps)
        if let Some(list) = ListRef::from_value(attr_value) {
            if list.is_empty() {
                return Some(Value::new_none());
            }
            return extract_executable(list.content()[0]);
        }

        // Handle single dependency
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
pub struct CtxSplitAttrStub;

impl std::fmt::Display for CtxSplitAttrStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<ctx.split_attr>")
    }
}

starlark::starlark_simple_value!(CtxSplitAttrStub);

#[starlark::values::starlark_value(type = "ctx_split_attr_stub")]
impl<'v> StarlarkValue<'v> for CtxSplitAttrStub {
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
/// Used as fallback when attrs is not available.
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
            "JAVA" => if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }.to_owned(),
            "JAVA_RUNFILES" => String::new(),
            "JAVABASE" => String::new(),
            "ABI_GLIBC_VERSION" => "2.17".to_owned(),
            "ABI" => "local".to_owned(),
            _ => {
                // Check --define values for unknown keys
                crate::interpreter::rule_defs::build_config::get_define(key)
                    .unwrap_or_default()
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
            "JAVA" => Some(if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }.to_owned()),
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
            heap.alloc_str(&format!("bazel-out/{cpu}-{comp_mode}/bin")).to_value(),
            heap.alloc_str(&format!("bazel-out/{cpu}-{comp_mode}/genfiles")).to_value(),
            heap.alloc_str(cpu).to_value(),
            heap.alloc_str(&comp_mode).to_value(),
            heap.alloc_str(cc).to_value(),
            heap.alloc_str("").to_value(), // CC_FLAGS
            heap.alloc_str(if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }).to_value(),
            heap.alloc_str("").to_value(), // JAVA_RUNFILES
            heap.alloc_str("").to_value(), // JAVABASE
            heap.alloc_str("2.17").to_value(), // ABI_GLIBC_VERSION
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
            ("JAVA", if cfg!(windows) { "java.exe" } else { "/usr/bin/java" }.to_owned()),
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
/// - `short_id`: Opaque configuration fingerprint
/// - `test_env`: Dict from --test_env flags
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BuildConfigurationStub {
    pub is_tool: bool,
}

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
                let env_map =
                    crate::interpreter::rule_defs::build_config::get_action_env();
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
                // Opaque configuration fingerprint based on compilation mode + CPU
                let comp_mode =
                    crate::interpreter::rule_defs::build_config::get_compilation_mode();
                let cpu = host_target_cpu();
                Some(heap.alloc_str(&format!("{cpu}-{comp_mode}")).to_value())
            }
            "test_env" => {
                // Return --test_env values from build config
                let env_map =
                    crate::interpreter::rule_defs::build_config::get_test_env();
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
fn build_configuration_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &BuildConfigurationStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }

    /// Returns whether this is a tool configuration (exec configuration).
    /// Tool configurations are used for build tools that run on the host machine.
    fn is_tool_configuration(this: &BuildConfigurationStub) -> starlark::Result<bool> {
        Ok(this.is_tool)
    }

    /// Returns whether this configuration has a separate genfiles directory.
    /// In modern Bazel, this is always false (genfiles merged with bin directory).
    fn has_separate_genfiles_directory(this: &BuildConfigurationStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }
}

// ============================================================================
// ExecGroupsDict - Stub for ctx.exec_groups
// ============================================================================

/// A dict-like stub for `ctx.exec_groups`.
///
/// In Bazel, `ctx.exec_groups` is a dictionary mapping execution group names
/// to resolved exec group info objects. Each exec group has a `toolchains`
/// attribute that provides resolved toolchains.
///
/// This stub returns an `ExecGroupInfo` for any key lookup, which in turn
/// returns `None` for any toolchain lookup. This causes rules to take their
/// fallback/legacy code paths.
///
/// TODO(bazel): Implement proper exec group resolution with real toolchain lookup.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExecGroupsDict;

impl std::fmt::Display for ExecGroupsDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<exec_groups>")
    }
}

starlark::starlark_simple_value!(ExecGroupsDict);

#[starlark::values::starlark_value(type = "exec_groups")]
impl<'v> StarlarkValue<'v> for ExecGroupsDict {
    /// Returns an ExecGroupInfo for any key.
    fn at(&self, _index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(ExecGroupInfo))
    }

    /// Returns True for any key - pretends all exec groups are defined.
    fn is_in(&self, _other: Value<'v>) -> starlark::Result<bool> {
        Ok(true)
    }
}

/// A stub for a resolved execution group.
///
/// Provides a `toolchains` attribute that returns `None` for any toolchain
/// type lookup, causing rules to take their fallback code paths.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExecGroupInfo;

impl std::fmt::Display for ExecGroupInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<exec_group_info>")
    }
}

starlark::starlark_simple_value!(ExecGroupInfo);

#[starlark::values::starlark_value(type = "exec_group_info")]
impl<'v> StarlarkValue<'v> for ExecGroupInfo {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "toolchains" | "exec_compatible_with")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "toolchains" => Some(heap.alloc(ExecGroupToolchains)),
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

/// A stub for toolchains within an execution group.
///
/// Returns `None` for any toolchain type lookup, indicating that the toolchain
/// is not resolved. This is consistent with `mandatory = False` toolchains.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExecGroupToolchains;

impl std::fmt::Display for ExecGroupToolchains {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<exec_group_toolchains>")
    }
}

starlark::starlark_simple_value!(ExecGroupToolchains);

#[starlark::values::starlark_value(type = "exec_group_toolchains")]
impl<'v> StarlarkValue<'v> for ExecGroupToolchains {
    /// Returns toolchain stub for known types, None for unknown.
    /// Uses the same logic as ToolchainsStub to resolve known toolchain types.
    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Delegate to ToolchainsStub's resolution logic
        let stub = ToolchainsStub { is_tool: false };
        stub.at(index, heap)
    }

    /// Returns True for known toolchain types, False for unknown.
    fn is_in(&self, _other: Value<'v>) -> starlark::Result<bool> {
        // Return true so rules can access toolchains via exec groups
        Ok(true)
    }
}

// ============================================================================

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
        Ok(matches!(
            feature_name,
            "supports_dynamic_linker" | "static_linking_mode"
        ))
    }

    /// Check if a feature was requested by the user.
    fn is_requested<'v>(
        this: &FeatureConfigurationStub,
        #[starlark(require = pos)] _feature_name: &str,
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
        let path = host_tool_path(key);
        Ok(heap.alloc_str(path).to_value())
    }

    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // Return true if the key is a known tool
        if let Some(key) = other.unpack_str() {
            Ok(matches!(
                key,
                "gcc"
                    | "g++"
                    | "cpp"
                    | "ar"
                    | "ld"
                    | "nm"
                    | "objcopy"
                    | "objdump"
                    | "strip"
                    | "gcov"
                    | "dwp"
                    | "llvm-profdata"
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
        let path = host_tool_path(key);
        if path.is_empty() {
            return Ok(default);
        }
        Ok(heap.alloc_str(path).to_value())
    }
}

// ============================================================================
// RustToolchainInfoStub - Stub for Rust toolchain info
// ============================================================================

/// A stub for the Rust toolchain info returned by ctx.toolchains for Rust rules.
///
/// Provides the attributes that rules_rust expects from a Rust toolchain provider.
/// Returns sensible defaults for the local system's Rust installation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RustToolchainInfoStub;

impl std::fmt::Display for RustToolchainInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<RustToolchainInfo>")
    }
}

starlark::starlark_simple_value!(RustToolchainInfoStub);

/// A stub for a Rust triple (exec_triple, target_triple).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RustTripleStub {
    triple_str: &'static str,
}

impl std::fmt::Display for RustTripleStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.triple_str)
    }
}

starlark::starlark_simple_value!(RustTripleStub);

#[starlark::values::starlark_value(type = "rust_triple")]
impl<'v> StarlarkValue<'v> for RustTripleStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "str" | "arch" | "system" | "abi" | "vendor")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "str" => Some(heap.alloc_str(self.triple_str).to_value()),
            "arch" => Some(heap.alloc_str("x86_64").to_value()),
            "system" => Some(heap.alloc_str("linux").to_value()),
            "abi" => Some(heap.alloc_str("gnu").to_value()),
            "vendor" => Some(heap.alloc_str("unknown").to_value()),
            _ => None,
        }
    }
}

/// A stub for a Rust compilation mode options struct.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RustCompilationModeOptsStub;

impl std::fmt::Display for RustCompilationModeOptsStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<RustCompilationModeOpts>")
    }
}

starlark::starlark_simple_value!(RustCompilationModeOptsStub);

#[starlark::values::starlark_value(type = "rust_compilation_mode_opts")]
impl<'v> StarlarkValue<'v> for RustCompilationModeOptsStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "debug_info" | "opt_level")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "debug_info" => Some(heap.alloc_str("2").to_value()),
            "opt_level" => Some(heap.alloc_str("0").to_value()),
            _ => None,
        }
    }
}

#[starlark::values::starlark_value(type = "RustToolchainInfo")]
impl<'v> StarlarkValue<'v> for RustToolchainInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "rustc"
                | "rust_doc"
                | "rustfmt"
                | "cargo"
                | "clippy_driver"
                | "llvm_cov"
                | "llvm_profdata"
                | "rustc_lib"
                | "rust_std"
                | "rust_std_paths"
                | "all_files"
                | "binary_ext"
                | "staticlib_ext"
                | "dylib_ext"
                | "default_edition"
                | "exec_triple"
                | "target_triple"
                | "target_arch"
                | "target_os"
                | "target_flag_value"
                | "target_json"
                | "sysroot"
                | "sysroot_short_path"
                | "env"
                | "extra_rustc_flags"
                | "extra_exec_rustc_flags"
                | "per_crate_rustc_flags"
                | "compilation_mode_opts"
                | "stdlib_linkflags"
                | "libstd_and_allocator_ccinfo"
                | "libstd_and_global_allocator_ccinfo"
                | "nostd_and_global_allocator_cc_info"
                | "_rename_first_party_crates"
                | "_third_party_dir"
                | "_pipelined_compilation"
                | "_no_std"
                | "_experimental_link_std_dylib"
                | "_experimental_toolchain_generated_sysroot"
                | "_experimental_use_cc_common_link"
                | "_experimental_use_coverage_metadata_files"
                | "_experimental_use_global_allocator"
                | "_incompatible_no_rustc_sysroot_env"
                | "_incompatible_test_attr_crate_and_srcs_mutually_exclusive"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::AllocList;
        match attribute {
            // Tool paths - return file-like stubs with .path attribute
            "rustc" => Some(heap.alloc(RustToolStub("rustc"))),
            "rust_doc" => Some(heap.alloc(RustToolStub("rustdoc"))),
            "rustfmt" => Some(Value::new_none()),
            "cargo" => Some(Value::new_none()),
            "clippy_driver" => Some(Value::new_none()),
            "llvm_cov" => Some(Value::new_none()),
            "llvm_profdata" => Some(Value::new_none()),
            "rustc_lib" => Some(Value::new_none()),

            // Depsets
            "rust_std" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),
            "rust_std_paths" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "all_files" => Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())),

            // File extensions
            "binary_ext" => Some(heap.alloc_str("").to_value()),
            "staticlib_ext" => Some(heap.alloc_str(".a").to_value()),
            "dylib_ext" => Some(heap.alloc_str(".so").to_value()),

            // Edition and triples
            "default_edition" => Some(heap.alloc_str("2021").to_value()),
            "exec_triple" => Some(heap.alloc(RustTripleStub {
                triple_str: "x86_64-unknown-linux-gnu",
            })),
            "target_triple" => Some(heap.alloc(RustTripleStub {
                triple_str: "x86_64-unknown-linux-gnu",
            })),
            "target_arch" => Some(heap.alloc_str("x86_64").to_value()),
            "target_os" => Some(heap.alloc_str("linux").to_value()),
            "target_flag_value" => Some(heap.alloc_str("x86_64-unknown-linux-gnu").to_value()),
            "target_json" => Some(Value::new_none()),

            // Sysroot
            "sysroot" => Some(heap.alloc_str("").to_value()),
            "sysroot_short_path" => Some(heap.alloc_str("").to_value()),

            // Env and flags
            "env" => {
                let map: SmallMap<Value, Value> = SmallMap::new();
                Some(heap.alloc(Dict::new(map)))
            }
            "extra_rustc_flags" => Some(heap.alloc(AllocList::EMPTY)),
            "extra_exec_rustc_flags" => Some(heap.alloc(AllocList::EMPTY)),
            "per_crate_rustc_flags" => Some(heap.alloc(AllocList::EMPTY)),

            // Compilation mode opts - dict mapping mode names to option structs
            "compilation_mode_opts" => {
                let mut map: SmallMap<Value, Value> = SmallMap::new();
                let dbg = heap.alloc(RustCompilationModeOptsStub);
                let opt = heap.alloc(RustCompilationModeOptsStub);
                let fastbuild = heap.alloc(RustCompilationModeOptsStub);
                map.insert_hashed(heap.alloc_str("dbg").to_value().get_hashed().unwrap(), dbg);
                map.insert_hashed(heap.alloc_str("opt").to_value().get_hashed().unwrap(), opt);
                map.insert_hashed(
                    heap.alloc_str("fastbuild").to_value().get_hashed().unwrap(),
                    fastbuild,
                );
                Some(heap.alloc(Dict::new(map)))
            }

            // CcInfo stubs for linking
            "stdlib_linkflags" => Some(heap.alloc(CcInfoStub)),
            "libstd_and_allocator_ccinfo" => Some(Value::new_none()),
            "libstd_and_global_allocator_ccinfo" => Some(Value::new_none()),
            "nostd_and_global_allocator_cc_info" => Some(Value::new_none()),

            // Internal/experimental flags
            "_rename_first_party_crates" => Some(Value::new_bool(false)),
            "_third_party_dir" => Some(heap.alloc_str("").to_value()),
            "_pipelined_compilation" => Some(Value::new_bool(false)),
            "_no_std" => Some(heap.alloc_str("off").to_value()),
            "_experimental_link_std_dylib" => Some(Value::new_bool(false)),
            "_experimental_toolchain_generated_sysroot" => Some(Value::new_bool(false)),
            "_experimental_use_cc_common_link" => Some(Value::new_bool(false)),
            "_experimental_use_coverage_metadata_files" => Some(Value::new_bool(false)),
            "_experimental_use_global_allocator" => Some(Value::new_bool(false)),
            "_incompatible_no_rustc_sysroot_env" => Some(Value::new_bool(false)),
            "_incompatible_test_attr_crate_and_srcs_mutually_exclusive" => {
                Some(Value::new_bool(false))
            }
            _ => None,
        }
    }
}

/// A stub for a Rust tool (rustc, rustdoc, etc.) that provides a .path attribute.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RustToolStub(&'static str);

/// Detect the path to a Rust tool by checking common locations.
fn detect_rust_tool_path(tool_name: &str) -> String {
    // Try `which` first via PATH lookup
    if let Ok(output) = std::process::Command::new("which").arg(tool_name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return path;
            }
        }
    }
    // Fallback to common locations
    let home = std::env::var("HOME").unwrap_or_default();
    let cargo_path = format!("{}/.cargo/bin/{}", home, tool_name);
    if std::path::Path::new(&cargo_path).exists() {
        return cargo_path;
    }
    let usr_local = format!("/usr/local/bin/{}", tool_name);
    if std::path::Path::new(&usr_local).exists() {
        return usr_local;
    }
    format!("/usr/bin/{}", tool_name)
}

impl std::fmt::Display for RustToolStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}>", self.0)
    }
}

starlark::starlark_simple_value!(RustToolStub);

#[starlark::values::starlark_value(type = "rust_tool")]
impl<'v> StarlarkValue<'v> for RustToolStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "dirname"
                | "extension"
                | "is_source"
                | "root"
                | "owner"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" | "short_path" => {
                let path = detect_rust_tool_path(self.0);
                Some(heap.alloc_str(&path).to_value())
            }
            "basename" => Some(heap.alloc_str(self.0).to_value()),
            "dirname" => {
                let path = detect_rust_tool_path(self.0);
                let dir = std::path::Path::new(&path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                Some(heap.alloc_str(&dir).to_value())
            }
            "extension" => Some(heap.alloc_str("").to_value()),
            "is_source" => Some(Value::new_bool(false)),
            "root" => Some(Value::new_none()),
            "owner" => Some(Value::new_none()),
            _ => None,
        }
    }
}

// ============================================================================
// PyToolchainInfoStub - Stub for Python toolchain info
// ============================================================================

/// A stub for the Python toolchain info returned by ctx.toolchains for Python rules.
///
/// Provides py3_runtime attribute that rules_python expects.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PyToolchainInfoStub;

impl std::fmt::Display for PyToolchainInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<PyToolchainInfo>")
    }
}

starlark::starlark_simple_value!(PyToolchainInfoStub);

#[starlark::values::starlark_value(type = "PyToolchainInfo")]
impl<'v> StarlarkValue<'v> for PyToolchainInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "py3_runtime" | "py2_runtime" | "exec_compatible_with"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "py3_runtime" => Some(heap.alloc(PyRuntimeInfoStub)),
            "py2_runtime" => Some(Value::new_none()),
            "exec_compatible_with" => Some(heap.alloc(starlark::values::list::AllocList::EMPTY)),
            _ => None,
        }
    }
}

/// A stub for PyRuntimeInfo that provides the Python interpreter path.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PyRuntimeInfoStub;

impl std::fmt::Display for PyRuntimeInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<PyRuntimeInfo>")
    }
}

starlark::starlark_simple_value!(PyRuntimeInfoStub);

#[starlark::values::starlark_value(type = "PyRuntimeInfo")]
impl<'v> StarlarkValue<'v> for PyRuntimeInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "interpreter_path"
                | "interpreter"
                | "files"
                | "coverage_tool"
                | "coverage_files"
                | "python_version"
                | "stub_shebang"
                | "bootstrap_template"
                | "implementation_name"
                | "flag_values"
                | "site_init_template"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "interpreter_path" => {
                // Use the magic sentinel to force the legacy bootstrap-template path.
                // This makes rules_python fall back to ctx.file._bootstrap_template
                // (which is @bazel_tools//tools/python:python_bootstrap_template.txt),
                // since our PyRuntimeInfoStub doesn't provide a real bootstrap_template.
                Some(
                    heap.alloc_str("/_magic_pyruntime_sentinel_do_not_use")
                        .to_value(),
                )
            }
            "interpreter" => Some(Value::new_none()),
            "files" => Some(Value::new_none()),
            "coverage_tool" => Some(Value::new_none()),
            "coverage_files" => Some(Value::new_none()),
            "python_version" => Some(heap.alloc_str("PY3").to_value()),
            "stub_shebang" => Some(heap.alloc_str("#!/usr/bin/env python3").to_value()),
            "bootstrap_template" => Some(Value::new_none()),
            "implementation_name" => Some(heap.alloc_str("cpython").to_value()),
            "flag_values" => Some(heap.alloc(starlark::values::dict::Dict::default())),
            "site_init_template" => Some(Value::new_none()),
            _ => None,
        }
    }
}

// ============================================================================
// Java toolchain stubs
// ============================================================================

/// Stub for the Java toolchain wrapper object.
/// In Bazel, `ctx.toolchains["@rules_java//java:toolchain_type"]` returns
/// an object with a `.java` attribute containing the `JavaToolchainInfo`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaToolchainWrapperStub;

impl std::fmt::Display for JavaToolchainWrapperStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JavaToolchainWrapper>")
    }
}

starlark::starlark_simple_value!(JavaToolchainWrapperStub);

#[starlark::values::starlark_value(type = "JavaToolchainWrapper")]
impl<'v> StarlarkValue<'v> for JavaToolchainWrapperStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "java" | "java_runtime")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "java" => Some(heap.alloc(JavaToolchainInfoStub)),
            "java_runtime" => Some(heap.alloc(JavaRuntimeInfoStub)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["java".to_owned(), "java_runtime".to_owned()]
    }
}

/// Stub for JavaToolchainInfo providing minimal attributes for rules_java.
///
/// Provides the key fields that rules_java Starlark code accesses:
/// bootclasspath, ijar, java_runtime, jvm_opt, source_version, target_version, etc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaToolchainInfoStub;

impl std::fmt::Display for JavaToolchainInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JavaToolchainInfo>")
    }
}

starlark::starlark_simple_value!(JavaToolchainInfoStub);

#[starlark::values::starlark_value(type = "JavaToolchainInfo")]
impl<'v> StarlarkValue<'v> for JavaToolchainInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "bootclasspath"
                | "ijar"
                | "jacocorunner"
                | "java_runtime"
                | "jvm_opt"
                | "label"
                | "proguard_allowlister"
                | "single_jar"
                | "source_version"
                | "target_version"
                | "tools"
                | "_javabuilder"
                | "_header_compiler"
                | "_header_compiler_direct"
                | "_header_compiler_builtin_processors"
                | "_javacopts"
                | "_javacopts_list"
                | "_compatible_javacopts"
                | "_javac_supports_workers"
                | "_javac_supports_multiplex_workers"
                | "_javac_supports_worker_cancellation"
                | "_javac_supports_worker_multiplex_sandboxing"
                | "_gen_class"
                | "_deps_checker"
                | "_forcibly_disable_header_compilation"
                | "_one_version_tool"
                | "_one_version_allowlist"
                | "_one_version_allowlist_for_tests"
                | "_android_linter"
                | "_bytecode_optimizer"
                | "_local_java_optimization_config"
                | "_timezone_data"
                | "_reduced_classpath_incompatible_processors"
                | "_package_configuration"
                | "_jspecify_info"
                | "_bootclasspath_info"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::list::AllocList;
        let empty_depset = || heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty());
        match attribute {
            "bootclasspath" => Some(empty_depset()),
            "ijar" => Some(Value::new_none()),
            "jacocorunner" => Some(Value::new_none()),
            "java_runtime" => Some(heap.alloc(JavaRuntimeInfoStub)),
            "jvm_opt" => Some(empty_depset()),
            "label" => Some(Value::new_none()),
            "proguard_allowlister" => Some(Value::new_none()),
            "single_jar" => Some(Value::new_none()),
            "source_version" => Some(heap.alloc_str("11").to_value()),
            "target_version" => Some(heap.alloc_str("11").to_value()),
            "tools" => Some(empty_depset()),
            // Internal fields used by rules_java Starlark
            "_javabuilder" => Some(Value::new_none()),
            "_header_compiler" => Some(Value::new_none()),
            "_header_compiler_direct" => Some(Value::new_none()),
            "_header_compiler_builtin_processors" => Some(heap.alloc(AllocList::EMPTY)),
            "_javacopts" => Some(empty_depset()),
            "_javacopts_list" => Some(heap.alloc(AllocList::EMPTY)),
            "_compatible_javacopts" => Some(heap.alloc(starlark::values::dict::Dict::default())),
            "_javac_supports_workers" => Some(Value::new_bool(true)),
            "_javac_supports_multiplex_workers" => Some(Value::new_bool(true)),
            "_javac_supports_worker_cancellation" => Some(Value::new_bool(false)),
            "_javac_supports_worker_multiplex_sandboxing" => Some(Value::new_bool(false)),
            "_gen_class" => Some(Value::new_none()),
            "_deps_checker" => Some(Value::new_none()),
            "_forcibly_disable_header_compilation" => Some(Value::new_bool(false)),
            "_one_version_tool" => Some(Value::new_none()),
            "_one_version_allowlist" => Some(Value::new_none()),
            "_one_version_allowlist_for_tests" => Some(Value::new_none()),
            "_android_linter" => Some(Value::new_none()),
            "_bytecode_optimizer" => Some(Value::new_none()),
            "_local_java_optimization_config" => Some(Value::new_none()),
            "_timezone_data" => Some(Value::new_none()),
            "_reduced_classpath_incompatible_processors" => Some(heap.alloc(AllocList::EMPTY)),
            "_package_configuration" => Some(heap.alloc(AllocList::EMPTY)),
            "_jspecify_info" => Some(Value::new_none()),
            "_bootclasspath_info" => Some(Value::new_none()),
            _ => None,
        }
    }
}

/// Stub for JavaRuntimeInfo providing minimal attributes for rules_java.
///
/// Provides java_home, java_executable paths, files, and version info.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaRuntimeInfoStub;

impl std::fmt::Display for JavaRuntimeInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JavaRuntimeInfo>")
    }
}

starlark::starlark_simple_value!(JavaRuntimeInfoStub);

#[starlark::values::starlark_value(type = "JavaRuntimeInfo")]
impl<'v> StarlarkValue<'v> for JavaRuntimeInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "java_home"
                | "java_home_runfiles_path"
                | "java_executable_exec_path"
                | "java_executable_runfiles_path"
                | "files"
                | "hermetic_files"
                | "hermetic_static_libs"
                | "lib_modules"
                | "lib_ct_sym"
                | "default_cds"
                | "version"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let empty_depset = || heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty());
        // Try to detect JAVA_HOME from environment
        let java_home = std::env::var("JAVA_HOME").unwrap_or_else(|_| {
            if cfg!(windows) {
                "C:/Program Files/Java/jdk-11".to_owned()
            } else {
                "/usr/lib/jvm/java-11".to_owned()
            }
        });
        let java_exe = if cfg!(windows) {
            format!("{}/bin/java.exe", java_home)
        } else {
            format!("{}/bin/java", java_home)
        };
        match attribute {
            "java_home" => Some(heap.alloc_str(&java_home).to_value()),
            "java_home_runfiles_path" => Some(heap.alloc_str(&java_home).to_value()),
            "java_executable_exec_path" => Some(heap.alloc_str(&java_exe).to_value()),
            "java_executable_runfiles_path" => Some(heap.alloc_str(&java_exe).to_value()),
            "files" => Some(empty_depset()),
            "hermetic_files" => Some(empty_depset()),
            "hermetic_static_libs" => Some(heap.alloc(starlark::values::list::AllocList::EMPTY)),
            "lib_modules" => Some(Value::new_none()),
            "lib_ct_sym" => Some(Value::new_none()),
            "default_cds" => Some(Value::new_none()),
            "version" => Some(heap.alloc(11)),
            _ => None,
        }
    }
}

/// Stub for the Java runtime toolchain wrapper.
/// `ctx.toolchains["@rules_java//java:runtime_toolchain_type"]` returns
/// an object with `.java_runtime` containing the `JavaRuntimeInfo`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaRuntimeToolchainWrapperStub;

impl std::fmt::Display for JavaRuntimeToolchainWrapperStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JavaRuntimeToolchainWrapper>")
    }
}

starlark::starlark_simple_value!(JavaRuntimeToolchainWrapperStub);

#[starlark::values::starlark_value(type = "JavaRuntimeToolchainWrapper")]
impl<'v> StarlarkValue<'v> for JavaRuntimeToolchainWrapperStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "java_runtime")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "java_runtime" => Some(heap.alloc(JavaRuntimeInfoStub)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["java_runtime".to_owned()]
    }
}

// ============================================================================
// OCI toolchain stubs (crane, registry, jq)
// ============================================================================

/// Detect a binary path using platform-appropriate lookup.
fn detect_binary_path(name: &str, extra_candidates: &[&str]) -> String {
    // Check extra candidates first
    for path in extra_candidates {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    // Use platform-appropriate PATH lookup
    let finder = if cfg!(windows) { "where" } else { "which" };
    if let Ok(output) = std::process::Command::new(finder).arg(name).output() {
        if output.status.success() {
            if let Ok(s) = std::str::from_utf8(&output.stdout) {
                let first_line = s.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    return first_line.to_string();
                }
            }
        }
    }
    name.to_string()
}

/// Detect the crane binary path from common locations.
fn detect_crane_path() -> String {
    let extra = if cfg!(windows) {
        vec![]
    } else {
        vec!["/usr/local/bin/crane", "/usr/bin/crane"]
    };
    detect_binary_path("crane", &extra)
}

/// Detect the jq binary path.
fn detect_jq_path() -> String {
    let extra = if cfg!(windows) {
        vec![]
    } else {
        vec!["/usr/bin/jq", "/usr/local/bin/jq"]
    };
    detect_binary_path("jq", &extra)
}

/// Get or create the OCI registry launcher script.
/// The launcher script implements start_registry/stop_registry using crane registry serve.
fn get_or_create_oci_launcher() -> String {
    let launcher_path = &format!(
        "{}/kuro_oci_registry_launcher.sh",
        std::env::temp_dir().to_string_lossy()
    );
    if !std::path::Path::new(launcher_path).exists() {
        let crane_path = detect_crane_path();
        let content = format!(
            r#"#!/usr/bin/env bash
# Kuro OCI registry launcher - auto-generated
CRANE_BIN="{crane_path}"

function start_registry() {{
    local storage_dir="$1"
    local output="$2"
    local deadline="${{3:-10}}"
    local registry_pid="${{storage_dir}}/proc.pid"

    mkdir -p "${{storage_dir}}"
    "${{CRANE_BIN}}" registry serve --disk="${{storage_dir}}" --address=localhost:0 >>"${{output}}" 2>&1 &
    echo "$!" > "${{registry_pid}}"

    local timeout=$((SECONDS + ${{deadline}}))
    local port=""
    while [ "${{SECONDS}}" -lt "${{timeout}}" ]; do
        port=$(grep -o "serving on port [0-9]*" "${{output}}" 2>/dev/null | tail -1 | grep -o "[0-9]*$")
        if [ -n "${{port}}" ]; then
            break
        fi
        sleep 0.1
    done

    if [ -z "${{port}}" ]; then
        echo "registry didn't become ready within ${{deadline}}s." >&2
        return 1
    fi
    echo "127.0.0.1:${{port}}"
    return 0
}}

function stop_registry() {{
    local storage_dir="$1"
    local registry_pid="${{storage_dir}}/proc.pid"
    if [ -f "${{registry_pid}}" ]; then
        kill "$(cat "${{registry_pid}}")" 2>/dev/null || true
        rm -f "${{registry_pid}}"
    fi
}}
"#,
        );
        let _ = std::fs::write(launcher_path, &content);
        // Make it executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(launcher_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(launcher_path, perms);
            }
        }
    }
    launcher_path.to_string()
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
                let root_path = if let Some(prefix) =
                    self.full_path.strip_suffix(&self.short_path)
                {
                    prefix.trim_end_matches('/').to_owned()
                } else {
                    "bazel-out".to_owned()
                };
                Some(heap.alloc(ArtifactRootStub { path: root_path }))
            }
            _ => None,
        }
    }
}

/// A stub that wraps a string path and provides a `.path` attribute.
/// Used for crane_info.binary, registry_info.launcher, jqinfo.bin, etc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FileLikePathStub {
    path: String,
}

impl std::fmt::Display for FileLikePathStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<file:{}>", self.path)
    }
}

starlark::starlark_simple_value!(FileLikePathStub);

#[starlark::values::starlark_value(type = "FileLikePath")]
impl<'v> StarlarkValue<'v> for FileLikePathStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "is_source"
                | "is_directory"
                | "extension"
                | "owner"
                | "root"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            "short_path" => Some(heap.alloc_str(&self.path).to_value()),
            "basename" => {
                let basename = std::path::Path::new(&self.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&self.path);
                Some(heap.alloc_str(basename).to_value())
            }
            "is_source" => Some(Value::new_bool(true)),
            "is_directory" => Some(Value::new_bool(false)),
            _ => None,
        }
    }
}

/// Stub for crane_info provider (CraneInfo from rules_oci).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OciCraneInfoStub {
    crane_path: String,
}

impl std::fmt::Display for OciCraneInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CraneInfo>")
    }
}

starlark::starlark_simple_value!(OciCraneInfoStub);

#[starlark::values::starlark_value(type = "CraneInfo")]
impl<'v> StarlarkValue<'v> for OciCraneInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "binary" | "version")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "binary" => Some(heap.alloc(FileLikePathStub {
                path: self.crane_path.clone(),
            })),
            "version" => Some(heap.alloc_str("0.19.1").to_value()),
            _ => None,
        }
    }
}

/// Stub for OCI crane toolchain.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OciCraneToolchainStub {
    crane_path: String,
}

impl std::fmt::Display for OciCraneToolchainStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<CraneToolchain>")
    }
}

starlark::starlark_simple_value!(OciCraneToolchainStub);

#[starlark::values::starlark_value(type = "CraneToolchain")]
impl<'v> StarlarkValue<'v> for OciCraneToolchainStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "crane_info")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "crane_info" => Some(heap.alloc(OciCraneInfoStub {
                crane_path: self.crane_path.clone(),
            })),
            _ => None,
        }
    }
}

/// Stub for registry_info provider (RegistryInfo from rules_oci).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OciRegistryInfoStub {
    launcher_path: String,
    registry_path: String,
}

impl std::fmt::Display for OciRegistryInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<RegistryInfo>")
    }
}

starlark::starlark_simple_value!(OciRegistryInfoStub);

#[starlark::values::starlark_value(type = "RegistryInfo")]
impl<'v> StarlarkValue<'v> for OciRegistryInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "launcher" | "registry")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "launcher" => Some(heap.alloc(FileLikePathStub {
                path: self.launcher_path.clone(),
            })),
            "registry" => Some(heap.alloc(FileLikePathStub {
                path: self.registry_path.clone(),
            })),
            _ => None,
        }
    }
}

/// Stub for OCI registry toolchain.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OciRegistryToolchainStub {
    launcher_path: String,
    registry_path: String,
}

impl std::fmt::Display for OciRegistryToolchainStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<RegistryToolchain>")
    }
}

starlark::starlark_simple_value!(OciRegistryToolchainStub);

#[starlark::values::starlark_value(type = "RegistryToolchain")]
impl<'v> StarlarkValue<'v> for OciRegistryToolchainStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "registry_info")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "registry_info" => Some(heap.alloc(OciRegistryInfoStub {
                launcher_path: self.launcher_path.clone(),
                registry_path: self.registry_path.clone(),
            })),
            _ => None,
        }
    }
}

/// Stub for jqinfo provider (from aspect_bazel_lib jq toolchain).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JqInfoStub {
    jq_path: String,
}

impl std::fmt::Display for JqInfoStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JqInfo>")
    }
}

starlark::starlark_simple_value!(JqInfoStub);

#[starlark::values::starlark_value(type = "JqInfo")]
impl<'v> StarlarkValue<'v> for JqInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "bin")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "bin" => Some(heap.alloc(FileLikePathStub {
                path: self.jq_path.clone(),
            })),
            _ => None,
        }
    }
}

/// Stub for jq toolchain (from aspect_bazel_lib).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JqToolchainStub {
    jq_path: String,
}

impl std::fmt::Display for JqToolchainStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<JqToolchain>")
    }
}

starlark::starlark_simple_value!(JqToolchainStub);

#[starlark::values::starlark_value(type = "JqToolchain")]
impl<'v> StarlarkValue<'v> for JqToolchainStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "jqinfo")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "jqinfo" => Some(heap.alloc(JqInfoStub {
                jq_path: self.jq_path.clone(),
            })),
            _ => None,
        }
    }
}

/// A generic toolchain stub for unrecognized toolchain types.
///
/// Instead of returning None (which crashes on attribute access), this returns
/// a stub that accepts any attribute access and returns None. This allows rules
/// to check for toolchain presence without crashing.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct GenericToolchainStub;

impl std::fmt::Display for GenericToolchainStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<GenericToolchain>")
    }
}

starlark::starlark_simple_value!(GenericToolchainStub);

#[starlark::values::starlark_value(type = "GenericToolchain")]
impl<'v> StarlarkValue<'v> for GenericToolchainStub {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // Accept any attribute to avoid crashes
        true
    }

    fn get_attr(&self, _attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        // Return None for any attribute - callers should check for None
        Some(Value::new_none())
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
