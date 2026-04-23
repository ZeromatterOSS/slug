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
use kuro_core::configuration::build_setting::BuildSettingLabel;
use kuro_core::configuration::build_setting::BuildSettingValue;
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
/// - `settings` is a dict of {input_setting_label: current_value} sourced from
///   the incoming `ConfigurationData.build_settings`, with the top-level
///   global `BUILD_CONFIG` as a fallback for flags the CLI has not yet
///   plumbed into the cfg (pre-Plan-19.4 compat).
/// - `attr` is a struct of attribute values (or `None` if no attrs declared).
///
/// Returns either a dict of {output_setting_label: new_value} (which is
/// folded into a new `ConfigurationData` via
/// `ConfigurationData::with_build_setting`), `None` / `{}` for a no-op, or
/// a `PlatformInfo` for legacy mixed-style transitions.
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

    // Build the settings dict from the transition's inputs list, reading
    // each input from conf.build_settings (falling back to global config
    // when absent — 19.4 will remove that fallback once the CLI populates
    // the cfg directly).
    let inputs = transition.inputs();
    let mut settings_entries: Vec<(&str, Value<'v>)> = Vec::new();
    for input in inputs {
        let value = resolve_setting_value(input, conf, eval)?;
        settings_entries.push((eval.heap().alloc_str(input).as_str(), value));
    }
    let settings_dict = eval.heap().alloc(starlark::values::dict::AllocDict(
        settings_entries.into_iter(),
    ));

    // Build the attr struct.
    let attr_value =
        attrs.unwrap_or_else(|| eval.heap().alloc(AllocStruct(Vec::<(&str, Value)>::new())));

    // Call the transition implementation: impl(settings, attr)
    let result = eval
        .eval_function(impl_, &[settings_dict, attr_value], &[])
        .map_err(kuro_error::Error::from)?;

    if result.is_none() {
        return Ok(TransitionApplied::Single(conf.dupe()));
    }

    if let Some(dict) = DictRef::from_value(result) {
        if dict.is_empty() {
            return Ok(TransitionApplied::Single(conf.dupe()));
        }

        if transition.is_split() {
            // Split transitions return {split_key: {setting_label: value}}.
            // Each inner dict is applied to the incoming cfg independently.
            let mut split = OrderedMap::new();
            for (k, v) in dict.iter() {
                let split_key = k.unpack_str().unwrap_or_default().to_owned();
                let inner = DictRef::from_value(v).ok_or_else(|| {
                    kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "split transition branch `{}` did not return a dict",
                        split_key
                    )
                })?;
                let branch_cfg = apply_setting_dict_to_cfg(conf, &inner, transition.outputs())?;
                split.insert(split_key, branch_cfg);
            }
            return Ok(TransitionApplied::Split(SortedMap::from(split)));
        }

        let new_cfg = apply_setting_dict_to_cfg(conf, &dict, transition.outputs())?;
        return Ok(TransitionApplied::Single(new_cfg));
    }

    // Legacy mixed-style transitions may return a PlatformInfo directly.
    match <&PlatformInfo>::unpack_value_err(result) {
        Ok(platform) => Ok(TransitionApplied::Single(platform.to_configuration()?)),
        Err(_) => Ok(TransitionApplied::Single(conf.dupe())),
    }
}

/// Folds a transition's returned `{label: value}` dict into a new
/// `ConfigurationData`. Downstream analyses run under the returned cfg
/// and read settings directly via `ConfigurationData.build_settings`, so
/// no global side effect is needed.
fn apply_setting_dict_to_cfg(
    conf: &ConfigurationData,
    dict: &DictRef<'_>,
    declared_outputs: &[String],
) -> kuro_error::Result<ConfigurationData> {
    let mut out = conf.dupe();
    for (k, v) in dict.iter() {
        let key_str = match k.unpack_str() {
            Some(s) => s,
            None => continue,
        };
        if v.is_none() {
            continue;
        }
        if !declared_outputs.is_empty() && !declared_outputs.iter().any(|o| o == key_str) {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "transition returned setting `{}` not declared in outputs={:?}",
                key_str,
                declared_outputs
            ));
        }
        let label = BuildSettingLabel::from_bazel_label(key_str)?;
        let value = build_setting_value_from_starlark(v)?;
        out = out.with_build_setting(label, value)?;
    }
    Ok(out)
}

