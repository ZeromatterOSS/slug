# Toolchain Resolution: Replace Stubs with Real Bazel Algorithm

> **Main Plan**: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)

## Overview

Replace the entire `ToolchainsStub` system (30+ stub types, 3000+ lines of hardcoded
tool detection) with Bazel's real toolchain resolution algorithm. This is the last major
piece of stub behavior preventing fully Bazel-compatible builds.

Currently, `ctx.toolchains[TYPE]` returns hardcoded stubs that guess compiler paths
from the host system at analysis time. Real Bazel resolves toolchains by matching
`constraint_value`s against registered `toolchain()` targets, which are created by
module extensions (`cc_configure_extension`, `rust.toolchain`, etc.) that probe the
host and generate proper toolchain definitions.

## Current State Analysis

### What Exists (Working)

- `constraint_setting()`, `constraint_value()`, `platform()` native rules — analyzed,
  providers created (`ConstraintSettingInfo`, `ConstraintValueInfo`, `ConfigurationInfo`)
- `toolchain_type()` native rule — analyzed, returns minimal providers
- `toolchain()` native rule — parsed and analyzed, but providers are empty stubs
- `register_toolchains()` in MODULE.bazel — parsed but discarded (no-op at `globals.rs:724`)
- `register_execution_platforms()` — parsed but discarded
- Module extension execution works (Plan 10) — `cc_configure_extension`, `rust.toolchain`,
  `rules_python` toolchain extensions CAN execute and create real toolchain repos
- `@local_config_platform//:host` — auto-detected host platform with correct constraints

### What's Stubbed (To Replace)

**Core dispatch** (`context.rs:2019`):
- `ToolchainsStub` — `ctx.toolchains[TYPE]` always returns `true` for `in` checks,
  dispatches on label string to hardcoded per-language stubs

**Per-language stubs** (all in `context.rs`):
- `CcToolchainInfoStub` (line 2138) — hardcoded `/usr/bin/gcc` etc.
- `RustToolchainInfoStub` (line 4003) — probes filesystem for `rustc`
- `PyToolchainInfoStub` (line 4424) — hardcoded `/usr/bin/python3`
- `JavaToolchainInfoStub` (line 4553) — hardcoded `/usr/lib/jvm`
- `OciCraneToolchainStub` (line 5025) — probes `which crane`
- `JqToolchainStub` (line 5151) — probes `which jq`
- `GenericToolchainStub` (line 5185) — fallback returning `/bin/true`

**Hardcoded tool detection functions** (`context.rs`):
- `host_tool_path()` (line 1748) — maps tool names to `/usr/bin/*`
- `host_cc_path()` (line 1735) — `/usr/bin/gcc` or `/usr/bin/clang`
- `detect_rust_tool_path()` (line 4276) — probes `~/.cargo/bin/*`
- `detect_crane_path()` (line 4778) — `which crane`
- `detect_jq_path()` (line 4788) — `which jq`

**Exec groups** (`context.rs:3723`):
- `ExecGroupsDict` / `ExecGroupToolchains` — delegates to `ToolchainsStub`

### The Bazel Toolchain Resolution Algorithm

In Bazel, toolchain resolution runs during the analysis phase, BEFORE the rule
implementation function executes. For each `(target, configuration)` pair:

1. Collect the target's required `toolchain_type`s (from `rule(toolchains=[...])`)
2. Collect registered toolchains in priority order (see Priority section below)
3. Collect available execution platforms in priority order
4. For each execution platform, find the first compatible toolchain for each type
5. Select the first execution platform that satisfies ALL mandatory toolchain types
6. The selected toolchains become implicit deps; `ctx.toolchains[TYPE]` returns
   their `ToolchainInfo` provider

