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

use dupe::Dupe;
use dupe::OptionDupedExt;
use either::Either;
use gazebo::prelude::*;
use kuro_common::package_listing::listing::PackageListing;
use kuro_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use kuro_core::configuration::transition::id::TransitionId;
use kuro_core::package::PackageLabel;
use kuro_core::plugins::PluginKindSet;
use kuro_core::target::label::interner::ConcurrentTargetLabelInterner;
use kuro_error::BuckErrorContext;
use kuro_fs::paths::file_name::FileNameBuf;
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
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::ValueError;
use starlark::values::ValueLike;
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
use crate::interpreter::build_context::PerFileTypeContext;
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

/// Resolve well-known configuration_field(fragment, name) pairs to real label strings.
///
/// In Bazel, `configuration_field(fragment="proto", name="proto_toolchain_for_cc")` reads
/// a value from the command-line configuration. Kuro doesn't have these flags, so we
/// map known fields to their well-known label targets.
fn resolve_configuration_field_to_label(fragment: &str, name: &str) -> Option<&'static str> {
    match (fragment, name) {
        ("proto", "proto_toolchain_for_cc") => Some("@bazel_tools//tools/proto:cc_toolchain"),
        ("proto", "proto_toolchain_for_java") => Some("@bazel_tools//tools/proto:java_toolchain"),
        ("proto", "proto_toolchain_for_javalite") => {
            Some("@bazel_tools//tools/proto:javalite_toolchain")
        }
        ("proto", "proto_compiler") => {
            Some("@protobuf//src/google/protobuf/compiler:protoc_minimal")
        }
        _ => None,
    }
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
        // Track configuration_field info before coercion loses it
        let mut config_field_info: Option<(String, String)> = None;

        let default = match default {
            None => None,
            Some(x) => {
                // Skip coercion for configuration_field() values - these are placeholders
                // indicating the value comes from configuration, not a static default.
                // The actual value will be resolved at analysis time from the configuration.
                // We treat these as having a default of None - this makes the attribute
                // optional (user doesn't need to provide a value), and the rule
                // implementation can check for None and handle it appropriately.
                //
                // For computed defaults (functions), Bazel calls the function during
                // analysis with (name, tags) to compute the actual default. We don't
                // support calling the function yet, but we treat it as having a default
                // of None. This makes the attribute optional (user doesn't need to
                // provide a value), and the rule implementation typically checks for
                // None and handles it appropriately (e.g., _def_parser in rules_cc).
                let value_type = x.get_type();
                if value_type == "configuration_field" {
                    // Extract fragment/name from the ConfigurationFieldRef before losing it.
                    // These are exposed via has_attr/get_attr on the StarlarkValue.
                    let heap = eval.heap();
                    let resolved_label =
                        if let Ok(Some(fragment_val)) = x.get_attr("fragment", heap) {
                            if let Ok(Some(name_val)) = x.get_attr("name", heap) {
                                let fragment = fragment_val.unpack_str().unwrap_or("").to_owned();
                                let name = name_val.unpack_str().unwrap_or("").to_owned();
                                let label = resolve_configuration_field_to_label(&fragment, &name);
                                config_field_info = Some((fragment, name));
                                label
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                    // If we resolved the configuration_field to a known label, coerce it
                    // as a real dependency default. Otherwise fall back to None.
                    match resolved_label {
                        Some(label_str) => {
                            let label_value = eval.heap().alloc(label_str);
                            let coerce_ctx = match attr_coercion_context_for_bzl(eval) {
                                Ok(ctx) => ctx,
                                Err(_) => {
                                    // No coercion context available (standalone mode) - skip default
                                    return Ok(StarlarkAttribute::new(Attribute::new(
                                        Some(Arc::new(
                                            kuro_node::attrs::coerced_attr::CoercedAttr::None,
                                        )),
                                        doc,
                                        coercer,
                                    )));
                                }
                            };
                            match coercer.coerce(AttrIsConfigurable::Yes, &coerce_ctx, label_value)
                            {
                                Ok(coerced) => Some(Arc::new(coerced)),
                                Err(_) => Some(Arc::new(
                                    kuro_node::attrs::coerced_attr::CoercedAttr::None,
                                )),
                            }
                        }
                        None => Some(Arc::new(kuro_node::attrs::coerced_attr::CoercedAttr::None)),
                    }
                } else if value_type == "function" || x.is_none() {
                    // Computed default or explicit None: use None as the default value.
                    // The rule implementation should check for None and handle it.
                    // For explicit None (from mandatory=False attributes), this makes
                    // the attribute optional without requiring coercion.
                    Some(Arc::new(kuro_node::attrs::coerced_attr::CoercedAttr::None))
                } else {
                    // If we can't get a coercion context (e.g., standalone/sync evaluator
                    // without a BuildContext), fall back to None default. The rule will
                    // re-coerce the default from BUILD file context when instantiated.
                    match attr_coercion_context_for_bzl(eval) {
                        Err(_) => Some(Arc::new(kuro_node::attrs::coerced_attr::CoercedAttr::None)),
                        Ok(coerce_ctx) => {
                            match coercer.coerce(AttrIsConfigurable::Yes, &coerce_ctx, x) {
                                Ok(coerced) => Some(Arc::new(coerced)),
                                Err(_) => {
                                    // Coercion failed (e.g., bare filename like "LICENSE").
                                    // Fall back to None - the rule will re-coerce from BUILD context.
                                    Some(Arc::new(
                                        kuro_node::attrs::coerced_attr::CoercedAttr::None,
                                    ))
                                }
                            }
                        }
                    }
                }
            }
        };
        let mut attr = Attribute::new(default, doc, coercer);
        if let Some((fragment, name)) = config_field_info {
            attr = attr.with_configuration_field(fragment, name);
        }
        Ok(StarlarkAttribute::new(attr))
    }
}

