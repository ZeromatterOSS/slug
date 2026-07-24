use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use anyhow::Context;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_workspace_v2::WorkspaceFileKey;
use slug_workspace_v2::WorkspaceFileValue;
use starlark::any::ProvidesStaticType;
use starlark::codemap::Span;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::ast::Argument;
use starlark::syntax::ast::AssignTarget;
use starlark::syntax::ast::AstExpr;
use starlark::syntax::ast::AstLiteral;
use starlark::syntax::ast::AstStmt;
use starlark::syntax::ast::Expr;
use starlark::syntax::ast::Stmt;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::tuple::TupleRef;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BzlmodCommandPolicyKey;
use crate::BzlmodEnvironmentPolicyKey;
use crate::LockfileMode;
use crate::LogicalModuleFileId;
use crate::LogicalSpan;
use crate::VisibleLockfileRead;
use crate::lockfile::bad_visible_lockfile_message;
use crate::lockfile::parse_visible_lockfile_content_for_mode;

/// A direct, literal `include()` request found while compiling one non-root
/// MODULE file. The later closure evaluator supplies and executes these files.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootIncludeRequest {
    pub path: CompactString,
    pub location: LogicalSpan,
}

/// Parser-backed syntax information for a single non-root MODULE file. Source
/// bytes and physical paths are intentionally not retained.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootModuleFileInspection {
    pub logical_id: LogicalModuleFileId,
    pub includes: Arc<[NonrootIncludeRequest]>,
}

/// Compile and inspect one non-root MODULE file. This deliberately does not
/// execute directives or compose an include closure.
pub fn inspect_nonroot_module_file(
    logical_id: LogicalModuleFileId,
    source: &[u8],
) -> anyhow::Result<NonrootModuleFileInspection> {
    let source = std::str::from_utf8(source).context("MODULE file is not valid UTF-8")?;
    let dialect = nonroot_module_dialect();
    let ast = AstModule::parse(logical_id.0.as_str(), source.to_owned(), &dialect)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut includes = Vec::new();
    let mut include_is_shadowed = false;

    inspect_nonroot_statement(
        ast.statement(),
        &ast,
        &logical_id,
        &mut include_is_shadowed,
        &mut includes,
    )?;

    Ok(NonrootModuleFileInspection {
        logical_id,
        includes: Arc::from(includes),
    })
}

fn nonroot_module_dialect() -> Dialect {
    let mut dialect = Dialect::Standard;
    dialect.enable_def = false;
    dialect.enable_lambda = false;
    dialect.enable_load = false;
    dialect.enable_top_level_stmt = false;
    dialect
}

fn inspect_nonroot_statement(
    statement: &AstStmt,
    ast: &AstModule,
    logical_id: &LogicalModuleFileId,
    include_is_shadowed: &mut bool,
    includes: &mut Vec<NonrootIncludeRequest>,
) -> anyhow::Result<()> {
    match &statement.node {
        Stmt::Statements(statements) => {
            for statement in statements {
                inspect_nonroot_statement(
                    statement,
                    ast,
                    logical_id,
                    include_is_shadowed,
                    includes,
                )?;
            }
        }
        Stmt::Expression(expression) => {
            if !inspect_direct_include(expression, ast, logical_id, *include_is_shadowed, includes)?
            {
                inspect_nonroot_expression(expression, *include_is_shadowed, false)?;
            }
        }
        Stmt::Assign(assignment) => {
            // The right hand side is evaluated before the assignment changes
            // `include`, so only then does the binding become shadowed.
            inspect_nonroot_expression(&assignment.rhs, *include_is_shadowed, false)?;
            inspect_assignment_target(&assignment.lhs, *include_is_shadowed, true)?;
            if matches!(&assignment.lhs.node, AssignTarget::Identifier(name) if name.node.ident == "include")
            {
                *include_is_shadowed = true;
            }
        }
        Stmt::AssignModify(lhs, _, rhs) => {
            inspect_nonroot_expression(rhs, *include_is_shadowed, false)?;
            inspect_assignment_target(lhs, *include_is_shadowed, true)?;
            if matches!(&lhs.node, AssignTarget::Identifier(name) if name.node.ident == "include") {
                *include_is_shadowed = true;
            }
        }
        // The parser dialect rejects these before this inspector runs. Keep
        // explicit errors here so a dialect regression cannot silently widen
        // the MODULE language.
        Stmt::Def(_) | Stmt::Load(_) | Stmt::If(_, _) | Stmt::IfElse(_, _) | Stmt::For(_) => {
            anyhow::bail!("restricted MODULE syntax is not permitted")
        }
        Stmt::Break | Stmt::Continue | Stmt::Pass | Stmt::Return(_) => {
            anyhow::bail!("restricted MODULE syntax is not permitted")
        }
    }
    Ok(())
}

