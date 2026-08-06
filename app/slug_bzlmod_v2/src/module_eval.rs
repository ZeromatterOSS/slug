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
use dice::DiceProjectionComputations;
use dice::DiceTransactionUpdater;
use dice::InjectedKey;
use dice::Key;
use dice::ProjectionKey;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_events_v2::CaptureEvaluationEvents;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_identity_v2::TargetName;
use slug_workspace_v2::WorkspaceFileKey;
use slug_workspace_v2::WorkspaceFileValue;
use slug_workspace_v2::WorkspaceRawFileKey;
use slug_workspace_v2::WorkspaceRawFileValue;
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
use starlark::starlark_simple_value;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::ast::Argument;
use starlark::syntax::ast::AssignTarget;
use starlark::syntax::ast::AstExpr;
use starlark::syntax::ast::AstLiteral;
use starlark::syntax::ast::AstStmt;
use starlark::syntax::ast::Expr;
use starlark::syntax::ast::Stmt;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::float::StarlarkFloat;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::TupleRef;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

use crate::BzlmodCommandPolicyKey;
use crate::BzlmodEnvironmentPolicyKey;
use crate::EvaluatedNonrootModule;
use crate::LockfileMode;
use crate::LogicalModuleFileId;
use crate::LogicalSpan;
use crate::NonrootAttributeKey;
use crate::NonrootAttributeValue;
use crate::NonrootDependency;
use crate::NonrootExtensionIsolationKey;
use crate::NonrootExtensionProxy;
use crate::NonrootExtensionTag;
use crate::NonrootExtensionUsage;
use crate::NonrootModuleBuilder;
use crate::NonrootModuleKey;
use crate::NonrootRepoImports;
use crate::VisibleLockfileRead;
use crate::lockfile::bad_visible_lockfile_message;
use crate::lockfile::parse_visible_lockfile_bytes_for_mode;

/// A direct, literal `include()` request found while compiling one non-root
/// MODULE file. The later closure evaluator supplies and executes these files.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct NonrootIncludeRequest {
    pub path: CompactString,
    pub location: LogicalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub(crate) struct ParsedRootInclude {
    package: PackageIdentifier,
    target: TargetName,
    raw_label: CompactString,
    location: LogicalSpan,
}

impl ParsedRootInclude {
    pub(crate) fn package(&self) -> &PackageIdentifier {
        &self.package
    }

    pub(crate) fn target(&self) -> &TargetName {
        &self.target
    }

    pub(crate) fn raw_label(&self) -> &str {
        self.raw_label.as_str()
    }

    pub(crate) fn location(&self) -> &LogicalSpan {
        &self.location
    }
}

pub(crate) fn parse_root_include(
    request: &NonrootIncludeRequest,
) -> Result<ParsedRootInclude, CompactString> {
    let (package, target) = parse_root_include_label(request.path.as_str())?;
    Ok(ParsedRootInclude {
        package: PackageIdentifier::new(CanonicalRepoName::root(), package),
        target,
        raw_label: request.path.clone(),
        location: request.location.clone(),
    })
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
    parse_and_inspect_nonroot_module_file(logical_id, source).map(|(_, inspection)| inspection)
}

