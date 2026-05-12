# Toolchain Resolution: Replace Stubs with Real Bazel Algorithm

> **Main Plan**: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)

## Overview

Replace the entire `ToolchainsStub` system (30+ stub types, 3000+ lines of hardcoded
tool detection) with Bazel's real toolchain resolution algorithm. This is the last major
piece of stub behavior preventing fully Bazel-compatible builds.

Currently, `ctx.toolchains[TYPE]` returns hardcoded stubs that guess compiler paths
from the host system at analysis time. Real Bazel resolves toolchains by matching
`constraint_value`s against registered `toolchain()` targets, which are created by
module extensions (`cc_configure_extension`, `rust.toolchain`, etc.) that probe the
host and generate proper toolchain definitions.

## Current ZeroMatter Blocker (2026-05-07)

ZeroMatter verification has moved past the old CC toolchain miss, the module-extension
identity/materialization failures, and the downstream `NoneType.coreutils_info`
crash. The current hard blocker is still owned by this plan: a mandatory
`bazel_lib` toolchain type now fails directly during toolchain resolution.

Repro:

```bash
cd /var/mnt/dev/slug
cargo build --bin slug

cd /var/mnt/dev/zeromatter
/var/mnt/dev/slug/slug build //sdk:sdk_contents 2>&1 | tee /tmp/zeromatter-sdk-contents-20260507.log
```

Observed with build ID `d9d514f6-4817-4ca2-99e2-7220893866be`:

```text
BUILD FAILED
Error running analysis for `zeromatter//sdk:sdk_contents`
...
Error running analysis for `zeromatter//sdk:sdk_info_json`
...
Toolchain resolution failed for 'zeromatter//sdk:sdk_info_json':
No execution platform found that provides all mandatory toolchain types:
["@bazel_lib//lib:expand_template_toolchain_type"]
 • '@bazel_lib//lib:expand_template_toolchain_type': NO toolchain() registrations found for type
```

Important negative evidence from `/tmp/zeromatter-sdk-contents-20260507.log`:

- No `Unable to find a CC toolchain`.
- No `File not found: zeromatter//extensions.bzl`.
- No `@rules_jvm_external//extensions.bzl%maven` root-path failure.
- No `repository_rule_attr` miss.
- No maven `Failed to write downloaded file`.
- No rules_python `Failed to delete the python tester`.

The relevant upstream Starlark in ZeroMatter's materialized repo:

- `/var/mnt/dev/zeromatter/bazel-external/bazel_lib+3.2.2/lib/private/expand_template.bzl`
- `_EXPAND_TEMPLATE_TOOLCHAIN = Label("@bazel_lib//lib:expand_template_toolchain_type")`
- rules request `@bazel_lib//lib:expand_template_toolchain_type`, while materialized
  extension toolchain wrappers currently use `@aspect_bazel_lib//lib:expand_template_toolchain_type`.

So this is expected to be default exec-group toolchain resolution, not named
exec-group handling from Plan 12. The next agent should first determine why
the required type and registered toolchain type disagree on `bazel_lib` vs
`aspect_bazel_lib`; this is likely another missing repository-mapping application
in BUILD/.bzl label coercion or extension-repo label materialization.

Likely touchpoints:

- `app/slug_interpreter_for_build/src/attrs/coerce/ctx.rs`: verify BUILD/.bzl
  label coercion applies the package's repository mapping, not only global cell
  aliases.
- `app/slug_analysis/src/analysis/env.rs`: verify extracted `toolchain()`
  wrapper metadata canonicalizes both sibling extension repos and apparent
  module repo names through the wrapper package's repository mapping.
- `app/slug_analysis/src/analysis/toolchain_resolution.rs`: verify comparison
  does not paper over repository-mapping mismatches with string-only aliases.

Useful focused log probes:

```bash
rg -n 'expand_template_toolchain_type|aspect_bazel_lib|bazel_lib|Resolved toolchain|RequiredToolchain|Toolchain resolution' /tmp/zeromatter-sdk-contents-20260507.log
```

Desired next outcome:

