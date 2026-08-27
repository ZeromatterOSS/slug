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
use std::ops::ControlFlow;
use std::sync::Arc;

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
use slug_starlark_v2::populate_universe;
use slug_workspace_v2::NormalizedAbsolutePath;
use slug_workspace_v2::ObservedPathFrontierError;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathOutcome;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::any::ProvidesStaticType;
use starlark::codemap::Span;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
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

use crate::NonrootModuleKey;
use crate::RootPackagePolicyProjectionError;
use crate::RootRepoFileSemanticsProjectionKey;
use crate::RootRepoFileUtf8Mode;
use crate::RootRepositoryRoute;
use crate::host_file::HostFileBytes;
use crate::host_file::HostFileBytesKey;
use crate::host_file::HostFileBytesObservationKey;
use crate::host_file::HostFileError;
use crate::host_file::ObservedHostFileBytes;
use crate::source_preparation::HostRepositorySourceFileKey;
use crate::source_preparation::HostRepositorySourceFileObservationKey;
use crate::source_preparation::HostRepositorySourceFileValue;
use crate::source_preparation::RepositorySourceFileError;
use crate::source_preparation::RepositorySourceFileKey;
use crate::source_preparation::RepositorySourceFileObservationKey;
use crate::source_preparation::RepositorySourceFileValue;
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
}

fn repo_globals() -> Globals {
    let mut builder = GlobalsBuilder::new();
    populate_universe(&mut builder);
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

enum HostRepoFileTerminal<'a> {
    Complete(Result<HostRepoFileValue, HostRepoFileError>),
    Evaluate {
        logical_path: &'a NormalizedAbsolutePath,
        bytes: &'a [u8],
        utf8_mode: RootRepoFileUtf8Mode,
    },
}

fn finalize_host_repo_file(
    ctx: &mut DiceComputations<'_>,
    capture_events: bool,
    terminal: HostRepoFileTerminal<'_>,
) -> Arc<Result<HostRepoFileValue, HostRepoFileError>> {
    let (value, batch) = match terminal {
        HostRepoFileTerminal::Complete(value) => (value, EventBatch::empty()),
        HostRepoFileTerminal::Evaluate {
            logical_path,
            bytes,
            utf8_mode,
        } => {
            let recording = capture_events.then(RecordingRepoEventReporter::default);
            let direct = DirectRepoEventReporter;
            let reporter: &dyn RepoEventReporter = match recording.as_ref() {
                Some(recording) => recording,
                None => &direct,
            };
            let value = evaluate_repo_file(logical_path, bytes, utf8_mode, reporter);
            let batch =
                recording.map_or_else(EventBatch::empty, RecordingRepoEventReporter::into_batch);
            (value, batch)
        }
    };
    if capture_events {
        ctx.store_evaluation_data(batch)
            .expect("Host REPO key stores exactly one event batch");
    }
    Arc::new(value)
}

fn observed_host_repo_terminal<'a>(
    logical_path: &'a NormalizedAbsolutePath,
    utf8_mode: RootRepoFileUtf8Mode,
    observed: &'a Result<ObservedHostFileBytes, ObservedPathFrontierError>,
) -> Result<(PathObservationEpoch, HostRepoFileTerminal<'a>), ObservedPathFrontierError> {
    let observed = observed.as_ref().map_err(|error| error.dupe())?;
    let observations = observed.observations().dupe();
    let terminal = match observed.result() {
        Err(error) => {
            HostRepoFileTerminal::Complete(Err(HostRepoFileError::HostFile(error.clone())))
        }
        Ok(HostFileBytes::Missing) => {
            HostRepoFileTerminal::Complete(Ok(HostRepoFileValue::empty()))
        }
        Ok(HostFileBytes::Present(bytes)) => HostRepoFileTerminal::Evaluate {
            logical_path,
            bytes,
            utf8_mode,
        },
    };
    Ok((observations, terminal))
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
                return PathOutcome::Complete(finalize_host_repo_file(
                    ctx,
                    capture_events,
                    HostRepoFileTerminal::Complete(Err(HostRepoFileError::PolicyProjection(error))),
                ));
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
                return PathOutcome::Complete(finalize_host_repo_file(
                    ctx,
                    capture_events,
                    HostRepoFileTerminal::Complete(Err(HostRepoFileError::HostFile(error))),
                ));
            }
            PathOutcome::Complete(Ok(HostFileBytes::Missing)) => {
                return PathOutcome::Complete(finalize_host_repo_file(
                    ctx,
                    capture_events,
                    HostRepoFileTerminal::Complete(Ok(HostRepoFileValue::empty())),
                ));
            }
            PathOutcome::Complete(Ok(HostFileBytes::Present(bytes))) => bytes,
        };
        PathOutcome::Complete(finalize_host_repo_file(
            ctx,
            capture_events,
            HostRepoFileTerminal::Evaluate {
                logical_path: &logical_path,
                bytes: &bytes,
                utf8_mode: semantics.utf8_mode,
            },
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRepoFile {
    result: Arc<Result<HostRepoFileValue, HostRepoFileError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRepoFile {
    pub(crate) fn result(&self) -> &Result<HostRepoFileValue, HostRepoFileError> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative, Dupe)]
pub(crate) struct HostRepoFileObservationKey {
    workspace: NormalizedAbsolutePath,
}

impl HostRepoFileObservationKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath) -> Self {
        Self { workspace }
    }
}

impl fmt::Display for HostRepoFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bzlmod-observed-host-repo-file:{}", self.workspace)
    }
}

#[async_trait]
impl Key for HostRepoFileObservationKey {
    type Value = PathOutcome<Result<ObservedHostRepoFile, ObservedPathFrontierError>>;

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
                return PathOutcome::Complete(Ok(ObservedHostRepoFile {
                    result: finalize_host_repo_file(
                        ctx,
                        capture_events,
                        HostRepoFileTerminal::Complete(Err(HostRepoFileError::PolicyProjection(
                            error,
                        ))),
                    ),
                    observations: PathObservationEpoch::empty(),
                }));
            }
        };
        let logical_path = NormalizedAbsolutePath::new(self.workspace.as_path().join("REPO.bazel"))
            .expect("joining a normalized absolute workspace remains absolute");
        let observed = dice_invariant(
            ctx.compute(&HostFileBytesObservationKey::new(logical_path.dupe()))
                .await,
        );
        let observed = match observed {
            PathOutcome::Need(need) => return PathOutcome::Need(need),
            PathOutcome::Complete(observed) => observed,
        };
        let (observations, terminal) =
            match observed_host_repo_terminal(&logical_path, semantics.utf8_mode, &observed) {
                Ok(value) => value,
                Err(error) => return PathOutcome::Complete(Err(error)),
            };
        PathOutcome::Complete(Ok(ObservedHostRepoFile {
            result: finalize_host_repo_file(ctx, capture_events, terminal),
            observations,
        }))
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryRepoFileKey {
    workspace: NormalizedAbsolutePath,
    module: NonrootModuleKey,
}

impl HostNonregistryRepoFileKey {
    pub(crate) fn new(workspace: NormalizedAbsolutePath, module: NonrootModuleKey) -> Self {
        Self { workspace, module }
    }
}

impl fmt::Display for HostNonregistryRepoFileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "host-nonregistry-repo-file:{}@{}",
            self.module.name, self.module.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostNonregistryRepoFileObservationKey(pub(crate) HostNonregistryRepoFileKey);

impl fmt::Display for HostNonregistryRepoFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostNonregistryRepoFile {
    result: Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostNonregistryRepoFile {
    pub(crate) fn result(&self) -> &Arc<Result<HostRepoFileValue, HostRouteRepoFileError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Clone, Copy)]
enum HostNonregistryRepoFileMode {
    Legacy,
    Observed,
}

type HostNonregistryRepoFileProjection = (
    Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>,
    PathObservationEpoch,
);
type HostNonregistryRepoFileDriverOutcome =
    SourcePreparationOutcome<Result<HostNonregistryRepoFileProjection, ObservedPathFrontierError>>;