fn inspect_direct_include(
    expression: &AstExpr,
    ast: &AstModule,
    logical_id: &LogicalModuleFileId,
    include_is_shadowed: bool,
    includes: &mut Vec<NonrootIncludeRequest>,
) -> anyhow::Result<bool> {
    let Expr::Call(callee, arguments) = &expression.node else {
        return Ok(false);
    };
    match &callee.node {
        Expr::Identifier(identifier)
            if identifier.node.ident == "include" && !include_is_shadowed =>
        {
            let [argument] = arguments.args.as_slice() else {
                anyhow::bail!("include() requires exactly one literal string argument")
            };
            let Argument::Positional(argument) = &argument.node else {
                anyhow::bail!("include() requires exactly one literal string argument")
            };
            let Expr::Literal(AstLiteral::String(path)) = &argument.node else {
                anyhow::bail!("include() requires exactly one literal string argument")
            };
            includes.push(NonrootIncludeRequest {
                path: CompactString::from(path.node.as_str()),
                location: logical_span(ast, logical_id, expression.span),
            });
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn inspect_assignment_target(
    target: &starlark::syntax::ast::AstAssignTarget,
    include_is_shadowed: bool,
    plain_include_assignment: bool,
) -> anyhow::Result<()> {
    match &target.node {
        AssignTarget::Identifier(identifier)
            if identifier.node.ident == "include" && !include_is_shadowed =>
        {
            if !plain_include_assignment {
                anyhow::bail!("include may only be used as a direct top-level call")
            }
        }
        AssignTarget::Identifier(_) => {}
        AssignTarget::Tuple(targets) => {
            for target in targets {
                inspect_assignment_target(target, include_is_shadowed, false)?;
            }
        }
        AssignTarget::Dot(receiver, _) => {
            inspect_nonroot_expression(receiver, include_is_shadowed, false)?;
        }
        AssignTarget::Index(values) => {
            inspect_nonroot_expression(&values.0, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.1, include_is_shadowed, false)?;
        }
    }
    Ok(())
}

fn inspect_nonroot_expression(
    expression: &AstExpr,
    include_is_shadowed: bool,
    direct_top_level_expression: bool,
) -> anyhow::Result<()> {
    match &expression.node {
        Expr::Tuple(values) | Expr::List(values) => {
            for value in values {
                inspect_nonroot_expression(value, include_is_shadowed, false)?;
            }
        }
        Expr::Dot(receiver, _)
        | Expr::Not(receiver)
        | Expr::Minus(receiver)
        | Expr::Plus(receiver)
        | Expr::BitNot(receiver) => {
            inspect_nonroot_expression(receiver, include_is_shadowed, false)?
        }
        Expr::Call(callee, arguments) => {
            let is_direct_builtin_include = direct_top_level_expression
                && !include_is_shadowed
                && matches!(&callee.node, Expr::Identifier(identifier) if identifier.node.ident == "include");
            if !is_direct_builtin_include {
                inspect_nonroot_expression(callee, include_is_shadowed, false)?;
            }
            for argument in &arguments.args {
                match &argument.node {
                    Argument::Args(_) => anyhow::bail!("*args is not permitted in MODULE files"),
                    Argument::KwArgs(value) => {
                        if !matches!(&value.node, Expr::Dict(_)) {
                            anyhow::bail!("**kwargs must be a literal dictionary in MODULE files")
                        }
                        inspect_nonroot_expression(value, include_is_shadowed, false)?;
                    }
                    Argument::Positional(value) | Argument::Named(_, value) => {
                        inspect_nonroot_expression(value, include_is_shadowed, false)?;
                    }
                }
            }
        }
        Expr::Index(values) => {
            inspect_nonroot_expression(&values.0, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.1, include_is_shadowed, false)?;
        }
        Expr::Index2(values) => {
            inspect_nonroot_expression(&values.0, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.1, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.2, include_is_shadowed, false)?;
        }
        Expr::Slice(receiver, start, end, step) => {
            inspect_nonroot_expression(receiver, include_is_shadowed, false)?;
            for value in [start.as_deref(), end.as_deref(), step.as_deref()]
                .into_iter()
                .flatten()
            {
                inspect_nonroot_expression(value, include_is_shadowed, false)?;
            }
        }
        Expr::Op(left, _, right) => {
            inspect_nonroot_expression(left, include_is_shadowed, false)?;
            inspect_nonroot_expression(right, include_is_shadowed, false)?;
        }
        Expr::If(values) => {
            inspect_nonroot_expression(&values.0, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.1, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.2, include_is_shadowed, false)?;
        }
        Expr::Dict(values) => {
            for (key, value) in values {
                inspect_nonroot_expression(key, include_is_shadowed, false)?;
                inspect_nonroot_expression(value, include_is_shadowed, false)?;
            }
        }
        Expr::ListComprehension(value, clause, clauses) => {
            inspect_assignment_target(&clause.var, include_is_shadowed, false)?;
            inspect_nonroot_expression(&clause.over, include_is_shadowed, false)?;
            for clause in clauses {
                match clause {
                    starlark::syntax::ast::Clause::For(clause) => {
                        inspect_assignment_target(&clause.var, include_is_shadowed, false)?;
                        inspect_nonroot_expression(&clause.over, include_is_shadowed, false)?
                    }
                    starlark::syntax::ast::Clause::If(condition) => {
                        inspect_nonroot_expression(condition, include_is_shadowed, false)?
                    }
                }
            }
            inspect_nonroot_expression(value, include_is_shadowed, false)?;
        }
        Expr::DictComprehension(values, clause, clauses) => {
            inspect_assignment_target(&clause.var, include_is_shadowed, false)?;
            inspect_nonroot_expression(&clause.over, include_is_shadowed, false)?;
            for clause in clauses {
                match clause {
                    starlark::syntax::ast::Clause::For(clause) => {
                        inspect_assignment_target(&clause.var, include_is_shadowed, false)?;
                        inspect_nonroot_expression(&clause.over, include_is_shadowed, false)?
                    }
                    starlark::syntax::ast::Clause::If(condition) => {
                        inspect_nonroot_expression(condition, include_is_shadowed, false)?
                    }
                }
            }
            inspect_nonroot_expression(&values.0, include_is_shadowed, false)?;
            inspect_nonroot_expression(&values.1, include_is_shadowed, false)?;
        }
        Expr::Lambda(_) => anyhow::bail!("lambda is not permitted in MODULE files"),
        Expr::FString(value) => {
            for expression in &value.expressions {
                inspect_nonroot_expression(expression, include_is_shadowed, false)?;
            }
        }
        Expr::Identifier(identifier)
            if identifier.node.ident == "include" && !include_is_shadowed =>
        {
            anyhow::bail!("include may only be used as a direct top-level call")
        }
        Expr::Identifier(_) | Expr::Literal(_) => {}
    }
    Ok(())
}

fn logical_span(ast: &AstModule, logical_id: &LogicalModuleFileId, span: Span) -> LogicalSpan {
    let span = ast.file_span(span).resolve_span();
    LogicalSpan {
        file: logical_id.clone(),
        start_line: u32::try_from(span.begin.line + 1).expect("Starlark line fits u32"),
        start_column: u32::try_from(span.begin.column + 1).expect("Starlark column fits u32"),
        end_line: u32::try_from(span.end.line + 1).expect("Starlark line fits u32"),
        end_column: u32::try_from(span.end.column + 1).expect("Starlark column fits u32"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleHeader {
    pub name: CompactString,
    pub version: Option<CompactString>,
    pub repo_name: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleDependency {
    pub name: CompactString,
    pub version: CompactString,
    pub repo_name: Option<CompactString>,
    pub nodep: bool,
    pub dev_dependency: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleOverrides(Arc<SmallMap<CompactString, RootModuleOverride>>);

impl RootModuleOverrides {
    pub fn get(&self, module_name: &str) -> Option<&RootModuleOverride> {
        self.0.get(module_name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&CompactString, &RootModuleOverride)> {
        self.0.iter()
    }
}

impl Default for RootModuleOverrides {
    fn default() -> Self {
        Self(Arc::new(SmallMap::new()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum RootModuleOverride {
    RegistrySingle(RegistrySingleOverride),
    RegistryMultiple(RegistryMultipleOverride),
    NonRegistry(RepoSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistrySingleOverride {
    pub version: CompactString,
    pub registry: CompactString,
    pub patches: Arc<[CanonicalLabel]>,
    pub patch_cmds: Arc<[CompactString]>,
    pub patch_strip: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegistryMultipleOverride {
    pub versions: Arc<[CompactString]>,
    pub registry: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepoSpec {
    pub rule_id: RepoRuleId,
    pub attributes: Arc<SmallMap<CompactString, OverrideAttributeValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepoRuleId {
    pub bzl_file: CanonicalLabel,
    pub rule_name: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum OverrideAttributeValue {
    None,
    Bool(bool),
    Int(i32),
    String(CompactString),
    Label(CanonicalLabel),
    Iterable(Arc<[OverrideAttributeValue]>),
    Map(Arc<SmallMap<OverrideAttributeKey, OverrideAttributeValue>>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative)]
pub enum OverrideAttributeKey {
    String(CompactString),
    Label(CanonicalLabel),
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
enum RecordedRootModuleOverride {
    RegistrySingle {
        version: CompactString,
        registry: CompactString,
        patches: Arc<[CompactString]>,
        patch_cmds: Arc<[CompactString]>,
        patch_strip: i32,
    },
    RegistryMultiple(RegistryMultipleOverride),
    NonRegistry {
        repo_spec: RepoSpec,
        patches_to_validate: Arc<[CompactString]>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleCommandPolicy {
    yanked_versions_policy: Arc<str>,
    ignore_dev_dependency: bool,
}

impl From<BzlmodCommandPolicyKey> for RootModuleCommandPolicy {
    fn from(policy: BzlmodCommandPolicyKey) -> Self {
        Self {
            yanked_versions_policy: Arc::from(policy.stable_serialize()),
            ignore_dev_dependency: policy.ignore_dev_dependency(),
        }
    }
}

impl RootModuleCommandPolicy {
    pub fn ignore_dev_dependency(&self) -> bool {
        self.ignore_dev_dependency
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleEnvironmentPolicy {
    yanked_versions_policy: Arc<str>,
}

impl From<BzlmodEnvironmentPolicyKey> for RootModuleEnvironmentPolicy {
    fn from(policy: BzlmodEnvironmentPolicyKey) -> Self {
        Self {
            yanked_versions_policy: Arc::from(policy.stable_serialize()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub struct RootModuleLockfileMode(Arc<str>);

impl From<LockfileMode> for RootModuleLockfileMode {
    fn from(mode: LockfileMode) -> Self {
        Self(Arc::from(mode.as_str()))
    }
}

impl RootModuleLockfileMode {
    pub(crate) fn semantic_mode(&self) -> LockfileMode {
        match self.0.as_ref() {
            "off" => LockfileMode::Off,
            "update" => LockfileMode::Update,
            "refresh" => LockfileMode::Refresh,
            "error" => LockfileMode::Error,
            mode => unreachable!("injected lockfile mode was not normalized: {mode}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleFileEvaluation {
    pub path: PathBuf,
    pub header: Option<RootModuleHeader>,
    pub includes: Arc<[CompactString]>,
    pub dependencies: Arc<[RootModuleDependency]>,
    override_contributions: Arc<SmallMap<CompactString, RecordedRootModuleOverride>>,
}

impl ModuleFileEvaluation {
    fn stripped_override_contributions(mut self) -> Self {
        self.override_contributions = Arc::new(SmallMap::new());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleFiles {
    pub root: ModuleFileEvaluation,
    pub includes: Arc<[ModuleFileEvaluation]>,
    pub visible_lockfile: VisibleLockfileRead,
    pub overrides: RootModuleOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleGraph {
    pub root: ModuleFileEvaluation,
    pub includes: Arc<[ModuleFileEvaluation]>,
    pub visible_lockfile: VisibleLockfileRead,
    pub repository_mapping: RepositoryMapping,
    pub command_policy: RootModuleCommandPolicy,
    pub environment_policy: RootModuleEnvironmentPolicy,
    pub lockfile_mode: RootModuleLockfileMode,
    pub overrides: RootModuleOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleCommandPolicyKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RootModuleCommandPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-command-policy:{}", self.workspace.display())
    }
}

impl InjectedKey for RootModuleCommandPolicyKey {
    type Value = RootModuleCommandPolicy;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleEnvironmentPolicyKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleEnvironmentPolicyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root-module-environment-policy:{}",
            self.workspace.display()
        )
    }
}
impl InjectedKey for RootModuleEnvironmentPolicyKey {
    type Value = RootModuleEnvironmentPolicy;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleLockfileModeKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleLockfileModeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-lockfile-mode:{}", self.workspace.display())
    }
}
impl InjectedKey for RootModuleLockfileModeKey {
    type Value = RootModuleLockfileMode;
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct VisibleLockfileKey {
    pub workspace: PathBuf,
}

impl fmt::Display for VisibleLockfileKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "visible-lockfile:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for VisibleLockfileKey {
    type Value = Arc<Result<VisibleLockfileRead, CompactString>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(mode) => mode.semantic_mode(),
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module lockfile mode: {error}"
                ))));
            }
        };
        if matches!(mode, LockfileMode::Off) {
            return Arc::new(Ok(VisibleLockfileRead::Ignored));
        }

        let value = match ctx
            .compute(&WorkspaceFileKey {
                workspace: self.workspace.clone(),
                path: self.workspace.join("MODULE.bazel.lock"),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let parsed = match value {
            WorkspaceFileValue::Present(source) => {
                parse_visible_lockfile_content_for_mode(&mode, Some(source.as_str()))
            }
            WorkspaceFileValue::Absent => parse_visible_lockfile_content_for_mode(&mode, None),
            WorkspaceFileValue::ReadError(error) => {
                Err(bad_visible_lockfile_message(error.as_str()))
            }
        };
        Arc::new(parsed.map_err(CompactString::new))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

/// Inject one normalized bzlmod request into a caller-owned DICE updater.
///
/// This helper deliberately supplies no defaults and does not commit. The
/// runtime owns the request transaction and must install all three values
/// before its sole commit.
pub fn inject_root_module_request_inputs(
    updater: &mut DiceTransactionUpdater,
    workspace: &Path,
    command_policy: BzlmodCommandPolicyKey,
    environment_policy: BzlmodEnvironmentPolicyKey,
    lockfile_mode: LockfileMode,
) -> anyhow::Result<()> {
    updater.changed_to(vec![(
        RootModuleCommandPolicyKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleCommandPolicy::from(command_policy),
    )])?;
    updater.changed_to(vec![(
        RootModuleEnvironmentPolicyKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleEnvironmentPolicy::from(environment_policy),
    )])?;
    updater.changed_to(vec![(
        RootModuleLockfileModeKey {
            workspace: workspace.to_path_buf(),
        },
        RootModuleLockfileMode::from(lockfile_mode),
    )])?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleFileEvaluationKey {
    pub workspace: PathBuf,
    pub path: PathBuf,
}
impl fmt::Display for ModuleFileEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module-file-evaluation:{}", self.path.display())
    }
}

#[async_trait]
impl Key for ModuleFileEvaluationKey {
    type Value = Arc<Result<ModuleFileEvaluation, CompactString>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let value = match ctx
            .compute(&WorkspaceFileKey {
                workspace: self.workspace.clone(),
                path: self.path.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        Arc::new(match value {
            WorkspaceFileValue::Present(source) => evaluate_module_file(&self.path, &source),
            WorkspaceFileValue::Absent => Err(CompactString::new(format!(
                "workspace file is absent: {}",
                self.path.display()
            ))),
            WorkspaceFileValue::ReadError(error) => Err(CompactString::new(error.as_str())),
        })
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleGraphKey {
    pub workspace: PathBuf,
}
impl fmt::Display for RootModuleGraphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-graph:{}", self.workspace.display())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct RootModuleFilesKey {
    pub workspace: PathBuf,
}

impl fmt::Display for RootModuleFilesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-files:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for RootModuleFilesKey {
    type Value = Arc<Result<RootModuleFiles, CompactString>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let root_path = self.workspace.join("MODULE.bazel");
        let root = match ctx
            .compute(&ModuleFileEvaluationKey {
                workspace: self.workspace.clone(),
                path: root_path,
            })
            .await
        {
            Ok(value) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(error)),
            },
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let mut seen = SmallSet::new();
        let mut horizon = VecDeque::from(root.includes.iter().cloned().collect::<Vec<_>>());
        let mut includes = Vec::new();
        while let Some(label) = horizon.pop_front() {
            if !seen.insert(label.clone()) {
                continue;
            }
            let path = match include_path(&self.workspace, label.as_str()) {
                Ok(path) => path,
                Err(error) => return Arc::new(Err(error)),
            };
            let value = match ctx
                .compute(&ModuleFileEvaluationKey {
                    workspace: self.workspace.clone(),
                    path,
                })
                .await
            {
                Ok(value) => match value.as_ref().clone() {
                    Ok(value) => value,
                    Err(error) => return Arc::new(Err(error)),
                },
                Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
            };
            if value.header.is_some() {
                return Arc::new(Err(CompactString::new(
                    "if module() is called, it must be called before any other functions",
                )));
            }
            horizon.extend(value.includes.iter().cloned());
            includes.push(value);
        }
        let visible_lockfile = match ctx
            .compute(&VisibleLockfileKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(error)),
            },
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let mut overrides = SmallMap::new();
        for file in std::iter::once(&root).chain(&includes) {
            for (module_name, override_) in file.override_contributions.iter() {
                let override_ = match materialize_override(override_, root.header.as_ref()) {
                    Ok(override_) => override_,
                    Err(error) => {
                        return Arc::new(Err(CompactString::new(error.to_string())));
                    }
                };
                if overrides.insert(module_name.clone(), override_).is_some() {
                    return Arc::new(Err(CompactString::new(format!(
                        "multiple overrides for module {module_name}"
                    ))));
                }
            }
        }
        Arc::new(Ok(RootModuleFiles {
            root: root.stripped_override_contributions(),
            includes: includes
                .into_iter()
                .map(ModuleFileEvaluation::stripped_override_contributions)
                .collect(),
            visible_lockfile,
            overrides: RootModuleOverrides(Arc::new(overrides)),
        }))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RootModuleGraphKey {
    type Value = Arc<Result<RootModuleGraph, CompactString>>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let files = match ctx
            .compute(&RootModuleFilesKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => match value.as_ref().clone() {
                Ok(value) => value,
                Err(error) => return Arc::new(Err(error)),
            },
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let command_policy = match ctx
            .compute(&RootModuleCommandPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module command policy: {error}"
                ))));
            }
        };
        let environment_policy = match ctx
            .compute(&RootModuleEnvironmentPolicyKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module environment policy: {error}"
                ))));
            }
        };
        let lockfile_mode = match ctx
            .compute(&RootModuleLockfileModeKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(mode) => mode,
            Err(error) => {
                return Arc::new(Err(CompactString::new(format!(
                    "missing injected root module lockfile mode: {error}"
                ))));
            }
        };
        Arc::new(Ok(RootModuleGraph {
            repository_mapping: match root_mapping(&files.root, &files.includes, &command_policy) {
                Ok(mapping) => mapping,
                Err(error) => return Arc::new(Err(error)),
            },
            root: files.root,
            includes: files.includes,
            visible_lockfile: files.visible_lockfile,
            command_policy,
            environment_policy,
            lockfile_mode,
            overrides: files.overrides,
        }))
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

fn include_path(workspace: &Path, label: &str) -> Result<PathBuf, CompactString> {
    let Some(repo_relative) = label.strip_prefix("//") else {
        return Err(CompactString::new(format!(
            "bad include label '{label}': include() must be called with repo-relative labels (starting with double slashes)"
        )));
    };
    let (package, target) = match repo_relative.split_once(':') {
        Some((package, "")) => (package, package.rsplit('/').next().unwrap_or_default()),
        Some((package, target)) => (package, target),
        None => (
            repo_relative,
            repo_relative.rsplit('/').next().unwrap_or_default(),
        ),
    };
    if package.contains(':')
        || target.contains(':')
        || repo_relative.contains('\\')
        || repo_relative.chars().any(char::is_control)
        || !(package.is_empty() || package.split('/').all(valid_package_segment))
        || !target.split('/').all(valid_target_segment)
        || !target.ends_with(".MODULE.bazel")
        || target
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with('.'))
    {
        return Err(CompactString::new(format!("bad include label '{label}'")));
    }
    Ok(if package.is_empty() {
        workspace.join(target)
    } else {
        workspace.join(package).join(target)
    })
}

fn valid_package_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.bytes().all(|byte| {
            byte.is_ascii()
                && !byte.is_ascii_control()
                && byte != b'\x7f'
                && !matches!(byte, b':' | b'\\')
        })
        && !segment.bytes().all(|byte| byte == b'.')
}

fn valid_target_segment(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
}

fn validate_module_name(name: &str) -> anyhow::Result<()> {
    let valid = name.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' => true,
        b'0'..=b'9' => index != 0,
        b'.' | b'_' | b'-' => index != 0 && index + 1 != name.len(),
        _ => false,
    });
    if !valid {
        anyhow::bail!("invalid module name '{name}'");
    }
    Ok(())
}

fn validate_repo_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    let valid = name.bytes().enumerate().all(|(index, byte)| match byte {
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => true,
        b'-' | b'.' | b'_' => index != 0,
        _ => false,
    });
    if !valid {
        anyhow::bail!("invalid repository name '{name}'");
    }
    Ok(())
}

fn validate_version(version: &str, directive: &str) -> anyhow::Result<()> {
    if !is_valid_version(version) {
        anyhow::bail!("Invalid version in {directive}()");
    }
    Ok(())
}

fn is_valid_version(version: &str) -> bool {
    if version.is_empty() {
        return true;
    }
    let mut build_split = version.split('+');
    let core = build_split.next().unwrap_or_default();
    let build = build_split.next();
    if build_split.next().is_some()
        || build.is_some_and(|build| build.is_empty() || !valid_version_chars(build, true))
    {
        return false;
    }
    let (release, prerelease) = match core.split_once('-') {
        Some((release, prerelease)) => (release, Some(prerelease)),
        None => (core, None),
    };
    valid_version_identifiers(release, false)
        && prerelease.is_none_or(|value| valid_version_identifiers(value, true))
}

fn valid_version_chars(value: &str, allow_hyphen: bool) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || (allow_hyphen && byte == b'-'))
}

fn valid_version_identifiers(value: &str, allow_hyphen: bool) -> bool {
    !value.is_empty()
        && valid_version_chars(value, allow_hyphen)
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && (!identifier.bytes().all(|byte| byte.is_ascii_digit())
                    || identifier.parse::<u64>().is_ok())
        })
}

fn validate_bazel_compatibility(value: &str) -> anyhow::Result<()> {
    let (operator, version) = if let Some(version) = value
        .strip_prefix("<=")
        .or_else(|| value.strip_prefix(">="))
    {
        (true, version)
    } else if let Some(version) = value.strip_prefix(['<', '>', '-']) {
        (true, version)
    } else {
        (false, "")
    };
    if !operator
        || version.split('.').count() != 3
        || version
            .split('.')
            .any(|segment| segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()))
    {
        anyhow::bail!("invalid bazel_compatibility value '{value}'");
    }
    Ok(())
}

fn normalize_patch_label(
    value: &str,
    header: Option<&RootModuleHeader>,
) -> anyhow::Result<CanonicalLabel> {
    let canonical = if let Some(value) = value.strip_prefix("@//") {
        format!("@@//{value}")
    } else if value.starts_with("//") {
        format!("@@{value}")
    } else if value.starts_with("@@") {
        value.to_owned()
    } else if let Some(value) = value.strip_prefix(':') {
        format!("@@//:{value}")
    } else if let Some(value) = value.strip_prefix('@') {
        let Some((repo, rest)) = value.split_once("//") else {
            anyhow::bail!("invalid patch label: @{value}");
        };
        let own_repo = header
            .and_then(|header| header.repo_name.as_deref().or(Some(header.name.as_str())))
            .is_some_and(|own| own == repo);
        if repo.is_empty() || own_repo {
            format!("@@//{rest}")
        } else {
            anyhow::bail!("patch label is not visible from the root module: @{value}");
        }
    } else {
        format!("@@//:{value}")
    };
    CanonicalLabel::parse(&canonical).map_err(anyhow::Error::msg)
}

fn repo_rule_id(bzl_file: &str, rule_name: &str) -> RepoRuleId {
    RepoRuleId {
        bzl_file: CanonicalLabel::parse(bzl_file)
            .expect("pinned Bazel repository rule label must be canonical"),
        rule_name: rule_name.into(),
    }
}

fn materialize_override(
    override_: &RecordedRootModuleOverride,
    header: Option<&RootModuleHeader>,
) -> anyhow::Result<RootModuleOverride> {
    Ok(match override_ {
        RecordedRootModuleOverride::RegistrySingle {
            version,
            registry,
            patches,
            patch_cmds,
            patch_strip,
        } => RootModuleOverride::RegistrySingle(RegistrySingleOverride {
            version: version.clone(),
            registry: registry.clone(),
            patches: patches
                .iter()
                .map(|patch| normalize_patch_label(patch, header))
                .collect::<anyhow::Result<_>>()?,
            patch_cmds: patch_cmds.clone(),
            patch_strip: *patch_strip,
        }),
        RecordedRootModuleOverride::RegistryMultiple(override_) => {
            RootModuleOverride::RegistryMultiple(override_.clone())
        }
        RecordedRootModuleOverride::NonRegistry {
            repo_spec,
            patches_to_validate,
        } => {
            for patch in patches_to_validate.iter() {
                normalize_patch_label(patch, header)?;
            }
            RootModuleOverride::NonRegistry(repo_spec.clone())
        }
    })
}

fn collect_patch_strings<'v>(
    values: impl Iterator<Item = Value<'v>>,
) -> anyhow::Result<Arc<[CompactString]>> {
    values
        .map(|value| {
            value
                .unpack_str()
                .map(CompactString::new)
                .ok_or_else(|| anyhow::anyhow!("patches must be a sequence of strings"))
        })
        .collect()
}

fn patch_strings(attrs: &DictRef) -> anyhow::Result<Arc<[CompactString]>> {
    let Some((_, patches)) = attrs
        .iter()
        .find(|(key, _)| key.unpack_str() == Some("patches"))
    else {
        return Ok(Arc::new([]));
    };
    if let Some(values) = ListRef::from_value(patches) {
        return collect_patch_strings(values.iter());
    }
    if let Some(values) = TupleRef::from_value(patches) {
        return collect_patch_strings(values.iter());
    }
    anyhow::bail!("patches must be a sequence of strings")
}

fn override_attributes(
    attrs: DictRef,
) -> anyhow::Result<SmallMap<CompactString, OverrideAttributeValue>> {
    attrs
        .iter()
        .map(|(key, value)| {
            let key = key.unpack_str().ok_or_else(|| {
                anyhow::anyhow!("repository override keyword names must be strings")
            })?;
            Ok((
                CompactString::new(key),
                override_attribute_value(value, &mut SmallSet::new())?,
            ))
        })
        .collect::<anyhow::Result<SmallMap<CompactString, OverrideAttributeValue>>>()
}

fn override_attribute_value<'v>(
    value: Value<'v>,
    active: &mut SmallSet<ValueIdentity<'v>>,
) -> anyhow::Result<OverrideAttributeValue> {
    if value.is_none() {
        return Ok(OverrideAttributeValue::None);
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(OverrideAttributeValue::Bool(value));
    }
    if let Some(value) = value.unpack_i32() {
        return Ok(OverrideAttributeValue::Int(value));
    }
    if let Some(value) = value.unpack_str() {
        return Ok(OverrideAttributeValue::String(value.into()));
    }
    let identity = value.identity();
    if !active.insert(identity) {
        anyhow::bail!("repository override attributes must not contain cyclic values");
    }
    if let Some(values) = ListRef::from_value(value) {
        let result = values
            .iter()
            .map(|value| override_attribute_value(value, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(OverrideAttributeValue::Iterable);
        active.shift_remove(&identity);
        return result;
    }
    if let Some(values) = TupleRef::from_value(value) {
        let result = values
            .iter()
            .map(|value| override_attribute_value(value, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(OverrideAttributeValue::Iterable);
        active.shift_remove(&identity);
        return result;
    }
    if let Some(values) = DictRef::from_value(value) {
        let result = values
            .iter()
            .map(|(key, value)| {
                let key = if let Some(key) = key.unpack_str() {
                    OverrideAttributeKey::String(key.into())
                } else {
                    anyhow::bail!("repository override map keys must be strings or labels");
                };
                Ok((key, override_attribute_value(value, active)?))
            })
            .collect::<anyhow::Result<SmallMap<_, _>>>()
            .map(|values| OverrideAttributeValue::Map(Arc::new(values)));
        active.shift_remove(&identity);
        return result;
    }
    active.shift_remove(&identity);
    anyhow::bail!("unsupported repository override attribute value: {value}")
}

fn root_mapping(
    root: &ModuleFileEvaluation,
    includes: &[ModuleFileEvaluation],
    policy: &RootModuleCommandPolicy,
) -> Result<RepositoryMapping, CompactString> {
    let mut mapping = RepositoryMapping::new(
        RepositoryMappingId::new("root-module").map_err(CompactString::new)?,
    );
    for dep in std::iter::once(root)
        .chain(includes.iter())
        .flat_map(|module| module.dependencies.iter())
        .filter(|dep| !dep.dev_dependency || !policy.ignore_dev_dependency())
        .filter(|dep| !dep.nodep)
    {
        let apparent = dep.repo_name.as_deref().unwrap_or(dep.name.as_str());
        mapping.insert(
            ApparentRepoName::new(apparent).map_err(CompactString::new)?,
            CanonicalRepoName::new(format!("{}+", dep.name)).map_err(CompactString::new)?,
        );
    }
    Ok(mapping)
}

#[derive(Default, ProvidesStaticType)]
struct Recorder {
    state: RefCell<RecordedModuleFile>,
}

#[derive(Default)]
struct RecordedModuleFile {
    header: Option<RootModuleHeader>,
    non_module_called: bool,
    includes: Vec<CompactString>,
    dependencies: Vec<RootModuleDependency>,
    overrides: SmallMap<CompactString, RecordedRootModuleOverride>,
}

fn record_override(
    state: &mut RecordedModuleFile,
    module_name: &str,
    override_: RecordedRootModuleOverride,
) -> anyhow::Result<()> {
    validate_module_name(module_name)?;
    if state
        .overrides
        .insert(module_name.into(), override_)
        .is_some()
    {
        anyhow::bail!("multiple overrides for module {module_name}");
    }
    state.non_module_called = true;
    Ok(())
}

fn recorder<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a Recorder> {
    eval.extra
        .and_then(|value| value.downcast_ref())
        .context("MODULE.bazel global invoked without module recorder")
}

#[starlark_module]
fn module_globals(builder: &mut GlobalsBuilder) {
    fn module(
        #[starlark(require = named, default = "")] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] compatibility_level: i32,
        #[starlark(require = named, default = "")] repo_name: &str,
        #[starlark(require = named, default = UnpackList::default())]
        bazel_compatibility: UnpackList<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = recorder(eval)?.state.borrow_mut();
        if state.header.is_some() {
            anyhow::bail!("module() can only be called once");
        }
        if state.non_module_called {
            anyhow::bail!("if module() is called, it must be called before any other functions");
        }
        let _compatibility_level = compatibility_level;
        if !name.is_empty() {
            validate_module_name(name)?;
        }
        validate_version(version, "module")?;
        validate_repo_name(repo_name)?;
        for value in bazel_compatibility.items {
            validate_bazel_compatibility(value)?;
        }
        state.header = Some(RootModuleHeader {
            name: name.into(),
            version: (!version.is_empty()).then(|| version.into()),
            repo_name: (!repo_name.is_empty()).then(|| repo_name.into()),
        });
        Ok(NoneType)
    }
    fn include(label: &str, eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        let mut state = recorder(eval)?.state.borrow_mut();
        state.non_module_called = true;
        state.includes.push(label.into());
        Ok(NoneType)
    }
    fn bazel_dep(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] max_compatibility_level: i32,
        #[starlark(require = named)] repo_name: Option<NoneOr<&str>>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let _max_compatibility_level = max_compatibility_level;
        validate_module_name(name)?;
        validate_version(version, "bazel_dep")?;
        if let Some(NoneOr::Other(repo_name)) = repo_name {
            validate_repo_name(repo_name)?;
        }
        let (repo_name, nodep) = match repo_name {
            Some(NoneOr::None) => (None, true),
            Some(NoneOr::Other("")) | None => (Some(name.into()), false),
            Some(NoneOr::Other(repo_name)) => (Some(repo_name.into()), false),
        };
        let mut state = recorder(eval)?.state.borrow_mut();
        state.non_module_called = true;
        state.dependencies.push(RootModuleDependency {
            name: name.into(),
            version: version.into(),
            repo_name,
            nodep,
            dev_dependency,
        });
        Ok(NoneType)
    }
    fn local_path_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] path: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = recorder(eval)?.state.borrow_mut();
        record_override(
            &mut state,
            module_name,
            RecordedRootModuleOverride::NonRegistry {
                repo_spec: RepoSpec {
                    rule_id: repo_rule_id(
                        "@@bazel_tools//tools/build_defs/repo:local.bzl",
                        "local_repository",
                    ),
                    attributes: Arc::new(SmallMap::from_iter([(
                        CompactString::new("path"),
                        OverrideAttributeValue::String(path.into()),
                    )])),
                },
                patches_to_validate: Arc::new([]),
            },
        )?;
        Ok(NoneType)
    }

    fn single_version_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = "")] registry: &str,
        #[starlark(require = named, default = UnpackList::default())] patches: UnpackList<&str>,
        #[starlark(require = named, default = UnpackList::default())] patch_cmds: UnpackList<&str>,
        #[starlark(require = named, default = 0)] patch_strip: i32,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_version(version, "single_version_override")?;
        let mut state = recorder(eval)?.state.borrow_mut();
        record_override(
            &mut state,
            module_name,
            RecordedRootModuleOverride::RegistrySingle {
                version: version.into(),
                registry: registry.into(),
                patches: patches.items.into_iter().map(Into::into).collect(),
                patch_cmds: patch_cmds.items.into_iter().map(Into::into).collect(),
                patch_strip,
            },
        )?;
        Ok(NoneType)
    }

    fn multiple_version_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] versions: UnpackList<&str>,
        #[starlark(require = named, default = "")] registry: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        if versions.items.len() < 2 {
            anyhow::bail!("multiple_version_override() requires at least two versions");
        }
        for version in &versions.items {
            validate_version(version, "multiple_version_override")?;
        }
        let mut state = recorder(eval)?.state.borrow_mut();
        record_override(
            &mut state,
            module_name,
            RecordedRootModuleOverride::RegistryMultiple(RegistryMultipleOverride {
                versions: versions.items.into_iter().map(Into::into).collect(),
                registry: registry.into(),
            }),
        )?;
        Ok(NoneType)
    }

    fn archive_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(kwargs)] attrs: DictRef<'v>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let patches_to_validate = patch_strings(&attrs)?;
        let attributes = override_attributes(attrs)?;
        let mut state = recorder(eval)?.state.borrow_mut();
        record_override(
            &mut state,
            module_name,
            RecordedRootModuleOverride::NonRegistry {
                repo_spec: RepoSpec {
                    rule_id: repo_rule_id(
                        "@@bazel_tools//tools/build_defs/repo:http.bzl",
                        "http_archive",
                    ),
                    attributes: Arc::new(attributes),
                },
                patches_to_validate,
            },
        )?;
        Ok(NoneType)
    }

    fn git_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(kwargs)] attrs: DictRef<'v>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let patches_to_validate = patch_strings(&attrs)?;
        let attributes = override_attributes(attrs)?;
        let mut state = recorder(eval)?.state.borrow_mut();
        record_override(
            &mut state,
            module_name,
            RecordedRootModuleOverride::NonRegistry {
                repo_spec: RepoSpec {
                    rule_id: repo_rule_id(
                        "@@bazel_tools//tools/build_defs/repo:git.bzl",
                        "git_repository",
                    ),
                    attributes: Arc::new(attributes),
                },
                patches_to_validate,
            },
        )?;
        Ok(NoneType)
    }
}

fn evaluate_module_file(path: &Path, source: &str) -> Result<ModuleFileEvaluation, CompactString> {
    let ast = AstModule::parse(
        &path.display().to_string(),
        source.to_owned(),
        &Dialect::Standard,
    )
    .map_err(|e| CompactString::new(e.to_string()))?;
    let module = Module::new();
    let recorder = Recorder::default();
    let globals = GlobalsBuilder::standard().with(module_globals).build();
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&recorder);
        evaluator
            .eval_module(ast, &globals)
            .map_err(|e| CompactString::new(e.to_string()))?;
    }
    let recorder = recorder.state.into_inner();
    Ok(ModuleFileEvaluation {
        path: path.to_path_buf(),
        header: recorder.header,
        includes: recorder.includes.into(),
        dependencies: recorder.dependencies.into(),
        override_contributions: Arc::new(recorder.overrides),
    })
}
