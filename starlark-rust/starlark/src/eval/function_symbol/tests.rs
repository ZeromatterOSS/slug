use std::fmt;

use super::*;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::environment::FrozenModule;
use crate::environment::Globals;
use crate::environment::GlobalsBuilder;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::starlark_module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::NoSerialize;
use crate::values::StarlarkValue;
use crate::values::list::ListRef;
use crate::values::starlark_value;
use crate::values::structs::starlark_structural_is_immutable;
use crate::values::tuple::TupleRef;

#[derive(Debug, Allocative)]
struct Owner(&'static str);
impl FunctionSymbolOwner for Owner {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equals(&self, other: &dyn FunctionSymbolOwner) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.0 == other.0)
    }
    fn hash_owner(&self, mut state: &mut dyn Hasher) {
        self.0.hash(&mut state);
    }
}
fn session(name: &'static str) -> FunctionSymbolSession {
    FunctionSymbolSession::new(Arc::new(Owner(name)))
}
fn ast(source: &str) -> AstModule {
    AstModule::parse(
        "same_physical_path.bzl",
        source.to_owned(),
        &Dialect {
            enable_top_level_stmt: true,
            ..Dialect::Standard
        },
    )
    .unwrap()
}
fn run(source: &str, owner: &'static str, exports: bool, prepared: bool) -> FrozenModule {
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_function_symbol_session(session(owner)).unwrap();
    if exports {
        eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
            .unwrap();
    }
    if prepared {
        let prepared = eval
            .prepare_module(ast(source), &Globals::standard())
            .unwrap();
        eval.eval_prepared_module(&prepared).unwrap();
    } else {
        eval.eval_module(ast(source), &Globals::standard()).unwrap();
    }
    drop(eval);
    module.freeze().unwrap()
}
fn global_name(value: Value<'_>) -> String {
    let facts = inspect_function_symbol(value).unwrap();
    match facts.state() {
        FunctionSymbolStateRef::Global { name } => name.to_owned(),
        other => panic!("expected export, got {other:?}"),
    }
}

#[test]
fn actual_tokens_use_complete_owner_and_function_only_local_ordinals() {
    let source = "def factory():\n    def local():\n        return 7\n    return local\na = factory()\nb = factory()\n";
    let a = run(source, "owner-a", false, false);
    let same = run(source, "owner-a", false, false);
    let other = run(source, "owner-b", false, false);
    let f = a.get("a").unwrap();
    assert!(f.value().equals(same.get("a").unwrap().value()).unwrap());
    assert!(!f.value().equals(a.get("b").unwrap().value()).unwrap());
    assert!(!f.value().equals(other.get("a").unwrap().value()).unwrap());
    assert_eq!(
        f.value().get_hashed().unwrap().hash(),
        same.get("a").unwrap().value().get_hashed().unwrap().hash()
    );
    assert_eq!(
        inspect_function_symbol(f.value()).unwrap().state(),
        FunctionSymbolStateRef::Local { ordinal: 1 }
    );
    assert!(!inspect_function_symbol(f.value()).unwrap().is_transient());
    // Exported symbols ignore their creation ordinal; only owner + first name.
    let first = session("exports");
    let second = session("exports");
    let x = first.next().unwrap();
    second.next().unwrap();
    let y = second.next().unwrap();
    x.export(&first, "name").unwrap();
    y.export(&second, "name").unwrap();
    assert!(x.equals(&y));
    let mut hx = StarlarkHasher::new();
    let mut hy = StarlarkHasher::new();
    x.write_hash(&mut hx);
    y.write_hash(&mut hy);
    assert_eq!(hx.finish(), hy.finish());

    // Two actual factory products exported successively to the same global
    // binding have equal language symbols, but remain distinct creation events.
    let module = run(
        "def factory():\n    def local():\n        return 1\n    return local\nselected = factory()\nsaved = [selected]\nselected = factory()\n",
        "selection",
        true,
        false,
    );
    let saved = module.get("saved").unwrap();
    let before = ListRef::from_value(saved.value()).unwrap().content()[0];
    let after = module.get("selected").unwrap();
    assert!(before.equals(after.value()).unwrap());
    assert_eq!(global_name(before), "selected");
    assert_eq!(global_name(after.value()), "selected");
    assert_ne!(
        inspect_function_symbol(before).unwrap().creation_ordinal(),
        inspect_function_symbol(after.value())
            .unwrap()
            .creation_ordinal(),
    );
}

