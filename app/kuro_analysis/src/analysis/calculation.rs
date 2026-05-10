/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use dupe::IterDupedExt;
use futures::FutureExt;
use kuro_build_api::analysis::AnalysisResult;
use kuro_build_api::analysis::calculation::EVAL_ANALYSIS_QUERY;
use kuro_build_api::analysis::calculation::RULE_ANALYSIS_CALCULATION;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculation;
use kuro_build_api::analysis::calculation::RuleAnalysisCalculationImpl;
use kuro_build_api::build::detailed_aggregated_metrics::dice::HasDetailedAggregatedMetrics;
use kuro_build_api::deferred::calculation::DeferredHolder;
use kuro_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use kuro_build_api::keep_going::KeepGoing;
use kuro_build_signals::env::WaitingData;
use kuro_common::dice::cells::HasCellResolver;
use kuro_common::legacy_configs::dice::HasLegacyConfigs;
use kuro_common::legacy_configs::key::BuckconfigKeyRef;
use kuro_core::configuration::build_setting::BuildSettingLabel;
use kuro_core::configuration::build_setting::BuildSettingValue;
use kuro_core::configuration::compatibility::MaybeCompatible;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
use kuro_core::target::label::label::TargetLabel;
use kuro_data::ToProtoMessage;
use kuro_data::error::ErrorTag;
use kuro_error::BuckErrorContext;
use kuro_error::internal_error;
use kuro_events::dispatch::async_record_root_spans;
use kuro_events::dispatch::record_root_spans;
use kuro_events::dispatch::span_async;
use kuro_events::dispatch::span_async_simple;
use kuro_events::span::SpanId;
use kuro_interpreter::dice::starlark_provider::StarlarkEvalKind;
use kuro_interpreter::file_loader::LoadedModule;
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::paths::module::StarlarkModulePath;
use kuro_interpreter::starlark_profiler::config::GetStarlarkProfilerInstrumentation;
use kuro_interpreter::starlark_profiler::data::StarlarkProfileDataAndStats;
use kuro_interpreter::starlark_profiler::mode::StarlarkProfileMode;
use kuro_node::attrs::attr_type::query::ResolvedQueryLiterals;
use kuro_node::attrs::coerced_attr::CoercedAttr;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
use kuro_node::nodes::configured::ConfiguredTargetNode;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use kuro_node::nodes::frontend::TargetGraphCalculation;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use kuro_node::rule_type::StarlarkRuleType;
use kuro_query::query::syntax::simple::eval::label_indexed::LabelIndexedSet;
use kuro_query::query::syntax::simple::eval::set::TargetSet;
use kuro_util::time_span::TimeSpan;
use smallvec::SmallVec;

use crate::analysis::aspect_key::AspectKey;
use crate::analysis::env::RuleSpec;
use crate::analysis::env::get_user_defined_rule_spec;
use crate::analysis::env::run_analysis;
use crate::attrs::resolve::ctx::AnalysisQueryResult;

struct RuleAnalysisCalculationInstance;

static ANALYSIS_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static ANALYSIS_MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static ANALYSIS_COMPLETED: AtomicUsize = AtomicUsize::new(0);

const ANALYSIS_DEP_BATCH_SIZE: usize = 128;

struct AnalysisActiveGuard;

impl AnalysisActiveGuard {
    fn new() -> (Self, usize, usize) {
        let active = ANALYSIS_ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
        let max_active = update_max_active(&ANALYSIS_MAX_ACTIVE, active);
        (Self, active, max_active)
    }
}

impl Drop for AnalysisActiveGuard {
    fn drop(&mut self) {
        ANALYSIS_ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

fn update_max_active(max: &AtomicUsize, active: usize) -> usize {
    let mut old = max.load(Ordering::Relaxed);
    while active > old {
        match max.compare_exchange_weak(old, active, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(_) => return active,
            Err(next) => old = next,
        }
    }
    old
}

fn analysis_checkpoint(
    checkpoint: &'static str,
    target: &ConfiguredTargetLabel,
    fields: impl IntoIterator<Item = (&'static str, usize)> + Clone,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    kuro_util::memory_checkpoint::checkpoint(checkpoint, fields.clone());
    tracing::warn!(
        target: "kuro_memory",
        checkpoint,
        target_label = %target,
        "analysis checkpoint {checkpoint} target={target}"
    );
}

fn analysis_result_checkpoint(
    checkpoint: &'static str,
    target: &ConfiguredTargetLabel,
    result: &AnalysisResult,
    active: usize,
    completed: usize,
    max_active: usize,
) {
    let counts = result.counts();
    let retained_bytes = result.retained_memory().unwrap_or(0);
    let provider_count = result.provider_count().unwrap_or(0);
    let profile_retained_bytes = result
        .profile_data
        .as_ref()
        .map(|p| p.total_retained_bytes())
        .unwrap_or(0);
    analysis_checkpoint(
        checkpoint,
        target,
        [
            ("active", active),
            ("completed", completed),
            ("max_active", max_active),
            ("retained_bytes", retained_bytes),
            ("profile_retained_bytes", profile_retained_bytes),
            ("providers", provider_count),
            ("actions", counts.actions),
            ("action_data", counts.action_data),
            ("transitive_sets", counts.transitive_sets),
            (
                "has_provider_collection",
                counts.has_provider_collection as usize,
            ),
            ("declared_actions", result.num_declared_actions as usize),
            ("declared_artifacts", result.num_declared_artifacts as usize),
        ],
    );
}

fn dep_analysis_checkpoint(
    checkpoint: &'static str,
    target: &ConfiguredTargetLabel,
    dep_analysis: &[(&ConfiguredTargetLabel, AnalysisResult)],
    query_count: usize,
) {
    let mut retained_bytes = 0usize;
    let mut profile_retained_bytes = 0usize;
    let mut providers = 0usize;
    let mut actions = 0usize;
    let mut action_data = 0usize;
    let mut transitive_sets = 0usize;
    let mut declared_actions = 0usize;
    let mut declared_artifacts = 0usize;
    for (_, result) in dep_analysis {
        retained_bytes = retained_bytes.saturating_add(result.retained_memory().unwrap_or(0));
        profile_retained_bytes = profile_retained_bytes.saturating_add(
            result
                .profile_data
                .as_ref()
                .map(|p| p.total_retained_bytes())
                .unwrap_or(0),
        );
        providers = providers.saturating_add(result.provider_count().unwrap_or(0));
        let counts = result.counts();
        actions = actions.saturating_add(counts.actions);
        action_data = action_data.saturating_add(counts.action_data);
        transitive_sets = transitive_sets.saturating_add(counts.transitive_sets);
        declared_actions = declared_actions.saturating_add(result.num_declared_actions as usize);
        declared_artifacts =
            declared_artifacts.saturating_add(result.num_declared_artifacts as usize);
    }
    analysis_checkpoint(
        checkpoint,
        target,
        [
            ("active", ANALYSIS_ACTIVE.load(Ordering::Relaxed)),
            ("completed", ANALYSIS_COMPLETED.load(Ordering::Relaxed)),
            ("max_active", ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed)),
            ("deps", dep_analysis.len()),
            ("queries", query_count),
            ("dep_retained_bytes", retained_bytes),
            ("dep_profile_retained_bytes", profile_retained_bytes),
            ("dep_providers", providers),
            ("dep_actions", actions),
            ("dep_action_data", action_data),
            ("dep_transitive_sets", transitive_sets),
            ("dep_declared_actions", declared_actions),
            ("dep_declared_artifacts", declared_artifacts),
        ],
    );
}

#[derive(
    Clone,
    Dupe,
    derive_more::Display,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Allocative
)]
#[display("{}", _0)]
pub struct AnalysisKey(pub ConfiguredTargetLabel);