- ZeroMatter `//sdk:sdk_contents` advances past `zeromatter//sdk:sdk_info_json`.
- Required toolchain types and registered toolchain wrappers agree after
  repository mapping is applied, without repo-name special cases.

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

1. ~~**Remote execution platform selection** — local execution only for now~~
   → **Superseded by [Plan 24](./24-exec-platform-resolution.md)**, which adds
   constraint-based exec platform resolution driven by
   `register_execution_platforms()` and `--extra_execution_platforms`.
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
**File**: `app/slug_bzlmod/src/types.rs`

Add fields to `ParsedModuleFile`:
```rust
pub struct ParsedModuleFile {
    // ... existing fields ...
    pub registered_toolchains: Vec<String>,
    pub registered_execution_platforms: Vec<String>,
}
```

#### 2. Record registrations in globals.rs
**File**: `app/slug_bzlmod/src/globals.rs`

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
**File**: `app/slug_common/src/legacy_configs/cells.rs`

After bzlmod resolution, collect all `register_toolchains()` labels in BFS order
of the module dependency graph. Store as a global ordered list accessible during
analysis.

#### 4. Handle `--extra_toolchains` / `--extra_execution_platforms` CLI flags
**File**: `app/slug_client/src/args.rs` or equivalent

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
**File**: `app/slug_analysis/src/analysis/native_rule_analysis.rs`

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

#### Status Note (2026-05-07):
Native `toolchain()` target construction now preserves user-supplied
`exec_compatible_with` and `target_compatible_with` lists. The native rule had
accepted these kwargs but dropped them before creating the `TargetNode`, so every
declared toolchain registered with empty compatibility constraints and the first
registered candidate matched every host/target platform. ZeroMatter exposed this as
darwin toolchains being eligible on a Linux host.

Fix landed in `b542145a`:
- `app/slug_interpreter_for_build/src/interpreter/native_rules.rs::toolchain`
  coerces both compatibility lists as `list(dep)` and passes them to
  `create_native_target_node`.
- `create_native_target_node` now merges `name`, `visibility`, and user attrs
  into one `(AttributeId, CoercedAttr)` vector and sorts once before
  `push_sorted`; this is required because `exec_compatible_with` is attr id 4
  and `visibility` is attr id 5.
- `app/slug_analysis/src/analysis/toolchain_resolution.rs` normalizes bzlmod
  module-version repo names for constraint matching, so apparent labels like
  `@platforms//os:linux` and canonical labels like
  `@platforms+1.0.0//os:linux` compare equal while extension repos such as
  `rules_cc+cc_configure_extension+local_config_cc_toolchains` remain distinct.

Verification:
- `cargo test -p slug_analysis toolchain_resolution --lib` passes.
- `cargo test -p slug_interpreter_for_build -p slug_analysis -p slug_node -p slug_common -p slug_configured -p slug_action_impl -p slug_core --lib`
  passes except the pre-existing `slug_core::pattern::pattern::tests::test_relaxed`.
- `cargo build --bin slug` passes.
- `examples/multi_package :gen_version_header` builds clean.
- ZeroMatter `rules_cc//:link_extra_lib --target-platforms=//bazel/platforms:linux-gnu-host`
  no longer reports `Unable to find a CC toolchain`; the run later failed with
  daemon/event-bus breakage after external-repo materialization warnings.

---

## Phase 3: Implement the Resolution Algorithm

### Overview
The core: given a target's required toolchain types, the target platform, registered
toolchains, and registered execution platforms, select the matching toolchains and
execution platform.

### Changes Required

#### 1. Create ToolchainResolutionKey DICE key
**File**: `app/slug_analysis/src/analysis/toolchain_resolution.rs` (new file)

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
**File**: `app/slug_analysis/src/analysis/calculation.rs`

Before calling the rule's implementation function:
1. Get the rule's declared `toolchains = [...]` from the rule definition
2. Run `ToolchainResolutionKey::compute()` via DICE
3. For each resolved toolchain label, analyze the toolchain impl target
4. Collect `ToolchainInfo` providers from each resolved toolchain
5. Build a `ResolvedToolchains` dict mapping type labels to providers

