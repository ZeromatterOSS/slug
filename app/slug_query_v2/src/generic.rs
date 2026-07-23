/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Generic query expression evaluation and function dispatch.

use std::hash::Hash;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use futures::FutureExt;
use futures::future::BoxFuture;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BinaryOperator;
use crate::QueryExpression;
use crate::QueryExpressionKind;
use crate::graph::QueryError;
use crate::loading_query_function;
use crate::traversal::reverse_dependencies;
use crate::traversal::some_path;
use crate::traversal::transitive_closure;

#[derive(Debug, Clone, Allocative)]
pub(crate) struct TargetSet<T>(SmallSet<T>);

impl<T> Default for TargetSet<T> {
    fn default() -> Self {
        Self(SmallSet::new())
    }
}

impl<T> TargetSet<T>
where
    T: Clone + Eq + Hash,
{
    pub(crate) fn singleton(value: T) -> Self {
        let mut values = SmallSet::new();
        values.insert(value);
        Self(values)
    }

    pub(crate) fn insert(&mut self, value: T) {
        self.0.insert(value);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    pub(crate) fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
}

#[async_trait]
pub(crate) trait QueryEnvironment {
    type Target: Clone + Eq + Hash + Send + Sync;
    type Set: Clone + Send + Sync;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set;
    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set;
    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set;
    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set;
    fn eval_all(&self, set: &Self::Set) -> TargetSet<Self::Target>;
    fn lift_one_delivery(&self, targets: TargetSet<Self::Target>) -> Self::Set;

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError>;

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError>;

    async fn same_pkg_direct_rdeps(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError>;

    async fn siblings(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError>;

    async fn loading_files(
        &mut self,
        targets: &Self::Set,
        include_buildfiles: bool,
    ) -> Result<Self::Set, QueryError>;

    async fn labels(
        &mut self,
        attribute: &str,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError>;

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError>;
}

pub(crate) struct QueryEvaluator<E> {
    pub(crate) environment: E,
    functions: LoadingQueryFunctions,
}

impl<E> QueryEvaluator<E>
where
    E: QueryEnvironment + Send,
{
    pub(crate) fn new(environment: E) -> Self {
        Self {
            environment,
            functions: LoadingQueryFunctions,
        }
    }

    pub(crate) async fn evaluate(
        &mut self,
        expression: &QueryExpression,
    ) -> Result<E::Set, QueryError> {
        let mut variables = SmallMap::new();
        self.evaluate_inner(expression, &mut variables).await
    }

    fn evaluate_inner<'a>(
        &'a mut self,
        expression: &'a QueryExpression,
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
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
                QueryExpressionKind::Integer(value) => {
                    self.environment
                        .resolve_literal(&format!("//:{value}"))
                        .await
                }
                QueryExpressionKind::Set(literals) => {
                    let mut resolved = Vec::with_capacity(literals.len());
                    for literal in literals.iter() {
                        resolved.push(self.environment.resolve_literal(&literal.value).await?);
                    }
                    Ok(self.environment.one_delivery(&resolved))
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
                            BinaryOperator::Union => self.environment.union(result, right),
                            BinaryOperator::Except => self.environment.except(&result, &right),
                            BinaryOperator::Intersect => {
                                self.environment.intersection(&result, &right)
                            }
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
trait QueryFunctions<E: QueryEnvironment>: Send + Sync {
    fn get(&self, name: &str) -> Option<&dyn QueryFunction<E>>;
}

trait QueryFunction<E: QueryEnvironment>: Send + Sync {
    fn spec(&self) -> &'static crate::QueryFunctionSpec;

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>>;
}

#[derive(Debug, Clone, Copy)]
struct LoadingQueryFunctions;

static DEPS_FUNCTION: DepsFunction = DepsFunction;
static RDEPS_FUNCTION: RdepsFunction = RdepsFunction;
static SAME_PKG_DIRECT_RDEPS_FUNCTION: SamePkgDirectRdepsFunction = SamePkgDirectRdepsFunction;
static SIBLINGS_FUNCTION: SiblingsFunction = SiblingsFunction;
static ALLPATHS_FUNCTION: AllPathsFunction = AllPathsFunction;
static SOME_FUNCTION: SomeFunction = SomeFunction;
static SOMEPATH_FUNCTION: SomePathFunction = SomePathFunction;
static BUILDFILES_FUNCTION: LoadingFilesFunction = LoadingFilesFunction {
    include_buildfiles: true,
};
static LOADFILES_FUNCTION: LoadingFilesFunction = LoadingFilesFunction {
    include_buildfiles: false,
};
static LABELS_FUNCTION: LabelsFunction = LabelsFunction;
static EXECUTABLES_FUNCTION: ExecutablesFunction = ExecutablesFunction;

impl<E> QueryFunctions<E> for LoadingQueryFunctions
where
    E: QueryEnvironment + Send,
{
    fn get(&self, name: &str) -> Option<&dyn QueryFunction<E>> {
        let spec = loading_query_function(name)?;
        if spec.status != crate::QueryFunctionStatus::Implemented {
            return None;
        }
        [
            &ALLPATHS_FUNCTION as &dyn QueryFunction<E>,
            &BUILDFILES_FUNCTION as &dyn QueryFunction<E>,
            &DEPS_FUNCTION as &dyn QueryFunction<E>,
            &EXECUTABLES_FUNCTION as &dyn QueryFunction<E>,
            &LABELS_FUNCTION as &dyn QueryFunction<E>,
            &LOADFILES_FUNCTION as &dyn QueryFunction<E>,
            &RDEPS_FUNCTION as &dyn QueryFunction<E>,
            &SAME_PKG_DIRECT_RDEPS_FUNCTION as &dyn QueryFunction<E>,
            &SIBLINGS_FUNCTION as &dyn QueryFunction<E>,
            &SOME_FUNCTION as &dyn QueryFunction<E>,
            &SOMEPATH_FUNCTION as &dyn QueryFunction<E>,
        ]
        .into_iter()
        .find(|function| std::ptr::eq(spec, function.spec()))
    }
}

trait QueryFunctionArg<E: QueryEnvironment>: Sized {
    fn accept_none() -> Option<Self> {
        None
    }

    fn eval<'a>(
        evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<Self, QueryError>>;
}

#[derive(Debug, Clone, Copy)]
struct QueryDepth(Option<i32>);

impl<E: QueryEnvironment> QueryFunctionArg<E> for QueryDepth {
    fn accept_none() -> Option<Self> {
        Some(Self(None))
    }

    fn eval<'a>(
        _evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        _variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<Self, QueryError>> {
        async move {
            expression
                .java_integer_literal()
                .map(|value| Self(Some(value)))
                .map_err(|raw| QueryError::syntax(format!("expected an integer literal: '{raw}'")))
        }
        .boxed()
    }
}

#[derive(Debug, Clone, Copy)]
struct QuerySelectionCount(i32);

impl<E: QueryEnvironment> QueryFunctionArg<E> for QuerySelectionCount {
    fn accept_none() -> Option<Self> {
        Some(Self(1))
    }

    fn eval<'a>(
        _evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        _variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<Self, QueryError>> {
        async move {
            expression
                .java_integer_literal()
                .map(Self)
                .map_err(|raw| QueryError::syntax(format!("expected an integer literal: '{raw}'")))
        }
        .boxed()
    }
}

fn eval_arg<'a, E, A>(
    evaluator: &'a mut QueryEvaluator<E>,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, E::Set>,
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

fn eval_set_arg<'a, E>(
    evaluator: &'a mut QueryEvaluator<E>,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, E::Set>,
    index: usize,
) -> BoxFuture<'a, Result<E::Set, QueryError>>
where
    E: QueryEnvironment + Send,
{
    match args.get(index) {
        Some(expression) => evaluator.evaluate_inner(expression, variables),
        None => async move { Err(QueryError::syntax("missing query function argument")) }.boxed(),
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
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let roots = eval_set_arg(evaluator, args, variables, 0).await?;
            let depth: QueryDepth = eval_arg(evaluator, args, variables, 1).await?;
            let roots = evaluator.environment.eval_all(&roots);
            let result = transitive_closure(&mut evaluator.environment, roots, depth.0).await?;
            Ok(evaluator.environment.lift_one_delivery(result))
        }
        .boxed()
    }
}

struct RdepsFunction;

impl<E> QueryFunction<E> for RdepsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("rdeps").expect("rdeps is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let universe = eval_set_arg(evaluator, args, variables, 0).await?;
            let from = eval_set_arg(evaluator, args, variables, 1).await?;
            let depth: QueryDepth = eval_arg(evaluator, args, variables, 2).await?;
            let universe = evaluator.environment.eval_all(&universe);
            let from = evaluator.environment.eval_all(&from);
            let result =
                reverse_dependencies(&mut evaluator.environment, universe, from, depth.0).await?;
            Ok(evaluator.environment.lift_one_delivery(result))
        }
        .boxed()
    }
}

struct AllPathsFunction;

struct SomeFunction;

struct SiblingsFunction;

impl<E> QueryFunction<E> for SiblingsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("siblings").expect("siblings is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let targets = eval_set_arg(evaluator, args, variables, 0).await?;
            evaluator.environment.siblings(&targets).await
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for SomeFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("some").expect("some is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let candidates = eval_set_arg(evaluator, args, variables, 0).await?;
            let count: QuerySelectionCount = eval_arg(evaluator, args, variables, 1).await?;
            let candidates = evaluator.environment.eval_all(&candidates);
            let mut selected = TargetSet::default();
            if count.0 > 0 {
                for candidate in candidates.iter().take(count.0 as usize) {
                    selected.insert(candidate.clone());
                }
            }
            if selected.iter().next().is_none() {
                Err(QueryError::evaluation("argument set is empty"))
            } else {
                Ok(evaluator.environment.lift_one_delivery(selected))
            }
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for AllPathsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("allpaths").expect("allpaths is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let from = eval_set_arg(evaluator, args, variables, 0).await?;
            let to = eval_set_arg(evaluator, args, variables, 1).await?;
            let from = evaluator.environment.eval_all(&from);
            let to = evaluator.environment.eval_all(&to);
            let result = reverse_dependencies(&mut evaluator.environment, from, to, None).await?;
            Ok(evaluator.environment.lift_one_delivery(result))
        }
        .boxed()
    }
}

struct SomePathFunction;

impl<E> QueryFunction<E> for SomePathFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("somepath").expect("somepath is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let from = eval_set_arg(evaluator, args, variables, 0).await?;
            let to = eval_set_arg(evaluator, args, variables, 1).await?;
            let from = evaluator.environment.eval_all(&from);
            let to = evaluator.environment.eval_all(&to);
            let result = some_path(&mut evaluator.environment, from, to).await?;
            Ok(evaluator.environment.lift_one_delivery(result))
        }
        .boxed()
    }
}

