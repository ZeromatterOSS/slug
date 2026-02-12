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
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use either::Either;
use itertools::Itertools;
use kuro_core::plugins::PluginKind;
use kuro_error::BuckErrorContext;
use kuro_interpreter::late_binding_ty::AnalysisContextReprLate;
use kuro_interpreter::late_binding_ty::ProviderReprLate;
use kuro_interpreter::late_binding_ty::TransitionReprLate;
use kuro_interpreter::starlark_promise::StarlarkPromise;
use kuro_interpreter::types::rule::FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL;
use kuro_interpreter::types::rule::FROZEN_RULE_GET_IMPL;
use kuro_interpreter::types::transition::transition_id_from_value;
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::list::ListLiteral;
use kuro_node::attrs::attr_type::string::StringLiteral;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::display::AttrDisplayWithContextExt;
use kuro_node::attrs::spec::AttributeSpec;
use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
use kuro_node::nodes::unconfigured::RuleKind;
use kuro_node::nodes::unconfigured::TargetNode;
use kuro_node::rule::Rule;
use kuro_node::rule::RuleIncomingTransition;
use kuro_node::rule_type::RuleType;
use kuro_node::rule_type::StarlarkRuleType;
use kuro_util::arc_str::ArcSlice;
use kuro_util::arc_str::ArcStr;
use starlark::any::ProvidesStaticType;
use starlark::docs::DocFunction;
use starlark::docs::DocItem;
use starlark::docs::DocMember;
use starlark::docs::DocStringKind;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::ParamSpec;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenRef;
use starlark::values::FrozenStringValue;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::StringValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueOfUnchecked;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list::ListType;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::typing::FrozenStarlarkCallable;
use starlark::values::typing::StarlarkCallable;
use starlark::values::typing::StarlarkCallableChecked;
use starlark_map::small_map::SmallMap;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;
use crate::interpreter::module_internals::ModuleInternals;
use crate::nodes::attr_spec::AttributeSpecExt;
use crate::nodes::unconfigured::TargetNodeExt;
use crate::plugins::PluginKindArg;

pub static NAME_ATTRIBUTE_FIELD: &str = "name";

#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative, Clone, Copy)]
enum RuleImpl<'v> {
    BuildRule(StarlarkCallable<'v, (FrozenValue,), ListType<FrozenValue>>),
    BxlAnon(StarlarkCallable<'v, (FrozenValue, FrozenValue), ListType<FrozenValue>>),
}

/// The callable that's returned from a `rule()` call. Once frozen, and called, it adds targets'
/// parameters to the context
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkRuleCallable<'v> {
    /// The import path that contains the rule() call; stored here so we can retrieve extra
    /// information during `export_as()`
    rule_path: BzlOrBxlPath,
    /// Once exported, the `import_path` and `name` of the callable. Used in DICE to retrieve rule
    /// implementations
    id: RefCell<Option<StarlarkRuleType>>,
    /// The implementation function for this rule.
    /// If is a build rule or anon rule in bzl must take a ctx,
    /// If is a bxl anon rule must take a bxl context and attrs.
    implementation: RuleImpl<'v>,
    // Field Name -> Attribute
    attributes: AttributeSpec,
    /// Type for the typechecker.
    ty: Ty,
    /// When specified, this transition will be applied to the target before configuring it.
    cfg: RuleIncomingTransition,
    /// The plugins that are used by these targets
    uses_plugins: Vec<PluginKind>,
    /// This kind of the rule, e.g. whether it can be used in configuration context.
    rule_kind: RuleKind,
    /// The raw docstring for this rule
    docs: Option<String>,
    /// When evaluating rule function, take only the `name` argument, ignore the others.
    ignore_attrs_for_profiling: bool,
    /// Optional map of the promise artifact name to starlark function.
    /// `None` for normal rules, `Some` for anon targets.
    artifact_promise_mappings: Option<ArtifactPromiseMappings<'v>>,
    /// Whether this is a Bazel test rule (created with `rule(test=True)`).
    is_test: bool,
}

