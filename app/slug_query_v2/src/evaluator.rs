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
use crate::graph::SubtreePackageSetKey;
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

    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
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

    async fn same_pkg_direct_rdeps(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError>;

    async fn siblings(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError>;
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
        &mut self,
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
                QueryExpressionKind::Integer(value) => {
                    self.environment
                        .resolve_literal(&format!("//:{value}"))
                        .await
                }
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
static RDEPS_FUNCTION: RdepsFunction = RdepsFunction;
static SAME_PKG_DIRECT_RDEPS_FUNCTION: SamePkgDirectRdepsFunction = SamePkgDirectRdepsFunction;
static SIBLINGS_FUNCTION: SiblingsFunction = SiblingsFunction;
static ALLPATHS_FUNCTION: AllPathsFunction = AllPathsFunction;
static SOME_FUNCTION: SomeFunction = SomeFunction;
static SOMEPATH_FUNCTION: SomePathFunction = SomePathFunction;

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
            &DEPS_FUNCTION as &dyn QueryFunction<E>,
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
struct QueryDepth(Option<i32>);

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
        _variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let universe: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            let from: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 1).await?;
            let depth: QueryDepth = eval_arg(evaluator, args, variables, 2).await?;
            reverse_dependencies(&mut evaluator.environment, universe, from, depth.0).await
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let targets: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let candidates: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            let count: QuerySelectionCount = eval_arg(evaluator, args, variables, 1).await?;
            let mut selected = TargetSet::default();
            if count.0 > 0 {
                for candidate in candidates.iter().take(count.0 as usize) {
                    selected.insert(candidate.clone());
                }
            }
            if selected.iter().next().is_none() {
                Err(QueryError::evaluation("argument set is empty"))
            } else {
                Ok(selected)
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let from: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            let to: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 1).await?;
            reverse_dependencies(&mut evaluator.environment, from, to, None).await
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let from: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            let to: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 1).await?;
            some_path(&mut evaluator.environment, from, to).await
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
        variables: &'a mut SmallMap<CompactString, TargetSet<E::Target>>,
    ) -> BoxFuture<'a, Result<TargetSet<E::Target>, QueryError>> {
        async move {
            let targets: TargetSet<E::Target> = eval_arg(evaluator, args, variables, 0).await?;
            evaluator.environment.same_pkg_direct_rdeps(&targets).await
        }
        .boxed()
    }
}