/// Converts a Starlark value returned by a transition into a typed
/// `BuildSettingValue`. String lists/sets are inferred by element types
/// rather than pre-declared; this keeps parity with Bazel's runtime
/// coercion while a proper typecheck against the output rule's
/// `build_setting_type` is added in a follow-up phase.
fn build_setting_value_from_starlark(v: Value<'_>) -> kuro_error::Result<BuildSettingValue> {
    if let Some(b) = v.unpack_bool() {
        return Ok(BuildSettingValue::Bool(b));
    }
    if let Some(i) = v.unpack_i32() {
        return Ok(BuildSettingValue::Int(i64::from(i)));
    }
    if let Some(s) = v.unpack_str() {
        return Ok(BuildSettingValue::String(s.to_owned()));
    }
    if let Some(list) = starlark::values::list::ListRef::from_value(v) {
        let items: Vec<String> = list
            .iter()
            .map(|e| e.unpack_str().map(|s| s.to_owned()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "transition list value must contain only strings"
                )
            })?;
        return Ok(BuildSettingValue::StringList(items));
    }
    Err(kuro_error::kuro_error!(
        kuro_error::ErrorTag::Input,
        "transition value type is not supported as a build setting: `{}`",
        v.get_type()
    ))
}

/// Resolve the current value of a build setting for the transition-input dict.
///
/// Priority order:
/// 1. `conf.build_settings` — the per-configuration value written by an earlier
///    transition or CLI flag.
/// 2. Known `//command_line_option:*` pseudo-labels backed by the global
///    `BUILD_CONFIG` (kept until Plan 19.4 routes CLI flags through the cfg).
/// 3. Other Starlark flag globals.
/// 4. Empty string fallback.
fn resolve_setting_value<'v>(
    setting_label: &str,
    conf: &ConfigurationData,
    eval: &mut Evaluator<'v, '_, '_>,
) -> kuro_error::Result<Value<'v>> {
    if let Ok(label) = BuildSettingLabel::from_bazel_label(setting_label) {
        if let Ok(Some(value)) = conf.get_build_setting(&label) {
            return Ok(build_setting_value_to_starlark(value, eval));
        }
    }

    if let Some(option_name) = setting_label.strip_prefix("//command_line_option:") {
        let value = match option_name {
            "compilation_mode" => {
                kuro_build_api::interpreter::rule_defs::build_config::get_compilation_mode()
            }
            "cpu" => {
                if cfg!(target_arch = "x86_64") {
                    "k8".to_owned()
                } else if cfg!(target_arch = "aarch64") {
                    "aarch64".to_owned()
                } else {
                    "unknown".to_owned()
                }
            }
            "crosstool_top" | "compiler" | "platforms" | "host_platform" => String::new(),
            _ => kuro_build_api::interpreter::rule_defs::build_config::get_starlark_flag(&format!(
                "//command_line_option:{option_name}"
            ))
            .unwrap_or_default(),
        };
        return Ok(eval.heap().alloc_str(&value).to_value());
    }

    if let Some(value) =
        kuro_build_api::interpreter::rule_defs::build_config::get_starlark_flag(setting_label)
    {
        return Ok(match value.as_str() {
            "True" | "true" => eval.heap().alloc(true),
            "False" | "false" => eval.heap().alloc(false),
            s => eval.heap().alloc_str(s).to_value(),
        });
    }

    Ok(eval.heap().alloc_str("").to_value())
}

fn build_setting_value_to_starlark<'v>(
    value: &BuildSettingValue,
    eval: &mut Evaluator<'v, '_, '_>,
) -> Value<'v> {
    match value {
        BuildSettingValue::Bool(b) => eval.heap().alloc(*b),
        BuildSettingValue::Int(i) => eval.heap().alloc(*i),
        BuildSettingValue::String(s) => eval.heap().alloc_str(s).to_value(),
        BuildSettingValue::StringList(xs) | BuildSettingValue::StringSet(xs) => eval
            .heap()
            .alloc(starlark::values::list::AllocList(xs.iter().cloned())),
    }
}

