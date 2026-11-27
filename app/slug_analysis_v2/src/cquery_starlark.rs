//! Request-owned formatting over accepted Analysis values.

use std::cell::RefCell;
use std::fmt::Write;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use slug_build_api_v2::AnalysisDepset;
use slug_loading_v2::provider::prepare_bounded_depset_methods;
use starlark::PrintHandler;
use starlark::PrintLocation;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::LibraryExtension;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::eval::inspect_def;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::ParseLimit;
use starlark::syntax::ParseLimits;
use starlark::values::BoundedText;
use starlark::values::Heap;
use starlark::values::OwnedFrozenValue;
use starlark::values::Value;
use starlark::values::ValueTextMode;
use starlark::values::dict::AllocDict;

use crate::ConfiguredNodeResult;
use crate::analysis_value::AnalysisConfiguredTargetValue;
use crate::analysis_value::AnalysisValueMaterializer;

pub const CQUERY_STARLARK_SOURCE_MAX_BYTES: usize = 64 * 1024;
const HEAP_BYTES: usize = 64 * 1024 * 1024;
const STREAM_BYTES: usize = 4 * 1024 * 1024;
const WORK_UNITS: usize = 16 * 1024 * 1024;
const WORK_TIME: Duration = Duration::from_secs(30);
const STACK_DEPTH: usize = 50;
const MAX_ROWS: usize = 65_536;
const PARSE_LIMITS: ParseLimits = ParseLimits::new(CQUERY_STARLARK_SOURCE_MAX_BYTES, 256, 128);

#[derive(Debug)]
pub enum CqueryStarlarkError {
    Setup { stderr: String },
    Request(anyhow::Error),
}

#[starlark::starlark_module]
fn query_globals(builder: &mut GlobalsBuilder) {
    #[starlark(bounded_function)]
    fn providers<'v>(target: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let budget = heap.execution_budget();
        if let Some(budget) = budget {
            budget.charge(1, 0)?;
        }
        let target = AnalysisConfiguredTargetValue::from_value(target).ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!("providers() requires a configured target"))
        })?;
        let entries = target.providers();
        let count = entries.len();
        if count == 0 {
            return Ok(Value::new_none());
        }
        if let Some(budget) = budget {
            return heap.alloc_budgeted_str_dict_from_rows(
                count,
                entries.map(|(identity, value)| {
                    let key = match identity.user_id() {
                        Some(id) => heap.alloc_budgeted_str_parts(&id.display_parts())?,
                        None => heap.alloc_budgeted_str(identity.name())?,
                    };
                    Ok((key, value.to_value()))
                }),
                budget,
            );
        }
        Ok(heap.alloc(AllocDict(entries.map(|(identity, value)| {
            let key = match identity.user_id() {
                Some(id) => heap.alloc_str(&id.to_string()),
                None => heap.alloc_str(identity.name()),
            };
            (key, value.to_value())
        }))))
    }
}

struct DiagnosticPrint<'a, 'budget>(&'a RefCell<BoundedText<'budget>>);

impl PrintHandler for DiagnosticPrint<'_, '_> {
    fn println(&self, _: PrintLocation, text: &str) -> starlark::Result<()> {
        let mut stream = self.0.borrow_mut();
        stream.push_str(text)?;
        stream.push_str("\n")?;
        Ok(())
    }
}

fn observed_limit(module: &Module, start: Instant) -> Option<ExecutionLimit> {
    if start.elapsed() >= WORK_TIME {
        return Some(ExecutionLimit::Work);
    }
    if module
        .heap()
        .allocated_bytes()
        .saturating_add(module.frozen_heap().allocated_bytes())
        > HEAP_BYTES
    {
        return Some(ExecutionLimit::AllocationBytes);
    }
    None
}

fn check_limits(module: &Module, start: Instant, budget: &ExecutionBudget) -> anyhow::Result<()> {
    budget.check()?;
    if let Some(limit) = observed_limit(module, start) {
        return Err(budget.refuse(limit).into());
    }
    Ok(())
}

fn check_refusal(error: &starlark::Error, budget: &ExecutionBudget) -> Result<(), ExecutionLimit> {
    budget.check()?;
    if let starlark::ErrorKind::Other(cause)
    | starlark::ErrorKind::Native(cause)
    | starlark::ErrorKind::Internal(cause) = error.kind()
        && let Some(limit) = cause.downcast_ref::<ExecutionLimit>()
    {
        return Err(budget.refuse(*limit));
    }
    Ok(())
}

