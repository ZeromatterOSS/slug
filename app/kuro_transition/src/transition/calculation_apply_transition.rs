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
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use dupe::OptionDupedExt;
use itertools::Itertools;
use kuro_build_api::actions::query::CONFIGURED_ATTR_TO_VALUE;
use kuro_build_api::actions::query::PackageLabelOption;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::interpreter::rule_defs::provider::builtin::platform_info::PlatformInfo;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use kuro_build_api::transition::TRANSITION_CALCULATION;
use kuro_build_api::transition::TransitionCalculation;
use kuro_core::configuration::cfg_diff::cfg_diff;
use kuro_core::configuration::data::ConfigurationData;
use kuro_core::configuration::transition::applied::TransitionApplied;
use kuro_core::configuration::transition::id::TransitionId;
use kuro_core::provider::label::ProvidersLabel;
use kuro_error::BuckErrorContext;
use kuro_events::dispatch::get_dispatcher;
use kuro_interpreter::dice::starlark_provider::StarlarkEvalKind;
use kuro_interpreter::factory::BuckStarlarkModule;
use kuro_interpreter::factory::StarlarkEvaluatorProvider;
use kuro_interpreter::print_handler::EventDispatcherPrintHandler;
use kuro_interpreter::soft_error::KuroStarlarkSoftErrorHandler;
use kuro_node::attrs::configured_attr::ConfiguredAttr;
use kuro_node::attrs::display::AttrDisplayWithContextExt;
use starlark::eval::Evaluator;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::structs::AllocStruct;
use starlark_map::ordered_map::OrderedMap;
use starlark_map::sorted_map::SortedMap;

use crate::transition::calculation_fetch_transition::FetchTransition;
use crate::transition::calculation_fetch_transition::TransitionData;

#[derive(kuro_error::Error, Debug)]
#[kuro(tag = Tier0)]
enum ApplyTransitionError {
    #[error("transition function not marked as `split` must return a `PlatformInfo`")]
    NonSplitTransitionMustReturnPlatformInfo,
    #[error("transition function marked `split` must return a dict of `str` to `PlatformInfo`")]
    SplitTransitionMustReturnDict,
    #[error(
        "transition applied again to transition output \
        did not produce identical `PlatformInfo`, the diff:\n{0}"
    )]
    SplitTransitionAgainDifferentPlatformInfo(String),
    #[error(
        "Transition object is not consistent with transition computation params, \
        this may happen because of how DICE recomputation works, \
        a user should never see this message"
    )]
    InconsistentTransitionAndComputation,
}

fn call_transition_function<'v>(
    transition: &TransitionData,
    conf: &ConfigurationData,
    refs: Value<'v>,
    attrs: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<TransitionApplied> {
    // Bazel-style transitions have inputs/outputs and use (settings, attr) signature
    if transition.is_bazel_style() {
        return call_bazel_transition_function(transition, conf, attrs, eval);
    }

    let mut args = vec![(
        "platform",
        eval.heap()
            .alloc_complex(PlatformInfo::from_configuration(conf, eval.heap())?),
    )];
    let impl_ = match transition {
        TransitionData::MagicObject(v) => {
            args.push(("refs", refs));
            v.implementation.to_value()
        }
        TransitionData::Target(v) => v.r#impl.to_value().get(),
    };
    if let Some(attrs) = attrs {
        args.push(("attrs", attrs));
    }
    let new_platforms = eval
        .eval_function(impl_, &[], &args)
        .map_err(kuro_error::Error::from)?;
    if transition.is_split() {
        match UnpackDictEntries::<&str, &PlatformInfo>::unpack_value(new_platforms)? {
            Some(dict) => {
                let mut split = OrderedMap::new();
                for (k, v) in dict.entries {
                    let prev = split.insert(k.to_owned(), v.to_configuration()?);
                    assert!(prev.is_none());
                }
                Ok(TransitionApplied::Split(SortedMap::from(split)))
            }
            None => Err(kuro_error::Error::from(
                ApplyTransitionError::SplitTransitionMustReturnDict,
            )
            .into()),
        }
    } else {
        match <&PlatformInfo>::unpack_value_err(new_platforms) {
            Ok(platform) => Ok(TransitionApplied::Single(platform.to_configuration()?)),
            Err(_) => Err(kuro_error::Error::from(
                ApplyTransitionError::NonSplitTransitionMustReturnPlatformInfo,
            )
            .into()),
        }
    }
}

