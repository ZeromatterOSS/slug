/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Public loading-query evaluation facade.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_bzlmod_v2::RootModuleLoadingAnchorObservationKey;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;

use crate::QueryPolicy;
use crate::generic::QueryEvaluator;
use crate::graph::QueryError;
use crate::loading_environment::LoadingQueryEnvironment;
pub use crate::output::QueryOrder;
pub use crate::output::QueryOutput;
use crate::parse_query_expression;
use crate::validate_loading_query;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, Allocative)]
pub enum QueryOutputCompletion {
    #[default]
    Standard,
    LabelKind,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RootQueryCommandKey {
    workspace: NormalizedAbsolutePath,
    source: CompactString,
    order: QueryOrder,
    policy: QueryPolicy,
    completion: QueryOutputCompletion,
}

#[doc(hidden)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Allocative)]
pub struct RootQueryCommandObservationKey(RootQueryCommandKey);

#[doc(hidden)]
#[derive(Debug, Clone, Eq, PartialEq, Allocative, Dupe)]
pub struct ObservedRootQueryCommand {
    result: Arc<Result<QueryOutput, QueryError>>,
    observations: PathObservationEpoch,
}

impl ObservedRootQueryCommand {
    #[doc(hidden)]
    pub fn result(&self) -> &Arc<Result<QueryOutput, QueryError>> {
        &self.result
    }

    #[doc(hidden)]
    pub fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }

    #[doc(hidden)]
    pub fn into_result(self) -> Arc<Result<QueryOutput, QueryError>> {
        self.result
    }
}

impl RootQueryCommandKey {
    pub fn new(
        workspace: NormalizedAbsolutePath,
        source: impl Into<CompactString>,
        order: QueryOrder,
        policy: QueryPolicy,
        completion: QueryOutputCompletion,
    ) -> Result<Self, QueryError> {
        let source = source.into();
        let expression = parse_query_expression(&source)
            .map_err(|error| QueryError::syntax(error.to_string()))?;
        validate_loading_query(&expression)
            .map_err(|error| QueryError::syntax(error.to_string()))?;
        Ok(Self {
            workspace,
            source,
            order,
            policy,
            completion,
        })
    }
}

impl RootQueryCommandObservationKey {
    #[doc(hidden)]
    pub fn new(
        workspace: NormalizedAbsolutePath,
        source: impl Into<CompactString>,
        order: QueryOrder,
        policy: QueryPolicy,
        completion: QueryOutputCompletion,
    ) -> Result<Self, QueryError> {
        RootQueryCommandKey::new(workspace, source, order, policy, completion).map(Self)
    }
}

impl fmt::Display for RootQueryCommandKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-query-command:{}", self.source)
    }
}

impl fmt::Display for RootQueryCommandObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Clone, Copy)]
enum RootQueryMode {
    Legacy,
    Observed,
}

type ObservedRootQueryValue =
    LoadingPreparationOutcome<Result<ObservedRootQueryCommand, ObservedPathFrontierError>>;

fn root_query_complete(
    result: Result<QueryOutput, QueryError>,
    observations: PathObservationEpoch,
) -> ObservedRootQueryValue {
    LoadingPreparationOutcome::Complete(Ok(ObservedRootQueryCommand {
        result: Arc::new(result),
        observations,
    }))
}

