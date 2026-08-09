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
use regex::Regex;
use regex::RegexBuilder;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BinaryOperator;
use crate::QueryExpression;
use crate::QueryExpressionKind;
use crate::QueryPolicy;
use crate::graph::QueryError;
use crate::loading_query_function;
use crate::traversal::reverse_dependencies;
use crate::traversal::some_path;
use crate::traversal::transitive_closure;

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

    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TestTargetKind {
    Test,
    Suite,
    Other,
}

#[derive(Debug, Clone)]
pub(crate) struct TestTargetInfo {
    pub(crate) label: CompactString,
    pub(crate) kind: TestTargetKind,
    pub(crate) tags: Arc<[CompactString]>,
    pub(crate) size: Option<CompactString>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TestSuiteAttribute {
    Tests,
    ImplicitTests,
}

impl TestSuiteAttribute {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Tests => "tests",
            Self::ImplicitTests => "$implicit_tests",
        }
    }
}

#[async_trait]
pub trait CqueryQueryEnvironment {
    type Set: Clone + Send + Sync;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set;
    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set;
    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set;
    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set;
    fn select_some(&self, targets: &Self::Set, count: i32) -> Result<Self::Set, QueryError>;
    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError>;
    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError>;
    async fn kind(&mut self, regex: &Regex, targets: &Self::Set) -> Result<Self::Set, QueryError>;
    async fn filter(&mut self, regex: &Regex, targets: &Self::Set)
    -> Result<Self::Set, QueryError>;
}

/// Evaluates the bounded configured-query subset with the shared expression
/// fold. Root resolution belongs to the caller and happens before this fold.
pub async fn evaluate_cquery_query<E>(
    environment: &mut E,
    expression: &QueryExpression,
) -> Result<E::Set, QueryError>
where
    E: CqueryQueryEnvironment + Send,
{
    let mut variables = SmallMap::new();
    let mut context = CqueryContext(environment);
    evaluate_query_expression_inner(&mut context, expression, &mut variables).await
}

struct CqueryContext<'a, E>(&'a mut E);

#[async_trait]
impl<E> CqueryQueryEnvironment for CqueryContext<'_, E>
where
    E: CqueryQueryEnvironment + Send,
{
    type Set = E::Set;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
        self.0.one_delivery(sets)
    }

    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
        self.0.union(left, right)
    }

    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        self.0.intersection(left, right)
    }

    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        self.0.except(left, right)
    }

    fn select_some(&self, targets: &Self::Set, count: i32) -> Result<Self::Set, QueryError> {
        self.0.select_some(targets, count)
    }

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
        self.0.resolve_literal(literal).await
    }

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        self.0.executables(targets).await
    }

    async fn kind(&mut self, regex: &Regex, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        self.0.kind(regex, targets).await
    }

    async fn filter(
        &mut self,
        regex: &Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        self.0.filter(regex, targets).await
    }
}

trait QueryExpressionContext: CqueryQueryEnvironment {
    fn evaluate_integer<'a>(
        &'a mut self,
        value: u64,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>>;

    fn evaluate_function<'a>(
        &'a mut self,
        name: &'a crate::Spanned<CompactString>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, Self::Set>,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>>;
}

impl<E> QueryExpressionContext for CqueryContext<'_, E>
where
    E: CqueryQueryEnvironment + Send,
{
    fn evaluate_integer<'a>(
        &'a mut self,
        _value: u64,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>> {
        async move { Err(QueryError::syntax("integer literals are not supported")) }.boxed()
    }

    fn evaluate_function<'a>(
        &'a mut self,
        name: &'a crate::Spanned<CompactString>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, Self::Set>,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>> {
        async move {
            match name.value.as_str() {
                "executables" => invoke_executables(self, args, variables).await,
                "kind" => invoke_kind(self, args, variables).await,
                "filter" => invoke_filter(self, args, variables).await,
                "some" => invoke_some(self, args, variables).await,
                _ => Err(QueryError::syntax("query functions are not supported")),
            }
        }
        .boxed()
    }
}

