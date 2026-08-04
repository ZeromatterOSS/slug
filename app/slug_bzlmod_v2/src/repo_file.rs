/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

#![allow(dead_code)] // Dormant until the root package-policy activation packet.

use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationDiagnosticLevel;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::PathOutcome;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::any::ProvidesStaticType;
use starlark::codemap::Span;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::environment::Module;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::ast::Argument;
use starlark::syntax::ast::AssignTarget;
use starlark::syntax::ast::AstExpr;
use starlark::syntax::ast::AstStmt;
use starlark::syntax::ast::Clause;
use starlark::syntax::ast::Expr;
use starlark::syntax::ast::Stmt;
use starlark::values::list::ListRef;
use starlark::values::none::NoneType;
use starlark::values::range::Range;
use starlark::values::tuple::TupleRef;

use crate::RootPackagePolicyProjectionError;
use crate::RootRepoFileSemanticsProjectionKey;
use crate::RootRepoFileUtf8Mode;
use crate::RootRepositoryRoute;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileError;
use crate::source_preparation::HostRepositorySourceFileKey;
use crate::source_preparation::HostRepositorySourceFileValue;
use crate::source_preparation::RepositorySourceFileError;
use crate::source_preparation::SourcePreparationOutcome;

const INVALID_UTF8: &str = "not a valid UTF-8 encoded file; this can lead to inconsistent behavior and will be disallowed in a future version of Bazel";
const INVALID_UTF8_ERROR_SUFFIX: &str =
    ". For a temporary workaround, see the --incompatible_enforce_starlark_utf8 flag.";

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostRepoFileValue {
    ignored_directories: Arc<[CompactString]>,
}

impl HostRepoFileValue {
    pub(crate) fn empty() -> Self {
        Self {
            ignored_directories: Arc::from([]),
        }
    }