#[test]
fn default_transient_sessions_are_distinct_and_explicit_continuation_keeps_counter() {
    let producer = run(
        "BASE = 'lexical'\ndef factory():\n    def local():\n        return BASE\n    return local\n",
        "producer",
        false,
        false,
    );
    let factory = producer.get("factory").unwrap();
    let one = Module::new();
    let two = Module::new();
    let mut eval_one = Evaluator::new(&one);
    let mut eval_two = Evaluator::new(&two);
    let a = eval_one.eval_function(factory.value(), &[], &[]).unwrap();
    one.set("a", a);
    let b = eval_one.eval_function(factory.value(), &[], &[]).unwrap();
    one.set("b", b);
    assert!(!a.equals(b).unwrap());
    assert!(inspect_function_symbol(a).unwrap().is_transient());
    assert!(
        inspect_function_symbol(a)
            .unwrap()
            .owner()
            .equals(&**eval_one.function_symbol_session().owner())
    );
    assert!(
        !inspect_function_symbol(a)
            .unwrap()
            .owner()
            .equals(&**eval_two.function_symbol_session().owner())
    );
    let continuation = eval_one.function_symbol_session().dupe();
    eval_two.set_function_symbol_session(continuation).unwrap();
    let c = eval_two.eval_function(factory.value(), &[], &[]).unwrap();
    two.set("c", c);
    assert_eq!(
        inspect_function_symbol(c).unwrap().state(),
        FunctionSymbolStateRef::Local { ordinal: 2 }
    );
    assert_eq!(
        eval_two.eval_function(c, &[], &[]).unwrap().unpack_str(),
        Some("lexical")
    );
    assert!(
        eval_one
            .set_function_symbol_session(session("reset"))
            .is_err()
    );
}

#[test]
fn post_assignment_exports_preserve_private_first_alias_binding_order_and_prepared_path() {
    let source = "def factory():\n    def local():\n        return 1\n    return local\n_private = factory()\npublic = _private\n_private = 0\nleft, right = factory(), factory()\nitems = [factory()]\nitems[0] = factory()\nif True:\n    indented = factory()\n";
    for prepared in [false, true] {
        let module = run(source, "module", true, prepared);
        assert_eq!(
            global_name(module.get("public").unwrap().value()),
            "_private"
        );
        assert_eq!(global_name(module.get("left").unwrap().value()), "left");
        assert_eq!(global_name(module.get("right").unwrap().value()), "right");
        assert!(matches!(
            inspect_function_symbol(module.get("indented").unwrap().value())
                .unwrap()
                .state(),
            FunctionSymbolStateRef::Local { .. }
        ));
        let consumer = Module::new();
        consumer.import_public_symbols(&module);
        let mut eval = Evaluator::new(&consumer);
        eval.set_function_symbol_session(session("foreign"))
            .unwrap();
        eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
            .unwrap();
        eval.eval_module(ast("renamed = public"), &Globals::standard())
            .unwrap();
        assert_eq!(global_name(consumer.get("renamed").unwrap()), "_private");
    }
}

#[starlark_module]
fn helpers(builder: &mut GlobalsBuilder) {
    fn immutable(value: Value<'_>) -> anyhow::Result<bool> {
        Ok(starlark_structural_is_immutable(value))
    }
    fn collect(eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<bool> {
        for n in 0..512 {
            eval.heap()
                .alloc_str(&format!("unreachable-{n}-{}", "x".repeat(128)));
        }
        let before = eval.heap().allocated_bytes();
        unsafe {
            eval.garbage_collect()
                .map_err(|error| error.into_anyhow())?;
        }
        Ok(eval.heap().allocated_bytes() < before)
    }
    fn owner(depth: usize, eval: &mut Evaluator<'_, '_, '_>) -> anyhow::Result<String> {
        Ok(eval
            .native_call_defining_module_client_data_at_depth(depth)
            .and_then(Value::unpack_str)
            .unwrap_or("none")
            .to_owned())
    }
}
fn globals() -> Globals {
    GlobalsBuilder::standard().with(helpers).build()
}

#[test]
fn pure_immutability_does_not_hash_and_first_export_after_local_hash_refuses() {
    let source = "def factory():\n    return lambda: 1\nbox = [factory()]\ncheck = immutable(box[0])\nexported = box[0]\n";
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
        .unwrap();
    eval.eval_module(ast(source), &globals()).unwrap();
    assert_eq!(global_name(module.get("exported").unwrap()), "exported");
    assert_eq!(module.get("check").unwrap().unpack_bool(), Some(true));
    assert!(
        !inspect_function_symbol(module.get("exported").unwrap())
            .unwrap()
            .local_hash_observed()
    );
    for prepared in [false, true] {
        let module = Module::new();
        let mut eval = Evaluator::new(&module);
        eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
            .unwrap();
        let source = "def factory():\n    return lambda: 1\nbox = [factory()]\nkeys = {box[0]: 1}\nexported = box[0]\n";
        let result = if prepared {
            let p = eval
                .prepare_module(ast(source), &Globals::standard())
                .unwrap();
            eval.eval_prepared_module(&p)
        } else {
            eval.eval_module(ast(source), &Globals::standard())
        };
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("exporting a local function after hashing is unsupported")
        );
        assert!(matches!(
            inspect_function_symbol(module.get("exported").unwrap())
                .unwrap()
                .state(),
            FunctionSymbolStateRef::Local { .. }
        ));
    }
}