/// Mappings of promise artifact name to the starlark function that will produce it, for anon targets.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
struct ArtifactPromiseMappings<'v> {
    mappings: SmallMap<StringValue<'v>, Value<'v>>,
}

/// Mappings of frozen promise artifact name to the frozen starlark function that will produce it, for anon targets.
#[derive(Debug, ProvidesStaticType, Trace, Allocative)]
pub struct FrozenArtifactPromiseMappings {
    pub mappings: SmallMap<FrozenStringValue, FrozenValue>,
}

impl<'v> Display for StarlarkRuleCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.id.borrow() {
            Some(id) => write!(f, "{}()", id.name),
            None => write!(f, "<unbound rule>"),
        }
    }
}

/// Errors around rule declaration, instantiation, validation, etc
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum RuleError {
    #[error("The output of rule() may only be called after the module is loaded")]
    RuleCalledBeforeFreezing,
    #[error("`{0}` is not a valid attribute name")]
    InvalidParameterName(String),
    #[error("Rule defined in `{0}` must be assigned to a variable, e.g. `my_rule = rule(...)`")]
    RuleNotAssigned(BzlOrBxlPath),
    #[error(
        "Rule defined with both `is_configuration_rule` and `is_toolchain_rule`, these options are mutually exclusive"
    )]
    IsConfigurationAndToolchain,
    #[error("`rule` can only be declared in bzl files")]
    RuleNonInBzl,
    #[error("Cannot specify `cfg` and `supports_incoming_transition` at the same time")]
    CfgAndSupportsIncomingTransition,
    #[error("{0} rules do not support incoming transitions")]
    RuleDoesNotSupportIncomingTransition(&'static str),
    #[error(
        "Cannot specify both `impl` and `implementation` in rule(). Use only `implementation` (Bazel-style) or `impl` (Kuro-style)"
    )]
    BothImplAndImplementation,
    #[error(
        "Missing `implementation` function in rule(). Specify either `implementation = _impl` (Bazel-style) or `impl = _impl` (Kuro-style)"
    )]
    MissingImplementation,
}