static ACTIVE_ANALYSIS_KEYS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

struct AnalysisKeyActiveSetGuard {
    key: String,
}

impl AnalysisKeyActiveSetGuard {
    fn new(target: &ConfiguredTargetLabel) -> Self {
        let key = target.to_string();
        ACTIVE_ANALYSIS_KEYS
            .lock()
            .expect("ACTIVE_ANALYSIS_KEYS poisoned")
            .insert(key.clone());
        Self { key }
    }
}

impl Drop for AnalysisKeyActiveSetGuard {
    fn drop(&mut self) {
        ACTIVE_ANALYSIS_KEYS
            .lock()
            .expect("ACTIVE_ANALYSIS_KEYS poisoned")
            .remove(&self.key);
    }
}

fn active_analysis_key_count() -> usize {
    ACTIVE_ANALYSIS_KEYS
        .lock()
        .expect("ACTIVE_ANALYSIS_KEYS poisoned")
        .len()
}

fn analysis_dep_checkpoint(
    checkpoint: &'static str,
    target: &ConfiguredTargetLabel,
    dep_label: Option<&ConfiguredTargetLabel>,
    batch_index: usize,
    dep_index: usize,
    total_deps: usize,
    started: Instant,
) {
    if !kuro_util::memory_checkpoint::enabled() {
        return;
    }
    let elapsed_ms = started.elapsed().as_millis().min(usize::MAX as u128) as usize;
    kuro_util::memory_checkpoint::checkpoint(
        checkpoint,
        [
            ("active", ANALYSIS_ACTIVE.load(Ordering::Relaxed)),
            ("active_keys", active_analysis_key_count()),
            ("completed", ANALYSIS_COMPLETED.load(Ordering::Relaxed)),
            ("max_active", ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed)),
            ("batch_index", batch_index),
            ("dep_index", dep_index),
            ("deps", total_deps),
            ("elapsed_ms", elapsed_ms),
            (
                "dep_label_len",
                dep_label.map(|label| label.to_string().len()).unwrap_or(0),
            ),
        ],
    );
    match dep_label {
        Some(dep_label) => tracing::warn!(
            target: "kuro_memory",
            checkpoint,
            target_label = %target,
            dep_label = %dep_label,
            batch_index,
            dep_index,
            total_deps,
            elapsed_ms,
            "analysis dep checkpoint {checkpoint} target={target} dep={dep_label} index={dep_index}/{total_deps} elapsed_ms={elapsed_ms}"
        ),
        None => tracing::warn!(
            target: "kuro_memory",
            checkpoint,
            target_label = %target,
            batch_index,
            dep_index,
            total_deps,
            elapsed_ms,
            "analysis dep checkpoint {checkpoint} target={target} index={dep_index}/{total_deps} elapsed_ms={elapsed_ms}"
        ),
    }
}

pub(crate) fn init_rule_analysis_calculation() {
    RULE_ANALYSIS_CALCULATION.init(&RuleAnalysisCalculationInstance);
}

