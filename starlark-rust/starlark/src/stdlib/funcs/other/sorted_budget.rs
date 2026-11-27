use std::cmp::Ordering;

use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::is_starlark_def;
use crate::values::StarlarkBigInt;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;
use crate::values::float::StarlarkFloat;

#[derive(Clone, Copy)]
struct Entry<'v> {
    value: Value<'v>,
    key: Value<'v>,
}

pub(super) fn sorted<'v>(
    input: Value<'v>,
    key: Option<Value<'v>>,
    reverse: bool,
    eval: &mut Evaluator<'v, '_, '_>,
    budget: &ExecutionBudget,
) -> crate::Result<Value<'v>> {
    let heap = eval.heap();
    let mut input = heap.bounded_iterator(input, "sorted")?;
    let mut entries = Vec::new();
    loop {
        budget.charge(1, 0)?;
        let Some(value) = input.next() else {
            break;
        };

        let required_len = entries.len() + 1;
        budget.reserve_vec(&mut entries, required_len)?;
        budget.charge(1, 0)?;
        let key = match key {
            None => value,
            Some(key) => {
                budget.charge(1, 0)?;
                if !key.vtable().starlark_value.HAS_invoke {
                    // The default invocation would report this error after adding the
                    // noncallable value's potentially expensive representation to a frame.
                    return ValueError::unsupported_type(key, "call()");
                }
                if !is_starlark_def(key) {
                    return Err(budget
                        .refuse(ExecutionLimit::UnsupportedOperation {
                            operation: "sorted key callback",
                            value_type: key.get_type(),
                        })
                        .into());
                }
                let result = key.invoke_pos(&[value], eval);
                budget.check()?;
                result?
            }
        };
        entries.push(Entry { value, key });
    }

    let entries = stable_sort(entries, reverse, budget)?;
    Ok(heap.alloc_budgeted_list(entries.into_iter().map(|entry| entry.value))?)
}

fn compare<'v>(
    left: Value<'v>,
    right: Value<'v>,
    budget: &ExecutionBudget,
) -> crate::Result<Ordering> {
    budget.charge(1, 0)?;
    if let Some(left_str) = left.unpack_str() {
        if let Some(right_str) = right.unpack_str() {
            budget.charge_text_work(left_str.len().min(right_str.len()))?;
        }
    } else if left.unpack_inline_int().is_some() || left.downcast_ref::<StarlarkFloat>().is_some() {
        if right.downcast_ref::<StarlarkBigInt>().is_some() {
            return Err(unsupported_compare(right, budget));
        }
    } else if left.unpack_bool().is_none() && left.vtable().starlark_value.HAS_compare {
        return Err(unsupported_compare(left, budget));
    }
    left.compare(right)
}

fn unsupported_compare(value: Value<'_>, budget: &ExecutionBudget) -> crate::Error {
    budget
        .refuse(ExecutionLimit::UnsupportedOperation {
            operation: "sorted comparison",
            value_type: value.get_type(),
        })
        .into()
}