fn parse_and_inspect_nonroot_module_file(
    logical_id: LogicalModuleFileId,
    source: &[u8],
) -> anyhow::Result<(AstModule, NonrootModuleFileInspection)> {
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

    Ok((
        ast,
        NonrootModuleFileInspection {
            logical_id,
            includes: Arc::from(includes),
        },
    ))
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

#[cfg(test)]
mod deferred_attribute_snapshot_tests {
    use super::*;
    use crate::interim_module::NonrootAttributeAdapterError;
    use crate::interim_module::NonrootAttributeAdapterKey;
    use crate::interim_module::NonrootAttributeAdapterProjection;
    use crate::interim_module::NonrootAttributeAdapterValue;
    use crate::interim_module::project_nonroot_attributes_for_adapter;

    fn evaluated_module(source: &str) -> (Module, starlark::environment::Globals) {
        let module = Module::new();
        let globals = starlark::environment::Globals::extended_internal();
        let ast = AstModule::parse(
            "snapshot.MODULE.bazel",
            source.to_owned(),
            &Dialect::Standard,
        )
        .unwrap();
        Evaluator::new(&module).eval_module(ast, &globals).unwrap();
        (module, globals)
    }

    fn snapshot_attrs(
        module: &Module,
        globals: &starlark::environment::Globals,
        extension_proxy: Option<ValueIdentity<'_>>,
    ) -> anyhow::Result<SmallMap<CompactString, NonrootAttributeValue>> {
        let attrs = DictRef::from_value(module.get("attrs").unwrap()).unwrap();
        let builtin_print = globals
            .iter()
            .find_map(|(name, value)| (name == "print").then_some(value.to_value().identity()))
            .context("source-backed builtin print is absent")?;
        let mut extension_proxies = SmallSet::new();
        if let Some(extension_proxy) = extension_proxy {
            extension_proxies.insert(extension_proxy);
        }
        let identities = deferred_attribute_snapshot_identities(builtin_print, extension_proxies);
        snapshot_deferred_attribute_values(attrs, &identities)
    }

    #[test]
    fn snapshot_observes_final_mutation_without_retaining_alias_identity() {
        let (module, globals) = evaluated_module(
            r#"
shared = ["before"]
alias = shared
shared.append("after")
attrs = {"first": shared, "second": alias}
"#,
        );
        let snapshot = snapshot_attrs(&module, &globals, None).unwrap();
        let expected = NonrootAttributeValue::List(Arc::from([
            NonrootAttributeValue::String("before".into()),
            NonrootAttributeValue::String("after".into()),
        ]));
        assert_eq!(snapshot.get("first"), Some(&expected));
        assert_eq!(snapshot.get("second"), Some(&expected));
    }

    #[test]
    fn snapshot_keeps_list_tuple_and_exact_deferred_tokens_distinct() {
        let (module, globals) = evaluated_module(
            r#"
proxy = []
lookalike = []
attrs = {
    "list": ["item"],
    "tuple": ("item",),
    "float": 3.14,
    "print": print,
    "proxy": proxy,
    "lookalike": lookalike,
    "float_key": {3.14: "value"},
}
"#,
        );
        let proxy = module.get("proxy").unwrap().identity();
        let snapshot = snapshot_attrs(&module, &globals, Some(proxy)).unwrap();
        assert_ne!(snapshot.get("list"), snapshot.get("tuple"));
        assert_eq!(
            snapshot.get("float"),
            Some(&NonrootAttributeValue::Float314)
        );
        assert_eq!(
            snapshot.get("print"),
            Some(&NonrootAttributeValue::BuiltinPrint)
        );
        assert_eq!(
            snapshot.get("proxy"),
            Some(&NonrootAttributeValue::ExtensionProxy)
        );
        assert!(matches!(
            snapshot.get("lookalike"),
            Some(NonrootAttributeValue::List(values)) if values.is_empty()
        ));
        assert!(matches!(
            snapshot.get("float_key"),
            Some(NonrootAttributeValue::Dict(values))
                if values.contains_key(&NonrootAttributeKey::DeferredFloat314)
        ));
    }

    #[test]
    fn snapshot_accepts_only_the_exact_self_list_cycle() {
        let (module, globals) = evaluated_module(
            r#"
self_cycle = []
self_cycle.append(self_cycle)
attrs = {"cycle": self_cycle}
"#,
        );
        assert_eq!(
            snapshot_attrs(&module, &globals, None)
                .unwrap()
                .get("cycle"),
            Some(&NonrootAttributeValue::SelfList)
        );

        let (module, globals) = evaluated_module(
            r#"
first = []
second = [first]
first.append(second)
attrs = {"cycle": first}
"#,
        );
        assert!(
            snapshot_attrs(&module, &globals, None)
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );

        let (module, globals) = evaluated_module(
            r#"
two_element_self_cycle = ["value"]
two_element_self_cycle.append(two_element_self_cycle)
attrs = {"cycle": two_element_self_cycle}
"#,
        );
        assert!(
            snapshot_attrs(&module, &globals, None)
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );
    }

    #[test]
    fn snapshot_rejects_unproven_float_and_opaque_boundaries() {
        for source in [
            "attrs = {\"value\": 3.15}",
            "attrs = {\"value\": range(1)}",
            "attrs = {\"value\": len}",
            "attrs = {\"value\": {3.15: \"not the oracle key\"}}",
        ] {
            let (module, globals) = evaluated_module(source);
            assert!(snapshot_attrs(&module, &globals, None).is_err());
        }
    }

    #[test]
    fn snapshot_keeps_positive_and_negative_arbitrary_precision_integers() {
        let (module, globals) = evaluated_module(
            r#"
attrs = {
    "positive": 100000000000000000000,
    "negative": -100000000000000000000,
}
"#,
        );
        let snapshot = snapshot_attrs(&module, &globals, None).unwrap();
        let NonrootAttributeValue::Int(positive) = snapshot.get("positive").unwrap() else {
            panic!("positive arbitrary-precision integer was not retained");
        };
        assert_eq!(positive.as_i32(), None);
        assert_eq!(positive.to_decimal(), "100000000000000000000");
        let NonrootAttributeValue::Int(negative) = snapshot.get("negative").unwrap() else {
            panic!("negative arbitrary-precision integer was not retained");
        };
        assert_eq!(negative.as_i32(), None);
        assert_eq!(negative.to_decimal(), "-100000000000000000000");
    }

    #[test]
    fn adapter_preserves_iteration_order_while_semantic_dict_equality_does_not() {
        let first = SmallMap::from_iter([
            (
                CompactString::from("first"),
                NonrootAttributeValue::String("one".into()),
            ),
            (
                CompactString::from("second"),
                NonrootAttributeValue::String("two".into()),
            ),
        ]);
        let second = SmallMap::from_iter([
            (
                CompactString::from("second"),
                NonrootAttributeValue::String("two".into()),
            ),
            (
                CompactString::from("first"),
                NonrootAttributeValue::String("one".into()),
            ),
        ]);
        assert_eq!(first, second);
        let projection = project_nonroot_attributes_for_adapter(&first).unwrap();
        let projected_names: Vec<_> = projection
            .attributes
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(projected_names, ["first", "second"]);

        let mut adapter_first = SmallMap::new();
        adapter_first.insert(
            CompactString::from("float"),
            NonrootAttributeValue::Float314,
        );
        adapter_first.insert(
            CompactString::from("later_schema"),
            NonrootAttributeValue::BuiltinPrint,
        );
        assert_eq!(
            project_nonroot_attributes_for_adapter(&adapter_first),
            Err(NonrootAttributeAdapterError::ExactFloat314)
        );

        let small_integer = SmallMap::from_iter([(
            CompactString::from("small"),
            NonrootAttributeValue::integer("7").unwrap(),
        )]);
        assert!(matches!(
            project_nonroot_attributes_for_adapter(&small_integer),
            Ok(NonrootAttributeAdapterProjection {
                attributes
            }) if matches!(&attributes[0].1, NonrootAttributeAdapterValue::Int(value) if value.as_i32() == Some(7))
        ));
        let large_integer = SmallMap::from_iter([(
            CompactString::from("large"),
            NonrootAttributeValue::integer("100000000000000000000").unwrap(),
        )]);
        assert_eq!(
            project_nonroot_attributes_for_adapter(&large_integer),
            Err(NonrootAttributeAdapterError::IntegerOutsideI32)
        );

        let dict_projection = project_nonroot_attributes_for_adapter(&SmallMap::from_iter([(
            CompactString::from("dict"),
            NonrootAttributeValue::Dict(Arc::new(SmallMap::from_iter([
                (
                    NonrootAttributeKey::String("first".into()),
                    NonrootAttributeValue::String("one".into()),
                ),
                (
                    NonrootAttributeKey::String("second".into()),
                    NonrootAttributeValue::String("two".into()),
                ),
            ]))),
        )]))
        .unwrap();
        assert!(matches!(
            &dict_projection.attributes[0].1,
            NonrootAttributeAdapterValue::Dict(values)
                if matches!(&values[0].0, NonrootAttributeAdapterKey::String(key) if key == "first")
                    && matches!(&values[1].0, NonrootAttributeAdapterKey::String(key) if key == "second")
        ));
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

/// The aggregate semantic result of executing the root MODULE.bazel and its
/// complete inline include closure.
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe, Default)]
pub struct RootModuleRegistrations {
    execution_platforms: Arc<[ApparentLabel]>,
    toolchains: Arc<[ApparentLabel]>,
}

impl RootModuleRegistrations {
    pub fn execution_platforms(&self) -> &[ApparentLabel] {
        &self.execution_platforms
    }

    pub fn toolchains(&self) -> &[ApparentLabel] {
        &self.toolchains
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct EvaluatedRootModule {
    pub header: Option<RootModuleHeader>,
    pub dependencies: Arc<[RootModuleDependency]>,
    pub registrations: RootModuleRegistrations,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) struct RootModuleEvaluation {
    pub(crate) module: EvaluatedRootModule,
    pub(crate) module_file_paths: Arc<[PathBuf]>,
    pub(crate) overrides: RootModuleOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleFiles {
    pub module: EvaluatedRootModule,
    pub module_file_paths: Arc<[PathBuf]>,
    pub visible_lockfile: VisibleLockfileRead,
    pub overrides: RootModuleOverrides,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RootModuleGraph {
    pub module: EvaluatedRootModule,
    pub module_file_paths: Arc<[PathBuf]>,
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
            .compute(&WorkspaceRawFileKey {
                workspace: self.workspace.clone(),
                path: self.workspace.join("MODULE.bazel.lock"),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(CompactString::new(error.to_string()))),
        };
        let parsed = match value {
            WorkspaceRawFileValue::Present(source) => {
                parse_visible_lockfile_bytes_for_mode(&mode, Some(source.as_ref()))
            }
            WorkspaceRawFileValue::Absent => parse_visible_lockfile_bytes_for_mode(&mode, None),
            WorkspaceRawFileValue::ReadError(error) => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Allocative, Dupe)]
struct RootModuleIgnoreDevDependencyProjectionKey;

impl fmt::Display for RootModuleIgnoreDevDependencyProjectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("root-module-ignore-dev-dependency-projection")
    }
}

impl ProjectionKey for RootModuleIgnoreDevDependencyProjectionKey {
    type DeriveFromKey = RootModuleCommandPolicyKey;
    type Value = bool;

    fn compute(
        &self,
        policy: &RootModuleCommandPolicy,
        _ctx: &DiceProjectionComputations,
    ) -> Self::Value {
        policy.ignore_dev_dependency()
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

pub(crate) async fn root_module_ignore_dev_dependency(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
) -> Result<bool, CompactString> {
    let policy = ctx
        .compute_opaque(&RootModuleCommandPolicyKey {
            workspace: workspace.to_path_buf(),
        })
        .await
        .map_err(|error| {
            CompactString::new(format!(
                "missing injected root module command policy: {error}"
            ))
        })?;
    ctx.projection(&policy, &RootModuleIgnoreDevDependencyProjectionKey)
        .map_err(|error| {
            CompactString::new(format!(
                "missing injected root module command policy: {error}"
            ))
        })
}

pub(crate) struct RootModuleSourceFile {
    pub(crate) path: PathBuf,
    pub(crate) source: Arc<String>,
    pub(crate) _inspection: NonrootModuleFileInspection,
}

#[derive(Default)]
struct RootModulePrintCapture {
    events: RefCell<Vec<EvaluationEvent>>,
}

impl RootModulePrintCapture {
    fn into_batch(self) -> EventBatch {
        EventBatch::from_events(self.events.into_inner())
    }
}

impl PrintHandler for RootModulePrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> starlark::Result<()> {
        let (file, line, column) = location.into_parts();
        self.events
            .borrow_mut()
            .push(EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(file, line, column),
                text: text.into(),
            });
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct RootModuleEvaluationKey {
    workspace: PathBuf,
}

impl fmt::Display for RootModuleEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "root-module-evaluation:{}", self.workspace.display())
    }
}

#[async_trait]
impl Key for RootModuleEvaluationKey {
    type Value = Arc<Result<RootModuleEvaluation, CompactString>>;

    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let capture_events = ctx
            .per_transaction_data()
            .data
            .get::<CaptureEvaluationEvents>()
            .is_ok();
        let mut event_batch = None;
        let value = async {
            let ignore_dev_dependency =
                root_module_ignore_dev_dependency(ctx, &self.workspace).await?;

            let root_path = self.workspace.join("MODULE.bazel");
            let root_source = read_root_module_source(ctx, &self.workspace, &root_path).await?;
            let root_inspection = inspect_nonroot_module_file(
                LogicalModuleFileId::new(root_path.display().to_string()),
                root_source.as_bytes(),
            )
            .map_err(|error| CompactString::new(error.to_string()))?;
            let mut horizon = VecDeque::from(root_inspection.includes.to_vec());
            let mut files = vec![RootModuleSourceFile {
                path: root_path,
                source: root_source,
                _inspection: root_inspection,
            }];
            let mut include_indices = SmallMap::new();
            while let Some(request) = horizon.pop_front() {
                if include_indices.contains_key(request.path.as_str()) {
                    continue;
                }
                let path = include_path(&self.workspace, request.path.as_str())?;
                let source = read_root_module_source(ctx, &self.workspace, &path).await?;
                let inspection = inspect_nonroot_module_file(
                    LogicalModuleFileId::new(path.display().to_string()),
                    source.as_bytes(),
                )
                .map_err(|error| CompactString::new(error.to_string()))?;
                let index = files.len();
                include_indices.insert(request.path, index);
                horizon.extend(inspection.includes.iter().cloned());
                files.push(RootModuleSourceFile {
                    path,
                    source,
                    _inspection: inspection,
                });
            }

            let mut module_file_paths = Vec::with_capacity(files.len());
            for file in &files {
                let path = file.path.strip_prefix(&self.workspace).map_err(|error| {
                    CompactString::new(format!(
                        "root MODULE file escaped its workspace: {}: {error}",
                        file.path.display()
                    ))
                })?;
                module_file_paths.push(path.to_path_buf());
            }
            module_file_paths.sort();
            module_file_paths.dedup();

            let (value, captured) = evaluate_root_module_closure_with_events(
                ignore_dev_dependency,
                files,
                include_indices,
                module_file_paths.into(),
                capture_events,
            );
            event_batch = captured;
            value
        }
        .await;
        if capture_events {
            ctx.store_evaluation_data(event_batch.unwrap_or_else(EventBatch::empty))
                .expect("root MODULE evaluation stores exactly one event batch");
        }
        Arc::new(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

async fn read_root_module_source(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    path: &Path,
) -> Result<Arc<String>, CompactString> {
    let value = ctx
        .compute(&WorkspaceFileKey {
            workspace: workspace.to_path_buf(),
            path: path.to_path_buf(),
        })
        .await
        .map_err(|error| CompactString::new(error.to_string()))?;
    match value {
        WorkspaceFileValue::Present(source) => Ok(source),
        WorkspaceFileValue::Absent => Err(CompactString::new(format!(
            "workspace file is absent: {}",
            path.display()
        ))),
        WorkspaceFileValue::ReadError(error) => Err(CompactString::new(error.as_str())),
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
        let evaluation = match ctx
            .compute(&RootModuleEvaluationKey {
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
        Arc::new(Ok(RootModuleFiles {
            module: evaluation.module,
            module_file_paths: evaluation.module_file_paths,
            visible_lockfile,
            overrides: evaluation.overrides,
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
            repository_mapping: match root_mapping(&files.module) {
                Ok(mapping) => mapping,
                Err(error) => return Arc::new(Err(error)),
            },
            module: files.module,
            module_file_paths: files.module_file_paths,
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
    let (package, target) = parse_root_include_label(label)?;
    Ok(workspace.join(package.as_str()).join(target.as_str()))
}

fn parse_root_include_label(label: &str) -> Result<(PackagePath, TargetName), CompactString> {
    let Some(repo_relative) = label.strip_prefix("//") else {
        return Err(CompactString::new(format!(
            "bad include label '{label}': include() must be called with repo-relative labels (starting with double slashes)"
        )));
    };
    let (package, target) = match repo_relative.split_once(':') {
        Some((package, target)) => (package, target),
        None => (
            repo_relative,
            repo_relative.rsplit('/').next().unwrap_or_default(),
        ),
    };
    if package.contains(':')
        || target.contains(':')
        || !(package.is_empty() || package.split('/').all(valid_package_segment))
    {
        return Err(CompactString::new(format!("bad include label '{label}'")));
    }
    let package = PackagePath::parse(package)
        .map_err(|_| CompactString::new(format!("bad include label '{label}'")))?;
    let target = TargetName::parse(target)
        .map_err(|_| CompactString::new(format!("bad include label '{label}'")))?;
    if !target.as_str().ends_with(".MODULE.bazel")
        || target
            .as_str()
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with('.'))
    {
        return Err(CompactString::new(format!("bad include label '{label}'")));
    }
    Ok((package, target))
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

fn normalize_version(version: &str) -> CompactString {
    version
        .split_once('+')
        .map_or(version, |(normalized, _)| normalized)
        .into()
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

/// Source-backed identities that are valid only while a MODULE evaluator is
/// alive. They are used while copying the final raw kwargs into compact state;
/// neither the identities nor the source values escape the snapshot boundary.
#[derive(Clone)]
#[allow(dead_code)]
struct DeferredAttributeSnapshotIdentities<'v> {
    builtin_print: ValueIdentity<'v>,
    extension_proxies: SmallSet<ValueIdentity<'v>>,
}

#[allow(dead_code)]
fn deferred_attribute_snapshot_identities<'v>(
    builtin_print: ValueIdentity<'v>,
    extension_proxies: SmallSet<ValueIdentity<'v>>,
) -> DeferredAttributeSnapshotIdentities<'v> {
    DeferredAttributeSnapshotIdentities {
        builtin_print,
        extension_proxies,
    }
}

/// Copy final evaluator-local kwargs into the heap-independent retained tree.
///
/// This runs only after file execution has observed post-call mutation. Acyclic
/// aliases are copied by final structural contents, while the single
/// oracle-proven `[self]` shape receives a bounded diagnostic token. The
/// caller supplies the exact print/proxy identities from the evaluator's
/// source-backed globals/directive state; there is deliberately no repr or
/// type-name recognition fallback.
#[allow(dead_code)]
fn snapshot_deferred_attribute_values<'v>(
    attributes: DictRef<'v>,
    identities: &DeferredAttributeSnapshotIdentities<'v>,
) -> anyhow::Result<SmallMap<CompactString, NonrootAttributeValue>> {
    attributes
        .iter()
        .map(|(key, value)| {
            let key = key
                .unpack_str()
                .context("module attribute names must be strings")?;
            Ok((
                CompactString::from(key),
                snapshot_deferred_attribute_value(value, identities, &mut SmallSet::new())?,
            ))
        })
        .collect::<anyhow::Result<SmallMap<CompactString, NonrootAttributeValue>>>()
}

#[allow(dead_code)]
fn snapshot_deferred_attribute_value<'v>(
    value: Value<'v>,
    identities: &DeferredAttributeSnapshotIdentities<'v>,
    active: &mut SmallSet<ValueIdentity<'v>>,
) -> anyhow::Result<NonrootAttributeValue> {
    let identity = value.identity();
    if identity == identities.builtin_print {
        return Ok(NonrootAttributeValue::BuiltinPrint);
    }
    if identities.extension_proxies.contains(&identity) {
        return Ok(NonrootAttributeValue::ExtensionProxy);
    }
    if value.is_none() {
        return Ok(NonrootAttributeValue::None);
    }
    if let Some(value) = value.unpack_bool() {
        return Ok(NonrootAttributeValue::Bool(value));
    }
    if let Some(value) = value.unpack_i32() {
        return NonrootAttributeValue::integer(&value.to_string()).map_err(anyhow::Error::msg);
    }
    // starlark-rust exposes only the i32 fast path publicly. For larger
    // Starlark integers, its canonical integer display is the source-backed
    // decimal spelling consumed immediately by the compact integer validator;
    // no type/repr marker is retained.
    if value.get_type() == "int" {
        return NonrootAttributeValue::integer(&value.to_string()).map_err(anyhow::Error::msg);
    }
    if let Some(value) = value.downcast_ref::<StarlarkFloat>() {
        if value.0.to_bits() == 3.14f64.to_bits() {
            return Ok(NonrootAttributeValue::Float314);
        }
        anyhow::bail!("unsupported deferred attribute float")
    }
    if let Some(value) = value.unpack_str() {
        return Ok(NonrootAttributeValue::String(value.into()));
    }
    if let Some(values) = ListRef::from_value(value) {
        if values.len() == 1 && values[0].identity() == identity {
            return Ok(NonrootAttributeValue::SelfList);
        }
        if !active.insert(identity) {
            anyhow::bail!("unsupported deferred attribute cycle")
        }
        let result = values
            .iter()
            .map(|value| snapshot_deferred_attribute_value(value, identities, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(NonrootAttributeValue::List);
        active.shift_remove(&identity);
        return result;
    }
    if let Some(values) = TupleRef::from_value(value) {
        if !active.insert(identity) {
            anyhow::bail!("unsupported deferred attribute cycle")
        }
        let result = values
            .iter()
            .map(|value| snapshot_deferred_attribute_value(value, identities, active))
            .collect::<anyhow::Result<Arc<_>>>()
            .map(NonrootAttributeValue::Tuple);
        active.shift_remove(&identity);
        return result;
    }
    if let Some(values) = DictRef::from_value(value) {
        if !active.insert(identity) {
            anyhow::bail!("unsupported deferred attribute cycle")
        }
        let result = values
            .iter()
            .map(|(key, value)| {
                let key = snapshot_deferred_attribute_key(key)?;
                Ok((
                    key,
                    snapshot_deferred_attribute_value(value, identities, active)?,
                ))
            })
            .collect::<anyhow::Result<SmallMap<_, _>>>()
            .map(|values| NonrootAttributeValue::Dict(Arc::new(values)));
        active.shift_remove(&identity);
        return result;
    }
    anyhow::bail!("unsupported deferred attribute value")
}

#[allow(dead_code)]
fn snapshot_deferred_attribute_key<'v>(value: Value<'v>) -> anyhow::Result<NonrootAttributeKey> {
    if let Some(value) = value.unpack_str() {
        return Ok(NonrootAttributeKey::String(value.into()));
    }
    if let Some(value) = value.downcast_ref::<StarlarkFloat>() {
        if value.0.to_bits() == 3.14f64.to_bits() {
            return Ok(NonrootAttributeKey::DeferredFloat314);
        }
    }
    anyhow::bail!("unsupported deferred attribute dictionary key")
}

// The non-root evaluator is deliberately private.  The later source/discovery
// owner supplies the file bytes and consumes this compact result.
#[allow(dead_code)]
#[derive(ProvidesStaticType)]
struct NonrootEvalContext {
    state: RefCell<NonrootEvalState>,
    include_indices: SmallMap<CompactString, usize>,
    file_ids: Vec<LogicalModuleFileId>,
}

#[allow(dead_code)]
struct NonrootEvalState {
    logical_id: LogicalModuleFileId,
    current_file: usize,
    module_called: bool,
    non_module_called: bool,
    builder: NonrootModuleBuilder,
    usages: Vec<NonrootUsageDraft>,
    roots: Vec<NonrootRoot>,
    repo_names: SmallSet<CompactString>,
}

#[allow(dead_code)]
struct NonrootUsageDraft {
    bzl_label: CompactString,
    extension_name: CompactString,
    active: bool,
    isolated: bool,
    proxies: Vec<NonrootProxyDraft>,
    tags: Vec<NonrootTagDraft>,
    exported_names: SmallSet<CompactString>,
}

#[allow(dead_code)]
struct NonrootProxyDraft {
    name: CompactString,
    dev_dependency: bool,
    location: LogicalSpan,
    imports: SmallMap<CompactString, CompactString>,
}

#[allow(dead_code)]
struct NonrootTagDraft {
    tag_class: CompactString,
    dev_dependency: bool,
    location: LogicalSpan,
    attributes: Option<SmallMap<CompactString, NonrootAttributeValue>>,
}

#[allow(dead_code)]
enum NonrootRootKind {
    Proxy,
    Tag { usage: usize, tag: usize },
}

#[allow(dead_code)]
struct NonrootRoot {
    file: usize,
    name: CompactString,
    kind: NonrootRootKind,
}

fn nonroot_context<'a>(eval: &'a Evaluator<'_, '_, '_>) -> anyhow::Result<&'a NonrootEvalContext> {
    eval.extra
        .and_then(|extra| extra.downcast_ref())
        .context("non-root MODULE global invoked without evaluator context")
}

fn nonroot_span(
    eval: &Evaluator<'_, '_, '_>,
    logical_id: &LogicalModuleFileId,
) -> anyhow::Result<LogicalSpan> {
    let span = eval
        .call_stack_top_location()
        .context("non-root MODULE directive has no source location")?
        .resolve_span();
    Ok(LogicalSpan {
        file: logical_id.clone(),
        start_line: u32::try_from(span.begin.line + 1).context("source line fits u32")?,
        start_column: u32::try_from(span.begin.column + 1).context("source column fits u32")?,
        end_line: u32::try_from(span.end.line + 1).context("source line fits u32")?,
        end_column: u32::try_from(span.end.column + 1).context("source column fits u32")?,
    })
}

fn nonroot_root_name(state: &NonrootEvalState, kind: &str) -> CompactString {
    CompactString::new(format!("\0slug:nonroot:{kind}:{}", state.roots.len()))
}

fn reserve_nonroot_repo_name(state: &mut NonrootEvalState, name: &str) -> anyhow::Result<()> {
    if !state.repo_names.insert(name.into()) {
        anyhow::bail!("repository name '{name}' is already defined");
    }
    Ok(())
}

fn validate_nonempty_repo_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        anyhow::bail!("repository name must not be empty");
    }
    validate_repo_name(name)
}

fn valid_starlark_identifier(value: &str) -> bool {
    value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphabetic() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
    }) && !value.is_empty()
}

fn normalize_nonroot_label(raw: &str, own_repo: &str) -> anyhow::Result<CompactString> {
    if raw.starts_with("@@") {
        let canonical = CanonicalLabel::parse(raw).map_err(anyhow::Error::msg)?;
        return Ok(canonical
            .to_string()
            .strip_prefix('@')
            .expect("canonical labels start with @@")
            .into());
    }
    let absolute = if raw.starts_with("@//") {
        format!("@{own_repo}{}", &raw[1..])
    } else if raw.starts_with('@') {
        raw.to_owned()
    } else if raw.starts_with("//") {
        format!("@{own_repo}{raw}")
    } else if raw.starts_with(':') {
        format!("@{own_repo}//{raw}")
    } else {
        format!("@{own_repo}//:{raw}")
    };
    ApparentLabel::parse(&absolute)
        .map(|label| label.to_string().into())
        .map_err(anyhow::Error::msg)
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ModuleExtensionProxy {
    usage: usize,
    proxy: usize,
}

starlark_simple_value!(ModuleExtensionProxy);

impl fmt::Display for ModuleExtensionProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("module_extension_proxy")
    }
}

#[starlark_value(type = "module_extension_proxy")]
impl<'v> StarlarkValue<'v> for ModuleExtensionProxy {
    type Canonical = Self;
    fn export_as(
        &self,
        variable_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        let mut state = nonroot_context(eval)
            .map_err(starlark::Error::new_other)?
            .state
            .borrow_mut();
        let usage = &mut state.usages[self.usage];
        let proxy = &mut usage.proxies[self.proxy];
        if proxy.name.is_empty() {
            proxy.name = variable_name.into();
            if usage.isolated && !proxy.dev_dependency {
                // The isolation key is constructed during finalization, after
                // the actual source binding is known.
            }
        }
        Ok(())
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        Some(heap.alloc(TagInvoker {
            usage: self.usage,
            proxy: self.proxy,
            tag_class: attribute.into(),
        }))
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct TagInvoker {
    usage: usize,
    proxy: usize,
    tag_class: CompactString,
}

starlark_simple_value!(TagInvoker);

impl fmt::Display for TagInvoker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", "module_extension_proxy", self.tag_class)
    }
}

fn reject_positions<'v>(
    args: &Arguments<'v, '_>,
    eval: &Evaluator<'v, '_, '_>,
) -> starlark::Result<()> {
    args.no_positional_args(eval.heap())
}

#[starlark_value(type = "module_extension_tag")]
impl<'v> StarlarkValue<'v> for TagInvoker {
    type Canonical = Self;
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        reject_positions(args, eval)?;
        let attributes = eval.heap().alloc(args.names_map()?);
        let mut state = nonroot_context(eval)
            .map_err(starlark::Error::new_other)?
            .state
            .borrow_mut();
        let logical_id = state.logical_id.clone();
        let span = nonroot_span(eval, &logical_id).map_err(starlark::Error::new_other)?;
        let (active, dev_dependency) = {
            let usage = &state.usages[self.usage];
            (usage.active, usage.proxies[self.proxy].dev_dependency)
        };
        if !active {
            return Ok(Value::new_none());
        }
        let root = nonroot_root_name(&state, "tag");
        eval.module().set(root.as_str(), attributes);
        let tag = state.usages[self.usage].tags.len();
        state.usages[self.usage].tags.push(NonrootTagDraft {
            tag_class: self.tag_class.clone(),
            dev_dependency,
            location: span,
            attributes: None,
        });
        let file = state.current_file;
        state.roots.push(NonrootRoot {
            file,
            name: root,
            kind: NonrootRootKind::Tag {
                usage: self.usage,
                tag,
            },
        });
        Ok(Value::new_none())
    }
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct RepoRuleProxy {
    usage: usize,
    rule_name: CompactString,
}

starlark_simple_value!(RepoRuleProxy);

impl fmt::Display for RepoRuleProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "repo_rule_proxy({})", self.rule_name)
    }
}