#[async_trait]
impl Key for AnalysisKey {
    type Value = kuro_error::Result<MaybeCompatible<AnalysisResult>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        let (_active_guard, active, max_active) = AnalysisActiveGuard::new();
        let _active_key_guard = AnalysisKeyActiveSetGuard::new(&self.0);
        let completed = ANALYSIS_COMPLETED.load(Ordering::Relaxed);
        analysis_checkpoint(
            "analysis_key_start",
            &self.0,
            [
                ("active", active),
                ("completed", completed),
                ("max_active", max_active),
            ],
        );
        let deferred_key = DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(self.0.dupe()));
        ctx.analysis_started(&deferred_key)?;
        let res = get_analysis_result(ctx, &self.0, cancellation)
            .await
            .with_buck_error_context(|| format!("Error running analysis for `{}`", &self.0))?;
        if let MaybeCompatible::Compatible(v) = &res {
            ctx.analysis_complete(&deferred_key, &DeferredHolder::Analysis(v.dupe()))?;
            let completed = ANALYSIS_COMPLETED.fetch_add(1, Ordering::SeqCst) + 1;
            analysis_result_checkpoint(
                "analysis_key_complete",
                &self.0,
                v,
                ANALYSIS_ACTIVE.load(Ordering::Relaxed),
                completed,
                ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed),
            );
        } else {
            let completed = ANALYSIS_COMPLETED.fetch_add(1, Ordering::SeqCst) + 1;
            analysis_checkpoint(
                "analysis_key_incompatible",
                &self.0,
                [
                    ("active", ANALYSIS_ACTIVE.load(Ordering::Relaxed)),
                    ("completed", completed),
                    ("max_active", ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed)),
                ],
            );
        }
        Ok(res)
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // analysis result is not comparable
        // TODO consider if we want analysis result to be eq
        false
    }
}

#[async_trait]
impl RuleAnalysisCalculationImpl for RuleAnalysisCalculationInstance {
    async fn get_analysis_result(
        &self,
        ctx: &mut DiceComputations<'_>,
        target: &ConfiguredTargetLabel,
    ) -> kuro_error::Result<MaybeCompatible<AnalysisResult>> {
        ctx.compute(&AnalysisKey(target.dupe())).await?
    }
}

pub async fn resolve_queries(
    ctx: &mut DiceComputations<'_>,
    configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<HashMap<String, Arc<AnalysisQueryResult>>> {
    let mut queries = configured_node.queries().peekable();

    if queries.peek().is_none() {
        return Ok(Default::default());
    }

    span_async_simple(
        kuro_data::AnalysisResolveQueriesStart {
            standard_target: Some(configured_node.label().as_proto().into()),
        },
        resolve_queries_impl(ctx, configured_node, queries),
        kuro_data::AnalysisResolveQueriesEnd {},
    )
    .await
}

async fn resolve_queries_impl(
    ctx: &mut DiceComputations<'_>,
    configured_node: ConfiguredTargetNodeRef<'_>,
    queries: impl IntoIterator<Item = (String, ResolvedQueryLiterals<ConfiguredProvidersLabel>)>,
) -> kuro_error::Result<HashMap<String, Arc<AnalysisQueryResult>>> {
    let deps: TargetSet<_> = configured_node.deps().duped().collect();
    let query_results = ctx
        .try_compute_join(
            queries,
            |ctx,
             (query, resolved_literals_labels): (
                String,
                ResolvedQueryLiterals<ConfiguredProvidersLabel>,
            )| {
                let deps = &deps;
                async move {
                    let mut resolved_literals =
                        HashMap::with_capacity(resolved_literals_labels.0.len());
                    for ((offset, len), label) in resolved_literals_labels.0 {
                        let literal = &query[offset..offset + len];
                        let node = deps.get(label.target()).with_internal_error(|| {
                            format!("Literal `{literal}` not found in `deps`")
                        })?;
                        resolved_literals.insert(literal.to_owned(), node.dupe());
                    }

                    let result =
                        (EVAL_ANALYSIS_QUERY.get()?)(ctx, &query, resolved_literals).await?;

                    // analysis for all the deps in the query result should already have been run since they must
                    // be in our dependency graph, and so we don't worry about parallelizing these lookups.
                    let mut query_results = Vec::new();
                    for node in result.iter() {
                        let label = node.label();
                        query_results.push((
                            label.dupe(),
                            ctx.get_analysis_result(label)
                                .await?
                                .require_compatible()?
                                .providers()?
                                .to_owned(),
                        ))
                    }

                    kuro_error::Ok((
                        query.to_owned(),
                        Arc::new(AnalysisQueryResult {
                            result: query_results,
                        }),
                    ))
                }
                .boxed()
            },
        )
        .await?;

    let query_results: HashMap<_, _> = query_results.into_iter().collect();
    Ok(query_results)
}

pub async fn get_dep_analysis<'v>(
    configured_node: ConfiguredTargetNodeRef<'v>,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<Vec<(&'v ConfiguredTargetLabel, AnalysisResult)>> {
    let started = Instant::now();
    let labels = configured_node
        .deps()
        .map(|dep| dep.label())
        .collect::<Vec<_>>();
    let total_deps = labels.len();
    let mut results = Vec::with_capacity(labels.len());
    analysis_dep_checkpoint(
        "analysis_deps_start",
        configured_node.label(),
        None,
        0,
        0,
        total_deps,
        started,
    );
    for (batch_index, start) in (0..total_deps).step_by(ANALYSIS_DEP_BATCH_SIZE).enumerate() {
        let end = (start + ANALYSIS_DEP_BATCH_SIZE).min(total_deps);
        analysis_dep_checkpoint(
            "analysis_dep_batch_start",
            configured_node.label(),
            labels.get(start).copied(),
            batch_index,
            start,
            total_deps,
            started,
        );
        let batch_results = KeepGoing::try_compute_join_all(ctx, start..end, |ctx, index| {
            let label = labels[index];
            async move {
                analysis_dep_checkpoint(
                    "analysis_dep_request_start",
                    configured_node.label(),
                    Some(label),
                    batch_index,
                    index,
                    total_deps,
                    started,
                );
                let res = ctx
                    .get_analysis_result(label)
                    .await
                    .and_then(|v| v.require_compatible());
                analysis_dep_checkpoint(
                    "analysis_dep_request_complete",
                    configured_node.label(),
                    Some(label),
                    batch_index,
                    index,
                    total_deps,
                    started,
                );
                res.map(|x| (label, x))
            }
            .boxed()
        })
        .await?;
        results.extend(batch_results);
        if labels.len() > ANALYSIS_DEP_BATCH_SIZE {
            analysis_checkpoint(
                "analysis_dep_batch_complete",
                configured_node.label(),
                [
                    ("active", ANALYSIS_ACTIVE.load(Ordering::Relaxed)),
                    ("completed", ANALYSIS_COMPLETED.load(Ordering::Relaxed)),
                    ("max_active", ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed)),
                    ("deps", labels.len()),
                    ("batch_index", batch_index),
                    ("batch_size", end - start),
                    ("results", results.len()),
                ],
            );
        }
    }
    Ok(results)
}

