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

- [ ] `kuro build //sdk:all_sdk_sources` → BUILD SUCCEEDED
- [ ] `kuro build //sdk:sdk` reaches "Error running analysis" (not loading errors)
- [ ] No "Error loading ... bazel_ci_rules" messages
- [ ] No `rules_java java_common.bzl` loading errors
- [ ] Existing test suite passes: `pytest tests/core/analysis/ -q`

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
- [ ] `cargo build -p kuro` compiles without errors
- [ ] `pytest tests/core/analysis/ -q` — existing tests pass
- [ ] `pytest tests/core/bzlmod/ -q` — existing tests pass

#### Manual Verification:
- [ ] zeromatter: count of registered toolchains is significantly reduced
      (check log output: "Collected N toolchain registration(s)")
- [ ] No `@bazel_ci_rules` cell-not-found errors (this was a dev-only dep)

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
- [ ] `cargo build -p kuro` compiles
- [ ] Existing test suite passes

#### Manual Verification:
- [ ] zeromatter: `kuro build //sdk:all_sdk_sources` → BUILD SUCCEEDED
- [ ] Failed toolchain loads produce WARN logs, not BUILD FAILED

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

### Success Criteria (Phase 3)

#### Automated Verification:
- [ ] `cargo build -p kuro` compiles
- [ ] All tests pass
- [ ] `kuro build //sdk:all_sdk_sources` completes in <30 seconds (no unnecessary downloads)

#### Manual Verification:
- [ ] `kuro build //sdk:sdk` reaches analysis phase

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