fn evaluate_query_expression_inner<'a, C>(
    context: &'a mut C,
    expression: &'a QueryExpression,
    variables: &'a mut SmallMap<CompactString, C::Set>,
) -> BoxFuture<'a, Result<C::Set, QueryError>>
where
    C: QueryExpressionContext + Send,
{
    async move {
        match &expression.kind {
            QueryExpressionKind::TargetLiteral(literal) => {
                if let Some(name) = literal.strip_prefix('$') {
                    return variables.get(name).cloned().ok_or_else(|| {
                        QueryError::evaluation(format!("undefined query variable '${name}'"))
                    });
                }
                context.resolve_literal(literal).await
            }
            QueryExpressionKind::Integer(value) => context.evaluate_integer(*value).await,
            QueryExpressionKind::Set(literals) => {
                let mut resolved = Vec::with_capacity(literals.len());
                for literal in literals.iter() {
                    resolved.push(context.resolve_literal(&literal.value).await?);
                }
                Ok(context.one_delivery(&resolved))
            }
            QueryExpressionKind::Let { name, value, body } => {
                let value = evaluate_query_expression_inner(context, value, variables).await?;
                let previous = variables.insert(name.value.clone(), value);
                let result = evaluate_query_expression_inner(context, body, variables).await;
                if let Some(previous) = previous {
                    variables.insert(name.value.clone(), previous);
                } else {
                    variables.shift_remove(name.value.as_str());
                }
                result
            }
            QueryExpressionKind::BinaryOpSequence { left, operations } => {
                let mut result = evaluate_query_expression_inner(context, left, variables).await?;
                for (operator, right) in operations.iter() {
                    let right = evaluate_query_expression_inner(context, right, variables).await?;
                    result = match operator {
                        BinaryOperator::Union => context.union(result, right),
                        BinaryOperator::Except => context.except(&result, &right),
                        BinaryOperator::Intersect => context.intersection(&result, &right),
                    };
                }
                Ok(result)
            }
            QueryExpressionKind::Function { name, args } => {
                context.evaluate_function(name, args, variables).await
            }
        }
    }
    .boxed()
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

    async fn attr(
        &mut self,
        attribute: &str,
        regex: &Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError>;

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError>;

    async fn filter(&mut self, regex: &Regex, targets: &Self::Set)
    -> Result<Self::Set, QueryError>;

    async fn kind(&mut self, regex: &Regex, targets: &Self::Set) -> Result<Self::Set, QueryError>;

    async fn visible(
        &mut self,
        callers: &TargetSet<Self::Target>,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError>;

    fn query_policy(&self) -> QueryPolicy;

    async fn test_target_info(
        &mut self,
        target: &Self::Target,
    ) -> Result<TestTargetInfo, QueryError>;

    async fn test_suite_members(
        &mut self,
        suite: &Self::Target,
        attribute: TestSuiteAttribute,
    ) -> Result<Arc<[Self::Target]>, QueryError>;
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
        evaluate_query_expression_inner(self, expression, variables)
    }
}

#[async_trait]
impl<E> CqueryQueryEnvironment for QueryEvaluator<E>
where
    E: QueryEnvironment + Send,
{
    type Set = E::Set;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
        self.environment.one_delivery(sets)
    }

    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
        self.environment.union(left, right)
    }

    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        self.environment.intersection(left, right)
    }

    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        self.environment.except(left, right)
    }

    fn select_some(&self, targets: &Self::Set, count: i32) -> Result<Self::Set, QueryError> {
        let candidates = self.environment.eval_all(targets);
        let mut selected = TargetSet::default();
        if count > 0 {
            for candidate in candidates.iter().take(count as usize) {
                selected.insert(candidate.clone());
            }
        }
        if selected.iter().next().is_none() {
            Err(QueryError::evaluation("argument set is empty"))
        } else {
            Ok(self.environment.lift_one_delivery(selected))
        }
    }

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
        self.environment.resolve_literal(literal).await
    }

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        self.environment.executables(targets).await
    }

    async fn kind(&mut self, regex: &Regex, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        self.environment.kind(regex, targets).await
    }

    async fn filter(
        &mut self,
        regex: &Regex,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        self.environment.filter(regex, targets).await
    }
}