fn bounded_message(budget: &ExecutionBudget, message: impl std::fmt::Display) -> anyhow::Error {
    let mut text = BoundedText::new(budget, STREAM_BYTES);
    if write!(text, "{message}").is_err() {
        return budget.refuse(ExecutionLimit::TextFormatting).into();
    }
    match text.finish() {
        Ok(text) => anyhow::Error::msg(text),
        Err(limit) => limit.into(),
    }
}

fn bounded_formatter_error(error: starlark::Error, budget: &ExecutionBudget) -> anyhow::Error {
    if let Err(limit) = check_refusal(&error, budget) {
        return limit.into();
    }
    if let starlark::ErrorKind::Other(cause) = error.kind()
        && let Some(limit) = cause.downcast_ref::<ParseLimit>()
    {
        return (*limit).into();
    }
    // Render the payload directly. Starlark's full diagnostic display builds
    // a separate annotated source snippet before writing to the sink.
    bounded_message(budget, error.without_diagnostic())
}

fn compile(
    name: &str,
    source: &str,
    budget: &Arc<ExecutionBudget>,
    printer: &DiagnosticPrint<'_, '_>,
    start: Instant,
) -> anyhow::Result<OwnedFrozenValue> {
    if source.len() > CQUERY_STARLARK_SOURCE_MAX_BYTES {
        return Err(ParseLimit::SourceBytes.into());
    }
    let mut input = BoundedText::new(budget, CQUERY_STARLARK_SOURCE_MAX_BYTES);
    input.push_str(source)?;
    let ast = AstModule::parse_with_limits(name, input.finish()?, &Dialect::Standard, PARSE_LIMITS)
        .map_err(|error| bounded_formatter_error(error, budget))?;
    let module = Module::new_with_execution_budget(budget.clone());
    check_limits(&module, start, budget)?;
    {
        static GLOBALS: LazyLock<Globals> = LazyLock::new(|| {
            AnalysisDepset::prepare_canonical_empties();
            GlobalsBuilder::extended_by(&[LibraryExtension::Print])
                .with(query_globals)
                .build()
        });
        let mut eval = Evaluator::new(&module);
        eval.set_print_handler(printer);
        eval.set_max_callstack_size(STACK_DEPTH)?;
        eval.set_check_cancelled(Box::new(|| observed_limit(&module, start).is_some()));
        let result = eval.eval_module(ast, &GLOBALS);
        check_limits(&module, start, budget)?;
        result.map_err(|error| bounded_formatter_error(error, budget))?;
    }
    let frozen = module
        .freeze_bounded()
        .map_err(|error| bounded_formatter_error(error, budget))?;
    if start.elapsed() >= WORK_TIME {
        return Err(budget.refuse(ExecutionLimit::Work).into());
    }
    let function = frozen
        .get_option("format")?
        .ok_or_else(|| anyhow::anyhow!("file does not define 'format'"))?;
    let definition = inspect_def(function.value(), None)
        .ok_or_else(|| anyhow::anyhow!("'format' must be a Starlark function"))?;
    if definition.parameter_count() != 1 {
        anyhow::bail!("'format' function must take exactly 1 argument");
    }
    Ok(function)
}

fn append_result(stream: &mut BoundedText<'_>, value: Value<'_>) -> Result<(), ExecutionLimit> {
    stream.write_value(value, ValueTextMode::Str)?;
    stream.push_str("\n")
}

/// Compile once and render the accepted targets in their existing order.
pub fn render_cquery_starlark<'a>(
    name: &str,
    source: &str,
    targets: impl Iterator<Item = (&'a ConfiguredNodeResult, &'a str)>,
) -> Result<(String, String), CqueryStarlarkError> {
    let start = Instant::now();
    prepare_bounded_depset_methods();
    let budget = Arc::new(
        ExecutionBudget::new(WORK_UNITS, HEAP_BYTES, STACK_DEPTH).with_closed_formatter_bytecode(),
    );
    render_with_budget(name, source, targets, &budget, start, STREAM_BYTES)
}