impl<'v> AllocValue<'v> for StarlarkRuleCallable<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> StarlarkRuleCallable<'v> {
    fn new(
        implementation: RuleImpl<'v>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        cfg: Option<Value>,
        supports_incoming_transition: Option<bool>,
        doc: &str,
        is_configuration_rule: bool,
        is_toolchain_rule: bool,
        uses_plugins: Vec<PluginKind>,
        artifact_promise_mappings: Option<ArtifactPromiseMappings<'v>>,
        is_test: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<StarlarkRuleCallable<'v>> {
        let build_context = BuildContext::from_context(eval)?;

        let rule_path: BzlOrBxlPath = match (&build_context.additional, &implementation) {
            (PerFileTypeContext::Bzl(bzl_path), RuleImpl::BuildRule(_)) => {
                BzlOrBxlPath::Bzl(bzl_path.bzl_path.clone())
            }
            (PerFileTypeContext::Bxl(bxl_path), RuleImpl::BxlAnon(_)) => {
                BzlOrBxlPath::Bxl(bxl_path.clone())
            }
            (PerFileTypeContext::Bxl(_), RuleImpl::BuildRule(_)) => {
                return Err(RuleError::RuleNonInBzl.into());
            }
            // TODO(nero): add error for it
            (_, _) => unreachable!(
                "unreachable, since bxl.anon_rule is not registered for eval for bzl files"
            ),
        };

        let sorted_validated_attrs = attrs
            .entries
            .into_iter()
            .sorted_by(|(k1, _), (k2, _)| Ord::cmp(k1, k2))
            .map(|(name, value)| {
                if name == NAME_ATTRIBUTE_FIELD {
                    Err(RuleError::InvalidParameterName(NAME_ATTRIBUTE_FIELD.to_owned()).into())
                } else {
                    Ok((name.to_owned(), value.clone_attribute()))
                }
            })
            .collect::<kuro_error::Result<Vec<(String, Attribute)>>>()?;

        let cfg = match (cfg, supports_incoming_transition) {
            (Some(_), Some(_)) => return Err(RuleError::CfgAndSupportsIncomingTransition.into()),
            (Some(cfg), None) => {
                // Handle the case where cfg is passed but is None (e.g., from a stub transition)
                // In Bazel compat mode, some transitions are stubbed as None
                if cfg.is_none() {
                    RuleIncomingTransition::None
                } else if cfg.get_type() == "config_transition" || cfg.get_type() == "Transition" {
                    // Bazel config.target() / config.exec() / transition() are no-ops in Kuro.
                    // config.target() = use target configuration (the default).
                    // config.exec() = use exec configuration (not yet implemented).
                    // transition() = custom configuration transition (not yet implemented).
                    RuleIncomingTransition::None
                } else {
                    RuleIncomingTransition::Fixed(transition_id_from_value(cfg)?)
                }
            }
            (None, Some(true)) => RuleIncomingTransition::FromAttribute,
            (None, Some(false) | None) => RuleIncomingTransition::None,
        };

        let rule_kind = match (is_configuration_rule, is_toolchain_rule) {
            (false, false) => RuleKind::Normal,
            (true, false) => RuleKind::Configuration,
            (false, true) => RuleKind::Toolchain,
            (true, true) => return Err(RuleError::IsConfigurationAndToolchain.into()),
        };

        if cfg != RuleIncomingTransition::None {
            let unsupported_rule_kind_str = match rule_kind {
                RuleKind::Normal => None,
                RuleKind::Configuration => Some("Configuration"),
                RuleKind::Toolchain => Some("Toolchain"),
            };
            if let Some(unsupported_rule_kind_str) = unsupported_rule_kind_str {
                return Err(RuleError::RuleDoesNotSupportIncomingTransition(
                    unsupported_rule_kind_str,
                )
                .into());
            }
        }

        let attributes = AttributeSpec::from(
            sorted_validated_attrs,
            artifact_promise_mappings.is_some(),
            &cfg,
        )?;
        let ty = Ty::ty_function(attributes.ty_function());

        Ok(StarlarkRuleCallable {
            rule_path,
            id: RefCell::new(None),
            implementation,
            attributes,
            ty,
            cfg,
            rule_kind,
            uses_plugins,
            docs: Some(doc.to_owned()),
            ignore_attrs_for_profiling: build_context.ignore_attrs_for_profiling,
            artifact_promise_mappings,
            is_test,
        })
    }

    fn new_anon_impl(
        implementation: RuleImpl<'v>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        doc: &str,
        artifact_promise_mappings: SmallMap<
            StringValue<'v>,
            StarlarkCallable<'v, (FrozenValue,), UnpackList<FrozenValue>>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<Self> {
        Self::new(
            implementation,
            attrs,
            None,
            None,
            doc,
            false,
            false,
            Vec::new(),
            Some(ArtifactPromiseMappings {
                mappings: artifact_promise_mappings
                    .iter()
                    .map(|(k, v)| (*k, v.0))
                    .collect::<SmallMap<_, _>>(),
            }),
            false, // anon rules are never test rules
            eval,
        )
    }

    fn new_anon(
        implementation: StarlarkCallable<'v, (FrozenValue,), ListType<FrozenValue>>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        doc: &str,
        artifact_promise_mappings: SmallMap<
            StringValue<'v>,
            StarlarkCallable<'v, (FrozenValue,), UnpackList<FrozenValue>>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<Self> {
        Self::new_anon_impl(
            RuleImpl::BuildRule(implementation),
            attrs,
            doc,
            artifact_promise_mappings,
            eval,
        )
    }

    pub fn new_bxl_anon(
        implementation: StarlarkCallable<'v, (FrozenValue, FrozenValue), ListType<FrozenValue>>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        doc: &str,
        artifact_promise_mappings: SmallMap<
            StringValue<'v>,
            StarlarkCallable<'v, (FrozenValue,), UnpackList<FrozenValue>>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<Self> {
        Self::new_anon_impl(
            RuleImpl::BxlAnon(implementation),
            attrs,
            doc,
            artifact_promise_mappings,
            eval,
        )
    }

    fn documentation_impl(&self) -> DocItem {
        let name = self
            .id
            .borrow()
            .as_ref()
            .map_or_else(|| "unbound_rule".to_owned(), |rt| rt.name.clone());
        let parameters_spec = self.attributes.signature_with_default_value(name);
        let parameter_types = self.attributes.starlark_types();
        let parameter_docs = self.attributes.docstrings();
        let params = parameters_spec.documentation_with_default_value_formatter(
            parameter_types,
            parameter_docs,
            |v| v.as_display_no_ctx().to_string(),
        );

        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::none(),
            self.docs.as_deref(),
        );

        DocItem::Member(DocMember::Function(function_docs))
    }
}