#[starlark_value(type = "repo_rule_proxy")]
impl<'v> StarlarkValue<'v> for RepoRuleProxy {
    type Canonical = Self;
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        reject_positions(args, eval)?;
        let mut attributes = args.names_map()?;
        let name = attributes
            .shift_remove("name")
            .and_then(|value| value.unpack_str().map(CompactString::from))
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!("repository rule requires name"))
            })?;
        validate_nonempty_repo_name(name.as_str()).map_err(starlark::Error::new_other)?;
        let dev_dependency = attributes
            .shift_remove("dev_dependency")
            .map(|value| {
                value.unpack_bool().ok_or_else(|| {
                    starlark::Error::new_other(anyhow::anyhow!("dev_dependency must be bool"))
                })
            })
            .transpose()?
            .unwrap_or(false);
        if dev_dependency {
            return Ok(Value::new_none());
        }
        let name_key = eval.heap().alloc_str("name");
        let name_value = eval.heap().alloc_str(name.as_str()).to_value();
        attributes.insert(name_key, name_value);
        let attrs = eval.heap().alloc(attributes);
        let mut state = nonroot_context(eval)
            .map_err(starlark::Error::new_other)?
            .state
            .borrow_mut();
        reserve_nonroot_repo_name(&mut state, name.as_str()).map_err(starlark::Error::new_other)?;
        if !state.usages[self.usage].exported_names.insert(name.clone()) {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "extension import exports the same name twice"
            )));
        }
        let logical_id = state.logical_id.clone();
        let span = nonroot_span(eval, &logical_id).map_err(starlark::Error::new_other)?;
        let root = nonroot_root_name(&state, "innate");
        eval.module().set(root.as_str(), attrs);
        state.usages[self.usage].proxies.push(NonrootProxyDraft {
            // This internal extension proxy is created inside the repo-rule
            // invocation, so no source assignment ever exports it.
            name: CompactString::new(""),
            dev_dependency: false,
            location: span.clone(),
            imports: SmallMap::from_iter([(name.clone(), name)]),
        });
        let tag = state.usages[self.usage].tags.len();
        state.usages[self.usage].tags.push(NonrootTagDraft {
            tag_class: "repo".into(),
            dev_dependency: false,
            location: span,
            attributes: None,
        });
        let file = state.current_file;
        state.roots.push(NonrootRoot {
            file,
            name: root,
            kind: NonrootRootKind::Tag {
                usage: self.usage,
                tag,
            },
        });
        Ok(Value::new_none())
    }
}

#[allow(dead_code)]
struct RejectPrint;
impl PrintHandler for RejectPrint {
    fn println(&self, _: PrintLocation, _: &str) -> starlark::Result<()> {
        Err(starlark::Error::new_other(anyhow::anyhow!(
            "print() is not permitted in MODULE.bazel"
        )))
    }
}

fn proxy_from_value<'v>(value: Value<'v>) -> anyhow::Result<&'v ModuleExtensionProxy> {
    ModuleExtensionProxy::from_value(value).context("use_repo() requires a module extension proxy")
}

fn register_import(
    state: &mut NonrootEvalState,
    proxy: &ModuleExtensionProxy,
    local: &str,
    exported: &str,
) -> anyhow::Result<()> {
    validate_nonempty_repo_name(local)?;
    validate_nonempty_repo_name(exported)?;
    reserve_nonroot_repo_name(state, local)?;
    if !state.usages[proxy.usage]
        .exported_names
        .insert(exported.into())
    {
        anyhow::bail!("extension import exports the same name twice");
    }
    let proxy_draft = &mut state.usages[proxy.usage].proxies[proxy.proxy];
    if proxy_draft
        .imports
        .insert(local.into(), exported.into())
        .is_some()
    {
        anyhow::bail!("extension import local name is repeated");
    }
    Ok(())
}

#[derive(Debug)]
enum NonrootIncludeError {
    BadLabel(CompactString),
    MissingSuppliedFile {
        label: CompactString,
        location: Option<LogicalSpan>,
    },
    DuplicateSuppliedFile(CompactString),
    UnreachableSuppliedFile(CompactString),
}

impl fmt::Display for NonrootIncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLabel(message) => f.write_str(message),
            Self::MissingSuppliedFile { label, location } => {
                if let Some(location) = location {
                    write!(
                        f,
                        "{}:{}:{}: include() has no supplied non-registry file for `{label}`",
                        location.file.0, location.start_line, location.start_column
                    )
                } else {
                    write!(
                        f,
                        "include() has no supplied non-registry file for `{label}`"
                    )
                }
            }
            Self::DuplicateSuppliedFile(label) => {
                write!(f, "duplicate supplied include label `{label}`")
            }
            Self::UnreachableSuppliedFile(label) => {
                write!(f, "supplied include label `{label}` is not reachable")
            }
        }
    }
}

impl std::error::Error for NonrootIncludeError {}

