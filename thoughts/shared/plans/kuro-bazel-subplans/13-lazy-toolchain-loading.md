# Sub-Plan 13: Lazy Toolchain Loading & dev_dependency Filtering

## Overview

Kuro currently loads ALL registered toolchain packages eagerly on every build,
regardless of whether they are needed for the target being built. This causes
builds to fail when transitive dependencies register toolchains whose
`.bzl` files reference unavailable repos (e.g., `@bazel_ci_rules`, `swiftc.exe`).

This plan addresses the three root causes:
1. `dev_dependency` flag on `register_toolchains()` and `use_extension()` is **ignored**
2. Toolchain packages from ALL transitive modules are collected, including
   dev-only registrations
3. `ensure_registered_toolchains_loaded()` eagerly loads every registered package,
   triggering unbounded transitive `.bzl` load chains

## Current State Analysis

### Evidence of the Problem (zeromatter, 2026-04-10)

Building `//sdk:all_sdk_sources` (a simple filegroup with zero external deps)
fails because:
1. `aspect_rules_lint` → `gazelle` → `go_deps` extension → reads go.mod → fails
2. `protobuf` → `rules_java` → `java_common.bzl` → `get_internal_java_common()` → fails
3. `rules_rust` → `test_extensions.bzl` → `@bazel_ci_rules` → cell not found
4. `llvm` → toolchain extension → downloads ALL platform toolchains → some fail

None of these are needed for a filegroup target.

### Root Cause 1: `dev_dependency` Ignored

**`register_toolchains()`** — `app/kuro_bzlmod/src/globals.rs:728-740`:
```rust
fn register_toolchains<'v>(
    #[starlark(args)] toolchains: UnpackTuple<&str>,
    #[starlark(require = named, default = false)] dev_dependency: bool,
    // ...
) -> starlark::Result<NoneType> {
    let _ = dev_dependency; // <-- IGNORED
```

**`use_extension()`** — `app/kuro_bzlmod/src/globals.rs:650`: records `dev_dependency`
on `ExtensionUsage`, but `aggregate_extensions()` (`app/kuro_bzlmod/src/extensions.rs:128`)
iterates all usages without filtering.

**`register_execution_platforms()`** — same pattern: `let _ = dev_dependency;`

In Bazel, `dev_dependency=True` on any of these directives means the item is
**completely ignored** when the declaring module is not the root module.

