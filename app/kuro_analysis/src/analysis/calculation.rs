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
use std::sync::Arc;

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
use kuro_build_api::keep_going::KeepGoing;
use kuro_build_signals::env::WaitingData;
use kuro_core::configuration::compatibility::MaybeCompatible;
use kuro_core::deferred::base_deferred_key::BaseDeferredKey;
use kuro_core::deferred::key::DeferredHolderKey;
use kuro_core::provider::label::ConfiguredProvidersLabel;
use kuro_core::target::configured_target_label::ConfiguredTargetLabel;
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
use kuro_node::bzl_or_bxl_path::BzlOrBxlPath;
use kuro_node::nodes::configured::ConfiguredTargetNode;
use kuro_node::nodes::configured::ConfiguredTargetNodeRef;
use kuro_node::nodes::configured_frontend::ConfiguredTargetNodeCalculation;
use kuro_node::rule_type::NativeRuleKind;
use kuro_node::rule_type::RuleType;
use kuro_node::rule_type::StarlarkRuleType;
use kuro_query::query::syntax::simple::eval::label_indexed::LabelIndexedSet;
use kuro_query::query::syntax::simple::eval::set::TargetSet;
use kuro_util::time_span::TimeSpan;
use smallvec::SmallVec;

use crate::analysis::env::RuleSpec;
use crate::analysis::env::get_user_defined_rule_spec;
use crate::analysis::env::run_analysis;
use crate::attrs::resolve::ctx::AnalysisQueryResult;

struct RuleAnalysisCalculationInstance;

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
        let deferred_key = DeferredHolderKey::Base(BaseDeferredKey::TargetLabel(self.0.dupe()));
        ctx.analysis_started(&deferred_key)?;
        let res = get_analysis_result(ctx, &self.0, cancellation)
            .await
            .with_buck_error_context(|| format!("Error running analysis for `{}`", &self.0))?;
        if let MaybeCompatible::Compatible(v) = &res {
            ctx.analysis_complete(&deferred_key, &DeferredHolder::Analysis(v.dupe()))?;
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
    KeepGoing::try_compute_join_all(ctx, configured_node.deps(), |ctx, dep| {
        async move {
            let res = ctx
                .get_analysis_result(dep.label())
                .await
                .and_then(|v| v.require_compatible());
            res.map(|x| (dep.label(), x))
        }
        .boxed()
    })
    .await
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

pub async fn get_rule_spec(
    ctx: &mut DiceComputations<'_>,
    func: &StarlarkRuleType,
) -> kuro_error::Result<impl RuleSpec + use<>> {
    let module = get_loaded_module(ctx, func).await?;
    Ok(get_user_defined_rule_spec(module.env().dupe(), func))
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

    // For precision, grab the *actual* rule type and not the *underlying* rule type.
    let target_rule_type_name = configured_node.rule_type().name().to_owned();

    let configured_node = configured_node.as_ref();

    let ((res, now), spans): ((kuro_error::Result<_>, _), _) = match configured_node.rule_type() {
        RuleType::Starlark(func) => {
            let (dep_analysis, query_results) = ctx
                .try_compute2(
                    |ctx| get_dep_analysis(configured_node, ctx).boxed(),
                    |ctx| resolve_queries(ctx, configured_node).boxed(),
                )
                .await?;

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
                            ),
                            kuro_data::AnalysisStageEnd {},
                        )
                        .await?;

                        profile = Some(make_analysis_profile(&result)?);
                        declared_artifacts = Some(result.num_declared_artifacts);
                        declared_actions = Some(result.num_declared_actions);

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

            let now = TimeSpan::start_now();
            let (res, spans) = record_root_spans(|| {
                let result = crate::analysis::native_rule_analysis::analyze_native_rule(
                    target,
                    configured_node,
                    kind,
                    dep_analysis,
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