#[starlark_module]
fn nonroot_module_globals(builder: &mut GlobalsBuilder) {
    fn include(label: &str, eval: &mut Evaluator) -> starlark::Result<NoneType> {
        include_path(Path::new("."), label)
            .map_err(NonrootIncludeError::BadLabel)
            .map_err(starlark::Error::new_other)?;
        let extra = eval.extra;
        let context = extra
            .and_then(|extra| extra.downcast_ref::<NonrootEvalContext>())
            .context("non-root include invoked without evaluator context")
            .map_err(starlark::Error::new_other)?;
        let index = *context.include_indices.get(label).ok_or_else(|| {
            starlark::Error::new_other(NonrootIncludeError::MissingSuppliedFile {
                label: label.into(),
                location: None,
            })
        })?;
        let previous = {
            let mut state = context.state.borrow_mut();
            let previous = (state.logical_id.clone(), state.current_file);
            state.logical_id = context.file_ids[index].clone();
            state.current_file = index;
            previous
        };
        let result = eval.eval_prepared_module_index(index);
        let mut state = context.state.borrow_mut();
        state.logical_id = previous.0;
        state.current_file = previous.1;
        result?;
        Ok(NoneType)
    }

    fn module(
        #[starlark(require = named, default = "")] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] compatibility_level: i32,
        #[starlark(require = named, default = "")] repo_name: &str,
        #[starlark(require = named, default = UnpackList::default())]
        bazel_compatibility: UnpackList<&str>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        if state.module_called {
            anyhow::bail!("module() can only be called once");
        }
        if state.non_module_called {
            anyhow::bail!("if module() is called, it must be called before any other functions");
        }
        if !name.is_empty() {
            validate_module_name(name)?;
        }
        validate_version(version, "module")?;
        let repo_name = if repo_name.is_empty() {
            name
        } else {
            repo_name
        };
        validate_repo_name(repo_name)?;
        for value in &bazel_compatibility.items {
            validate_bazel_compatibility(value)?;
        }
        state.module_called = true;
        reserve_nonroot_repo_name(&mut state, repo_name)?;
        state.builder.declared_name = name.into();
        state.builder.declared_version = normalize_version(version);
        state.builder.repo_name = repo_name.into();
        state.builder.bazel_compatibility = bazel_compatibility
            .items
            .into_iter()
            .map(Into::into)
            .collect();
        let _ = compatibility_level;
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
        validate_module_name(name)?;
        validate_version(version, "bazel_dep")?;
        if let Some(NoneOr::Other(repo)) = repo_name {
            validate_repo_name(repo)?;
        }
        let (repo, nodep) = match repo_name {
            Some(NoneOr::None) => (name, true),
            Some(NoneOr::Other("")) | None => (name, false),
            Some(NoneOr::Other(repo)) => (repo, false),
        };
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !nodep {
            reserve_nonroot_repo_name(&mut state, repo)?;
        }
        if !dev_dependency {
            let dep = NonrootDependency::new(name, normalize_version(version));
            if nodep {
                state.builder.nodep_dependencies.push(dep);
            } else if state
                .builder
                .dependencies
                .insert(repo.into(), dep)
                .is_some()
            {
                anyhow::bail!("bazel_dep repo name is repeated");
            }
        }
        let _ = max_compatibility_level;
        Ok(NoneType)
    }

    fn register_execution_platforms(
        #[starlark(args)] labels: Value,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let labels =
            TupleRef::from_value(labels).context("register_execution_platforms expects labels")?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !dev_dependency {
            for label in labels.iter() {
                let label = label
                    .unpack_str()
                    .context("registration labels must be strings")?;
                if !label.starts_with("//") && !label.starts_with('@') {
                    anyhow::bail!("registration labels must be absolute target patterns");
                }
                state.builder.execution_platforms.push(label.into());
            }
        }
        Ok(NoneType)
    }

    fn register_toolchains(
        #[starlark(args)] labels: Value,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let labels = TupleRef::from_value(labels).context("register_toolchains expects labels")?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !dev_dependency {
            for label in labels.iter() {
                let label = label
                    .unpack_str()
                    .context("registration labels must be strings")?;
                if !label.starts_with("//") && !label.starts_with('@') {
                    anyhow::bail!("registration labels must be absolute target patterns");
                }
                state.builder.toolchains.push(label.into());
            }
        }
        Ok(NoneType)
    }

    fn flag_alias(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] starlark_flag: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        let starlark_flag =
            normalize_nonroot_label(starlark_flag, state.builder.repo_name.as_str())?;
        if state
            .builder
            .flag_aliases
            .insert(name.into(), starlark_flag)
            .is_some()
        {
            anyhow::bail!("flag alias is repeated");
        }
        Ok(NoneType)
    }

    fn use_extension<'v>(
        bzl_file: &str,
        extension_name: &str,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        #[starlark(require = named, default = false)] isolate: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<Value<'v>> {
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !valid_starlark_identifier(extension_name) {
            anyhow::bail!("extension name is not a valid identifier: {extension_name}");
        }
        let bzl_file = normalize_nonroot_label(bzl_file, state.builder.repo_name.as_str())?;
        let logical_id = state.logical_id.clone();
        let location = nonroot_span(eval, &logical_id)?;
        let usage = if !isolate && !dev_dependency {
            state
                .usages
                .iter()
                .position(|usage| {
                    usage.active
                        && !usage.isolated
                        && usage.bzl_label == bzl_file
                        && usage.extension_name == extension_name
                })
                .unwrap_or_else(|| {
                    state.usages.push(NonrootUsageDraft {
                        bzl_label: bzl_file.clone(),
                        extension_name: extension_name.into(),
                        active: true,
                        isolated: false,
                        proxies: Vec::new(),
                        tags: Vec::new(),
                        exported_names: SmallSet::new(),
                    });
                    state.usages.len() - 1
                })
        } else {
            state.usages.push(NonrootUsageDraft {
                bzl_label: bzl_file,
                extension_name: extension_name.into(),
                active: !dev_dependency,
                isolated: isolate,
                proxies: Vec::new(),
                tags: Vec::new(),
                exported_names: SmallSet::new(),
            });
            state.usages.len() - 1
        };
        let root = nonroot_root_name(&state, "proxy");
        let proxy = state.usages[usage].proxies.len();
        state.usages[usage].proxies.push(NonrootProxyDraft {
            name: CompactString::new(""),
            dev_dependency,
            location,
            imports: SmallMap::new(),
        });
        let file = state.current_file;
        state.roots.push(NonrootRoot {
            file,
            name: root.clone(),
            kind: NonrootRootKind::Proxy,
        });
        let value = eval.heap().alloc(ModuleExtensionProxy { usage, proxy });
        eval.module().set(root.as_str(), value);
        Ok(value)
    }

    fn use_repo<'v>(
        #[starlark(require = pos)] proxy: Value<'v>,
        #[starlark(args)] repos: Value<'v>,
        #[starlark(kwargs)] aliases: DictRef<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let proxy = proxy_from_value(proxy)?;
        let repos = TupleRef::from_value(repos)
            .context("use_repo positional arguments must be repository names")?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        let module_name = state.builder.declared_name.clone();
        let module_version = state.builder.declared_version.clone();
        for repo in repos.iter() {
            let repo = repo
                .unpack_str()
                .context("repository names must be strings")?;
            register_import(&mut state, proxy, repo, repo)?;
        }
        for (local, exported) in aliases.iter() {
            let local = local
                .unpack_str()
                .context("repository names must be strings")?;
            let exported = exported
                .unpack_str()
                .context("repository names must be strings")?
                .replace("{name}", module_name.as_str())
                .replace("{version}", module_version.as_str());
            register_import(&mut state, proxy, local, exported.as_str())?;
        }
        Ok(NoneType)
    }

    fn override_repo<'v>(
        #[starlark(require = pos)] proxy: Value<'v>,
        #[starlark(args)] _repos: Value<'v>,
        #[starlark(kwargs)] _aliases: DictRef<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let _ = proxy_from_value(proxy)?;
        nonroot_context(eval)?.state.borrow_mut().non_module_called = true;
        Ok(NoneType)
    }
    fn inject_repo<'v>(
        #[starlark(require = pos)] proxy: Value<'v>,
        #[starlark(args)] _repos: Value<'v>,
        #[starlark(kwargs)] _aliases: DictRef<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let _ = proxy_from_value(proxy)?;
        nonroot_context(eval)?.state.borrow_mut().non_module_called = true;
        Ok(NoneType)
    }

    fn use_repo_rule(
        bzl_file: &str,
        rule_name: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<RepoRuleProxy> {
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        let extension_name = CompactString::new(format!("{bzl_file} {rule_name}"));
        let usage = state
            .usages
            .iter()
            .position(|usage| {
                usage.active
                    && !usage.isolated
                    && usage.bzl_label == "//:MODULE.bazel"
                    && usage.extension_name == extension_name
            })
            .unwrap_or_else(|| {
                state.usages.push(NonrootUsageDraft {
                    bzl_label: "//:MODULE.bazel".into(),
                    extension_name,
                    active: true,
                    isolated: false,
                    proxies: Vec::new(),
                    tags: Vec::new(),
                    exported_names: SmallSet::new(),
                });
                state.usages.len() - 1
            });
        Ok(RepoRuleProxy {
            usage,
            rule_name: rule_name.into(),
        })
    }

    fn local_path_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] path: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_module_name(module_name)?;
        let _ = path;
        nonroot_context(eval)?.state.borrow_mut().non_module_called = true;
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
        validate_module_name(module_name)?;
        validate_version(version, "single_version_override")?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        for patch in patches.items {
            let header = RootModuleHeader {
                name: state.builder.declared_name.clone(),
                version: Some(state.builder.declared_version.clone()),
                repo_name: Some(state.builder.repo_name.clone()),
            };
            let _ = normalize_patch_label(patch, Some(&header))?;
        }
        let _ = (registry, patch_cmds, patch_strip);
        state.non_module_called = true;
        Ok(NoneType)
    }
    fn multiple_version_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] versions: UnpackList<&str>,
        #[starlark(require = named, default = "")] registry: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_module_name(module_name)?;
        if versions.items.len() < 2 {
            anyhow::bail!("multiple_version_override() requires at least two versions");
        }
        for version in versions.items {
            validate_version(version, "multiple_version_override")?;
        }
        let _ = registry;
        nonroot_context(eval)?.state.borrow_mut().non_module_called = true;
        Ok(NoneType)
    }
    fn archive_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(kwargs)] attrs: DictRef<'v>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_module_name(module_name)?;
        let patches = patch_strings(&attrs)?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        let header = RootModuleHeader {
            name: state.builder.declared_name.clone(),
            version: Some(state.builder.declared_version.clone()),
            repo_name: Some(state.builder.repo_name.clone()),
        };
        for patch in patches.iter() {
            let _ = normalize_patch_label(patch.as_str(), Some(&header))?;
        }
        state.non_module_called = true;
        Ok(NoneType)
    }
    fn git_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(kwargs)] attrs: DictRef<'v>,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        validate_module_name(module_name)?;
        let patches = patch_strings(&attrs)?;
        let mut state = nonroot_context(eval)?.state.borrow_mut();
        let header = RootModuleHeader {
            name: state.builder.declared_name.clone(),
            version: Some(state.builder.declared_version.clone()),
            repo_name: Some(state.builder.repo_name.clone()),
        };
        for patch in patches.iter() {
            let _ = normalize_patch_label(patch.as_str(), Some(&header))?;
        }
        state.non_module_called = true;
        Ok(NoneType)
    }
}

#[allow(dead_code)]
fn evaluate_nonroot_module_file(
    expected_key: NonrootModuleKey,
    logical_id: LogicalModuleFileId,
    source: &[u8],
    force_gc_after_eval: bool,
) -> anyhow::Result<EvaluatedNonrootModule> {
    evaluate_nonroot_module_file_with_includes(
        expected_key,
        logical_id,
        source,
        &[],
        force_gc_after_eval,
    )
}

#[allow(dead_code)]
struct SuppliedNonrootModuleFile<'a> {
    raw_label: &'a str,
    logical_id: LogicalModuleFileId,
    source: &'a [u8],
}

pub(crate) struct DirectNonregistryIncludeFile<'a> {
    pub(crate) raw_label: &'a str,
    pub(crate) logical_id: LogicalModuleFileId,
    pub(crate) source: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(crate) enum DirectNonregistryEvaluationError {
    Preparation(CompactString),
    Execution(CompactString),
    Finalization(CompactString),
    DeclaredNameMismatch {
        expected: NonrootModuleKey,
        declared: CompactString,
    },
    DeclaredVersionMismatch {
        expected: NonrootModuleKey,
        declared: CompactString,
    },
}

impl fmt::Display for DirectNonregistryEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preparation(message) => write!(f, "failed to prepare MODULE closure: {message}"),
            Self::Execution(message) => write!(f, "failed to execute MODULE closure: {message}"),
            Self::Finalization(message) => {
                write!(f, "failed to finalize MODULE closure: {message}")
            }
            Self::DeclaredNameMismatch { expected, declared } => write!(
                f,
                "the MODULE.bazel file of {}@{} declares a different name ({declared})",
                expected.name, expected.version
            ),
            Self::DeclaredVersionMismatch { expected, declared } => write!(
                f,
                "the MODULE.bazel file of {}@{} declares a different version ({declared})",
                expected.name, expected.version
            ),
        }
    }
}

