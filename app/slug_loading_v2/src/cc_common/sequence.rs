//! Slug-native immutable-element adapter for CcStarlarkInternal.intern_seq.
//! Bazel 9.2:397-405 weak-interns representatives; that substitution is deferred.
use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::list::ListRef;
use starlark::values::range::Range;
use starlark::values::structs::starlark_structural_is_immutable;
use starlark::values::tuple::AllocTuple;
use starlark::values::tuple::TupleRef;

pub(super) fn immutable_tuple<'v>(seq: Value<'v>, heap: Heap<'v>) -> anyhow::Result<Value<'v>> {
    anyhow::ensure!(
        ListRef::from_value(seq).is_some()
            || TupleRef::from_value(seq).is_some()
            || Range::from_value(seq).is_some(),
        "intern_seq requires a sequence (list, tuple, or range)"
    );
    // Genuine sequences and the structural capability invoke no Starlark code.
    // Validate before tuple allocation; no mutation can occur between these passes.
    for value in seq.iterate(heap).map_err(starlark::Error::into_anyhow)? {
        anyhow::ensure!(
            starlark_structural_is_immutable(value),
            "intern_seq mutable elements are unsupported/deferred"
        );
    }
    Ok(heap.alloc(AllocTuple(
        seq.iterate(heap).map_err(starlark::Error::into_anyhow)?,
    )))
}

#[cfg(test)]
mod tests;