#[starlark_value(type = "Rule")]
impl<'v> StarlarkValue<'v> for StarlarkRuleCallable<'v> {
    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        *self.id.borrow_mut() = Some(StarlarkRuleType {
            path: self.rule_path.clone(),
            name: variable_name.to_owned(),
        });
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(RuleError::RuleCalledBeforeFreezing).into())
    }

    fn documentation(&self) -> DocItem {
        self.documentation_impl()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.ty.clone())
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::function(ParamSpec::kwargs(Ty::any()), Ty::none())
    }
}

#[derive(Debug, ProvidesStaticType, Allocative, Clone, Dupe)]
enum FrozenRuleImpl {
    BuildRule(FrozenStarlarkCallable<(FrozenValue,), ListType<FrozenValue>>),
    BxlAnon(FrozenStarlarkCallable<(FrozenValue, FrozenValue), ListType<FrozenValue>>),
}

impl FrozenRuleImpl {
    fn to_frozen_value(self) -> FrozenValue {
        match self {
            FrozenRuleImpl::BuildRule(callable) => callable.0,
            FrozenRuleImpl::BxlAnon(callable) => callable.0,
        }
    }
}

impl<'v> Freeze for RuleImpl<'v> {
    type Frozen = FrozenRuleImpl;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        match self {
            RuleImpl::BuildRule(impl_) => Ok(FrozenRuleImpl::BuildRule(impl_.freeze(freezer)?)),
            RuleImpl::BxlAnon(impl_) => Ok(FrozenRuleImpl::BxlAnon(impl_.freeze(freezer)?)),
        }
    }
}

impl<'v> Freeze for StarlarkRuleCallable<'v> {
    type Frozen = FrozenStarlarkRuleCallable;
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let frozen_impl = self.implementation.freeze(freezer)?;
        let rule_docs = self.documentation_impl();
        let id = match self.id.into_inner() {
            Some(x) => x,
            None => {
                return Err(FreezeError::new(
                    RuleError::RuleNotAssigned(self.rule_path).to_string(),
                ));
            }
        };
        let rule_type = Arc::new(id);
        let rule_name = rule_type.name.to_owned();

        // For StarlarkRuleCallable, it doesn't rely on `signature` to get the default value, instead we get the default value from `Rule.attributes`,
        // so use `signature(rule_name)` method here.
        // TODO(nero): It need to some refactor to make it more clear, e.g. add a new type `ParametersSpec<NoDefaults>` here.
        let signature = self.attributes.signature(rule_name).freeze(freezer)?;

        let artifact_promise_mappings = match self.artifact_promise_mappings {
            Some(artifacts) => {
                let mut mappings = SmallMap::new();
                for (name, implementation) in artifacts.mappings {
                    mappings.insert(name.freeze(freezer)?, implementation.freeze(freezer)?);
                }
                Some(FrozenArtifactPromiseMappings { mappings })
            }
            None => None,
        };

        Ok(FrozenStarlarkRuleCallable {
            rule: Arc::new(Rule {
                attributes: self.attributes,
                rule_type: RuleType::Starlark(rule_type.dupe()),
                cfg: self.cfg,
                rule_kind: self.rule_kind,
                uses_plugins: self.uses_plugins,
                is_test: self.is_test,
            }),
            rule_type,
            implementation: frozen_impl,
            signature,
            rule_docs,
            ty: self.ty,
            ignore_attrs_for_profiling: self.ignore_attrs_for_profiling,
            artifact_promise_mappings,
        })
    }
}