#### 2. Replace ToolchainsStub with ResolvedToolchains
**File**: `app/slug_build_api/src/interpreter/rule_defs/context.rs`

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
- [x] `cargo check` passes with resolution wired in
- [x] Build still reaches same point as before (no regressions)
- [x] `ResolvedToolchains` type created with `is_in()` and `at()` methods
- [x] Resolution runs during analysis and logs results at debug level
- [x] `ResolvedToolchains` created with DICE-analyzed impl target providers (falls back to stubs when analysis fails)
- [x] `ctx.toolchains` returns `ResolvedToolchains` object (with ToolchainsStub fallback for unresolved types)
- [x] `toolchain_types` correctly extracted from `rule(toolchains=[config_common.toolchain_type(...)])` (ToolchainTypeRequirement parsing fixed)
- [x] Label normalization strips all `@` prefixes for consistent matching
- [x] `ctx.toolchains["@bazel_tools//tools/cpp:toolchain_type"]` returns real CcToolchainInfo via ToolchainInfo provider extraction
- [x] Stubs fully removed (Phase 7 complete — all per-language toolchain stubs deleted)

#### Status Note (2026-04-01 update):
Resolved blockers for cc_toolchain_config analysis on Linux:
- [x] Implicit source file targets (`:builtin_include_directory_paths` resolves)
- [x] `actions.transform_version_file` / `transform_info_file` stubs
- [x] `ApplePlatformStub` with `platform_type` for `ctx.fragments.apple.single_arch_platform`
- [x] `XcodeVersionConfig` provider with `minimum_os_for_platform_type()` method
- [x] `xcode_config` native rule in `bazel_tools//tools/osx:current_xcode_config`
- [x] `configuration_field("apple", "xcode_config_label")` resolves to xcode_config target
- [x] `cc_common_internal.exec_os()` and `target_os()` methods
- [x] `PackageSpecificationInfo` provider on `package_group` native rule analysis
- [x] `cc_common_internal.cc_toolchain_features()` (returns CcToolchainFeatures with configure_features/default_features_and_action_configs)
- [x] `cc_common_internal.cc_toolchain_variables()` (returns build variables) — already implemented
- [x] Other `_cc_internal` methods: `check_private_api`, `freeze`, `get_artifact_name_for_category` — already implemented
- [x] `cc_common_internal.solib_symlink_action()` stub (returns artifact unchanged)
- [x] `FeatureConfiguration.is_enabled()` and `is_requested()` methods
- [x] Subrule ctx injection for `create_fdo_context` (thread-local ctx + implicit attr injection)
- [x] `CppFragment.minimum_os_version()`, `interface_shared_objects()`, and other missing methods
- [x] `ResolvedToolchains.at()` extracts `ToolchainInfo` provider (not raw ProviderCollection)
- [x] Compiler executable path fixed: `RepositoryPath` Display now renders raw path instead of `<repository_path>` wrapper
- [x] `hello_bin` and `hello_test_static` build successfully in cc_test_example using real CC toolchain

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
- [x] `rust_toolchains` materializes with real BUILD.bazel (already exists from Plan 10)
- [x] Registration collection identifies `local_config_cc_toolchains` as needing materialization
- [x] Repo existence check correctly scans bazel-external/ for versioned names
- [x] `local_config_cc_toolchains` materializes via cc_configure_extension execution (real BUILD with toolchain() targets)
- [x] `local_config_cc` materializes with full cc_toolchain definitions and support files
- [x] Resolution finds CC toolchains from materialized repos (`cc-toolchain-k8` resolves to `local_config_cc//:cc-compiler-k8`)
- [x] `@bazel_tools//tools/cpp` BUILD updated with aliases to rules_cc and module stubs
- [x] `local_config_cc//:cc-compiler-k8` source file deps resolve (implicit source file targets implemented)
- [x] `actions.transform_version_file` and `transform_info_file` stubbed for cc_build_info
- [x] `ctx.fragments.apple.single_arch_platform` returns stub with `platform_type`
- [x] `local_config_cc//:cc-compiler-k8` full analysis succeeds (cc_toolchain_features, subrule ctx injection, ToolchainInfo extraction all working)