impl<E> QueryExpressionContext for QueryEvaluator<E>
where
    E: QueryEnvironment + Send,
{
    fn evaluate_integer<'a>(
        &'a mut self,
        value: u64,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>> {
        async move {
            self.environment
                .resolve_literal(&format!("//:{value}"))
                .await
        }
        .boxed()
    }

    fn evaluate_function<'a>(
        &'a mut self,
        name: &'a crate::Spanned<CompactString>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, Self::Set>,
    ) -> BoxFuture<'a, Result<Self::Set, QueryError>> {
        async move {
            let functions = self.functions;
            let function = functions.get(&name.value).ok_or_else(|| {
                QueryError::syntax(format!("query function '{}' was not validated", name.value))
            })?;
            function.invoke(self, args, variables).await
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
static ATTR_FUNCTION: AttrFunction = AttrFunction;
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
static FILTER_FUNCTION: RegexFunction = RegexFunction::Filter;
static KIND_FUNCTION: RegexFunction = RegexFunction::Kind;
static TESTS_FUNCTION: TestsFunction = TestsFunction;
static VISIBLE_FUNCTION: VisibleFunction = VisibleFunction;

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
            &ATTR_FUNCTION as &dyn QueryFunction<E>,
            &BUILDFILES_FUNCTION as &dyn QueryFunction<E>,
            &DEPS_FUNCTION as &dyn QueryFunction<E>,
            &EXECUTABLES_FUNCTION as &dyn QueryFunction<E>,
            &FILTER_FUNCTION as &dyn QueryFunction<E>,
            &KIND_FUNCTION as &dyn QueryFunction<E>,
            &LABELS_FUNCTION as &dyn QueryFunction<E>,
            &LOADFILES_FUNCTION as &dyn QueryFunction<E>,
            &RDEPS_FUNCTION as &dyn QueryFunction<E>,
            &SAME_PKG_DIRECT_RDEPS_FUNCTION as &dyn QueryFunction<E>,
            &SIBLINGS_FUNCTION as &dyn QueryFunction<E>,
            &SOME_FUNCTION as &dyn QueryFunction<E>,
            &SOMEPATH_FUNCTION as &dyn QueryFunction<E>,
            &TESTS_FUNCTION as &dyn QueryFunction<E>,
            &VISIBLE_FUNCTION as &dyn QueryFunction<E>,
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

impl QuerySelectionCount {
    fn from_expression(expression: &QueryExpression) -> Result<Self, QueryError> {
        expression
            .java_integer_literal()
            .map(Self)
            .map_err(|raw| QueryError::syntax(format!("expected an integer literal: '{raw}'")))
    }
}

impl<E: QueryEnvironment> QueryFunctionArg<E> for QuerySelectionCount {
    fn accept_none() -> Option<Self> {
        Some(Self(1))
    }

    fn eval<'a>(
        _evaluator: &'a mut QueryEvaluator<E>,
        expression: &'a QueryExpression,
        _variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<Self, QueryError>> {
        async move { Self::from_expression(expression) }.boxed()
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
        invoke_some(evaluator, args, variables)
    }
}

/// Shared `some()` invocation for loading and configured query. The caller's
/// environment defines the ordered, distinct selection domain.
fn invoke_some<'a, C>(
    context: &'a mut C,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, C::Set>,
) -> BoxFuture<'a, Result<C::Set, QueryError>>
where
    C: QueryExpressionContext + Send,
{
    async move {
        let operand = args
            .first()
            .ok_or_else(|| QueryError::syntax("missing query function argument"))?;
        let count = match args.get(1) {
            Some(expression) => QuerySelectionCount::from_expression(expression)?,
            None => QuerySelectionCount(1),
        };
        let targets = evaluate_query_expression_inner(context, operand, variables).await?;
        context.select_some(&targets, count.0)
    }
    .boxed()
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

struct AttrFunction;

struct LabelsFunction;

struct ExecutablesFunction;

#[derive(Clone, Copy)]
enum RegexFunction {
    Filter,
    Kind,
}

const SLUG_REGEX_PATTERN_LIMIT: usize = 4_096;
const SLUG_REGEX_PROGRAM_LIMIT: usize = 1_048_576;
const SLUG_REGEX_NEST_LIMIT: u32 = 128;

fn compile_slug_regex(pattern: &str) -> Result<Regex, QueryError> {
    if pattern.len() > SLUG_REGEX_PATTERN_LIMIT {
        return Err(QueryError::syntax(
            "Slug regex resource limit exceeded: pattern is longer than 4096 bytes",
        ));
    }

    let mut builder = RegexBuilder::new(pattern);
    builder
        .size_limit(SLUG_REGEX_PROGRAM_LIMIT)
        .dfa_size_limit(SLUG_REGEX_PROGRAM_LIMIT)
        .nest_limit(SLUG_REGEX_NEST_LIMIT)
        .case_insensitive(false)
        .multi_line(false)
        .dot_matches_new_line(false)
        .crlf(false)
        .line_terminator(b'\n')
        .swap_greed(false)
        .ignore_whitespace(false)
        .unicode(true)
        .octal(false);
    builder.build().map_err(|error| match error {
        regex::Error::CompiledTooBig(_) => QueryError::syntax(
            "Slug regex resource limit exceeded: compiled program is larger than 1048576 bytes",
        ),
        _ => QueryError::syntax("invalid Slug regex: unsupported or malformed syntax"),
    })
}

/// The only configured-query function is also a loading-query function. Keep
/// its compilation-before-operand order in one place so configured query uses
/// the established Rust-native regex contract without a second evaluator.
fn invoke_filter<'a, C>(
    context: &'a mut C,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, C::Set>,
) -> BoxFuture<'a, Result<C::Set, QueryError>>
where
    C: QueryExpressionContext + Send,
{
    async move {
        let pattern = match args.first().map(|argument| &argument.kind) {
            Some(QueryExpressionKind::TargetLiteral(value)) => value.as_str(),
            Some(_) => return Err(QueryError::syntax("regex pattern must be a word")),
            None => return Err(QueryError::syntax("missing query function argument")),
        };
        let operand = args
            .get(1)
            .ok_or_else(|| QueryError::syntax("missing query function argument"))?;
        // This is deliberately after cquery's eager root universe has
        // completed, but before the already-resolved operand is folded.
        let regex = compile_slug_regex(pattern)?;
        let targets = evaluate_query_expression_inner(context, operand, variables).await?;
        context.filter(&regex, &targets).await
    }
    .boxed()
}