/// Coercion context for evaluating bzl files (attr default, transition rules).
pub(crate) fn attr_coercion_context_for_bzl<'v>(
    eval: &Evaluator<'v, '_, '_>,
) -> kuro_error::Result<BuildAttrCoercionContext> {
    let build_context = BuildContext::from_context(eval)?;

    // For bzl files, use the bzl file's package as context for relative label resolution.
    // This allows defaults like `default = "empty.tar"` to coerce to `:empty.tar`
    // relative to the bzl file's package (matching Bazel semantics where attr defaults
    // are resolved relative to the .bzl file's package).
    if let PerFileTypeContext::Bzl(bzl_ctx) = &build_context.additional {
        let bzl_package_path = bzl_ctx.bzl_path.path_parent();
        if let Ok(package_label) = PackageLabel::from_cell_path(bzl_package_path) {
            // Use empty listing - we don't have real file listing here,
            // but that's OK since we only need label resolution, not path coercion.
            let empty_listing = PackageListing::empty(FileNameBuf::unchecked_new("BUILD.bazel"));
            let bzl_cell_path = bzl_package_path.to_owned();
            return Ok(BuildAttrCoercionContext::new_with_package(
                build_context.cell_info().cell_resolver().dupe(),
                build_context.cell_info().cell_alias_resolver().dupe(),
                (package_label, empty_listing),
                false,
                // It is OK to not deduplicate because we don't coerce a lot of labels in bzl files.
                Arc::new(ConcurrentTargetLabelInterner::default()),
                CellPathWithAllowedRelativeDir::backwards_relative_not_supported(bzl_cell_path),
            ));
        }
    }

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
/// Handles the case where some providers are `None` (e.g., `CcInfo = None` for Bazel compat)
/// by skipping them rather than failing.
///
/// Also handles Bazel's nested list syntax for "any-of" provider constraints:
/// - `[ProviderA, ProviderB]` - flat list, all required
/// - `[[ProviderA], [ProviderB]]` - nested list, any-of (currently flattened)
fn dep_like_attr_handle_providers_arg(providers: Vec<Value>) -> kuro_error::Result<ProviderIdSet> {
    let mut result = Vec::new();
    for v in providers {
        // Skip None/NoneType values - this handles Bazel-compat providers like CcInfo that are
        // set to None because they're defined in Starlark (rules_cc) rather than native.
        // We check both is_none() (for the canonical None) and type name (for NoneType constants).
        if v.is_none() || v.get_type() == "NoneType" {
            continue;
        }

        // Check if this element is a nested list (for any-of provider constraints)
        // Bazel syntax: providers = [[ProviderA], [ProviderB]] means any-of
        // (dep must satisfy at least ONE group, not all).
        // Since ProviderIdSet doesn't support OR semantics, we skip nested lists
        // to be permissive rather than overly restrictive (requiring ALL groups).
        // TODO(bazel): Properly implement any-of provider constraint semantics
        if starlark::values::list::ListRef::from_value(v).is_some() {
            // OR group detected - skip to avoid requiring ALL groups
            continue;
        }

        match v.as_provider_callable() {
            Some(callable) => result.push(callable.id()?.dupe()),
            None => {
                return Err(
                    starlark::Error::from(ValueError::IncorrectParameterTypeNamed(
                        "providers".to_owned(),
                    ))
                    .into(),
                );
            }
        }
    }
    Ok(ProviderIdSet::from(result))
}

/// Helper to parse allow_files/allow_single_file parameters.
/// These can be:
/// - None/unset -> false
/// - bool -> the bool value
/// - list of strings -> true (with extensions, not yet filtered)
fn parse_allow_files_param<'v>(
    value: Option<Value<'v>>,
    param_name: &str,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<bool> {
    match value {
        None => Ok(false),
        Some(v) => {
            if let Some(b) = v.unpack_bool() {
                Ok(b)
            } else if v.iterate(eval.heap()).is_ok() {
                // It's a list/tuple of extensions - treat as allow_files=true
                // The extension filtering is not implemented yet, but we accept the param
                Ok(true)
            } else {
                Err(ValueError::IncorrectParameterTypeNamed(param_name.to_owned()).into())
            }
        }
    }
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
        // Bazel-compatible: restrict to specific values (e.g., ["expanded", "hir"])
        // Currently accepted but not enforced
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        values: UnpackListOrTuple<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = values;
        // Bazel semantics: if mandatory = False (default) and no default, use empty string
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(eval.heap().alloc("")),
            (None, true) => None,
        };
        Ok(Attribute::attr(
            eval,
            effective_default,
            doc,
            AttrType::string(),
        )?)
    }

    /// Takes an int from the user, supplies an int to the rule.
    /// Bazel-compatible alias for attrs.int().
    ///
    /// `values` restricts the allowed values to a specific set (not yet enforced).
    fn int<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        // Bazel-compatible: restrict to specific values (e.g., [0, 1, -1] for stamp attribute)
        // Currently accepted but not enforced
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        values: UnpackListOrTuple<i32>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // TODO(bazel): Enforce values constraint during coercion
        let _unused = values;
        // Bazel semantics: if mandatory = False (default) and no default, use 0
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(eval.heap().alloc(0)),
            (None, true) => None,
        };
        Ok(Attribute::attr(
            eval,
            effective_default,
            doc,
            AttrType::int(),
        )?)
    }

    /// Takes a boolean from the user, supplies a boolean to the rule.
    /// Bazel-compatible alias for attrs.bool().
    fn bool<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // Bazel semantics: if mandatory = False (default) and no default, use False
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(eval.heap().alloc(false)),
            (None, true) => None,
        };
        Ok(Attribute::attr(
            eval,
            effective_default,
            doc,
            AttrType::bool(),
        )?)
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
    ///
    /// `cfg` specifies a configuration transition:
    /// - `"exec"` - dependency runs in the execution platform (for tools)
    /// - `"target"` - dependency runs in the target platform (default)
    ///
    /// `aspects` is a list of aspects to be applied to the targets of this attribute.
    fn label<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] executable: bool,
        // Bazel-compatible: allow_files can be bool or list of extension strings
        // True = allow any file, False = don't allow, ["ext"] = allow files with extension
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        #[starlark(require = named)] allow_single_file: Option<Value<'v>>,
        // Bazel-compatible: configuration transition for the dependency
        // Can be a string ("exec", "target") or a config_transition object (config.exec(...))
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        // Bazel-compatible: allow_rules restricts which rule types can be used
        // Currently accepted but not enforced. Accepts None or list of strings.
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        // Bazel's `flags` parameter for internal attribute metadata (e.g., DIRECT_COMPILE_TIME_INPUT).
        // Currently accepted but ignored.
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        flags: UnpackListOrTuple<&str>,
        // Bazel-compatible: aspects to apply to targets of this attribute
        // Currently accepted but not executed (stub implementation - Phase 8a)
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        aspects: UnpackListOrTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // Parse allow_files: can be bool or list of extension strings
        let allow_files_bool = parse_allow_files_param(allow_files, "allow_files", eval)?;
        // Parse allow_single_file: can be bool or list of extension strings
        let allow_single_file_bool =
            parse_allow_files_param(allow_single_file, "allow_single_file", eval)?;
        // Either allow_files or allow_single_file means we accept source files
        let accept_files = allow_files_bool || allow_single_file_bool;
        // TODO(bazel): Enforce allow_rules constraint during coercion
        let _unused = (executable, allow_rules, flags);

        // Bazel semantics: if mandatory = False (default) and no default provided,
        // the attribute defaults to None. If mandatory = True, a value must be provided.
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => {
                // Not mandatory and no default -> default to None
                Some(eval.heap().alloc(starlark::values::none::NoneType))
            }
            (None, true) => None, // Mandatory, no default -> required attribute
        };

        // Extract aspect types from the aspects parameter (Phase 8c - UPDATED)
        // Note: aspects may be unfrozen at rule definition time, so handle both cases
        use std::sync::Arc;

        use crate::aspect::FrozenStarlarkAspectCallable;
        use crate::aspect::StarlarkAspectCallable;
        let mut aspect_types = Vec::new();
        for aspect_val in aspects.items {
            // Try frozen first, then unfrozen
            if let Some(frozen) = aspect_val.unpack_frozen() {
                if let Some(aspect) = frozen.downcast_ref::<FrozenStarlarkAspectCallable>() {
                    aspect_types.push(Arc::new(aspect.aspect_type()));
                } else {
                    return Err(
                        ValueError::IncorrectParameterTypeNamed("aspects".to_owned()).into(),
                    );
                }
            } else if let Some(aspect) = aspect_val.downcast_ref::<StarlarkAspectCallable>() {
                // For unfrozen aspects, use the aspect_type_unfrozen method
                aspect_types.push(Arc::new(aspect.aspect_type_unfrozen()?));
            } else {
                return Err(ValueError::IncorrectParameterTypeNamed("aspects".to_owned()).into());
            }
        }

        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        // Handle cfg parameter: "exec" or config.exec(...) means use exec_dep, otherwise use regular dep
        let is_exec = match cfg {
            Some(v) => {
                if let Some(s) = v.unpack_str() {
                    s == "exec"
                } else {
                    // Check if it's a config.exec transition (contains "exec" in repr)
                    v.to_repr().contains("exec")
                }
            }
            None => false,
        };
        // Build the coercer based on cfg and allow_files
        // IMPORTANT: Try dep first, then source. Both accept label strings like ":foo",
        // but deps (targets) should take precedence over source files.
        let coercer = if accept_files {
            let dep_type = if is_exec {
                AttrType::exec_dep(required_providers)
            } else {
                AttrType::dep(required_providers, PluginKindSet::EMPTY)
            };
            AttrType::one_of(vec![
                dep_type,
                AttrType::source(false), // allow_directory = false
            ])
        } else if is_exec {
            AttrType::exec_dep(required_providers)
        } else {
            AttrType::dep(required_providers, PluginKindSet::EMPTY)
        };

        // Create attribute with aspects attached (Phase 8c)
        let base_attr = Attribute::attr(eval, effective_default, doc, coercer)?;
        Ok(if aspect_types.is_empty() {
            base_attr
        } else {
            StarlarkAttribute::new(base_attr.clone_attribute().with_aspects(aspect_types))
        })
    }

    /// Takes a list of target labels from the user and supplies a list of
    /// dependencies to the rule.
    /// Bazel-compatible: equivalent to attrs.list(attrs.dep()).
    ///
    /// `aspects` is a list of aspects to be applied to the targets of this attribute.
    /// `allow_empty` controls whether the list can be empty (default True).
    /// `allow_rules` restricts which rule types can be used (not yet enforced).
    fn label_list<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        // Bazel-compatible: allow_files can be bool or list of extension strings
        #[starlark(require = named)] allow_files: Option<Value<'v>>,
        // Bazel-compatible: whether the list can be empty (default True)
        // Currently accepted but not enforced
        #[starlark(require = named, default = true)] allow_empty: bool,
        // Bazel-compatible: allow_rules restricts which rule types can be used
        // Currently accepted but not enforced. Accepts None or list of strings.
        #[starlark(require = named)] allow_rules: Option<Value<'v>>,
        // Bazel's `flags` parameter for internal attribute metadata (e.g., DIRECT_COMPILE_TIME_INPUT).
        // Currently accepted but ignored.
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        flags: UnpackListOrTuple<&str>,
        // Bazel-compatible: configuration transition for dependencies
        // Can be a string ("exec", "target") or a config_transition object
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        // Bazel-compatible: aspects to apply to targets of this attribute
        // Currently accepted but not executed (stub implementation - Phase 8a)
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        aspects: UnpackListOrTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let allow_files_bool = parse_allow_files_param(allow_files, "allow_files", eval)?;
        // TODO(bazel): Enforce allow_empty constraint during coercion
        // TODO(bazel): Enforce allow_rules constraint during coercion
        let _unused = (allow_empty, allow_rules, flags);

        // Bazel semantics: if mandatory = False (default) and no default provided,
        // the attribute defaults to an empty list. If mandatory = True, a value must be provided.
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => {
                // Not mandatory and no default -> default to empty list
                Some(eval.heap().alloc(Vec::<Value>::new()))
            }
            (None, true) => None, // Mandatory, no default -> required attribute
        };

        // Extract aspect types from the aspects parameter (Phase 8c - UPDATED)
        // Note: aspects may be unfrozen at rule definition time, so handle both cases
        use std::sync::Arc;

        use crate::aspect::FrozenStarlarkAspectCallable;
        use crate::aspect::StarlarkAspectCallable;
        let mut aspect_types = Vec::new();
        for aspect_val in aspects.items {
            // Try frozen first, then unfrozen
            if let Some(frozen) = aspect_val.unpack_frozen() {
                if let Some(aspect) = frozen.downcast_ref::<FrozenStarlarkAspectCallable>() {
                    aspect_types.push(Arc::new(aspect.aspect_type()));
                } else {
                    return Err(
                        ValueError::IncorrectParameterTypeNamed("aspects".to_owned()).into(),
                    );
                }
            } else if let Some(aspect) = aspect_val.downcast_ref::<StarlarkAspectCallable>() {
                // For unfrozen aspects, use the aspect_type_unfrozen method
                aspect_types.push(Arc::new(aspect.aspect_type_unfrozen()?));
            } else {
                return Err(ValueError::IncorrectParameterTypeNamed("aspects".to_owned()).into());
            }
        }

        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        // Handle cfg parameter: "exec" or config.exec(...) means use exec_dep
        let is_exec = match cfg {
            Some(v) => {
                if let Some(s) = v.unpack_str() {
                    s == "exec"
                } else {
                    v.to_repr().contains("exec")
                }
            }
            None => false,
        };
        // When allow_files = True, accept both source files and deps
        // This is critical for srcs attributes which can contain "file.c" or ":target"
        // IMPORTANT: Try dep first, then source. Both accept label strings like ":foo",
        // but deps (targets) should take precedence over source files. Source files
        // are typically specified as bare filenames like "file.c", not labels.
        let dep_type = if is_exec {
            AttrType::exec_dep(required_providers)
        } else {
            AttrType::dep(required_providers, PluginKindSet::EMPTY)
        };
        let inner = if allow_files_bool {
            AttrType::one_of(vec![
                dep_type,
                AttrType::source(false), // allow_directory = false
            ])
        } else {
            dep_type
        };
        let coercer = AttrType::list(inner);

        // Create attribute with aspects attached (Phase 8c)
        let base_attr = Attribute::attr(eval, effective_default, doc, coercer)?;
        Ok(if aspect_types.is_empty() {
            base_attr
        } else {
            StarlarkAttribute::new(base_attr.clone_attribute().with_aspects(aspect_types))
        })
    }

    /// Takes a list of strings from the user.
    /// Bazel-compatible: equivalent to attrs.list(attrs.string()).
    fn string_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        // Bazel-compatible: whether the list can be empty (default True)
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // TODO(bazel): Enforce allow_empty constraint during coercion
        let _unused = allow_empty;
        // Bazel semantics: if mandatory = False (default) and no default, use empty list
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(eval.heap().alloc(Vec::<Value>::new())),
            (None, true) => None,
        };
        let coercer = AttrType::list(AttrType::string());
        Ok(Attribute::attr(eval, effective_default, doc, coercer)?)
    }

    /// Takes a list of integers from the user.
    /// Bazel-compatible: equivalent to attrs.list(attrs.int()).
    fn int_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        // Bazel-compatible: whether the list can be empty (default True)
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        // TODO(bazel): Enforce allow_empty constraint during coercion
        let _unused = allow_empty;
        // Bazel semantics: if mandatory = False (default) and no default, use empty list
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(eval.heap().alloc(Vec::<Value>::new())),
            (None, true) => None,
        };
        let coercer = AttrType::list(AttrType::int());
        Ok(Attribute::attr(eval, effective_default, doc, coercer)?)
    }

    /// Takes a dict with string keys and string values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.string()).
    fn string_dict<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = allow_empty;
        // Bazel semantics: if mandatory = False (default) and no default, use empty dict
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(
                eval.heap()
                    .alloc(starlark::collections::SmallMap::<Value, Value>::new()),
            ),
            (None, true) => None,
        };
        let coercer = AttrType::dict(AttrType::string(), AttrType::string(), false);
        Ok(Attribute::attr(eval, effective_default, doc, coercer)?)
    }

    /// Takes a dict with string keys and list of string values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.list(attrs.string())).
    fn string_list_dict<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (mandatory, allow_empty);
        let coercer = AttrType::dict(
            AttrType::string(),
            AttrType::list(AttrType::string()),
            false,
        );
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }

    /// Takes a dict with label keys and string values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.dep(), attrs.string()).
    fn label_keyed_string_dict<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] allow_files: bool,
        #[starlark(require = named, default = false)] allow_empty: bool,
        // Bazel-compatible: configuration transition
        #[starlark(require = named)] cfg: Option<Value<'v>>,
        // Bazel-compatible: aspects to apply to dependencies
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        aspects: UnpackListOrTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (allow_files, allow_empty, cfg, aspects);
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let label_type = AttrType::dep(required_providers, PluginKindSet::EMPTY);
        let coercer = AttrType::dict(label_type, AttrType::string(), false);
        // Bazel semantics: non-mandatory dicts default to empty dict
        let effective_default = match (default, mandatory) {
            (Some(d), _) => Some(d),
            (None, false) => Some(
                eval.heap()
                    .alloc(starlark::collections::SmallMap::<Value, Value>::new()),
            ),
            (None, true) => None,
        };
        Ok(Attribute::attr(eval, effective_default, doc, coercer)?)
    }

    /// Takes a dict with string keys and label values.
    /// Bazel-compatible: equivalent to attrs.dict(attrs.string(), attrs.dep()).
    /// Used by @bazel_tools//tools/build_defs/repo:http.bzl for the "files" attribute.
    fn string_keyed_label_dict<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = false)] allow_files: bool,
        #[starlark(require = named, default = false)] allow_single_file: bool,
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (mandatory, allow_files, allow_single_file, allow_empty);
        let required_providers = dep_like_attr_handle_providers_arg(providers.items)?;
        let label_type = AttrType::dep(required_providers, PluginKindSet::EMPTY);
        // Note: string keys, label values (inverse of label_keyed_string_dict)
        let coercer = AttrType::dict(AttrType::string(), label_type, false);
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
        // Output attributes in Bazel declare output file names.
        // We mark this as an output attr so that when this target is recorded,
        // the output filename is registered for Bazel-compatible output file label resolution.
        let mut sa = Attribute::attr(eval, default, doc, AttrType::string())?;
        sa.is_output = true;
        Ok(sa)
    }

    /// Declares a list of output files that the rule will generate.
    fn output_list<'v>(
        #[starlark(require = named)] default: Option<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] mandatory: bool,
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAttribute> {
        let _unused = (mandatory, allow_empty);
        let coercer = AttrType::list(AttrType::string());
        Ok(Attribute::attr(eval, default, doc, coercer)?)
    }
}