fn stable_sort<'v>(
    mut entries: Vec<Entry<'v>>,
    reverse: bool,
    budget: &ExecutionBudget,
) -> crate::Result<Vec<Entry<'v>>> {
    if entries.len() < 2 {
        return Ok(entries);
    }

    // Each pass merges adjacent sorted runs into one scratch allocation. A comparison
    // or copy refusal stops immediately, without performing more sorting work.
    let mut scratch = Vec::new();
    budget.reserve_vec(&mut scratch, entries.len())?;
    let right_first = if reverse {
        Ordering::Greater
    } else {
        Ordering::Less
    };
    let mut width = 1;
    while width < entries.len() {
        scratch.clear();
        let merged_width = width.saturating_mul(2);
        for run in entries.chunks(merged_width) {
            let (left, right) = run.split_at(width.min(run.len()));
            let mut left = left.iter().copied().peekable();
            let mut right = right.iter().copied().peekable();
            while let (Some(left_entry), Some(right_entry)) = (left.peek(), right.peek()) {
                // Take the left entry on equality, including in reverse order.
                let entry = if compare(right_entry.key, left_entry.key, budget)? == right_first {
                    right.next().unwrap()
                } else {
                    left.next().unwrap()
                };
                budget.push_vec(&mut scratch, entry)?;
            }
            for entry in left.chain(right) {
                budget.push_vec(&mut scratch, entry)?;
            }
        }
        std::mem::swap(&mut entries, &mut scratch);
        width = merged_width;
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;
    use std::sync::Arc;

    use super::*;
    use crate::environment::Globals;
    use crate::environment::Module;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::list::ListRef;

    fn evaluate<'v>(module: &'v Module, source: &str) -> crate::Result<Value<'v>> {
        let ast = AstModule::parse("sorted.star", source.to_owned(), &Dialect::Standard).unwrap();
        Evaluator::new(module).eval_module(ast, &Globals::standard())
    }

    #[test]
    fn sorted_refuses_native_key_work_before_invocation() {
        let budget = Arc::new(ExecutionBudget::new(1000, 1000, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let source = "sorted([range(16)], key=list)";
        assert!(evaluate(&Module::new(), source).is_ok());
        assert!(evaluate(&module, source).is_err());
        assert!(matches!(
            budget.check(),
            Err(ExecutionLimit::UnsupportedOperation {
                operation: "sorted key callback",
                ..
            })
        ));
    }

    #[test]
    fn sorted_budget_preserves_stable_order_and_key_calls() {
        let source = r#"
seen = []
def by_length(value):
    seen.append(value)
    return len(value)

values = ["β", "alpha", "雪", "bar", "cat", "z", "sky"]
asc = sorted(values, key=by_length)
desc = sorted(values, key=by_length, reverse=True)
(asc, desc, seen, sorted(values), sorted([3, -2.5, 1, 1.0]), sorted([True, False, True]))
"#;
        let budget = Arc::new(ExecutionBudget::new(4096, 8192, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let ordinary = Module::new();
        let result = evaluate(&module, source).unwrap();
        assert_eq!(
            result.to_repr(),
            evaluate(&ordinary, source).unwrap().to_repr()
        );
        assert_eq!(budget.check(), Ok(()));
    }

    #[test]
    fn sorted_budget_checks_pair_capacity_before_invoking_the_key() {
        let budget = Arc::new(ExecutionBudget::new(1000, 0, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let seen = module.heap().alloc_list(&[]);
        module.set("seen", seen);
        module.set("input", module.heap().alloc_list(&[module.heap().alloc(3)]));
        assert!(evaluate(
            &module,
            "def record(value):\n    seen.append(value)\n    return value\nsorted(input, key=record)",
        )
        .is_err());
        assert!(ListRef::from_value(seen).unwrap().is_empty());
        assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
    }

    #[test]
    fn sorted_budget_checks_merge_scratch_and_final_heap_before_allocating() {
        let entry_bytes = size_of::<Entry<'_>>();
        // Two entries grow the pair vector to capacities one and two. Merging needs
        // a separate two-entry vector before the final list and its array are built.
        for bytes in [3 * entry_bytes, 5 * entry_bytes] {
            let budget = Arc::new(ExecutionBudget::new(1000, bytes, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            let heap = module.heap();
            module.set("input", heap.alloc_list(&[heap.alloc(2), heap.alloc(1)]));
            let before = heap.allocated_bytes();
            assert!(evaluate(&module, "sorted(input)").is_err());
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
            assert_eq!(heap.allocated_bytes(), before);
        }
    }

    #[test]
    fn sorted_budget_admits_comparison_work_and_refuses_unmodeled_paths() {
        let budget = Arc::new(ExecutionBudget::new(64, 4096, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let heap = module.heap();
        let prefix = "x".repeat(32 * 1024);
        let input = heap.alloc_list(&[heap.alloc(format!("{prefix}z")), heap.alloc(prefix)]);
        module.set("input", input);
        assert!(evaluate(&module, "sorted(input)").is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::Work));

        for (source, operation, value_type) in [
            ("sorted('abc'.codepoints())", "sorted", "iterator"),
            ("sorted([[1], [0]])", "sorted comparison", "list"),
        ] {
            let budget = Arc::new(ExecutionBudget::new(1000, 4096, 16));
            let module = Module::new_with_execution_budget(budget.clone());
            assert!(evaluate(&module, source).is_err());
            assert_eq!(
                budget.check(),
                Err(ExecutionLimit::UnsupportedOperation {
                    operation,
                    value_type
                }),
            );
        }
    }

    #[test]
    fn sorted_budget_keeps_noncallable_keys_an_ordinary_error() {
        let budget = Arc::new(ExecutionBudget::new(1000, 4096, 16));
        let module = Module::new_with_execution_budget(budget.clone());
        let ordinary = Module::new();
        assert_eq!(
            evaluate(&module, "sorted([], key=1)").unwrap().to_repr(),
            "[]"
        );
        let source = "sorted([1], key=1)";
        assert_eq!(
            evaluate(&module, source)
                .unwrap_err()
                .without_diagnostic()
                .to_string(),
            evaluate(&ordinary, source)
                .unwrap_err()
                .without_diagnostic()
                .to_string(),
        );
        assert_eq!(budget.check(), Ok(()));
    }
}