/// Call a Bazel-style transition function.
///
/// Bazel transitions have the signature `def impl(settings, attr)` where:
/// - `settings` is a dict of {input_setting_label: current_value}
/// - `attr` is a struct of attribute values (or None if no attrs declared)
///
/// Returns a dict of {output_setting_label: new_value}, or {} for no-op.
fn call_bazel_transition_function<'v>(
    transition: &TransitionData,
    conf: &ConfigurationData,
    attrs: Option<Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<TransitionApplied> {
    let impl_ = match transition {
        TransitionData::MagicObject(v) => v.implementation.to_value(),
        TransitionData::Target(v) => v.r#impl.to_value().get(),
    };

    // Build the settings dict from the transition's inputs list.
    // Each input is a label like "//command_line_option:cpu" or "//pkg:flag".
    let inputs = transition.inputs();
    let mut settings_entries: Vec<(&str, Value<'v>)> = Vec::new();
    for input in inputs {
        let value = resolve_setting_value(input, conf, eval)?;
        // We need to alloc the key string on the heap for the dict
        settings_entries.push((eval.heap().alloc_str(input).as_str(), value));
    }
    let settings_dict = eval.heap().alloc(starlark::values::dict::AllocDict(settings_entries.into_iter()));

    // Build the attr struct (or None if no attrs declared)
    let attr_value = attrs.unwrap_or_else(|| eval.heap().alloc(AllocStruct(Vec::<(&str, Value)>::new())));

    // Call the transition implementation: impl(settings, attr)
    let result = eval
        .eval_function(impl_, &[settings_dict, attr_value], &[])
        .map_err(kuro_error::Error::from)?;

    // Parse the return value.
    // Bazel transitions return a dict of {setting_label: new_value}
    // or {} / None for no changes.
    if result.is_none() {
        // No-op transition - return current configuration unchanged
        return Ok(TransitionApplied::Single(conf.dupe()));
    }

    // Check if it's a dict
    if let Some(dict) = DictRef::from_value(result) {
        if dict.is_empty() {
            // Empty dict = no-op
            return Ok(TransitionApplied::Single(conf.dupe()));
        }

        // For split transitions, the return is a dict of {split_key: {setting: value}}
        if transition.is_split() {
            let mut split = OrderedMap::new();
            for (k, _v) in dict.iter() {
                let key = k.unpack_str().unwrap_or_default();
                // For split Bazel transitions, each value should be a dict of settings.
                // For now, pass through the current configuration since we don't yet
                // apply setting changes to the configuration.
                split.insert(key.to_owned(), conf.dupe());
            }
            return Ok(TransitionApplied::Split(SortedMap::from(split)));
        }

        // Non-split: the returned dict maps setting labels to new values.
        // Store the setting changes in the build config for later resolution.
        let outputs = transition.outputs();
        for (k, v) in dict.iter() {
            let key = k.unpack_str().unwrap_or_default();
            // Validate that returned keys are in outputs
            if !outputs.is_empty() && !outputs.iter().any(|o| o.as_str() == key) {
                // Setting not declared in outputs - log but don't error
                eprintln!(
                    "Warning: transition returned setting '{}' not declared in outputs",
                    key
                );
            }
            // Apply the setting value to the build config
            let value_str = if v.is_none() {
                continue; // None means no change
            } else if let Some(b) = v.unpack_bool() {
                if b { "True" } else { "False" }.to_owned()
            } else {
                v.unpack_str().map(|s| s.to_owned()).unwrap_or_else(|| format!("{}", v))
            };
            // Store in the global build config so ctx.build_setting_value picks it up
            kuro_build_api::interpreter::rule_defs::build_config::set_starlark_flag(
                key, &value_str,
            );
        }

        // Return the current configuration (settings are applied via BuildConfig)
        return Ok(TransitionApplied::Single(conf.dupe()));
    }

    // Try treating as PlatformInfo (compatibility with mixed-style transitions)
    match <&PlatformInfo>::unpack_value_err(result) {
        Ok(platform) => Ok(TransitionApplied::Single(platform.to_configuration()?)),
        Err(_) => {
            // If it's not a dict and not PlatformInfo, return current config as no-op
            // Unexpected return type - treat as no-op
            Ok(TransitionApplied::Single(conf.dupe()))
        }
    }
}

/// Resolve the current value of a build setting for transition inputs.
fn resolve_setting_value<'v>(
    setting_label: &str,
    _conf: &ConfigurationData,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<Value<'v>> {
    // Check for command_line_option settings
    if setting_label.starts_with("//command_line_option:") {
        let option_name = &setting_label["//command_line_option:".len()..];
        let value = match option_name {
            "compilation_mode" => kuro_build_api::interpreter::rule_defs::build_config::get_compilation_mode(),
            "cpu" => {
                if cfg!(target_arch = "x86_64") {
                    "k8".to_owned()
                } else if cfg!(target_arch = "aarch64") {
                    "aarch64".to_owned()
                } else {
                    "unknown".to_owned()
                }
            }
            "crosstool_top" => "".to_owned(),
            "compiler" => "".to_owned(),
            "platforms" => "".to_owned(),
            "host_platform" => "".to_owned(),
            _ => {
                // Check if there's a Starlark flag override
                kuro_build_api::interpreter::rule_defs::build_config::get_starlark_flag(
                    &format!("//command_line_option:{}", option_name),
                )
                .unwrap_or_default()
            }
        };
        return Ok(eval.heap().alloc_str(&value).to_value());
    }

    // Check for user-defined build settings (Starlark flags)
    if let Some(value) = kuro_build_api::interpreter::rule_defs::build_config::get_starlark_flag(setting_label) {
        // Parse the value appropriately
        return Ok(match value.as_str() {
            "True" | "true" => eval.heap().alloc(true),
            "False" | "false" => eval.heap().alloc(false),
            s => eval.heap().alloc_str(s).to_value(),
        });
    }

    // Default: return empty string for unknown settings
    Ok(eval.heap().alloc_str("").to_value())
}