/// Check whether all `flag_values` entries in a `config_setting` target match their
/// `build_setting_default` attribute values. This is used to determine whether the
/// config_setting should match in the absence of CLI flag overrides.
///
/// Returns `true` if all flag values match (or if `flag_values` is empty), `false` otherwise.
/// Returns `false` (conservative: no match) if a flag target can't be found or read.
async fn check_config_setting_flag_values(
    configured_node: ConfiguredTargetNodeRef<'_>,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<bool> {
    let target_node = configured_node.to_owned();
    let target_ref = target_node.target_node().as_ref();

    let flag_values_attr = match target_ref.attr_or_none("flag_values", AttrInspectOptions::All) {
        Some(attr) => attr,
        None => return Ok(true), // No flag_values attr → vacuously matches
    };

    let pairs = match &flag_values_attr.value {
        CoercedAttr::Dict(d) => d.0.clone(),
        _ => return Ok(true), // Not a dict → vacuously matches
    };

    if pairs.is_empty() {
        return Ok(true); // Empty flag_values → vacuously matches
    }

    // Get cell resolver and alias resolver for label parsing
    let cell_resolver = ctx.get_cell_resolver().await?;
    // Use the config_setting's cell for relative label resolution
    let config_setting_cell = target_ref.label().pkg().cell_name();
    let cell_alias_resolver = ctx.get_cell_alias_resolver(config_setting_cell).await?;

    let config_setting_pkg = target_ref.label().pkg();
    for (key, expected_value) in pairs.iter() {
        let raw_label_str = match key {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false), // Unexpected key type → conservative: no match
        };

        let expected_str = match expected_value {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false), // Unexpected value type → conservative: no match
        };

        // Bazel-compatible label resolution in `flag_values`: the dict keys are
        // flag target labels and may appear as:
        //   * `@repo//pkg:name` — fully qualified; parse as absolute.
        //   * `//pkg:name` — main-cell absolute.
        //   * `:name` — relative to the config_setting's own package.
        //   * `name` — bare target name, relative to the config_setting's own
        //     package (common with `native.config_setting(flag_values = {name: ...})`
        //     where `name` is a sibling flag rule).
        // TargetLabel::parse rejects the last form ("Invalid absolute target
        // pattern"); normalize bare names to `//<pkg>:<name>` first.
        let label_str = if raw_label_str.starts_with('@') || raw_label_str.starts_with("//") {
            raw_label_str.clone()
        } else {
            let bare = raw_label_str.trim_start_matches(':');
            let pkg_path = config_setting_pkg.cell_relative_path().as_str();
            if pkg_path.is_empty() {
                format!("//:{bare}")
            } else {
                format!("//{pkg_path}:{bare}")
            }
        };

        // Parse the label string into a TargetLabel.
        let flag_target_label = match TargetLabel::parse(
            &label_str,
            config_setting_cell,
            &cell_resolver,
            &cell_alias_resolver,
        ) {
            Ok(label) => label,
            Err(_) => return Ok(false),
        };

        // Look up the flag target's TargetNode to read its build_setting_default.
        let flag_node = match ctx.get_target_node(&flag_target_label).await {
            Ok(node) => node,
            Err(_) => return Ok(false),
        };

        // Check for CLI override via --//pkg:target=value first
        let flag_pkg = flag_target_label.pkg().cell_relative_path().as_str();
        let flag_name = flag_target_label.name().as_str();
        let flag_label_str = if flag_pkg.is_empty() {
            format!("//:{}", flag_name)
        } else {
            format!("//{}:{}", flag_pkg, flag_name)
        };

        // Handle the flag's current value in two shapes:
        //   * Scalar (string/bool/int): compared against expected_str for equality.
        //   * List (config.string_list / config.int_list): `flag_values` on a
        //     list-typed setting matches when the expected string appears in
        //     the list. Bazel semantics — the canonical example is
        //     `@llvm-project//llvm:driver-tools`, a string_list_flag whose
        //     default is the full tool name list. Every
        //     `driver-tools-include-<tool>` config_setting should match
        //     against the default because each tool is in the list.
        let cfg_value: Option<BuildSettingValue> =
            BuildSettingLabel::from_bazel_label(&flag_label_str)
                .ok()
                .and_then(|l| {
                    configured_node
                        .label()
                        .cfg()
                        .get_build_setting(&l)
                        .ok()
                        .flatten()
                        .cloned()
                });

        let (scalar_actual, list_actual): (Option<String>, Option<Vec<String>>) =
            if let Some(value) = &cfg_value {
                match value {
                    BuildSettingValue::String(s) => (Some(s.clone()), None),
                    BuildSettingValue::Bool(b) => {
                        (Some(if *b { "True" } else { "False" }.to_owned()), None)
                    }
                    BuildSettingValue::Int(i) => (Some(i.to_string()), None),
                    BuildSettingValue::StringList(xs) | BuildSettingValue::StringSet(xs) => {
                        (None, Some(xs.clone()))
                    }
                }
            } else if let Some(cli_val) =
                kuro_build_api::interpreter::rule_defs::build_config::get_starlark_flag(
                    &flag_label_str,
                )
            {
                // CLI overrides arrive as a single string; string_list flags are
                // passed comma-separated, so split if there's a comma.
                if cli_val.contains(',') {
                    (
                        None,
                        Some(cli_val.split(',').map(|s| s.to_owned()).collect()),
                    )
                } else {
                    (Some(cli_val), None)
                }
            } else {
                match flag_node.attr_or_none("build_setting_default", AttrInspectOptions::All) {
                    Some(attr) => match &attr.value {
                        CoercedAttr::String(s) => (Some(s.0.as_str().to_owned()), None),
                        CoercedAttr::Bool(b) => {
                            (Some(if b.0 { "True" } else { "False" }.to_owned()), None)
                        }
                        CoercedAttr::Int(i) => (Some(i.to_string()), None),
                        CoercedAttr::List(list) => {
                            let items: Vec<String> = list
                                .0
                                .iter()
                                .filter_map(|item| match item {
                                    CoercedAttr::String(s) => Some(s.0.as_str().to_owned()),
                                    CoercedAttr::Int(i) => Some(i.to_string()),
                                    CoercedAttr::Bool(b) => {
                                        Some(if b.0 { "True" } else { "False" }.to_owned())
                                    }
                                    _ => None,
                                })
                                .collect();
                            (None, Some(items))
                        }
                        _ => return Ok(false),
                    },
                    None => return Ok(false),
                }
            };

        let matched = match (&scalar_actual, &list_actual) {
            (Some(actual), _) => actual == &expected_str,
            (_, Some(list)) => list.iter().any(|v| v == &expected_str),
            _ => false,
        };
        if !matched {
            return Ok(false);
        }
    }

    Ok(true) // All flag_values match their build_setting_defaults
}

