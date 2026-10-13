//! Original defining-module metadata survives native calls, optimization and GC.

use super::Evaluator;
use crate as starlark;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::ReturnFileLoader;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;
use crate::values::list::AllocList;
use crate::values::none::NoneType;
use crate::values::tuple::AllocTuple;

#[starlark_module]
fn client_globals(builder: &mut GlobalsBuilder) {
    fn context<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        Ok(eval
            .native_call_defining_module_client_data()
            .unwrap_or_else(Value::new_none))
    }

    fn probe<'v>(eval: &mut Evaluator<'v, '_, '_>) -> anyhow::Result<Value<'v>> {
        let context = eval
            .native_call_defining_module_client_data()
            .unwrap_or_else(Value::new_none);
        let inlined = eval
            .call_stack
            .native_call_innermost_inlined_function()
            .is_some();
        Ok(eval
            .heap()
            .alloc(AllocTuple([context, eval.heap().alloc(inlined)])))
    }

    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<NoneType> {
        // No moving Value is retained in native Rust locals. Module metadata,
        // bytecode locals and exported results are all traced evaluator roots.
        unsafe {
            eval.garbage_collect();
        }
        Ok(NoneType)
    }
}

fn globals() -> Globals {
    GlobalsBuilder::standard().with(client_globals).build()
}

fn compile(context: Option<&str>, source: &str) -> FrozenModule {
    let module = Module::new();
    if let Some(context) = context {
        module
            .set_client_data_no_overwrite(module.heap().alloc(context))
            .unwrap();
    }
    let ast = AstModule::parse(
        "identical_filename.bzl",
        source.to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&module)
        .eval_module(ast, &globals())
        .unwrap();
    module.freeze().unwrap()
}

#[test]
fn module_client_data_is_set_once_traced_and_frozen_without_changing_extra_value() {
    let module = Module::new();
    module.set_extra_value(module.heap().alloc("separate extra root"));
    module
        .set_client_data_no_overwrite(
            module
                .heap()
                .alloc(AllocList(["metadata", "only client root"])),
        )
        .unwrap();
    assert!(
        module
            .set_client_data_no_overwrite(Value::new_none())
            .is_err()
    );
    let ast = AstModule::parse(
        "module.bzl",
        "before = context()\ncollect()\nafter = context()\ndef f():\n    return context()\n"
            .to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&module)
        .eval_module(ast, &globals())
        .unwrap();
    assert_eq!(
        module.client_data().unwrap().to_repr(),
        "[\"metadata\", \"only client root\"]"
    );
    assert_eq!(module.get("before").unwrap(), module.get("after").unwrap());
    assert_eq!(
        module.extra_value().unwrap().unpack_str(),
        Some("separate extra root")
    );
    let module = module.freeze().unwrap();
    assert_eq!(
        module.client_data().unwrap().to_value().to_repr(),
        "[\"metadata\", \"only client root\"]"
    );
    assert_eq!(
        module.extra_value().unwrap().unpack_str(),
        Some("separate extra root")
    );
    let function = module.get("f").unwrap();
    let scratch = Module::new();
    let result = Evaluator::new(&scratch)
        .eval_function(function.value(), &[], &[])
        .unwrap();
    assert_eq!(result.to_repr(), "[\"metadata\", \"only client root\"]");

    let none = Module::new();
    none.set_client_data_no_overwrite(Value::new_none())
        .unwrap();
    assert!(
        none.set_client_data_no_overwrite(none.heap().alloc("replacement"))
            .is_err()
    );
}

#[test]
fn native_constructor_passed_as_value_uses_helper_module_and_missing_original_stays_missing() {
    let helper = compile(
        Some("defining helper"),
        "def via(ctor):\n    return ctor()\n",
    );
    let constructor = compile(Some("constructor exporter"), "constructor = context\n")
        .get("constructor")
        .unwrap();
    let via = helper.get("via").unwrap();
    drop(helper);
    let scratch = Module::new();
    scratch
        .set_client_data_no_overwrite(scratch.heap().alloc("consumer metadata"))
        .unwrap();
    let function = via.owned_value(scratch.frozen_heap());
    let constructor = constructor.owned_value(scratch.frozen_heap());
    drop(via);
    assert_eq!(
        Evaluator::new(&scratch)
            .eval_function(function, &[constructor], &[])
            .unwrap()
            .unpack_str(),
        Some("defining helper")
    );

    let missing = compile(None, "def plain():\n    return context()\n");
    let plain = missing.get("plain").unwrap();
    let plain = plain.owned_value(scratch.frozen_heap());
    drop(missing);
    assert!(
        Evaluator::new(&scratch)
            .eval_function(plain, &[], &[])
            .unwrap()
            .is_none()
    );
    // A native call from Rust has no executing Starlark definition or module
    // bytecode, even if the scratch Module contains unrelated client metadata.
    assert!(
        Evaluator::new(&scratch)
            .eval_function(constructor, &[], &[])
            .unwrap()
            .is_none()
    );
}