impl std::error::Error for DirectNonregistryEvaluationError {}

enum NonrootEvaluationFailure {
    Preparation(anyhow::Error),
    Execution(anyhow::Error),
    Finalization(anyhow::Error),
}

impl NonrootEvaluationFailure {
    fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Preparation(error) | Self::Execution(error) | Self::Finalization(error) => error,
        }
    }
}

impl From<NonrootEvaluationFailure> for DirectNonregistryEvaluationError {
    fn from(error: NonrootEvaluationFailure) -> Self {
        match error {
            NonrootEvaluationFailure::Preparation(error) => {
                Self::Preparation(CompactString::new(error.to_string()))
            }
            NonrootEvaluationFailure::Execution(error) => {
                Self::Execution(CompactString::new(error.to_string()))
            }
            NonrootEvaluationFailure::Finalization(error) => {
                Self::Finalization(CompactString::new(error.to_string()))
            }
        }
    }
}

#[allow(dead_code)]
fn evaluate_nonroot_module_file_with_includes(
    expected_key: NonrootModuleKey,
    logical_id: LogicalModuleFileId,
    source: &[u8],
    supplied: &[SuppliedNonrootModuleFile<'_>],
    force_gc_after_eval: bool,
) -> anyhow::Result<EvaluatedNonrootModule> {
    let mut include_indices = SmallMap::with_capacity(supplied.len());
    let mut file_ids = Vec::with_capacity(supplied.len() + 1);
    file_ids.push(logical_id.clone());
    for (index, file) in supplied.iter().enumerate() {
        include_path(Path::new("."), file.raw_label).map_err(NonrootIncludeError::BadLabel)?;
        if include_indices
            .insert(CompactString::from(file.raw_label), index + 1)
            .is_some()
        {
            return Err(NonrootIncludeError::DuplicateSuppliedFile(file.raw_label.into()).into());
        }
        file_ids.push(file.logical_id.clone());
    }
    let inspections = std::iter::once((logical_id.clone(), source))
        .chain(
            supplied
                .iter()
                .map(|file| (file.logical_id.clone(), file.source)),
        )
        .map(|(logical_id, source)| inspect_nonroot_module_file(logical_id, source))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut reachable = vec![false; supplied.len() + 1];
    let mut horizon = VecDeque::from([0]);
    reachable[0] = true;
    while let Some(file_index) = horizon.pop_front() {
        for request in inspections[file_index].includes.iter() {
            include_path(Path::new("."), request.path.as_str())
                .map_err(NonrootIncludeError::BadLabel)?;
            let included_index = *include_indices.get(request.path.as_str()).ok_or_else(|| {
                NonrootIncludeError::MissingSuppliedFile {
                    label: request.path.clone(),
                    location: Some(request.location.clone()),
                }
            })?;
            if !reachable[included_index] {
                reachable[included_index] = true;
                horizon.push_back(included_index);
            }
        }
    }
    if let Some((index, _)) = reachable
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, reachable)| !**reachable)
    {
        return Err(NonrootIncludeError::UnreachableSuppliedFile(
            supplied[index - 1].raw_label.into(),
        )
        .into());
    }
    let reject_print = RejectPrint;
    let evaluated = evaluate_nonroot_module_closure(
        expected_key,
        logical_id,
        source,
        supplied,
        include_indices,
        file_ids,
        Some(&reject_print),
        force_gc_after_eval,
        true,
    )
    .map_err(NonrootEvaluationFailure::into_anyhow)?;
    Ok(evaluated)
}