async fn transitive_closure<E>(
    environment: &mut E,
    roots: TargetSet<E::Target>,
    max_depth: Option<i32>,
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

#[derive(Clone)]
struct ResolvedGraphNode<T> {
    target: T,
    children: Vec<u32>,
}

/// A request-local, integer-indexed graph adapted from Buck2
/// `query/graph/graph.rs::Graph`. DICE continues to own immutable package
/// nodes; this type owns only traversal state for one query evaluation.
struct ResolvedGraph<T> {
    nodes: Vec<ResolvedGraphNode<T>>,
    target_to_index: SmallMap<T, u32>,
}

impl<T> ResolvedGraph<T>
where
    T: Clone + Eq + Hash,
{
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            target_to_index: SmallMap::new(),
        }
    }

    fn record_node(&mut self, target: T) {
        self.get_or_create(target);
    }

    fn record_edge(&mut self, from: T, to: T) {
        let (from, _) = self.get_or_create(from);
        let (to, _) = self.get_or_create(to);
        let children = &mut self.nodes[from as usize].children;
        if !children.contains(&to) {
            children.push(to);
        }
    }

    fn selected_induced(&self, selected: &TargetSet<T>) -> Self
    where
        T: Ord,
    {
        let mut targets = selected.iter().cloned().collect::<Vec<_>>();
        targets.sort_unstable();
        let mut graph = Self::new();
        graph.nodes.reserve(targets.len());
        graph.target_to_index.reserve(targets.len());
        for target in targets {
            graph.get_or_create(target);
        }
        for index in 0..graph.nodes.len() {
            let target = graph.nodes[index].target.clone();
            let mut children = self
                .target_to_index
                .get(&target)
                .into_iter()
                .flat_map(|recorded| self.nodes[*recorded as usize].children.iter())
                .filter_map(|child| {
                    graph
                        .target_to_index
                        .get(&self.nodes[*child as usize].target)
                        .copied()
                })
                .collect::<Vec<_>>();
            children.sort_unstable_by(|left, right| {
                graph.nodes[*left as usize]
                    .target
                    .cmp(&graph.nodes[*right as usize].target)
            });
            children.dedup();
            graph.nodes[index].children = children;
        }
        graph
    }

    fn deterministic_topological_order(&self) -> Vec<T> {
        let mut visited = SmallSet::new();
        let mut postorder = Vec::with_capacity(self.nodes.len());
        for root in 0..self.nodes.len() {
            let root: u32 = root
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            if !visited.insert(root) {
                continue;
            }
            let mut stack = vec![(root, 0_usize)];
            while let Some((index, next_child)) = stack.last_mut() {
                if let Some(child) = self.nodes[*index as usize]
                    .children
                    .get(*next_child)
                    .copied()
                {
                    *next_child += 1;
                    if visited.insert(child) {
                        stack.push((child, 0));
                    }
                } else {
                    let (index, _) = stack.pop().expect("query DFS stack is non-empty");
                    postorder.push(self.nodes[index as usize].target.clone());
                }
            }
        }
        postorder.reverse();
        postorder
    }

    async fn build_stable_forward<E>(
        environment: &mut E,
        roots: impl IntoIterator<Item = T>,
    ) -> Result<Self, QueryError>
    where
        E: QueryEnvironment<Target = T> + Send,
    {
        let roots = roots.into_iter().collect::<Vec<_>>();
        let mut graph = Self {
            nodes: Vec::new(),
            target_to_index: SmallMap::new(),
        };
        let mut pending = VecDeque::new();
        for root in roots.iter().cloned() {
            let (index, created) = graph.get_or_create(root);
            if created {
                pending.push_back(index);
            }
        }

        while let Some(index) = pending.pop_front() {
            let target = graph.nodes[index as usize].target.clone();
            let dependencies = environment.dependencies(&target).await?;
            let mut children = Vec::with_capacity(dependencies.len());
            for dependency in dependencies.iter().cloned() {
                let (child, created) = graph.get_or_create(dependency);
                children.push(child);
                if created {
                    pending.push_back(child);
                }
            }
            children.shrink_to_fit();
            graph.nodes[index as usize].children = children;
        }

        Ok(graph.stable_dfs_order(roots))
    }

    fn get_or_create(&mut self, target: T) -> (u32, bool) {
        if let Some(index) = self.target_to_index.get(&target) {
            return (*index, false);
        }
        let index: u32 = self
            .nodes
            .len()
            .try_into()
            .expect("query graph exceeds u32 node capacity");
        self.target_to_index.insert(target.clone(), index);
        self.nodes.push(ResolvedGraphNode {
            target,
            children: Vec::new(),
        });
        (index, true)
    }

    fn stable_dfs_order(self, roots: Vec<T>) -> Self {
        let mut visited = SmallSet::new();
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = roots
            .iter()
            .rev()
            .filter_map(|root| self.target_to_index.get(root).copied())
            .collect::<Vec<_>>();
        while let Some(index) = stack.pop() {
            if !visited.insert(index) {
                continue;
            }
            order.push(index);
            for child in self.nodes[index as usize].children.iter().rev() {
                stack.push(*child);
            }
        }
        debug_assert_eq!(order.len(), self.nodes.len());
        self.remap(order)
    }

    fn remap(self, old_indices: Vec<u32>) -> Self {
        let mut old_to_new = vec![None; self.nodes.len()];
        for (new, old) in old_indices.iter().copied().enumerate() {
            old_to_new[old as usize] = Some(
                new.try_into()
                    .expect("query graph exceeds u32 node capacity"),
            );
        }

        let mut nodes = Vec::with_capacity(old_indices.len());
        let mut target_to_index = SmallMap::with_capacity(old_indices.len());
        for old in old_indices {
            let old_node = &self.nodes[old as usize];
            let new_index: u32 = nodes
                .len()
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            let target = old_node.target.clone();
            let children = old_node
                .children
                .iter()
                .filter_map(|child| old_to_new[*child as usize])
                .collect();
            target_to_index.insert(target.clone(), new_index);
            nodes.push(ResolvedGraphNode { target, children });
        }
        Self {
            nodes,
            target_to_index,
        }
    }

    fn reverse(mut self) -> Self {
        let mut reversed = (0..self.nodes.len())
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for (parent, node) in self.nodes.iter().enumerate() {
            for child in node.children.iter() {
                reversed[*child as usize].push(
                    parent
                        .try_into()
                        .expect("query graph exceeds u32 node capacity"),
                );
            }
        }
        for (node, children) in self.nodes.iter_mut().zip(reversed) {
            node.children = children;
        }
        self
    }

    fn take_max_depth(self, roots: &[T], max_depth: i32) -> Self {
        let mut visited = SmallSet::new();
        let mut retained = Vec::new();
        let mut edge = VecDeque::new();
        for root in roots {
            let Some(index) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if visited.insert(index) {
                retained.push(index);
                edge.push_back((index, 0_i32));
            }
        }

        while let Some((index, depth)) = edge.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for child in self.nodes[index as usize].children.iter().copied() {
                if visited.insert(child) {
                    retained.push(child);
                    edge.push_back((child, depth.saturating_add(1)));
                }
            }
        }
        if retained.len() == self.nodes.len() {
            self
        } else {
            self.remap(retained)
        }
    }

    fn contains(&self, target: &T) -> bool {
        self.target_to_index.get(target).is_some()
    }

    fn depth_first_postorder(&self, roots: &[T]) -> TargetSet<T> {
        let mut result = TargetSet::default();
        let mut visited = SmallSet::new();
        for root in roots {
            let Some(root) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if !visited.insert(root) {
                continue;
            }
            let mut stack = vec![(root, 0_usize)];
            while let Some((index, next_child)) = stack.last_mut() {
                let node = &self.nodes[*index as usize];
                if let Some(child) = node.children.get(*next_child).copied() {
                    *next_child += 1;
                    if visited.insert(child) {
                        stack.push((child, 0));
                    }
                } else {
                    let (index, _) = stack.pop().expect("query DFS stack is non-empty");
                    result.insert(self.nodes[index as usize].target.clone());
                }
            }
        }
        result
    }

    /// Compact integer-index BFS and parent reconstruction adapted from Buck2
    /// `query/graph/async_bfs.rs::async_bfs_find_path`. The forward graph is
    /// already resolved through the serial mutable-DICE transaction, so this
    /// V2 seam queues integer indices rather than lookup futures.
    fn shortest_path(&self, roots: &[T], destinations: &TargetSet<T>) -> TargetSet<T> {
        let mut visited = SmallSet::new();
        let mut parents = vec![None; self.nodes.len()];
        let mut pending = VecDeque::new();

        for root in roots {
            let Some(root_index) = self.target_to_index.get(root).copied() else {
                continue;
            };
            if destinations.contains(root) {
                return TargetSet::singleton(root.clone());
            }
            if visited.insert(root_index) {
                pending.push_back(root_index);
            }
        }

        while let Some(parent) = pending.pop_front() {
            for child in self.nodes[parent as usize].children.iter().copied() {
                if !visited.insert(child) {
                    continue;
                }
                parents[child as usize] = Some(parent);
                if destinations.contains(&self.nodes[child as usize].target) {
                    let mut path = vec![child];
                    let mut cursor = child;
                    while let Some(parent) = parents[cursor as usize] {
                        path.push(parent);
                        cursor = parent;
                    }
                    path.reverse();
                    let mut result = TargetSet::default();
                    for index in path {
                        result.insert(self.nodes[index as usize].target.clone());
                    }
                    return result;
                }
                pending.push_back(child);
            }
        }

        TargetSet::default()
    }
}