impl<E> QueryFunction<E> for AttrFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("attr").expect("attr is in the static Bazel registry")
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
                _ => return Err(QueryError::syntax("attr attribute must be a word")),
            };
            let pattern = match &args[1].kind {
                QueryExpressionKind::TargetLiteral(value) => value.as_str(),
                _ => return Err(QueryError::syntax("attr regex pattern must be a word")),
            };
            // Match the active Rust-native regex functions' error precedence:
            // compile once before evaluating the operand.
            let regex = compile_slug_regex(pattern)?;
            let targets = eval_set_arg(evaluator, args, variables, 2).await?;
            evaluator
                .environment
                .attr(attribute, &regex, &targets)
                .await
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for RegexFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function(match self {
            Self::Filter => "filter",
            Self::Kind => "kind",
        })
        .expect("regex function is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            if matches!(self, Self::Filter) {
                return invoke_filter(evaluator, args, variables).await;
            }
            invoke_kind(evaluator, args, variables).await
        }
        .boxed()
    }
}

struct TestsFunction;

struct VisibleFunction;

impl<E> QueryFunction<E> for VisibleFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("visible").expect("visible is in the static Bazel registry")
    }

    fn invoke<'a>(
        &'a self,
        evaluator: &'a mut QueryEvaluator<E>,
        args: &'a [QueryExpression],
        variables: &'a mut SmallMap<CompactString, E::Set>,
    ) -> BoxFuture<'a, Result<E::Set, QueryError>> {
        async move {
            // QueryUtil.evalAll materializes the predicate by its printed-label
            // key. The streamed input deliberately remains unmaterialized: a
            // later fake candidate with the same label can still pass.
            let callers = eval_set_arg(evaluator, args, variables, 0).await?;
            let callers = evaluator.environment.eval_all(&callers);
            let targets = eval_set_arg(evaluator, args, variables, 1).await?;
            evaluator.environment.visible(&callers, &targets).await
        }
        .boxed()
    }
}

impl<E> QueryFunction<E> for TestsFunction
where
    E: QueryEnvironment + Send,
{
    fn spec(&self) -> &'static crate::QueryFunctionSpec {
        loading_query_function("tests").expect("tests is in the static Bazel registry")
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
            let strict = evaluator.environment.query_policy().strict_test_suite;
            let mut unique_tests = SmallSet::new();
            let mut unique_suites = SmallSet::new();
            let mut pending_suites = Vec::new();

            for target in targets.iter() {
                let info = evaluator.environment.test_target_info(target).await?;
                match info.kind {
                    TestTargetKind::Test => {
                        unique_tests.insert(target.clone());
                    }
                    TestTargetKind::Suite => {
                        if unique_suites.insert(target.clone()) {
                            pending_suites.push((target.clone(), info));
                        }
                    }
                    TestTargetKind::Other => {}
                }
            }

            while let Some((suite, suite_info)) = pending_suites.pop() {
                let (required_tags, excluded_tags) = split_test_tags(&suite_info.tags);
                let explicit = suite_members_with_prefix(
                    &mut evaluator.environment,
                    &suite,
                    &suite_info.label,
                    TestSuiteAttribute::Tests,
                )
                .await?;
                for member in explicit.iter() {
                    let info = evaluator.environment.test_target_info(member).await?;
                    match info.kind {
                        TestTargetKind::Test => {
                            if include_test(&info, &required_tags, &excluded_tags) {
                                unique_tests.insert(member.clone());
                            }
                        }
                        TestTargetKind::Suite => {
                            if unique_suites.insert(member.clone()) {
                                pending_suites.push((member.clone(), info));
                            }
                        }
                        TestTargetKind::Other if strict => {
                            return Err(QueryError::evaluation(format!(
                                "The label '{}' in the test_suite '{}' does not refer to a test or test_suite rule!",
                                info.label, suite_info.label
                            )));
                        }
                        TestTargetKind::Other => {}
                    }
                }

                let implicit = suite_members_with_prefix(
                    &mut evaluator.environment,
                    &suite,
                    &suite_info.label,
                    TestSuiteAttribute::ImplicitTests,
                )
                .await?;
                for member in implicit.iter() {
                    let info = evaluator.environment.test_target_info(member).await?;
                    if info.kind == TestTargetKind::Test
                        && include_test(&info, &required_tags, &excluded_tags)
                    {
                        unique_tests.insert(member.clone());
                    }
                }
            }

            Ok(evaluator
                .environment
                .lift_one_delivery(TargetSet(unique_tests)))
        }
        .boxed()
    }
}

