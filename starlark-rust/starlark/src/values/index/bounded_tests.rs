use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;

use crate as starlark;
use crate::collections::StarlarkHasher;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::private::Private;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::ProvidesStaticType;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::AllocDict;
use crate::values::list::AllocList;
use crate::values::starlark_value;

fn eval<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
    let ast = AstModule::parse("index.star", source.to_owned(), &Dialect::Standard).unwrap();
    Evaluator::new(module).eval_module(ast, &Globals::standard())
}

fn budget(work: usize, bytes: usize) -> Arc<ExecutionBudget> {
    Arc::new(ExecutionBudget::new(work, bytes, 16).with_bounded_operations())
}

#[test]
fn ordinary_frozen_two_level_index_reads_the_original_value_under_budget() {
    let source = Module::new();
    eval(
        &source,
        "def lookup(available, keys): return available[keys[0]]",
    )
    .unwrap();
    let source = source.freeze().unwrap();

    let budget = budget(4096, 16_384);
    let caller = Module::new_with_execution_budget(budget.clone());
    let heap = caller.heap();
    let map_key = caller.frozen_heap().alloc("canonical%CrateInfo");
    let stored_value = caller.frozen_heap().alloc("original provider field");
    let table = caller
        .frozen_heap()
        .alloc(AllocDict([(map_key, stored_value)]))
        .to_value();
    let query_key = heap.alloc("canonical%CrateInfo");
    assert!(!map_key.to_value().ptr_eq(query_key));
    let keys = heap.alloc(AllocList([query_key]));
    let lookup = source
        .get("lookup")
        .unwrap()
        .owned_value(caller.frozen_heap());
    let answer = Evaluator::new(&caller)
        .eval_function(lookup, &[table, keys], &[])
        .unwrap();
    assert!(answer.ptr_eq(stored_value.to_value()));
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_index_preserves_sequence_normalization_and_modeled_errors() {
    let ordinary = Module::new();
    let budget = budget(50_000, 1 << 20);
    let bounded = Module::new_with_execution_budget(budget.clone());
    for module in [&ordinary, &bounded] {
        let heap = module.heap();
        module.set("items", heap.alloc(AllocList([13, 21])));
        module.set("pair", heap.alloc((34, 55)));
        module.set("table", heap.alloc(AllocDict([("雪", 89)])));
        for (source, expected) in [("items[-1]", 21), ("pair[-2]", 34), ("table['雪']", 89)] {
            assert_eq!(eval(module, source).unwrap().unpack_i32(), Some(expected));
        }
    }
    for source in [
        "items['invalid']",
        "items[-5]",
        "pair[2]",
        "table['missing雪']",
    ] {
        let ordinary_error = eval(&ordinary, source).unwrap_err();
        let bounded_error = eval(&bounded, source).unwrap_err();
        assert_eq!(
            ordinary_error.without_diagnostic().to_string(),
            bounded_error.without_diagnostic().to_string(),
            "{source}",
        );
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_dict_probe_admits_each_real_collision_with_no_allocation() {
    // These pairs have the same full Starlark hash under pinned FxHasher64.
    #[cfg(target_endian = "little")]
    let (first, second) = ("key00006", "key0000a");
    #[cfg(target_endian = "big")]
    let (first, second) = ("voyWdJCo", "yxBMlJCo");

    for extra in [0, 32] {
        // Dispatch, key admission, hash, whole ordered row scan, two comparisons.
        let exact_work = 7 + extra;
        for work in [exact_work - 1, exact_work] {
            let budget = budget(work, 0);
            let module = Module::new_with_execution_budget(budget.clone());
            let heap = module.heap();
            let first = heap.alloc(first);
            let second = heap.alloc(second);
            assert_eq!(
                first.get_hashed().unwrap().hash(),
                second.get_hashed().unwrap().hash()
            );
            let table = heap.alloc(AllocDict(
                [(first, heap.alloc(17)), (second, heap.alloc(31))]
                    .into_iter()
                    .chain((0..extra).map(|i| (heap.alloc(i as i32), Value::new_none()))),
            ));
            let before = heap.allocated_bytes();
            let result = table.at(second, heap);
            if work == exact_work {
                assert_eq!(result.unwrap().unpack_i32(), Some(31));
                assert_eq!(budget.check(), Ok(()));
                assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::Work));
            } else {
                assert!(result.is_err());
                assert_eq!(budget.check(), Err(ExecutionLimit::Work));
            }
            assert_eq!(heap.allocated_bytes(), before);
        }
    }
}

#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("index_trap")]
struct IndexTrap;

starlark::starlark_simple_value!(IndexTrap);

#[starlark_value(type = "index_trap")]
impl<'v> StarlarkValue<'v> for IndexTrap {
    fn at(&self, _: Value<'v>, _: Heap<'v>) -> crate::Result<Value<'v>> {
        panic!("unadmitted index hook was called")
    }

    fn at2(&self, _: Value<'v>, _: Value<'v>, _: Heap<'v>, _: Private) -> crate::Result<Value<'v>> {
        panic!("unadmitted two-argument index hook was called")
    }

    fn write_hash(&self, _: &mut StarlarkHasher) -> crate::Result<()> {
        panic!("unadmitted dictionary key hash was called")
    }
}

#[test]
fn closed_sequence_index_errors_do_not_render_invalid_values() {
    let budget = Arc::new(ExecutionBudget::new(100, 512, 16).with_closed_formatter_bytecode());
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let items = heap.alloc(AllocList([1]));
    for (index, message) in [
        (heap.alloc(IndexTrap), "got `index_trap`"),
        (heap.alloc("雪".repeat(4096)), "got `string`"),
        (heap.alloc(4_294_967_296i64), "Integer overflow"),
    ] {
        let error = items.at(index, heap).unwrap_err();
        let diagnostic = error.without_diagnostic().to_string();
        assert!(diagnostic.contains(message), "{diagnostic}");
        assert_eq!(budget.check(), Ok(()));
    }
    assert_eq!(items.at(heap.alloc(0), heap).unwrap().unpack_i32(), Some(1));
}

#[test]
fn strict_indexing_refuses_native_hooks_and_bigint_formatting() {
    let unsupported = ExecutionLimit::UnsupportedOperation {
        operation: "indexing",
        value_type: "index_trap",
    };
    for (source, expected) in [
        ("trap[0]", unsupported),
        ("trap[0, 1]", unsupported),
        ("table[trap]", unsupported),
        ("items[huge]", ExecutionLimit::UnsupportedText("int")),
    ] {
        let budget = budget(4096, 16_384);
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        module.set("trap", heap.alloc(IndexTrap));
        module.set("table", heap.alloc(AllocDict([("secret", 1)])));
        module.set("items", heap.alloc(AllocList([1])));
        module.set("huge", heap.alloc(4_294_967_296i64));
        let error = eval(&module, source).unwrap_err();
        assert!(matches!(
            error.kind(),
            crate::ErrorKind::Other(cause) if cause.downcast_ref::<ExecutionLimit>() == Some(&expected)
        ));
        assert_eq!(budget.check(), Err(expected), "{source}");
    }
}