type HostNonregistryRepoSourceOutcome = SourcePreparationOutcome<
    Result<
        (
            Result<RepositorySourceFileValue, RepositorySourceFileError>,
            PathObservationEpoch,
        ),
        ObservedPathFrontierError,
    >,
>;

fn finish_nonregistry_repo_source(
    outcome: HostNonregistryRepoSourceOutcome,
) -> ControlFlow<
    HostNonregistryRepoFileDriverOutcome,
    (
        Result<RepositorySourceFileValue, RepositorySourceFileError>,
        PathObservationEpoch,
    ),
> {
    match outcome {
        SourcePreparationOutcome::Need(need) => {
            ControlFlow::Break(SourcePreparationOutcome::Need(need))
        }
        SourcePreparationOutcome::Complete(Err(error)) => {
            ControlFlow::Break(SourcePreparationOutcome::Complete(Err(error)))
        }
        SourcePreparationOutcome::Complete(Ok(source)) => ControlFlow::Continue(source),
    }
}

fn project_nonregistry_repo_legacy(
    outcome: HostNonregistryRepoFileDriverOutcome,
) -> SourcePreparationOutcome<Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>> {
    match outcome {
        SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
        SourcePreparationOutcome::Complete(Ok((result, _))) => {
            SourcePreparationOutcome::Complete(result)
        }
        SourcePreparationOutcome::Complete(Err(_)) => {
            unreachable!("legacy nonregistry REPO cannot produce an observed outer error")
        }
    }
}