**Constraint matching**: A toolchain's `exec_compatible_with` / `target_compatible_with`
matches a platform if for every constraint_value in the list, the platform has that
same constraint_value for its constraint_setting (or the setting's default matches).

**Priority ordering** (highest to lowest):
1. `--extra_toolchains` flags (last flag = highest priority)
2. Root module's `register_toolchains()` calls (in order)
3. Non-root modules' `register_toolchains()` calls (BFS order of dep graph)
4. Automatically registered toolchains

## Desired End State

After implementation:
- `ctx.toolchains[TYPE]` returns real `ToolchainInfo` providers from analyzed
  `toolchain()` targets, not stubs
- `register_toolchains()` in MODULE.bazel is wired to the resolution algorithm
- Module extensions that create toolchain repos (`local_config_cc`, `rust_toolchains`,
  `pythons_hub`) are materialized on demand when toolchain resolution needs them
- The `ToolchainsStub` and all 30+ per-language stubs are deleted
- Exec groups perform independent resolution per group
- `--extra_toolchains` and `--extra_execution_platforms` flags work

## What We're NOT Doing

1. **Remote execution platform selection** — local execution only for now
2. **Split transitions** — `cfg = "exec"` and `cfg = "target"` on toolchain attrs
   (we'll use the simpler "all deps use target config" approach initially)
3. **`target_settings` on toolchain()** — config_setting filtering (can be added later)
4. **`--toolchain_resolution_debug`** — debug output (nice to have, not blocking)
5. **Incremental toolchain resolution caching** — resolution runs fresh per build
   (DICE caching handles cross-invocation caching naturally)

## Phase 1: Collect Toolchain Registrations

### Overview
Parse and store `register_toolchains()` and `register_execution_platforms()` from
MODULE.bazel files. Currently these are no-ops.

### Changes Required

#### 1. Add registration storage to ParsedModuleFile
**File**: `app/kuro_bzlmod/src/types.rs`

Add fields to `ParsedModuleFile`:
```rust
pub struct ParsedModuleFile {
    // ... existing fields ...
    pub registered_toolchains: Vec<String>,
    pub registered_execution_platforms: Vec<String>,
}
```

#### 2. Record registrations in globals.rs
**File**: `app/kuro_bzlmod/src/globals.rs`

Replace the no-op `register_toolchains()` with code that stores the labels:
```rust
fn register_toolchains<'v>(
    #[starlark(args)] toolchains: UnpackTuple<&str>,
    #[starlark(require = named, default = false)] dev_dependency: bool,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<NoneType> {
    let internals = ModuleFileInternals::from_context(eval)?;
    for tc in toolchains.items {
        internals.registered_toolchains.push(tc.to_owned());
    }
    Ok(NoneType)
}
```

Same for `register_execution_platforms()`.

#### 3. Build priority-ordered lists in cells.rs
**File**: `app/kuro_common/src/legacy_configs/cells.rs`

After bzlmod resolution, collect all `register_toolchains()` labels in BFS order
of the module dependency graph. Store as a global ordered list accessible during
analysis.

#### 4. Handle `--extra_toolchains` / `--extra_execution_platforms` CLI flags
**File**: `app/kuro_client/src/args.rs` or equivalent

These already exist as accepted-but-ignored flags. Wire them to prepend to the
toolchain/platform registration lists.

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes
- [x] `register_toolchains("@foo//:all")` in MODULE.bazel is stored (not discarded)
- [x] Registration order matches Bazel: root module first, then BFS order of deps

---

## Phase 2: Analyze toolchain() and toolchain_type() Properly

### Overview
Make `toolchain()` and `toolchain_type()` native rules produce real providers with
the constraint information needed for resolution.

### Changes Required

#### 1. toolchain() analysis must store constraint info
**File**: `app/kuro_analysis/src/analysis/native_rule_analysis.rs`

The `toolchain()` rule has these attributes:
- `toolchain_type` — label of the toolchain_type
- `exec_compatible_with` — list of constraint_value labels
- `target_compatible_with` — list of constraint_value labels
- `toolchain` — label of the actual toolchain implementation
- `target_settings` — list of config_setting labels

Analysis of `toolchain()` must:
1. Resolve `toolchain_type` to get the type label
2. Store `exec_compatible_with` and `target_compatible_with` constraint_value labels
3. Store the `toolchain` implementation label
4. Return a `ToolchainInfo`-like provider with all this metadata

#### 2. Store resolved constraint_values from platform() analysis
The `platform()` rule's analysis already stores constraint_values. Ensure the
`ConfigurationInfo` provider from platform analysis is accessible during resolution.

### Success Criteria

#### Automated Verification:
- [x] `toolchain()` targets analyzed with real constraint info
- [x] `toolchain_type()` targets return proper type identifiers

---

## Phase 3: Implement the Resolution Algorithm

### Overview
The core: given a target's required toolchain types, the target platform, registered
toolchains, and registered execution platforms, select the matching toolchains and
execution platform.

### Changes Required

#### 1. Create ToolchainResolutionKey DICE key
**File**: `app/kuro_analysis/src/analysis/toolchain_resolution.rs` (new file)

```rust
/// DICE key for toolchain resolution.
/// Inputs: required types, target platform, current configuration.
/// Output: selected execution platform + resolved toolchain labels.
struct ToolchainResolutionKey {
    required_types: Vec<(TargetLabel, bool)>, // (type_label, mandatory)
    target_platform: TargetLabel,
    exec_constraints: Vec<TargetLabel>, // from target's exec_compatible_with
}
```

#### 2. Implement constraint matching
```rust
fn constraint_matches(
    required: &[TargetLabel],     // constraint_values
    platform: &ConfigurationInfo, // platform's constraint_values
) -> bool {
    // For each required constraint_value, check platform has it
}
```

#### 3. Implement the resolution loop
Follow the algorithm from the Bazel spec:
1. Filter exec platforms by target's exec_compatible_with
2. Filter toolchains by target_settings (config_setting matching)
3. For each exec platform × toolchain type: find first compatible toolchain
4. Eliminate platforms missing mandatory types
5. Select first (highest priority) valid platform

#### 4. Cache resolution results via DICE
The resolution result is keyed by `(required_types, target_platform, configuration)`.
DICE handles caching automatically.

### Success Criteria

#### Automated Verification:
- [x] Resolution selects correct toolchain for simple single-type case
- [x] Resolution correctly eliminates incompatible exec platforms
- [x] Optional toolchains return None when unmatched

---

## Phase 4: Wire Resolution into Analysis

### Overview
Replace `ToolchainsStub` with real resolution results in `ctx.toolchains`.

### Changes Required

#### 1. Run resolution before rule implementation
**File**: `app/kuro_analysis/src/analysis/calculation.rs`

Before calling the rule's implementation function:
1. Get the rule's declared `toolchains = [...]` from the rule definition
2. Run `ToolchainResolutionKey::compute()` via DICE
3. For each resolved toolchain label, analyze the toolchain impl target
4. Collect `ToolchainInfo` providers from each resolved toolchain
5. Build a `ResolvedToolchains` dict mapping type labels to providers

#### 2. Replace ToolchainsStub with ResolvedToolchains
**File**: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`

Create `ResolvedToolchains` struct:
```rust
struct ResolvedToolchains {
    toolchains: HashMap<TargetLabel, Option<Value>>, // type → ToolchainInfo or None
    exec_platform: TargetLabel,
}
```

Implement `StarlarkValue` with:
- `is_in()` → check if type is in the map
- `at()` → return the ToolchainInfo value (or None for optional)

Wire into `ctx.toolchains` attribute.

#### 3. Handle exec groups
For each exec group declared in `rule(exec_groups={...})`, run independent
resolution with that group's toolchain types and exec constraints.

#### 4. Delete ToolchainsStub and all per-language stubs
Once resolution works, delete:
- `ToolchainsStub` and its `at()` dispatch
- All `*ToolchainInfoStub`, `*ToolStub`, `*RuntimeStub` types
- All `host_tool_path()`, `detect_*_path()`, `host_cc_path()` functions
- `ExecGroupsDict` / `ExecGroupToolchains` stubs
- `GenericToolchainStub` fallback

This removes ~3000 lines of stub code from `context.rs`.

### Success Criteria

#### Automated Verification:
- [ ] `ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]` returns real CcToolchainInfo
- [ ] `ctx.toolchains` returns None for optional unmatched types
- [ ] `cargo check` passes with all stubs removed
- [ ] `examples/multi_package` builds (uses rules_cc)

#### Manual Verification:
- [ ] `//lib/hash:hash` in zeromatter builds with real CC toolchain from local_config_cc
- [ ] Rust targets use real rust_toolchain from rules_rust extension
- [ ] Python targets use real python toolchain from rules_python extension

---

## Phase 5: Ensure Extension-Created Toolchain Repos Materialize

### Overview
Toolchain resolution will reference toolchain targets in extension repos like
`local_config_cc`, `rust_toolchains`, `pythons_hub`. These repos must be
materialized before their targets can be analyzed.

### Changes Required

#### 1. Trigger extension execution during toolchain registration collection
When collecting registered toolchains from `register_toolchains("@repo//:target")`,
if `@repo` is an extension repo, ensure the extension has been executed and the
repo materialized.

#### 2. Handle the chicken-and-egg problem
Toolchain resolution needs analyzed toolchain targets → which need materialized
repos → which need extension execution → which may need other toolchains.

Bazel handles this by having extensions run WITHOUT toolchain access (extensions
use `repository_ctx` methods like `which()` and `execute()` to probe the host
directly, not through toolchains).

The resolution order is:
1. Extensions execute (no toolchain access needed)
2. Extension repos materialize (toolchain targets created)
3. Toolchain registration collects all toolchain targets
4. Analysis of regular targets triggers resolution

### Success Criteria

#### Automated Verification:
- [ ] `local_config_cc` materializes with real BUILD.bazel and cc_toolchain targets
- [ ] `rust_toolchains` materializes with real rust_toolchain targets
- [ ] Resolution finds these toolchains without manual intervention

---

## Anti-Patterns to Avoid

### DO NOT keep any per-language stub types
Every `*ToolchainInfoStub` must go. If a toolchain type's extension doesn't work,
fix the extension execution (Plan 10), don't add a stub.

### DO NOT probe the host filesystem during analysis
`detect_rust_tool_path()`, `which crane`, etc. are wrong. Toolchain probing happens
in repository rules (via `repository_ctx.which()`, `repository_ctx.execute()`), and
the results are stored in generated BUILD files. Analysis just reads those BUILD files.

### DO NOT hardcode tool paths
`/usr/bin/gcc`, `/usr/bin/python3`, etc. must come from the resolved toolchain's
providers, not from hardcoded constants.

## References

- [Bazel Toolchains Documentation](https://bazel.build/extending/toolchains)
- [Bazel Toolchain Resolution Algorithm](https://bazel.build/configure/toolchain-resolution)
- [Bazel Execution Groups](https://bazel.build/extending/exec-groups)
- Current stubs: `app/kuro_build_api/src/interpreter/rule_defs/context.rs`
- Native rules: `app/kuro_interpreter_for_build/src/interpreter/native_rules.rs`
- Registration parsing: `app/kuro_bzlmod/src/globals.rs`
- Analysis: `app/kuro_analysis/src/analysis/calculation.rs`