#### Status Note (2026-04-01 evening update):
All blockers for cc_toolchain analysis on Linux resolved:
- `cc_common_internal.cc_toolchain_features()` implemented (CcToolchainFeatures type)
- Subrule `create_fdo_context` works via thread-local ctx injection
- `FeatureConfiguration.is_enabled()` and `is_requested()` methods added
- `ResolvedToolchains.at()` now extracts ToolchainInfo (not raw ProviderCollection)
- `CppFragment` extended with `minimum_os_version()`, `interface_shared_objects()`, etc.
- Removed `static_link_cpp_runtimes` from default features (needs explicit toolchain setup)

cc_library analysis completes and reaches action execution. The remaining gap is at
execution time: compiler path uses `<repository_path>` placeholder instead of real path.

---

## Phase 6: Eager Loading of Registered Toolchain Packages

### Overview
Close the materialization gap: make the analysis pipeline eagerly load
registered toolchain packages via DICE so that extension repos materialize and
their `toolchain()` targets populate the `DeclaredToolchainInfo` registry.

This is the equivalent of Bazel's `RegisteredToolchainsFunction` SkyFunction,
which loads and analyzes all registered toolchain targets before any rule
analysis can request toolchain resolution.

### The Problem
`cc_configure_extension` repos (`local_config_cc`, `local_config_cc_toolchains`)
have `ExtensionRepoCellSetup` from `use_repo()`, but their lazy materialization
never triggers because `ToolchainsStub` short-circuits real toolchain lookup.
Nothing in the build graph accesses these repos, so nothing forces extension
execution → repo creation → package loading → toolchain() analysis.

### Changes Required

#### 1. Create `ensure_registered_toolchains_loaded()` async function
**File**: `app/slug_analysis/src/analysis/env.rs` (or new `toolchain_loading.rs`)

This function runs ONCE per build session (guarded by a static `OnceLock`).
It takes a `&mut DiceComputations` and:
1. Reads the global `REGISTERED_TOOLCHAINS` list
2. For each label pattern (e.g., `@local_config_cc_toolchains//:all`):
   a. Parse the label to extract the cell name
   b. Resolve the cell via `CellResolver` (triggers `ExtensionRepoCellSetup`
      → lazy materialization → extension execution)
   c. Load the package via `InterpreterResultsKey` (triggers BUILD.bazel parsing)
   d. Each `toolchain()` target in the package gets analyzed via normal DICE flow,
      which populates `DeclaredToolchainInfo` registry (Phase 2)
3. After all packages are loaded, the registry contains all declared toolchains

The key is using DICE's `ctx.compute()` for each step, which naturally handles:
- Extension repo materialization (triggered by cell access)
- Package loading (triggered by interpreter results key)
- Target analysis (triggered by analysis key for toolchain() targets)

#### 2. Call from first resolution attempt
**File**: `app/slug_analysis/src/analysis/env.rs`

In `run_analysis_with_env_underlying()`, before the resolution code:
```rust
// One-time: ensure all registered toolchain packages are loaded
ensure_registered_toolchains_loaded(dice).await;
```

Use `tokio::sync::OnceCell` or `std::sync::OnceLock` with an atomic flag
to ensure this only runs once per daemon session.

#### 3. Handle `:all` and `//...` patterns
Registered labels like `@repo//:all` mean "all targets in the root package."
`@repo//...` means "all targets recursively." For the initial implementation,
handle `:all` by loading the package and finding all `toolchain()` targets.
Pattern expansion for `//...` can be deferred.

### Implementation Notes (2026-03-31)

Implemented in `app/slug_analysis/src/analysis/env.rs`:

1. `ensure_registered_toolchains_loaded()` — async function guarded by `AtomicBool`
   (TOOLCHAINS_LOADING_DONE). Reads `REGISTERED_TOOLCHAINS`, parses labels, resolves
   cells via `CellResolver`, loads packages via `dice.get_interpreter_results()`,
   iterates targets to find `toolchain()` rules, extracts metadata from unconfigured
   `CoercedAttr` values, and registers in `DeclaredToolchainInfo` registry.

