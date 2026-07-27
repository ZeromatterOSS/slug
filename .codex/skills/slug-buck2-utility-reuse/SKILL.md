---
name: slug-buck2-utility-reuse
description: Preserve Buck2-derived hot-path utility reuse in Slug V2. Use when a change creates or alters retained data structures, hashing, compact collections/strings, interning, clone cost, graph storage, or memory accounting. Do not trigger for ordinary CLI, formatter, protocol, test, or call-flow work over unchanged representations.
---

# Slug Buck2 Utility Reuse

Use the live checkout as authority, then compare against `/var/mnt/dev/buck2` or the `slug-v1-archive` ref. Keep the V2 clean-root boundary: import or wrap bounded utilities intentionally; do not restore the old Buck2 tree wholesale.

## Workflow

1. Read `AGENTS.md`, the active owner section, and only the matching row in
   `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md`.
2. Check `git status --short --branch` and avoid trampling unrelated staged clean-root work.
3. For changed retained hot-path Rust only, scan for
   `std::collections::{HashMap, HashSet}`, `String`, `Vec`, repeated `clone()`,
   and newly invented interner/cache code. Skip this skill when storage and
   representation are unchanged.
4. Prefer the Buck2 utility below when its semantics match. If importing from V1 or Buck2-derived code, record the source, import mode, oracle, validation, and residual risk in the Stage 9 ledger.
5. Validate with focused tests plus `scripts/v2_archive_status.sh` and `git diff --check` when touching Slug V2.

## Utility Checklist

- Fast hashers: keep `FxHashMap`/`FxHashSet` for DICE and traversal-style hot sets, and consider a Slug wrapper matching Buck2's `BuckHasher`/`BuckHasherBuilder` when a stable project-default fast hasher is useful. Source anchors: `/var/mnt/dev/buck2/dice/dice/src/lib.rs`, `/var/mnt/dev/buck2/app/buck2_util/src/hash.rs`, `/var/mnt/dev/buck2/starlark-rust/starlark_map/src/hasher.rs`.
- Compact deterministic maps/sets: use `starlark_map::{small_map::SmallMap, small_set::SmallSet, ordered_map::OrderedMap, sorted_map::SortedMap, sorted_vec::SortedVec}` for small attr/provider/Starlark-style maps where deterministic order and low memory matter. Avoid replacing them with `BTreeMap` unless sorted ordering is a correctness oracle requirement. Source anchors: `/var/mnt/dev/buck2/starlark-rust/starlark_map/src/small_map.rs`, `small_set.rs`, `ordered_map.rs`, `sorted_map.rs`, `sorted_vec.rs`.
- Precomputed hash wrappers: use `starlark_map::Hashed` or `gazebo::hash::Hashed` when repeated lookups would otherwise recompute key hashes, especially dict/set/Starlark value paths. Source anchors: `/var/mnt/dev/buck2/starlark-rust/starlark_map/src/hashed.rs`, `/var/mnt/dev/buck2/gazebo/gazebo/src/hash.rs`.
- String and slice sharing: keep `ArcStr`, `ThinArcStr`, `ArcSlice`, `ThinArcSlice`, and `ThinBoxSlice` patterns for duplicated labels, attrs, package names, path segments, and compact immutable slices. Do not promote these back to owned `String`/`Vec` in shared graph data without a measured reason. Source anchors: `/var/mnt/dev/buck2/app/buck2_util/src/arc_str.rs`, `arc_str/fat.rs`, `arc_str/thin.rs`, `arc_str/slice.rs`, `arc_str/thin_slice.rs`, `/var/mnt/dev/buck2/app/buck2_util/src/thin_box.rs`.
- Interners: retain or re-create bounded interners for repeated labels, coerced attrs, Starlark strings, static keys, and shared directories. Buck2's important patterns are hash-first lookup, `hashbrown::HashTable` raw entry use, sharded lock-free tables for global interners, and weak-entry cleanup for directory sharing. Source anchors: `/var/mnt/dev/buck2/app/buck2_core/src/target/label/interner.rs`, `/var/mnt/dev/buck2/app/buck2_interpreter_for_build/src/attrs/coerce/interner.rs`, `arc_str_interner.rs`, `ctx.rs`, `/var/mnt/dev/buck2/starlark-rust/starlark/src/values/types/string/intern/interner.rs`, `/var/mnt/dev/buck2/shed/static_interner/src/lib.rs`, `/var/mnt/dev/buck2/app/buck2_directory/src/directory/dashmap_directory_interner.rs`.
- Cheap clone signaling: use `dupe::Dupe` for pointer-sized or constant-time clone values so reviews can distinguish cheap ref bumps from expensive deep clones. Source anchor: `/var/mnt/dev/buck2/gazebo/dupe/src/lib.rs`.
- Memory accounting: derive or preserve `allocative::Allocative` on long-lived graph, DICE, label, provider, and cache structs so V2 can keep Buck2-style memory diagnostics. Source anchor: `/var/mnt/dev/buck2/allocative/README.md`.
- Strong structural hashing: use `strong_hash::StrongHash` plus a strong hasher for digest/equality boundaries where a weak precomputed hash is not acceptable. Do not use `Hashed`'s weak hash as a content identity. Source anchors: `/var/mnt/dev/buck2/gazebo/strong_hash/src/lib.rs`, `/var/mnt/dev/buck2/app/buck2_util/src/strong_hasher.rs`.
- Temporary allocation buffers: consider Starlark's `Alloca` pattern only for tight evaluator/compiler loops that need short-lived scratch slices. Do not generalize it into ordinary application code. Source anchor: `/var/mnt/dev/buck2/starlark-rust/starlark/src/collections/alloca.rs`.

## Review Heuristics

- Prefer deterministic containers for output-facing order, but use compact ordered structures before reaching for `BTreeMap` in every internal path.
- Avoid new global interners unless lifetime and cleanup are clear; prefer evaluation-scoped interners for attrs and parsing, global interners for canonical labels/static identities, and weak interning for shareable directory DAGs.
- Keep wrappers V2-owned. A clean-root import can copy/adapt the small utility and tests; it should not reintroduce unrelated Buck2 workspace membership.
- When performance intent is unproven, add an oracle or microtest that verifies semantic identity first, then leave benchmarking as a named residual instead of claiming parity.