struct SamePkgDirectRdepsFunction;

impl<E> QueryFunction<E> for SamePkgDirectRdepsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("same_pkg_direct_rdeps")
            .expect("same_pkg_direct_rdeps is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let targets = eval_set_arg(evaluator, args, variables, 0).await?;
            let targets = evaluator.environment.eval_all(&targets);
            let result = evaluator
                .environment
                .same_pkg_direct_rdeps(&targets)
                .await?;
            Ok(evaluator.environment.lift_one_delivery(result))
        }
        .boxed()
    }
}

struct LoadingFilesFunction {
    include_buildfiles: bool,
}

struct LabelsFunction;

struct ExecutablesFunction;

impl<E> QueryFunction<E> for ExecutablesFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("executables").expect("executables is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            // Match Bazel's ExecutablesFunction: evaluate the sole operand
            // once, then filter every callback delivery in place.
            let targets = eval_set_arg(evaluator, args, variables, 0).await?;
            evaluator.environment.executables(&targets).await
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for LabelsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("labels").expect("labels is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let attribute = match &args[0].kind {
                QueryExpressionKind::TargetLiteral(value) => value.as_str(),
                _ => return Err(QueryError::syntax("labels attribute must be a word")),
            };
            let targets = eval_set_arg(evaluator, args, variables, 1).await?;
            evaluator.environment.labels(attribute, &targets).await
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for LoadingFilesFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function(if self.include_buildfiles {
            "buildfiles"
        } else {
            "loadfiles"
        })
        .expect("loading file function is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            let targets = eval_set_arg(evaluator, args, variables, 0).await?;
            evaluator
                .environment
                .loading_files(&targets, self.include_buildfiles)
                .await
        }
        .boxed()
    }
}