#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("{}()", rule.rule_type.name())]
pub struct FrozenStarlarkRuleCallable {
    rule: Arc<Rule>,
    /// Identical to `rule.rule_type` but more specific type.
    rule_type: Arc<StarlarkRuleType>,
    implementation: FrozenRuleImpl,
    /// We don't need rely on `signature` to get the default value here, instead we get the default
    /// value from `Rule.attributes`. So use in the ParametersSpecNoDefaults for more clarity
    signature: ParametersSpec<FrozenValue>,
    rule_docs: DocItem,
    ty: Ty,
    ignore_attrs_for_profiling: bool,
    artifact_promise_mappings: Option<FrozenArtifactPromiseMappings>,
}
starlark_simple_value!(FrozenStarlarkRuleCallable);

fn unpack_frozen_rule(
    rule: FrozenValue,
) -> kuro_error::Result<FrozenRef<'static, FrozenStarlarkRuleCallable>> {
    rule.downcast_frozen_ref::<FrozenStarlarkRuleCallable>()
        .buck_error_context("Expecting FrozenRuleCallable")
}

pub(crate) fn init_frozen_rule_get_impl() {
    FROZEN_RULE_GET_IMPL.init(|rule| {
        let rule = unpack_frozen_rule(rule)?;
        Ok(rule.implementation.dupe().to_frozen_value())
    })
}

pub(crate) fn init_frozen_promise_artifact_mappings_get_impl() {
    FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL.init(|rule| {
        let rule = unpack_frozen_rule(rule)?;
        Ok(rule
            .artifact_promise_mappings
            .as_ref()
            .map_or_else(SmallMap::new, |m| m.mappings.clone()))
    })
}

impl FrozenStarlarkRuleCallable {
    pub fn rule_type(&self) -> &Arc<StarlarkRuleType> {
        &self.rule_type
    }

    pub fn attributes(&self) -> &AttributeSpec {
        &self.rule.attributes
    }

    pub fn artifact_promise_mappings(&self) -> &Option<FrozenArtifactPromiseMappings> {
        &self.artifact_promise_mappings
    }
}

#[starlark_value(type = "Rule")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkRuleCallable {
    type Canonical = StarlarkRuleCallable<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let record_target_call_stack =
            ModuleInternals::from_context(eval, self.rule.rule_type.name())?
                .record_target_call_stacks();
        let call_stack = if record_target_call_stack {
            Some(eval.call_stack())
        } else {
            None
        };
        let arg_count = args.len()?;
        self.signature
            .parser(args, eval, |param_parser, eval| {
                // The body of the callable returned by `rule()`.
                // Records the target in this package's `TargetMap`.
                let internals = ModuleInternals::from_context(eval, self.rule.rule_type.name())?;
                let target_node = TargetNode::from_params(
                    self.rule.dupe(),
                    internals.package(),
                    internals,
                    param_parser,
                    arg_count,
                    self.ignore_attrs_for_profiling,
                    call_stack,
                )?;
                internals.record(target_node)?;
                Ok(Value::new_none())
            })
            .map_err(Into::into)
    }

    fn documentation(&self) -> DocItem {
        self.rule_docs.clone()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.ty.clone())
    }

    fn get_type_starlark_repr() -> Ty {
        StarlarkRuleCallable::get_type_starlark_repr()
    }
}

// ============================================================================
// ExecGroupValue - Returned by exec_group() function
// ============================================================================

/// A Bazel execution group value.
///
/// Created by the `exec_group()` function and stored in rule definitions.
/// At analysis time, these are replaced by resolved exec group info in `ctx.exec_groups`.
///
/// TODO(bazel): Implement proper execution group support with real toolchain resolution.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct ExecGroupValue<'v> {
    /// Toolchain requirements for this execution group
    toolchains: Value<'v>,
    /// Execution compatibility constraints
    exec_compatible_with: Value<'v>,
}

impl<'v> std::fmt::Display for ExecGroupValue<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exec_group()")
    }
}

impl<'v> AllocValue<'v> for ExecGroupValue<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

#[starlark_value(type = "exec_group")]
impl<'v> StarlarkValue<'v> for ExecGroupValue<'v> {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "toolchains" | "exec_compatible_with")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "toolchains" => Some(self.toolchains),
            "exec_compatible_with" => Some(self.exec_compatible_with),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["toolchains".to_owned(), "exec_compatible_with".to_owned()]
    }
}