async fn do_apply_transition(
    ctx: &mut DiceComputations<'_>,
    attrs: Option<&[Option<Arc<ConfiguredAttr>>]>,
    conf: &ConfigurationData,
    transition_id: &TransitionId,
    cancellation: &CancellationContext,
) -> kuro_error::Result<TransitionApplied> {
    // For unbound/unspecified platforms, transitions are no-ops.
    // Bazel always has a bound platform, but in Kuro the platform may be
    // unspecified when no default_target_platform is configured.
    if !conf.is_bound() {
        return Ok(TransitionApplied::Single(conf.dupe()));
    }

    let transition = ctx.fetch_transition(transition_id).await?;
    let mut refs = Vec::new();
    let mut refs_refs = Vec::new();
    for (s, t) in transition.refs() {
        let provider_collection_value = ctx.fetch_transition_function_reference(&t).await?;
        refs.push((
            *s,
            // This is safe because we store a reference to provider collection in `refs_refs`.
            unsafe { provider_collection_value.value().to_frozen_value() },
        ));
        refs_refs.push(provider_collection_value);
    }
    let print = EventDispatcherPrintHandler(get_dispatcher());
    let eval_kind = StarlarkEvalKind::Transition(Arc::new(transition_id.clone()));
    let provider = StarlarkEvaluatorProvider::new(ctx, eval_kind).await?;
    BuckStarlarkModule::with_profiling(|module| {
        let (finished_eval, res) = provider
            .with_evaluator(&module, cancellation.into(), |eval, _| {
                eval.set_print_handler(&print);
                eval.set_soft_error_handler(&KuroStarlarkSoftErrorHandler);
                let refs = module.heap().alloc(AllocStruct(refs));
                let attrs = match (transition.attr_names(), attrs) {
                    (Some(names), Some(values)) => {
                        let mut attrs = Vec::new();
                        for (name, value) in names.into_iter().zip_eq(values.iter()) {
                            let value = match value {
                                Some(value) => (CONFIGURED_ATTR_TO_VALUE.get()?)(
                                    &value,
                                    PackageLabelOption::TransitionAttr,
                                    module.heap(),
                                )
                                .with_buck_error_context(|| {
                                    format!(
                                        "Error converting attribute `{}={}` to Starlark value",
                                        name,
                                        value.as_display_no_ctx(),
                                    )
                                })?,
                                None => Value::new_none(),
                            };
                            attrs.push((name, value));
                        }
                        Some(module.heap().alloc(AllocStruct(attrs)))
                    }
                    (None, None) => None,
                    (Some(_), None) | (None, Some(_)) => {
                        return Err(
                            ApplyTransitionError::InconsistentTransitionAndComputation.into()
                        );
                    }
                };
                match call_transition_function(&transition, conf, refs, attrs, eval)? {
                    TransitionApplied::Single(new) => {
                        let new_2 =
                            match call_transition_function(&transition, &new, refs, attrs, eval)
                                .buck_error_context(
                                    "applying transition again on transition output",
                                )? {
                                TransitionApplied::Single(new_2) => new_2,
                                TransitionApplied::Split(_) => {
                                    unreachable!(
                                        "split transition filtered out in call_transition_function"
                                    )
                                }
                            };
                        if let Err(diff) = cfg_diff(&new, &new_2) {
                            return Err(
                                ApplyTransitionError::SplitTransitionAgainDifferentPlatformInfo(
                                    diff,
                                )
                                .into(),
                            );
                        }
                        Ok(TransitionApplied::Single(new))
                    }
                    TransitionApplied::Split(split) => {
                        // Not validating split transitions yet, because it's not 100% clear what to validate,
                        // and because it is not that important, because split transitions
                        // are not used in per-rule transitions.
                        Ok(TransitionApplied::Split(split))
                    }
                }
            })
            .map_err(kuro_error::Error::from)?;
        let (token, _) = finished_eval.finish(None)?;
        Ok((token, res))
    })
}