    pub(crate) fn ignored_directories(&self) -> &[CompactString] {
        &self.ignored_directories
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRepoFileError {
    PolicyProjection(RootPackagePolicyProjectionError),
    HostFile(HostFileError),
    InvalidUtf8 {
        logical_path: NormalizedAbsolutePath,
    },
    Syntax {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    RestrictedSyntax {
        logical_path: NormalizedAbsolutePath,
        messages: Arc<[CompactString]>,
    },
    Compile {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
    Evaluation {
        logical_path: NormalizedAbsolutePath,
        message: CompactString,
    },
}

impl fmt::Display for HostRepoFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyProjection(error) => error.fmt(f),
            Self::HostFile(error) => write!(f, "failed to read REPO.bazel: {error:?}"),
            Self::InvalidUtf8 { logical_path } => {
                write!(f, "{logical_path}: {INVALID_UTF8}")
            }
            Self::Syntax {
                logical_path,
                message,
            } => write!(
                f,
                "error parsing REPO.bazel file at {logical_path}: {message}"
            ),
            Self::RestrictedSyntax {
                logical_path,
                messages,
            } => write!(
                f,
                "error parsing REPO.bazel file at {logical_path}: {}",
                messages
                    .iter()
                    .map(CompactString::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Self::Compile {
                logical_path,
                message,
            } => write!(
                f,
                "error preparing REPO.bazel file at {logical_path}: {message}"
            ),
            Self::Evaluation {
                logical_path,
                message,
            } => write!(
                f,
                "error evaluating REPO.bazel file at {logical_path}: {message}"
            ),
        }
    }
}

impl std::error::Error for HostRepoFileError {}

trait RepoEventReporter {
    fn print(&self, location: PrintLocation, text: &str) -> starlark::Result<()>;
    fn diagnostic(&self, level: EvaluationDiagnosticLevel, text: &str);
}

struct DirectRepoEventReporter;

impl RepoEventReporter for DirectRepoEventReporter {
    fn print(&self, _location: PrintLocation, text: &str) -> starlark::Result<()> {
        eprintln!("{text}");
        Ok(())
    }

    fn diagnostic(&self, _level: EvaluationDiagnosticLevel, text: &str) {
        eprintln!("{text}");
    }
}

#[derive(Default)]
struct RecordingRepoEventReporter {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl RecordingRepoEventReporter {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl RepoEventReporter for RecordingRepoEventReporter {
    fn print(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }

    fn diagnostic(&self, level: EvaluationDiagnosticLevel, text: &str) {
        self.events.borrow_mut().push(EvaluationEvent::Diagnostic {
            level,
            text: text.into(),
        });
    }
}

struct RepoPrintHandler<'a> {
    reporter: &'a dyn RepoEventReporter,
}

impl PrintHandler for RepoPrintHandler<'_> {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        self.reporter.print(location, text)
    }
}

#[derive(Default)]
struct RepoEvaluationState {
    repo_called: bool,
    ignore_directories_called: bool,
    ignored_directories: Vec<CompactString>,
}

#[derive(ProvidesStaticType)]
struct RepoEvaluationContext {
    state: RefCell<RepoEvaluationState>,
}

fn repo_context<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a RepoEvaluationContext> {
    eval.extra
        .and_then(|extra| extra.downcast_ref())
        .ok_or_else(|| anyhow::anyhow!("REPO.bazel global invoked without evaluator context"))
}

fn custom_error(message: impl Into<String>) -> starlark::Error {
    starlark::Error::new_other(anyhow::anyhow!(message.into()))
}

fn spelling_suggestion(name: &str, target: &str) -> String {
    let target = target.to_lowercase().encode_utf16().collect::<Vec<_>>();
    let name = name.to_lowercase().encode_utf16().collect::<Vec<_>>();
    let mut prior = (0..=target.len()).collect::<Vec<_>>();
    for (left_index, left) in name.iter().enumerate() {
        let mut current = vec![left_index + 1; target.len() + 1];
        for (right_index, right) in target.iter().enumerate() {
            current[right_index + 1] = (prior[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(prior[right_index] + usize::from(left != right));
        }
        prior = current;
    }
    if prior[target.len()] < 5.min((name.len() + 1) / 2) {
        format!(" (did you mean '{}'?)", String::from_utf16_lossy(&target))
    } else {
        String::new()
    }
}

#[starlark_module]
fn repo_file_globals(builder: &mut GlobalsBuilder) {
    fn repo<'v>(
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        if args.positions(eval.heap())?.next().is_some() {
            return Err(custom_error("repo() got unexpected positional argument"));
        }
        let kwargs = args.names_map()?;
        let mut state = repo_context(eval)
            .map_err(starlark::Error::new_other)?
            .state
            .borrow_mut();
        if state.repo_called {
            return Err(custom_error(
                "'repo' can only be called once in the REPO.bazel file",
            ));
        }
        if state.ignore_directories_called {
            return Err(custom_error(
                "if repo() is called, it must be called before any other functions",
            ));
        }
        if kwargs.is_empty() {
            return Err(custom_error(
                "at least one argument must be given to the 'repo' function",
            ));
        }
        // Values are evaluated by Starlark before this call. This projection
        // deliberately owns no debug representation or package-argument copy.
        state.repo_called = true;
        Ok(NoneType)
    }

    fn ignore_directories<'v>(
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let positional = args.positions(eval.heap())?.collect::<Vec<_>>();
        let admitted = |value| {
            ListRef::from_value(value).is_some()
                || TupleRef::from_value(value).is_some()
                || Range::from_value(value).is_some()
        };
        let type_error = |value: starlark::values::Value<'v>| {
            custom_error(format!(
                "in call to ignore_directories(), parameter 'dirs' got value of type '{}', want 'sequence'",
                value.get_type()
            ))
        };

        let mut dirs = positional.first().copied();
        if let Some(value) = dirs
            && !admitted(value)
        {
            return Err(type_error(value));
        }
        for (name, value) in args.names_map()? {
            if name.as_str() != "dirs" {
                return Err(custom_error(format!(
                    "ignore_directories() got unexpected keyword argument '{}'{}",
                    name.as_str(),
                    spelling_suggestion(name.as_str(), "dirs")
                )));
            }
            if !admitted(value) {
                return Err(type_error(value));
            }
            if dirs.is_some() {
                return Err(custom_error(
                    "ignore_directories() got multiple values for argument 'dirs'",
                ));
            }
            dirs = Some(value);
        }
        if positional.len() > 1 {
            return Err(custom_error(format!(
                "ignore_directories() accepts no more than 1 positional argument but got {}",
                positional.len()
            )));
        }
        let Some(dirs) = dirs else {
            return Err(custom_error(
                "ignore_directories() missing 1 required positional argument: dirs",
            ));
        };

        let mut patterns = Vec::new();
        for value in dirs.iterate(eval.heap())? {
            let value = value.unpack_str().ok_or_else(|| {
                custom_error(format!(
                    "at index {} of dirs, got element of type {}, want string",
                    patterns.len(),
                    value.get_type()
                ))
            })?;
            patterns.push(CompactString::new(value));
        }

        let mut state = repo_context(eval)
            .map_err(starlark::Error::new_other)?
            .state
            .borrow_mut();
        if state.ignore_directories_called {
            return Err(custom_error(
                "'ignored_directories()' can only be called once",
            ));
        }
        state.ignore_directories_called = true;
        state.ignored_directories = patterns;
        Ok(NoneType)
    }

    fn set<'v>(
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let positional = args.positions(eval.heap())?.collect::<Vec<_>>();
        let ensure_iterable = |value: starlark::values::Value<'v>| {
            value.iterate(eval.heap()).map(|_| ()).map_err(|_| {
                custom_error(format!(
                    "in call to set(), parameter 'elements' got value of type '{}', want 'iterable'",
                    value.get_type()
                ))
            })
        };
        let mut elements = positional.first().copied();
        if let Some(value) = elements {
            ensure_iterable(value)?;
        }
        for (name, value) in args.names_map()? {
            if name.as_str() != "elements" {
                return Err(custom_error(format!(
                    "set() got unexpected keyword argument '{}'{}",
                    name.as_str(),
                    spelling_suggestion(name.as_str(), "elements")
                )));
            }
            ensure_iterable(value)?;
            if elements.is_some() {
                return Err(custom_error(
                    "set() got multiple values for argument 'elements'",
                ));
            }
            elements = Some(value);
        }
        if positional.len() > 1 {
            return Err(custom_error(format!(
                "set() accepts no more than 1 positional argument but got {}",
                positional.len()
            )));
        }
        Err(custom_error(
            "Use of set() requires --experimental_enable_starlark_set",
        ))
    }
}

fn repo_globals() -> Globals {
    static STANDARD: OnceLock<Globals> = OnceLock::new();
    const STANDARD_NAMES: &[&str] = &[
        "False",
        "True",
        "None",
        "min",
        "max",
        "abs",
        "all",
        "any",
        "sorted",
        "reversed",
        "tuple",
        "list",
        "len",
        "str",
        "repr",
        "bool",
        "float",
        "int",
        "dict",
        "enumerate",
        "hash",
        "range",
        "hasattr",
        "getattr",
        "dir",
        "fail",
        "type",
        "zip",
    ];
    let standard = STANDARD.get_or_init(Globals::standard);
    let mut builder = GlobalsBuilder::new();
    for (name, value) in standard.iter() {
        if STANDARD_NAMES.contains(&name) {
            builder.set(name, value);
        }
    }
    LibraryExtension::Print.add(&mut builder);
    repo_file_globals(&mut builder);
    builder.build()
}

