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

use kuro_core::configuration::transition::id::TransitionId;
use kuro_core::plugins::PluginKindSet;
use kuro_core::target::label::interner::ConcurrentTargetLabelInterner;
use kuro_error::BuckErrorContext;
use kuro_interpreter::coerce::COERCE_PROVIDERS_LABEL_FOR_BZL;
use kuro_interpreter::types::provider::callable::ValueAsProviderCallableLike;
use kuro_interpreter::types::transition::transition_id_from_value;
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::attr_type::any::AnyAttrType;
use kuro_node::attrs::coercion_context::AttrCoercionContext;
use kuro_node::attrs::configurable::AttrIsConfigurable;
use kuro_node::attrs::display::AttrDisplayWithContextExt;
use kuro_node::provider_id_set::ProviderIdSet;
use dupe::Dupe;
use dupe::OptionDupedExt;
use either::Either;
use gazebo::prelude::*;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::ValueError;
use starlark::values::ValueOf;
use starlark::values::ValueTypedComplex;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::tuple::UnpackTuple;
use tracing::error;

use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::ctx::BuildAttrCoercionContext;
use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::attrs::starlark_attribute::register_attr_type;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::selector::StarlarkSelector;
use crate::plugins::AllPlugins;
use crate::plugins::PluginKindArg;

const OPTION_NONE_EXPLANATION: &str = "`None` as an attribute value always picks the default. For `attrs.option`, if the default isn't `None`, there is no way to express `None`.";

#[derive(kuro_error::Error, Debug)]
#[kuro(input)]
enum AttrError {
    #[error(
        "`attrs.option` `default` parameter must be `None` or absent, got `{0}`.\n{}",
        OPTION_NONE_EXPLANATION
    )]
    OptionDefaultNone(String),
    #[error("`attrs.default_only` argument must have a default")]
    DefaultOnlyMustHaveDefault,
}

pub(crate) trait AttributeExt {
    /// Helper to create an attribute from attrs.foo functions
    fn attr<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        default: Option<Value<'v>>,
        doc: &str,
        coercer: AttrType,
    ) -> kuro_error::Result<StarlarkAttribute>;
}

impl AttributeExt for Attribute {
    /// Helper to create an attribute from attrs.foo functions
    fn attr<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
        default: Option<Value<'v>>,
        doc: &str,
        coercer: AttrType,
    ) -> kuro_error::Result<StarlarkAttribute> {
        let default = match default {
            None => None,
            Some(x) => Some(Arc::new(
                coercer
                    .coerce(
                        AttrIsConfigurable::Yes,
                        &attr_coercion_context_for_bzl(eval)?,
                        x,
                    )
                    .buck_error_context("Error coercing attribute default")?,
            )),
        };
        Ok(StarlarkAttribute::new(Attribute::new(
            default, doc, coercer,
        )))
    }
}

/// Coerction context for evaluating bzl files (attr default, transition rules).
pub(crate) fn attr_coercion_context_for_bzl<'v>(
    eval: &Evaluator<'v, '_, '_>,
) -> kuro_error::Result<BuildAttrCoercionContext> {
    let build_context = BuildContext::from_context(eval)?;
    Ok(BuildAttrCoercionContext::new_no_package(
        build_context.cell_info().cell_resolver().dupe(),
        build_context.cell_info().name().name(),
        build_context.cell_info().cell_alias_resolver().dupe(),
        // It is OK to not deduplicate because we don't coerce a lot of labels in bzl files.
        Arc::new(ConcurrentTargetLabelInterner::default()),
    ))
}

pub(crate) fn init_coerce_providers_label_for_bzl() {
    COERCE_PROVIDERS_LABEL_FOR_BZL
        .init(|eval, value| attr_coercion_context_for_bzl(eval)?.coerce_providers_label(value))
}

/// Common code to handle `providers` argument of dep-like attrs.
fn dep_like_attr_handle_providers_arg(providers: Vec<Value>) -> kuro_error::Result<ProviderIdSet> {
    Ok(ProviderIdSet::from(providers.try_map(|v| {
        match v.as_provider_callable() {
            Some(callable) => kuro_error::Ok(callable.id()?.dupe()),
            None => Err(
                starlark::Error::from(ValueError::IncorrectParameterTypeNamed(
                    "providers".to_owned(),
                ))
                .into(),
            ),
        }
    })?))
}