pub(crate) fn evaluate_direct_nonregistry_module_closure_with_events(
    expected_key: NonrootModuleKey,
    logical_id: LogicalModuleFileId,
    source: &[u8],
    included: &[DirectNonregistryIncludeFile<'_>],
    capture_events: bool,
) -> (
    Result<EvaluatedNonrootModule, DirectNonregistryEvaluationError>,
    Option<EventBatch>,
) {
    let capture = capture_events.then(RootModulePrintCapture::default);
    let value = (|| -> Result<_, NonrootEvaluationFailure> {
        let supplied = included
            .iter()
            .map(|file| SuppliedNonrootModuleFile {
                raw_label: file.raw_label,
                logical_id: file.logical_id.clone(),
                source: file.source,
            })
            .collect::<Vec<_>>();
        let mut include_indices = SmallMap::with_capacity(supplied.len());
        let mut file_ids = Vec::with_capacity(supplied.len() + 1);
        file_ids.push(logical_id.clone());
        for (index, file) in supplied.iter().enumerate() {
            include_path(Path::new("."), file.raw_label)
                .map_err(NonrootIncludeError::BadLabel)
                .map_err(anyhow::Error::new)
                .map_err(NonrootEvaluationFailure::Preparation)?;
            include_indices.insert(CompactString::from(file.raw_label), index + 1);
            file_ids.push(file.logical_id.clone());
        }
        let evaluated = evaluate_nonroot_module_closure(
            expected_key.clone(),
            logical_id,
            source,
            &supplied,
            include_indices,
            file_ids,
            capture.as_ref().map(|capture| capture as &dyn PrintHandler),
            false,
            false,
        )?;
        Ok(evaluated)
    })()
    .map_err(DirectNonregistryEvaluationError::from)
    .and_then(|evaluated| validate_nonroot_module_identity(expected_key, evaluated));
    (value, capture.map(RootModulePrintCapture::into_batch))
}

fn validate_nonroot_module_identity(
    expected: NonrootModuleKey,
    evaluated: EvaluatedNonrootModule,
) -> Result<EvaluatedNonrootModule, DirectNonregistryEvaluationError> {
    if evaluated.base.declared_name != expected.name {
        return Err(DirectNonregistryEvaluationError::DeclaredNameMismatch {
            declared: evaluated.base.declared_name.clone(),
            expected,
        });
    }
    if !expected.version.is_empty() && evaluated.base.declared_version != expected.version {
        return Err(DirectNonregistryEvaluationError::DeclaredVersionMismatch {
            declared: evaluated.base.declared_version.clone(),
            expected,
        });
    }
    Ok(evaluated)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_nonroot_module_closure(
    expected_key: NonrootModuleKey,
    logical_id: LogicalModuleFileId,
    source: &[u8],
    supplied: &[SuppliedNonrootModuleFile<'_>],
    include_indices: SmallMap<CompactString, usize>,
    file_ids: Vec<LogicalModuleFileId>,
    print_handler: Option<&dyn PrintHandler>,
    force_gc_after_eval: bool,
    seed_expected_declaration: bool,
) -> Result<EvaluatedNonrootModule, NonrootEvaluationFailure> {
    let (root_ast, supplied_asts) = (|| {
        let source = std::str::from_utf8(source).context("MODULE file is not valid UTF-8")?;
        let root_ast = AstModule::parse(
            logical_id.0.as_str(),
            source.to_owned(),
            &nonroot_module_dialect(),
        )
        .map_err(starlark::Error::into_anyhow)?;
        let supplied_asts = supplied
            .iter()
            .map(|file| {
                let source = std::str::from_utf8(file.source)
                    .context("included MODULE file is not valid UTF-8")?;
                AstModule::parse(
                    file.logical_id.0.as_str(),
                    source.to_owned(),
                    &nonroot_module_dialect(),
                )
                .map_err(starlark::Error::into_anyhow)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok::<_, anyhow::Error>((root_ast, supplied_asts))
    })()
    .map_err(NonrootEvaluationFailure::Preparation)?;
    let builder = if seed_expected_declaration {
        NonrootModuleBuilder::new(
            expected_key.clone(),
            expected_key.name.clone(),
            expected_key.version.clone(),
            expected_key.name.clone(),
        )
    } else {
        NonrootModuleBuilder::new(expected_key, "", "", "")
    };
    let context = NonrootEvalContext {
        state: RefCell::new(NonrootEvalState {
            logical_id: logical_id.clone(),
            current_file: 0,
            module_called: false,
            non_module_called: false,
            builder,
            usages: Vec::new(),
            roots: Vec::new(),
            repo_names: SmallSet::new(),
        }),
        include_indices,
        file_ids,
    };
    let globals = GlobalsBuilder::extended_by(&[LibraryExtension::Print])
        .with(nonroot_module_globals)
        .build();
    let module = Module::new();
    let included_modules: Vec<_> = supplied.iter().map(|_| Box::new(Module::new())).collect();
    let programs = (|| {
        let mut evaluator = Evaluator::new(&module);
        let mut programs = Vec::with_capacity(supplied.len() + 1);
        programs.push(
            evaluator
                .prepare_module(root_ast, &globals)
                .map_err(starlark::Error::into_anyhow)?,
        );
        for (index, ast) in supplied_asts.into_iter().enumerate() {
            programs.push(
                evaluator
                    .prepare_module_in(included_modules[index].as_ref(), ast, &globals)
                    .map_err(starlark::Error::into_anyhow)?,
            );
        }
        Ok::<_, anyhow::Error>(programs)
    })()
    .map_err(NonrootEvaluationFailure::Preparation)?;
    let mut evaluator = Evaluator::new(&module);
    evaluator.extra = Some(&context);
    if let Some(print_handler) = print_handler {
        evaluator.set_print_handler(print_handler);
    }
    evaluator
        .set_prepared_modules(programs)
        .map_err(starlark::Error::into_anyhow)
        .map_err(NonrootEvaluationFailure::Preparation)?;
    evaluator
        .eval_prepared_module_index(0)
        .map_err(starlark::Error::into_anyhow)
        .map_err(NonrootEvaluationFailure::Execution)?;
    drop(evaluator);
    let modules: Vec<&Module> = std::iter::once(&module)
        .chain(included_modules.iter().map(Box::as_ref))
        .collect();
    if force_gc_after_eval {
        // `Evaluator::garbage_collect` is not usable on the cleared frame
        // left by `eval_module` (it asserts in starlark-rust). Allocate
        // unreachable evaluator-heap data, then use a fresh evaluator on the
        // same Module: its first statement performs the normal safe possible
        // GC while every hidden module slot is a root.
        for (index, module) in modules.iter().enumerate() {
            for _ in 0..2048 {
                let _ = module.heap().alloc_str(
                    "nonroot-gc-probe-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                );
            }
            let before = module.heap().allocated_bytes();
            let probe_name = format!("nonroot-gc-probe-{index}.MODULE.bazel");
            let probe = AstModule::parse(
                &probe_name,
                "gc_probe = 1".to_owned(),
                &nonroot_module_dialect(),
            )
            .map_err(starlark::Error::into_anyhow)
            .map_err(NonrootEvaluationFailure::Finalization)?;
            let mut evaluator = Evaluator::new(module);
            evaluator
                .eval_module(probe, &globals)
                .map_err(starlark::Error::into_anyhow)
                .map_err(NonrootEvaluationFailure::Finalization)?;
            if module.heap().allocated_bytes() >= before / 2 {
                return Err(NonrootEvaluationFailure::Finalization(anyhow::anyhow!(
                    "non-root GC probe did not collect unreachable evaluator data for file {index}"
                )));
            }
        }
    }
    (|| {
        let builtin_print = globals
            .iter()
            .find_map(|(name, value)| (name == "print").then_some(value.to_value().identity()))
            .context("print global is absent")?;
        let mut state = context.state.into_inner();
        let mut proxy_ids = SmallSet::new();
        for root in &state.roots {
            if matches!(root.kind, NonrootRootKind::Proxy) {
                proxy_ids.insert(
                    modules[root.file]
                        .get(root.name.as_str())
                        .context("missing hidden proxy root")?
                        .identity(),
                );
            }
        }
        let identities = deferred_attribute_snapshot_identities(builtin_print, proxy_ids);
        let roots: Vec<_> = state
            .roots
            .iter()
            .filter_map(|root| match root.kind {
                NonrootRootKind::Tag { usage, tag } => {
                    Some((root.file, root.name.clone(), usage, tag))
                }
                _ => None,
            })
            .collect();
        for (file, name, usage, tag) in roots {
            let attrs = DictRef::from_value(
                modules[file]
                    .get(name.as_str())
                    .context("missing hidden tag root")?,
            )
            .context("hidden tag root is not a dict")?;
            let values = snapshot_deferred_attribute_values(attrs, &identities)?;
            state.usages[usage].tags[tag].attributes = Some(values);
        }
        finalize_nonroot_state(state)
    })()
    .map_err(NonrootEvaluationFailure::Finalization)
}

#[allow(dead_code)]
fn finalize_nonroot_state(mut state: NonrootEvalState) -> anyhow::Result<EvaluatedNonrootModule> {
    if state.builder.expected_key.name != "bazel_tools" && state.repo_names.contains("bazel_tools")
    {
        anyhow::bail!("bazel_tools is a built-in dependency and its repo name is reserved");
    }
    let usages = std::mem::take(&mut state.usages);
    for usage in usages {
        if !usage.active {
            continue;
        }
        let isolation = if usage.isolated {
            anyhow::ensure!(
                usage.proxies.len() == 1,
                "isolated extension must have exactly one proxy"
            );
            let proxy = usage
                .proxies
                .first()
                .context("isolated extension has no proxy")?;
            if proxy.name.is_empty() {
                anyhow::bail!("isolated extension proxy must be assigned");
            }
            Some(NonrootExtensionIsolationKey {
                module: state.builder.expected_key.clone(),
                exported_proxy_name: proxy.name.clone(),
            })
        } else {
            None
        };
        let proxies = usage
            .proxies
            .iter()
            .filter(|proxy| !proxy.dev_dependency)
            .map(|proxy| {
                Ok(NonrootExtensionProxy {
                    proxy_name: proxy.name.clone(),
                    containing_file: proxy.location.file.clone(),
                    dev_dependency: proxy.dev_dependency,
                    location: proxy.location.clone(),
                    imports: NonrootRepoImports::from_local_to_exported(proxy.imports.clone())
                        .map_err(anyhow::Error::msg)?,
                })
            })
            .collect::<anyhow::Result<Arc<_>>>()?;
        let tags = usage
            .tags
            .into_iter()
            .map(|tag| {
                Ok(NonrootExtensionTag {
                    tag_class: tag.tag_class,
                    attributes: Arc::new(tag.attributes.context("missing tag snapshot")?),
                    dev_dependency: tag.dev_dependency,
                    location: tag.location,
                })
            })
            .collect::<anyhow::Result<Arc<_>>>()?;
        state.builder.extension_usages.push(NonrootExtensionUsage {
            bzl_label: usage.bzl_label.clone(),
            extension_name: usage.extension_name.clone(),
            proxies,
            tags,
            repo_overrides: Arc::new(SmallMap::new()),
            isolation,
        });
    }
    state.builder.build().map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod nonroot_directive_evaluator_tests {
    use super::*;

    fn evaluate(source: &str) -> anyhow::Result<EvaluatedNonrootModule> {
        evaluate_nonroot_module_file(
            NonrootModuleKey::new("subject", "1.0"),
            LogicalModuleFileId::new("@@subject+//:MODULE.bazel"),
            source.as_bytes(),
            true,
        )
    }

    fn evaluate_with_includes(
        source: &str,
        supplied: &[SuppliedNonrootModuleFile<'_>],
    ) -> anyhow::Result<EvaluatedNonrootModule> {
        evaluate_nonroot_module_file_with_includes(
            NonrootModuleKey::new("subject", "1.0"),
            LogicalModuleFileId::new("@@subject+//:MODULE.bazel"),
            source.as_bytes(),
            supplied,
            true,
        )
    }

    fn evaluate_direct<'a>(
        expected: NonrootModuleKey,
        source: &str,
        included: &[DirectNonregistryIncludeFile<'a>],
        capture: bool,
    ) -> (
        Result<EvaluatedNonrootModule, DirectNonregistryEvaluationError>,
        Option<EventBatch>,
    ) {
        evaluate_direct_nonregistry_module_closure_with_events(
            expected,
            LogicalModuleFileId::new("@@subject+//:MODULE.bazel"),
            source.as_bytes(),
            included,
            capture,
        )
    }

    #[test]
    fn strict_supplied_file_seam_keeps_expected_seed_without_identity_validation() {
        let omitted = evaluate("").unwrap();
        assert_eq!(omitted.base.declared_name, "subject");
        assert_eq!(omitted.base.declared_version, "1.0");
        assert_eq!(omitted.base.repo_name, "subject");

        let mismatched = evaluate("module(name='other', version='2.0')").unwrap();
        assert_eq!(
            mismatched.base.expected_key,
            NonrootModuleKey::new("subject", "1.0")
        );
        assert_eq!(mismatched.base.declared_name, "other");
        assert_eq!(mismatched.base.declared_version, "2.0");
    }

    #[test]
    fn shared_core_installs_the_prepared_table_in_the_preparation_stage() {
        let source = include_str!("module_eval.rs");
        let core = source
            .split("fn evaluate_nonroot_module_closure(")
            .nth(1)
            .unwrap()
            .split("fn finalize_nonroot_state")
            .next()
            .unwrap();
        let install = core.find(".set_prepared_modules(programs)").unwrap();
        let execute = core.find(".eval_prepared_module_index(0)").unwrap();
        assert!(install < execute);
        assert!(core[install..execute].contains("NonrootEvaluationFailure::Preparation"));
    }

    #[test]
    fn direct_adapter_separates_expected_identity_from_empty_declarations() {
        let (omitted, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "print('prefix')",
            &[],
            true,
        );
        assert!(matches!(
            omitted,
            Err(DirectNonregistryEvaluationError::DeclaredNameMismatch {
                expected,
                declared,
            }) if expected == NonrootModuleKey::new("subject", "") && declared.is_empty()
        ));
        assert!(matches!(
            batch.as_ref().map(EventBatch::events),
            Some([EvaluationEvent::StarlarkPrint { text, .. }]) if text == "prefix"
        ));

        let (name_first, _) = evaluate_direct(
            NonrootModuleKey::new("subject", "1.0"),
            "module(name='other', version='2.0')",
            &[],
            false,
        );
        assert!(matches!(
            name_first,
            Err(DirectNonregistryEvaluationError::DeclaredNameMismatch {
                expected,
                declared,
            }) if expected.version == "1.0" && declared == "other"
        ));

        let (version, _) = evaluate_direct(
            NonrootModuleKey::new("subject", "1.0"),
            "module(name='subject', version='2.0')",
            &[],
            false,
        );
        assert!(matches!(
            version,
            Err(DirectNonregistryEvaluationError::DeclaredVersionMismatch {
                expected,
                declared,
            }) if expected.version == "1.0" && declared == "2.0"
        ));

        let (accepted, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "module(name='subject', version='2.0')",
            &[],
            false,
        );
        let accepted = accepted.unwrap();
        assert_eq!(accepted.base.expected_key.version, "");
        assert_eq!(accepted.base.declared_version, "2.0");
        assert_eq!(accepted.base.repo_name, "subject");
        assert!(batch.is_none());
    }

    #[test]
    fn direct_adapter_prepares_every_occurrence_and_uses_last_raw_label() {
        let included = [
            DirectNonregistryIncludeFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:first.MODULE.bazel"),
                source: b"print('first')",
            },
            DirectNonregistryIncludeFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:last.MODULE.bazel"),
                source: b"print('last')",
            },
        ];
        let (evaluated, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "module(name='subject')\ninclude('//:same.MODULE.bazel')\ninclude('//:same.MODULE.bazel')",
            &included,
            true,
        );
        evaluated.unwrap();
        let events = batch.unwrap();
        assert!(matches!(
            events.events(),
            [
                EvaluationEvent::StarlarkPrint { location: first, text: first_text },
                EvaluationEvent::StarlarkPrint { location: second, text: second_text },
            ] if first_text == "last"
                && second_text == "last"
                && first.to_string().contains("last.MODULE.bazel")
                && second.to_string().contains("last.MODULE.bazel")
        ));

        let invalid = [
            DirectNonregistryIncludeFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:invalid.MODULE.bazel"),
                source: b"value = undefined_earlier_occurrence",
            },
            DirectNonregistryIncludeFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:valid.MODULE.bazel"),
                source: b"value = 1",
            },
        ];
        let (error, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "module(name='subject')\nprint('root-not-run')\ninclude('//:same.MODULE.bazel')",
            &invalid,
            true,
        );
        assert!(matches!(
            error,
            Err(DirectNonregistryEvaluationError::Preparation(message))
                if message.contains("undefined_earlier_occurrence")
        ));
        assert!(batch.unwrap().events().is_empty());
    }

    #[test]
    fn direct_adapter_prints_directly_or_captures_nested_prefix_before_failure() {
        let (direct, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "module(name='subject')\nprint('direct')",
            &[],
            false,
        );
        direct.unwrap();
        assert!(batch.is_none());

        let included = [DirectNonregistryIncludeFile {
            raw_label: "//:child.MODULE.bazel",
            logical_id: LogicalModuleFileId::new("@@subject+//:child.MODULE.bazel"),
            source: b"print('child')\nfail('boom')",
        }];
        let (error, batch) = evaluate_direct(
            NonrootModuleKey::new("subject", ""),
            "module(name='subject')\nprint('root')\ninclude('//:child.MODULE.bazel')",
            &included,
            true,
        );
        assert!(matches!(
            error,
            Err(DirectNonregistryEvaluationError::Execution(message)) if message.contains("boom")
        ));
        assert!(matches!(
            batch.unwrap().events(),
            [
                EvaluationEvent::StarlarkPrint { location: root, text: root_text },
                EvaluationEvent::StarlarkPrint { location: child, text: child_text },
            ] if root_text == "root"
                && child_text == "child"
                && root.to_string().contains(":MODULE.bazel")
                && child.to_string().contains("child.MODULE.bazel")
        ));
    }

    #[test]
    fn snapshots_final_mutation_and_source_identities_after_gc() {
        let evaluated = evaluate(
            r#"
module(name = "subject", version = "1.0", repo_name = "subject_repo")
bazel_dep(name = "dep", version = "2.0", repo_name = "dep_alias")
proxy = use_extension("//:extension.bzl", "extension")
values = ["before"]
proxy.tag(value = values, builtin = print, proxy = proxy, float = 3.14, float_key = {3.14: "ok"})
values.append("after")
use_repo(proxy, "generated", alias = "renamed")
repo = use_repo_rule("//:repo.bzl", "repo_rule")
repo(name = "innate", value = values)
"#,
        )
        .unwrap();
        assert_eq!(
            evaluated.base.dependencies.get("dep_alias").unwrap().name,
            "dep"
        );
        assert_eq!(evaluated.extension_usages.len(), 2);
        let extension = &evaluated.extension_usages[0];
        assert_eq!(extension.proxies[0].proxy_name, "proxy");
        assert_eq!(
            extension.proxies[0].location.file.0,
            "@@subject+//:MODULE.bazel"
        );
        assert_eq!(
            extension.proxies[0]
                .imports
                .local_to_exported
                .get("generated")
                .unwrap(),
            "generated"
        );
        assert_eq!(
            extension.proxies[0]
                .imports
                .local_to_exported
                .get("alias")
                .unwrap(),
            "renamed"
        );
        let attrs = &extension.tags[0].attributes;
        assert!(
            matches!(attrs.get("value"), Some(NonrootAttributeValue::List(values)) if values.len() == 2)
        );
        assert_eq!(
            attrs.get("builtin"),
            Some(&NonrootAttributeValue::BuiltinPrint)
        );
        assert_eq!(
            attrs.get("proxy"),
            Some(&NonrootAttributeValue::ExtensionProxy)
        );
        assert_eq!(attrs.get("float"), Some(&NonrootAttributeValue::Float314));
        assert!(
            matches!(attrs.get("float_key"), Some(NonrootAttributeValue::Dict(values)) if values.contains_key(&NonrootAttributeKey::DeferredFloat314))
        );
        let innate = &evaluated.extension_usages[1].tags[0].attributes;
        assert_eq!(innate.keys().last().unwrap(), "name");
        assert_eq!(evaluated.extension_usages[1].tags[0].tag_class, "repo");
    }

    #[test]
    fn rejects_positions_and_print_invocation() {
        let positions = evaluate(
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\np.tag('bad')",
        )
        .unwrap_err()
        .to_string();
        assert!(positions.contains("positional"));
        let printed = evaluate("module(name='subject', version='1.0')\nprint('no')")
            .unwrap_err()
            .to_string();
        assert!(printed.contains("print() is not permitted"));
        let innate_positions = evaluate(
            "module(name='subject', version='1.0')\nr=use_repo_rule('//:repo.bzl','repo')\nr('bad')",
        )
        .unwrap_err()
        .to_string();
        assert!(innate_positions.contains("positional"));
    }

    #[test]
    fn accepts_only_the_bounded_deferred_attribute_forms() {
        let accepted = evaluate(
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\ncycle=[]\ncycle.append(cycle)\np.tag(cycle=cycle)",
        )
        .unwrap();
        assert_eq!(
            accepted.extension_usages[0].tags[0].attributes.get("cycle"),
            Some(&NonrootAttributeValue::SelfList)
        );
        for source in [
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\np.tag(value=3.15)",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\np.tag(value=len)",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\np.tag(value={3.15: 'bad'})",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\nr=use_repo_rule('//:repo.bzl','repo')\np.tag(value=r)",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }
    }

    #[test]
    fn dev_proxy_reserves_import_but_is_discarded() {
        let evaluated = evaluate(
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x', dev_dependency=True)\nuse_repo(p, 'reserved')",
        )
        .unwrap();
        assert!(evaluated.extension_usages.is_empty());
        assert!(evaluate("module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x', dev_dependency=True)\nuse_repo(p, 'reserved')\nq=use_extension('//:y.bzl','y')\nuse_repo(q, 'reserved')").is_err());
    }

    #[test]
    fn all_nonroot_override_forms_validate_then_discard() {
        let evaluated = evaluate(
            r#"
module(name = "subject", version = "1.0")
local_path_override(module_name = "local", path = "../local")
single_version_override(module_name = "single", version = "1.2.3", registry = "https://registry", patches = ["//:p.patch"], patch_cmds = ["true"])
multiple_version_override(module_name = "multiple", versions = ["1.0", "2.0"], registry = "https://registry")
archive_override(module_name = "archive", urls = ["https://example.invalid/a.tgz"], integrity = "sha256-x")
git_override(module_name = "git", remote = "https://example.invalid/r.git", commit = "deadbeef")
"#,
        )
        .unwrap();
        assert!(evaluated.extension_usages.is_empty());
        assert!(evaluated.base.dependencies.contains_key("bazel_tools"));
    }

    #[test]
    fn produces_the_complete_ordered_compact_result() {
        let evaluated = evaluate(
            r#"
module(name = "subject", version = "1.0+module-build", repo_name = "subject_self", bazel_compatibility = [">=9.0.0", "-9.1.0"])
bazel_dep(name = "ordinary", version = "2.0+dep-build", repo_name = "ordinary_alias")
bazel_dep(name = "nodep", version = "3.0", repo_name = None)
bazel_dep(name = "dev_only", version = "4.0", repo_name = "dev_reserved", dev_dependency = True)
register_execution_platforms("//:platform_b", "//:platform_a")
register_execution_platforms("ignored-relative", dev_dependency = True)
register_toolchains("@tools//:toolchain_b", "//:toolchain_a")
register_toolchains("ignored-relative", dev_dependency = True)
flag_alias(name = "mode", starlark_flag = "//:mode")
first = use_extension("//:extension.bzl", "extension")
first_alias = first
first.alpha(order = 1)
second = use_extension("@external//pkg:extension.bzl", "other")
second.beta(order = 2)
use_repo(first, local = "{name}-{version}")
isolated = use_extension("//:extension.bzl", "extension", isolate = True)
isolated_alias = isolated
isolated.gamma(order = 3)
use_repo(isolated, "isolated_repo")
dev = use_extension("//:dev.bzl", "dev", dev_dependency = True)
dev.ignored(value = len)
use_repo(dev, "dev_import")
override_repo(first, 1, bad = len)
inject_repo(first, 2, bad = print)
repo_rule = use_repo_rule("//:repo.bzl", "make_repo")
repo_rule(name = "innate_one", order = 4)
repo_rule(name = "innate_two", order = 5)
repo_rule(name = "ignored_innate", dev_dependency = True, value = len)
"#,
        )
        .unwrap();

        assert_eq!(evaluated.base.repo_name, "subject_self");
        assert_eq!(evaluated.base.declared_version, "1.0");
        assert_eq!(
            evaluated.base.bazel_compatibility.as_ref(),
            [
                CompactString::from(">=9.0.0"),
                CompactString::from("-9.1.0")
            ]
        );
        assert_eq!(
            evaluated
                .base
                .dependencies
                .keys()
                .map(CompactString::as_str)
                .collect::<Vec<_>>(),
            ["ordinary_alias", "bazel_tools"]
        );
        assert_eq!(
            evaluated
                .base
                .dependencies
                .get("ordinary_alias")
                .unwrap()
                .version,
            "2.0"
        );
        assert_eq!(evaluated.base.nodep_dependencies.len(), 1);
        assert_eq!(evaluated.base.nodep_dependencies[0].name, "nodep");
        assert_eq!(
            evaluated.base.execution_platforms.as_ref(),
            [
                CompactString::from("//:platform_b"),
                CompactString::from("//:platform_a")
            ]
        );
        assert_eq!(
            evaluated.base.toolchains.as_ref(),
            [
                CompactString::from("@tools//:toolchain_b"),
                CompactString::from("//:toolchain_a")
            ]
        );
        assert_eq!(
            evaluated.base.flag_aliases.get("mode").unwrap(),
            "@subject_self//:mode"
        );
        assert_eq!(
            evaluated.base.original_dependencies,
            evaluated.base.dependencies
        );

        assert_eq!(evaluated.extension_usages.len(), 4);
        let ordinary = &evaluated.extension_usages[0];
        assert_eq!(ordinary.bzl_label, "@subject_self//:extension.bzl");
        assert_eq!(ordinary.extension_name, "extension");
        assert_eq!(ordinary.proxies.len(), 1);
        assert_eq!(ordinary.proxies[0].proxy_name, "first");
        assert_eq!(
            ordinary.proxies[0]
                .imports
                .local_to_exported
                .get("local")
                .unwrap(),
            "subject-1.0"
        );
        assert_eq!(ordinary.tags[0].tag_class, "alpha");
        assert_eq!(
            ordinary.tags[0].attributes.get("order"),
            Some(&NonrootAttributeValue::integer("1").unwrap())
        );
        assert!(ordinary.tags[0].location.start_line > 0);

        let external = &evaluated.extension_usages[1];
        assert_eq!(external.bzl_label, "@external//pkg:extension.bzl");
        assert_eq!(external.tags[0].tag_class, "beta");

        let isolated = &evaluated.extension_usages[2];
        assert_eq!(
            isolated.isolation,
            Some(NonrootExtensionIsolationKey {
                module: NonrootModuleKey::new("subject", "1.0"),
                exported_proxy_name: "isolated".into(),
            })
        );
        assert_eq!(isolated.tags[0].tag_class, "gamma");

        let innate = &evaluated.extension_usages[3];
        assert_eq!(innate.bzl_label, "//:MODULE.bazel");
        assert_eq!(innate.extension_name, "//:repo.bzl make_repo");
        assert_eq!(innate.proxies.len(), 2);
        assert!(
            innate
                .proxies
                .iter()
                .all(|proxy| proxy.proxy_name.is_empty())
        );
        assert_eq!(innate.tags.len(), 2);
        assert!(innate.tags.iter().all(|tag| tag.tag_class == "repo"));
        assert_eq!(innate.tags[0].attributes.keys().last().unwrap(), "name");
        assert_eq!(innate.tags[1].attributes.keys().last().unwrap(), "name");
    }

    #[test]
    fn enforces_repo_name_and_export_collision_boundaries() {
        for source in [
            "module(name='subject', version='1.0')\nbazel_dep(name='dep', version='1.0', repo_name='subject')",
            "module(name='subject', version='1.0')\nbazel_dep(name='dev', version='1.0', repo_name='reserved', dev_dependency=True)\np=use_extension('//:x.bzl','x')\nuse_repo(p, 'reserved')",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\nuse_repo(p, first='same', second='same')",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x', dev_dependency=True)\nuse_repo(p, 'bazel_tools')",
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\nuse_repo(p, '')",
            "module(name='subject', version='1.0')\nr=use_repo_rule('//:repo.bzl','repo')\nr(name='')",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }

        let distinct_usages = evaluate(
            "module(name='subject', version='1.0')\na=use_extension('//:a.bzl','a')\nb=use_extension('//:b.bzl','b')\nuse_repo(a, first='same')\nuse_repo(b, second='same')",
        )
        .unwrap();
        assert_eq!(distinct_usages.extension_usages.len(), 2);
    }

    #[test]
    fn validates_directive_order_labels_and_dynamic_call_shapes() {
        for source in [
            "bazel_dep(name='dep', version='1.0')\nmodule(name='subject', version='1.0')",
            "module(name='subject', version='1.0')\nmodule(name='subject', version='1.0')",
            "module(name='subject', version='1.0')\nregister_toolchains('relative')",
            "module(name='subject', version='1.0')\nregister_execution_platforms('relative')",
            "module(name='subject', version='1.0')\nuse_extension('//:x.bzl', 'not-valid')",
            "module(name='subject', version='1.0')\nsingle_version_override(module_name='dep', patches=['@other//:patch'])",
        ] {
            assert!(evaluate(source).is_err(), "{source}");
        }

        let ignored = evaluate(
            "module(name='subject', version='1.0')\np=use_extension('//:x.bzl','x')\noverride_repo(p, 1, alias=len)\ninject_repo(p, 2, alias=print)\narchive_override(module_name='archive', arbitrary=len)\ngit_override(module_name='git', arbitrary=print)",
        )
        .unwrap();
        assert_eq!(ignored.extension_usages.len(), 1);
        assert!(ignored.extension_usages[0].repo_overrides.is_empty());
    }

    #[test]
    fn records_exact_proxy_tag_and_innate_call_spans() {
        let evaluated = evaluate(
            "module(name='subject', version='1.0')\nproxy = use_extension('//:x.bzl', 'x')\nproxy.tag(value = 1)\nrepo = use_repo_rule('//:repo.bzl', 'repo')\nrepo(name = 'generated')",
        )
        .unwrap();
        let expected = |line, start_column, end_column| LogicalSpan {
            file: LogicalModuleFileId::new("@@subject+//:MODULE.bazel"),
            start_line: line,
            start_column,
            end_line: line,
            end_column,
        };
        assert_eq!(
            evaluated.extension_usages[0].proxies[0].location,
            expected(2, 22, 22)
        );
        assert_eq!(
            evaluated.extension_usages[0].tags[0].location,
            expected(3, 10, 10)
        );
        assert_eq!(
            evaluated.extension_usages[1].proxies[0].location,
            expected(5, 5, 5)
        );
        assert_eq!(
            evaluated.extension_usages[1].tags[0].location,
            expected(5, 5, 5)
        );
    }

    #[test]
    fn composes_nested_and_repeated_includes_with_per_file_gc_roots() {
        let supplied = [
            SuppliedNonrootModuleFile {
                raw_label: "//:outer.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:outer.MODULE.bazel"),
                source: br#"
proxy = use_extension("//:extension.bzl", "extension")
proxy.marker(marker = "outer-before")
include("//:nested.MODULE.bazel")
proxy.marker(marker = "outer-after")
"#,
            },
            SuppliedNonrootModuleFile {
                raw_label: "//:nested.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:nested.MODULE.bazel"),
                source: br#"
proxy = use_extension("//:extension.bzl", "extension")
proxy.marker(marker = "nested-a")
"#,
            },
            SuppliedNonrootModuleFile {
                raw_label: "//:repeat.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:repeat.MODULE.bazel"),
                source: br#"
proxy = use_extension("//:extension.bzl", "extension")
proxy.marker(marker = "repeat-a")
"#,
            },
        ];
        let evaluated = evaluate_with_includes(
            r#"
module(name = "subject", version = "1.0")
include("//:outer.MODULE.bazel")
include("//:repeat.MODULE.bazel")
include("//:repeat.MODULE.bazel")
"#,
            &supplied,
        )
        .unwrap();

        let usage = evaluated
            .extension_usages
            .iter()
            .find(|usage| usage.extension_name == "extension")
            .unwrap();
        let markers: Vec<_> = usage
            .tags
            .iter()
            .map(|tag| match tag.attributes.get("marker") {
                Some(NonrootAttributeValue::String(marker)) => marker.as_str(),
                marker => panic!("unexpected marker: {marker:?}"),
            })
            .collect();
        assert_eq!(
            markers,
            [
                "outer-before",
                "nested-a",
                "outer-after",
                "repeat-a",
                "repeat-a"
            ]
        );
        assert_eq!(
            usage.tags[0].location.file.0,
            "@@subject+//:outer.MODULE.bazel"
        );
        assert_eq!(
            usage.tags[1].location.file.0,
            "@@subject+//:nested.MODULE.bazel"
        );
        assert_eq!(
            usage.tags[3].location.file.0,
            "@@subject+//:repeat.MODULE.bazel"
        );
    }

    #[test]
    fn compiles_the_supplied_closure_before_root_effects() {
        let supplied = [SuppliedNonrootModuleFile {
            raw_label: "//:late.MODULE.bazel",
            logical_id: LogicalModuleFileId::new("@@subject+//:late.MODULE.bazel"),
            source: b"value = undefined_late_symbol",
        }];
        let error = evaluate_with_includes(
            r#"
module(name = "subject", version = "1.0")
bazel_dep(name = "dep", version = "not a version")
include("//:late.MODULE.bazel")
"#,
            &supplied,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("undefined_late_symbol"), "{error}");
        assert!(!error.contains("not a version"), "{error}");
    }

    #[test]
    fn rejects_a_missing_nested_file_before_any_directive_effects() {
        let supplied = [SuppliedNonrootModuleFile {
            raw_label: "//:outer.MODULE.bazel",
            logical_id: LogicalModuleFileId::new("@@subject+//:outer.MODULE.bazel"),
            source: b"bazel_dep(name='dep', version='not a version')\ninclude('//:missing.MODULE.bazel')",
        }];
        let error = evaluate_with_includes(
            "module(name='subject', version='1.0')\nbazel_dep(name='earlier', version='also not a version')\ninclude('//:outer.MODULE.bazel')",
            &supplied,
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains(
                "@@subject+//:outer.MODULE.bazel:2:1: include() has no supplied non-registry file"
            ),
            "{error}"
        );
        assert!(error.contains("//:missing.MODULE.bazel"), "{error}");
        assert!(!error.contains("also not a version"), "{error}");
        assert!(!error.contains("not a version"), "{error}");
    }

    #[test]
    fn reports_nested_include_sites_and_typed_label_failures() {
        let supplied = [
            SuppliedNonrootModuleFile {
                raw_label: "//:outer.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:outer.MODULE.bazel"),
                source: b"include(\"//:nested.MODULE.bazel\")",
            },
            SuppliedNonrootModuleFile {
                raw_label: "//:nested.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:nested.MODULE.bazel"),
                source: b"fail(\"nested sentinel\")",
            },
        ];
        let nested = evaluate_with_includes(
            "module(name='subject', version='1.0')\ninclude('//:outer.MODULE.bazel')",
            &supplied,
        )
        .unwrap_err()
        .to_string();
        assert!(nested.contains("nested sentinel"), "{nested}");
        let root_frame = nested
            .find("@@subject+//:MODULE.bazel:2")
            .unwrap_or_else(|| panic!("{nested}"));
        let outer_frame = nested
            .find("@@subject+//:outer.MODULE.bazel:1")
            .unwrap_or_else(|| panic!("{nested}"));
        let nested_frame = nested
            .find("@@subject+//:nested.MODULE.bazel:1")
            .unwrap_or_else(|| panic!("{nested}"));
        assert!(root_frame < outer_frame, "{nested}");
        assert!(outer_frame < nested_frame, "{nested}");
        assert_eq!(nested.matches("in <module>").count(), 1, "{nested}");
        assert_eq!(nested.matches("in include").count(), 2, "{nested}");

        let bad =
            evaluate("module(name='subject', version='1.0')\ninclude('relative.MODULE.bazel')")
                .unwrap_err()
                .to_string();
        assert!(bad.contains("bad include label"), "{bad}");

        let missing =
            evaluate("module(name='subject', version='1.0')\ninclude('//:missing.MODULE.bazel')")
                .unwrap_err()
                .to_string();
        assert!(
            missing.contains("no supplied non-registry file"),
            "{missing}"
        );

        let duplicate = [
            SuppliedNonrootModuleFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:first.MODULE.bazel"),
                source: b"value = 1",
            },
            SuppliedNonrootModuleFile {
                raw_label: "//:same.MODULE.bazel",
                logical_id: LogicalModuleFileId::new("@@subject+//:second.MODULE.bazel"),
                source: b"value = 2",
            },
        ];
        let duplicate = evaluate_with_includes("module(name='subject', version='1.0')", &duplicate)
            .unwrap_err()
            .to_string();
        assert!(
            duplicate.contains("duplicate supplied include label"),
            "{duplicate}"
        );

        let unreachable = [SuppliedNonrootModuleFile {
            raw_label: "//:unused.MODULE.bazel",
            logical_id: LogicalModuleFileId::new("@@subject+//:unused.MODULE.bazel"),
            source: b"value = 1",
        }];
        let unreachable =
            evaluate_with_includes("module(name='subject', version='1.0')", &unreachable)
                .unwrap_err()
                .to_string();
        assert!(
            unreachable
                .contains("supplied include label `//:unused.MODULE.bazel` is not reachable"),
            "{unreachable}"
        );
    }
}

