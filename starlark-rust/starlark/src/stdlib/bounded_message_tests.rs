use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;

use allocative::Allocative;

use crate as starlark;
use crate::environment::Globals;
use crate::environment::LibraryExtension;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::stdlib::PrintHandler;
use crate::stdlib::PrintLocation;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueTextMode;
use crate::values::starlark_value;

#[derive(Default)]
struct PrintCapture(RefCell<Vec<(PrintLocation, String)>>);

impl PrintHandler for PrintCapture {
    fn println(&self, location: PrintLocation, text: &str) -> crate::Result<()> {
        self.0.borrow_mut().push((location, text.to_owned()));
        Ok(())
    }
}

fn eval<'v>(module: &'v Module, source: &str, capture: &PrintCapture) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("messages.star", source.to_owned(), &Dialect::Standard).unwrap();
    let mut evaluator = Evaluator::new(module);
    evaluator.set_print_handler(capture);
    evaluator.eval_module(ast, &Globals::extended_by(&[LibraryExtension::Print]))
}

fn strict_budget(work: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(work, 1 << 20, 16).with_bounded_operations())
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct DistinctText {
    bounded: bool,
}

starlark::starlark_simple_value!(DistinctText);

impl fmt::Display for DistinctText {
    fn fmt(&self, _writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("diagnostic content used Rust Display")
    }
}

#[starlark_value(type = "distinct_message_text")]
impl<'v> StarlarkValue<'v> for DistinctText {
    fn collect_str(&self, output: &mut String) {
        assert!(!self.bounded, "bounded content used legacy str");
        output.push_str("str-form");
    }

    fn collect_repr(&self, output: &mut String) {
        assert!(!self.bounded, "bounded content used legacy repr");
        output.push_str("repr-form");
    }

    fn write_bounded_text(
        &self,
        mode: ValueTextMode,
        output: &mut dyn fmt::Write,
    ) -> Option<fmt::Result> {
        assert!(self.bounded, "ordinary content used the bounded writer");
        Some(output.write_str(match mode {
            ValueTextMode::Str => "str-form",
            ValueTextMode::Repr => "repr-form",
            ValueTextMode::Display => panic!("diagnostic content used the Display mode"),
        }))
    }
}

#[test]
fn bounded_print_and_fail_preserve_native_wrapper_message_grammar() {
    let budget = strict_budget(10_000);
    let ordinary = Module::new();
    let strict = Module::new_with_execution_budget(budget.clone());
    let mut observations = Vec::new();
    for (module, bounded) in [(&ordinary, false), (&strict, true)] {
        module.set("subject", module.heap().alloc(DistinctText { bounded }));
        let capture = PrintCapture::default();
        let error = eval(
            module,
            r#"print()
print("first\nsecond", subject, ["nested"], False)
fail("first\nsecond", subject, ["nested"], False, attr="a'b")"#,
            &capture,
        )
        .unwrap_err();
        let crate::ErrorKind::Fail(cause) = error.kind() else {
            panic!("expected fail error, got {error}");
        };
        assert_eq!(
            cause.to_string(),
            " first\nsecond repr-form [\"nested\"] False (in attribute 'a'b')"
        );
        let seen = capture.0.into_inner();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].1, "");
        assert_eq!(seen[1].1, "first\nsecond str-form [\"nested\"] False");
        observations.push(seen);

        for (source, expected) in [("fail()", ""), ("fail('', attr='')", " ")] {
            let error = eval(module, source, &PrintCapture::default()).unwrap_err();
            let crate::ErrorKind::Fail(cause) = error.kind() else {
                panic!("expected fail error, got {error}");
            };
            assert_eq!(cause.to_string(), expected);
        }
    }
    assert_eq!(observations[0], observations[1]);
    assert_eq!(budget.check(), Ok(()));
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct MessagePoison;

starlark::starlark_simple_value!(MessagePoison);

impl fmt::Display for MessagePoison {
    fn fmt(&self, _writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("rendered an unmodeled diagnostic value")
    }
}

#[starlark_value(type = "message_poison")]
impl<'v> StarlarkValue<'v> for MessagePoison {
    fn collect_repr(&self, _output: &mut String) {
        panic!("rendered an unmodeled diagnostic value")
    }
}

fn assert_limit(error: crate::Error, budget: &ExecutionBudget, limit: ExecutionLimit) {
    assert!(matches!(error.kind(), crate::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&limit)));
    assert_eq!(budget.check(), Err(limit));
}

#[test]
fn bounded_fail_validates_attr_before_rendering_and_refuses_legacy_repr() {
    let budget = strict_budget(10_000);
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("poison", module.heap().alloc(MessagePoison));
    let capture = PrintCapture::default();
    let error = eval(&module, "fail(poison, attr=poison)", &capture).unwrap_err();
    let crate::ErrorKind::Value(cause) = error.kind() else {
        panic!("expected a parameter type error, got {error}");
    };
    assert_eq!(
        cause.to_string(),
        "Type of parameter `attr` doesn't match, expected `string`, actual `message_poison`"
    );
    assert_eq!(budget.check(), Ok(()));
    assert_limit(
        eval(&module, "fail('first', poison)", &capture).unwrap_err(),
        &budget,
        ExecutionLimit::UnsupportedText("message_poison"),
    );
}

#[test]
fn bounded_print_never_emits_partial_content_on_refusal() {
    let budget = strict_budget(10_000);
    let module = Module::new_with_execution_budget(budget.clone());
    module.set("poison", module.heap().alloc(MessagePoison));
    let capture = PrintCapture::default();
    assert_limit(
        eval(
            &module,
            "print('already emitted')\nprint('must stay private', poison)",
            &capture,
        )
        .unwrap_err(),
        &budget,
        ExecutionLimit::UnsupportedText("message_poison"),
    );
    let seen = capture.0.into_inner();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].1, "already emitted");

    for message in ["small".to_owned(), "x".repeat(256 * 1024)] {
        let budget = strict_budget(512);
        let module = Module::new_with_execution_budget(budget.clone());
        module.set("message", module.heap().alloc(message.as_str()));
        let capture = PrintCapture::default();
        let result = eval(&module, "print('prefix', message)", &capture);
        let seen = capture.0.into_inner();
        if message == "small" {
            result.unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].1, "prefix small");
        } else {
            assert_limit(result.unwrap_err(), &budget, ExecutionLimit::Work);
            assert!(seen.is_empty());
        }
    }
}