#[starlark_module]
pub fn register_rule_function(builder: &mut GlobalsBuilder) {
    /// Define a rule. Supports both Bazel-style (`implementation`) and Kuro-style (`impl`)
    /// parameter names for the implementation function.
    ///
    /// Bazel example:
    /// ```python
    /// def _my_rule(ctx: AnalysisContext) -> list[Provider]:
    ///     output = ctx.actions.write("hello.txt", ctx.attrs.contents, executable = ctx.attrs.exe)
    ///     return [DefaultInfo(outputs = [output])]
    ///
    /// MyRule = rule(
    ///     implementation = _my_rule,  # Bazel-style
    ///     attrs = {
    ///         "contents": attrs.string(),
    ///         "exe": attrs.option(attrs.bool(), default = False),
    ///     },
    /// )
    /// ```
    ///
    /// Kuro example (also supported):
    /// ```python
    /// MyRule = rule(impl = _my_rule, attrs = {...})
    /// ```
    fn rule<'v>(
        // Bazel also allows passing implementation as first positional arg:
        //   rule(_my_impl, attrs = {...})
        #[starlark(default = NoneOr::None)] positional_impl: NoneOr<
            StarlarkCallableChecked<
                'v,
                (AnalysisContextReprLate,),
                Either<ListType<ProviderReprLate>, StarlarkPromise<'v>>,
            >,
        >,
        // Kuro-style parameter name
        #[starlark(require = named)] r#impl: Option<
            StarlarkCallableChecked<
                'v,
                (AnalysisContextReprLate,),
                Either<ListType<ProviderReprLate>, StarlarkPromise<'v>>,
            >,
        >,
        // Bazel-style parameter name (preferred)
        #[starlark(require = named)] implementation: Option<
            StarlarkCallableChecked<
                'v,
                (AnalysisContextReprLate,),
                Either<ListType<ProviderReprLate>, StarlarkPromise<'v>>,
            >,
        >,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named)] cfg: Option<ValueOfUnchecked<'v, TransitionReprLate>>,
        #[starlark(require = named)] supports_incoming_transition: Option<bool>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] is_configuration_rule: bool,
        #[starlark(require = named, default = false)] is_toolchain_rule: bool,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        uses_plugins: UnpackListOrTuple<PluginKindArg>,
        // Bazel-compatible: providers that this rule outputs (not yet implemented, just accepted)
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        provides: UnpackListOrTuple<Value<'v>>,
        // Bazel-compatible: toolchains required by this rule (not yet implemented, just accepted)
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        toolchains: UnpackListOrTuple<Value<'v>>,
        // Bazel-compatible: configuration fragments required by this rule (e.g., ["cpp", "java"])
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        fragments: UnpackListOrTuple<&str>,
        // Bazel-compatible: subrules that this rule uses (not yet implemented, just accepted)
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        subrules: UnpackListOrTuple<Value<'v>>,
        // Bazel-compatible: initializer function called before implementation (not yet implemented)
        #[starlark(require = named)] initializer: Option<Value<'v>>,
        // Bazel-compatible: execution groups for the rule (not yet implemented)
        #[starlark(require = named)] exec_groups: Option<Value<'v>>,
        // Bazel-compatible: implicit outputs defined by the rule (e.g., {"dwp_file": "%{name}.dwp"})
        #[starlark(require = named)] outputs: Option<Value<'v>>,
        // Bazel-compatible: whether this rule produces an executable
        #[starlark(require = named, default = false)] executable: bool,
        // Bazel-compatible: whether this rule is a test rule
        #[starlark(require = named, default = false)] test: bool,
        // Bazel-compatible: build setting descriptor (e.g., config.bool(flag=True))
        // TODO(bazel): Implement build setting value tracking via build_setting parameter
        #[starlark(require = named)] build_setting: Option<Value<'v>>,
        // Catch-all for Bazel private params like _skylark_testable
        #[starlark(kwargs)] extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkRuleCallable<'v>> {
        // TODO(bazel): Use the provides parameter to validate rule outputs
        // TODO(bazel): Use the toolchains parameter for toolchain resolution
        // TODO(bazel): Use the fragments parameter for configuration fragment access
        // TODO(bazel): Use the subrules parameter for subrule composition
        // TODO(bazel): Use the initializer parameter for pre-analysis attribute validation
        // TODO(bazel): Use the exec_groups parameter for execution groups
        // TODO(bazel): Use the outputs parameter for implicit outputs
        // TODO(bazel): Use the executable parameter
        let _unused = (
            provides,
            toolchains,
            fragments,
            subrules,
            initializer,
            exec_groups,
            outputs,
            executable,
            extra_kwargs,
        );

        // When build_setting is specified, add build_setting_default as an implicit attribute.
        // In Bazel, rules with build_setting automatically accept build_setting_default.
        // Check if the build setting has allow_multiple by checking if it has that attribute
        let build_setting_allows_multiple = build_setting
            .and_then(|v| v.get_attr("allow_multiple", eval.heap()).ok().flatten())
            .and_then(|v| v.unpack_bool())
            .unwrap_or(false);
        let mut attrs = attrs;
        if build_setting.is_some() {
            // Add build_setting_default as an any-typed, non-mandatory attribute
            let bsd_attr = StarlarkAttribute::new(Attribute::new(
                Some(Arc::new(CoercedAttr::None)),
                "Default value for the build setting",
                AttrType::any(),
            ));
            let bsd_value = eval.heap().alloc(bsd_attr);
            let bsd_ref = <&StarlarkAttribute>::unpack_value_err(bsd_value).unwrap();
            attrs.entries.push(("build_setting_default", bsd_ref));

            // Add help as an optional string attribute for build settings
            let help_attr = StarlarkAttribute::new(Attribute::new(
                Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                    "",
                ))))),
                "Help text for the build setting",
                AttrType::string(),
            ));
            let help_value = eval.heap().alloc(help_attr);
            let help_ref = <&StarlarkAttribute>::unpack_value_err(help_value).unwrap();
            attrs.entries.push(("help", help_ref));

            // Add _build_setting_allows_multiple hidden attribute
            if build_setting_allows_multiple {
                let am_attr = StarlarkAttribute::new(Attribute::new(
                    Some(Arc::new(CoercedAttr::Bool(
                        kuro_node::attrs::attr_type::bool::BoolLiteral(true),
                    ))),
                    "Whether this build setting allows multiple values",
                    AttrType::bool(),
                ));
                let am_value = eval.heap().alloc(am_attr);
                let am_ref = <&StarlarkAttribute>::unpack_value_err(am_value).unwrap();
                attrs
                    .entries
                    .push(("_build_setting_allows_multiple", am_ref));
            }
        }

        // Bazel: test=True rules get standard test attributes automatically.
        // Use any() for flaky/local/shard_count since Bazel accepts bool/int/string
        // (e.g., abseil-cpp uses `flaky = 1` instead of `flaky = True`).
        if test {
            let test_attrs: &[(&str, &str, AttrType)] = &[
                ("size", "small", AttrType::string()),
                ("timeout", "", AttrType::string()),
                ("flaky", "", AttrType::any()),
                ("shard_count", "", AttrType::any()),
                ("local", "", AttrType::any()),
                ("args", "", AttrType::list(AttrType::string())),
                (
                    "env",
                    "",
                    AttrType::dict(AttrType::string(), AttrType::string(), false),
                ),
                ("env_inherit", "", AttrType::list(AttrType::string())),
            ];
            for (name, default_val, attr_type) in test_attrs {
                // Only add if not already defined by the rule
                if !attrs.entries.iter().any(|(n, _)| *n == *name) {
                    let default = if default_val.is_empty() {
                        if *attr_type == AttrType::string() || *attr_type == AttrType::any() {
                            Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                                "",
                            )))))
                        } else if *name == "env" {
                            use kuro_node::attrs::attr_type::dict::DictLiteral;
                            Some(Arc::new(CoercedAttr::Dict(
                                DictLiteral(ArcSlice::default()),
                            )))
                        } else {
                            Some(Arc::new(CoercedAttr::List(
                                ListLiteral(ArcSlice::default()),
                            )))
                        }
                    } else {
                        Some(Arc::new(CoercedAttr::String(StringLiteral(ArcStr::from(
                            *default_val,
                        )))))
                    };
                    let test_attr =
                        StarlarkAttribute::new(Attribute::new(default, name, attr_type.clone()));
                    let test_value = eval.heap().alloc(test_attr);
                    let test_ref = <&StarlarkAttribute>::unpack_value_err(test_value).unwrap();
                    attrs.entries.push((name, test_ref));
                }
            }
        }

        // Support positional arg, `implementation` (Bazel), and `impl` (Kuro) parameter names
        let positional_impl = positional_impl.into_option();
        let impl_fn = match (positional_impl, implementation, r#impl) {
            (Some(positional), None, None) => positional,
            (None, Some(implementation), None) => implementation,
            (None, None, Some(impl_fn)) => impl_fn,
            (None, None, None) => {
                return Err(kuro_error::Error::from(RuleError::MissingImplementation).into());
            }
            _ => {
                return Err(kuro_error::Error::from(RuleError::BothImplAndImplementation).into());
            }
        };

        Ok(StarlarkRuleCallable::new(
            RuleImpl::BuildRule(StarlarkCallable::unchecked_new(impl_fn.0)),
            attrs,
            cfg.map(|v| v.get()),
            supports_incoming_transition,
            doc,
            is_configuration_rule,
            is_toolchain_rule,
            uses_plugins
                .items
                .into_iter()
                .map(|PluginKindArg { plugin_kind }| plugin_kind)
                .collect(),
            None,
            test,
            eval,
        )?)
    }

    /// Define an anon rule, similar to how a normal rule is defined, except with an extra `artifact_promise_mappings` field. This
    /// is a dict where the keys are the string name of the artifact, and the values are the callable functions that produce
    /// the artifact. This is only intended to be used with anon targets.
    fn anon_rule<'v>(
        #[starlark(require = named)] r#impl: StarlarkCallable<
            'v,
            (FrozenValue,),
            ListType<FrozenValue>,
        >,
        #[starlark(require = named)] attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named)] artifact_promise_mappings: SmallMap<
            StringValue<'v>,
            StarlarkCallable<'v, (FrozenValue,), UnpackList<FrozenValue>>,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkRuleCallable<'v>> {
        StarlarkRuleCallable::new_anon(r#impl, attrs, doc, artifact_promise_mappings, eval)
            .map_err(Into::into)
    }

    /// Type symbol for Rule.
    const Rule: StarlarkValueAsType<StarlarkRuleCallable> = StarlarkValueAsType::new();

    /// Define an execution group for a rule.
    ///
    /// Execution groups allow a rule to define different execution platforms for different
    /// actions. For example, a C++ rule might have one group for compilation and another
    /// for linking, each potentially running on different machines.
    ///
    /// Bazel example:
    /// ```python
    /// my_rule = rule(
    ///     exec_groups = {
    ///         "compile": exec_group(toolchains = use_cc_toolchain()),
    ///         "link": exec_group(toolchains = use_cc_toolchain()),
    ///     },
    /// )
    /// ```
    ///
    /// Currently this is a stub that returns a placeholder value.
    /// TODO(bazel): Implement proper execution group support.
    fn exec_group<'v>(
        // Bazel-compatible: toolchains required by this exec group
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        toolchains: UnpackListOrTuple<Value<'v>>,
        // Bazel-compatible: execution requirements (e.g., {"cpu": "4"})
        #[starlark(require = named)] exec_compatible_with: Option<Value<'v>>,
        // Bazel-compatible: copy from rule (no-op for stub)
        #[starlark(require = named, default = false)] copy_from_rule: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(bazel): Implement proper execution group support with real toolchain resolution
        let _ = copy_from_rule;
        let heap = eval.heap();
        Ok(heap.alloc(ExecGroupValue {
            toolchains: heap.alloc(toolchains.items),
            exec_compatible_with: exec_compatible_with.unwrap_or(Value::new_none()),
        }))
    }
}