async fn suite_members_with_prefix<E>(
    environment: &mut E,
    suite: &E::Target,
    suite_label: &str,
    attribute: TestSuiteAttribute,
) -> Result<Arc<[E::Target]>, QueryError>
where
    E: QueryEnvironment + Send,
{
    environment
        .test_suite_members(suite, attribute)
        .await
        .map_err(|error| {
            if error.is_preparation_restart() {
                return error;
            }
            let message = format!(
                "couldn't expand '{}' attribute of test_suite {}: {error}",
                attribute.name(),
                suite_label
            );
            error.with_message(message)
        })
}

fn split_test_tags(tags: &[CompactString]) -> (SmallSet<CompactString>, SmallSet<CompactString>) {
    let mut required = SmallSet::new();
    let mut excluded = SmallSet::new();
    for tag in tags {
        if let Some(tag) = tag.strip_prefix('-') {
            excluded.insert(CompactString::new(tag));
        } else if let Some(tag) = tag.strip_prefix('+') {
            required.insert(CompactString::new(tag));
        } else if tag != "manual" {
            required.insert(tag.clone());
        }
    }
    (required, excluded)
}

fn include_test(
    test: &TestTargetInfo,
    required: &SmallSet<CompactString>,
    excluded: &SmallSet<CompactString>,
) -> bool {
    let has_tag = |tag: &CompactString| {
        test.tags.iter().any(|candidate| candidate == tag) || test.size.as_ref() == Some(tag)
    };
    excluded.iter().all(|tag| !has_tag(tag)) && required.iter().all(has_tag)
}

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
        invoke_executables(evaluator, args, variables)
    }
}

fn invoke_executables<'a, C>(
    context: &'a mut C,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, C::Set>,
) -> BoxFuture<'a, Result<C::Set, QueryError>>
where
    C: QueryExpressionContext + Send,
{
    async move {
        // Match Bazel's ExecutablesFunction: evaluate the sole operand once,
        // then filter every callback delivery in place.
        let operand = args
            .first()
            .ok_or_else(|| QueryError::syntax("missing query function argument"))?;
        let targets = evaluate_query_expression_inner(context, operand, variables).await?;
        context.executables(&targets).await
    }
    .boxed()
}

