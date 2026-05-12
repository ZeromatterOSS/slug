# Starlark compilation persistence (Plan 17.6)

**Date:** 2026-04-22
**Scope:** `app/slug_interpreter/src/`, `app/slug_external_cells_bundled/`
**Verdict:** No persistent compile cache exists. Whether this matters
depends on measurement; recommend deferring a fix until the Plan 16
harness shows daemon cold-start load as a significant fraction of
cold wall.

## What's cached today

### DICE in-memory cache
`DiceComputations::get_loaded_module` is documented as "cached on the
dice graph" (`app/slug_interpreter/src/load_module.rs:58`). This
caches parsed+frozen modules for the lifetime of a DICE graph — in
practice, the lifetime of a daemon session.

Re-loading during a single daemon session: O(1) cache hit.

### Bundled-cell source files
`app/slug_external_cells_bundled/build.rs` embeds prelude and
bazel_tools source trees via `include_bytes!` / generated Rust source.
No compilation happens at build time — just raw file bytes.

### Persistent on-disk compile cache
**None.** Cross-daemon-restart: every bundled-cell `.bzl` gets
re-parsed, re-type-checked, and re-frozen. This is the gap the plan
17.6 phase names.

## When it matters

Only on daemon cold starts:
- CI builds (fresh daemon every run)
- Developer machine after `slug kill` or reboot
- After long idle timeouts that killed the daemon

Once the daemon is warm, DICE caching handles everything.

## Bazel's equivalent

Bazel persists compiled Starlark modules across invocations via its
Skyframe on-disk cache. When Bazel restarts, it reads the cache and
skips re-parsing unchanged `.bzl` files. This is measurable — Bazel
cold-start analysis on the LLVM clang target is ~0.75s, whereas
slug's is ~2.1s. Starlark compile time might be part of that gap.

## Fix sketch (deferred)

A fix would need:
1. Serialize frozen `LoadedModule`s to disk, keyed by (source hash,
   slug binary hash). Binary hash invalidates cache across version
   bumps.
2. On daemon start, check the cache before parsing. If all source
   hashes match, skip parsing.
3. Invalidate on bundled-cell source changes (already triggered by
   `touch app/slug_external_cells_bundled/build.rs` convention).

Non-trivial: FrozenModule is Starlark-specific and doesn't serialize
cleanly today. Would need a serde-style bridge.

## Recommendation

**Do not implement this yet.** Per Plan 17's discipline, every
optimization needs a before/after harness measurement. Until the
cold-start harness run shows Starlark parsing as a dominant slice of
the 1.4s cold analysis wall, this is speculative work.

If the `--cold` harness numbers from Plan 16.8 reveal Starlark
compile as a >300ms cost, open a dedicated plan phase to implement
the bridge above.

## Artifacts referenced

- `app/slug_interpreter/src/load_module.rs:58` — DICE caching docstring
- `app/slug_external_cells_bundled/build.rs:36` —
  `cargo:rerun-if-changed=prelude` (invalidates bundled data only on
  prelude changes, not unrelated source edits)
- `app/slug_external_cells_bundled/src/lib.rs` — embedded file
  catalog
