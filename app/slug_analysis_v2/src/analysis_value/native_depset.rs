use std::hash::Hash;
use std::sync::atomic::AtomicUsize;

use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::AnalysisDepsetOccurrence;
use slug_build_api_v2::AnalysisDepsetSuccessor;
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetSuccessor;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
use slug_loading_v2::provider::alloc_budgeted_starlark_depset_parts;
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
// Retained depsets use AnalysisDepset's occurrence equality and hash,
// including the canonical empty occurrence supplied for each order.
pub(super) type RetainedMemo = SmallMap<AnalysisDepset, (AnalysisDepset, FrozenValue)>;

trait ProjectionSource: Dupe {
    type Key: Dupe + Eq + Hash;
    type Leaf;

    const RETAINED: bool;

    fn memo_key(&self) -> Self::Key;

    fn ordered_row(&self) -> impl ExactSizeIterator<Item = DepsetSuccessor<&Self::Leaf, Self>>;

    fn traversal_successor(&self, index: usize) -> Option<DepsetSuccessor<&Self::Leaf, Self>> {
        let mut row = self.ordered_row();
        let index = if Self::RETAINED {
            // The original retained DFS pushed children in row order onto a
            // LIFO stack. Preserve that callback order and emit rows forwards.
            row.len().checked_sub(index)?.checked_sub(1)?
        } else {
            index
        };
        row.nth(index)
    }
}

impl<T> ProjectionSource for Depset<T> {
    type Key = NodeKey;
    type Leaf = T;

    const RETAINED: bool = false;

    fn memo_key(&self) -> Self::Key {
        self.node_key()
    }

    fn ordered_row(&self) -> impl ExactSizeIterator<Item = DepsetSuccessor<&T, Self>> {
        self.successors()
    }
}

impl ProjectionSource for AnalysisDepset {
    type Key = AnalysisDepset;
    type Leaf = AnalysisValue;

    const RETAINED: bool = true;

    fn memo_key(&self) -> Self::Key {
        self.dupe()
    }

    fn ordered_row(&self) -> impl ExactSizeIterator<Item = DepsetSuccessor<&AnalysisValue, Self>> {
        self.successors().map(|successor| match successor {
            AnalysisDepsetSuccessor::Direct(leaf) => DepsetSuccessor::Direct(leaf),
            AnalysisDepsetSuccessor::Transitive(child) => DepsetSuccessor::Transitive(child),
        })
    }
}

type ProjectionMemo<N> = SmallMap<<N as ProjectionSource>::Key, (N, FrozenValue)>;

struct Frame<N: ProjectionSource> {
    node: N,
    key: Hashed<N::Key>,
    next: usize,
}

fn step(budget: Option<&ExecutionBudget>) -> Result<(), ExecutionLimit> {
    budget.map_or(Ok(()), |budget| budget.charge(1, 0))
}

fn latch(budget: Option<&ExecutionBudget>, limit: ExecutionLimit) -> ExecutionLimit {
    budget.map_or(limit, |budget| budget.refuse(limit))
}

fn push_frame<N: ProjectionSource>(
    stack: &mut Vec<Frame<N>>,
    frame: Frame<N>,
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

fn lookup<N: ProjectionSource>(
    memo: &ProjectionMemo<N>,
    key: &Hashed<N::Key>,
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

fn remember<N: ProjectionSource>(
    memo: &mut ProjectionMemo<N>,
    key: Hashed<N::Key>,
    node: N,
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
    try_project_node(
        memo,
        root,
        budget,
        |memo| memo,
        |_, value| leaf(value).map_err(|limit| latch(budget, limit)),
        |_, node, row| materialize(node, row).map_err(|limit| latch(budget, limit)),
    )
}

pub(super) fn alloc_budgeted_native_node<T>(
    heap: &FrozenHeap,
    budget: &ExecutionBudget,
    node: &Depset<T>,
    element_type: &'static str,
    retain_empty: bool,
    successors: Successors,
) -> Result<FrozenValue, ExecutionLimit> {
    let empty = node.is_empty();
    let retained = (empty && retain_empty).then(|| AnalysisDepset::empty(node.order()));
    let occurrence = if let Some(retained) = &retained {
        retained.occurrence()
    } else {
        budget.charge(1, size_of::<[AtomicUsize; 2]>())?;
        AnalysisDepsetOccurrence::new()
    };
    alloc_budgeted_starlark_depset_parts(
        heap,
        budget,
        node.order(),
        (!empty).then_some(element_type),
        occurrence,
        retained,
        node.depth(),
        successors,
    )
}

/// Project retained nodes through the caller's materializer. The leaf callback
/// may reenter projection through the same append-only memo. Budget failures
/// from walker operations convert to the caller's typed error alongside native
/// shape errors. Callbacks use the same budget and latch any resource refusals.
pub(super) fn try_project_retained<S, E: From<ExecutionLimit>>(
    state: &mut S,
    root: &AnalysisDepset,
    budget: Option<&ExecutionBudget>,
    memo: impl Fn(&mut S) -> &mut RetainedMemo,
    leaf: impl FnMut(&mut S, &AnalysisValue) -> Result<FrozenValue, E>,
    materialize: impl FnMut(&S, &AnalysisDepset, Successors) -> Result<FrozenValue, E>,
) -> Result<FrozenValue, E> {
    try_project_node(state, root, budget, memo, leaf, materialize)
}

fn try_project_node<N: ProjectionSource, S, E: From<ExecutionLimit>>(
    state: &mut S,
    root: &N,
    budget: Option<&ExecutionBudget>,
    memo: impl Fn(&mut S) -> &mut ProjectionMemo<N>,
    mut leaf: impl FnMut(&mut S, &N::Leaf) -> Result<FrozenValue, E>,
    mut materialize: impl FnMut(&S, &N, Successors) -> Result<FrozenValue, E>,
) -> Result<FrozenValue, E> {
    step(budget)?;
    let root_key = Hashed::new(root.memo_key());
    if let Some(found) = lookup(memo(state), &root_key, budget)? {
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
        if let Some(successor) = current.node.traversal_successor(current.next) {
            current.next += 1;
            if let DepsetSuccessor::Transitive(child) = successor {
                let key = Hashed::new(child.memo_key());
                if lookup(memo(state), &key, budget)?.is_none() {
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
        if N::RETAINED
            && let Some(value) = lookup(memo(state), &key, budget)?
        {
            // A child with the same retained occurrence completed this key.
            if stack.is_empty() {
                return Ok(value);
            }
            continue;
        }
        let memo_len = if N::RETAINED { memo(state).len() } else { 0 };
        let mut input = node.ordered_row();
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
                DepsetSuccessor::Direct(value) => {
                    StarlarkDepsetSuccessorGen::Direct(leaf(state, value)?)
                }
                DepsetSuccessor::Transitive(child) => {
                    let key = Hashed::new(child.memo_key());
                    StarlarkDepsetSuccessorGen::Transitive(
                        lookup(memo(state), &key, budget)?.expect("child materialized first"),
                    )
                }
            });
        }
        drop(input);
        // Leaf materialization can project nested retained values. Reuse the
        // already-published occurrence if that path completed this same key.
        if N::RETAINED
            && memo(state).len() != memo_len
            && let Some(value) = lookup(memo(state), &key, budget)?
        {
            if stack.is_empty() {
                return Ok(value);
            }
            continue;
        }
        let value = materialize(state, &node, successors)?;
        remember(memo(state), key, node, value, budget)?;
        if stack.is_empty() {
            return Ok(value);
        }
    }
}

#[cfg(test)]
#[path = "native_depset_tests.rs"]
mod tests;