fn latin1_projection(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn diagnostic_message(
    logical_path: &NormalizedAbsolutePath,
    message: impl fmt::Display,
) -> CompactString {
    CompactString::new(format!("{}: {message}", logical_path.as_path().display()))
}

struct RestrictedSyntaxIssue {
    span: Span,
    message: CompactString,
}

fn inspect_restricted_syntax(ast: &AstModule) -> Vec<RestrictedSyntaxIssue> {
    fn push(out: &mut Vec<RestrictedSyntaxIssue>, span: Span, message: &str) {
        out.push(RestrictedSyntaxIssue {
            span,
            message: CompactString::new(message),
        });
    }

    fn assignment(
        target: &starlark::syntax::ast::AstAssignTarget,
        out: &mut Vec<RestrictedSyntaxIssue>,
    ) {
        match &target.node {
            AssignTarget::Tuple(values) => {
                for value in values {
                    assignment(value, out);
                }
            }
            AssignTarget::Index(values) => {
                expression(&values.0, out);
                expression(&values.1, out);
            }
            AssignTarget::Dot(receiver, _) => expression(receiver, out),
            AssignTarget::Identifier(_) => {}
        }
    }

    fn clauses(
        first: &starlark::syntax::ast::ForClause,
        rest: &[starlark::syntax::ast::Clause],
        out: &mut Vec<RestrictedSyntaxIssue>,
    ) {
        assignment(&first.var, out);
        expression(&first.over, out);
        for clause in rest {
            match clause {
                Clause::For(clause) => {
                    assignment(&clause.var, out);
                    expression(&clause.over, out);
                }
                Clause::If(value) => expression(value, out),
            }
        }
    }

    fn expression(value: &AstExpr, out: &mut Vec<RestrictedSyntaxIssue>) {
        match &value.node {
            Expr::Tuple(values) | Expr::List(values) => {
                for value in values {
                    expression(value, out);
                }
            }
            Expr::Dot(receiver, _)
            | Expr::Not(receiver)
            | Expr::Minus(receiver)
            | Expr::Plus(receiver)
            | Expr::BitNot(receiver) => expression(receiver, out),
            Expr::Call(callee, args) => {
                for argument in &args.args {
                    match &argument.node {
                        Argument::Args(_) => push(
                            out,
                            argument.span,
                            "*args arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                        ),
                        Argument::KwArgs(value) => {
                            push(
                                out,
                                argument.span,
                                "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                            );
                            if !matches!(&value.node, Expr::Dict(_)) {
                                push(
                                    out,
                                    argument.span,
                                    "**kwargs arguments must be a literal dict in REPO.bazel files.",
                                );
                            }
                        }
                        Argument::Positional(_) | Argument::Named(_, _) => {}
                    }
                }
                expression(callee, out);
                for argument in &args.args {
                    expression(argument.node.expr(), out);
                }
            }
            Expr::Index(values) => {
                expression(&values.0, out);
                expression(&values.1, out);
            }
            Expr::Index2(values) => {
                expression(&values.0, out);
                expression(&values.1, out);
                expression(&values.2, out);
            }
            Expr::Slice(receiver, start, end, step) => {
                expression(receiver, out);
                for value in [start.as_deref(), end.as_deref(), step.as_deref()]
                    .into_iter()
                    .flatten()
                {
                    expression(value, out);
                }
            }
            Expr::Op(left, _, right) => {
                expression(left, out);
                expression(right, out);
            }
            Expr::If(values) => {
                expression(&values.0, out);
                expression(&values.1, out);
                expression(&values.2, out);
            }
            Expr::Dict(values) => {
                for (key, value) in values {
                    expression(key, out);
                    expression(value, out);
                }
            }
            Expr::ListComprehension(value, first, rest) => {
                clauses(first, rest, out);
                expression(value, out);
            }
            Expr::DictComprehension(values, first, rest) => {
                clauses(first, rest, out);
                expression(&values.0, out);
                expression(&values.1, out);
            }
            Expr::FString(value) => {
                for expression_value in &value.node.expressions {
                    expression(expression_value, out);
                }
            }
            Expr::Lambda(_) => push(
                out,
                value.span,
                "functions may not be defined in REPO.bazel files.",
            ),
            Expr::Identifier(_) | Expr::Literal(_) => {}
        }
    }

    fn statement(value: &AstStmt, out: &mut Vec<RestrictedSyntaxIssue>) {
        match &value.node {
            Stmt::Statements(values) => {
                for value in values {
                    statement(value, out);
                }
            }
            Stmt::Expression(value) => expression(value, out),
            Stmt::Assign(value) => {
                expression(&value.rhs, out);
                assignment(&value.lhs, out);
            }
            Stmt::AssignModify(target, _, value) => {
                expression(value, out);
                assignment(target, out);
            }
            Stmt::Load(_) => push(
                out,
                value.span,
                "`load` statements may not be used in REPO.bazel files",
            ),
            Stmt::Def(_) => push(
                out,
                value.span,
                "functions may not be defined in REPO.bazel files.",
            ),
            Stmt::For(_) => push(
                out,
                value.span,
                "`for` statements are not allowed in REPO.bazel files. You may inline the loop or as a last resort use a list comprehension.",
            ),
            Stmt::If(_, _) | Stmt::IfElse(_, _) => push(
                out,
                value.span,
                "`if` statements are not allowed in REPO.bazel files. You may use an `if` expression for simple cases.",
            ),
            Stmt::Return(value) => {
                if let Some(value) = value {
                    expression(value, out);
                }
            }
            Stmt::Break | Stmt::Continue | Stmt::Pass => {}
        }
    }

    let mut messages = Vec::new();
    statement(ast.statement(), &mut messages);
    messages
}

fn evaluate_repo_file(
    logical_path: &NormalizedAbsolutePath,
    bytes: &[u8],
    utf8_mode: RootRepoFileUtf8Mode,
    reporter: &dyn RepoEventReporter,
) -> Result<HostRepoFileValue, HostRepoFileError> {
    if std::str::from_utf8(bytes).is_err() {
        let located_invalid_utf8 = format!("{}: {INVALID_UTF8}", logical_path.as_path().display());
        match utf8_mode {
            RootRepoFileUtf8Mode::Off => {}
            RootRepoFileUtf8Mode::Warning => {
                reporter.diagnostic(EvaluationDiagnosticLevel::Warning, &located_invalid_utf8)
            }
            RootRepoFileUtf8Mode::Error => {
                reporter.diagnostic(
                    EvaluationDiagnosticLevel::Error,
                    &format!("{located_invalid_utf8}{INVALID_UTF8_ERROR_SUFFIX}"),
                );
                return Err(HostRepoFileError::InvalidUtf8 {
                    logical_path: logical_path.dupe(),
                });
            }
        }
    }

    let source = latin1_projection(bytes);
    let dialect = Dialect {
        enable_top_level_stmt: true,
        ..Dialect::Standard
    };
    let ast = AstModule::parse(
        &logical_path.as_path().display().to_string(),
        source,
        &dialect,
    )
    .map_err(|error| {
        let message = diagnostic_message(logical_path, error);
        reporter.diagnostic(EvaluationDiagnosticLevel::Error, message.as_str());
        HostRepoFileError::Syntax {
            logical_path: logical_path.dupe(),
            message,
        }
    })?;

    let restricted = inspect_restricted_syntax(&ast);
    if !restricted.is_empty() {
        for issue in &restricted {
            let span = ast.file_span(issue.span).resolve_span();
            reporter.diagnostic(
                EvaluationDiagnosticLevel::Error,
                &format!(
                    "{}:{}:{}: {}",
                    logical_path.as_path().display(),
                    span.begin.line + 1,
                    span.begin.column + 1,
                    issue.message
                ),
            );
        }
        return Err(HostRepoFileError::RestrictedSyntax {
            logical_path: logical_path.dupe(),
            messages: restricted
                .into_iter()
                .map(|issue| issue.message)
                .collect::<Vec<_>>()
                .into(),
        });
    }

    let module = Module::new();
    let context = RepoEvaluationContext {
        state: RefCell::new(RepoEvaluationState::default()),
    };
    let print_handler = RepoPrintHandler { reporter };
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);
    eval.set_print_handler(&print_handler);
    let globals = repo_globals();
    let prepared = eval.prepare_module(ast, &globals).map_err(|error| {
        let message = CompactString::new(error.to_string());
        reporter.diagnostic(EvaluationDiagnosticLevel::Error, message.as_str());
        HostRepoFileError::Compile {
            logical_path: logical_path.dupe(),
            message,
        }
    })?;
    eval.eval_prepared_module(&prepared).map_err(|error| {
        let message = CompactString::new(error.to_string());
        reporter.diagnostic(EvaluationDiagnosticLevel::Error, message.as_str());
        HostRepoFileError::Evaluation {
            logical_path: logical_path.dupe(),
            message,
        }
    })?;
    drop(eval);
    let state = context.state.into_inner();
    Ok(HostRepoFileValue {
        ignored_directories: state.ignored_directories.into(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRepoFileKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRepoFileKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRepoFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-repo-file:{}", self.workspace)
    }
}

#[track_caller]
fn dice_invariant<T, E: fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("Host REPO-file DICE invariant failed: {error:?}"))
}

#[async_trait]
impl Key for HostRepoFileKey {
    type Value = PathOutcome<Arc<Result<HostRepoFileValue, HostRepoFileError>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let semantics = match dice_invariant(
            ctx.compute(&RootRepoFileSemanticsProjectionKey::new(
                self.workspace.dupe(),
            ))
            .await,
        ) {
            Ok(semantics) => semantics,
            Err(error) => {
                if capture_events {
                    ctx.store_evaluation_data(EventBatch::empty())
                        .expect("Host REPO key stores exactly one event batch");
                }
                return PathOutcome::Complete(Arc::new(Err(HostRepoFileError::PolicyProjection(
                    error,
                ))));
            }
        };
        let logical_path = NormalizedAbsolutePath::new(self.workspace.as_path().join("REPO.bazel"))
            .expect("joining a normalized absolute workspace remains absolute");
        let bytes = match dice_invariant(
            ctx.compute(&HostFileBytesKey::new(logical_path.dupe()))
                .await,
        ) {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(Err(error)) => {
                if capture_events {
                    ctx.store_evaluation_data(EventBatch::empty())
                        .expect("Host REPO key stores exactly one event batch");
                }
                return PathOutcome::Complete(Arc::new(Err(HostRepoFileError::HostFile(error))));
            }
            PathOutcome::Complete(Ok(HostFileBytes::Missing)) => {
                if capture_events {
                    ctx.store_evaluation_data(EventBatch::empty())
                        .expect("Host REPO key stores exactly one event batch");
                }
                return PathOutcome::Complete(Arc::new(Ok(HostRepoFileValue::empty())));
            }
            PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => bytes,
        };

        let recording = capture_events.then(RecordingRepoEventReporter::default);
        let direct = DirectRepoEventReporter;
        let reporter: &dyn RepoEventReporter = match recording.as_ref() {
            Some(recording) => recording,
            None => &direct,
        };
        let value = evaluate_repo_file(&logical_path, &bytes, semantics.utf8_mode, reporter);
        if let Some(recording) = recording {
            ctx.store_evaluation_data(recording.into_batch())
                .expect("Host REPO key stores exactly one event batch");
        }
        PathOutcome::Complete(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum HostRouteRepoFileError {
    PolicyProjection(RootPackagePolicyProjectionError),
    Source(RepositorySourceFileError),
    Evaluation(HostRepoFileError),
}

impl fmt::Display for HostRouteRepoFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyProjection(error) => error.fmt(f),
            Self::Source(error) => write!(f, "failed to read routed REPO.bazel: {error:?}"),
            Self::Evaluation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for HostRouteRepoFileError {}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct HostRouteRepoFileKey {
    route: RootRepositoryRoute,
}

impl HostRouteRepoFileKey {
    pub(crate) fn new(route: RootRepositoryRoute) -> Self {
        Self { route }
    }
}

impl std::hash::Hash for HostRouteRepoFileKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.route.hash(state);
    }
}

impl fmt::Display for HostRouteRepoFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-route-repo-file:{}", self.route.canonical_repo())
    }
}