#[test]
fn actual_function_tokens_dictionary_aliases_survive_forced_gc_freeze_and_producer_drop() {
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_function_symbol_session(session("gc-owner"))
        .unwrap();
    eval.eval_module(
        ast("def f():\n    return 31\nalias = f\nkeys = {f: 'kept'}\ncollected = collect()\n"),
        &globals(),
    )
    .unwrap();
    assert_eq!(module.get("collected").unwrap().unpack_bool(), Some(true));
    let before = inspect_function_symbol(module.get("f").unwrap())
        .unwrap()
        .state();
    assert_eq!(before, FunctionSymbolStateRef::Local { ordinal: 0 });
    drop(eval);
    let frozen = module.freeze().unwrap();
    let f = frozen.get("f").unwrap();
    let alias = frozen.get("alias").unwrap();
    let keys = frozen.get("keys").unwrap();
    drop(frozen);
    let consumer = Module::new();
    consumer.set("f", consumer.frozen_heap().alloc(f).to_value());
    consumer.set("alias", consumer.frozen_heap().alloc(alias).to_value());
    consumer.set("keys", consumer.frozen_heap().alloc(keys).to_value());
    let mut eval = Evaluator::new(&consumer);
    eval.eval_module(
        ast("same = f == alias\nresult = f()\nkey = keys[alias]\n"),
        &Globals::standard(),
    )
    .unwrap();
    assert_eq!(consumer.get("same").unwrap().unpack_bool(), Some(true));
    assert_eq!(consumer.get("result").unwrap().unpack_i32(), Some(31));
    assert_eq!(consumer.get("key").unwrap().unpack_str(), Some("kept"));
    assert_eq!(
        inspect_function_symbol(consumer.get("f").unwrap())
            .unwrap()
            .state(),
        FunctionSymbolStateRef::Local { ordinal: 0 }
    );
}

#[test]
fn logical_caller_depth_borrows_original_module_metadata_without_filename_fallback() {
    let producer = Module::new();
    producer
        .set_client_data_no_overwrite(producer.heap().alloc_str("producer").to_value())
        .unwrap();
    Evaluator::new(&producer)
        .eval_module(
            ast("def inner():\n    return (owner(0), owner(1), owner(2))\n"),
            &globals(),
        )
        .unwrap();
    let producer = producer.freeze().unwrap();
    let consumer = Module::new();
    consumer.import_public_symbols(&producer);
    consumer
        .set_client_data_no_overwrite(consumer.heap().alloc_str("consumer").to_value())
        .unwrap();
    Evaluator::new(&consumer)
        .eval_module(
            ast("def outer():\n    return inner()\nresult = outer()\n"),
            &globals(),
        )
        .unwrap();
    let values = TupleRef::from_value(consumer.get("result").unwrap()).unwrap();
    assert_eq!(
        values
            .iter()
            .map(|v| v.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["producer", "consumer", "none"]
    );
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct FakeFunction;
crate::starlark_simple_value!(FakeFunction);
impl fmt::Display for FakeFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fake")
    }
}
#[starlark_value(type = "function")]
impl<'v> StarlarkValue<'v> for FakeFunction {}

#[test]
fn typed_symbol_inspection_rejects_function_named_host_values() {
    let module = Module::new();
    let fake = module.heap().alloc_simple(FakeFunction);
    assert!(!is_starlark_def(fake));
    assert!(inspect_function_symbol(fake).is_none());
}