fn root_mapping(module: &EvaluatedRootModule) -> Result<RepositoryMapping, CompactString> {
    let mut mapping = RepositoryMapping::new(
        RepositoryMappingId::new("root-module").map_err(CompactString::new)?,
    );
    for dep in module.dependencies.iter().filter(|dep| !dep.nodep) {
        let apparent = dep.repo_name.as_deref().unwrap_or(dep.name.as_str());
        mapping.insert(
            ApparentRepoName::new(apparent).map_err(CompactString::new)?,
            CanonicalRepoName::new(format!("{}+", dep.name)).map_err(CompactString::new)?,
        );
    }
    Ok(mapping)
}

#[derive(ProvidesStaticType)]
struct RootEvaluationContext {
    state: RefCell<RecordedRootModule>,
    include_indices: SmallMap<CompactString, usize>,
}

struct RecordedRootModule {
    header: Option<RootModuleHeader>,
    non_module_called: bool,
    dependencies: Vec<RootModuleDependency>,
    execution_platforms: Vec<ApparentLabel>,
    toolchains: Vec<ApparentLabel>,
    overrides: SmallMap<CompactString, RecordedRootModuleOverride>,
    ignore_dev_dependency: bool,
    current_file: usize,
}

fn record_override(
    state: &mut RecordedRootModule,
    module_name: &str,
    override_: RecordedRootModuleOverride,
) -> anyhow::Result<()> {
    validate_module_name(module_name)?;
    state.non_module_called = true;
    if state.ignore_dev_dependency {
        return Ok(());
    }
    if state
        .overrides
        .insert(module_name.into(), override_)
        .is_some()
    {
        anyhow::bail!("multiple overrides for module {module_name}");
    }
    Ok(())
}

