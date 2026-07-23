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
use dupe::Dupe;
use futures::FutureExt;
use futures::future::BoxFuture;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::discover_build_file_companion;
use slug_loading_v2::keys::PackageLoadKey;
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
use crate::provenance::QueryCandidate;
use crate::provenance::QueryCandidateArena;
use crate::provenance::QueryCandidateBatches;
use crate::provenance::QueryCandidateId;
use crate::validate_loading_query;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Allocative)]
pub enum QueryOrder {
    Auto,
    Full,
}

impl QueryOrder {
    pub const fn is_full(self) -> bool {
        matches!(self, Self::Full)
    }

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
    graph: SelectedQueryGraph,
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

    /// Render the selected graph retained by the evaluation that produced this
    /// output. Formatting never re-enters DICE or query evaluation.
    pub fn graph_stdout(&self, factored: bool, sort_labels: bool) -> String {
        self.graph.stdout(factored, sort_labels)
    }
}

/// Request-local selected graph. This is intentionally compact: labels are
/// shared `CompactString`s and edges are checked `u32` node indexes.
#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct SelectedQueryGraph {
    nodes: Vec<SelectedQueryGraphNode>,
    generated_file_labels: SmallSet<CompactString>,
}

#[derive(Debug, Clone, Eq, PartialEq, Allocative)]
struct SelectedQueryGraphNode {
    label: CompactString,
    successors: Vec<u32>,
}

impl SelectedQueryGraph {
    const NODE_LIMIT: usize = 512;
    const RESERVED_LABEL_CHARS: usize = "\\n...and 9999999 more items".len();

    fn stdout(&self, factored: bool, sort_labels: bool) -> String {
        let mut classes = if factored {
            self.factored_classes(sort_labels)
        } else {
            (0..self.nodes.len())
                .map(|index| {
                    vec![
                        index
                            .try_into()
                            .expect("query graph exceeds u32 node capacity"),
                    ]
                })
                .collect()
        };
        if !factored && sort_labels {
            classes.sort_unstable_by(|left, right| {
                self.nodes[left[0] as usize]
                    .label
                    .cmp(&self.nodes[right[0] as usize].label)
            });
        }
        let mut class_for_node = vec![0_u32; self.nodes.len()];
        for (class, nodes) in classes.iter().enumerate() {
            let class: u32 = class
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            for node in nodes {
                class_for_node[*node as usize] = class;
            }
        }
        let labels = classes
            .iter()
            .map(|class| self.class_label(class))
            .collect::<Vec<_>>();
        let mut successors = vec![Vec::<u32>::new(); classes.len()];
        for (class, nodes) in classes.iter().enumerate() {
            let class_id: u32 = class
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            let mut seen = SmallSet::new();
            for node in nodes {
                for successor in &self.nodes[*node as usize].successors {
                    let successor_class = class_for_node[*successor as usize];
                    if successor_class != class_id && seen.insert(successor_class) {
                        successors[class].push(successor_class);
                    }
                }
            }
            if sort_labels {
                // With label sorting enabled, class IDs are the ranks of
                // Bazel's lexicographical node-sequence comparator. Comparing
                // joined DOT labels would be wrong at a `\\n` boundary.
                successors[class].sort_unstable();
            }
        }

        let order = topological_order(&successors);
        let mut output = String::from("digraph mygraph {\n  node [shape=box];\n");
        for node in order {
            let label = &labels[node as usize];
            output.push_str("  \"");
            output.push_str(label);
            output.push_str("\"\n");
            for successor in &successors[node as usize] {
                output.push_str("  \"");
                output.push_str(label);
                output.push_str("\" -> \"");
                output.push_str(&labels[*successor as usize]);
                output.push_str("\"\n");
            }
        }
        output.push_str("}\n");
        output
    }

