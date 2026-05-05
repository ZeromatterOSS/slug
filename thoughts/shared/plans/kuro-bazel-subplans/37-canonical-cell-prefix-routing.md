# Plan 37: Canonical-cell-prefix routing for `@@//pkg`

**Status: COMPLETE (2026-05-05)** — fix landed in `app/kuro_core/src/pattern/pattern.rs`. Follow-up `crates__yaml-rust2-0.8.1//` wait tracked separately.

## Context

Bzlmod-generated BUILDs use Bazel's *canonical apparent repository name*
syntax to refer to the main repo. For example, `rules_rs`'s
`crate_universe` writes path-dependency edges as:

```
deps = [
    "@@//lib/wirebuf",     # main repo (root cell), `//lib/wirebuf:wirebuf`
    "@crates//:foo-1.0",
]
```

Two prefix forms exist and they MEAN DIFFERENT THINGS:

- `@@//pkg`  → main module's repo (root cell). The double `@` is a
  canonical-repo-name marker; the empty name is "the main module".
- `@//pkg`   → current repo (same cell as the BUILD file).
- `//pkg`    → also current repo.
- `@<n>//`   → apparent-name `<n>` resolved via the cell alias map.
- `@@<n>//`  → canonical-name `<n>` (in kuro this is the same alias map).

## The bug

`lex_provider_pattern` in `app/kuro_core/src/pattern/pattern.rs` did:

```rust
Some((a, p)) => (Some(a.trim_start_matches('@')), p),
```

`trim_start_matches('@')` collapses `"@@"` → `""`, indistinguishable
from `"@"` → `""`. Then in `parse_target_pattern_no_validate`:

```rust
let cell = cell_alias_resolver.resolve(cell_alias.unwrap_or_default())?;
```

`CellAliasResolver::resolve("")` returns `self.current` (the cell that
owns the BUILD file currently being coerced — see
`app/kuro_core/src/cells.rs:405`).

Net effect: `@@//lib/wirebuf` written inside
`bazel-external/rules_rs+crate+crates__rstar-0.12.2-zm/BUILD.bazel`
was being coerced as `crates__rstar-0.12.2-zm//lib/wirebuf` instead of
`<root-cell>//lib/wirebuf`. That target doesn't exist in the rstar
cell, but kuro tried to load `lib/wirebuf` as a sub-package of the
materialized rstar spoke. The package walker then either span-waited
or stalled long enough that DICE displayed
`Waiting on crates__rstar-0.12.2-zm//lib/wirebuf -- loading package
file tree`.

The user-visible symptom was zeromatter's `//sdk:sdk_contents` build
hanging ~30s and dropping the daemon connection, immediately after
~550 crate spokes lazily materialized via Plan 36.

## The fix

Two-line change in `app/kuro_core/src/pattern/pattern.rs`:

1. In `lex_provider_pattern`, preserve a literal `"@@"` sentinel when
   the prefix is exactly `@@` with empty canonical name:
   ```rust
   let alias = if a == "@@" { "@@" } else { a.trim_start_matches('@') };
   ```
2. In `parse_target_pattern_no_validate`, route the sentinel to the
   root cell:
   ```rust
   let cell = if cell_alias == Some("@@") {
       cell_resolver.root_cell()
   } else {
       cell_alias_resolver.resolve(cell_alias.unwrap_or_default())?
   };
   ```

`@@<name>//pkg` continues to behave as `@<name>//pkg` (alias map
lookup), which is correct in kuro because canonical and apparent
names share the same alias resolver.

`BazelLabel::parse` (`app/kuro_build_api/src/interpreter/rule_defs/bazel_label.rs:217`)
already normalised `@@//pkg` correctly at analysis time — only the
*coerce-time* path was broken.

## Verification

- `cargo test -p kuro_bzlmod --lib` → 163/163 pass (baseline).
- `cargo test -p kuro_core --lib pattern` → all pattern tests that
  passed before still pass. `pattern::tests::test_relaxed` was
  failing before this change and continues to fail; unrelated.
- `examples/multi_package :gen_version_header` builds cleanly.
- `zeromatter //sdk:sdk_contents` no longer hangs on
  `crates__rstar-0.12.2-zm//lib/wirebuf`. Build now reaches a
  different blocker on `crates__yaml-rust2-0.8.1//` — see the
  follow-up note below.

## Follow-up: `crates__yaml-rust2-0.8.1//` wait

After the fix, zeromatter `//sdk:sdk_contents` advances further and then
sits on:

```
Waiting on crates__yaml-rust2-0.8.1// -- loading package file tree, and 7 other actions
```

This is a *different* shape: the wait is on the cell-root package
(`//`), not a non-existent sub-package. yaml-rust2's `BUILD.bazel`
references only `@crates//...` (no `@@//`), so the canonical-prefix
bug does not apply here. The cell IS materialized on disk (large
extracted tree with `documents/`, `examples/`, `tests/`, `tools/`,
`src/`).

Likely candidates for the next investigation:

1. The wait is on the *first* of N concurrent DICE keys; the actual
   blocker is one of the "and 7 other actions". Expose all pending
   keys (extend `display_event`, or add a debug subcommand to dump
   active DICE waits).
2. The package walker recursively scans the materialized cell for
   nested BUILDs and the tree is unusually large (yaml-rust2 has a
   `documents/` and `tests/` subtree with many YAML files).
3. The `block_in_place` bridge in `materialize_spoke_sync` is
   deadlocking on a small tokio runtime when many spokes are
   resolved in parallel from sync starlark code. Plan 36 phase 4 was
   meant to address this with a fully async path; not yet landed.

When picked up, start by dumping all active DICE keys when the wait
re-fires past 5s. That tells us whether the displayed key is the
real blocker or just the first one in the span list.