fn render_with_budget<'a>(
    name: &str,
    source: &str,
    targets: impl Iterator<Item = (&'a ConfiguredNodeResult, &'a str)>,
    budget: &Arc<ExecutionBudget>,
    start: Instant,
    stream_bytes: usize,
) -> Result<(String, String), CqueryStarlarkError> {
    let stderr = RefCell::new(BoundedText::new(budget, stream_bytes));
    let printer = DiagnosticPrint(&stderr);
    let function = match compile(name, source, budget, &printer, start) {
        Ok(function) => function,
        Err(error) if error.is::<ExecutionLimit>() || error.is::<ParseLimit>() => {
            return Err(CqueryStarlarkError::Request(error));
        }
        Err(error) => {
            let mut stderr = stderr.into_inner();
            writeln!(stderr, "ERROR: invalid --starlark:file: {error}").map_err(|_| {
                CqueryStarlarkError::Request(budget.refuse(ExecutionLimit::TextFormatting).into())
            })?;
            let stderr = stderr
                .finish()
                .map_err(|limit| CqueryStarlarkError::Request(limit.into()))?;
            return Err(CqueryStarlarkError::Setup { stderr });
        }
    };
    let stdout = (|| -> anyhow::Result<_> {
        let mut stdout = BoundedText::new(budget, stream_bytes);
        for (index, (target, label)) in targets.enumerate() {
            if index >= MAX_ROWS {
                anyhow::bail!("cquery formatter row limit exceeded");
            }
            budget.charge(1, 0)?;
            let module = Module::new_with_execution_budget(budget.clone());
            check_limits(&module, start, budget)?;
            let row =
                AnalysisValueMaterializer::new_with_budget(module.frozen_heap(), Some(budget))
                    .budgeted_cquery_target(target.key(), target.providers())
                    .map_err(|error| bounded_formatter_error(error, budget))?
                    .to_value();
            let function = function.owned_value_budgeted(module.frozen_heap(), budget)?;
            check_limits(&module, start, budget)?;
            let mut eval = Evaluator::new(&module);
            eval.set_print_handler(&printer);
            eval.set_max_callstack_size(STACK_DEPTH)?;
            eval.set_check_cancelled(Box::new(|| observed_limit(&module, start).is_some()));
            let result = eval.eval_function(function, &[row], &[]);
            check_limits(&module, start, budget)?;
            match result {
                Ok(value) => append_result(&mut stdout, value)?,
                Err(error) => {
                    check_refusal(&error, budget)?;
                    writeln!(
                        stderr.borrow_mut(),
                        "ERROR: Starlark evaluation error for {label}: {}",
                        error.without_diagnostic()
                    )
                    .map_err(|_| budget.refuse(ExecutionLimit::TextFormatting))?;
                }
            }
            check_limits(&module, start, budget)?;
        }
        Ok(stdout.finish()?)
    })()
    .map_err(CqueryStarlarkError::Request)?;
    let stderr = stderr
        .into_inner()
        .finish()
        .map_err(|limit| CqueryStarlarkError::Request(limit.into()))?;
    Ok((stdout, stderr))
}

#[cfg(test)]
mod tests {
    use slug_build_api_v2::AnalysisValue;
    use slug_build_api_v2::DefaultInfo;
    use slug_build_api_v2::ProviderCollection;
    use slug_build_api_v2::ProviderId;
    use slug_build_api_v2::ProviderIdentity;
    use slug_build_api_v2::ProviderOccurrence;
    use slug_build_api_v2::ProviderValue;
    use slug_build_api_v2::RunfilesPackageDepset;
    use slug_identity_v2::CanonicalLabel;
    use starlark::values::dict::DictRef;
    use starlark::values::list::AllocList;
    use starlark::values::tuple::AllocTuple;

    use super::*;
    use crate::ConfiguredNodeKey;
    use crate::ConfiguredNodeKind;

