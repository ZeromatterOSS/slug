use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetSuccessor;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
use slug_loading_v2::provider::alloc_starlark_depset_parts;
use starlark::eval::ExecutionBudget;
use starlark::eval::ExecutionLimit;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;
use starlark_map::Hashed;
use starlark_map::small_map::BoundedMapError;
use starlark_map::small_map::SmallMap;

type NodeKey = (usize, u32);
type Successors = Vec<StarlarkDepsetSuccessorGen<FrozenValue>>;
// Keep the source owner alive for every raw node key. Structurally equal
// independent nodes have distinct native Starlark occurrences.
pub(super) type Memo<T> = SmallMap<NodeKey, (Depset<T>, FrozenValue)>;

struct Frame<T> {
    node: Depset<T>,
    key: Hashed<NodeKey>,
    next: usize,
}

fn step(budget: Option<&ExecutionBudget>) -> Result<(), ExecutionLimit> {
    budget.map_or(Ok(()), |budget| budget.charge(1, 0))
}

fn latch(budget: Option<&ExecutionBudget>, limit: ExecutionLimit) -> ExecutionLimit {
    budget.map_or(limit, |budget| budget.refuse(limit))
}

fn push_frame<T>(
    stack: &mut Vec<Frame<T>>,
    frame: Frame<T>,
    budget: Option<&ExecutionBudget>,
) -> Result<(), ExecutionLimit> {
    if let Some(budget) = budget {
        budget.push_vec(stack, frame)
    } else {
        stack.push(frame);
        Ok(())
    }
}

fn memo_error(budget: &ExecutionBudget, error: BoundedMapError<ExecutionLimit>) -> ExecutionLimit {
    match error {
        BoundedMapError::Operation(limit) => limit,
        BoundedMapError::SizeOverflow => budget.refuse(ExecutionLimit::AllocationBytes),
        BoundedMapError::AllocationFailed => budget.refuse(ExecutionLimit::AllocationFailed),
        BoundedMapError::RowCountMismatch => unreachable!("memo operation has one key"),
    }
}

fn lookup<T>(
    memo: &Memo<T>,
    key: &Hashed<NodeKey>,
    budget: Option<&ExecutionBudget>,
) -> Result<Option<FrozenValue>, ExecutionLimit> {
    let found = if let Some(budget) = budget {
        memo.get_hashed_bounded(
            key.hash(),
            |existing| Ok(key.key() == existing),
            |work, bytes| budget.charge(work, bytes),
        )
        .map_err(|error| memo_error(budget, error))?
    } else {
        memo.get_hashed(key.as_ref())
    };
    Ok(found.map(|(_, value)| *value))
}

fn remember<T>(
    memo: &mut Memo<T>,
    key: Hashed<NodeKey>,
    node: Depset<T>,
    value: FrozenValue,
    budget: Option<&ExecutionBudget>,
) -> Result<(), ExecutionLimit> {
    if let Some(budget) = budget {
        memo.try_insert_unique_hashed_bounded(key, (node, value), |work, bytes| {
            budget.charge(work, bytes)
        })
        .map_err(|error| memo_error(budget, error))?;
    } else {
        memo.insert_hashed_unique_unchecked(key, (node, value));
    }
    Ok(())
}

pub(super) fn project<T>(
    heap: &FrozenHeap,
    root: &Depset<T>,
    memo: &mut Memo<T>,
    element_type: &'static str,
    retain_empty: bool,
    mut leaf: impl FnMut(&T) -> FrozenValue,
) -> FrozenValue {
    try_project(
        root,
        memo,
        None,
        |value| Ok(leaf(value)),
        |current, successors| {
            let empty = current.is_empty();
            let retained = (empty && retain_empty).then(|| AnalysisDepset::empty(current.order()));
            Ok(alloc_starlark_depset_parts(
                heap,
                current.order(),
                (!empty).then(|| CompactString::new(element_type)),
                retained
                    .as_ref()
                    .map(AnalysisDepset::occurrence)
                    .unwrap_or_default(),
                retained,
                current.depth(),
                successors,
            ))
        },
    )
    .expect("ordinary depset projection has no admission callback")
}

/// Walk source nodes, admitting work and temporary storage before use.
/// Callers admit the native allocations and payload work inside both callbacks.
/// Only a fully materialized node enters the owning memo.
pub(super) fn try_project<T>(
    root: &Depset<T>,
    memo: &mut Memo<T>,
    budget: Option<&ExecutionBudget>,
    mut leaf: impl FnMut(&T) -> Result<FrozenValue, ExecutionLimit>,
    mut materialize: impl FnMut(&Depset<T>, Successors) -> Result<FrozenValue, ExecutionLimit>,
) -> Result<FrozenValue, ExecutionLimit> {
    step(budget)?;
    let root_key = Hashed::new(root.node_key());
    if let Some(found) = lookup(memo, &root_key, budget)? {
        return Ok(found);
    }

    // The dense row's nth is constant time. Keep one cursor per active node
    // and emit the original ordered row once all child values are available.
    let mut stack = Vec::new();
    push_frame(
        &mut stack,
        Frame {
            node: root.dupe(),
            key: root_key,
            next: 0,
        },
        budget,
    )?;
    loop {
        step(budget)?;
        let current = stack.last_mut().expect("active depset frame");
        if let Some(successor) = current.node.successors().nth(current.next) {
            current.next += 1;
            if let DepsetSuccessor::Transitive(child) = successor {
                let key = Hashed::new(child.node_key());
                if lookup(memo, &key, budget)?.is_none() {
                    push_frame(
                        &mut stack,
                        Frame {
                            node: child,
                            key,
                            next: 0,
                        },
                        budget,
                    )?;
                }
            }
            continue;
        }

        let Frame { node, key, .. } = stack.pop().expect("active depset frame");
        let mut input = node.successors();
        let count = input.len();
        let mut successors = if let Some(budget) = budget {
            let mut successors = Vec::new();
            budget.reserve_vec(&mut successors, count)?;
            successors
        } else {
            Vec::with_capacity(count)
        };
        for _ in 0..count {
            step(budget)?;
            successors.push(match input.next().expect("fixed successor count") {
                DepsetSuccessor::Direct(value) => StarlarkDepsetSuccessorGen::Direct(
                    leaf(value).map_err(|limit| latch(budget, limit))?,
                ),
                DepsetSuccessor::Transitive(child) => {
                    let key = Hashed::new(child.node_key());
                    StarlarkDepsetSuccessorGen::Transitive(
                        lookup(memo, &key, budget)?.expect("child materialized first"),
                    )
                }
            });
        }
        let value = materialize(&node, successors).map_err(|limit| latch(budget, limit))?;
        remember(memo, key, node, value, budget)?;
        if stack.is_empty() {
            return Ok(value);
        }
    }
}

#[cfg(test)]
#[path = "native_depset_tests.rs"]
mod tests;
