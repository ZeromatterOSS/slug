/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Generic query evaluation and the loading-query DICE environment.

use std::collections::VecDeque;
use std::fmt;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use futures::FutureExt;
use futures::future::BoxFuture;
use slug_identity_v2::TargetPattern;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BinaryOperator;
use crate::QueryExpression;
use crate::QueryExpressionKind;
use crate::graph::QueryError;
use crate::graph::QueryLabel;
use crate::graph::QueryNode;
use crate::graph::RootPackageSetKey;
use crate::graph::UnconfiguredPackageGraph;
use crate::graph::UnconfiguredPackageGraphKey;
use crate::loading_query_function;
use crate::parse_query_expression;
use crate::validate_loading_query;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum QueryOrder {
    Auto,
    Full,
}

impl QueryOrder {
    pub fn parse(value: &str) -> Result<Self, QueryError> {
        match value {
            "auto" => Ok(Self::Auto),
            "full" => Ok(Self::Full),
            "deps" | "no" => Err(QueryError::syntax(format!(
                "--order_output={value} is not supported by this loading-query slice"
            ))),
            _ => Err(QueryError::syntax(format!(
                "unknown --order_output value: {value}"
            ))),
        }
    }
}

impl fmt::Display for QueryOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::Full => "full",
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
pub struct QueryOutput {
    pub labels: Arc<[CompactString]>,
}

impl QueryOutput {
    pub fn stdout(&self) -> String {
        let mut output = String::new();
        for label in self.labels.iter() {
            output.push_str(label);
            output.push('\n');
        }
        output
    }
}

#[derive(Debug, Clone, Allocative)]
pub struct TargetSet<T>(SmallSet<T>);

impl<T> Default for TargetSet<T> {
    fn default() -> Self {
        Self(SmallSet::new())
    }
}

impl<T> TargetSet<T>
where
    T: Clone + Eq + Hash,
{
    pub fn singleton(value: T) -> Self {
        let mut values = SmallSet::new();
        values.insert(value);
        Self(values)
    }

    pub fn insert(&mut self, value: T) {
        self.0.insert(value);
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    fn union(mut self, other: Self) -> Self {
        for value in other.0 {
            self.0.insert(value);
        }
        self
    }

    fn difference(mut self, other: &Self) -> Self {
        for value in other.iter() {
            self.0.shift_remove(value);
        }
        self
    }

    fn intersection(mut self, other: &Self) -> Self {
        let remove = self
            .iter()
            .filter(|value| !other.0.contains(*value))
            .cloned()
            .collect::<Vec<_>>();
        for value in remove {
            self.0.shift_remove(&value);
        }
        self
    }
}

#[async_trait]
pub trait QueryEnvironment {
    type Target: Clone + Eq + Hash + Send + Sync;

    async fn resolve_literal(
        &mut self,
        literal: &str,
    ) -> Result<TargetSet<Self::Target>, QueryError>;

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError>;
}

pub struct QueryEvaluator<E> {
    environment: E,
    functions: LoadingQueryFunctions,
}

impl<E> QueryEvaluator<E>
where
    E: QueryEnvironment + Send,
{
    pub fn new(environment: E) -> Self {
        Self {
            environment,
            functions: LoadingQueryFunctions,
        }
    }

    pub async fn evaluate(
        mut self,
        expression: &QueryExpression,
    ) -> Result<TargetSet<E::Target>, QueryError> {
        let mut variables = SmallMap::new();
        self.evaluate_inner(expression, &mut variables).await
    }

    fn evaluate_inner<'a>(
        &'a mut self,
        expression: &'a QueryExpression,
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            match &expression.kind {
                QueryExpressionKind::TargetLiteral(literal) => {
                    if let Some(name) = literal.strip_prefix('$') {
                        return variables.get(name).cloned().ok_or_else(|| {
                            QueryError::evaluation(format!("undefined query variable '${name}'"))
                        });
                    }
                    self.environment.resolve_literal(literal).await
                }
                QueryExpressionKind::Integer(_) => Err(QueryError::syntax(
                    "an integer is only valid as a query function argument",
                )),
                QueryExpressionKind::Set(literals) => {
                    let mut result = TargetSet::default();
                    for literal in literals.iter() {
                        result =
                            result.union(self.environment.resolve_literal(&literal.value).await?);
                    }
                    Ok(result)
                }
                QueryExpressionKind::Let { name, value, body } => {
                    let value = self.evaluate_inner(value, variables).await?;
                    let previous = variables.insert(name.value.clone(), value);
                    let result = self.evaluate_inner(body, variables).await;
                    if let Some(previous) = previous {
                        variables.insert(name.value.clone(), previous);
                    } else {
                        variables.shift_remove(name.value.as_str());
                    }
                    result
                }
                QueryExpressionKind::BinaryOpSequence { left, operations } => {
                    let mut result = self.evaluate_inner(left, variables).await?;
                    for (operator, right) in operations.iter() {
                        let right = self.evaluate_inner(right, variables).await?;
                        result = match operator {
                            BinaryOperator::Union => result.union(right),
                            BinaryOperator::Except => result.difference(&right),
                            BinaryOperator::Intersect => result.intersection(&right),
                        };
                    }
                    Ok(result)
                }
                QueryExpressionKind::Function { name, args } => {
                    let functions = self.functions;
                    let function = functions.get(&name.value).ok_or_else(|| {
                        QueryError::syntax(format!(
                            "query function '{}' was not validated",
                            name.value
                        ))
                    })?;
                    function.invoke(self, args, variables).await
                }
            }
        }
        .boxed()
    }
}