**Source**: [MODULE.bazel API reference](https://bazel.build/rules/lib/globals/module):
> "If true, this dependency will be ignored if the current module is not the root module
> or `--ignore_dev_dependency` is enabled."

### Root Cause 2: All Modules' Toolchains Collected

`app/kuro_common/src/legacy_configs/cells.rs:1114-1131`:
```rust
for (_module_name, parsed_mod) in &parsed_modules {
    all_toolchains.extend(parsed_mod.registered_toolchains.iter().cloned());
    // No filtering by which module this is or dev_dependency flag
}
```

This collects toolchains from ALL ~50 transitive deps, even those like
`aspect_rules_lint` that register `@sarif_parser_toolchains//:all` which the
user's build never needs.

### Root Cause 3: Eager Loading Triggers Unbounded Load Chains

`app/kuro_analysis/src/analysis/env.rs:386-491`:
`ensure_registered_toolchains_loaded()` calls `dice.get_interpreter_results()`
for every registered toolchain package. This loads the BUILD file, which
transitively loads all `.bzl` files via `load()` statements, which can chain
across repositories indefinitely.

For example, `@rules_rust//:all` → loads `BUILD.bazel` → loads `rust/defs.bzl`
→ loads `test/test_extensions.bzl` → loads `@bazel_ci_rules//:rbe_repo.bzl`
→ **cell not found → BUILD FAILED**.

In Bazel, this same chain exists but doesn't fail because:
1. Dev-only `register_toolchains()` from non-root modules are skipped
2. Package loading is demand-driven (only loads what the target needs)
3. The `toolchain()` wrapper rule is lightweight — the heavy implementation
   target is only loaded if the toolchain is **selected** by constraint matching

**Sources**:
- [Bazel Toolchain Resolution](https://bazel.build/extending/toolchains#toolchain-resolution):
  "Only the resolved toolchain target is actually made a dependency of the target"
- [RegisteredToolchainsFunction.java](https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/skyframe/toolchains/RegisteredToolchainsFunction.java):
  Loads `DeclaredToolchainInfo` from wrapper targets, not implementations
- [Bazel Issue #20354](https://github.com/bazelbuild/bazel/issues/20354):
  Documents unexpected toolchain loading from bzlmod modules

## Desired End State

After implementing this plan:

1. `kuro build //sdk:all_sdk_sources` succeeds on zeromatter
2. `kuro build //sdk:sdk` reaches the analysis phase (may still fail for
   other reasons, but not due to toolchain loading)
3. Dev-dependency toolchains and extensions from non-root modules are skipped
4. Toolchain package loading errors are non-fatal (stub packages created)
5. Only toolchains from the root module and its non-dev transitive deps are loaded

### Verification Criteria

- [x] `kuro build //sdk:all_sdk_sources` → BUILD SUCCEEDED (2026-04-11)
- [x] `kuro build //sdk:sdk` reaches "Error running analysis" (not loading errors) (2026-04-11)
- [x] No "Error loading ... bazel_ci_rules" messages (2026-04-11)
- [x] No `rules_java java_common.bzl` loading errors (2026-04-11)
- [x] Existing test suite passes: `pytest tests/core/analysis/ -q` (293 passed, 5 pre-existing failures, 2026-04-11)

## What We're NOT Doing

1. **Full Bazel-style lazy toolchain resolution** — Bazel only loads toolchain
   packages when a target's `toolchains` attribute requires that type. This
   requires deep DICE integration. Out of scope for this plan.
2. **Per-target toolchain filtering** — We still load all non-dev toolchains
   eagerly. This is suboptimal but functional.
3. **Extension lazy evaluation** — We don't change when extensions execute.
   We only filter which ones are collected.
4. **WORKSPACE compatibility** — Removed in Bazel 9.0, not relevant.

## Implementation Approach

Three phases, each independently valuable:

1. **Phase 1**: Filter `dev_dependency` — Skip dev-only items from non-root modules
2. **Phase 2**: Make toolchain loading resilient — Errors become warnings
3. **Phase 3**: Filter root-module-only toolchain registrations

## Phase 1: Filter `dev_dependency` at Collection Time

### Overview

Stop collecting `register_toolchains()`, `register_execution_platforms()`,
and `use_extension()` items that have `dev_dependency=True` from non-root modules.

### Changes Required

#### 1a. Track `dev_dependency` in `register_toolchains()` / `register_execution_platforms()`

**File**: `app/kuro_bzlmod/src/globals.rs`

Currently `dev_dependency` is discarded (`let _ = dev_dependency`). Instead,
store it alongside the label:

```rust
// In register_toolchains() (line 728):
fn register_toolchains<'v>(
    #[starlark(args)] toolchains: UnpackTuple<&str>,
    #[starlark(require = named, default = false)] dev_dependency: bool,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<NoneType> {
    let ctx = get_module_context(eval)?;
    let mut ctx = ctx.borrow_mut();
    for tc in toolchains.items {
        ctx.registered_toolchains.push(RegisteredItem {
            label: tc.to_owned(),
            dev_dependency,
        });
    }
    Ok(NoneType)
}
```

**File**: `app/kuro_bzlmod/src/types.rs`

Add new type and update `ParsedModuleFile`:

```rust
/// A registered toolchain or execution platform, with dev_dependency tracking.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegisteredItem {
    pub label: String,
    pub dev_dependency: bool,
}

// Update ParsedModuleFile:
pub registered_toolchains: Vec<RegisteredItem>,  // was Vec<String>
pub registered_execution_platforms: Vec<RegisteredItem>,  // was Vec<String>
```

**File**: `app/kuro_bzlmod/src/globals.rs` (ModuleContext struct, ~line 55):
Update the field types in `ModuleContext` to match.

**File**: `app/kuro_bzlmod/src/parser.rs` (~line 143):
Update the clone to use the new type.

#### 1b. Filter non-root dev_dependency items during collection

**File**: `app/kuro_common/src/legacy_configs/cells.rs` (~line 1114-1131)

```rust
let root_module_name = &parsed.module.name;
let mut all_toolchains = Vec::new();
let mut all_exec_platforms = Vec::new();
for (module_name, parsed_mod) in &parsed_modules {
    let is_root = module_name == root_module_name || module_name == "_main";
    for item in &parsed_mod.registered_toolchains {
        // Skip dev_dependency items from non-root modules (Bazel 9.0 behavior)
        if item.dev_dependency && !is_root {
            continue;
        }
        all_toolchains.push(item.label.clone());
    }
    for item in &parsed_mod.registered_execution_platforms {
        if item.dev_dependency && !is_root {
            continue;
        }
        all_exec_platforms.push(item.label.clone());
    }
}
```

#### 1c. Filter dev_dependency extension usages during aggregation

**File**: `app/kuro_bzlmod/src/extensions.rs` (`aggregate_extensions()`, ~line 128)

Add filtering:
```rust
// Skip dev_dependency usages from non-root modules
if usage.dev_dependency && module_name != root_module_name {
    continue;
}
```

This requires passing `root_module_name` to `aggregate_extensions()`.

**File**: `app/kuro_bzlmod/src/pending_repo_cells.rs` (`pre_compute_extension_repo_cells`, ~line 148)

Same filtering: skip `use_repo()` declarations from dev_dependency extensions
of non-root modules.

### Success Criteria (Phase 1)

#### Automated Verification:
- [x] `cargo build -p kuro` compiles without errors (2026-04-11)
- [x] `pytest tests/core/analysis/ -q` — 293 passed, 5 pre-existing failures (2026-04-11)
- [x] `pytest tests/core/bzlmod/ -q` — 13 passed (2026-04-11)

#### Manual Verification:
- [x] zeromatter: count of registered toolchains is significantly reduced (2026-04-11)
- [x] No `@bazel_ci_rules` cell-not-found errors (2026-04-11)

---

## Phase 2: Make Toolchain Loading Resilient

### Overview

When `ensure_registered_toolchains_loaded()` fails to load a toolchain package
(due to `.bzl` load errors, missing cells, etc.), log a warning and continue
rather than propagating the error.

### Changes Required

#### 2a. Wrap `get_interpreter_results()` errors

**File**: `app/kuro_analysis/src/analysis/env.rs` (~line 452)

The current code already has `continue` on `Err`:
```rust
let eval_result = match dice.get_interpreter_results(package_label.dupe()).await {
    Ok(r) => r,
    Err(e) => {
        tracing::debug!("Failed to load toolchain package '{}': {}", tc_label_str, e);
        continue;
    }
};
```

This looks correct, but the error may be propagating through DICE's async
computation graph rather than being caught here. The issue is that DICE
computations can produce cascading errors — when a `get_loaded_module()` fails
inside `eval_build_file()`, the error may cause the entire DICE transaction to
fail rather than being returned as an `Err`.

**Fix**: Add `catch_unwind`-style error containment around the DICE computation.
If this is not feasible in DICE's model, add a timeout and fallback:

```rust
let eval_result = match tokio::time::timeout(
    std::time::Duration::from_secs(30),
    dice.get_interpreter_results(package_label.dupe()),
).await {
    Ok(Ok(r)) => r,
    Ok(Err(e)) => {
        tracing::warn!("Toolchain package '{}' load failed: {}", tc_label_str, e);
        continue;
    }
    Err(_timeout) => {
        tracing::warn!("Toolchain package '{}' load timed out", tc_label_str);
        continue;
    }
};
```

#### 2b. Ensure DICE errors from transitive loads don't abort the build

The deeper issue is that a `.bzl` load failure (e.g., `@bazel_ci_rules` not found)
inside `eval_deps()` (`dice_calculation_delegate.rs:218-242`) returns an error
via `try_compute_join`, which propagates up the DICE stack. Even though
`ensure_registered_toolchains_loaded` catches the error, the DICE framework may
mark the transaction as poisoned.

**Investigation needed**: Check whether DICE's `try_compute_join` failure
corrupts the computation graph for subsequent requests in the same transaction.
If yes, we need to run each toolchain package load in a separate DICE scope
or use the existing error-recovery mechanism.

### Success Criteria (Phase 2)

#### Automated Verification:
- [x] `cargo build -p kuro` compiles (2026-04-11)
- [x] Existing test suite passes (no regressions, 2026-04-11)

#### Manual Verification:
- [x] zeromatter: `kuro build //sdk:all_sdk_sources` → BUILD SUCCEEDED (2026-04-11)
- [x] Failed toolchain loads produce WARN logs, not BUILD FAILED (2026-04-11)

---

## Phase 3: Filter Toolchain Registrations by Relevance (Optional)

### Overview

Further reduce the set of toolchains loaded by only including registrations from
modules that are in the transitive dependency closure of the target being built.
This is a more aggressive optimization that brings kuro closer to Bazel's model.

**Note**: This phase is optional and may not be needed if Phases 1-2 are
sufficient. Only implement if builds are still too slow or fail due to
non-essential toolchain loading.

### Approach

Instead of collecting toolchains from ALL resolved modules at startup, defer
collection to analysis time and only include modules reachable from the target's
BUILD.bazel transitive load graph.

This requires:
1. Building a "module reachability" graph during MVS resolution
2. At analysis time, computing which modules are transitively needed
3. Only loading toolchains from those modules

### Complexity Assessment

This is a significant architectural change. Bazel achieves this naturally because
its Skyframe framework is inherently demand-driven — `RegisteredToolchainsFunction`
only runs when a target requests toolchain resolution, and only considers
toolchains from modules in the resolved dep graph.

Kuro's DICE is also demand-driven, but `ensure_registered_toolchains_loaded()`
explicitly defeats this by eagerly loading everything. Moving to true lazy
loading would require:
- Removing `ensure_registered_toolchains_loaded()` entirely
- Making `get_declared_toolchains()` a DICE computation that loads on demand
- Potentially restructuring how `DeclaredToolchainInfo` is stored

**Recommendation**: Defer this phase. Phases 1-2 should be sufficient for the
zeromatter use case. If needed, revisit after confirming Phase 2 results.

### 2026-05-04 update: real-world evidence the deferral was wrong

End-to-end verification of Plan 11 Phase 8 (toolchain alias resolution) on
zeromatter `//sdk:sdk` reproduced exactly the pathology Phase 3 was
scoped to fix:

- `Waiting on multitool//toolchains -- loading package file tree` for **20+
  consecutive minutes** while the daemon serially loads the bzlmod transitive
  closure (multitool, rules_robolectric, rules_swift, rules_kotlin,
  rules_fuzzing, rules_jvm_external, rules_java, …).
- ~900 external repos materialized for an `//sdk:sdk` target whose actual
  toolchain dependency footprint is well under 50.
- Phases 1+2 successfully eliminated the *failure* mode (no more BUILD FAILED
  on irrelevant load errors), but did nothing for the *latency* mode — kuro
  still loads and materializes everything.
- Bazel building the same target finishes the analogous step in seconds,
  because Skyframe only loads toolchain packages whose modules are in the
  target's transitive closure.

Status escalated: Phase 3 is no longer optional. It's the structural fix for
real-world parity on any non-trivial bzlmod workspace. The Approach B path
(remove `ensure_registered_toolchains_loaded`, make `get_declared_toolchains`
a DICE computation, populate lazily on first `ctx.toolchains[T]` access) is
the right shape — same architecture Bazel uses.

### Phase 3 design (refined 2026-05-04)

**Minimum viable cut: track origin module, eager-load root-only, lazy fallback.**

#### Storage change

`app/kuro_bzlmod/src/lib.rs::REGISTERED_TOOLCHAINS` becomes
`Vec<RegisteredToolchain { module: String, label: String, is_root: bool }>`.

`set_registered_toolchains` takes the per-module structured list instead of a
flat `Vec<String>`. `cells.rs::~843` already iterates `parsed_modules` with
known `is_root` — pipe that through.

#### Eager-load behavior change

`app/kuro_analysis/src/analysis/env.rs::ensure_registered_toolchains_loaded`
filters `to_load` to root-module registrations only. Transitive registrations
go into a global `DEFERRED_TOOLCHAINS` pool (mirroring the existing
`DECLARED_TOOLCHAINS`).

#### Lazy fallback on resolution miss

`app/kuro_analysis/src/analysis/toolchain_resolution.rs::resolve_toolchains`
gains a fallback path: when a required mandatory `toolchain_type` produces no
match in `DECLARED_TOOLCHAINS`, call a new
`ensure_deferred_toolchains_loaded(ctx, &required_type)` which:

1. Iterates `DEFERRED_TOOLCHAINS`.
2. Filters to registrations whose label's *repo name* appears in the
   `required_type`'s repo name OR the registration's *origin module* name
   matches the type's repo name. (Heuristic: most rules_X repos register
   toolchains for their own type.)
3. Loads only the filtered subset via `dice.get_interpreter_results` (using
   the same parallel `try_compute_join` machinery as the eager path).
4. Marks loaded entries in a "loaded" set so re-queries for the same type
   don't reload.

If the heuristic still misses, fall through to "load all deferred" once. This
guarantees correctness — at worst we degrade to current Phase 1+2 behavior
for that build.

#### Correctness invariants

- **Root toolchains always loaded**: same as today.
- **Transitive toolchains only loaded when a target's resolution needs them**:
  matches Bazel.
- **Determinism**: the filter is purely a function of `(deferred set, required
  type)` — no ordering dependency.
- **No silent miss**: full-deferred fallback ensures we never claim "no
  toolchain found" if a deferred entry would have matched.

#### Touchpoints

- `app/kuro_bzlmod/src/lib.rs` — type change for `REGISTERED_TOOLCHAINS`,
  add `RegisteredToolchain` struct, update getter signature.
- `app/kuro_common/src/legacy_configs/cells.rs:843-902` — populate structured
  list with `is_root` and `module_name`.
- `app/kuro_analysis/src/analysis/env.rs::ensure_registered_toolchains_loaded`
  — filter to root-only; populate `DEFERRED_TOOLCHAINS`.
- `app/kuro_analysis/src/analysis/native_rule_analysis.rs` — add
  `DEFERRED_TOOLCHAINS` static + helpers, plus a `LOADED_DEFERRED_KEYS`
  marker set.
- `app/kuro_analysis/src/analysis/toolchain_resolution.rs::resolve_toolchains`
  — accept `&mut DiceComputations` (or via callback) and call lazy-load on
  miss.

#### Risks

- `resolve_toolchains` currently has no DICE access (pure function). Threading
  DICE through is the biggest refactor cost.
- Bundle cells (`@bazel_tools`, `@local_config_platform`) currently load
  through the same eager path — must remain in the eager set.
- The heuristic-based filter may need iteration. Start with permissive (load
  all-deferred) and tighten.

### Success Criteria (Phase 3)

#### Automated Verification:
- [x] `cargo build -p kuro` compiles (2026-05-04)
- [x] `cargo test -p kuro_bzlmod --lib` — 161 passed (2026-05-04)
- [x] `cargo test -p kuro_analysis --lib` — 11 passed (2026-05-04)

#### Manual Verification:
- [x] zeromatter `//sdk:sdk_contents` reaches analysis phase
  (2026-05-04). Wall time **~3m 20s** (18:27:45 start → 18:31:05 BUILD
  FAILED) vs. the prior 30+-min hang on
  `Waiting on multitool//toolchains -- loading package file tree`. The new
  failure is unrelated to Plan 13: an `@crates` extension repo from
  rules_rs crate_universe failed to generate, surfacing as
  `Module has no symbol all_crate_deps` while loading
  `bazel/rules/rust.bzl` — a downstream rules_rs/crate_universe issue.
- [ ] `bazel-external/` count for `//sdk:sdk_contents` drops correspondingly
  (separate measurement; this run hit the `@crates` failure before the
  full closure materialized).

### Implementation summary (2026-05-04)

Code changes per the touchpoints listed in the design section:

- `app/kuro_bzlmod/src/lib.rs` — added `RegisteredToolchain { module, label,
  is_root }`; `REGISTERED_TOOLCHAINS` is now `Vec<RegisteredToolchain>`;
  `set_registered_toolchains` / `get_registered_toolchains` updated.
- `app/kuro_common/src/legacy_configs/cells.rs` — collection loop now
  produces structured `RegisteredToolchain` entries with `is_root` derived
  from the same predicate that drives `dev_dependency` filtering. Bundled
  rules_python auto-injected entries are marked `is_root = true` so they
  remain in the eager set.
- `app/kuro_analysis/src/analysis/native_rule_analysis.rs` — added
  `DeferredToolchain`, `DEFERRED_TOOLCHAINS` pool, `LOADED_DEFERRED_KEYS`
  marker set, and `LOADED_ALL_DEFERRED` fallback flag, plus accessor
  helpers (`set_deferred_toolchains`, `get_deferred_toolchains`,
  `deferred_key_already_loaded`, `mark_deferred_key_loaded`,
  `deferred_all_loaded`, `mark_deferred_all_loaded`).
- `app/kuro_analysis/src/analysis/env.rs`:
  - `ensure_registered_toolchains_loaded` partitions the registry into
    eager (root + bundled cells via new `is_bundled_eager_toolchain`) and
    deferred subsets; only the eager set is loaded immediately, the
    deferred set is parked in `DEFERRED_TOOLCHAINS`.
  - Extracted the parallel load + register loop into
    `load_and_register_toolchain_packages` and the parse + cell-resolve
    pre-filter into `prepare_toolchain_load_list` so the new lazy path
    reuses the same machinery (and the same `canonicalize_toolchain_type_label`
    alias-following logic from Plan 11 Phase 8).
  - Added `ensure_deferred_toolchains_loaded(dice, &required_types)`:
    heuristic filter on `(origin module, label repo)` ∩ `(required type
    repo names)`, with a "load all remaining" fallback when the heuristic
    misses. Marks loaded keys in `LOADED_DEFERRED_KEYS` to avoid reload.
  - `resolve_toolchain_types` is now `async fn` taking `&mut
    DiceComputations`; first runs the pure resolution; on `Err` or any
    `None` mandatory result, calls `ensure_deferred_toolchains_loaded`
    with all required type labels and retries once.
- `examples/lazy_toolchain/` — Smoke 3 harness (MODULE.bazel + BUILD.bazel +
  src/main.rs + .buckconfig) declaring rules_rust + 7 unrelated rules
  modules, with a single `rust_binary` target.

Status: code complete. zeromatter `//sdk:sdk` end-to-end verification
deferred to a follow-up session — note any progress past the 30-min wall
back into this plan when run.

### Smoke test harness (Smoke 3)

Phase 3's verification target: `examples/lazy_toolchain/` — a focused example
that demonstrates the loading-wall pathology in miniature without requiring
zeromatter.

Layout:
```
examples/lazy_toolchain/
  MODULE.bazel
  BUILD.bazel
  src/main.rs
```

`MODULE.bazel` declares `bazel_dep` for ~8–10 rules repos that each register
their own toolchains (rules_cc, rules_rust, rules_python, rules_oci,
protobuf, rules_pkg, rules_java, rules_go), but the target uses only one
(rules_rust). Pre-Phase-3 baseline: build wall time dominated by
materialization of all bzlmod deps' toolchain repos. Post-Phase-3 target:
build wall < 60s on warm daemon, with `bazel-external/` count limited to
modules actually reachable from the rust_library closure.

Acceptance:
- `kuro build //:bin` completes in < 60s (warm daemon)
- `bazel-external/` count after build is < 50 repos (vs. ~200+ pre-Phase-3)
- `kuro build //:bin -v 5 2>&1 | grep "Skipping deferred"` shows transitive
  modules' toolchain registrations were deferred and never loaded

---

## Alternative Approaches Considered

### A. Make ALL load errors non-fatal (stub everything)

**Approach**: When any `.bzl` load fails during toolchain package evaluation,
create a stub module that exports empty symbols instead of erroring.

**Pros**: Simple to implement, handles all edge cases.
**Cons**: Masks real errors — if a `.bzl` file that IS needed fails, the user
gets confusing "symbol not found" errors instead of clear load errors. Also
doesn't reduce the number of repos that need to be materialized (still downloads
everything).

**Verdict**: Rejected as primary approach. Useful as a defense-in-depth layer.

### B. Skip toolchain loading entirely, resolve on-demand

**Approach**: Remove `ensure_registered_toolchains_loaded()` and populate the
`DeclaredToolchainInfo` registry lazily when toolchain resolution runs.

**Pros**: Matches Bazel's architecture perfectly. Only loads what's needed.
**Cons**: Requires significant refactoring of how `DeclaredToolchainInfo` is
stored and queried. The current global `DECLARED_TOOLCHAINS` registry would
need to become a DICE-managed computation.

**Verdict**: Ideal long-term solution, but too complex for the immediate need.
Could be Phase 3 or a future plan.

### C. Pre-filter by platform constraints

**Approach**: Before loading a toolchain package, check if its
`target_compatible_with` could possibly match the host platform. Skip
packages for incompatible platforms.

**Pros**: Would skip most cross-platform toolchain downloads.
**Cons**: Requires evaluating the `toolchain()` target first to read constraints,
which requires loading the BUILD file — the very thing we're trying to avoid.
Chicken-and-egg problem.

**Verdict**: Not feasible without the two-layer architecture (wrapper + impl)
that Bazel uses.

### D. Maintain an allowlist of essential toolchains

**Approach**: Only load toolchains from a hardcoded or configured list of
essential modules (e.g., `rules_cc`, `rules_rust`, `local_config_platform`).

**Pros**: Simple, predictable.
**Cons**: Breaks when users add new toolchain rules. Not maintainable.

**Verdict**: Rejected — too fragile.

## References

- [Bazel Toolchain Resolution docs](https://bazel.build/extending/toolchains#toolchain-resolution)
- [MODULE.bazel API — dev_dependency](https://bazel.build/rules/lib/globals/module)
- [Bazel Issue #20354 — Unexpected toolchain loading from bzlmod](https://github.com/bazelbuild/bazel/issues/20354)
- [Bazel Issue #8466 — Lazy loading of external repositories](https://github.com/bazelbuild/bazel/issues/8466)
- [Aspect Blog — Avoiding eager fetches](https://blog.aspect.build/avoid-eager-fetches)
- Prior kuro work: `thoughts/shared/plans/kuro-bazel-subplans/11-toolchain-resolution.md`
- Prior kuro work: `thoughts/shared/plans/kuro-bazel-subplans/12-stub-cleanup-and-exec-groups.md`
- zeromatter session: 10 commits on 2026-04-10 fixing repo-level compatibility