#[test]
fn identical_filenames_keep_distinct_original_modules_through_actual_frozen_inlining() {
    for metadata in [
        Some("canonical module A / mapping one"),
        Some("canonical module B / mapping two"),
        None,
    ] {
        let producer = compile(metadata, "def immediate():\n    return probe()\n");
        let modules = [("producer", &producer)].into_iter().collect();
        let loader = ReturnFileLoader { modules: &modules };
        let consumer = Module::new();
        consumer
            .set_client_data_no_overwrite(consumer.heap().alloc("consumer module"))
            .unwrap();
        let ast = AstModule::parse(
            "identical_filename.bzl",
            "load('producer', 'immediate')\ndef run():\n    return immediate()\n".to_owned(),
            &Dialect::Standard,
        )
        .unwrap();
        let mut eval = Evaluator::new(&consumer);
        eval.set_loader(&loader);
        eval.eval_module(ast, &globals()).unwrap();
        drop(eval);
        let consumer = consumer.freeze().unwrap();
        let run = consumer.get("run").unwrap();
        drop(consumer);
        drop(modules);
        drop(producer);
        let scratch = Module::new();
        let function = run.owned_value(scratch.frozen_heap());
        drop(run);
        let result = Evaluator::new(&scratch)
            .eval_function(function, &[], &[])
            .unwrap();
        assert_eq!(
            result.to_repr(),
            format!(
                "({}, True)",
                metadata
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|| "None".into())
            ),
            "the native probe must execute inside an actual inlined defining function"
        );
    }
}

#[test]
fn runtime_nested_function_keeps_producer_metadata_after_gc_refreeze_and_owner_drop() {
    let producer = compile(
        Some("original nested producer"),
        "def make():\n    def nested():\n        return context()\n    return nested\n",
    );
    let make = producer.get("make").unwrap();
    drop(producer);
    let runtime = Module::new();
    runtime
        .set_client_data_no_overwrite(runtime.heap().alloc("runtime consumer"))
        .unwrap();
    let factory = make.owned_value(runtime.frozen_heap());
    drop(make);
    let mut eval = Evaluator::new(&runtime);
    let nested = eval.eval_function(factory, &[], &[]).unwrap();
    runtime.set("nested", nested);
    // Collection runs within actual bytecode, with an initialized frame and
    // the returned moving function rooted in the runtime Module.
    let ast = AstModule::parse(
        "runtime.bzl",
        "collect()\nresult = nested()\n".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    eval.eval_module(ast, &globals()).unwrap();
    assert_eq!(
        runtime.get("result").unwrap().unpack_str(),
        Some("original nested producer")
    );
    drop(eval);
    let frozen = runtime.freeze().unwrap();
    assert_eq!(
        frozen.client_data().unwrap().unpack_str(),
        Some("runtime consumer")
    );
    let nested = frozen.get("nested").unwrap();
    drop(frozen);
    let final_module = Module::new();
    let function = nested.owned_value(final_module.frozen_heap());
    drop(nested);
    assert_eq!(
        Evaluator::new(&final_module)
            .eval_function(function, &[], &[])
            .unwrap()
            .unpack_str(),
        Some("original nested producer")
    );
}

#[test]
fn foreign_frozen_client_data_requires_and_retains_the_existing_heap_reference() {
    let foreign = crate::values::OwnedFrozenValue::alloc("owned foreign metadata");
    let module = Module::new();
    // owned_value installs the existing heap reference; the setter does not.
    module
        .set_client_data_no_overwrite(foreign.owned_value(module.frozen_heap()))
        .unwrap();
    drop(foreign);
    let frozen = module.freeze().unwrap();
    assert_eq!(
        frozen.client_data().unwrap().unpack_str(),
        Some("owned foreign metadata")
    );
}