/// This type is available as a global `attrs` symbol, to allow the definition of attributes to the `rule` function.
///
/// As an example:
///
/// ```python
/// rule(impl = _impl, attrs = {"foo": attrs.string(), "bar": attrs.int(default = 42)})
/// ```
///
/// Most attributes take at least two optional parameters:
///
/// * A `doc` parameter, which specifies documentation for the attribute.
///
/// * A `default` parameter, which if present specifies the default value for the attribute if omitted.
///   If there is no default, the user of the rule must supply that parameter.
///
/// Each attribute defines what values it accepts from the user, and which values it gives to the rule.
/// For simple types like `attrs.string` these are the same, for more complex types like `attrs.dep` these
/// are different (string from the user, dependency to the rule).
#[starlark_module]
fn attr_module(registry: &mut GlobalsBuilder) {
    /// Takes a string from the user, supplies a string to the rule.
    fn string<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named)] validate: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = validate;
        Ok(Attribute::attr(eval, default, doc, AttrType::string())?)
    }

    /// Takes a list from the user, supplies a list to the rule.
    fn list<'v>(
        #[starlark(require = pos)] inner: &StarlarkAttribute,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let coercer = AttrType::list(inner.coercer_for_inner()?);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a target from the user, as a string, and supplies a dependency to the rule.
    /// The dependency will transition to the execution platform. Use `exec_dep` if you
    /// plan to execute things from this dependency as part of the compilation.
    fn exec_dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let coercer = AttrType::exec_dep(required_providers);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a target from the user, as a string, and supplies a dependency to the rule.
    /// The dependency will be a toolchain dependency, meaning that its execution platform
    /// dependencies will be used to select the execution platform for this rule.
    fn toolchain_dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let coercer = AttrType::toolchain_dep(required_providers);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    fn transition_dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let label_coercion_ctx = attr_coercion_context_for_bzl(eval)?;

        // FIXME(JakobDegen): Use a proper unpack for this. Easier to do after deleting old API
        let transition_id = if let Some(cfg) = cfg {
            Some(if let Some(s) = StringValue::new(cfg) {
                let transition_target = label_coercion_ctx.coerce_providers_label(&s)?;
                Arc::new(TransitionId::Target(transition_target))
            } else {
                transition_id_from_value(cfg)?
            })
        } else {
            None
        };

        let coercer = AttrType::transition_dep(required_providers, transition_id);
        let coerced_default = match default {
            None => None,
            Some(default) => {
                Some(coercer.coerce(AttrIsConfigurable::Yes, &label_coercion_ctx, default)?)
            }
        };

        Ok(StarlarkAttribute::new(Attribute::new(
            coerced_default.map(Arc::new),
            doc,
            coercer,
        )))
    }

    fn configured_dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let coercer = AttrType::configured_dep(required_providers);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    fn split_transition_dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] cfg: Value<'v>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let transition_id = transition_id_from_value(cfg)?;
        let coercer = AttrType::split_transition_dep(required_providers, transition_id);

        let coerced_default = match default {
            None => None,
            Some(default) => Some(coercer.coerce(
                AttrIsConfigurable::Yes,
                &attr_coercion_context_for_bzl(eval)?,
                default,
            )?),
        };

        Ok(StarlarkAttribute::new(Attribute::new(
            coerced_default.map(Arc::new),
            doc,
            coercer,
        )))
    }

    /// Takes a target label from the user and registers it as a plugin dependency.
    ///
    /// Plugin dependencies are propagated as unconfigured target labels up the build graph,
    /// then configured as exec deps when used by a rule with `uses_plugins`. This is useful
    /// for dependencies like Rust proc macros that need to be accessible to transitive dependents.
    ///
    /// See the [`plugins`](../plugins) namespace documentation for a full explanation and examples.
    fn plugin_dep<'v>(
        #[starlark(require = named)] kind: PluginKindArg,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(
            eval,
            default,
            doc,
            AttrType::plugin_dep(kind.plugin_kind),
        )?)
    }

    /// Takes a target from the user, as a string, and supplies a dependency to the rule.
    /// A target can be specified as an absolute dependency `foo//bar:baz`, omitting the
    /// cell (`//bar:baz`) or omitting the package name (`:baz`).
    ///
    /// If supplied the `providers` argument ensures that specific providers will be present
    /// on the dependency.
    ///
    /// The `pulls_plugins` and `pulls_and_pushes_plugins` parameters control plugin propagation.
    /// See the [`plugins`](../plugins) namespace documentation for a full explanation.
    fn dep<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        pulls_plugins: UnpackListOrTuple<PluginKindArg>,
        #[starlark(require = named, default = Either::Left(UnpackListOrTuple::default()))]
        pulls_and_pushes_plugins: Either<UnpackListOrTuple<PluginKindArg>, &'v AllPlugins>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let plugin_kinds = match pulls_and_pushes_plugins {
            Either::Right(_) => PluginKindSet::ALL,
            Either::Left(pulls_and_pushes_plugins) => {
                let pulls_and_pushes_plugins: Vec<_> = pulls_and_pushes_plugins
                    .items
                    .into_iter()
                    .map(|PluginKindArg { plugin_kind }| plugin_kind)
                    .collect();
                let pulls_plugins: Vec<_> = pulls_plugins
                    .items
                    .into_iter()
                    .map(|PluginKindArg { plugin_kind }| plugin_kind)
                    .collect();
                PluginKindSet::new(pulls_plugins, pulls_and_pushes_plugins)?
            }
        };

        let coercer = AttrType::dep(required_providers, plugin_kinds);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes most builtin literals and passes them to the rule as a string.
    /// Discouraged, as it provides little type safety and destroys the structure.
    fn any<'v>(
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named)] default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, default, doc, AttrType::any())?)
    }

    /// Takes a boolean and passes it to the rule as a boolean.
    fn bool<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, default, doc, AttrType::bool())?)
    }

    /// Takes a value that may be `None` or some inner type, and passes either `None` or the
    /// value corresponding to the inner to the rule. Often used to make a rule optional:
    ///
    /// ```python
    /// attrs.option(attr.string(), default = None)
    /// ```
    fn option<'v>(
        #[starlark(require = pos)] inner: &StarlarkAttribute,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let coercer = AttrType::option(inner.coercer_for_inner()?);
        let attr = Attribute::attr(eval, default, doc, coercer)?;

        match attr.default() {
            Some(default) if !default.may_return_none() => Err(kuro_error::Error::from(
                AttrError::OptionDefaultNone(default.as_display_no_ctx().to_string()),
            )
            .into()),
            _ => Ok(attr),
        }
    }

    /// Rejects all values and uses the default for the inner argument.
    /// Often used to resolve dependencies, which otherwise can't be resolved inside a rule.
    ///
    /// ```python
    /// attrs.default_only(attrs.dep(default = "foo//my_package:my_target"))
    /// ```
    fn default_only<'v>(
        #[starlark(require = pos)] inner: &StarlarkAttribute,
        #[starlark(require = named, default = "")] doc: &str,
    ) -> starlark::Result<StarlarkAttribute> {
        let Some(default) = inner.default().duped() else {
            return Err(kuro_error::Error::from(AttrError::DefaultOnlyMustHaveDefault).into());
        };
        Ok(StarlarkAttribute::new(Attribute::new_default_only(
            default,
            doc,
            inner.coercer_for_default_only(),
        )))
    }

    /// Takes a target (as per `deps`) and passes a `label` to the rule.
    /// Validates that the target exists, but does not introduce a dependency on it.
    fn label<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, default, doc, AttrType::label())?)
    }

    /// Takes a dict from the user, supplies a dict to the rule.
    fn dict<'v>(
        // TODO(nga): require positional only for key and value.
        key: &StarlarkAttribute,
        value: &StarlarkAttribute,
        #[starlark(require = named, default = false)] sorted: bool,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let coercer = AttrType::dict(key.coercer_for_inner()?, value.coercer_for_inner()?, sorted);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a command line argument from the user and supplies a `cmd_args` compatible value to the rule.
    /// The argument may contain special macros such as `$(location :my_target)` or `$(exe :my_target)` which
    /// will be replaced with references to those values in the rule. Takes in an optional `anon_target_compatible`
    /// flag, which indicates whether the args can be passed into anon targets. Note that there is a slight memory
    /// hit when using this flag.
    fn arg<'v>(
        #[starlark(require = named, default = false)] json: bool,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] anon_target_compatible: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = json;
        Ok(Attribute::attr(
            eval,
            default,
            doc,
            AttrType::arg(anon_target_compatible),
        )?)
    }

    /// Takes a string from one of the variants given, and gives that string to the rule.
    /// Strings are matched case-insensitively, and always passed to the rule lowercase.
    fn r#enum<'v>(
        #[starlark(require = pos)] variants: UnpackListOrTuple<String>,
        #[starlark(require = named)] default: Option<
            ValueOf<'v, Either<StringValue<'v>, ValueTypedComplex<'v, StarlarkSelector<'v>>>>,
        >,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // Value seems to usually be a `[String]`, listing the possible values of the
        // enumeration. Unfortunately, for things like `exported_lang_preprocessor_flags`
        // it ends up being `Type` which doesn't match the data we see.
        Ok(Attribute::attr(
            eval,
            default.map(|v| v.value),
            doc,
            AttrType::enumeration(variants.items)?,
        )?)
    }

    fn configuration_label<'v>(
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // TODO(nga): explain how this is different from `dep`.
        //   This probably meant to be similar to `label`, but not configurable.
        Ok(Attribute::attr(
            eval,
            None,
            doc,
            AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        )?)
    }

    /// Currently an alias for `attrs.string`.
    fn regex<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, default, doc, AttrType::string())?)
    }

    fn set<'v>(
        #[starlark(require = pos)] value_type: &StarlarkAttribute,
        #[starlark(require = named, default = false)] sorted: bool,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = sorted;
        let coercer = AttrType::list(value_type.coercer_for_inner()?);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    fn named_set<'v>(
        #[starlark(require = pos)] value_type: &StarlarkAttribute,
        #[starlark(require = named, default = false)] sorted: bool,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let value_coercer = value_type.coercer_for_inner()?;
        let coercer = AttrType::one_of(vec![
            AttrType::dict(AttrType::string(), value_coercer.dupe(), sorted),
            AttrType::list(value_coercer),
        ]);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Given a list of alternative attributes, selects the first that matches and gives that to the rule.
    fn one_of<'v>(
        #[starlark(args)] args: UnpackTuple<&StarlarkAttribute>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let coercer = AttrType::one_of(args.items.into_try_map(|arg| arg.coercer_for_inner())?);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a tuple of values and gives a tuple to the rule.
    fn tuple<'v>(
        #[starlark(args)] args: UnpackTuple<&StarlarkAttribute>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let coercer = AttrType::tuple(args.items.into_try_map(|arg| arg.coercer_for_inner())?);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes an int from the user, supplies an int to the rule.
    fn int<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, default, doc, AttrType::int())?)
    }

    fn query<'v>(
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(eval, None, doc, AttrType::query())?)
    }

    fn versioned<'v>(
        value_type: &StarlarkAttribute,
        #[starlark(require = named, default = "")] doc: &str,
    ) -> starlark::Result<StarlarkAttribute> {
        // A versioned field looks like:
        // [ ({"key":"value1"}, arg), ({"key":"value2"}, arg) ]
        let element_type = AttrType::tuple(vec![
            AttrType::dict(AttrType::string(), AttrType::string(), false),
            value_type.coercer_for_inner()?,
        ]);
        let coercer = AttrType::list(element_type.dupe());

        Ok(StarlarkAttribute::new(Attribute::new(
            Some(Arc::new(AnyAttrType::empty_list())),
            doc,
            coercer,
        )))
    }

    /// Takes a source file from the user, supplies an artifact to the rule.
    /// The source file may be specified as a literal string
    /// (representing the path within this package), or a target (which must have a
    /// `DefaultInfo` with a `default_outputs` value).
    fn source<'v>(
        #[starlark(require = named, default = false)] allow_directory: bool,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        Ok(Attribute::attr(
            eval,
            default,
            doc,
            AttrType::source(allow_directory),
        )?)
    }
}