/// Check whether all `values` entries in a `config_setting` target match the current config.
///
/// Supports two key formats:
/// - Bazel-style simple keys: `"compilation_mode"`, `"cpu"`, `"define"`, etc.
///   These map to Bazel command-line flags and are resolved against known defaults.
/// - Buck2-style dotted keys: `"section.property"` format (buckconfig key-value).
///
/// Returns `true` if all values match (or if `values` is empty), `false` otherwise.
async fn check_config_setting_values(
    configured_node: ConfiguredTargetNodeRef<'_>,
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<bool> {
    let target_node = configured_node.to_owned();
    let target_ref = target_node.target_node().as_ref();

    let values_attr = match target_ref.attr_or_none("values", AttrInspectOptions::All) {
        Some(attr) => attr,
        None => return Ok(true), // No values attr → vacuously matches
    };

    let pairs = match &values_attr.value {
        CoercedAttr::Dict(d) => d.0.clone(),
        _ => return Ok(true), // Not a dict → vacuously matches
    };

    if pairs.is_empty() {
        return Ok(true); // Empty values dict → vacuously matches
    }

    let config_setting_cell = target_ref.label().pkg().cell_name();

    for (key, expected_value) in pairs.iter() {
        let key_str = match key {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false), // Unexpected key type
        };

        let expected_str = match expected_value {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false), // Unexpected value type
        };

        // Check if this is a Bazel-style simple key (no dot) or Buck2-style dotted key
        if let Some(dot_pos) = key_str.find('.') {
            // Buck2-style: "section.property" format
            let section = &key_str[..dot_pos];
            let property = &key_str[dot_pos + 1..];

            let actual_value = ctx
                .get_legacy_config_property(
                    config_setting_cell,
                    BuckconfigKeyRef { section, property },
                )
                .await?;

            match actual_value {
                Some(v) if v.as_ref() == expected_str.as_str() => {
                    // This key matches
                }
                _ => return Ok(false), // Mismatch or not set
            }
        } else {
            // Bazel-style: resolve well-known keys against host defaults
            let matches = resolve_bazel_config_value(&key_str, &expected_str);
            if !matches {
                return Ok(false);
            }
        }
    }

    Ok(true) // All values match
}

/// Check whether all `define_values` entries in a `config_setting` target match the current
/// --define flags. In Bazel, `config_setting(define_values = {"FOO": "bar"})` matches when
/// `--define FOO=bar` is passed on the command line.
fn check_config_setting_define_values(
    configured_node: ConfiguredTargetNodeRef<'_>,
) -> kuro_error::Result<bool> {
    let target_node = configured_node.to_owned();
    let target_ref = target_node.target_node().as_ref();

    let define_attr = match target_ref.attr_or_none("define_values", AttrInspectOptions::All) {
        Some(attr) => attr,
        None => return Ok(true), // No define_values attr → vacuously matches
    };

    let pairs = match &define_attr.value {
        CoercedAttr::Dict(d) => d.0.clone(),
        _ => return Ok(true), // Not a dict → vacuously matches
    };

    if pairs.is_empty() {
        return Ok(true); // Empty define_values dict → vacuously matches
    }

    for (key, expected_value) in pairs.iter() {
        let key_str = match key {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false),
        };
        let expected_str = match expected_value {
            CoercedAttr::String(s) => s.0.as_str().to_owned(),
            _ => return Ok(false),
        };

        match kuro_build_api::interpreter::rule_defs::build_config::get_define(&key_str) {
            Some(actual) if actual == expected_str => {} // Matches
            _ => return Ok(false),                       // No match
        }
    }

    Ok(true)
}

