//! Immediate native caller locals, including captured cells and frozen defs.

use super::Evaluator;
use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;

#[starlark_module]
fn globals(builder: &mut GlobalsBuilder) {
    fn local<'v>(name: &str, eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        Ok(eval
            .native_call_local_value(name)
            .unwrap_or_else(Value::new_none))
    }
}

#[test]
fn native_local_values_stay_in_the_immediate_frame_and_unwrap_captures() {
    let module = Module::new();
    let globals = GlobalsBuilder::standard().with(globals).build();
    let ast = AstModule::parse(
        "native_locals.star",
        r#"
value = "module value"
def helper(value):
    def keep():
        return value
    return [local("value"), local("missing"), keep()]
def inner():
    return local("value")
def outer(value):
    return inner()
direct = local("value")
mutable = helper([1, 2])
no_outer = outer("outer value")
"#
        .to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&module).eval_module(ast, &globals).unwrap();
    assert!(module.get("direct").unwrap().is_none());
    assert!(module.get("no_outer").unwrap().is_none());
    assert_eq!(
        module.get("mutable").unwrap().to_repr(),
        "[[1, 2], None, [1, 2]]"
    );
    let frozen = module.freeze().unwrap();
    let helper = frozen.get("helper").unwrap();
    let scratch = Module::new();
    let value = scratch.heap().alloc("after freeze");
    let result = Evaluator::new(&scratch)
        .eval_function(helper.value(), &[value], &[])
        .unwrap();
    assert_eq!(
        result.to_repr(),
        r#"["after freeze", None, "after freeze"]"#
    );
}
