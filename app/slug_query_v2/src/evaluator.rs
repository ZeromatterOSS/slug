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
use slug_bzlmod_v2::RootModuleLoadingAnchorKey;
use slug_loading_v2::LoadingPreparationOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;

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

impl fmt::Display for RootQueryCommandKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-query-command:{}", self.source)
    }
}

#[async_trait]
impl Key for RootQueryCommandKey {
    type Value = LoadingPreparationOutcome<Arc<Result<QueryOutput, QueryError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&RootModuleLoadingAnchorKey::new(self.workspace.clone()))
            .await
            .expect("root module loading anchor DICE invariant")
        {
            LoadingPreparationOutcome::Need(need) => {
                return LoadingPreparationOutcome::Need(need);
            }
            LoadingPreparationOutcome::Complete(anchor) => {
                if let Err(error) = anchor.as_ref() {
                    return LoadingPreparationOutcome::Complete(Arc::new(Err(
                        QueryError::package_loading(error.to_string()),
                    )));
                }
            }
        }

        let expression = match parse_query_expression(&self.source) {
            Ok(expression) => expression,
            Err(error) => {
                return LoadingPreparationOutcome::Complete(Arc::new(Err(QueryError::syntax(
                    error.to_string(),
                ))));
            }
        };
        let mut evaluator = QueryEvaluator::new(LoadingQueryEnvironment::new_root(
            ctx,
            self.workspace.clone(),
            self.policy,
        ));
        let result =
            evaluate_parsed_query(&mut evaluator, &expression, self.order, self.completion).await;
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
        assert!(
            evaluator.environment.take_preparation_needs().is_none(),
            "typed query Needs require the private restart sentinel"
        );
        LoadingPreparationOutcome::Complete(Arc::new(result))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
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