fn root_evaluation_context<'a>(
    eval: &'a Evaluator<'_, '_, '_>,
) -> anyhow::Result<&'a RootEvaluationContext> {
    eval.extra
        .and_then(|value| value.downcast_ref())
        .context("MODULE.bazel global invoked without root evaluation context")
}

fn direct_registration_labels<'v>(labels: Value<'v>) -> anyhow::Result<Vec<ApparentLabel>> {
    let labels = TupleRef::from_value(labels).context("registration expects labels")?;
    labels
        .iter()
        .map(|label| {
            let label = label
                .unpack_str()
                .context("registration labels must be strings")?;
            let target = label.rsplit_once(':').map(|(_, target)| target);
            let recursive = target.is_none() && label.ends_with("/...");
            if recursive || matches!(target, Some("all" | "all-targets" | "*")) {
                anyhow::bail!("registration labels must name direct targets")
            }
            ApparentLabel::parse(label).map_err(anyhow::Error::msg)
        })
        .collect()
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
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
        let extra = eval.extra;
        let context = extra
            .and_then(|value| value.downcast_ref::<RootEvaluationContext>())
            .context("MODULE.bazel include invoked without root evaluation context")?;
        let index = *context
            .include_indices
            .get(label)
            .with_context(|| format!("included MODULE file was not prepared: {label}"))?;
        let previous_file = {
            let mut state = context.state.borrow_mut();
            state.non_module_called = true;
            let previous_file = state.current_file;
            state.current_file = index;
            previous_file
        };
        let result = eval.eval_prepared_module_index(index);
        context.state.borrow_mut().current_file = previous_file;
        result.map_err(starlark::Error::into_anyhow)?;
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !dev_dependency || !state.ignore_dev_dependency {
            state.dependencies.push(RootModuleDependency {
                name: name.into(),
                version: version.into(),
                repo_name,
                nodep,
                dev_dependency,
            });
        }
        Ok(NoneType)
    }
    fn register_execution_platforms<'v>(
        #[starlark(args)] labels: Value<'v>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let labels = direct_registration_labels(labels)?;
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !dev_dependency || !state.ignore_dev_dependency {
            state.execution_platforms.extend(labels);
        }
        Ok(NoneType)
    }
    fn register_toolchains<'v>(
        #[starlark(args)] labels: Value<'v>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> anyhow::Result<NoneType> {
        let labels = direct_registration_labels(labels)?;
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        state.non_module_called = true;
        if !dev_dependency || !state.ignore_dev_dependency {
            state.toolchains.extend(labels);
        }
        Ok(NoneType)
    }
    fn local_path_override(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] path: &str,
        eval: &mut Evaluator,
    ) -> anyhow::Result<NoneType> {
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        for patch in &patches.items {
            let _ = normalize_patch_label(patch, state.header.as_ref())?;
        }
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        for patch in patches_to_validate.iter() {
            let _ = normalize_patch_label(patch.as_str(), state.header.as_ref())?;
        }
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
        let mut state = root_evaluation_context(eval)?.state.borrow_mut();
        for patch in patches_to_validate.iter() {
            let _ = normalize_patch_label(patch.as_str(), state.header.as_ref())?;
        }
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

fn root_module_globals() -> Globals {
    GlobalsBuilder::extended_by(&[LibraryExtension::Print])
        .with(module_globals)
        .build()
}

#[cfg(test)]
std::thread_local! {
    static VALIDATED_ROOT_MODULE_LOGICAL_IDS: RefCell<Vec<LogicalModuleFileId>> =
        const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn clear_validated_root_module_logical_ids() {
    VALIDATED_ROOT_MODULE_LOGICAL_IDS.with(|logical_ids| logical_ids.borrow_mut().clear());
}

#[cfg(test)]
pub(crate) fn take_validated_root_module_logical_ids() -> Vec<LogicalModuleFileId> {
    VALIDATED_ROOT_MODULE_LOGICAL_IDS
        .with(|logical_ids| std::mem::take(&mut *logical_ids.borrow_mut()))
}

/// Parse, inspect, scope-check, and compile one root MODULE file without
/// retaining any Starlark state. Host discovery uses this before crossing the
/// next DICE await so preparation failures cannot request a later horizon.
pub(crate) fn validate_root_module_source(
    logical_id: LogicalModuleFileId,
    source: &[u8],
) -> Result<NonrootModuleFileInspection, CompactString> {
    #[cfg(test)]
    VALIDATED_ROOT_MODULE_LOGICAL_IDS
        .with(|logical_ids| logical_ids.borrow_mut().push(logical_id.clone()));

    let (ast, inspection) = parse_and_inspect_nonroot_module_file(logical_id, source)
        .map_err(|error| CompactString::new(error.to_string()))?;
    let module = Module::new();
    let globals = root_module_globals();
    let mut evaluator = Evaluator::new(&module);
    evaluator
        .prepare_module(ast, &globals)
        .map_err(|error| CompactString::new(error.to_string()))?;
    Ok(inspection)
}

pub(crate) fn evaluate_root_module_closure_with_events(
    ignore_dev_dependency: bool,
    files: Vec<RootModuleSourceFile>,
    include_indices: SmallMap<CompactString, usize>,
    module_file_paths: Arc<[PathBuf]>,
    capture_events: bool,
) -> (
    Result<RootModuleEvaluation, CompactString>,
    Option<EventBatch>,
) {
    let print_capture = capture_events.then(RootModulePrintCapture::default);
    let value = evaluate_root_module_closure(
        ignore_dev_dependency,
        files,
        include_indices,
        module_file_paths,
        print_capture.as_ref(),
    );
    (value, print_capture.map(RootModulePrintCapture::into_batch))
}

fn evaluate_root_module_closure(
    ignore_dev_dependency: bool,
    files: Vec<RootModuleSourceFile>,
    include_indices: SmallMap<CompactString, usize>,
    module_file_paths: Arc<[PathBuf]>,
    print_capture: Option<&RootModulePrintCapture>,
) -> Result<RootModuleEvaluation, CompactString> {
    let asts = files
        .iter()
        .map(|file| {
            AstModule::parse(
                &file.path.display().to_string(),
                file.source.as_str().to_owned(),
                &nonroot_module_dialect(),
            )
            .map_err(|error| CompactString::new(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let module = Module::new();
    let included_modules = files
        .iter()
        .skip(1)
        .map(|_| Box::new(Module::new()))
        .collect::<Vec<_>>();
    let globals = root_module_globals();
    let programs = {
        let mut asts = asts.into_iter();
        let root_ast = asts
            .next()
            .ok_or_else(|| CompactString::new("root MODULE file is absent"))?;
        let mut evaluator = Evaluator::new(&module);
        let mut programs = Vec::with_capacity(files.len());
        programs.push(
            evaluator
                .prepare_module(root_ast, &globals)
                .map_err(|error| CompactString::new(error.to_string()))?,
        );
        for (included_module, ast) in included_modules.iter().zip(asts) {
            programs.push(
                evaluator
                    .prepare_module_in(included_module.as_ref(), ast, &globals)
                    .map_err(|error| CompactString::new(error.to_string()))?,
            );
        }
        programs
    };
    let context = RootEvaluationContext {
        state: RefCell::new(RecordedRootModule {
            header: None,
            non_module_called: false,
            dependencies: Vec::new(),
            execution_platforms: Vec::new(),
            toolchains: Vec::new(),
            overrides: SmallMap::new(),
            ignore_dev_dependency,
            current_file: 0,
        }),
        include_indices,
    };
    {
        let mut evaluator = Evaluator::new(&module);
        evaluator.extra = Some(&context);
        if let Some(print_capture) = print_capture {
            evaluator.set_print_handler(print_capture);
        }
        evaluator
            .set_prepared_modules(programs)
            .map_err(|error| CompactString::new(error.to_string()))?;
        evaluator
            .eval_prepared_module_index(0)
            .map_err(|error| CompactString::new(error.to_string()))?;
    }
    let state = context.state.into_inner();
    let mut overrides = SmallMap::new();
    for (module_name, override_) in state.overrides {
        let override_ = materialize_override(&override_, state.header.as_ref())
            .map_err(|error| CompactString::new(error.to_string()))?;
        overrides.insert(module_name, override_);
    }
    Ok(RootModuleEvaluation {
        module: EvaluatedRootModule {
            header: state.header,
            dependencies: state.dependencies.into(),
            registrations: RootModuleRegistrations {
                execution_platforms: state.execution_platforms.into(),
                toolchains: state.toolchains.into(),
            },
        },
        module_file_paths,
        overrides: RootModuleOverrides(Arc::new(overrides)),
    })
}
