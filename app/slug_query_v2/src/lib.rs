/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod evaluator;
pub mod expr;
mod generic;
pub mod graph;
mod loading_environment;
mod output;
mod parser;
pub(crate) mod provenance;
mod traversal;

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    Eq,
    PartialEq,
    Hash,
    allocative::Allocative
)]
pub struct QueryPolicy {
    pub strict_test_suite: bool,
}

#[doc(hidden)]
pub use evaluator::ObservedRootQueryCommand;
pub use evaluator::QueryOrder;
pub use evaluator::QueryOutput;
pub use evaluator::QueryOutputCompletion;
pub use evaluator::RootQueryCommandKey;
#[doc(hidden)]
pub use evaluator::RootQueryCommandObservationKey;
pub use evaluator::evaluate_loading_query;
pub use evaluator::evaluate_loading_query_with_policy;
pub use evaluator::evaluate_loading_query_with_policy_and_output_completion;
pub use expr::AqueryExpressionSpec;
pub use expr::AqueryScope;
pub use expr::BinaryOperator;
pub use expr::QueryExpression;
pub use expr::QueryExpressionKind;
pub use expr::QueryFunctionSpec;
pub use expr::QueryFunctionStatus;
pub use expr::QueryParseError;
pub use expr::SourceSpan;
pub use expr::Spanned;
pub use expr::aquery_expression_spec;
pub use expr::aquery_literal;
pub use expr::cquery_literals;
pub use expr::loading_query_function;
pub use expr::loading_query_functions;
pub use expr::parse_query_expression;
pub use expr::validate_cquery_query;
pub use expr::validate_loading_query;
pub use generic::CqueryQueryEnvironment;
pub use generic::TargetSet;
pub use generic::evaluate_cquery_query;
pub use generic::preflight_cquery_query;
pub use graph::QueryAttribute;
pub use graph::QueryEdge;
pub use graph::QueryEdgeKind;
pub use graph::QueryError;
pub use graph::QueryLabel;
pub use graph::QueryNode;
pub use graph::QueryNodeKind;
pub use graph::SubtreePackageSet;
pub use graph::SubtreePackageSetKey;
pub use graph::UnconfiguredPackageGraph;
pub use graph::UnconfiguredPackageGraphKey;
pub use output::render_unfactored_dot;
pub use slug_loading_v2::LoadingPreparationNeeds as QueryPreparationNeeds;
pub use slug_loading_v2::LoadingPreparationOutcome as QueryPreparationOutcome;