/// Buck2-shaped dynamic query function registry. The evaluator performs only
/// generic lookup/invoke; each function owns typed argument conversion and
/// implementation.
pub trait QueryFunctions<E: QueryEnvironment>: Send + Sync {
    fn get(&self, name: &str) -> Option<&dyn QueryFunction<E>>;
}

pub trait QueryFunction<E: QueryEnvironment>: Send + Sync {
    fn spec(&self) -> &'static crate::QueryFunctionSpec;

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>>;
}

#[derive(Debug, Clone, Copy)]
pub struct LoadingQueryFunctions;

static DEPS_FUNCTION: DepsFunction = DepsFunction;

impl<E> QueryFunctions<E> for LoadingQueryFunctions
where
    E: QueryEnvironment + Send,
{
    fn get(&self, name: &str) -> Option<&dyn QueryFunction<E>> {
        let spec = loading_query_function(name)?;
        if spec.status != crate::QueryFunctionStatus::Implemented {
            return None;
        }
        // The implemented registry contains one entry in this vertical. New
        // entries are added here, not in evaluator dispatch.
        if std::ptr::eq(
            spec,
            <DepsFunction as QueryFunction<E>>::spec(&DEPS_FUNCTION),
        ) {
            Some(&DEPS_FUNCTION)
        } else {
            None
        }
    }
}

trait QueryFunctionArg<E: QueryEnvironment>: Sized {
    fn accept_none() -> Option<Self> {
        None
    }

    fn eval<'a>(
        evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<Self, QueryError>>;
}

impl<E> QueryFunctionArg<E> for TargetSet<E::Target>
where
    E: QueryEnvironment + Send,
{
    fn eval<'a>(
        evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<Self, QueryError>> {
        evaluator.evaluate_inner(expression, variables)
    }
}

#[derive(Debug, Clone, Copy)]
struct QueryDepth(Option<u64>);

impl<E: QueryEnvironment> QueryFunctionArg<E> for QueryDepth {
    fn accept_none() -> Option<Self> {
        Some(Self(None))
    }

    fn eval<'a>(
        _evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        _variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<Self, QueryError>> {
        async move {
            match expression.kind {
                QueryExpressionKind::Integer(value) => Ok(Self(Some(value))),
                _ => Err(QueryError::syntax("expected an integer query argument")),
            }
        }
        .boxed()
    }
}

fn eval_arg<'a, E, A>(
    evaluator: &'a mut QueryEvaluator<E>,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    index: usize,
) -> BoxFuture<'a, Result<A, QueryError>>
where
    E: QueryEnvironment + Send,
    A: QueryFunctionArg<E> + Send + 'a,
{
    match args.get(index) {
        Some(expression) => A::eval(evaluator, expression, variables),
        None => async move {
            A::accept_none().ok_or_else(|| QueryError::syntax("missing query function argument"))
        }
        .boxed(),
    }
}

struct DepsFunction;

impl<E> QueryFunction<E> for DepsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("deps").expect("deps is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let roots: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            let depth: QueryDepth = eval_arg(evaluator, args, variables, 1).await?;
            transitive_closure(&mut evaluator.environment, roots, depth.0).await
        }
        .boxed()
    }
}

async fn transitive_closure<E>(
    environment: &mut E,
    roots: TargetSet<E::Target>,
    max_depth: Option<u64>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let mut result = TargetSet::default();
    async_depth_limited_traversal(environment, roots.iter().cloned(), max_depth, |target| {
        result.insert(target)
    })
    .await?;
    Ok(result)
}