async fn reverse_dependencies<E>(
    environment: &mut E,
    universe: TargetSet<E::Target>,
    from: TargetSet<E::Target>,
    max_depth: Option<i32>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let graph = ResolvedGraph::build_stable_forward(environment, universe.iter().cloned()).await?;
    if max_depth.is_some_and(|depth| depth < 0) {
        return Ok(TargetSet::default());
    }
    let graph = graph.reverse();
    let roots = from
        .iter()
        .filter(|target| graph.contains(*target))
        .cloned()
        .collect::<Vec<_>>();
    let graph = match max_depth {
        Some(depth) => graph.take_max_depth(&roots, depth),
        None => graph,
    };
    Ok(graph.depth_first_postorder(&roots))
}

async fn some_path<E>(
    environment: &mut E,
    from: TargetSet<E::Target>,
    to: TargetSet<E::Target>,
) -> Result<TargetSet<E::Target>, QueryError>
where
    E: QueryEnvironment + Send,
{
    let graph = ResolvedGraph::build_stable_forward(environment, from.iter().cloned()).await?;
    let roots = from.iter().cloned().collect::<Vec<_>>();
    Ok(graph.shortest_path(&roots, &to))
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
    max_depth: Option<i32>,
    mut visit: impl FnMut(E::Target),
) -> Result<(), QueryError>
where
    E: QueryEnvironment + Send,
{
    if max_depth.is_some_and(|depth| depth < 0) {
        return Ok(());
    }
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
                pending.push_back((dependency.clone(), depth.saturating_add(1)));
            }
        }
    }
    Ok(())
}

pub struct LoadingQueryEnvironment<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    workspace: PathBuf,
    evaluation_graph: ResolvedGraph<QueryLabel>,
}