#[async_trait]
pub(crate) trait ApplyTransition {
    /// Resolve `refs` param of transition function.
    async fn fetch_transition_function_reference(
        &mut self,
        target: &ProvidersLabel,
    ) -> kuro_error::Result<FrozenProviderCollectionValue>;
}

#[async_trait]
impl ApplyTransition for DiceComputations<'_> {
    async fn fetch_transition_function_reference(
        &mut self,
        target: &ProvidersLabel,
    ) -> kuro_error::Result<FrozenProviderCollectionValue> {
        Ok(self.get_configuration_analysis_result(target).await?.dupe())
    }
}

struct TransitionCalculationImpl;

pub(crate) fn init_transition_calculation() {
    TRANSITION_CALCULATION.init(&TransitionCalculationImpl);
}

#[async_trait]
impl TransitionCalculation for TransitionCalculationImpl {
    async fn apply_transition(
        &self,
        ctx: &mut DiceComputations<'_>,
        configured_attrs: &OrderedMap<&str, Arc<ConfiguredAttr>>,
        cfg: &ConfigurationData,
        transition_id: &TransitionId,
    ) -> kuro_error::Result<Arc<TransitionApplied>> {
        #[derive(Debug, Eq, PartialEq, Hash, Clone, Display, Allocative)]
        #[display("{} ({}){}", transition_id, cfg, self.fmt_attrs())]
        struct TransitionKey {
            cfg: ConfigurationData,
            transition_id: TransitionId,
            /// Attributes which requested by transition function, not all attributes.
            /// The attr value index is the index of attribute in transition object.
            /// Attributes are added here so multiple targets with the equal attributes
            /// (e.g. the same `java_version = 14`) share the transition computation.
            attrs: Option<Vec<Option<Arc<ConfiguredAttr>>>>,
        }

        impl TransitionKey {
            fn fmt_attrs(&self) -> String {
                if let Some(attrs) = &self.attrs {
                    format!(
                        " [{}]",
                        attrs
                            .iter()
                            .map(|a| {
                                if let Some(attr) = a {
                                    attr.as_display_no_ctx().to_string()
                                } else {
                                    "None".to_owned()
                                }
                            })
                            .join(", ")
                    )
                } else {
                    String::new()
                }
            }
        }

        #[async_trait]
        impl Key for TransitionKey {
            type Value = kuro_error::Result<Arc<TransitionApplied>>;

            async fn compute(
                &self,
                ctx: &mut DiceComputations,
                cancellation: &CancellationContext,
            ) -> Self::Value {
                let v: kuro_error::Result<_> = try {
                    do_apply_transition(
                        ctx,
                        self.attrs.as_deref(),
                        &self.cfg,
                        &self.transition_id,
                        cancellation,
                    )
                    .await?
                };

                Ok(Arc::new(v.with_buck_error_context(|| {
                    format!("Error computing transition `{__self}`")
                })?))
            }

            fn equality(x: &Self::Value, y: &Self::Value) -> bool {
                if let (Ok(x), Ok(y)) = (x, y) {
                    x == y
                } else {
                    false
                }
            }
        }

        let transition = ctx.fetch_transition(transition_id).await?;

        #[allow(clippy::manual_map)]
        let attrs = if let Some(attrs) = transition.attr_names() {
            Some(
                attrs
                    .into_iter()
                    .map(|attr| configured_attrs.get(attr).duped())
                    .collect(),
            )
        } else {
            None
        };

        let key = TransitionKey {
            cfg: cfg.dupe(),
            transition_id: transition_id.clone(),
            attrs,
        };

        ctx.compute(&key).await?.map_err(kuro_error::Error::from)
    }
}