2. `parse_registered_toolchain_label()` — extracts `(repo_name, pkg_path)` from
   `@repo//pkg:target` patterns. Unit tested.

3. `extract_toolchain_info_from_node()` — reads `toolchain_type`, `toolchain`,
   `exec_compatible_with`, `target_compatible_with` from unconfigured target node attrs.

4. Called from `run_analysis_with_env_underlying()` before toolchain resolution.

5. Resolution result is now properly computed (no longer discarded with `_` prefix)
   and logged with target-specific messages.

6. Added `slug_bzlmod` as dependency to `slug_analysis/Cargo.toml`.

**Remaining gap**: Extension repos like `local_config_cc_toolchains` must actually
materialize (their `cc_configure_extension` must execute successfully) for the
package to contain `toolchain()` targets. The infrastructure to trigger materialization
is in place — the DICE `get_interpreter_results()` call will trigger it — but the
extension execution itself must produce valid BUILD.bazel files.

### Success Criteria

#### Automated Verification:
- [x] `ensure_registered_toolchains_loaded()` implemented and wired into analysis pipeline
- [x] `parse_registered_toolchain_label()` correctly parses `@repo//pkg:target` patterns (unit tested)
- [x] `cargo check` passes, 190 analysis tests pass with no regressions
- [x] Resolution result no longer discarded (logs resolved toolchain types)
- [x] Function called from `get_analysis_result_inner()` (covers both native and Starlark rules)
- [x] 67 registered toolchain packages loaded from manual_test project
- [x] `@rules_foreign_cc//toolchains` loads 11 real toolchain() targets (55 total across packages)
- [x] `DeclaredToolchainInfo` registry populated with real toolchain entries
- [x] `local_config_cc_toolchains` has real toolchain() targets (cc_configure_extension now produces real BUILD files)
- [x] `resolve_toolchains()` returns real CC toolchain match (cc-toolchain-k8 → local_config_cc//:cc-compiler-k8)
- [x] `ctx.toolchains` returns real `ToolchainInfo` (cc_toolchain analysis succeeds, ToolchainInfo extracted from provider collection)

#### Status Note (2026-04-01):
Extension repos `local_config_cc` and `local_config_cc_toolchains` now materialize with real
BUILD files containing toolchain() targets and cc_toolchain definitions respectively.
cc_configure_extension executes successfully and creates proper content.

The full resolution pipeline works end-to-end: register_toolchains → eager package loading →
DeclaredToolchainInfo registry → resolve_toolchains → DICE analysis of impl targets →
ResolvedToolchains on ctx → ToolchainInfo provider extraction → real CcToolchainInfo.

**2026-04-01 evening**: All analysis blockers resolved. The pipeline:
1. `cc_common_internal.cc_toolchain_features()` creates CcToolchainFeatures from CcToolchainConfigInfo
2. Subrule `create_fdo_context` receives ctx via thread-local injection
3. `ResolvedToolchains.at()` extracts ToolchainInfo from provider collection
4. cc_library rules get real CcToolchainInfo with real tool paths and features
5. Compilation reaches action execution (compiler path still needs fixing)

#### Manual Verification:
- [ ] `//lib/hash:hash` in zeromatter builds using real CC toolchain
- [ ] No hardcoded compiler paths in build actions
- [ ] `ToolchainsStub` and all per-language stubs can be deleted

---

## Phase 7: Delete All Stubs

### Overview
Once Phase 6 validates that real toolchain resolution works end-to-end,
delete all stub types from `context.rs` (~3000 lines).

### Changes Required

#### 1. Remove ToolchainsStub and all per-language stubs
Delete from `context.rs`:
- `ToolchainsStub` (line ~2030) and its `at()` dispatch
- `CcToolchainInfoStub` and all its helper types
- `RustToolchainInfoStub`, `RustToolStub`, `RustTripleStub`, etc.
- `PyToolchainInfoStub`, `PyRuntimeInfoStub`
- `JavaToolchainInfoStub`, `JavaRuntimeInfoStub`
- `OciCraneToolchainStub`, `JqToolchainStub`
- `GenericToolchainStub`
- `ExecGroupsDict`, `ExecGroupToolchains`

#### 2. Remove hardcoded tool detection
Delete from `context.rs`:
- `host_tool_path()`, `host_cc_path()`, `host_target_cpu()`, `host_rust_triple()`
- `detect_rust_tool_path()`, `detect_crane_path()`, `detect_jq_path()`
- `get_or_create_oci_launcher()`

#### 3. Remove ToolchainsStub fallback from ctx.toolchains
Make `ctx.toolchains` REQUIRE resolved toolchains — error if resolution
didn't produce results instead of falling back to stubs.

### Success Criteria
- [x] `context.rs` reduced by ~2100 lines (5417→3308; remaining are non-toolchain ctx types)
- [x] No `*ToolchainInfoStub`, `*ToolStub`, `ToolchainsStub`, or `GenericToolchainStub` types remain
- [x] `host_tool_path()`, `host_rust_triple()`, `detect_*_path()`, `get_or_create_oci_launcher()` deleted
- [x] `ctx.toolchains` returns empty `ResolvedToolchains` (not stubs) when no resolution ran
- [x] `ResolvedToolchains.at()` errors on unresolved types (no stub fallback)
- [x] CC builds succeed with real toolchain resolution (cc_test_example hello_bin, hello_test_static)
- [ ] All builds use real toolchain resolution (Rust/Python/Java not yet verified)

#### Notes:
- `host_target_cpu()` and `host_cc_path()` retained for Make variables (ctx.var) — these are
  not toolchain stubs but default values for BINDIR/CC Make variables
- `CompilationContextStub` / `LinkingContextStub` retained — used by cc_common.rs for creating
  empty CcInfo providers, not toolchain dispatch
- `ExecGroupsDict` / `ExecGroupToolchains` retained as structural types but updated to not
  depend on ToolchainsStub (ExecGroupToolchains.at() returns None instead of stub dispatch)

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

## Phase 8: Alias Resolution for `toolchain_type` Labels (follow-up, 2026-05-04)

### Problem

Discovered while closing Plan 10 (module extension execution). rules_rust's
`BUILD_for_toolchain` template emits `toolchain_type = "@rules_rust//rust:toolchain"`
literally, but `@rules_rust//rust:toolchain` is an `alias()` whose `actual` is
`:toolchain_type` (the real `toolchain_type()` rule). Rules' Starlark uses
`Label("//rust:toolchain_type")` for resolution. Slug's exact-string match in
`toolchain_resolution.rs:253-256` (`tc_type_norm != req_type_norm`) doesn't follow
aliases, so the `rust` toolchain never resolves and `ctx.toolchains` returns
empty.

### Root cause

- Generated BUILD: `app/slug_analysis/src/analysis/env.rs::extract_toolchain_info_from_node`
  stores the literal `toolchain_type` attr value.
- Match site: `app/slug_analysis/src/analysis/toolchain_resolution.rs:253-256`
  compares the stored value to `req.type_label` after `normalize_constraint_label`
  (which only strips `@@`/`@` prefixes).
- The two labels `@rules_rust//rust:toolchain` and `@rules_rust//rust:toolchain_type`
  are not string-equal, so the loop falls through and `found = None`.

This is structural: Bazel's analysis dereferences the alias when registering
toolchains; slug stops at the alias label.

### Fix

Canonicalize `toolchain_type` labels at registration time.
`ensure_registered_toolchains_loaded` (`env.rs:509`) already has DICE access. After
`extract_toolchain_info_from_node` returns a `DeclaredToolchainInfo`:

1. Parse `info.toolchain_type` to a `TargetLabel`.
2. Load the target's package via `ctx.get_interpreter_results(pkg)`.
3. Look up the target node by name. If `RuleType::Native(NativeRuleKind::Alias)`,
   read its `actual` attribute and recurse (with a depth cap to prevent cycles).
4. Replace `info.toolchain_type` with the canonical `toolchain_type()` rule label
   before `register_declared_toolchain`.

Cache `(label → canonical label)` in a small map for the duration of the
registration pass. Most toolchain registrations point at the same handful of
aliases.

### Touchpoints

- `app/slug_analysis/src/analysis/env.rs::ensure_registered_toolchains_loaded` —
  add alias-resolution step after `extract_toolchain_info_from_node`.
- `app/slug_analysis/src/analysis/env.rs::extract_toolchain_info_from_node` —
  unchanged (alias-resolution happens at the call site, which has DICE).
- `app/slug_analysis/src/analysis/toolchain_resolution.rs` — unchanged. Match
  logic stays exact-string after normalization; canonicalization happens upstream.

### Success criteria

#### Automated
- `cargo check -p slug_analysis` clean
- `cargo test -p slug_analysis --lib` — no regressions

#### Manual
- `slug build //sdk:sdk` in zeromatter: `rust_library` targets resolve
  `@rules_rust//rust:toolchain_type` to the registered rust toolchain.
- `tracing::debug!` log shows canonical `toolchain_type` after registration
  (e.g. `type='@rules_rust//rust:toolchain_type'` not `:toolchain`).

### Status

**Implemented 2026-05-04** in `app/slug_analysis/src/analysis/env.rs`:
- Added `canonicalize_toolchain_type_label()` async helper — parses the label,
  loads the package via DICE, walks `alias()` chains by reading the `actual`
  attr; cycle-safe (visited set), depth-capped (8), benign on errors (returns
  input unchanged).
- Wired into `ensure_registered_toolchains_loaded`'s parallel loader: after
  `extract_toolchain_info_from_node`, replaces `info.toolchain_type` with the
  canonical label before `register_declared_toolchain`. Per-task `alias_cache`
  HashMap; DICE handles cross-task dedup of `get_interpreter_results`.

**Verified:**
- `cargo check -p slug_analysis` clean
- `cargo build -p slug` clean
- `cargo test -p slug_analysis --lib` — 11/11 pass

**End-to-end runtime exercise pending**: zeromatter's `//sdk:sdk` build
is now blocked by the **Plan 13 Phase 3** toolchain-loading wall (30+ minute
serial materialization of all bzlmod transitive toolchain registrations)
rather than by extension execution failures. The Plan 10 Phase 4 follow-up
that previously blocked extension materialization (cross-module
relative-label canonicalization in `attr.label` tag attributes) was fixed
2026-05-04 in the same session.

Additionally, zeromatter uses **rules_rs**, whose
`declare_rustc_toolchains` macro emits the canonical
`@rules_rust//rust:toolchain_type` label directly — so even when the build
progresses past extension materialization, the alias case wouldn't fire in
this specific repo. The alias case requires **rules_rust's own
`rust_register_toolchains()` workflow** (`BUILD_for_toolchain` template at
`repository_utils.bzl:474-501`), which emits
`toolchain_type = "@rules_rust//rust:toolchain"` (alias) literally. Any repo
using that workflow rather than rules_rs will exercise this code path.

The alias case is specific to **rules_rust's own `rust_register_toolchains()`
workflow** (`BUILD_for_toolchain` template at `repository_utils.bzl:474-501`),
which emits `toolchain_type = "@rules_rust//rust:toolchain"` (alias) literally.
Any repo using that workflow rather than rules_rs will exercise this code path.

The fix is defensive and correct; full runtime exercise will land naturally
the first time a `rust_register_toolchains()`-based workspace is built. A
debug-level trace logs `Resolved toolchain_type alias '<src>' -> '<dst>'` on
each canonicalization for observability.

---

## References

- [Bazel Toolchains Documentation](https://bazel.build/extending/toolchains)
- [Bazel Toolchain Resolution Algorithm](https://bazel.build/configure/toolchain-resolution)
- [Bazel Execution Groups](https://bazel.build/extending/exec-groups)
- Current stubs: `app/slug_build_api/src/interpreter/rule_defs/context.rs`
- Native rules: `app/slug_interpreter_for_build/src/interpreter/native_rules.rs`
- Registration parsing: `app/slug_bzlmod/src/globals.rs`
- Analysis: `app/slug_analysis/src/analysis/calculation.rs`