impl<'a, 'd> LoadingQueryEnvironment<'a, 'd> {
    pub fn new(ctx: &'a mut DiceComputations<'d>, workspace: PathBuf) -> Self {
        Self {
            ctx,
            workspace,
            evaluation_graph: ResolvedGraph::new(),
        }
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
        let node = graph.nodes.get(&label).cloned().ok_or_else(|| {
            QueryError::evaluation(format!(
                "no such target '{}': target '{}' not declared in package '{}'",
                label,
                label.target(),
                label.package()
            ))
        })?;
        self.evaluation_graph.record_node(label);
        Ok(node)
    }

    fn record_pattern_graph(
        &mut self,
        graph: &UnconfiguredPackageGraph,
        selected: &TargetSet<QueryLabel>,
    ) {
        for node in graph.nodes.values() {
            if !selected.contains(&node.label) {
                continue;
            }
            self.evaluation_graph.record_node(node.label.clone());
            for dependency in node
                .dependencies
                .iter()
                .filter(|dependency| selected.contains(dependency))
            {
                self.evaluation_graph
                    .record_edge(node.label.clone(), dependency.clone());
            }
        }
    }

    async fn resolve_recursive(
        &mut self,
        prefix: &str,
    ) -> Result<TargetSet<QueryLabel>, QueryError> {
        let packages = self
            .ctx
            .compute(&SubtreePackageSetKey {
                workspace: self.workspace.clone(),
                prefix: PathBuf::from(prefix),
            })
            .await
            .map_err(|error| QueryError::evaluation(error.to_string()))?;
        let packages = packages.as_ref().as_ref().map_err(|error| error.clone())?;
        let mut result = TargetSet::default();
        let mut graphs = Vec::with_capacity(packages.packages.len());
        for package in packages.packages.iter() {
            let graph = self.package_graph(package).await?;
            for (label, node) in graph.nodes.iter() {
                if node.kind.is_rule() {
                    result.insert(label.clone());
                }
            }
            graphs.push(graph);
        }
        if result.iter().next().is_none() {
            return Err(QueryError::evaluation(format!(
                "no targets found beneath '{prefix}'"
            )));
        }
        for graph in graphs {
            self.record_pattern_graph(&graph, &result);
        }
        Ok(result)
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
            return self.resolve_recursive("").await;
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
                self.record_pattern_graph(&graph, &result);
                Ok(result)
            }
            TargetPattern::Recursive { repo, package } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                self.resolve_recursive(package.as_str()).await
            }
        }
    }

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError> {
        let node = self.resolve_single(target.clone()).await?;
        for dependency in node.dependencies.iter() {
            self.evaluation_graph
                .record_edge(target.clone(), dependency.clone());
        }
        Ok(node.dependencies)
    }

    async fn same_pkg_direct_rdeps(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError> {
        let mut by_package = SmallMap::<CompactString, SmallSet<QueryLabel>>::new();
        for target in targets.iter() {
            by_package
                .entry(CompactString::new(target.package()))
                .or_default()
                .insert(target.clone());
        }

        let mut result = TargetSet::default();
        for (package, package_targets) in by_package {
            let graph = self.package_graph(&package).await?;
            for node in graph.nodes.values() {
                self.evaluation_graph.record_node(node.label.clone());
                for dependency in node.dependencies.iter() {
                    self.evaluation_graph
                        .record_edge(node.label.clone(), dependency.clone());
                }
                if node
                    .dependencies
                    .iter()
                    .any(|dependency| package_targets.contains(dependency))
                {
                    result.insert(node.label.clone());
                }
            }
        }
        Ok(result)
    }

    async fn siblings(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError> {
        let packages = targets
            .iter()
            .map(|target| CompactString::new(target.package()))
            .collect::<SmallSet<_>>();
        let mut result = TargetSet::default();
        for package in packages {
            let graph = self.package_graph(&package).await?;
            for label in graph.nodes.keys() {
                self.evaluation_graph.record_node(label.clone());
                result.insert(label.clone());
            }
        }
        Ok(result)
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
    let mut evaluator = QueryEvaluator::new(LoadingQueryEnvironment::new(ctx, workspace));
    let targets = evaluator.evaluate(&expression).await?;
    let mut labels = if order == QueryOrder::Full {
        evaluator
            .environment
            .evaluation_graph
            .selected_induced(&targets)
            .deterministic_topological_order()
    } else {
        targets.iter().cloned().collect::<Vec<_>>()
    };
    if order == QueryOrder::Auto && !expression.is_top_level_somepath() {
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