/// Resolve a Bazel-style config_setting value key against known defaults.
///
/// In Bazel, `config_setting(values = {"compilation_mode": "opt"})` matches
/// against command-line flags like `--compilation_mode=opt`.
fn resolve_bazel_config_value(key: &str, expected: &str) -> bool {
    match key {
        "compilation_mode" => {
            let mode = kuro_build_api::interpreter::rule_defs::build_config::get_compilation_mode();
            expected == mode
        }
        "cpu" => {
            // Match against host CPU
            let host_cpu = if cfg!(target_arch = "x86_64") {
                "k8"
            } else if cfg!(target_arch = "aarch64") {
                "aarch64"
            } else {
                "unknown"
            };
            expected == host_cpu
        }
        "host_cpu" => {
            let host_cpu = if cfg!(target_arch = "x86_64") {
                "k8"
            } else if cfg!(target_arch = "aarch64") {
                "aarch64"
            } else {
                "unknown"
            };
            expected == host_cpu
        }
        "define" => {
            // Check if expected is "KEY=VALUE" format and match against --define flags
            if let Some((def_key, def_val)) = expected.split_once('=') {
                kuro_build_api::interpreter::rule_defs::build_config::get_define(def_key).as_deref()
                    == Some(def_val)
            } else {
                // Just check if the key exists
                kuro_build_api::interpreter::rule_defs::build_config::get_define(expected).is_some()
            }
        }
        "stamp" => {
            // Default stamping is off
            expected == "0"
        }
        "features" => {
            // No features set by default
            false
        }
        "crosstool_top" => {
            // Default crosstool_top matches a typical auto-detected toolchain
            expected.contains("local_config_cc") || expected.contains("cc_toolchain_suite")
        }
        "compiler" => {
            // Match against host compiler type
            if cfg!(windows) {
                // MSVC is default on Windows
                expected == "msvc-cl" || expected == "cl" || expected == "msvc"
            } else if cfg!(target_os = "macos") {
                expected == "clang" || expected == "compiler"
            } else {
                expected == "gcc" || expected == "compiler"
            }
        }
        "host_crosstool_top" => {
            expected.contains("local_config_cc") || expected.contains("cc_toolchain_suite")
        }
        // For unknown keys, don't match
        _ => false,
    }
}

