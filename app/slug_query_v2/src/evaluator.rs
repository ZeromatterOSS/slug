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

use std::path::PathBuf;

use compact_str::CompactString;
use dice::DiceComputations;

use crate::QueryPolicy;
use crate::generic::QueryEvaluator;
use crate::graph::QueryError;
use crate::loading_environment::LoadingQueryEnvironment;
pub use crate::output::QueryOrder;
pub use crate::output::QueryOutput;
use crate::parse_query_expression;
use crate::validate_loading_query;

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
    let expression =
        parse_query_expression(source).map_err(|error| QueryError::syntax(error.to_string()))?;
    validate_loading_query(&expression).map_err(|error| QueryError::syntax(error.to_string()))?;
    let mut evaluator = QueryEvaluator::new(LoadingQueryEnvironment::new(ctx, workspace, policy));
    let targets = evaluator.evaluate(&expression).await?;
    let graph = evaluator.environment.selected_graph(&targets);
    let labels: Vec<CompactString> = if order == QueryOrder::Full {
        evaluator
            .environment
            .selected_full_order(&targets)
            .into_iter()
            .map(|label| CompactString::new(label.to_string()))
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
            .map(|label| CompactString::new(label.to_string()))
            .collect()
    };
    Ok(QueryOutput {
        labels: labels.into(),
        graph,
    })
}