/// Generic depth-limited traversal adapted from Buck2
/// `buck2_query/src/query/traversal.rs::async_depth_limited_traversal`.
///
/// Buck2 can queue concurrent immutable node lookups. Loading query owns a
/// mutable `DiceComputations`, so this port preserves the generic lookup /
/// visited / ordered-queue abstraction while resolving one node at a time.
pub async fn async_depth_limited_traversal<E>(
    environment: &mut E,
    roots: impl IntoIterator<Item = E::Target>,
    max_depth: Option<u64>,
    mut visit: impl FnMut(E::Target),
) -> Result<(), QueryError>
where
    E: QueryEnvironment + Send,
{
    let mut visited = SmallSet::new();
    let mut pending = VecDeque::new();
    for root in roots {
        if visited.insert(root.clone()) {
            pending.push_back((root, 0));
        }
    }
    while let Some((target, depth)) = pending.pop_front() {
        visit(target.clone());
        if max_depth.is_some_and(|limit| depth >= limit) {
            continue;
        }
        for dependency in environment.dependencies(&target).await?.iter() {
            if visited.insert(dependency.clone()) {
                pending.push_back((dependency.clone(), depth + 1));
            }
        }
    }
    Ok(())
}

pub struct LoadingQueryEnvironment<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    workspace: PathBuf,
}

impl<'a, 'd> LoadingQueryEnvironment<'a, 'd> {
    pub fn new(ctx: &'a mut DiceComputations<'d>, workspace: PathBuf) -> Self {
        Self { ctx, workspace }
    }

    async fn package_graph(
        &mut self,
        package: &str,
    ) -> Result<Arc<UnconfiguredPackageGraph>, QueryError> {
        let value = self
            .ctx
            .compute(&UnconfiguredPackageGraphKey {
                workspace: self.workspace.clone(),
                package: PathBuf::from(package),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        value.as_ref().clone()
    }

    async fn resolve_single(&mut self, label: QueryLabel) -> Result<QueryNode, QueryError> {
        if !label.is_root_repository() {
            return Err(QueryError::evaluation(format!(
                "external repository query labels are deferred: {label}"
            )));
        }
        let graph = self.package_graph(label.package()).await.map_err(|error| {
            if error.message.contains("no BUILD.bazel or BUILD file")
                || error.message.contains("package directory is absent")
            {
                QueryError::evaluation(format!(
                    "no such package '{}': BUILD file not found",
                    label.package()
                ))
            } else {
                error
            }
        })?;
        graph.nodes.get(&label).cloned().ok_or_else(|| {
            QueryError::evaluation(format!(
                "no such target '{}': target '{}' not declared in package '{}'",
                label,
                label.target(),
                label.package()
            ))
        })
    }
}

#[async_trait]
impl QueryEnvironment for LoadingQueryEnvironment<'_, '_> {
    type Target = QueryLabel;

    async fn resolve_literal(
        &mut self,
        literal: &str,
    ) -> Result<TargetSet<Self::Target>, QueryError> {
        if literal == "//..." {
            let packages = self
                .ctx
                .compute(&RootPackageSetKey {
                    workspace: self.workspace.clone(),
                })
                .await
                .map_err(|error| QueryError::evaluation(error.to_string()))?;
            let packages = packages.as_ref().as_ref().map_err(|error| error.clone())?;
            let mut result = TargetSet::default();
            for package in packages.packages.iter() {
                let graph = self.package_graph(package).await?;
                for (label, node) in graph.nodes.iter() {
                    if node.kind.is_rule() {
                        result.insert(label.clone());
                    }
                }
            }
            return Ok(result);
        }

        let pattern = TargetPattern::parse(literal).map_err(QueryError::evaluation)?;
        match pattern {
            TargetPattern::Single(label) => {
                let label = QueryLabel::parse_root(&label.to_string())?;
                self.resolve_single(label.clone())
                    .await
                    .map(|_| TargetSet::singleton(label))
            }
            TargetPattern::PackageAll { repo, package } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                let graph = self.package_graph(package.as_str()).await?;
                let mut result = TargetSet::default();
                for (label, node) in graph.nodes.iter() {
                    if node.kind.is_rule() {
                        result.insert(label.clone());
                    }
                }
                Ok(result)
            }
            TargetPattern::Recursive { .. } => Err(QueryError::evaluation(
                "only root recursive pattern //... is implemented",
            )),
        }
    }

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError> {
        self.resolve_single(target.clone())
            .await
            .map(|node| node.dependencies)
    }
}

pub async fn evaluate_loading_query(
    ctx: &mut DiceComputations<'_>,
    workspace: PathBuf,
    source: &str,
    order: QueryOrder,
) -> Result<QueryOutput, QueryError> {
    let expression =
        parse_query_expression(source).map_err(|error| QueryError::syntax(error.to_string()))?;
    validate_loading_query(&expression).map_err(|error| QueryError::syntax(error.to_string()))?;
    let targets = QueryEvaluator::new(LoadingQueryEnvironment::new(ctx, workspace))
        .evaluate(&expression)
        .await?;
    let mut labels = targets.iter().cloned().collect::<Vec<_>>();
    if order == QueryOrder::Auto {
        labels.sort_unstable();
    }
    Ok(QueryOutput {
        labels: labels
            .into_iter()
            .map(|label| CompactString::new(label.to_string()))
            .collect::<Vec<_>>()
            .into(),
    })
}