    fn factored_classes(&self, sort_labels: bool) -> Vec<Vec<u32>> {
        let mut predecessors = vec![Vec::<u32>::new(); self.nodes.len()];
        for (node, value) in self.nodes.iter().enumerate() {
            for successor in &value.successors {
                predecessors[*successor as usize].push(
                    node.try_into()
                        .expect("query graph exceeds u32 node capacity"),
                );
            }
        }
        for predecessors in &mut predecessors {
            predecessors.sort_unstable();
        }

        let mut assigned = vec![false; self.nodes.len()];
        let mut classes = Vec::new();
        for node in 0..self.nodes.len() {
            if assigned[node] {
                continue;
            }
            assigned[node] = true;
            let mut class = vec![
                node.try_into()
                    .expect("query graph exceeds u32 node capacity"),
            ];
            for sibling in (node + 1)..self.nodes.len() {
                if !assigned[sibling]
                    && predecessors[node] == predecessors[sibling]
                    && self.nodes[node].successors == self.nodes[sibling].successors
                {
                    assigned[sibling] = true;
                    class.push(
                        sibling
                            .try_into()
                            .expect("query graph exceeds u32 node capacity"),
                    );
                }
            }
            if sort_labels {
                class.sort_unstable_by(|left, right| {
                    self.nodes[*left as usize]
                        .label
                        .cmp(&self.nodes[*right as usize].label)
                });
            } else if class.iter().all(|node| {
                self.generated_file_labels
                    .contains(&self.nodes[*node as usize].label)
            }) {
                // Bazel's unsorted factored visitor renders output-list
                // members in its reverse traversal order. Preserve that
                // order only for the generated-file equivalence class;
                // ordinary factored classes retain their existing order.
                class.reverse();
            }
            classes.push(class);
        }
        if sort_labels {
            classes.sort_unstable_by(|left, right| {
                left.iter()
                    .map(|node| &self.nodes[*node as usize].label)
                    .cmp(right.iter().map(|node| &self.nodes[*node as usize].label))
            });
        }
        classes
    }

    fn class_label(&self, class: &[u32]) -> CompactString {
        let mut label = String::new();
        let actual_limit = Self::NODE_LIMIT - Self::RESERVED_LABEL_CHARS;
        for (count, node) in class.iter().enumerate() {
            let item = &self.nodes[*node as usize].label;
            if count != 0 {
                label.push_str("\\n");
                if label.len() + item.len() > actual_limit {
                    label.push_str("...and ");
                    label.push_str(&(class.len() - count).to_string());
                    label.push_str(" more items");
                    break;
                }
            }
            label.push_str(item);
        }
        CompactString::new(label)
    }
}

