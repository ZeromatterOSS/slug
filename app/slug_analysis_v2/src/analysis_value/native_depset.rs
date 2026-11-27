use std::collections::HashMap;

use compact_str::CompactString;
use dupe::Dupe;
use slug_build_api_v2::AnalysisDepset;
use slug_build_api_v2::Depset;
use slug_build_api_v2::DepsetSuccessor;
use slug_loading_v2::provider::StarlarkDepsetSuccessorGen;
use slug_loading_v2::provider::alloc_starlark_depset_parts;
use starlark::values::FrozenHeap;
use starlark::values::FrozenValue;

// Keep the source owner alive for every raw node key. Structurally equal
// independent nodes have distinct native Starlark occurrences.
pub(super) type Memo<T> = HashMap<(usize, u32), (Depset<T>, FrozenValue)>;

pub(super) fn project<T>(
    heap: &FrozenHeap,
    root: &Depset<T>,
    memo: &mut Memo<T>,
    element_type: &'static str,
    retain_empty: bool,
    mut leaf: impl FnMut(&T) -> FrozenValue,
) -> FrozenValue {
    let root_key = root.node_key();
    if let Some((_, found)) = memo.get(&root_key) {
        return *found;
    }

    // The dense row's nth is constant time. Keep one cursor per active node
    // and emit the original ordered row once all child values are available.
    let mut stack = vec![(root.dupe(), 0)];
    while let Some((current, next)) = stack.last_mut() {
        if let Some(successor) = current.successors().nth(*next) {
            *next += 1;
            if let DepsetSuccessor::Transitive(child) = successor
                && !memo.contains_key(&child.node_key())
            {
                stack.push((child, 0));
            }
            continue;
        }

        let (current, _) = stack.pop().expect("active depset frame");
        let successors = current
            .successors()
            .map(|successor| match successor {
                DepsetSuccessor::Direct(value) => StarlarkDepsetSuccessorGen::Direct(leaf(value)),
                DepsetSuccessor::Transitive(child) => StarlarkDepsetSuccessorGen::Transitive(
                    memo.get(&child.node_key())
                        .expect("child materialized first")
                        .1,
                ),
            })
            .collect();
        let empty = current.is_empty();
        let retained = (empty && retain_empty).then(|| AnalysisDepset::empty(current.order()));
        let result = alloc_starlark_depset_parts(
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
        );
        memo.insert(current.node_key(), (current, result));
    }
    memo.get(&root_key).expect("root materialized").1
}

#[cfg(test)]
#[path = "native_depset_tests.rs"]
mod tests;