#[async_trait]
impl Key for RootQueryCommandKey {
    type Value = LoadingPreparationOutcome<Arc<Result<QueryOutput, QueryError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match compute_root_query_command(self, ctx, RootQueryMode::Legacy).await {
            LoadingPreparationOutcome::Need(need) => LoadingPreparationOutcome::Need(need),
            LoadingPreparationOutcome::Complete(Ok(value)) => {
                LoadingPreparationOutcome::Complete(value.result)
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                panic!("legacy root query produced observed outer error: {error}")
            }
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for RootQueryCommandObservationKey {
    type Value = ObservedRootQueryValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_root_query_command(&self.0, ctx, RootQueryMode::Observed).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

async fn compute_root_query_command(
    key: &RootQueryCommandKey,
    ctx: &mut DiceComputations<'_>,
    mode: RootQueryMode,
) -> ObservedRootQueryValue {
    let observations = match mode {
        RootQueryMode::Legacy => match ctx
            .compute(&RootModuleLoadingAnchorKey::new(key.workspace.clone()))
            .await
            .expect("root module loading anchor DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(anchor) => match anchor.as_ref() {
                Ok(_) => PathObservationEpoch::empty(),
                Err(error) => {
                    return root_query_complete(
                        Err(QueryError::package_loading(error.to_string())),
                        PathObservationEpoch::empty(),
                    );
                }
            },
        },
        RootQueryMode::Observed => match ctx
            .compute(&RootModuleLoadingAnchorObservationKey::new(
                key.workspace.clone(),
            ))
            .await
            .expect("observed root module loading anchor DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(Err(error)) => {
                return LoadingPreparationOutcome::Complete(Err(error));
            }
            LoadingPreparationOutcome::Complete(Ok(anchor)) => match anchor.result() {
                Ok(_) => anchor.observations().dupe(),
                Err(error) => {
                    return root_query_complete(
                        Err(QueryError::package_loading(error.to_string())),
                        anchor.observations().dupe(),
                    );
                }
            },
        },
    };

    let expression = match parse_query_expression(&key.source) {
        Ok(expression) => expression,
        Err(error) => {
            return root_query_complete(Err(QueryError::syntax(error.to_string())), observations);
        }
    };
    let environment = match mode {
        RootQueryMode::Legacy => {
            LoadingQueryEnvironment::new_root(ctx, key.workspace.clone(), key.policy)
        }
        RootQueryMode::Observed => LoadingQueryEnvironment::new_root_observed(
            ctx,
            key.workspace.clone(),
            key.policy,
            observations,
        ),
    };
    let mut evaluator = QueryEvaluator::new(environment);
    let result =
        evaluate_parsed_query(&mut evaluator, &expression, key.order, key.completion).await;
    if result
        .as_ref()
        .is_err_and(QueryError::is_preparation_restart)
    {
        return LoadingPreparationOutcome::Need(
            evaluator
                .environment
                .take_preparation_needs()
                .expect("query restart sentinel requires typed preparation Needs"),
        );
    }
    if result
        .as_ref()
        .is_err_and(QueryError::is_observation_restart)
    {
        return LoadingPreparationOutcome::Complete(Err(evaluator
            .environment
            .take_observation_outer()
            .expect("query observation sentinel requires typed outer failure")));
    }
    assert!(
        evaluator.environment.take_preparation_needs().is_none(),
        "typed query Needs require the private restart sentinel"
    );
    assert!(
        evaluator.environment.take_observation_outer().is_none(),
        "typed query outer failures require the private restart sentinel"
    );
    let observations = evaluator.environment.take_observations();
    root_query_complete(result, observations)
}

pub async fn evaluate_loading_query(
    ctx: &mut DiceComputations<'_>,
    workspace: PathBuf,
    source: &str,
    order: QueryOrder,
) -> Result<QueryOutput, QueryError> {
    evaluate_loading_query_with_policy(ctx, workspace, source, order, QueryPolicy::default()).await
}

pub async fn evaluate_loading_query_with_policy(
    ctx: &mut DiceComputations<'_>,
    workspace: PathBuf,
    source: &str,
    order: QueryOrder,
    policy: QueryPolicy,
) -> Result<QueryOutput, QueryError> {
    evaluate_loading_query_with_policy_and_output_completion(
        ctx,
        workspace,
        source,
        order,
        policy,
        QueryOutputCompletion::Standard,
    )
    .await
}

pub async fn evaluate_loading_query_with_policy_and_output_completion(
    ctx: &mut DiceComputations<'_>,
    workspace: PathBuf,
    source: &str,
    order: QueryOrder,
    policy: QueryPolicy,
    completion: QueryOutputCompletion,
) -> Result<QueryOutput, QueryError> {
    let expression =
        parse_query_expression(source).map_err(|error| QueryError::syntax(error.to_string()))?;
    validate_loading_query(&expression).map_err(|error| QueryError::syntax(error.to_string()))?;
    let mut evaluator = QueryEvaluator::new(LoadingQueryEnvironment::new(ctx, workspace, policy));
    evaluate_parsed_query(&mut evaluator, &expression, order, completion).await
}

async fn evaluate_parsed_query(
    evaluator: &mut QueryEvaluator<LoadingQueryEnvironment<'_, '_>>,
    expression: &crate::QueryExpression,
    order: QueryOrder,
    completion: QueryOutputCompletion,
) -> Result<QueryOutput, QueryError> {
    let targets = evaluator.evaluate(&expression).await?;
    if completion == QueryOutputCompletion::LabelKind {
        evaluator.environment.complete_label_kinds(&targets).await?;
    }
    let graph = evaluator.environment.selected_graph(&targets, completion);
    let labels: Vec<CompactString> = if order == QueryOrder::Full {
        evaluator
            .environment
            .selected_full_order(&targets)
            .into_iter()
            .map(|label| label.output_label())
            .collect()
    } else {
        let mut labels = targets
            .unique_output_labels(&evaluator.environment.candidates)
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if !expression.is_top_level_somepath() {
            labels.sort_unstable();
        }
        labels
            .into_iter()
            .map(|label| label.output_label())
            .collect()
    };
    Ok(QueryOutput {
        labels: labels.into(),
        graph,
    })
}