    #[test]
    fn cquery_providers_reuses_native_values_and_displayed_identity_keys() {
        let key = ConfiguredNodeKey::null(CanonicalLabel::parse("@@//pkg:target").unwrap());
        let id = ProviderId::new("@@repo+//rust/private:providers.bzl", "CrateInfo").unwrap();
        let providers = ProviderCollection::new(vec![
            ProviderValue::DefaultInfo(DefaultInfo::empty()),
            ProviderValue::Occurrence(ProviderOccurrence::new(
                ProviderIdentity::user(id.clone()),
                [("name", AnalysisValue::string("target"))],
            )),
        ])
        .unwrap();
        let globals = GlobalsBuilder::new().with(query_globals).build();
        for strict in [false, true] {
            let module = if strict {
                Module::new_with_execution_budget(Arc::new(
                    ExecutionBudget::new(100_000, 1 << 20, 16).with_bounded_operations(),
                ))
            } else {
                Module::new()
            };
            let target = if let Some(budget) = module.heap().execution_budget() {
                AnalysisValueMaterializer::new_with_budget(module.frozen_heap(), Some(budget))
                    .budgeted_cquery_target(&key, &providers)
                    .unwrap()
            } else {
                AnalysisValueMaterializer::new(module.frozen_heap())
                    .configured_dependency_target(&key, providers.clone())
                    .unwrap()
            };
            module.set("target", target.to_value());
            let ast = AstModule::parse(
                "providers.star",
                "providers(target)".to_owned(),
                &Dialect::Standard,
            )
            .unwrap();
            let result = Evaluator::new(&module).eval_module(ast, &globals).unwrap();
            let dict = DictRef::from_value(result).unwrap();
            let target = AnalysisConfiguredTargetValue::from_value(target.to_value()).unwrap();
            assert_eq!(dict.len(), target.providers().len());
            for ((key, value), (identity, native)) in dict.iter().zip(target.providers()) {
                let expected = identity
                    .user_id()
                    .map_or_else(|| identity.name().to_owned(), ToString::to_string);
                assert_eq!(key.unpack_str(), Some(expected.as_str()));
                assert!(value.ptr_eq(native.to_value()));
            }
            assert_eq!(
                dict.get_str(&id.to_string())
                    .unwrap()
                    .get_attr("name", module.heap())
                    .unwrap()
                    .unwrap()
                    .unpack_str(),
                Some("target"),
            );
        }
    }

    #[test]
    fn cquery_formatter_setup_resource_refusal_is_request_failure() {
        let budget = Arc::new(ExecutionBudget::new(100, 0, 16).with_closed_formatter_bytecode());
        let result = render_with_budget(
            "budget.star",
            "def format(target): return str(target)",
            std::iter::empty(),
            &budget,
            Instant::now(),
            STREAM_BYTES,
        );
        match result.unwrap_err() {
            CqueryStarlarkError::Request(error) => {
                assert_eq!(
                    error.downcast_ref::<ExecutionLimit>(),
                    Some(&ExecutionLimit::AllocationBytes)
                );
            }
            error => panic!("unexpected formatter failure: {error:?}"),
        }
    }

    #[test]
    fn cquery_formatter_setup_validates_def_and_loader() {
        assert!(
            render_cquery_starlark(
                "format.star",
                "def format(target): return target\n",
                std::iter::empty(),
            )
            .is_ok()
        );
        for (source, message) in [
            ("", "does not define 'format'"),
            ("format = len\n", "must be a Starlark function"),
            ("def format(x, y): return x\n", "exactly 1 argument"),
            ("load('format.bzl', 'format')\n", "load"),
            ("fail('setup failure')\n", "setup failure"),
        ] {
            let error =
                render_cquery_starlark("format.star", source, std::iter::empty()).unwrap_err();
            let CqueryStarlarkError::Setup { stderr } = error else {
                panic!("expected a setup failure, got {error:?}");
            };
            assert!(stderr.contains(message), "{stderr}");
        }
        let source = "x = 1\n".repeat(80);
        let error = render_cquery_starlark("format.star", &source, std::iter::empty()).unwrap_err();
        assert!(matches!(error, CqueryStarlarkError::Request(error)
            if error.downcast_ref::<ParseLimit>() == Some(&ParseLimit::GrammarTokens)));
    }