fn invoke_kind<'a, C>(
    context: &'a mut C,
    args: &'a [QueryExpression],
    variables: &'a mut SmallMap<CompactString, C::Set>,
) -> BoxFuture<'a, Result<C::Set, QueryError>>
where
    C: QueryExpressionContext + Send,
{
    async move {
        let pattern = match args.first().map(|argument| &argument.kind) {
            Some(QueryExpressionKind::TargetLiteral(value)) => value.as_str(),
            Some(_) => return Err(QueryError::syntax("regex pattern must be a word")),
            None => return Err(QueryError::syntax("missing query function argument")),
        };
        let operand = args
            .get(1)
            .ok_or_else(|| QueryError::syntax("missing query function argument"))?;
        // Like filter(), cquery compiles only after eager roots are prepared,
        // but before evaluating the recursive operand.
        let regex = compile_slug_regex(pattern)?;
        let targets = evaluate_query_expression_inner(context, operand, variables).await?;
        context.kind(&regex, &targets).await
    }
    .boxed()
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

#[cfg(test)]
mod tests {
    use super::*;

    struct CqueryEnvironment {
        events: Vec<String>,
    }

    #[async_trait]
    impl CqueryQueryEnvironment for CqueryEnvironment {
        type Set = Vec<String>;

        fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
            let mut result = Vec::new();
            for set in sets {
                for value in set {
                    if !result.contains(value) {
                        result.push(value.clone());
                    }
                }
            }
            result
        }

        fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
            self.one_delivery(&[left, right])
        }

        fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
            left.iter()
                .filter(|value| right.contains(value))
                .cloned()
                .collect()
        }

        fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
            left.iter()
                .filter(|value| !right.contains(value))
                .cloned()
                .collect()
        }

        fn select_some(&self, targets: &Self::Set, count: i32) -> Result<Self::Set, QueryError> {
            let selected = if count > 0 {
                targets.iter().take(count as usize).cloned().collect()
            } else {
                Vec::new()
            };
            if selected.is_empty() {
                Err(QueryError::evaluation("argument set is empty"))
            } else {
                Ok(selected)
            }
        }

        async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
            self.events.push(format!("resolve:{literal}"));
            Ok(vec![literal.to_owned()])
        }

        async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
            self.events
                .push(format!("executables:{}", targets.join(",")));
            Ok(targets
                .iter()
                .filter(|target| target.ends_with(":bin"))
                .cloned()
                .collect())
        }

        async fn kind(
            &mut self,
            regex: &Regex,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.events.push(format!("kind:{}", targets.join(",")));
            Ok(targets
                .iter()
                .filter(|target| regex.find(target).is_some())
                .cloned()
                .collect())
        }

        async fn filter(
            &mut self,
            regex: &Regex,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.events.push(format!("filter:{}", targets.join(",")));
            Ok(targets
                .iter()
                .filter(|target| regex.find(target).is_some())
                .cloned()
                .collect())
        }
    }

    #[test]
    fn cquery_filter_reuses_the_shared_fold_in_operand_order() {
        let expression =
            QueryExpression::parse("filter('^//pkg:bin$', set(//pkg:lib //pkg:bin //pkg:lib))")
                .unwrap();
        let mut environment = CqueryEnvironment { events: Vec::new() };
        let result =
            futures::executor::block_on(evaluate_cquery_query(&mut environment, &expression))
                .unwrap();
        assert_eq!(result, ["//pkg:bin"]);
        assert_eq!(
            environment.events,
            [
                "resolve://pkg:lib",
                "resolve://pkg:bin",
                "resolve://pkg:lib",
                "filter://pkg:lib,//pkg:bin",
            ]
        );
    }

    #[test]
    fn cquery_executables_reuses_the_shared_recursive_fold() {
        let expression = QueryExpression::parse(
            "executables(some(filter('^//pkg:', set(//pkg:lib //pkg:bin //pkg:bin)), 2))",
        )
        .unwrap();
        let mut environment = CqueryEnvironment { events: Vec::new() };
        let result =
            futures::executor::block_on(evaluate_cquery_query(&mut environment, &expression))
                .unwrap();
        assert_eq!(result, ["//pkg:bin"]);
        assert_eq!(
            environment.events,
            [
                "resolve://pkg:lib",
                "resolve://pkg:bin",
                "resolve://pkg:bin",
                "filter://pkg:lib,//pkg:bin",
                "executables://pkg:lib,//pkg:bin",
            ]
        );
    }

    #[test]
    fn cquery_kind_reuses_the_shared_recursive_fold() {
        let expression = QueryExpression::parse(
            "kind('bin$', executables(some(filter('^//pkg:', set(//pkg:lib //pkg:bin //pkg:bin)), 2)))",
        )
        .unwrap();
        let mut environment = CqueryEnvironment { events: Vec::new() };
        let result =
            futures::executor::block_on(evaluate_cquery_query(&mut environment, &expression))
                .unwrap();
        assert_eq!(result, ["//pkg:bin"]);
        assert_eq!(
            environment.events,
            [
                "resolve://pkg:lib",
                "resolve://pkg:bin",
                "resolve://pkg:bin",
                "filter://pkg:lib,//pkg:bin",
                "executables://pkg:lib,//pkg:bin",
                "kind://pkg:bin",
            ]
        );
    }

    #[test]
    fn cquery_some_reuses_the_shared_count_and_recursive_fold() {
        let expression = QueryExpression::parse(
            "some(filter('^//pkg:', set(//pkg:lib //pkg:bin //pkg:lib)), 2)",
        )
        .unwrap();
        let mut environment = CqueryEnvironment { events: Vec::new() };
        let result =
            futures::executor::block_on(evaluate_cquery_query(&mut environment, &expression))
                .unwrap();
        assert_eq!(result, ["//pkg:lib", "//pkg:bin"]);
        assert_eq!(
            environment.events,
            [
                "resolve://pkg:lib",
                "resolve://pkg:bin",
                "resolve://pkg:lib",
                "filter://pkg:lib,//pkg:bin",
            ]
        );
    }

    struct VisibleEnvironment {
        events: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl QueryEnvironment for VisibleEnvironment {
        type Target = String;
        type Set = Vec<String>;

        fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
            sets.iter().flatten().cloned().collect()
        }

        fn union(&self, mut left: Self::Set, right: Self::Set) -> Self::Set {
            left.extend(right);
            left
        }

        fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
            left.iter()
                .filter(|value| right.contains(value))
                .cloned()
                .collect()
        }

        fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
            left.iter()
                .filter(|value| !right.contains(value))
                .cloned()
                .collect()
        }

        fn eval_all(&self, set: &Self::Set) -> TargetSet<Self::Target> {
            self.events.lock().unwrap().push("eval_all".to_owned());
            let mut result = TargetSet::default();
            for value in set {
                result.insert(value.clone());
            }
            result
        }

        fn lift_one_delivery(&self, targets: TargetSet<Self::Target>) -> Self::Set {
            targets.iter().cloned().collect()
        }

        async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
            self.events
                .lock()
                .unwrap()
                .push(format!("resolve:{literal}"));
            Ok(vec![literal.to_owned()])
        }

        async fn dependencies(
            &mut self,
            _: &Self::Target,
        ) -> Result<Arc<[Self::Target]>, QueryError> {
            unreachable!()
        }

        async fn same_pkg_direct_rdeps(
            &mut self,
            _: &TargetSet<Self::Target>,
        ) -> Result<TargetSet<Self::Target>, QueryError> {
            unreachable!()
        }

        async fn siblings(&mut self, _: &Self::Set) -> Result<Self::Set, QueryError> {
            unreachable!()
        }

        async fn loading_files(&mut self, _: &Self::Set, _: bool) -> Result<Self::Set, QueryError> {
            unreachable!()
        }

        async fn labels(&mut self, _: &str, _: &Self::Set) -> Result<Self::Set, QueryError> {
            unreachable!()
        }

        async fn attr(
            &mut self,
            attribute: &str,
            regex: &Regex,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.events
                .lock()
                .unwrap()
                .push(format!("attr:{attribute}"));
            Ok(targets
                .iter()
                .filter(|target| regex.find(target).is_some())
                .cloned()
                .collect())
        }

        async fn executables(&mut self, _: &Self::Set) -> Result<Self::Set, QueryError> {
            unreachable!()
        }

        async fn filter(
            &mut self,
            regex: &Regex,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.events.lock().unwrap().push(format!(
                "filter:{}",
                targets
                    .iter()
                    .filter(|target| regex.find(target).is_some())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            Ok(targets
                .iter()
                .filter(|target| regex.find(target).is_some())
                .cloned()
                .collect())
        }

        async fn kind(
            &mut self,
            regex: &Regex,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.filter(regex, targets).await
        }

        async fn visible(
            &mut self,
            callers: &TargetSet<Self::Target>,
            targets: &Self::Set,
        ) -> Result<Self::Set, QueryError> {
            self.events.lock().unwrap().push(format!(
                "visible:{}:{:?}",
                callers.iter().cloned().collect::<Vec<_>>().join(","),
                targets
            ));
            Ok(targets.clone())
        }

        fn query_policy(&self) -> QueryPolicy {
            QueryPolicy::default()
        }

        async fn test_target_info(
            &mut self,
            _: &Self::Target,
        ) -> Result<TestTargetInfo, QueryError> {
            unreachable!()
        }

        async fn test_suite_members(
            &mut self,
            _: &Self::Target,
            _: TestSuiteAttribute,
        ) -> Result<Arc<[Self::Target]>, QueryError> {
            unreachable!()
        }
    }

    #[test]
    fn visible_materializes_only_the_once_evaluated_predicate_before_streaming_input() {
        let expression = QueryExpression::parse("visible(predicate, input)").unwrap();
        let mut evaluator = QueryEvaluator::new(VisibleEnvironment {
            events: std::sync::Mutex::new(Vec::new()),
        });
        let result = futures::executor::block_on(evaluator.evaluate(&expression)).unwrap();
        assert_eq!(result, ["input"]);
        assert_eq!(
            *evaluator.environment.events.lock().unwrap(),
            [
                "resolve:predicate",
                "eval_all",
                "resolve:input",
                "visible:predicate:[\"input\"]",
            ]
        );
    }

    #[test]
    fn regex_functions_compile_before_evaluating_the_operand_and_reuse_search_semantics() {
        for pattern in ["(?=unsupported)", r"\1"] {
            let expression =
                QueryExpression::parse(&format!("filter('{pattern}', input)")).unwrap();
            let mut evaluator = QueryEvaluator::new(VisibleEnvironment {
                events: std::sync::Mutex::new(Vec::new()),
            });
            let error = futures::executor::block_on(evaluator.evaluate(&expression)).unwrap_err();
            assert_eq!(
                error.to_string(),
                "invalid Slug regex: unsupported or malformed syntax"
            );
            assert_eq!(error.exit_code, 2);
            assert!(evaluator.environment.events.lock().unwrap().is_empty());
        }

        let expression = QueryExpression::parse("filter('(?i)input', input)").unwrap();
        let mut evaluator = QueryEvaluator::new(VisibleEnvironment {
            events: std::sync::Mutex::new(Vec::new()),
        });
        let result = futures::executor::block_on(evaluator.evaluate(&expression)).unwrap();
        assert_eq!(result, ["input"]);
        assert_eq!(
            *evaluator.environment.events.lock().unwrap(),
            ["resolve:input", "filter:input"]
        );

        let unicode = compile_slug_regex(r"(?i)^\p{Greek}+$").unwrap();
        assert!(unicode.find("Συν").is_some());
        assert!(unicode.find("latin").is_none());
        assert!(unicode.find("συν").is_some());
    }

    #[test]
    fn active_attr_function_compiles_before_operand_and_preserves_argument_order() {
        let functions = LoadingQueryFunctions;
        let active: Option<&dyn QueryFunction<VisibleEnvironment>> = functions.get("attr");
        assert!(active.is_some());
        assert_eq!(
            loading_query_function("attr").unwrap().status,
            crate::QueryFunctionStatus::Implemented
        );

        for pattern in ["(?=unsupported)", r"\1"] {
            let expression =
                QueryExpression::parse(&format!("attr(name, '{pattern}', missing)")).unwrap();
            let QueryExpressionKind::Function { args, .. } = expression.kind else {
                panic!("expected attr call");
            };
            let mut evaluator = QueryEvaluator::new(VisibleEnvironment {
                events: std::sync::Mutex::new(Vec::new()),
            });
            let error = futures::executor::block_on(ATTR_FUNCTION.invoke(
                &mut evaluator,
                &args,
                &mut SmallMap::new(),
            ))
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                "invalid Slug regex: unsupported or malformed syntax"
            );
            assert!(evaluator.environment.events.lock().unwrap().is_empty());
        }

        let expression = QueryExpression::parse("attr(name, put, input)").unwrap();
        let mut evaluator = QueryEvaluator::new(VisibleEnvironment {
            events: std::sync::Mutex::new(Vec::new()),
        });
        let result = futures::executor::block_on(evaluator.evaluate(&expression)).unwrap();
        assert_eq!(result, ["input"]);
        assert_eq!(
            *evaluator.environment.events.lock().unwrap(),
            ["resolve:input", "attr:name"]
        );
    }

    #[test]
    fn slug_regex_limits_and_error_classes_are_stable() {
        let exact = "a".repeat(SLUG_REGEX_PATTERN_LIMIT);
        assert!(compile_slug_regex(&exact).is_ok());
        let oversized = "a".repeat(SLUG_REGEX_PATTERN_LIMIT + 1);
        assert_eq!(
            compile_slug_regex(&oversized).unwrap_err().to_string(),
            "Slug regex resource limit exceeded: pattern is longer than 4096 bytes"
        );
        let nested = format!(
            "{}a{}",
            "(".repeat(SLUG_REGEX_NEST_LIMIT as usize + 1),
            ")".repeat(SLUG_REGEX_NEST_LIMIT as usize + 1)
        );
        assert_eq!(
            compile_slug_regex(&nested).unwrap_err().to_string(),
            "invalid Slug regex: unsupported or malformed syntax"
        );
        assert_eq!(
            compile_slug_regex("(?:a|b){100000}")
                .unwrap_err()
                .to_string(),
            "Slug regex resource limit exceeded: compiled program is larger than 1048576 bytes"
        );
    }
}