pub(crate) fn register_attrs(globals: &mut GlobalsBuilder) {
    globals.namespace("attrs", attr_module);
    // Also register Bazel-compatible "attr" namespace (singular)
    globals.namespace("attr", bazel_attr_module);
    register_attr_type(globals);
}

/// Bazel-compatible attribute module.
///
/// This module provides Bazel 9.0-style attribute functions that map to Kuro's internal
/// attribute types. The naming follows Bazel conventions:
///
/// - `attr.label()` instead of `attrs.dep()`
/// - `attr.label_list()` instead of `attrs.list(attrs.dep())`
/// - `attr.string_list()` instead of `attrs.list(attrs.string())`
/// - etc.
///
/// Example:
/// ```python
/// my_rule = rule(
///     implementation = _impl,
///     attrs = {
///         "deps": attr.label_list(providers = [SomeInfo]),
///         "name": attr.string(mandatory = True),
///     },
/// )
/// ```
#[starlark_module]
fn bazel_attr_module(registry: &mut GlobalsBuilder) {
    /// Takes a string from the user, supplies a string to the rule.
    /// Bazel-compatible alias for attrs.string().
    fn string<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        Ok(Attribute::attr(eval, default, doc, AttrType::string())?)
    }

    /// Takes an int from the user, supplies an int to the rule.
    /// Bazel-compatible alias for attrs.int().
    fn int<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        Ok(Attribute::attr(eval, default, doc, AttrType::int())?)
    }

    /// Takes a boolean from the user, supplies a boolean to the rule.
    /// Bazel-compatible alias for attrs.bool().
    fn bool<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        Ok(Attribute::attr(eval, default, doc, AttrType::bool())?)
    }

    /// Takes a target label from the user (e.g., "//pkg:target") and supplies a
    /// dependency to the rule.
    /// Bazel-compatible alias for attrs.dep().
    ///
    /// If `providers` is specified, the dependency must return those providers.
    ///
    /// `allow_single_file` can be:
    /// - `True` (allow any single file)
    /// - `False` (default, don't allow files)
    /// - A list of extensions like `[".txt", ".json"]` (allow single file with those extensions)
    fn label<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = false)] allow_files: bool,
        #[starlark(require = named)] allow_single_file: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // Parse allow_single_file: can be bool or list of extension strings
        let allow_single_file_bool = match allow_single_file {
            None => false,
            Some(v) => {
                if let Some(b) = v.unpack_bool() {
                    b
                } else if v.iterate(eval.heap()).is_ok() {
                    // It's a list/tuple of extensions - treat as allow_single_file=true
                    // The extension filtering is not implemented yet, but we accept the param
                    true
                } else {
                    return Err(ValueError::IncorrectParameterTypeNamed(
                        "allow_single_file".to_owned(),
                    )
                    .into());
                }
            }
        };
        let _unused = (mandatory, executable, allow_files, allow_single_file_bool);
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let coercer = AttrType::dep(required_providers, PluginKindSet::EMPTY);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a list of target labels from the user and supplies a list of
    /// dependencies to the rule.
    /// Bazel-compatible: equivalent to attrs.list(attrs.dep()).
    fn label_list<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] allow_files: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (mandatory, allow_files);
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let inner = AttrType::dep(required_providers, PluginKindSet::EMPTY);
        let coercer = AttrType::list(inner);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a list of strings from the user.
    /// Bazel-compatible: equivalent to attrs.list(attrs.string()).
    fn string_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        let coercer = AttrType::list(AttrType::string());
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a list of integers from the user.
    /// Bazel-compatible: equivalent to attrs.list(attrs.int()).
    fn int_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        let coercer = AttrType::list(AttrType::int());
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a dict with string keys and string values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.string()).
    fn string_dict<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        let coercer = AttrType::dict(AttrType::string(), AttrType::string(), false);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a dict with string keys and list of string values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.list(attrs.string())).
    fn string_list_dict<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        let coercer = AttrType::dict(AttrType::string(), AttrType::list(AttrType::string()), false);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a dict with string keys and label values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.dep()).
    fn label_keyed_string_dict<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] allow_files: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (mandatory, allow_files);
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let label_type = AttrType::dep(required_providers, PluginKindSet::EMPTY);
        let coercer = AttrType::dict(label_type, AttrType::string(), false);
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Declares an output file that the rule will generate.
    /// In Kuro, this is handled via ctx.actions.declare_file() during analysis.
    /// This attribute accepts a string naming the output file.
    fn output<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        // Output attributes in Bazel are typically strings that name the output file.
        // The actual output declaration happens during the rule's implementation via
        // ctx.actions.declare_file(). We use a string attr to capture the name.
        Ok(Attribute::attr(eval, default, doc, AttrType::string())?)
    }

    /// Declares a list of output files that the rule will generate.
    fn output_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = mandatory;
        let coercer = AttrType::list(AttrType::string());
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }
}