async fn do_apply_transition(
    ctx: &mut DiceComputations<'_>,
    attrs: Option<&[Option<Arc<ConfiguredAttr>>]>,
    conf: &ConfigurationData,
    transition_id: &TransitionId,
    cancellation: &CancellationContext,
    bazel_all_attrs: Option<&[(String, Arc<ConfiguredAttr>)]>,
) -> kuro_error::Result<TransitionApplied> {
    // For unbound/unspecified platforms, transitions are no-ops.
    // Bazel always has a bound platform, but in Kuro the platform may be
    // unspecified when no default_target_platform is configured.
    if !conf.is_bound() {
        return Ok(TransitionApplied::Single(conf.dupe()));
    }

    // Anonymous `rule(cfg = dict(implementation=..., inputs=[...], outputs=[...]))`
    // transitions (used by rules_python's py_binary builder) are stored with a
    // magic id `(<defining_bzl>, "_anonymous_transition")`. The transition
    // object itself is never bound to a module-level global, so the
    // fetch-by-name lookup always fails. Kuro does not yet execute Starlark
    // transitions, so treat anonymous cfg= transitions as identity (matches
    // the behaviour we already apply to `config.target()` and bazel-style
    // no-op transitions in `rule.rs::call`).
    if let TransitionId::MagicObject { name, .. } = transition_id {
        if name == "_anonymous_transition" {
            return Ok(TransitionApplied::Single(conf.dupe()));
        }
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
                let attrs = if let Some(bazel_attrs) = bazel_all_attrs {
                    // Bazel-style: build attrs struct from all rule attributes
                    let mut attr_pairs: Vec<(&str, Value)> = Vec::new();
                    for (name, value) in bazel_attrs {
                        let v = match (CONFIGURED_ATTR_TO_VALUE.get()?)(
                            value,
                            PackageLabelOption::TransitionAttr,
                            module.heap(),
                        ) {
                            Ok(v) => v,
                            Err(_) => Value::new_none(),
                        };
                        attr_pairs.push((name.as_str(), v));
                    }
                    Some(module.heap().alloc(AllocStruct(attr_pairs)))
                } else {
                    match (transition.attr_names(), attrs) {
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
        // Anonymous `rule(cfg = dict(...))` transitions: the transition
        // object is never bound to a module-level global, so the later
        // `ctx.fetch_transition(transition_id)` call inside this function
        // fails. Kuro does not execute Starlark transitions, so treat the
        // anonymous form as identity and short-circuit the whole path.
        // See also `do_apply_transition` below for the same guard applied
        // through the DICE key route.
        if let TransitionId::MagicObject { name, .. } = transition_id {
            if name == "_anonymous_transition" {
                return Ok(Arc::new(TransitionApplied::Single(cfg.dupe())));
            }
        }
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
            /// For Bazel-style transitions: all rule attributes as (name, value) pairs.
            /// Bazel transitions can access any rule attribute via `attr.xxx`.
            bazel_all_attrs: Option<Vec<(String, Arc<ConfiguredAttr>)>>,
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
                        self.bazel_all_attrs.as_deref(),
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

        // For Bazel-style transitions, store all rule attrs as named pairs
        // so the transition can access them via `attr.xxx`.
        let bazel_all_attrs: Option<Vec<(String, Arc<ConfiguredAttr>)>> =
            if transition.is_bazel_style() {
                Some(
                    configured_attrs
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.dupe()))
                        .collect(),
                )
            } else {
                None
            };

        let key = TransitionKey {
            cfg: cfg.dupe(),
            transition_id: transition_id.clone(),
            attrs,
            bazel_all_attrs,
        };

        ctx.compute(&key).await?.map_err(kuro_error::Error::from)
    }
}

#[cfg(test)]
mod tests {
    use kuro_core::configuration::build_setting::BuildSettingLabel;
    use kuro_core::configuration::build_setting::BuildSettingValue;
    use kuro_core::configuration::data::ConfigurationData;
    use kuro_core::configuration::data::ConfigurationDataData;

    #[test]
    fn bazel_label_parses_unprefixed() {
        let l = BuildSettingLabel::from_bazel_label("//:my_flag").unwrap();
        // Synthetic cell form is stable and round-trips.
        let l2 = BuildSettingLabel::from_bazel_label("//:my_flag").unwrap();
        assert_eq!(l, l2);
    }

    #[test]
    fn bazel_label_parses_command_line_option() {
        let a =
            BuildSettingLabel::from_bazel_label("//command_line_option:compilation_mode").unwrap();
        let b = BuildSettingLabel::from_bazel_label("//command_line_option:cpu").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn bazel_label_parses_cell_prefixed() {
        let l = BuildSettingLabel::from_bazel_label("@bazel_tools//tools/cpp:compilation_mode")
            .unwrap();
        let l2 =
            BuildSettingLabel::from_bazel_label("//command_line_option:compilation_mode").unwrap();
        // A prefixed label and a pseudo-label both resolve to valid, distinct keys.
        assert_ne!(l, l2);
    }

    #[test]
    fn bazel_label_rejects_bare() {
        assert!(BuildSettingLabel::from_bazel_label("my_flag").is_err());
    }

    /// `with_build_setting` on a cfg with no prior settings adds the entry and
    /// the resulting cfg has a distinct output hash. Exercises the plumbing
    /// that `apply_setting_dict_to_cfg` relies on without requiring a
    /// Starlark evaluator.
    #[test]
    fn apply_setting_changes_cfg_identity() -> kuro_error::Result<()> {
        let base = ConfigurationData::from_platform(
            "cfg_for//:testing".to_owned(),
            ConfigurationDataData::empty(),
        )?;
        let label = BuildSettingLabel::from_bazel_label("//:my_flag")?;
        let updated =
            base.with_build_setting(label.clone(), BuildSettingValue::String("baz".to_owned()))?;
        assert_ne!(base.output_hash(), updated.output_hash());
        assert_eq!(
            updated.get_build_setting(&label)?,
            Some(&BuildSettingValue::String("baz".to_owned()))
        );
        assert_eq!(base.get_build_setting(&label)?, None);
        Ok(())
    }
}