    #[test]
    fn cquery_formatter_setup_failure_preserves_prints_within_the_stderr_limit() {
        let source = "print('setup first')\nprint('setup second')\nfail('setup failure')\n";
        let expected = concat!(
            "setup first\nsetup second\n",
            "ERROR: invalid --starlark:file: fail: setup failure\n"
        );
        let run = |stream_bytes| {
            let budget = Arc::new(
                ExecutionBudget::new(10_000, 1 << 20, STACK_DEPTH).with_closed_formatter_bytecode(),
            );
            let error = render_with_budget(
                "format.star",
                source,
                std::iter::from_fn(|| panic!("setup failure must precede target iteration")),
                &budget,
                Instant::now(),
                stream_bytes,
            )
            .unwrap_err();
            (error, budget)
        };

        let (error, budget) = run(expected.len());
        let CqueryStarlarkError::Setup { stderr } = error else {
            panic!("expected a setup failure, got {error:?}");
        };
        assert_eq!(stderr, expected);
        assert_eq!(budget.check(), Ok(()));

        let (error, budget) = run(expected.len() - 1);
        assert!(matches!(error, CqueryStarlarkError::Request(error)
            if error.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::TextBytes)));
        assert_eq!(budget.check(), Err(ExecutionLimit::TextBytes));
    }

    #[test]
    fn cquery_formatter_renders_native_str_with_bounded_expanded_shape() {
        let module = Module::new();
        let budget = ExecutionBudget::new(4096, 4096, 16);
        let mut text = BoundedText::new(&budget, 128);
        let string = module.heap().alloc("x");
        let tuple = module
            .heap()
            .alloc(AllocTuple([string, module.heap().alloc(2)]));
        append_result(&mut text, tuple).unwrap();
        assert_eq!(text.as_str(), "(\"x\", 2)\n");

        let mut shared = string;
        for _ in 0..8 {
            shared = module.heap().alloc(AllocList([shared, shared]));
        }
        assert_eq!(
            append_result(&mut text, shared),
            Err(ExecutionLimit::TextBytes)
        );
        assert_eq!(text.finish(), Err(ExecutionLimit::TextBytes));
    }

    fn target(name: &str) -> ConfiguredNodeResult {
        ConfiguredNodeResult::new_native(
            ConfiguredNodeKey::null(CanonicalLabel::parse(name).unwrap()),
            ConfiguredNodeKind::SourceFile,
            ProviderCollection::new(vec![ProviderValue::DefaultInfo(DefaultInfo::empty())])
                .unwrap(),
            None,
            RunfilesPackageDepset::empty(),
        )
    }

    #[test]
    fn cquery_formatter_keeps_setup_prints_and_skips_only_the_failed_row() {
        let b = target("@@//pkg:b");
        let skip = target("@@//pkg:skip");
        let a = target("@@//pkg:a");
        let source = r#"print("setup")
def format(target):
    label = str(target.label)
    if label.endswith(":skip"):
        fail("row failure")
    return label
"#;
        let (stdout, stderr) = render_cquery_starlark(
            "format.star",
            source,
            [(&b, "//pkg:b"), (&skip, "//pkg:skip"), (&a, "//pkg:a")].into_iter(),
        )
        .unwrap();
        assert_eq!(stdout, "@@//pkg:b\n@@//pkg:a\n");
        assert_eq!(
            stderr,
            "setup\nERROR: Starlark evaluation error for //pkg:skip: fail: row failure\n"
        );
    }

    #[test]
    fn cquery_formatter_request_refuses_one_more_print_across_setup_and_rows() {
        let a = target("@@//pkg:a");
        let b = target("@@//pkg:b");
        let source = "print('setup')\ndef format(target):\n    print('row')\n    return 'value'\n";
        let run = |budget: &Arc<ExecutionBudget>, count| {
            render_with_budget(
                "format.star",
                source,
                [(&a, "//pkg:a"), (&b, "//pkg:b")].into_iter().take(count),
                budget,
                Instant::now(),
                "setup\nrow\n".len(),
            )
        };
        let precise = Arc::new(
            ExecutionBudget::new(10_000, 1 << 20, STACK_DEPTH).with_closed_formatter_bytecode(),
        );
        assert_eq!(
            run(&precise, 1).unwrap(),
            ("value\n".into(), "setup\nrow\n".into())
        );
        let short = Arc::new(
            ExecutionBudget::new(10_000, 1 << 20, STACK_DEPTH).with_closed_formatter_bytecode(),
        );
        assert!(
            matches!(run(&short, 2).unwrap_err(), CqueryStarlarkError::Request(error)
            if error.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::TextBytes))
        );
        assert_eq!(short.check(), Err(ExecutionLimit::TextBytes));
    }
}
