//! One GC-traced module root for Loading's evaluator-local scratch values.
//! Never place moving Values in Evaluator.extra or an untraced host map.
use starlark::environment::Module;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;

fn roots<'v>(module: &'v Module) -> [Value<'v>; 4] {
    if let Some(root) = module.extra_value() {
        let root = TupleRef::from_value(root).expect("Loading owns module extra value");
        return [
            root.content()[0],
            root.content()[1],
            root.content()[2],
            root.content()[3],
        ];
    }
    let roots = [
        Value::new_none(),
        module.heap().alloc(AllocDict(Vec::<(Value, Value)>::new())),
        Value::new_none(),
        Value::new_none(),
    ];
    module.set_extra_value(module.heap().alloc(AllocTuple(roots)));
    roots
}

pub(crate) fn empty_depsets<'v>(module: &'v Module) -> Option<Value<'v>> {
    let value = roots(module)[0];
    (!value.is_none()).then_some(value)
}

pub(crate) fn set_empty_depsets<'v>(module: &'v Module, cache: Value<'v>) {
    let mut values = roots(module);
    values[0] = cache;
    module.set_extra_value(module.heap().alloc(AllocTuple(values)));
}

pub(crate) fn callable_registry<'v>(module: &'v Module) -> Value<'v> {
    roots(module)[1]
}

pub(crate) fn existing_callable_registry<'v>(module: &'v Module) -> Option<Value<'v>> {
    module
        .extra_value()
        .map(|root| TupleRef::from_value(root).expect("Loading roots").content()[1])
}

pub(crate) fn analysis_context<'v>(module: &'v Module) -> Option<Value<'v>> {
    let value = module
        .extra_value()
        .map(|root| TupleRef::from_value(root).expect("Loading roots").content()[2])?;
    (!value.is_none()).then_some(value)
}

pub(crate) fn set_analysis_context<'v>(module: &'v Module, context: Value<'v>) {
    let mut values = roots(module);
    values[2] = context;
    module.set_extra_value(module.heap().alloc(AllocTuple(values)));
}

pub(crate) fn pending_cc_actions<'v>(module: &'v Module) -> Value<'v> {
    roots(module)[3]
}

pub(crate) fn set_pending_cc_actions<'v>(module: &'v Module, pending: Value<'v>) {
    let mut values = roots(module);
    values[3] = pending;
    module.set_extra_value(module.heap().alloc(AllocTuple(values)));
}