fn store_route_repo_batch(ctx: &mut DiceComputations<'_>, capture_events: bool, batch: EventBatch) {
    if capture_events {
        ctx.store_evaluation_data(batch)
            .expect("routed REPO key stores exactly one event batch");
    }
}

#[async_trait]
impl Key for HostRouteRepoFileKey {
    type Value = SourcePreparationOutcome<Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let semantics = match dice_invariant(
            ctx.compute(&RootRepoFileSemanticsProjectionKey::new(
                self.route.workspace().dupe(),
            ))
            .await,
        ) {
            Ok(semantics) => semantics,
            Err(error) => {
                store_route_repo_batch(ctx, capture_events, EventBatch::empty());
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostRouteRepoFileError::PolicyProjection(error),
                )));
            }
        };
        let source = match dice_invariant(
            ctx.compute(&HostRepositorySourceFileKey::new(
                self.route.clone(),
                "REPO.bazel".into(),
            ))
            .await,
        ) {
            SourcePreparationOutcome::Need(need) => {
                return SourcePreparationOutcome::Need(need);
            }
            SourcePreparationOutcome::Complete(Err(error)) => {
                store_route_repo_batch(ctx, capture_events, EventBatch::empty());
                return SourcePreparationOutcome::Complete(Arc::new(Err(
                    HostRouteRepoFileError::Source(error),
                )));
            }
            SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Absent)) => {
                store_route_repo_batch(ctx, capture_events, EventBatch::empty());
                return SourcePreparationOutcome::Complete(Arc::new(
                    Ok(HostRepoFileValue::empty()),
                ));
            }
            SourcePreparationOutcome::Complete(Ok(HostRepositorySourceFileValue::Present {
                bytes,
                logical_path,
            })) => (bytes, logical_path),
        };

        let recording = capture_events.then(RecordingRepoEventReporter::default);
        let direct = DirectRepoEventReporter;
        let reporter: &dyn RepoEventReporter = recording
            .as_ref()
            .map_or(&direct, |recording| recording as &dyn RepoEventReporter);
        let value = evaluate_repo_file(&source.1, &source.0, semantics.utf8_mode, reporter)
            .map_err(HostRouteRepoFileError::Evaluation);
        store_route_repo_batch(
            ctx,
            capture_events,
            recording.map_or_else(EventBatch::empty, RecordingRepoEventReporter::into_batch),
        );
        SourcePreparationOutcome::Complete(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::Mutex;

    use compact_str::CompactString;
    #[cfg(unix)]
    use dice::ActivationData;
    #[cfg(unix)]
    use dice::ActivationKind;
    #[cfg(unix)]
    use dice::ActivationTracker;
    #[cfg(unix)]
    use dice::DetectCycles;
    #[cfg(unix)]
    use dice::Dice;
    #[cfg(unix)]
    use dice::DynKey;
    #[cfg(unix)]
    use dice::RichActivation;
    #[cfg(unix)]
    use dice::UserComputationData;
    #[cfg(unix)]
    use dupe::Dupe;
    #[cfg(unix)]
    use slug_events_v2::CaptureEvaluationEvents;
    use slug_events_v2::EvaluationDiagnosticLevel;
    use slug_events_v2::EvaluationEvent;
    #[cfg(unix)]
    use slug_events_v2::EventBatch;
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationNamespace;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationOperation;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOperationResult;
    #[cfg(unix)]
    use slug_workspace_v2::PathOutcome;

    use super::HostRepoFileError;
    use super::INVALID_UTF8;
    use super::INVALID_UTF8_ERROR_SUFFIX;
    use super::RecordingRepoEventReporter;
    use super::RootRepoFileUtf8Mode;
    use super::evaluate_repo_file;
    use super::repo_globals;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;

    fn path() -> NormalizedAbsolutePath {
        #[cfg(unix)]
        {
            NormalizedAbsolutePath::new("/workspace/REPO.bazel").unwrap()
        }
        #[cfg(windows)]
        {
            NormalizedAbsolutePath::new(r"C:\workspace\REPO.bazel").unwrap()
        }
    }

    fn evaluate(
        source: &[u8],
        mode: RootRepoFileUtf8Mode,
    ) -> (
        Result<super::HostRepoFileValue, HostRepoFileError>,
        Vec<EvaluationEvent>,
    ) {
        let reporter = RecordingRepoEventReporter::default();
        let value = evaluate_repo_file(&path(), source, mode, &reporter);
        (value, reporter.into_batch().events().to_vec())
    }

    #[test]
    fn latin1_projection_and_utf8_modes_report_before_parsing() {
        let bytes = b"ignore_directories([\"\xff\"])\n";
        let (off, off_events) = evaluate(bytes, RootRepoFileUtf8Mode::Off);
        assert_eq!(off.unwrap().ignored_directories(), ["\u{ff}"]);
        assert!(off_events.is_empty());

        let (warning, warning_events) = evaluate(bytes, RootRepoFileUtf8Mode::Warning);
        assert_eq!(warning.unwrap().ignored_directories(), ["\u{ff}"]);
        assert!(matches!(
            warning_events.as_slice(),
            [EvaluationEvent::Diagnostic {
                level: EvaluationDiagnosticLevel::Warning,
                text,
            }] if text == &format!("{}: {INVALID_UTF8}", path().as_path().display())
        ));

        let (error, error_events) =
            evaluate(b"\xff this is not Starlark", RootRepoFileUtf8Mode::Error);
        assert!(matches!(error, Err(HostRepoFileError::InvalidUtf8 { .. })));
        assert!(matches!(
            error_events.as_slice(),
            [EvaluationEvent::Diagnostic {
                level: EvaluationDiagnosticLevel::Error,
                text,
            }] if text == &format!(
                "{}: {INVALID_UTF8}{INVALID_UTF8_ERROR_SUFFIX}",
                path().as_path().display()
            )
        ));
    }

    #[test]
    fn repo_globals_discard_kwargs_and_accept_exact_sequence_implementations() {
        for source in [
            "repo(value = {'nested': [1, 2]})\nignore_directories(['a', 'b'])",
            "repo(value = object())\nignore_directories(('a', 'b'))",
        ] {
            let source = source.replace("object()", "dict()");
            let (value, events) = evaluate(source.as_bytes(), RootRepoFileUtf8Mode::Warning);
            assert_eq!(value.unwrap().ignored_directories(), ["a", "b"]);
            assert!(events.is_empty());
        }

        let (range, _) = evaluate(
            b"ignore_directories(range(2))",
            RootRepoFileUtf8Mode::Warning,
        );
        assert!(matches!(range, Err(HostRepoFileError::Evaluation { .. })));
        assert!(
            evaluate(
                b"ignore_directories(range(0))",
                RootRepoFileUtf8Mode::Warning
            )
            .0
            .is_ok()
        );
    }

    #[test]
    fn repo_global_names_are_exact() {
        assert_eq!(
            repo_globals()
                .names()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            [
                "False",
                "None",
                "True",
                "abs",
                "all",
                "any",
                "bool",
                "dict",
                "dir",
                "enumerate",
                "fail",
                "float",
                "getattr",
                "hasattr",
                "hash",
                "ignore_directories",
                "int",
                "len",
                "list",
                "max",
                "min",
                "print",
                "range",
                "repo",
                "repr",
                "reversed",
                "set",
                "sorted",
                "str",
                "tuple",
                "type",
                "zip",
            ]
        );
    }

    #[test]
    fn restricted_syntax_is_source_ordered_and_prunes_rejected_children() {
        let (value, events) = evaluate(
            b"load('//:x.bzl', 'x')\ndef f():\n  repo(**value)\nif True:\n  f(*xs)\nrepo(**value)\n",
            RootRepoFileUtf8Mode::Warning,
        );
        let Err(HostRepoFileError::RestrictedSyntax { messages, .. }) = value else {
            panic!("expected restricted syntax error");
        };
        assert_eq!(
            messages
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>(),
            [
                "`load` statements may not be used in REPO.bazel files",
                "functions may not be defined in REPO.bazel files.",
                "`if` statements are not allowed in REPO.bazel files. You may use an `if` expression for simple cases.",
                "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                "**kwargs arguments must be a literal dict in REPO.bazel files.",
            ]
        );
        assert_eq!(events.len(), messages.len());
        assert!(matches!(
            &events[0],
            EvaluationEvent::Diagnostic { text, .. }
                if text.ends_with("REPO.bazel:1:1: `load` statements may not be used in REPO.bazel files")
        ));
        assert!(matches!(
            events.last().unwrap(),
            EvaluationEvent::Diagnostic { text, .. }
                if text.ends_with("REPO.bazel:6:6: **kwargs arguments must be a literal dict in REPO.bazel files.")
        ));
    }

    #[test]
    fn restricted_syntax_uses_bazel_comprehension_and_assignment_visit_order() {
        let expected = [
            "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
            "**kwargs arguments must be a literal dict in REPO.bazel files.",
            "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
        ];
        for source in [
            "[repo(**{}) for _ in repo(**clause)]",
            "values[repo(**{})] += repo(**rhs)",
            "values[repo(**{})] = repo(**rhs)",
        ] {
            let (value, _) = evaluate(source.as_bytes(), RootRepoFileUtf8Mode::Warning);
            let Err(HostRepoFileError::RestrictedSyntax { messages, .. }) = value else {
                panic!("expected restricted syntax error for {source:?}");
            };
            assert_eq!(
                messages
                    .iter()
                    .map(CompactString::as_str)
                    .collect::<Vec<_>>(),
                expected
            );
        }

        let (value, _) = evaluate(
            b"repo(**then) if repo(**{}) else repo(**otherwise)",
            RootRepoFileUtf8Mode::Warning,
        );
        let Err(HostRepoFileError::RestrictedSyntax { messages, .. }) = value else {
            panic!("expected conditional-expression restricted syntax error");
        };
        assert_eq!(
            messages
                .iter()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            [
                "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                "**kwargs arguments must be a literal dict in REPO.bazel files.",
                "**kwargs arguments are not allowed in REPO.bazel files. Pass the arguments in explicitly.",
                "**kwargs arguments must be a literal dict in REPO.bazel files.",
            ]
        );
    }

    #[test]
    fn prints_and_runtime_failure_remain_interleaved_in_the_recording_seam() {
        let (value, events) = evaluate(
            b"print('before')\nfail('after')",
            RootRepoFileUtf8Mode::Warning,
        );
        assert!(matches!(value, Err(HostRepoFileError::Evaluation { .. })));
        assert!(matches!(
            events.as_slice(),
            [
                EvaluationEvent::StarlarkPrint { location, text },
                EvaluationEvent::Diagnostic {
                    level: EvaluationDiagnosticLevel::Error,
                    ..
                }
            ] if text == "before"
                && location.to_string() == format!("{}:1:6", path().as_path().display())
        ));
    }

    #[test]
    fn direct_reporter_keeps_capture_disabled_prints_out_of_event_storage() {
        const CHILD_ENV: &str = "SLUG_DIRECT_REPO_REPORTER_CHILD";
        if std::env::var_os(CHILD_ENV).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "repo_file::tests::direct_reporter_keeps_capture_disabled_prints_out_of_event_storage",
                    "--nocapture",
                ])
                .env(CHILD_ENV, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "child failed: stdout={:?}, stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stderr, b"direct\n");
            assert!(
                !output
                    .stderr
                    .windows(b"DEBUG:".len())
                    .any(|value| value == b"DEBUG:")
            );
            return;
        }

        let value = evaluate_repo_file(
            &path(),
            b"print('direct')\nignore_directories(['out'])\n",
            RootRepoFileUtf8Mode::Warning,
            &super::DirectRepoEventReporter,
        )
        .unwrap();
        assert_eq!(value.ignored_directories(), ["out"]);

        let recording = RecordingRepoEventReporter::default();
        evaluate_repo_file(
            &path(),
            b"print('direct')\nignore_directories(['out'])\n",
            RootRepoFileUtf8Mode::Warning,
            &recording,
        )
        .unwrap();
        assert!(matches!(
            recording.into_batch().events(),
            [EvaluationEvent::StarlarkPrint { text, .. }] if text == "direct"
        ));
    }

    #[test]
    fn repo_and_ignore_call_order_and_typo_diagnostics_are_exact() {
        for (source, expected) in [
            (
                "repo()\n",
                "at least one argument must be given to the 'repo' function",
            ),
            (
                "repo(value = 1)\nrepo(value = 2)",
                "'repo' can only be called once in the REPO.bazel file",
            ),
            (
                "ignore_directories([])\nrepo(value = 1)",
                "if repo() is called, it must be called before any other functions",
            ),
            (
                "ignore_directories([])\nignore_directories([])",
                "'ignored_directories()' can only be called once",
            ),
            (
                "set()",
                "Use of set() requires --experimental_enable_starlark_set",
            ),
            (
                "ignore_directories('bad')",
                "in call to ignore_directories(), parameter 'dirs' got value of type 'string', want 'sequence'",
            ),
            (
                "ignore_directories([1])",
                "at index 0 of dirs, got element of type int, want string",
            ),
            (
                "ignore_directories([], dirs = [], bad = 1)",
                "ignore_directories() got multiple values for argument 'dirs'",
            ),
            (
                "ignore_directories([], bad = 1, dirs = [])",
                "ignore_directories() got unexpected keyword argument 'bad'",
            ),
            (
                "ignore_directories('bad', bad = 1)",
                "in call to ignore_directories(), parameter 'dirs' got value of type 'string', want 'sequence'",
            ),
            (
                "ignore_directories([], dirs = 'bad')",
                "in call to ignore_directories(), parameter 'dirs' got value of type 'string', want 'sequence'",
            ),
            (
                "ignore_directories([])\nignore_directories([1])",
                "at index 0 of dirs, got element of type int, want string",
            ),
            (
                "ignore_directories(dir = [])",
                "ignore_directories() got unexpected keyword argument 'dir' (did you mean 'dirs'?)",
            ),
            (
                "ignore_directories(drs = [])",
                "ignore_directories() got unexpected keyword argument 'drs' (did you mean 'dirs'?)",
            ),
            (
                "ignore_directories(dirsx = [])",
                "ignore_directories() got unexpected keyword argument 'dirsx' (did you mean 'dirs'?)",
            ),
            (
                "ignore_directories(DIRS = [])",
                "ignore_directories() got unexpected keyword argument 'DIRS' (did you mean 'dirs'?)",
            ),
            (
                "ignore_directories(dixx = [])",
                "ignore_directories() got unexpected keyword argument 'dixx'",
            ),
            (
                "set(1, 2)",
                "in call to set(), parameter 'elements' got value of type 'int', want 'iterable'",
            ),
            (
                "set(bad = 1)",
                "set() got unexpected keyword argument 'bad'",
            ),
            (
                "set(element = [])",
                "set() got unexpected keyword argument 'element' (did you mean 'elements'?)",
            ),
            (
                "set('bad')",
                "in call to set(), parameter 'elements' got value of type 'string', want 'iterable'",
            ),
            (
                "set([], elements = 1)",
                "in call to set(), parameter 'elements' got value of type 'int', want 'iterable'",
            ),
            (
                "set([], elements = [])",
                "set() got multiple values for argument 'elements'",
            ),
            (
                "set([], [])",
                "set() accepts no more than 1 positional argument but got 2",
            ),
        ] {
            let (value, _) = evaluate(source.as_bytes(), RootRepoFileUtf8Mode::Warning);
            let Err(HostRepoFileError::Evaluation { message, .. }) = value else {
                panic!("expected evaluation error for {source:?}");
            };
            assert!(message.contains(expected), "{message:?}");
        }
    }

    #[cfg(unix)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RepoActivation {
        kind: ActivationKind,
        batch: Option<EventBatch>,
    }

    #[cfg(unix)]
    #[derive(Default)]
    struct RepoEventTracker {
        activations: Mutex<Vec<RepoActivation>>,
    }

    #[cfg(unix)]
    impl RepoEventTracker {
        fn take(&self) -> Vec<RepoActivation> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }
    }

    #[cfg(unix)]
    impl ActivationTracker for RepoEventTracker {
        fn key_activated(
            &self,
            _key: &DynKey,
            _deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<super::HostRepoFileKey>().is_none() {
                return;
            }
            self.activations.lock().unwrap().push(RepoActivation {
                kind: activation.kind(),
                batch: activation
                    .evaluation_data()
                    .and_then(|data| data.downcast_ref::<EventBatch>())
                    .map(Dupe::dupe),
            });
        }
    }

    #[cfg(unix)]
    fn repo_epoch(source: Option<&'static [u8]>, variant: i64) -> PathObservationEpoch {
        let lstat = |kind| PathLstat::new(kind, variant, variant, variant, variant, 0o755);
        let demand = |path: &str, operation| {
            PathObservationDemand::new(
                PathObservationNamespace::Host,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let mut entries = vec![
            (
                demand("/", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                ))),
            ),
            (
                demand("/workspace", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                    PathNodeKind::Directory,
                ))),
            ),
        ];
        match source {
            Some(source) => {
                entries.push((
                    demand("/workspace/REPO.bazel", PathObservationOperation::Lstat),
                    PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                        PathNodeKind::RegularFile,
                    ))),
                ));
                entries.push((
                    demand("/workspace/REPO.bazel", PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        source,
                    ))),
                ));
            }
            None => entries.push((
                demand("/workspace/REPO.bazel", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Missing),
            )),
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    #[cfg(unix)]
    async fn observed_repo(
        dice: &Arc<Dice>,
        tracker: Arc<RepoEventTracker>,
        epoch: PathObservationEpoch,
    ) -> PathOutcome<Arc<Result<super::HostRepoFileValue, HostRepoFileError>>> {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        user_data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(user_data);
        inject_root_package_policy_inputs(
            &mut updater,
            RootPackagePolicyInputs::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                Arc::from([NormalizedAbsolutePath::new("/workspace").unwrap()]),
                std::iter::empty::<&str>(),
                None,
                Some("warning"),
            )
            .unwrap(),
        )
        .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&super::HostRepoFileKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn host_repo_key_captures_complete_batches_and_never_replays_on_warm_reuse() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(RepoEventTracker::default());

        let source_a = b"print('A')\nignore_directories(['a'])\n";
        let a = observed_repo(&dice, tracker.dupe(), repo_epoch(Some(source_a), 1)).await;
        assert!(matches!(a, PathOutcome::Complete(value) if value.is_ok()));
        let cold = tracker.take();
        assert!(matches!(
            cold.as_slice(),
            [RepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if matches!(
                batch.events(),
                [EvaluationEvent::StarlarkPrint { text, .. }] if text == "A"
            )
        ));

        observed_repo(&dice, tracker.dupe(), repo_epoch(Some(source_a), 1)).await;
        assert!(
            tracker
                .take()
                .iter()
                .all(|activation| activation.batch.is_none())
        );

        let source_b = b"print('B')\nignore_directories(['b'])\n";
        observed_repo(&dice, tracker.dupe(), repo_epoch(Some(source_b), 2)).await;
        assert!(tracker.take().iter().any(|activation| matches!(
            activation.batch.as_ref().map(EventBatch::events),
            Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "B"
        )));

        let failure = b"print('PREFIX')\nfail('boom')\n";
        let failed = observed_repo(&dice, tracker.dupe(), repo_epoch(Some(failure), 3)).await;
        assert!(matches!(
            failed,
            PathOutcome::Complete(value)
                if matches!(value.as_ref(), Err(HostRepoFileError::Evaluation { .. }))
        ));
        assert!(tracker.take().iter().any(|activation| matches!(
            activation.batch.as_ref().map(EventBatch::events),
            Some([
                EvaluationEvent::StarlarkPrint { text, .. },
                EvaluationEvent::Diagnostic {
                    level: EvaluationDiagnosticLevel::Error,
                    ..
                }
            ]) if text == "PREFIX"
        )));

        observed_repo(&dice, tracker.dupe(), repo_epoch(None, 4)).await;
        assert!(tracker.take().iter().any(|activation| matches!(
            activation.batch.as_ref(),
            Some(batch) if batch.events().is_empty()
        )));

        observed_repo(&dice, tracker.dupe(), repo_epoch(Some(source_a), 5)).await;
        assert!(tracker.take().iter().any(|activation| matches!(
            activation.batch.as_ref().map(EventBatch::events),
            Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "A"
        )));
    }
}
