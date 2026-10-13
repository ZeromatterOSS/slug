//! Native properties are fetched before arguments, then their value is called.
use std::fmt;

use allocative::Allocative;
use starlark_derive::Coerce;
use starlark_derive::Freeze;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;
use starlark_derive::starlark_module;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert::Assert;
use crate::environment::GlobalsBuilder;
use crate::environment::Methods;
use crate::environment::MethodsBuilder;
use crate::environment::MethodsStatic;
use crate::starlark_complex_value;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::list::ListRef;

#[derive(
    Debug,
    Coerce,
    Trace,
    Freeze,
    ProvidesStaticType,
    NoSerialize,
    Allocative
)]
#[repr(C)]
struct AttributeReceiverGen<V> {
    state: V,
}

impl<V> fmt::Display for AttributeReceiverGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<attribute_receiver>")
    }
}

starlark_complex_value!(AttributeReceiver);

#[starlark_value(type = "attribute_receiver")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for AttributeReceiverGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_methods() -> Option<&'static Methods> {
        static METHODS: MethodsStatic = MethodsStatic::new();
        METHODS.methods(receiver_methods)
    }
}

fn selected<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
    let receiver = AttributeReceiver::from_value(this).unwrap();
    let state = ListRef::from_value(receiver.state).unwrap();
    state
        .content()
        .first()
        .copied()
        .ok_or_else(|| starlark::Error::new_other(anyhow::anyhow!("empty receiver state")))
}

#[starlark_module]
fn receiver_methods(builder: &mut MethodsBuilder) {
    #[starlark(attribute)]
    fn callable<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        selected(this)
    }

    // These names collide with compiler-known list/dict methods. Their actual
    // Methods entries are NativeAttribute, so those specializations cannot apply.
    #[starlark(attribute)]
    fn append<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        selected(this)
    }

    #[starlark(attribute)]
    fn clear<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        selected(this)
    }

    #[starlark(attribute)]
    fn broken<'v>(this: Value<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Err(starlark::Error::new_other(anyhow::anyhow!("GETTER_ERROR")))
    }

    fn add(this: Value, first: i32, second: i32) -> anyhow::Result<i32> {
        let _ = this;
        Ok(first + second)
    }
}

#[starlark_module]
fn constructors(builder: &mut GlobalsBuilder) {
    fn native_attr<'v>(state: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
        if ListRef::from_value(state).is_none() {
            anyhow::bail!("native_attr state must be a list")
        }
        Ok(heap.alloc_complex(AttributeReceiverGen { state }))
    }
}

fn assertion() -> Assert<'static> {
    let mut assertion = Assert::new();
    assertion.disable_static_typechecking();
    assertion.globals_add(constructors);
    assertion
}

#[test]
fn native_attribute_noncallable_values_reject_direct_and_separated_calls() {
    let mut assertion = assertion();
    assertion.pass("receiver = native_attr([{'key': 'value'}]); assert_eq(receiver.callable, {'key': 'value'})");
    for expression in [
        "receiver.callable()",
        "property = receiver.callable; property()",
    ] {
        assertion.fails(
            &format!("receiver = native_attr([{{'key': 'value'}}]); {expression}"),
            &["Operation `call()` not supported on type `dict`"],
        );
    }
    let frozen = assertion.pass_module("receiver = native_attr([{'key': 'value'}])");
    assertion.module_add("frozen", frozen);
    assertion.fails(
        "load('frozen', 'receiver'); receiver.callable()",
        &["Operation `call()` not supported on type `dict`"],
    );
}

const FORWARDING: &str = r#"
def actual(first, second=0, *rest, **named):
    return [first, second, list(rest), named]
receiver = native_attr([actual])
def invoke(receiver):
    assert_eq(receiver.callable(1, second=2), [1, 2, [], {}])
    assert_eq(receiver.callable(*[1, 2, 3], **{'named': 4}), [1, 2, [3], {'named': 4}])
    assert_eq(receiver.callable(1, 2, named=5, *[3, 4], **{'other': 6}), [1, 2, [3, 4], {'named': 5, 'other': 6}])
    separated = receiver.callable
    assert_eq(separated(1, second=2), [1, 2, [], {}])
    assert_eq(receiver.add(2, second=3), 5)
    assert_eq(receiver.add(*[2], **{'second': 3}), 5)
invoke(receiver)
"#;

#[test]
fn native_attribute_callable_forwards_all_argument_forms_and_preserves_native_methods() {
    let mut assertion = assertion();
    let frozen = assertion.pass_module(FORWARDING);
    assertion.module_add("frozen", frozen);
    assertion.pass(
        "load('frozen', 'receiver', 'invoke'); invoke(receiver); assert_eq(receiver.callable(1, second=2), [1, 2, [], {}]); assert_eq(receiver.add(4, 5), 9)",
    );
}

#[test]
fn native_attribute_getter_failure_precedes_argument_failure_without_attribute_frame() {
    let assertion = assertion();
    let error = assertion.fail(
        "def invoke(receiver):\n    return receiver.broken(fail('ARGUMENT_ERROR'))\ninvoke(native_attr([None]))",
        "GETTER_ERROR",
    );
    assert_eq!(error.without_diagnostic().to_string(), "GETTER_ERROR");
    assert!(
        error
            .call_stack()
            .frames
            .iter()
            .all(|frame| !frame.name.contains("broken")),
        "getter evaluation must not fabricate an attribute function frame: {:?}",
        error.call_stack()
    );
}

#[test]
fn native_attribute_captures_callee_before_arguments_even_for_known_method_names() {
    assertion().pass(
        r#"
def before(arg): return ['before', arg]
def after(arg): return ['after', arg]
def mutate(state):
    state[0] = after
    return 'argument'
def verify():
    state = [before]
    receiver = native_attr(state)
    assert_eq(receiver.callable(mutate(state)), ['before', 'argument'])
    assert_eq(receiver.callable('next'), ['after', 'next'])
    state[0] = before
    assert_eq(receiver.append(mutate(state)), ['before', 'argument'])
    assert_eq(receiver.append('next'), ['after', 'next'])
    state[0] = before
    assert_eq(receiver.clear(mutate(state)), ['before', 'argument'])
    assert_eq(receiver.clear('next'), ['after', 'next'])
    items = []
    items.append('ordinary list method')
    assert_eq(items, ['ordinary list method'])
    mapping = {'entry': 'ordinary dict method'}
    mapping.clear()
    assert_eq(mapping, {})
verify()
"#,
    );
}

#[test]
fn native_attribute_selected_dynamic_callable_survives_argument_gc_without_old_roots() {
    assertion().pass(
        r#"
def make(payload):
    def chosen(argument, retained=payload):
        return [retained, argument]
    return chosen
def replacement(argument): return ['replacement', argument]
def release_and_collect(state):
    state[0] = replacement
    garbage_collect()
    churn = [str(index) for index in range(200)]
    garbage_collect()
    return len(churn)
def prepare():
    state = [make(['chosen-only'])]
    return [native_attr(state), state]
def verify():
    # The dynamically created function and its mutable default have no module
    # binding. Creation frames have returned, eliminating their temporary roots.
    # Once state[0] is replaced, only the selected-call bytecode slot may retain
    # the callable across the argument's collection.
    prepared = prepare()
    receiver = prepared[0]
    state = prepared[1]
    result = receiver.callable(release_and_collect(state))
    assert_eq(result, [['chosen-only'], 200])
    assert_eq(receiver.callable(7), ['replacement', 7])
verify()
garbage_collect()
"#,
    );
}