/// Compute aspect results for dependencies that have aspects attached via attributes.
///
/// This scans the target's attributes for those with `aspects` (e.g.,
/// `attr.label_list(aspects=[cc_proto_aspect])`), identifies which deps need aspect
/// computation, and runs the aspects via DICE (leveraging caching from gather_deps()).
///
/// Returns a map from dep label to the aspect's provider collection, which should be
/// merged into the dep's base provider collection during resolution.
async fn compute_dep_aspects<'v>(
    configured_node: ConfiguredTargetNodeRef<'v>,
    _dep_analysis: &[(&'v ConfiguredTargetLabel, AnalysisResult)],
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>> {
    use kuro_core::provider::label::ConfiguredProvidersLabel;
    use kuro_node::attrs::configured_traversal::ConfiguredAttrTraversal;

    let pkg = configured_node.label().pkg();

    // Collect (dep_label, aspect_type) pairs by traversing each attribute.
    // Only apply aspects to deps from the specific attribute that declares those aspects.
    struct DepCollector {
        deps: Vec<ConfiguredTargetLabel>,
    }
    impl ConfiguredAttrTraversal for DepCollector {
        fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> kuro_error::Result<()> {
            self.deps.push(dep.target().dupe());
            Ok(())
        }
    }

    let aspect_keys = {
        let mut seen: Vec<(ConfiguredTargetLabel, usize)> = Vec::new();
        let mut keys = Vec::new();

        for a in configured_node.attrs(AttrInspectOptions::All) {
            let aspects: Vec<_> = a.attr.aspects().iter().map(|asp| asp.dupe()).collect();
            if aspects.is_empty() {
                continue;
            }

            // Traverse this attribute to find which deps it resolves to
            let mut collector = DepCollector { deps: Vec::new() };
            let _ = a.value.traverse(pkg, &mut collector);

            // Create AspectKeys only for these deps × this attribute's aspects
            for dep_label in &collector.deps {
                for aspect_type in &aspects {
                    let id = Arc::as_ptr(aspect_type) as usize;
                    if !seen.iter().any(|(l, i)| l == dep_label && *i == id) {
                        seen.push((dep_label.dupe(), id));
                        keys.push(AspectKey::new(dep_label.dupe(), aspect_type.dupe()));
                    }
                }
            }
        }
        keys
    };

    if aspect_keys.is_empty() {
        return Ok(HashMap::new());
    }

    // Compute all aspect keys via DICE (cached from gather_deps)
    let mut aspect_results: HashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue> =
        HashMap::new();

    for key in &aspect_keys {
        match ctx.compute(key).await {
            Ok(Ok(aspect_value)) => {
                let dep_label = key.target.dupe();
                if let Some(existing) = aspect_results.get(&dep_label) {
                    let merged = kuro_build_api::interpreter::rule_defs::provider::collection::merge_provider_collections(
                        existing,
                        &aspect_value.providers,
                    );
                    aspect_results.insert(dep_label, merged);
                } else {
                    aspect_results.insert(dep_label, aspect_value.providers);
                }
            }
            Ok(Err(e)) => {
                // Aspect computation failed - skip this dep but log the error
                tracing::warn!("Aspect computation failed for dep {}: {}", key.target, e);
            }
            Err(e) => {
                // Aspect DICE computation failed - skip this dep but log the error
                tracing::warn!("Aspect computation failed for dep {}: {}", key.target, e);
            }
        }
    }

    Ok(aspect_results)
}

pub async fn get_loaded_module(
    ctx: &mut DiceComputations<'_>,
    func: &StarlarkRuleType,
) -> kuro_error::Result<LoadedModule> {
    let module = match &func.path {
        BzlOrBxlPath::Bxl(bxl_file_path) => {
            let module_path = StarlarkModulePath::BxlFile(&bxl_file_path);
            ctx.get_loaded_module(module_path).await?
        }
        BzlOrBxlPath::Bzl(import_path) => {
            ctx.get_loaded_module_from_import_path(import_path).await?
        }
    };
    Ok(module)
}

/// Load `@kuro_builtins//:exports.bzl` via DICE so analysis can reach
/// `rule_implementation_wrapper` and `aspect_implementation_wrapper`.
/// Returns `None` for workspaces where `@kuro_builtins` is not
/// registered (legacy non-bzlmod projects without `[external_cells]
/// kuro_builtins = bundled`); analysis falls back to direct impl
/// invocation in that case. Made `pub(crate)` so the aspect dispatch
/// path
/// (`super::aspect_calculation`) can reuse the same DICE round-trip.
pub(crate) async fn get_kuro_builtins_module(
    ctx: &mut DiceComputations<'_>,
) -> kuro_error::Result<Option<starlark::environment::FrozenModule>> {
    use kuro_core::bzl::ImportPath;
    use kuro_core::cells::build_file_cell::BuildFileCell;
    use kuro_core::cells::cell_path::CellPath;
    use kuro_core::cells::paths::CellRelativePathBuf;

    let cell_resolver = ctx.get_cell_resolver().await?;
    let root_cell = cell_resolver.root_cell();
    let alias_resolver = ctx.get_cell_alias_resolver(root_cell).await?;

    // Cheap pre-check: if the alias isn't registered, skip the DICE
    // round-trip entirely. Fresh kuro builds always register
    // @kuro_builtins via `kuro_common::legacy_configs::cells`, but
    // legacy and external-test-cell setups may not.
    if alias_resolver.resolve("kuro_builtins").is_err() {
        return Ok(None);
    }

    let kb_cell = match alias_resolver.resolve("kuro_builtins") {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let cell_path = CellPath::new(
        kb_cell,
        CellRelativePathBuf::unchecked_new("exports.bzl".to_owned()),
    );
    let import_path =
        match ImportPath::new_with_build_file_cells(cell_path, BuildFileCell::new(root_cell)) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
    match ctx.get_loaded_module_from_import_path(&import_path).await {
        Ok(m) => Ok(Some(m.env().dupe())),
        Err(_) => Ok(None),
    }
}

pub async fn get_rule_spec(
    ctx: &mut DiceComputations<'_>,
    func: &StarlarkRuleType,
) -> kuro_error::Result<impl RuleSpec + use<>> {
    let module = get_loaded_module(ctx, func).await?;
    let builtins_module = get_kuro_builtins_module(ctx).await?;
    Ok(get_user_defined_rule_spec(
        module.env().dupe(),
        func,
        builtins_module,
    ))
}

async fn get_analysis_result(
    ctx: &mut DiceComputations<'_>,
    target: &ConfiguredTargetLabel,
    cancellation: &CancellationContext,
) -> kuro_error::Result<MaybeCompatible<AnalysisResult>> {
    get_analysis_result_inner(ctx, target, cancellation)
        .await
        .tag(ErrorTag::Analysis)
}

async fn get_analysis_result_inner(
    ctx: &mut DiceComputations<'_>,
    target: &ConfiguredTargetLabel,
    cancellation: &CancellationContext,
) -> kuro_error::Result<MaybeCompatible<AnalysisResult>> {
    let configured_node: MaybeCompatible<ConfiguredTargetNode> =
        ctx.get_configured_target_node(target).await?;
    let configured_node: ConfiguredTargetNode = match configured_node {
        MaybeCompatible::Incompatible(reason) => {
            return Ok(MaybeCompatible::Incompatible(reason));
        }
        MaybeCompatible::Compatible(configured_node) => configured_node,
    };

    // Phase 6: Eagerly load registered toolchain packages so the
    // DeclaredToolchainInfo registry is populated before resolution runs.
    // This runs once per session (guarded by AtomicBool in ensure_registered_toolchains_loaded).
    crate::analysis::env::ensure_registered_toolchains_loaded(ctx).await;

    // For precision, grab the *actual* rule type and not the *underlying* rule type.
    let target_rule_type_name = configured_node.rule_type().name().to_owned();

    let configured_node = configured_node.as_ref();

    let ((res, now), spans): ((kuro_error::Result<_>, _), _) = match configured_node.rule_type() {
        RuleType::Starlark(func) => {
            let dep_analysis = get_dep_analysis(configured_node, ctx).await?;
            let query_results = resolve_queries(ctx, configured_node).await?;
            dep_analysis_checkpoint(
                "analysis_deps_ready",
                target,
                &dep_analysis,
                query_results.len(),
            );

            // Phase 8h: Compute aspect results for deps that have aspects on their attributes.
            let aspect_results = compute_dep_aspects(configured_node, &dep_analysis, ctx).await?;
            analysis_checkpoint(
                "analysis_aspects_ready",
                target,
                [
                    ("active", ANALYSIS_ACTIVE.load(Ordering::Relaxed)),
                    ("completed", ANALYSIS_COMPLETED.load(Ordering::Relaxed)),
                    ("max_active", ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed)),
                    ("deps", dep_analysis.len()),
                    ("queries", query_results.len()),
                    ("aspects", aspect_results.len()),
                ],
            );

            let now = TimeSpan::start_now();
            let (res, spans) = async_record_root_spans(async {
                let rule_spec = get_rule_spec(ctx, func).await?;
                let start_event = kuro_data::AnalysisStart {
                    target: Some(target.as_proto().into()),
                    rule: func.to_string(),
                };

                span_async(start_event, async {
                    let mut profile = None;
                    let mut declared_artifacts = None;
                    let mut declared_actions = None;

                    let result: kuro_error::Result<_> = try {
                        let result = span_async_simple(
                            kuro_data::AnalysisStageStart {
                                stage: Some(kuro_data::analysis_stage_start::Stage::EvaluateRule(
                                    (),
                                )),
                            },
                            run_analysis(
                                ctx,
                                target,
                                dep_analysis,
                                query_results,
                                configured_node.execution_platform_resolution(),
                                &rule_spec,
                                configured_node,
                                cancellation,
                                aspect_results,
                            ),
                            kuro_data::AnalysisStageEnd {},
                        )
                        .await?;

                        profile = Some(make_analysis_profile(&result)?);
                        declared_artifacts = Some(result.num_declared_artifacts);
                        declared_actions = Some(result.num_declared_actions);
                        analysis_result_checkpoint(
                            "analysis_evaluate_rule_result",
                            target,
                            &result,
                            ANALYSIS_ACTIVE.load(Ordering::Relaxed),
                            ANALYSIS_COMPLETED.load(Ordering::Relaxed),
                            ANALYSIS_MAX_ACTIVE.load(Ordering::Relaxed),
                        );

                        MaybeCompatible::Compatible(result)
                    };

                    (
                        result,
                        kuro_data::AnalysisEnd {
                            target: Some(target.as_proto().into()),
                            rule: func.to_string(),
                            profile,
                            declared_actions,
                            declared_artifacts,
                        },
                    )
                })
                .await
            })
            .await;

            ((res, now), spans)
        }
        RuleType::Forward => {
            let mut dep_analysis = get_dep_analysis(configured_node, ctx).await?;
            let now = TimeSpan::start_now();
            let (res, spans) = record_root_spans(|| {
                let one_dep_analysis = dep_analysis
                    .pop()
                    .internal_error("Forward node analysis produced no results")?;
                if !dep_analysis.is_empty() {
                    return Err(internal_error!(
                        "Forward node analysis produced more than one result"
                    ));
                }
                Ok(MaybeCompatible::Compatible(one_dep_analysis.1))
            });

            ((res, now), spans)
        }
        RuleType::Native(kind) => {
            // Native rules are built-in rules like constraint_setting and constraint_value
            // that are required for Bazel compatibility with BCR packages like @platforms.
            let dep_analysis = get_dep_analysis(configured_node, ctx).await?;

            // For config_setting, pre-compute whether flag_values and values match.
            // This must be done asynchronously (DICE lookup) before the sync analyze_native_rule call.
            let (flag_values_match, values_match, define_values_match) =
                if matches!(kind, NativeRuleKind::ConfigSetting) {
                    let fv = check_config_setting_flag_values(configured_node, ctx).await?;
                    let vm = check_config_setting_values(configured_node, ctx).await?;
                    let dv = check_config_setting_define_values(configured_node)?;
                    (fv, vm, dv)
                } else {
                    (true, true, true) // irrelevant for non-config_setting rules
                };

            let now = TimeSpan::start_now();
            let (res, spans) = record_root_spans(|| {
                let result = crate::analysis::native_rule_analysis::analyze_native_rule(
                    target,
                    configured_node,
                    kind,
                    dep_analysis,
                    flag_values_match && define_values_match,
                    values_match,
                )?;
                Ok(MaybeCompatible::Compatible(result))
            });

            ((res, now), spans)
        }
    };

    ctx.store_evaluation_data(AnalysisKeyActivationData {
        waiting_data: WaitingData::new(),
        time_span: now.end_now(),
        spans,
        analysis_with_extra_data: AnalysisWithExtraData {
            target_rule_type_name: Some(target_rule_type_name),
        },
    })?;

    res
}

fn make_analysis_profile(res: &AnalysisResult) -> kuro_error::Result<kuro_data::AnalysisProfile> {
    let heap = res.providers()?.owner();

    Ok(kuro_data::AnalysisProfile {
        starlark_allocated_bytes: heap.allocated_bytes() as u64,
        starlark_available_bytes: heap.available_bytes() as u64,
    })
}

fn all_deps(nodes: &[ConfiguredTargetNode]) -> LabelIndexedSet<ConfiguredTargetNode> {
    let mut stack = nodes.to_vec();
    let mut visited = LabelIndexedSet::new();
    let mut result = LabelIndexedSet::new();
    while let Some(node) = stack.pop() {
        if visited.insert(node.dupe()) {
            match node.rule_type() {
                RuleType::Starlark(_) => {
                    result.insert(node.dupe());
                }
                RuleType::Forward => {
                    // No starlark code ran on forward node.
                }
                RuleType::Native(_) => {
                    // Native rules don't run starlark code for analysis.
                    // They may still have deps that run starlark.
                    result.insert(node.dupe());
                }
            }

            stack.extend(node.deps().duped());
        }
    }
    result
}

pub async fn profile_analysis(
    ctx: &mut DiceComputations<'_>,
    targets: &[ConfiguredTargetLabel],
) -> kuro_error::Result<StarlarkProfileDataAndStats> {
    // Self check.
    for target in targets {
        let profile_mode = ctx
            .get_starlark_profiler_mode(&StarlarkEvalKind::Analysis(target.dupe()))
            .await?;
        if !matches!(profile_mode, StarlarkProfileMode::Profile(_)) {
            return Err(internal_error!("recursive analysis configured incorrectly"));
        }
    }

    let nodes: Vec<ConfiguredTargetNode> = ctx
        .try_compute_join(targets.iter(), |ctx, target| {
            async move {
                let node = ctx
                    .get_configured_target_node(target)
                    .await?
                    .require_compatible()?;
                kuro_error::Ok(node)
            }
            .boxed()
        })
        .await?;

    let all_deps = all_deps(&nodes);

    let profile_datas = ctx
        .try_compute_join(all_deps.iter(), |ctx, node| {
            async move {
                let result = ctx
                    .get_analysis_result(node.label())
                    .await?
                    .require_compatible()?;
                // This may be `None` if we are running profiling for a subset of the targets.
                kuro_error::Ok(result.profile_data)
            }
            .boxed()
        })
        .await?;

    StarlarkProfileDataAndStats::merge(
        profile_datas
            .iter()
            .filter_map(|o| o.as_ref())
            .map(|x| &**x),
    )
}

pub struct AnalysisKeyActivationData {
    pub waiting_data: WaitingData,
    pub time_span: TimeSpan,
    pub spans: SmallVec<[SpanId; 1]>,
    pub analysis_with_extra_data: AnalysisWithExtraData,
}

#[derive(Clone)]
pub struct AnalysisWithExtraData {
    pub target_rule_type_name: Option<String>,
}
