//! Actual frozen callbacks; direct backend units are not publication certificates.
use std::sync::Arc;

use slug_build_api_v2::actions::ResourceSetOs;
use starlark::environment::Globals;
use starlark::environment::LibraryExtension;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[derive(Default)]
struct Consumer {
    cancelled: Cell<bool>,
    cancel_after: Cell<Option<usize>>,
    checks: Cell<usize>,
    prints: RefCell<Vec<String>>,
    print_error: RefCell<Option<ResourceSetError>>,
    cancel_on_print: Cell<bool>,
}
impl ResourceSetEvaluationContext for Consumer {
    fn is_cancelled(&self) -> bool {
        let checked = self.checks.get() + 1;
        self.checks.set(checked);
        self.cancelled.get()
            || self
                .cancel_after
                .get()
                .is_some_and(|limit| checked >= limit)
    }
    fn print(&self, text: &str) -> Result<(), ResourceSetError> {
        self.prints.borrow_mut().push(text.to_owned());
        if self.cancel_on_print.get() {
            self.cancelled.set(true);
        }
        match self.print_error.borrow().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn mnemonic(&self) -> &str {
        "NumericConsumer"
    }
}
fn runtime(source: &str) -> StarlarkResourceSetRuntime {
    let producer = Module::new();
    Evaluator::new(&producer)
        .eval_module(
            AstModule::parse(
                "unrelated_original_resource.bzl",
                source.to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::extended_by(&[LibraryExtension::Print]),
        )
        .unwrap();
    // Both producer and FrozenModule are disposed before the first call.
    StarlarkResourceSetRuntime::new(producer.freeze().unwrap().get("resource").unwrap())
}
fn returned(expression: &str) -> StarlarkResourceSetRuntime {
    runtime(&format!("def resource(os, count): return {expression}\n"))
}
fn inputs() -> ResourceSetInputs {
    ResourceSetInputs {
        os: ResourceSetOs::Linux,
        parent_input_count: 0,
    }
}
fn invalid(expression: &str, expected: &str) {
    let error = returned(expression)
        .evaluate(inputs(), &Consumer::default())
        .unwrap_err();
    assert!(
        matches!(&error, ResourceSetError::InvalidReturn(message)
        if message.contains(expected) && message.contains("NumericConsumer")),
        "{expression}: {error}"
    );
}

#[test]
fn numeric_original_globals_and_cells_survive_producer_drop_for_all_os_and_counts() {
    let backend = Arc::new(runtime(
        r#"
DATA = [13]
OS = {'linux':1,'osx':2,'windows':3,'freebsd':4,'openbsd':5,'unknown':6}
def make():
    cell = 7
    def original(os, count):
        return {'memory': DATA[0] + cell + OS[os] + count}
    return original
resource = make()
"#,
    ));
    let weak = Arc::downgrade(&backend);
    let retained = backend.clone();
    drop(backend);
    assert!(weak.upgrade().is_some());
    for (os, offset) in [
        (ResourceSetOs::Linux, 1),
        (ResourceSetOs::Osx, 2),
        (ResourceSetOs::Windows, 3),
        (ResourceSetOs::FreeBsd, 4),
        (ResourceSetOs::OpenBsd, 5),
        (ResourceSetOs::Unknown, 6),
    ] {
        for count in [0, 1, 42] {
            let value = retained
                .evaluate(
                    ResourceSetInputs {
                        os,
                        parent_input_count: count,
                    },
                    &Consumer::default(),
                )
                .unwrap();
            assert_eq!(
                value,
                ResourceEstimate {
                    memory_mb: (20 + offset + count) as f64,
                    cpu: 1.0,
                    local_test: 0
                }
            );
        }
    }
    drop(retained);
    assert!(
        weak.upgrade().is_none(),
        "last backend Arc must release the actual original owner"
    );
    // Arc Weak observes the backend, not private FrozenHeapRef internals; the
    // actual post-producer values above prove original globals/cells remain live.
}

#[test]
fn numeric_defaults_types_complete_key_validation_and_deferred_arity_are_exact() {
    for (expression, expected) in [
        ("{}", (250.0, 1.0, 0)),
        ("{'cpu':2.5}", (250.0, 2.5, 0)),
        ("{'memory':7,'local_test':3.9}", (7.0, 1.0, 3)),
        ("{'memory':7.25,'cpu':2,'local_test':-3.9}", (7.25, 2.0, -3)),
    ] {
        let value = returned(expression)
            .evaluate(inputs(), &Consumer::default())
            .unwrap();
        assert_eq!((value.memory_mb, value.cpu, value.local_test), expected);
    }
    for expression in ["None", "True", "3", "3.0", "'dictionary'", "[]", "()"] {
        invalid(expression, "expected dictionary");
    }
    for key in ["memory", "cpu", "local_test"] {
        for value in ["True", "False", "None", "'3'", "[]", "{}"] {
            invalid(&format!("{{'{key}':{value}}}"), "expected int or float");
        }
    }
    // Entire key type pass precedes unknown names and invalid numeric values.
    invalid("{'unknown':0,17:1,'memory':False}", "keys must be strings");
    invalid("{'memory':False,'unknown':0}", "unknown resource key");
    invalid("{'unknown':0,'cpu':None}", "unknown resource key");
    for source in [
        "def resource(os): return {}\n",
        "def resource(os, count, extra): return {}\n",
    ] {
        let callback = runtime(source); // construction does not invoke/validate arity
        let error = callback
            .evaluate(inputs(), &Consumer::default())
            .unwrap_err();
        assert!(
            matches!(&error, ResourceSetError::Callback(message)
            if message.contains("NumericConsumer")),
            "{error}"
        );
    }
}

#[test]
fn numeric_int_float_negative_nonfinite_and_java_i32_conversion_are_preserved() {
    let estimate = returned("{'memory':-0.0,'cpu':-2.5,'local_test':-3.75}")
        .evaluate(inputs(), &Consumer::default())
        .unwrap();
    assert_eq!(estimate.memory_mb.to_bits(), (-0.0_f64).to_bits());
    assert_eq!((estimate.cpu, estimate.local_test), (-2.5, -3));
    let estimate =
        returned("{'memory':9007199254740993,'cpu':1 << 1024,'local_test':-(1 << 1024)}")
            .evaluate(inputs(), &Consumer::default())
            .unwrap();
    assert_eq!(estimate.memory_mb, 9007199254740992.0);
    assert_eq!(estimate.cpu, f64::INFINITY);
    assert_eq!(estimate.local_test, i32::MIN);
    let estimate =
        returned("{'memory':float('-inf'),'cpu':float('nan'),'local_test':float('nan')}")
            .evaluate(inputs(), &Consumer::default())
            .unwrap();
    assert_eq!(estimate.memory_mb, f64::NEG_INFINITY);
    assert!(estimate.cpu.is_nan());
    assert_eq!(estimate.local_test, 0);
    for (expression, expected) in [
        ("2147483647", i32::MAX),
        ("2147483648", i32::MAX),
        ("-2147483649", i32::MIN),
        ("float('inf')", i32::MAX),
        ("float('-inf')", i32::MIN),
        ("1e100", i32::MAX),
        ("-1e100", i32::MIN),
    ] {
        assert_eq!(
            returned(&format!("{{'local_test':{expression}}}"))
                .evaluate(inputs(), &Consumer::default())
                .unwrap()
                .local_test,
            expected
        );
    }
}

#[test]
fn numeric_value_order_is_memory_then_cpu_then_local_independent_of_insertion() {
    // A real ~512KiB immutable integer makes later conversion exceed result W.
    // Earlier illegal numeric types must still win; no synthetic budget padding.
    let bits = 64 * (slug_build_api_v2::published_value::NODE_LIMIT + 1);
    // int(base16) constructs the same 2^bits without the per-operation
    // 100_000-bit left-shift operand limit in the producer engine.
    let hexadecimal_digits = bits / 4;
    for expression in [
        "{'local_test':HUGE,'cpu':False,'memory':False}",
        "{'cpu':False,'memory':False,'local_test':HUGE}",
        "{'local_test':HUGE,'cpu':False,'memory':1}",
        "{'memory':1,'cpu':False,'local_test':HUGE}",
    ] {
        let callback = runtime(&format!(
            "HUGE = int('1' + '0' * {hexadecimal_digits}, 16)\ndef resource(os,count): return {expression}\n"
        ));
        assert!(matches!(
            callback.evaluate(inputs(), &Consumer::default()),
            Err(ResourceSetError::InvalidReturn(_))
        ));
    }
    let callback = runtime(&format!(
        "HUGE = int('1' + '0' * {hexadecimal_digits}, 16)\ndef resource(os,count): return {{'cpu':False,'memory':HUGE}}\n"
    ));
    assert!(matches!(
        callback.evaluate(inputs(), &Consumer::default()),
        Err(ResourceSetError::Limit(_))
    ));
}

#[test]
fn numeric_print_current_cancellation_failure_and_next_consumer_are_borrowed() {
    let callback = runtime(
        "def resource(os,count):\n    print('actual original', count)\n    return {'memory':count}\n",
    );
    let first = Consumer::default();
    assert_eq!(
        callback
            .evaluate(
                ResourceSetInputs {
                    parent_input_count: 4,
                    ..inputs()
                },
                &first
            )
            .unwrap()
            .memory_mb,
        4.0
    );
    assert_eq!(*first.prints.borrow(), ["actual original 4"]);
    let rejected = Consumer::default();
    let sink_error = ResourceSetError::InvalidReturn("borrowed print sink rejection".into());
    *rejected.print_error.borrow_mut() = Some(sink_error.clone());
    assert_eq!(callback.evaluate(inputs(), &rejected), Err(sink_error));
    let cancelled = Consumer::default();
    cancelled.cancelled.set(true);
    assert_eq!(
        callback.evaluate(inputs(), &cancelled),
        Err(ResourceSetError::Cancelled)
    );
    assert!(cancelled.prints.borrow().is_empty());
    cancelled.cancelled.set(false);
    callback.evaluate(inputs(), &cancelled).unwrap();
    assert_eq!(*cancelled.prints.borrow(), ["actual original 0"]);
    let during = Consumer::default();
    during.cancel_on_print.set(true);
    assert_eq!(
        callback.evaluate(inputs(), &during),
        Err(ResourceSetError::Cancelled)
    );
    assert_eq!(*during.prints.borrow(), ["actual original 0"]);
    let looping = runtime(
        "def resource(os,count):\n    print('entered original loop')\n    for n in range(1000000):\n        count = count + 1\n    return {}\n",
    );
    let checkpoint = Consumer::default();
    checkpoint.cancel_after.set(Some(8));
    assert_eq!(
        looping.evaluate(inputs(), &checkpoint),
        Err(ResourceSetError::Cancelled)
    );
    assert_eq!(*checkpoint.prints.borrow(), ["entered original loop"]);
    assert!(checkpoint.checks.get() >= 8);
    let next = Consumer::default();
    assert_eq!(
        callback
            .evaluate(
                ResourceSetInputs {
                    parent_input_count: 9,
                    ..inputs()
                },
                &next
            )
            .unwrap()
            .memory_mb,
        9.0
    );
    assert_eq!(*next.prints.borrow(), ["actual original 9"]);
    assert_eq!(*first.prints.borrow(), ["actual original 4"]);
}

#[test]
fn numeric_private_limit_controls_preserve_default_policy_and_fresh_call_recovery() {
    let defaults = ResourceRuntimeLimits::default();
    assert_eq!(
        (defaults.heap_bytes, defaults.timeout, defaults.call_stack),
        (64 * 1024 * 1024, Duration::from_secs(30), 50)
    );
    for limits in [
        ResourceRuntimeLimits {
            timeout: Duration::ZERO,
            ..defaults
        },
        ResourceRuntimeLimits {
            heap_bytes: 0,
            ..defaults
        },
    ] {
        let mut callback = returned("{'memory':17}");
        callback.limits = limits;
        assert!(matches!(
            callback.evaluate(inputs(), &Consumer::default()),
            Err(ResourceSetError::Limit(_))
        ));
        callback.limits = defaults;
        assert_eq!(
            callback
                .evaluate(inputs(), &Consumer::default())
                .unwrap()
                .memory_mb,
            17.0
        );
    }
    let mut callback = runtime("def inner(): return {}\ndef resource(os,count): return inner()\n");
    callback.limits.call_stack = 1;
    let error = callback
        .evaluate(inputs(), &Consumer::default())
        .unwrap_err();
    assert!(
        matches!(&error,ResourceSetError::Callback(message) if message.contains("NumericConsumer")),
        "{error}"
    );
    callback.limits = defaults;
    assert_eq!(
        callback.evaluate(inputs(), &Consumer::default()).unwrap(),
        ResourceEstimate {
            memory_mb: 250.0,
            cpu: 1.0,
            local_test: 0
        }
    );
}