fn nonregistry_repo_complete(
    ctx: &mut DiceComputations<'_>,
    capture_events: bool,
    result: Result<HostRepoFileValue, HostRouteRepoFileError>,
    observations: PathObservationEpoch,
    batch: EventBatch,
) -> HostNonregistryRepoFileDriverOutcome {
    store_route_repo_batch(ctx, capture_events, batch);
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

async fn drive_host_nonregistry_repo_file(
    ctx: &mut DiceComputations<'_>,
    key: &HostNonregistryRepoFileKey,
    mode: HostNonregistryRepoFileMode,
) -> HostNonregistryRepoFileDriverOutcome {
    let capture_events = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok();
    let source_key = RepositorySourceFileKey {
        workspace: key.workspace.as_path().to_owned(),
        module_name: key.module.name.clone(),
        repo_relative_path: "REPO.bazel".into(),
    };
    let (source, observations) = match mode {
        HostNonregistryRepoFileMode::Legacy => match dice_invariant(ctx.compute(&source_key).await)
        {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(source) => (
                match source.as_ref() {
                    Ok(value) => Ok(value.dupe()),
                    Err(error) => Err(error.dupe()),
                },
                PathObservationEpoch::empty(),
            ),
        },
        HostNonregistryRepoFileMode::Observed => {
            let outcome = match dice_invariant(
                ctx.compute(&RepositorySourceFileObservationKey(source_key))
                    .await,
            ) {
                SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
                SourcePreparationOutcome::Complete(Err(error)) => {
                    SourcePreparationOutcome::Complete(Err(error))
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => {
                    SourcePreparationOutcome::Complete(Ok((
                        observed.result().as_ref().clone(),
                        observed.observations().dupe(),
                    )))
                }
            };
            match finish_nonregistry_repo_source(outcome) {
                ControlFlow::Continue(source) => source,
                ControlFlow::Break(outcome) => return outcome,
            }
        }
    };
    let source = match source {
        Ok(RepositorySourceFileValue::Absent) => {
            return nonregistry_repo_complete(
                ctx,
                capture_events,
                Ok(HostRepoFileValue::empty()),
                observations,
                EventBatch::empty(),
            );
        }
        Ok(RepositorySourceFileValue::Present(bytes)) => bytes,
        Err(error) => {
            return nonregistry_repo_complete(
                ctx,
                capture_events,
                Err(HostRouteRepoFileError::Source(error)),
                observations,
                EventBatch::empty(),
            );
        }
    };
    let semantics = match dice_invariant(
        ctx.compute(&RootRepoFileSemanticsProjectionKey::new(
            key.workspace.dupe(),
        ))
        .await,
    ) {
        Ok(semantics) => semantics,
        Err(error) => {
            return nonregistry_repo_complete(
                ctx,
                capture_events,
                Err(HostRouteRepoFileError::PolicyProjection(error)),
                observations,
                EventBatch::empty(),
            );
        }
    };
    let logical_path = NormalizedAbsolutePath::new(
        key.workspace
            .as_path()
            .join(".slug-nonregistry")
            .join(key.module.name.as_str())
            .join("REPO.bazel"),
    )
    .expect("joining a normalized workspace remains absolute");
    let recording = capture_events.then(RecordingRepoEventReporter::default);
    let direct = DirectRepoEventReporter;
    let reporter: &dyn RepoEventReporter = recording
        .as_ref()
        .map_or(&direct, |recording| recording as &dyn RepoEventReporter);
    let result = evaluate_repo_file(&logical_path, &source, semantics.utf8_mode, reporter)
        .map_err(HostRouteRepoFileError::Evaluation);
    nonregistry_repo_complete(
        ctx,
        capture_events,
        result,
        observations,
        recording.map_or_else(EventBatch::empty, RecordingRepoEventReporter::into_batch),
    )
}

#[async_trait]
impl Key for HostNonregistryRepoFileKey {
    type Value = SourcePreparationOutcome<Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        project_nonregistry_repo_legacy(
            drive_host_nonregistry_repo_file(ctx, self, HostNonregistryRepoFileMode::Legacy).await,
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[async_trait]
impl Key for HostNonregistryRepoFileObservationKey {
    type Value = SourcePreparationOutcome<
        Result<ObservedHostNonregistryRepoFile, ObservedPathFrontierError>,
    >;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_nonregistry_repo_file(ctx, &self.0, HostNonregistryRepoFileMode::Observed)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedHostNonregistryRepoFile {
                    result,
                    observations,
                })
            })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct HostRouteRepoFileObservationKey(pub(crate) HostRouteRepoFileKey);

impl fmt::Display for HostRouteRepoFileObservationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "observed-{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct ObservedHostRouteRepoFile {
    result: Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>,
    observations: PathObservationEpoch,
}

impl ObservedHostRouteRepoFile {
    pub(crate) fn result(&self) -> &Arc<Result<HostRepoFileValue, HostRouteRepoFileError>> {
        &self.result
    }

    pub(crate) fn observations(&self) -> &PathObservationEpoch {
        &self.observations
    }
}

#[derive(Clone, Copy)]
enum HostRouteRepoFileMode {
    Legacy,
    Observed,
}

type HostRouteRepoFileProjection = (
    Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>,
    PathObservationEpoch,
);
type HostRouteRepoFileDriverOutcome =
    SourcePreparationOutcome<Result<HostRouteRepoFileProjection, ObservedPathFrontierError>>;

fn store_route_repo_batch(ctx: &mut DiceComputations<'_>, capture_events: bool, batch: EventBatch) {
    if capture_events {
        ctx.store_evaluation_data(batch)
            .expect("routed REPO key stores exactly one event batch");
    }
}

fn route_repo_complete(
    ctx: &mut DiceComputations<'_>,
    capture_events: bool,
    result: Result<HostRepoFileValue, HostRouteRepoFileError>,
    observations: PathObservationEpoch,
    batch: EventBatch,
) -> HostRouteRepoFileDriverOutcome {
    store_route_repo_batch(ctx, capture_events, batch);
    SourcePreparationOutcome::Complete(Ok((Arc::new(result), observations)))
}

async fn drive_host_route_repo_file(
    ctx: &mut DiceComputations<'_>,
    key: &HostRouteRepoFileKey,
    mode: HostRouteRepoFileMode,
) -> HostRouteRepoFileDriverOutcome {
    let capture_events = ctx
        .per_transaction_data()
        .data
        .get::<CaptureEvaluationEvents>()
        .is_ok();
    let semantics = match dice_invariant(
        ctx.compute(&RootRepoFileSemanticsProjectionKey::new(
            key.route.workspace().dupe(),
        ))
        .await,
    ) {
        Ok(semantics) => semantics,
        Err(error) => {
            return route_repo_complete(
                ctx,
                capture_events,
                Err(HostRouteRepoFileError::PolicyProjection(error)),
                PathObservationEpoch::empty(),
                EventBatch::empty(),
            );
        }
    };
    let source_key = HostRepositorySourceFileKey::new(key.route.clone(), "REPO.bazel".into());
    let (source, observations) = match mode {
        HostRouteRepoFileMode::Legacy => match dice_invariant(ctx.compute(&source_key).await) {
            SourcePreparationOutcome::Need(need) => return SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(source) => (source, PathObservationEpoch::empty()),
        },
        HostRouteRepoFileMode::Observed => {
            let observed = match dice_invariant(
                ctx.compute(&HostRepositorySourceFileObservationKey::new(
                    key.route.clone(),
                    "REPO.bazel".into(),
                ))
                .await,
            ) {
                SourcePreparationOutcome::Need(need) => {
                    return SourcePreparationOutcome::Need(need);
                }
                SourcePreparationOutcome::Complete(Err(error)) => {
                    return SourcePreparationOutcome::Complete(Err(error));
                }
                SourcePreparationOutcome::Complete(Ok(observed)) => observed,
            };
            (
                observed.result().as_ref().clone(),
                observed.observations().dupe(),
            )
        }
    };
    let (bytes, logical_path) = match source {
        Ok(HostRepositorySourceFileValue::Absent) => {
            return route_repo_complete(
                ctx,
                capture_events,
                Ok(HostRepoFileValue::empty()),
                observations,
                EventBatch::empty(),
            );
        }
        Ok(HostRepositorySourceFileValue::Present {
            bytes,
            logical_path,
        }) => (bytes, logical_path),
        Err(error) => {
            return route_repo_complete(
                ctx,
                capture_events,
                Err(HostRouteRepoFileError::Source(error)),
                observations,
                EventBatch::empty(),
            );
        }
    };
    let recording = capture_events.then(RecordingRepoEventReporter::default);
    let direct = DirectRepoEventReporter;
    let reporter: &dyn RepoEventReporter = recording
        .as_ref()
        .map_or(&direct, |recording| recording as &dyn RepoEventReporter);
    let result = evaluate_repo_file(&logical_path, &bytes, semantics.utf8_mode, reporter)
        .map_err(HostRouteRepoFileError::Evaluation);
    route_repo_complete(
        ctx,
        capture_events,
        result,
        observations,
        recording.map_or_else(EventBatch::empty, RecordingRepoEventReporter::into_batch),
    )
}

#[async_trait]
impl Key for HostRouteRepoFileKey {
    type Value = SourcePreparationOutcome<Arc<Result<HostRepoFileValue, HostRouteRepoFileError>>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        match drive_host_route_repo_file(ctx, self, HostRouteRepoFileMode::Legacy).await {
            SourcePreparationOutcome::Need(need) => SourcePreparationOutcome::Need(need),
            SourcePreparationOutcome::Complete(Ok((result, _))) => {
                SourcePreparationOutcome::Complete(result)
            }
            SourcePreparationOutcome::Complete(Err(_)) => {
                unreachable!("legacy routed REPO cannot produce an observed outer error")
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
impl Key for HostRouteRepoFileObservationKey {
    type Value =
        SourcePreparationOutcome<Result<ObservedHostRouteRepoFile, ObservedPathFrontierError>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        drive_host_route_repo_file(ctx, &self.0, HostRouteRepoFileMode::Observed)
            .await
            .map(|outcome| {
                outcome.map(|(result, observations)| ObservedHostRouteRepoFile {
                    result,
                    observations,
                })
            })
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::sync::Mutex;
    #[cfg(unix)]
    use std::sync::atomic::AtomicUsize;
    #[cfg(unix)]
    use std::sync::atomic::Ordering;

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
    use dice::Key;
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
    #[cfg(unix)]
    use slug_identity_v2::ApparentRepoName;
    #[cfg(unix)]
    use slug_identity_v2::CanonicalLabel;
    #[cfg(unix)]
    use slug_identity_v2::CanonicalRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    #[cfg(unix)]
    use slug_workspace_v2::ObservedPathFrontierError;
    #[cfg(unix)]
    use slug_workspace_v2::PathIoErrorKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathLstat;
    #[cfg(unix)]
    use slug_workspace_v2::PathNodeKind;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationDemand;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpoch;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochError;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationEpochKey;
    #[cfg(unix)]
    use slug_workspace_v2::PathObservationError;
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
    #[cfg(unix)]
    use starlark_map::small_map::SmallMap;

    use super::HostRepoFileError;
    #[cfg(unix)]
    use super::HostRepoFileObservationKey;
    use super::INVALID_UTF8;
    use super::INVALID_UTF8_ERROR_SUFFIX;
    #[cfg(unix)]
    use super::ObservedHostRepoFile;
    use super::RecordingRepoEventReporter;
    use super::RootRepoFileUtf8Mode;
    use super::evaluate_repo_file;
    #[cfg(unix)]
    use super::observed_host_repo_terminal;
    use super::repo_globals;
    #[cfg(unix)]
    use crate::OverrideAttributeValue;
    #[cfg(unix)]
    use crate::RepoSpec;
    #[cfg(unix)]
    use crate::RootPackagePolicyInputs;
    #[cfg(unix)]
    use crate::RootRepositoryRoute;
    #[cfg(unix)]
    use crate::SourcePreparationOutcome;
    #[cfg(unix)]
    use crate::inject_root_module_request_inputs;
    #[cfg(unix)]
    use crate::inject_root_package_policy_inputs;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationEpochEntry;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationKind;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationRequest;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationRequestId;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResult;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpoch;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationResultEpochKey;
    #[cfg(unix)]
    use crate::source_preparation::RepositoryMaterializationSuccess;

    #[cfg(unix)]
    pub(crate) fn routed_policy_route() -> RootRepositoryRoute {
        RootRepositoryRoute::for_test(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            ApparentRepoName::new("dep_alias").unwrap(),
            "dep".into(),
            CanonicalRepoName::new("dep+").unwrap(),
            RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                    )
                    .unwrap(),
                    rule_name: "local_repository".into(),
                },
                attributes: Arc::new(SmallMap::from_iter([(
                    CompactString::new("path"),
                    OverrideAttributeValue::String("dep".into()),
                )])),
            },
        )
    }

    #[cfg(unix)]
    pub(crate) fn routed_policy_epoch(
        repo: Option<(&'static [u8], PathNodeKind)>,
        ignore: Option<(&'static [u8], PathNodeKind)>,
        variant: i64,
    ) -> PathObservationEpoch {
        let lstat = |kind| PathLstat::new(kind, variant, variant, variant, variant, 0o755);
        let demand = |path, operation| {
            PathObservationDemand::new(PathObservationNamespace::Host, path, operation)
        };
        let mut entries = ["/", "/workspace", "/workspace/dep"]
            .map(|path| {
                (
                    demand(
                        NormalizedAbsolutePath::new(path).unwrap(),
                        PathObservationOperation::Lstat,
                    ),
                    PathObservationResult::Lstat(PathOperationResult::Present(lstat(
                        PathNodeKind::Directory,
                    ))),
                )
            })
            .into_iter()
            .collect::<Vec<_>>();
        for (name, source) in [("REPO.bazel", repo), (".bazelignore", ignore)] {
            let path = NormalizedAbsolutePath::new(format!("/workspace/dep/{name}")).unwrap();
            entries.push((
                demand(path.dupe(), PathObservationOperation::Lstat),
                PathObservationResult::Lstat(match source {
                    Some((_, kind)) => PathOperationResult::Present(lstat(kind)),
                    None => PathOperationResult::Missing,
                }),
            ));
            if let Some((source, kind)) = source
                && kind != PathNodeKind::Directory
            {
                entries.push((
                    demand(path, PathObservationOperation::FileBytes),
                    PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                        source,
                    ))),
                ));
            }
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    #[cfg(unix)]
    pub(crate) async fn routed_policy_transaction(
        dice: &Arc<Dice>,
        tracker: Arc<dyn ActivationTracker>,
        epoch: PathObservationEpoch,
        inject_policy: bool,
    ) -> dice::DiceTransaction {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        if inject_policy {
            inject_root_package_policy_inputs(
                &mut updater,
                RootPackagePolicyInputs::new(
                    workspace.dupe(),
                    Arc::from([workspace.dupe()]),
                    std::iter::empty::<&str>(),
                    None,
                    Some("warning"),
                )
                .unwrap(),
            )
            .unwrap();
        }
        let route = routed_policy_route();
        let request = Arc::new(RepositoryMaterializationRequest {
            id: RepositoryMaterializationRequestId {
                workspace: workspace.dupe(),
                canonical_repo: route.canonical_repo().clone(),
            },
            repo_spec: route.repo_spec().clone(),
            kind: RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/workspace/dep").unwrap(),
            },
        });
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
                    [RepositoryMaterializationEpochEntry {
                        request,
                        result: RepositoryMaterializationResult::Success(
                            RepositoryMaterializationSuccess::Local,
                        ),
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    async fn compute_observed_routed_repo(
        dice: &Arc<Dice>,
        tracker: &Arc<ObservedRepoEventTracker>,
        epoch: PathObservationEpoch,
    ) -> SourcePreparationOutcome<Result<super::ObservedHostRouteRepoFile, ObservedPathFrontierError>>
    {
        let mut transaction = routed_policy_transaction(
            dice,
            tracker.dupe() as Arc<dyn ActivationTracker>,
            epoch,
            true,
        )
        .await;
        transaction
            .compute(&super::HostRouteRepoFileObservationKey(
                super::HostRouteRepoFileKey::new(routed_policy_route()),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_routed_repo_retains_terminals_events_and_a_b_a() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let wrong_epoch = routed_policy_epoch(
            Some((b"", PathNodeKind::Directory)),
            Some((b"", PathNodeKind::RegularFile)),
            70,
        );
        let SourcePreparationOutcome::Complete(Ok(wrong)) =
            compute_observed_routed_repo(&dice, &tracker, wrong_epoch.dupe()).await
        else {
            panic!("wrong-kind routed REPO must retain a carrier");
        };
        assert!(matches!(
            wrong.result().as_ref(),
            Err(super::HostRouteRepoFileError::Source(_))
        ));
        assert!(
            !wrong
                .observations()
                .observations()
                .keys()
                .any(|demand| demand.path().as_path().ends_with(".bazelignore"))
        );
        let failed_epoch = routed_policy_epoch(
            Some((
                b"print('PREFIX')\nfail('boom')\n",
                PathNodeKind::RegularFile,
            )),
            None,
            71,
        );
        let SourcePreparationOutcome::Complete(Ok(failed)) =
            compute_observed_routed_repo(&dice, &tracker, failed_epoch).await
        else {
            panic!("routed REPO evaluation failure must retain a carrier");
        };
        assert!(matches!(
            failed.result().as_ref(),
            Err(super::HostRouteRepoFileError::Evaluation { .. })
        ));
        assert!(tracker.take().iter().any(|activation| matches!(
            activation.batch.as_ref().map(EventBatch::events),
            Some([
                EvaluationEvent::StarlarkPrint { text, .. },
                EvaluationEvent::Diagnostic { .. }
            ]) if text == "PREFIX"
        )));

        const A: &[u8] = b"print('A')\nignore_directories(['a'])\n";
        const B: &[u8] = b"print('B')\nignore_directories(['b'])\n";
        let a = routed_policy_epoch(Some((A, PathNodeKind::RegularFile)), None, 72);
        let b = routed_policy_epoch(Some((B, PathNodeKind::RegularFile)), None, 73);
        let mut first = None;
        for (epoch, expected) in [
            (a.dupe(), "A"),
            (b, "B"),
            (routed_policy_epoch(None, None, 74), ""),
            (a.dupe(), "A"),
        ] {
            let SourcePreparationOutcome::Complete(Ok(value)) =
                compute_observed_routed_repo(&dice, &tracker, epoch).await
            else {
                panic!("routed REPO lifecycle must complete");
            };
            if expected.is_empty() {
                assert_empty_batches(tracker.take());
            } else {
                assert_print(tracker.take(), expected);
            }
            first.get_or_insert_with(|| value.dupe());
            if expected == "A" {
                assert_eq!(
                    value.result().as_ref(),
                    first.as_ref().unwrap().result().as_ref()
                );
            }
        }
    }

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
    fn repo_uses_the_shared_set_builtin() {
        let (value, events) = evaluate(
            b"ignore_directories(sorted(set(['b', 'a', 'b'])))",
            RootRepoFileUtf8Mode::Warning,
        );
        assert_eq!(value.unwrap().ignored_directories(), ["a", "b"]);
        assert!(events.is_empty());
        for source in [b"set(elements = [])".as_slice(), b"set([], [])"] {
            assert!(matches!(
                evaluate(source, RootRepoFileUtf8Mode::Warning).0,
                Err(HostRepoFileError::Evaluation { .. })
            ));
        }
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
    #[derive(Default)]
    struct ObservedRepoEventTracker {
        activations: Mutex<Vec<RepoActivation>>,
        legacy_repo: AtomicUsize,
        legacy_host_file: AtomicUsize,
        observed_host_file: AtomicUsize,
        upper: AtomicUsize,
        rows: Mutex<Vec<(String, Vec<String>)>>,
    }

    #[cfg(unix)]
    impl ObservedRepoEventTracker {
        fn take(&self) -> Vec<RepoActivation> {
            std::mem::take(&mut *self.activations.lock().unwrap())
        }

        fn take_rows(&self) -> Vec<(String, Vec<String>)> {
            std::mem::take(&mut *self.rows.lock().unwrap())
        }

        fn assert_legacy_keys_inactive(&self) {
            assert_eq!(self.legacy_repo.load(Ordering::SeqCst), 0);
            assert_eq!(self.legacy_host_file.load(Ordering::SeqCst), 0);
        }
    }

    #[cfg(unix)]
    fn assert_no_batches(activations: Vec<RepoActivation>) {
        assert!(
            activations
                .iter()
                .all(|activation| activation.batch.is_none())
        );
    }

    #[cfg(unix)]
    fn assert_empty_batches(activations: Vec<RepoActivation>) {
        assert_eq!(activations.len(), 1);
        assert_eq!(activations[0].kind, ActivationKind::Evaluated);
        assert!(matches!(&activations[0].batch, Some(batch) if batch.events().is_empty()));
    }

    #[cfg(unix)]
    fn assert_print(activations: Vec<RepoActivation>, expected: &str) {
        assert!(matches!(
            activations.as_slice(),
            [RepoActivation {
                kind: ActivationKind::Evaluated,
                batch: Some(batch),
            }] if matches!(
                batch.events(),
                [EvaluationEvent::StarlarkPrint { text, .. }] if text == expected
            )
        ));
    }

    #[cfg(unix)]
    fn assert_nonregistry_rows(
        tracker: &ObservedRepoEventTracker,
        observed_key: &super::HostNonregistryRepoFileObservationKey,
        legacy_key: &super::HostNonregistryRepoFileKey,
        semantics: bool,
        observed: bool,
        legacy: bool,
    ) {
        let deps = |observed| {
            let source = format!(
                "{}repository-source-file:dep:REPO.bazel",
                if observed { "observed-" } else { "" }
            );
            [source]
                .into_iter()
                .chain(semantics.then(|| "root-repo-file-semantics:\"/workspace\"".to_owned()))
                .collect()
        };
        let mut expected = Vec::new();
        if observed {
            expected.push((observed_key.to_string(), deps(true)));
        }
        if legacy {
            expected.push((legacy_key.to_string(), deps(false)));
        }
        assert_eq!(tracker.take_rows(), expected);
    }

    #[cfg(unix)]
    fn assert_epoch_ptrs(expected: &PathObservationEpoch, actual: &PathObservationEpoch) {
        assert_eq!(expected, actual);
        for (demand, result) in expected.observations() {
            assert!(Arc::ptr_eq(result, actual.get(demand).unwrap()));
        }
    }

    #[cfg(unix)]
    impl ActivationTracker for ObservedRepoEventTracker {
        fn key_activated(
            &self,
            key: &DynKey,
            deps: &mut dyn Iterator<Item = &DynKey>,
            _activation: ActivationData,
        ) {
            let name = key.to_string();
            if [
                "host-nonregistry-repository-ignore:",
                "host-nonregistry-package-preflight:",
                "host-nonregistry-module-closure:",
                "module-source-preparation:",
                "host-discovered-module:",
                "host-selected-module-graph:",
                "registry-file:",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            {
                self.upper.fetch_add(1, Ordering::SeqCst);
            }
            if name.starts_with("host-nonregistry-repo-file:")
                || name.starts_with("observed-host-nonregistry-repo-file:")
            {
                self.rows
                    .lock()
                    .unwrap()
                    .push((name, deps.map(ToString::to_string).collect()));
            }
        }

        fn tracks_rich_activations(&self) -> bool {
            true
        }

        fn key_activated_rich(&self, key: &DynKey, activation: RichActivation<'_>) {
            if key.downcast_ref::<HostRepoFileObservationKey>().is_some()
                || key
                    .downcast_ref::<super::HostRouteRepoFileObservationKey>()
                    .is_some()
                || key
                    .downcast_ref::<super::HostNonregistryRepoFileObservationKey>()
                    .is_some()
                || key
                    .downcast_ref::<super::HostNonregistryRepoFileKey>()
                    .is_some()
            {
                self.activations.lock().unwrap().push(RepoActivation {
                    kind: activation.kind(),
                    batch: activation
                        .evaluation_data()
                        .and_then(|data| data.downcast_ref::<EventBatch>())
                        .map(Dupe::dupe),
                });
            } else if key
                .downcast_ref::<super::RepositorySourceFileObservationKey>()
                .is_some()
                || key
                    .downcast_ref::<super::RepositorySourceFileKey>()
                    .is_some()
            {
                assert!(activation.evaluation_data().is_none());
            } else if key.downcast_ref::<super::HostRepoFileKey>().is_some() {
                self.legacy_repo.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<crate::host_file::HostFileBytesKey>()
                .is_some()
            {
                self.legacy_host_file.fetch_add(1, Ordering::SeqCst);
            } else if key
                .downcast_ref::<crate::host_file::HostFileBytesObservationKey>()
                .is_some()
            {
                self.observed_host_file.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[cfg(unix)]
    async fn observed_repo_frontier(
        dice: &Arc<Dice>,
        tracker: Arc<ObservedRepoEventTracker>,
        epoch: PathObservationEpoch,
        capture_events: bool,
        inject_policy: bool,
    ) -> PathOutcome<Result<ObservedHostRepoFile, ObservedPathFrontierError>> {
        let mut user_data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        if capture_events {
            user_data.data.set(CaptureEvaluationEvents);
        }
        let mut updater = dice.updater_with_data(user_data);
        if inject_policy {
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
        }
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        let mut transaction = updater.commit().await;
        transaction
            .compute(&HostRepoFileObservationKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
            ))
            .await
            .unwrap()
    }

    #[cfg(unix)]
    fn repo_epoch_with(
        workspace: PathOperationResult<PathLstat>,
        repo: PathOperationResult<PathLstat>,
        bytes: Option<PathOperationResult<Arc<[u8]>>>,
    ) -> PathObservationEpoch {
        let lstat = |kind| PathLstat::new(kind, 11, 11, 11, 11, 0o755);
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
                PathObservationResult::Lstat(workspace),
            ),
            (
                demand("/workspace/REPO.bazel", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(repo),
            ),
        ];
        if let Some(bytes) = bytes {
            entries.push((
                demand("/workspace/REPO.bazel", PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(bytes),
            ));
        }
        PathObservationEpoch::new(entries).unwrap()
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
        assert_no_batches(tracker.take());

        let source_b = b"print('B')\nignore_directories(['b'])\n";
        observed_repo(&dice, tracker.dupe(), repo_epoch(Some(source_b), 2)).await;
        assert_print(tracker.take(), "B");

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
        assert_print(tracker.take(), "A");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_repo_file_retains_exact_frontier_events_and_private_dependencies() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let epoch = repo_epoch(Some(b"print('A')\nignore_directories(['a'])\n"), 20);
        let complete =
            observed_repo_frontier(&dice, tracker.dupe(), epoch.dupe(), true, true).await;
        assert!(HostRepoFileObservationKey::validity(&complete));
        let PathOutcome::Complete(Ok(observed)) = &complete else {
            panic!("observed REPO file must complete with a frontier carrier");
        };
        let value = observed.result().as_ref().unwrap();
        assert_eq!(value.ignored_directories(), ["a"]);
        for (demand, expected) in epoch.observations() {
            let retained = observed
                .observations()
                .get(demand)
                .expect("every consumed Host observation is retained");
            assert!(Arc::ptr_eq(expected, retained));
        }
        assert!(Arc::ptr_eq(&observed.result, &observed.result.dupe()));
        tracker.assert_legacy_keys_inactive();
        assert_eq!(tracker.observed_host_file.load(Ordering::SeqCst), 1);
        assert_print(tracker.take(), "A");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_repo_file_policy_need_and_outer_errors_publish_no_partial_state() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let policy = observed_repo_frontier(
            &dice,
            tracker.dupe(),
            PathObservationEpoch::empty(),
            true,
            false,
        )
        .await;
        let PathOutcome::Complete(Ok(policy)) = &policy else {
            panic!("missing policy is a completed semantic error");
        };
        assert!(matches!(
            policy.result(),
            Err(HostRepoFileError::PolicyProjection(_))
        ));
        assert!(policy.observations().observations().is_empty());
        tracker.assert_legacy_keys_inactive();
        assert_eq!(tracker.observed_host_file.load(Ordering::SeqCst), 0);
        assert!(matches!(
            tracker.take().as_slice(),
            [RepoActivation {
                batch: Some(batch),
                ..
            }] if batch.events().is_empty()
        ));

        let need_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let need_tracker = Arc::new(ObservedRepoEventTracker::default());
        let need = observed_repo_frontier(
            &need_dice,
            need_tracker.dupe(),
            PathObservationEpoch::empty(),
            true,
            true,
        )
        .await;
        assert!(matches!(need, PathOutcome::Need(_)));
        assert_no_batches(need_tracker.take());
        need_tracker.assert_legacy_keys_inactive();

        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path(),
            PathObservationOperation::FileBytes,
        );
        let lower: Result<crate::host_file::ObservedHostFileBytes, _> =
            Err(ObservedPathFrontierError::from(
                PathObservationEpochError::ConflictingDemand(demand.dupe()),
            ));
        let Err(error) =
            observed_host_repo_terminal(&path(), RootRepoFileUtf8Mode::Warning, &lower)
        else {
            panic!("outer frontier error must bypass semantic terminal construction");
        };
        assert!(matches!(error, ObservedPathFrontierError::Epoch(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_repo_file_preserves_all_legacy_semantic_terminals_without_capture() {
        let lstat = |kind| PathLstat::new(kind, 12, 12, 12, 12, 0o755);
        let io = || PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        };
        let cases = vec![
            ("missing", repo_epoch(None, 30)),
            (
                "wrong-kind",
                repo_epoch_with(
                    PathOperationResult::Present(lstat(PathNodeKind::Directory)),
                    PathOperationResult::Present(lstat(PathNodeKind::Directory)),
                    None,
                ),
            ),
            (
                "resolution-error",
                repo_epoch_with(
                    PathOperationResult::Error(io()),
                    PathOperationResult::Missing,
                    None,
                ),
            ),
            (
                "file-bytes-error",
                repo_epoch_with(
                    PathOperationResult::Present(lstat(PathNodeKind::Directory)),
                    PathOperationResult::Present(lstat(PathNodeKind::RegularFile)),
                    Some(PathOperationResult::Error(io())),
                ),
            ),
            ("parse", repo_epoch(Some(b"("), 31)),
            (
                "restricted",
                repo_epoch(Some(b"load('//:x.bzl', 'x')\n"), 32),
            ),
            ("evaluation", repo_epoch(Some(b"fail('boom')\n"), 33)),
            (
                "success",
                repo_epoch(Some(b"ignore_directories(['ok'])\n"), 34),
            ),
        ];
        for (name, epoch) in cases {
            let legacy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let legacy = observed_repo(
                &legacy_dice,
                Arc::new(RepoEventTracker::default()),
                epoch.dupe(),
            )
            .await;
            let observed_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let tracker = Arc::new(ObservedRepoEventTracker::default());
            let observed =
                observed_repo_frontier(&observed_dice, tracker.dupe(), epoch.dupe(), false, true)
                    .await;
            let PathOutcome::Complete(legacy) = legacy else {
                panic!("legacy {name} unexpectedly needed observations");
            };
            let PathOutcome::Complete(Ok(observed)) = observed else {
                panic!("observed {name} unexpectedly lacked a complete carrier");
            };
            assert_eq!(legacy.as_ref(), observed.result(), "{name}");
            if name == "resolution-error" {
                assert_eq!(observed.observations().observations().len(), 2);
            }
            for (demand, retained) in observed.observations().observations() {
                assert!(Arc::ptr_eq(
                    epoch
                        .get(demand)
                        .expect("retained demand came from input epoch"),
                    retained,
                ));
            }
            tracker.assert_legacy_keys_inactive();
            assert_no_batches(tracker.take());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_repo_file_warm_and_a_b_a_event_lifecycle_is_exact() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let a = repo_epoch(Some(b"print('A')\nignore_directories(['a'])\n"), 40);
        let b = repo_epoch(Some(b"print('B')\nignore_directories(['b'])\n"), 41);
        let first = observed_repo_frontier(&dice, tracker.dupe(), a.dupe(), true, true).await;
        assert_print(tracker.take(), "A");

        let warm = observed_repo_frontier(&dice, tracker.dupe(), a.dupe(), true, true).await;
        assert!(HostRepoFileObservationKey::equality(&warm, &first));
        assert_no_batches(tracker.take());

        observed_repo_frontier(&dice, tracker.dupe(), b, true, true).await;
        assert_print(tracker.take(), "B");

        let restored = observed_repo_frontier(&dice, tracker.dupe(), a, true, true).await;
        assert!(HostRepoFileObservationKey::equality(&restored, &first));
        assert_print(tracker.take(), "A");
    }

    #[cfg(unix)]
    fn nonregistry_epoch(
        root: &str,
        source: Option<(&[u8], PathNodeKind)>,
        namespace: PathObservationNamespace,
        source_root: &str,
        variant: i64,
    ) -> PathObservationEpoch {
        let lstat = |kind| {
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, variant, variant, variant, variant, 0o755,
            )))
        };
        let demand = |namespace, path: &str, operation| {
            PathObservationDemand::new(
                namespace,
                NormalizedAbsolutePath::new(path).unwrap(),
                operation,
            )
        };
        let mut entries = SmallMap::new();
        for path in ["/", "/workspace"] {
            entries.insert(
                demand(
                    PathObservationNamespace::Host,
                    path,
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::Directory),
            );
        }
        entries.insert(
            demand(
                PathObservationNamespace::Host,
                "/workspace/MODULE.bazel",
                PathObservationOperation::Lstat,
            ),
            lstat(PathNodeKind::RegularFile),
        );
        entries.insert(
            demand(
                PathObservationNamespace::Host,
                "/workspace/MODULE.bazel",
                PathObservationOperation::FileBytes,
            ),
            PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
                root.as_bytes(),
            ))),
        );
        let source_path = format!("{source_root}/REPO.bazel");
        for ancestor in std::path::Path::new(&source_path).ancestors().skip(1) {
            entries.insert(
                demand(
                    namespace,
                    ancestor.to_str().unwrap(),
                    PathObservationOperation::Lstat,
                ),
                lstat(PathNodeKind::Directory),
            );
        }
        entries.insert(
            demand(namespace, &source_path, PathObservationOperation::Lstat),
            source.map_or(
                PathObservationResult::Lstat(PathOperationResult::Missing),
                |(_, kind)| lstat(kind),
            ),
        );
        if let Some((bytes, kind)) = source
            && kind == PathNodeKind::RegularFile
        {
            entries.insert(
                demand(namespace, &source_path, PathObservationOperation::FileBytes),
                PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(bytes))),
            );
        }
        PathObservationEpoch::new(entries).unwrap()
    }

    #[cfg(unix)]
    async fn nonregistry_transaction(
        dice: &Arc<Dice>,
        tracker: Arc<ObservedRepoEventTracker>,
        root: &str,
        epoch: PathObservationEpoch,
        result: Option<RepositoryMaterializationResult>,
        inject_policy: bool,
    ) -> dice::DiceTransaction {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        let mut updater = dice.updater_with_data(data);
        updater
            .changed_to(vec![(
                slug_workspace_v2::WorkspaceSnapshotKey {
                    workspace: workspace.as_path().to_owned(),
                },
                Arc::new(slug_workspace_v2::WorkspaceSnapshot {
                    files: Arc::new(starlark_map::sorted_map::SortedMap::from_iter([(
                        workspace.as_path().join("MODULE.bazel"),
                        slug_workspace_v2::WorkspaceFileValue::Present(Arc::new(root.to_owned())),
                    )])),
                }),
            )])
            .unwrap();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch)])
            .unwrap();
        inject_root_module_request_inputs(
            &mut updater,
            workspace.as_path(),
            crate::BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            crate::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            crate::LockfileMode::Off,
        )
        .unwrap();
        if inject_policy {
            inject_root_package_policy_inputs(
                &mut updater,
                RootPackagePolicyInputs::new(
                    workspace.dupe(),
                    [workspace.dupe()],
                    std::iter::empty::<&str>(),
                    None,
                    Some("warning"),
                )
                .unwrap(),
            )
            .unwrap();
        }
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(workspace.dupe(), []).unwrap(),
            )])
            .unwrap();
        let mut transaction = updater.commit().await;
        let pending = transaction
            .compute(&super::RepositorySourceFileObservationKey(
                super::RepositorySourceFileKey {
                    workspace: workspace.as_path().to_owned(),
                    module_name: "dep".into(),
                    repo_relative_path: "REPO.bazel".into(),
                },
            ))
            .await
            .unwrap();
        let SourcePreparationOutcome::Need(need) = pending else {
            panic!("missing injected result must expose the exact request")
        };
        let request = need
            .repository_materializations()
            .values()
            .next()
            .unwrap()
            .as_ref()
            .clone();
        let mut updater = transaction.into_updater();
        let Some(result) = result else {
            return updater.commit().await;
        };
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: workspace.dupe(),
                },
                RepositoryMaterializationResultEpoch::new(
                    workspace,
                    [RepositoryMaterializationEpochEntry {
                        request: Arc::new(request),
                        result,
                    }],
                )
                .unwrap(),
            )])
            .unwrap();
        updater.commit().await
    }

    #[cfg(unix)]
    fn observed_nonregistry(
        value: &SourcePreparationOutcome<
            Result<super::ObservedHostNonregistryRepoFile, ObservedPathFrontierError>,
        >,
    ) -> &super::ObservedHostNonregistryRepoFile {
        match value {
            SourcePreparationOutcome::Complete(Ok(value)) => value,
            _ => panic!("observed nonregistry REPO did not complete: {value:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn observed_nonregistry_repo_identity_and_child_outcome_polarity_are_exact() {
        let key =
            super::HostNonregistryRepoFileObservationKey(super::HostNonregistryRepoFileKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                crate::NonrootModuleKey::new("dep", "1"),
            ));
        assert_eq!(key.to_string(), "observed-host-nonregistry-repo-file:dep@1");
        let different =
            super::HostNonregistryRepoFileObservationKey(super::HostNonregistryRepoFileKey::new(
                NormalizedAbsolutePath::new("/workspace").unwrap(),
                crate::NonrootModuleKey::new("other", "1"),
            ));
        let mut identities = std::collections::HashSet::from([key.clone()]);
        assert!(!identities.insert(key.clone()) && identities.insert(different));
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/dep/REPO.bazel").unwrap(),
            PathObservationOperation::FileBytes,
        );
        let shared = Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Present(Arc::from(b"repo".as_slice())),
        ));
        let epoch = PathObservationEpoch::from_shared([(demand.dupe(), shared.dupe())]).unwrap();
        let continued =
            super::finish_nonregistry_repo_source(SourcePreparationOutcome::Complete(Ok((
                Ok(crate::source_preparation::RepositorySourceFileValue::Absent),
                epoch.dupe(),
            ))));
        let std::ops::ControlFlow::Continue((_, retained)) = continued else {
            panic!("complete source must continue")
        };
        assert!(Arc::ptr_eq(retained.get(&demand).unwrap(), &shared));
        let semantic = Arc::new(Ok(super::HostRepoFileValue::empty()));
        let projected = super::project_nonregistry_repo_legacy(SourcePreparationOutcome::Complete(
            Ok((semantic.dupe(), epoch)),
        ));
        let SourcePreparationOutcome::Complete(projected) = projected else {
            panic!("legacy projection must remain complete")
        };
        assert!(Arc::ptr_eq(&semantic, &projected));
        for error in [
            PathObservationEpochError::ConflictingDemand(demand.dupe()),
            PathObservationEpochError::OperationMismatch {
                demand,
                result_operation: PathObservationOperation::DirectoryEntries,
            },
        ] {
            let expected = ObservedPathFrontierError::from(error);
            let stopped = super::finish_nonregistry_repo_source(
                SourcePreparationOutcome::Complete(Err(expected.dupe())),
            )
            .break_value()
            .unwrap();
            let SourcePreparationOutcome::Complete(Err(actual)) = stopped else {
                panic!("outer source error must pass through")
            };
            assert_eq!(actual, expected);
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_nonregistry_repo_real_stops_are_carrierless_and_suppress_semantics() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let root = "module(name='root')\nlocal_path_override(module_name='dep',path='dep')\n";
        let key = super::HostNonregistryRepoFileKey::new(
            workspace.dupe(),
            crate::NonrootModuleKey::new("dep", "1"),
        );
        let observed_key = super::HostNonregistryRepoFileObservationKey(key.clone());
        let child_key = super::RepositorySourceFileObservationKey(super::RepositorySourceFileKey {
            workspace: workspace.as_path().to_owned(),
            module_name: "dep".into(),
            repo_relative_path: "REPO.bazel".into(),
        });
        let demand = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new("/workspace/dep/REPO.bazel").unwrap(),
            PathObservationOperation::FileBytes,
        );

        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let epoch = nonregistry_epoch(
            root,
            Some((b"print('A')\n", PathNodeKind::RegularFile)),
            PathObservationNamespace::Host,
            "/workspace/dep",
            90,
        );
        let mut need_tx =
            nonregistry_transaction(&dice, tracker.dupe(), root, epoch.dupe(), None, true).await;
        tracker.take();
        tracker.take_rows();
        let mut cancelled = Box::pin(need_tx.compute(&observed_key));
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(cancelled.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        drop(cancelled);
        assert_no_batches(tracker.take());
        let need = need_tx.compute(&observed_key).await.unwrap();
        assert!(!super::HostNonregistryRepoFileObservationKey::validity(
            &need
        ));
        assert!(!super::HostNonregistryRepoFileObservationKey::equality(
            &need, &need
        ));
        assert_no_batches(tracker.take());
        assert_eq!(
            tracker.take_rows(),
            [(observed_key.to_string(), vec![child_key.to_string()])]
        );
        let mut recovered = nonregistry_transaction(
            &dice,
            tracker.dupe(),
            root,
            epoch.dupe(),
            Some(RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Local,
            )),
            true,
        )
        .await;
        assert!(
            recovered
                .compute(&observed_key)
                .await
                .unwrap()
                .is_complete()
        );
        assert_print(tracker.take(), "A");

        let outer_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let outer_tracker = Arc::new(ObservedRepoEventTracker::default());
        let mut outer_data = UserComputationData {
            activation_tracker: Some(outer_tracker.dupe()),
            ..Default::default()
        };
        outer_data.data.set(CaptureEvaluationEvents);
        let mut outer_tx = outer_dice.updater_with_data(outer_data);
        outer_tx
            .changed_to(vec![(
                child_key,
                SourcePreparationOutcome::Complete(Err(ObservedPathFrontierError::from(
                    PathObservationEpochError::OperationMismatch {
                        demand,
                        result_operation: PathObservationOperation::DirectoryEntries,
                    },
                ))),
            )])
            .unwrap();
        let mut outer_tx = outer_tx.commit().await;
        let outer = outer_tx.compute(&observed_key).await.unwrap();
        assert!(
            super::HostNonregistryRepoFileObservationKey::validity(&outer)
                && super::HostNonregistryRepoFileObservationKey::equality(&outer, &outer)
                && matches!(outer, SourcePreparationOutcome::Complete(Err(_)))
        );
        assert_no_batches(outer_tracker.take());
        assert_eq!(
            outer_tracker.take_rows(),
            [(
                observed_key.to_string(),
                vec!["observed-repository-source-file:dep:REPO.bazel".to_owned()]
            )]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn observed_nonregistry_repo_preserves_families_prefixes_events_and_lifecycle() {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let policy_dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(ObservedRepoEventTracker::default());
        let root = "module(name='root')\nprint('ROOT')\nlocal_path_override(module_name='dep',path='dep')\n";
        let key = super::HostNonregistryRepoFileKey::new(
            NormalizedAbsolutePath::new("/workspace").unwrap(),
            crate::NonrootModuleKey::new("dep", "1"),
        );
        let observed_key = super::HostNonregistryRepoFileObservationKey(key.clone());
        let source_key =
            super::RepositorySourceFileObservationKey(super::RepositorySourceFileKey {
                workspace: "/workspace".into(),
                module_name: "dep".into(),
                repo_relative_path: "REPO.bazel".into(),
            });
        let (mut first, mut held, mut held_result, mut held_epoch) = (None, None, None, None);
        let a = b"print('A')\nignore_directories(['a'])\n".as_slice();
        let local_sources = [
            Some((a, PathNodeKind::RegularFile)),
            Some((a, PathNodeKind::RegularFile)),
            Some((
                b"print('B')\nignore_directories(['b'])\n".as_slice(),
                PathNodeKind::RegularFile,
            )),
            None,
            Some((b"".as_slice(), PathNodeKind::Directory)),
            Some((b"(".as_slice(), PathNodeKind::RegularFile)),
            Some((
                b"print('PREFIX')\nfail('boom')\n".as_slice(),
                PathNodeKind::RegularFile,
            )),
            Some((a, PathNodeKind::RegularFile)),
        ];
        for (index, source) in local_sources.into_iter().enumerate() {
            let epoch = nonregistry_epoch(
                root,
                source,
                PathObservationNamespace::Host,
                "/workspace/dep",
                [99, 100, 101, 102, 103, 104, 105, 100][index],
            );
            let mut transaction = nonregistry_transaction(
                if index == 0 { &policy_dice } else { &dice },
                tracker.dupe(),
                root,
                epoch.dupe(),
                Some(RepositoryMaterializationResult::Success(
                    RepositoryMaterializationSuccess::Local,
                )),
                index != 0,
            )
            .await;
            let source = transaction.compute(&source_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(source)) = source else {
                panic!("observed source must complete")
            };
            let expected = source.observations().dupe();
            tracker.take();
            tracker.take_rows();
            let value = transaction.compute(&observed_key).await.unwrap();
            let observed = observed_nonregistry(&value);
            assert_epoch_ptrs(&expected, observed.observations());
            match index {
                0 => {
                    assert!(matches!(
                        observed.result().as_ref(),
                        Err(super::HostRouteRepoFileError::PolicyProjection(
                            crate::RootPackagePolicyProjectionError::MissingInput { .. }
                        ))
                    ));
                    let observed_events = tracker.take();
                    assert_empty_batches(observed_events.clone());
                    let SourcePreparationOutcome::Complete(legacy) =
                        transaction.compute(&key).await.unwrap()
                    else {
                        panic!("legacy policy failure must complete")
                    };
                    assert_eq!(legacy.as_ref(), observed.result().as_ref());
                    assert_eq!(tracker.take(), observed_events);
                    assert_nonregistry_rows(&tracker, &observed_key, &key, true, true, true);
                }
                1 => {
                    first = Some(value.clone());
                    held = Some(observed.dupe());
                    held_result = Some(observed.result().dupe());
                    held_epoch = Some(observed.observations().dupe());
                    assert_print(tracker.take(), "A");
                    assert_nonregistry_rows(&tracker, &observed_key, &key, true, true, false);
                    let warm = transaction.compute(&observed_key).await.unwrap();
                    assert!(super::HostNonregistryRepoFileObservationKey::equality(
                        &value, &warm
                    ));
                    assert_no_batches(tracker.take());
                    tracker.take_rows();
                    let SourcePreparationOutcome::Complete(legacy) =
                        transaction.compute(&key).await.unwrap()
                    else {
                        panic!("legacy REPO must complete")
                    };
                    assert_eq!(legacy.as_ref(), observed.result().as_ref());
                    assert_print(tracker.take(), "A");
                    assert_nonregistry_rows(&tracker, &observed_key, &key, true, false, true);
                }
                3 | 4 => {
                    assert!(match (index, observed.result().as_ref()) {
                        (3, Ok(value)) => value.ignored_directories().is_empty(),
                        (
                            4,
                            Err(super::HostRouteRepoFileError::Source(
                                crate::source_preparation::RepositorySourceFileError::WrongKind {
                                    actual: PathNodeKind::Directory,
                                    ..
                                },
                            )),
                        ) => true,
                        _ => false,
                    });
                    assert_empty_batches(tracker.take());
                    if index == 4 {
                        let SourcePreparationOutcome::Complete(legacy) =
                            transaction.compute(&key).await.unwrap()
                        else {
                            panic!("legacy wrong-kind must complete")
                        };
                        assert_eq!(legacy.as_ref(), observed.result().as_ref());
                        assert_empty_batches(tracker.take());
                    }
                    assert_nonregistry_rows(&tracker, &observed_key, &key, false, true, index == 4);
                }
                5 | 6 => {
                    let observed_events = tracker.take();
                    assert_eq!(observed_events.len(), 1);
                    assert_eq!(observed_events[0].kind, ActivationKind::Evaluated);
                    let batch = observed_events[0].batch.as_ref().unwrap();
                    assert_eq!(batch.events().len(), index - 4);
                    assert!(matches!(
                        batch.events().last(),
                        Some(EvaluationEvent::Diagnostic {
                            level: EvaluationDiagnosticLevel::Error,
                            ..
                        })
                    ));
                    if index == 6 {
                        assert!(matches!(batch.events().first(), Some(
                            EvaluationEvent::StarlarkPrint { text, .. }) if text == "PREFIX"));
                    }
                    assert!(match (index, observed.result().as_ref()) {
                        (
                            5,
                            Err(super::HostRouteRepoFileError::Evaluation(
                                HostRepoFileError::Syntax { message, .. },
                            )),
                        ) => !message.is_empty(),
                        (
                            6,
                            Err(super::HostRouteRepoFileError::Evaluation(
                                HostRepoFileError::Evaluation { message, .. },
                            )),
                        ) => message.contains("boom"),
                        _ => false,
                    });
                    let SourcePreparationOutcome::Complete(legacy) =
                        transaction.compute(&key).await.unwrap()
                    else {
                        panic!("legacy REPO error must complete")
                    };
                    assert_eq!(legacy.as_ref(), observed.result().as_ref());
                    assert_eq!(tracker.take(), observed_events);
                    assert_nonregistry_rows(&tracker, &observed_key, &key, true, true, true);
                }
                7 => {
                    let held = held.as_ref().unwrap();
                    assert!(super::HostNonregistryRepoFileObservationKey::equality(
                        first.as_ref().unwrap(),
                        &value
                    ));
                    assert_eq!(held.result().as_ref(), observed.result().as_ref());
                    assert!(Arc::ptr_eq(held.result(), held_result.as_ref().unwrap()));
                    assert_epoch_ptrs(held_epoch.as_ref().unwrap(), held.observations());
                    tracker.take();
                }
                _ => {
                    tracker.take();
                }
            }
            tracker.take_rows();
        }

        immutable_lifecycle(&dice, tracker.dupe(), &observed_key, &source_key).await;
    }

    #[cfg(unix)]
    async fn immutable_lifecycle(
        dice: &Arc<Dice>,
        tracker: Arc<ObservedRepoEventTracker>,
        observed_key: &super::HostNonregistryRepoFileObservationKey,
        source_key: &super::RepositorySourceFileObservationKey,
    ) {
        let archive = "module(name='root')\narchive_override(module_name='dep',urls=['https://example.invalid/a.tgz'],integrity='sha256-x')\n";
        let instance = slug_workspace_v2::PathObservationInstanceId::new(7);
        let success =
            RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Immutable {
                source_identity: Arc::from("sha256-x"),
                generation_root: std::path::PathBuf::from("/immutable/7"),
                observation_instance: instance,
            });
        let (mut values, mut immutable_held) = (Vec::new(), None);
        let a = b"ignore_directories(['a'])".as_slice();
        let immutable_sources = [
            Some((a, PathNodeKind::RegularFile)),
            Some((
                b"ignore_directories(['b'])".as_slice(),
                PathNodeKind::RegularFile,
            )),
            None,
            Some((b"".as_slice(), PathNodeKind::Directory)),
            Some((a, PathNodeKind::RegularFile)),
        ];
        for (index, source) in immutable_sources.into_iter().enumerate() {
            let epoch = nonregistry_epoch(
                archive,
                source,
                PathObservationNamespace::Materialization(instance),
                "/immutable/7",
                [200, 201, 202, 203, 200][index],
            );
            let mut tx = nonregistry_transaction(
                &dice,
                tracker.dupe(),
                archive,
                epoch.dupe(),
                Some(success.clone()),
                true,
            )
            .await;
            let source = tx.compute(source_key).await.unwrap();
            let SourcePreparationOutcome::Complete(Ok(source)) = source else {
                panic!("observed immutable source must complete")
            };
            let expected = source.observations().dupe();
            let value = tx.compute(observed_key).await.unwrap();
            let observed = observed_nonregistry(&value);
            if index == 0 {
                immutable_held = Some((observed.result().dupe(), observed.observations().dupe()));
            }
            assert_epoch_ptrs(&expected, observed.observations());
            values.push(value);
            tracker.take();
            tracker.take_rows();
        }
        assert!(super::HostNonregistryRepoFileObservationKey::equality(
            &values[0], &values[4]
        ));
        assert!(!super::HostNonregistryRepoFileObservationKey::equality(
            &values[0], &values[1]
        ));
        let held = observed_nonregistry(&values[0]);
        let restored = observed_nonregistry(&values[4]);
        assert_eq!(held.result().as_ref(), restored.result().as_ref());
        let (held_result, held_epoch) = immutable_held.unwrap();
        assert!(Arc::ptr_eq(held.result(), &held_result));
        assert_epoch_ptrs(&held_epoch, held.observations());
        assert!(!held.observations().observations().is_empty());
        assert_eq!(tracker.upper.load(Ordering::SeqCst), 0);
    }
}