fn topological_order(successors: &[Vec<u32>]) -> Vec<u32> {
    let starts = (0..successors.len())
        .map(|index| {
            index
                .try_into()
                .expect("query graph exceeds u32 node capacity")
        })
        .collect::<Vec<u32>>();
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(successors.len());
    for start in starts {
        if visited[start as usize] {
            continue;
        }
        visited[start as usize] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((node, next_child)) = stack.last_mut() {
            let children = &successors[*node as usize];
            if let Some(child) = children.get(*next_child).copied() {
                *next_child += 1;
                if !visited[child as usize] {
                    visited[child as usize] = true;
                    stack.push((child, 0));
                }
            } else {
                let (node, _) = stack.pop().expect("query DFS stack is non-empty");
                postorder.push(node);
            }
        }
    }
    postorder.reverse();
    postorder
}

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
    fn singleton(value: T) -> Self {
        let mut values = SmallSet::new();
        values.insert(value);
        Self(values)
    }

    fn insert(&mut self, value: T) {
        self.0.insert(value);
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    fn contains(&self, value: &T) -> bool {
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
    environment: E,
    functions: LoadingQueryFunctions,
}

impl<E> QueryEvaluator<E>
where
    E: QueryEnvironment + Send,
{
    fn new(environment: E) -> Self {
        Self {
            environment,
            functions: LoadingQueryFunctions,
        }
    }

    async fn evaluate(&mut self, expression: &QueryExpression) -> Result<E::Set, QueryError> {
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
async fn async_depth_limited_traversal<E>(
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

struct LoadingQueryEnvironment<'a, 'd> {
    ctx: &'a mut DiceComputations<'d>,
    workspace: PathBuf,
    evaluation_graph: ResolvedGraph<QueryLabel>,
    generated_file_labels: SmallSet<QueryLabel>,
    candidates: QueryCandidateArena,
}

impl<'a, 'd> LoadingQueryEnvironment<'a, 'd> {
    fn new(ctx: &'a mut DiceComputations<'d>, workspace: PathBuf) -> Self {
        Self {
            ctx,
            workspace,
            evaluation_graph: ResolvedGraph::new(),
            generated_file_labels: SmallSet::new(),
            candidates: QueryCandidateArena::new(),
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
                error.with_message(format!(
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
        if matches!(node.kind, crate::QueryNodeKind::GeneratedFile) {
            self.generated_file_labels.insert(label.dupe());
        }
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

    fn real_delivery(
        &mut self,
        labels: impl IntoIterator<Item = QueryLabel>,
    ) -> QueryCandidateBatches {
        QueryCandidateBatches::from_delivery(
            &mut self.candidates,
            labels.into_iter().map(QueryCandidate::real),
        )
    }

    fn selected_graph(&self, targets: &QueryCandidateBatches) -> SelectedQueryGraph {
        let materialized = targets.materialized_by_label(&self.candidates);
        let mut included = SmallMap::<QueryLabel, bool>::new();
        for (label, id) in materialized {
            let candidate = self.candidates.get(id);
            let real = candidate.evaluation_graph_label().is_some();
            if !real || self.evaluation_graph.contains(&label) {
                included.insert(label, real);
            }
        }

        let has_selected_generated_files = included
            .keys()
            .any(|label| self.generated_file_labels.contains(label));
        let mut selected = included.keys().cloned().collect::<Vec<_>>();
        if !has_selected_generated_files {
            // Preserve the established ordinary-query graph order. Generated
            // outputs retain callback/materialization order because sorting
            // here would erase producer order before Bazel's reverse visitor.
            selected.sort_unstable();
        }
        let mut target_to_index = SmallMap::with_capacity(selected.len());
        let mut nodes = Vec::with_capacity(selected.len());
        let mut generated_file_labels = SmallSet::new();
        for label in selected {
            let index: u32 = nodes
                .len()
                .try_into()
                .expect("query graph exceeds u32 node capacity");
            target_to_index.insert(label.clone(), index);
            if self.generated_file_labels.contains(&label) {
                generated_file_labels.insert(CompactString::new(label.to_string()));
            }
            nodes.push(SelectedQueryGraphNode {
                label: CompactString::new(label.to_string()),
                successors: Vec::new(),
            });
        }
        for (label, real) in &included {
            if !real {
                continue;
            }
            let Some(index) = self.evaluation_graph.target_to_index.get(label).copied() else {
                continue;
            };
            for child in self.evaluation_graph.nodes[index as usize]
                .children
                .iter()
                .copied()
            {
                let child = &self.evaluation_graph.nodes[child as usize].target;
                if included.get(child).copied() == Some(true)
                    && let (Some(from), Some(to)) =
                        (target_to_index.get(label), target_to_index.get(child))
                {
                    let successors = &mut nodes[*from as usize].successors;
                    if !successors.contains(to) {
                        successors.push(*to);
                    }
                }
            }
        }
        for node in &mut nodes {
            node.successors.sort_unstable();
        }
        SelectedQueryGraph {
            nodes,
            generated_file_labels,
        }
    }

    // Text FULL is an existing public ordering contract. Keep its reverse
    // postorder implementation separate from graph rendering, whose Bazel
    // formatter uses its own graph visitor order.
    fn selected_full_order(&self, targets: &QueryCandidateBatches) -> Vec<QueryLabel> {
        let materialized = targets.materialized_by_label(&self.candidates);
        let mut included = SmallMap::<QueryLabel, bool>::new();
        for (label, id) in materialized {
            let candidate = self.candidates.get(id);
            let real = candidate.evaluation_graph_label().is_some();
            if !real || self.evaluation_graph.contains(&label) {
                included.insert(label, real);
            }
        }

        let mut labels = included.keys().cloned().collect::<Vec<_>>();
        labels.sort_unstable();
        let mut renderer = ResolvedGraph::new();
        for label in labels {
            renderer.record_node(label);
        }
        for (label, real) in &included {
            if !real {
                continue;
            }
            let Some(index) = self.evaluation_graph.target_to_index.get(label).copied() else {
                continue;
            };
            for child in self.evaluation_graph.nodes[index as usize]
                .children
                .iter()
                .copied()
            {
                let child = &self.evaluation_graph.nodes[child as usize].target;
                if included.get(child).copied() == Some(true) {
                    renderer.record_edge(label.clone(), child.clone());
                }
            }
        }
        renderer.deterministic_topological_order()
    }
}

#[async_trait]
impl QueryEnvironment for LoadingQueryEnvironment<'_, '_> {
    type Target = QueryCandidateId;
    type Set = QueryCandidateBatches;

    fn one_delivery(&self, sets: &[Self::Set]) -> Self::Set {
        let mut seen = SmallSet::new();
        let mut ids = Vec::new();
        for set in sets {
            for (label, id) in set.materialized_by_label(&self.candidates) {
                if seen.insert(label) {
                    ids.push(id);
                }
            }
        }
        QueryCandidateBatches::from_materialized_ids(ids)
    }

    fn union(&self, left: Self::Set, right: Self::Set) -> Self::Set {
        left.union(right)
    }

    fn intersection(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        left.intersection(&self.candidates, right)
    }

    fn except(&self, left: &Self::Set, right: &Self::Set) -> Self::Set {
        left.except(&self.candidates, right)
    }

    fn eval_all(&self, set: &Self::Set) -> TargetSet<Self::Target> {
        let mut result = TargetSet::default();
        if let Some(batch) = set.eval_all(&self.candidates) {
            for id in batch.ids() {
                result.insert(*id);
            }
        }
        result
    }

    fn lift_one_delivery(&self, targets: TargetSet<Self::Target>) -> Self::Set {
        QueryCandidateBatches::from_materialized_ids(targets.iter().copied().collect())
    }

    async fn resolve_literal(&mut self, literal: &str) -> Result<Self::Set, QueryError> {
        if literal == "//..." {
            let labels = self.resolve_recursive("").await?;
            return Ok(self.real_delivery(labels.iter().cloned()));
        }
        let pattern = TargetPattern::parse(literal).map_err(QueryError::evaluation)?;
        match pattern {
            TargetPattern::Single(label) => {
                let label = QueryLabel::parse_root(&label.to_string())?;
                self.resolve_single(label.clone()).await?;
                Ok(self.real_delivery([label]))
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
                Ok(self.real_delivery(result.iter().cloned()))
            }
            TargetPattern::Recursive { repo, package } => {
                if !repo.is_root() {
                    return Err(QueryError::evaluation(format!(
                        "external repository query patterns are deferred: {literal}"
                    )));
                }
                let labels = self.resolve_recursive(package.as_str()).await?;
                Ok(self.real_delivery(labels.iter().cloned()))
            }
        }
    }

    async fn dependencies(
        &mut self,
        target: &Self::Target,
    ) -> Result<Arc<[Self::Target]>, QueryError> {
        let candidate = self.candidates.get(*target).clone();
        let Some(label) = candidate.evaluation_graph_label().cloned() else {
            return Ok(Arc::from([]));
        };
        let node = self.resolve_single(label.clone()).await?;
        let mut dependencies = Vec::with_capacity(node.dependencies.len());
        for dependency in node.dependencies.iter() {
            self.evaluation_graph
                .record_edge(label.clone(), dependency.clone());
            dependencies.push(
                self.candidates
                    .intern(QueryCandidate::real(dependency.clone())),
            );
        }
        Ok(dependencies.into())
    }

    async fn same_pkg_direct_rdeps(
        &mut self,
        targets: &TargetSet<Self::Target>,
    ) -> Result<TargetSet<Self::Target>, QueryError> {
        let mut by_package = SmallMap::<CompactString, SmallSet<QueryLabel>>::new();
        for target in targets.iter() {
            let Some(target) = self
                .candidates
                .get(*target)
                .evaluation_graph_label()
                .cloned()
            else {
                continue;
            };
            by_package
                .entry(CompactString::new(target.package()))
                .or_default()
                .insert(target);
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
                    result.insert(
                        self.candidates
                            .intern(QueryCandidate::real(node.label.clone())),
                    );
                }
            }
        }
        Ok(result)
    }

    async fn siblings(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        let packages = targets.sibling_packages(&self.candidates);
        let mut result = QueryCandidateBatches::empty();
        for package in packages.iter() {
            let graph = self.package_graph(package).await?;
            let mut labels = Vec::with_capacity(graph.nodes.len());
            for label in graph.nodes.keys() {
                self.evaluation_graph.record_node(label.clone());
                labels.push(label.clone());
            }
            result = result.union(self.real_delivery(labels));
        }
        Ok(result)
    }

    async fn loading_files(
        &mut self,
        targets: &Self::Set,
        include_buildfiles: bool,
    ) -> Result<Self::Set, QueryError> {
        let mut seen_packages = SmallSet::new();
        let mut seen_bzl_labels = SmallSet::new();
        let mut seen_output_labels = SmallSet::new();
        let mut result = QueryCandidateBatches::empty();

        for batch in targets.batches() {
            let ids = batch.ids().to_vec();
            let mut delivered = Vec::new();
            for id in ids {
                let candidate = self.candidates.get(id).clone();
                let candidate_package = CompactString::new(candidate.printed_label().package());
                if !seen_packages.insert(candidate_package) {
                    continue;
                }
                let owner = candidate.owner_package();
                let package = self.workspace.join(owner.as_str());
                let value = self
                    .ctx
                    .compute(&PackageLoadKey {
                        workspace: self.workspace.clone(),
                        package,
                    })
                    .await
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;
                let loaded = value
                    .as_ref()
                    .as_ref()
                    .map_err(|error| QueryError::evaluation(error.to_string()))?;

                if include_buildfiles {
                    let basename = loaded
                        .build_file
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or_else(|| {
                            QueryError::evaluation("loaded BUILD file has no UTF-8 basename")
                        })?;
                    let label = QueryLabel::parse_root(&format!("//{owner}:{basename}"))?;
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::real(label));
                    }
                }

                for load in loaded.reachable_loads.iter() {
                    let label = QueryLabel::from_canonical(load.label.clone());
                    if !seen_bzl_labels.insert(label.clone()) {
                        continue;
                    }
                    if seen_output_labels.insert(label.clone()) {
                        delivered.push(QueryCandidate::fake(label.clone(), owner.clone()));
                    }
                    if include_buildfiles {
                        let load_package =
                            self.workspace.join(load.label.package().package().as_str());
                        let companion =
                            discover_build_file_companion(self.ctx, &self.workspace, &load_package)
                                .await
                                .map_err(|error| QueryError::evaluation(error.to_string()))?;
                        if let Some(companion) = companion {
                            let label = QueryLabel::from_canonical(companion.label);
                            if seen_output_labels.insert(label.clone()) {
                                delivered.push(QueryCandidate::fake(label, owner.clone()));
                            }
                        }
                    }
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery(
                &mut self.candidates,
                delivered,
            ));
        }
        Ok(result)
    }

    async fn labels(
        &mut self,
        attribute: &str,
        targets: &Self::Set,
    ) -> Result<Self::Set, QueryError> {
        let mut labels = SmallSet::new();
        for (_, id) in targets.materialized_by_label(&self.candidates) {
            let candidate = self.candidates.get(id).clone();
            let Some(label) = candidate.evaluation_graph_label().cloned() else {
                continue;
            };
            let node = self.resolve_single(label.clone()).await?;
            if !node.kind.is_rule() {
                continue;
            }
            let Some(attribute) = node
                .attributes
                .iter()
                .find(|projection| projection.name == attribute)
            else {
                continue;
            };
            for label in attribute.labels.iter().cloned() {
                self.resolve_single(label.clone()).await.map_err(|error| {
                    let message =
                        format!("in '{}' of rule {}: {error}", attribute.name, node.label);
                    error.with_message(message)
                })?;
                labels.insert(label);
            }
        }
        Ok(self.real_delivery(labels))
    }

    async fn executables(&mut self, targets: &Self::Set) -> Result<Self::Set, QueryError> {
        let mut result = QueryCandidateBatches::empty();
        for batch in targets.batches() {
            let mut delivered = Vec::with_capacity(batch.ids().len());
            for id in batch.ids().iter().copied() {
                let Some(label) = self.candidates.get(id).evaluation_graph_label().cloned() else {
                    // Fake candidates have no loaded target and must neither
                    // be classified nor create a graph node/edge.
                    continue;
                };
                let node = self.resolve_single(label).await?;
                if node.rule_capability.as_ref().is_some_and(|capability| {
                    capability.executable && !capability.rule_class.ends_with("_test")
                }) {
                    delivered.push(id);
                }
            }
            result = result.union(QueryCandidateBatches::from_delivery_ids(delivered));
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

#[cfg(test)]
mod graph_output_tests {
    use compact_str::CompactString;

    use super::SelectedQueryGraph;
    use super::SelectedQueryGraphNode;

    fn graph(nodes: &[(&str, &[u32])]) -> SelectedQueryGraph {
        SelectedQueryGraph {
            nodes: nodes
                .iter()
                .map(|(label, successors)| SelectedQueryGraphNode {
                    label: CompactString::new(*label),
                    successors: successors.to_vec(),
                })
                .collect(),
            generated_file_labels: Default::default(),
        }
    }

    #[test]
    fn full_factored_dot_matches_bazel_node_then_outgoing_edge_layout() {
        let output = graph(&[("//a:root", &[1, 2]), ("//a:left", &[]), ("//a:right", &[])])
            .stdout(true, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:root\"\n",
                "  \"//a:root\" -> \"//a:left\\n//a:right\"\n",
                "  \"//a:left\\n//a:right\"\n",
                "}\n",
            )
        );
    }

    #[test]
    fn unfactored_dot_keeps_equivalent_nodes_separate() {
        let output = graph(&[("//a:root", &[1, 2]), ("//a:left", &[]), ("//a:right", &[])])
            .stdout(false, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:root\"\n",
                "  \"//a:root\" -> \"//a:left\"\n",
                "  \"//a:root\" -> \"//a:right\"\n",
                "  \"//a:right\"\n",
                "  \"//a:left\"\n",
                "}\n",
            )
        );
    }

    #[test]
    fn factoring_requires_matching_predecessors_and_deduplicates_quotient_edges() {
        let different_predecessors = graph(&[
            ("//a:left_parent", &[2]),
            ("//a:right_parent", &[3]),
            ("//a:left", &[4]),
            ("//a:right", &[4]),
            ("//a:leaf", &[]),
        ]);
        assert!(
            !different_predecessors
                .factored_classes(true)
                .iter()
                .map(|class| different_predecessors.class_label(class))
                .any(|label| label == "//a:left\\n//a:right"),
            "equal successors alone must not factor nodes"
        );

        let duplicate_quotient_edges = graph(&[
            ("//a:root", &[1, 2]),
            ("//a:left", &[3]),
            ("//a:right", &[3]),
            ("//a:leaf", &[]),
        ])
        .stdout(true, true);
        assert_eq!(
            duplicate_quotient_edges
                .matches("\"//a:root\" -> \"//a:left\\n//a:right\"")
                .count(),
            1,
            "{duplicate_quotient_edges}"
        );
    }

    #[test]
    fn factored_order_compares_member_label_sequences_not_joined_dot_labels() {
        let output = graph(&[
            ("//a:a", &[]),
            ("//z:z", &[]),
            ("//a:a0", &[3]),
            ("//x:leaf", &[]),
        ])
        .stdout(true, true);
        assert_eq!(
            output,
            concat!(
                "digraph mygraph {\n",
                "  node [shape=box];\n",
                "  \"//a:a0\"\n",
                "  \"//a:a0\" -> \"//x:leaf\"\n",
                "  \"//x:leaf\"\n",
                "  \"//a:a\\n//z:z\"\n",
                "}\n",
            )
        );
    }
}
