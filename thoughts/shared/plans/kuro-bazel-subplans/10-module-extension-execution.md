# Module Extension Execution: Let Real Extensions Run

> **Main Plan**: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)

## Overview

Remove the entire synthetic repository system and let ALL module extensions
execute via DICE. The synthetic repos were a stopgap during early development —
they are a crutch that must be removed entirely. The DICE-based extension
execution infrastructure already exists and works. The only thing blocking real
execution is that `synthetic_repos.rs` intercepts known extensions and
short-circuits them with hardcoded stubs.

## Current State Analysis

### What Works

- `ModuleExtensionExecutionKey::compute()` in `extension_execution_dice.rs` —
  full DICE pipeline with lockfile caching
- `ConcreteModuleExtensionExecutor::try_execute_starlark()` — loads `.bzl` files,
  evaluates extension implementations, captures `RepoSpec` objects
- `ModuleContext` in `module_ctx.rs` — complete Bazel-compatible `module_ctx` with
  `path()`, `execute()`, `download()`, `download_and_extract()`, `read()`,
  `which()`, `os.name/arch`, `modules`, etc.
- `repository_rule.rs:403-424` — `in_extension_context()` correctly captures
  `RepoSpec` instead of executing downloads
- `ExtensionRepoExecutionKey` in `repository_execution.rs` — lazy materialization
  of individual repos from `RepoSpec`
- `pre_compute_extension_repo_cells()` — registers extension repos as
  `ExtensionRepoCellSetup` cells that trigger DICE on first access

### What's Broken

`synthetic_repos.rs` line 99-187 (`generate_synthetic_repos_for_extension`)
intercepts ALL known extensions and returns hardcoded `SyntheticRepo` objects.
These are materialized as regular cells (not `ExtensionRepo` cells), which means
they never trigger DICE execution. The crate extension was already changed to
return `None`, but it fails because its dependencies are still stubbed.

### The Extension Dependency Chain (zeromatter)

```
zeromatter MODULE.bazel
  |
  +-- @rules_rs//rs/experimental:rules_rust.bzl "rules_rust"
  |     (override_repo: makes rules_rust available)
  |
  +-- @rules_rust//rust:extensions.bzl "rust"
  |     (rust.toolchain(edition="2024", versions=["1.91.1"]))
  |     -> creates @rust_toolchains
  |
  +-- @rules_rs//rs:extensions.bzl "crate"
        (crate.from_cargo(name="crates", ...))
        -> loads @rs_rust_host_tools//:defs.bzl (RS_HOST_CARGO_LABEL)
        -> creates @crates hub + spoke repos

rules_rust MODULE.bazel (transitive)
  |
  +-- //rust/private:internal_extensions.bzl "i"
  |     -> creates @rules_rust_tinyjson, @rrra, @rrra__anyhow-1.0.71, etc.
  |
  +-- //cargo/private:internal_extensions.bzl "i"
  |     -> creates @rrc, @rrc__cargo_toml-0.20.5, etc.
  |
  +-- //rust:extensions.bzl "rust_host_tools"
  |     -> creates @rust_host_tools (cargo/rustc paths)
  |
  +-- //crate_universe/private:internal_extensions.bzl "i"
  |     -> creates @cui__*, crate_universe internal deps
  |
  +-- //test:test_extensions.bzl "rust_test"
        -> creates @buildkite_config, @libc, etc.

rules_rs MODULE.bazel (transitive)
  |
  +-- //rs/experimental/toolchains:module_extension.bzl "toolchains"
  |     -> creates @rs_rust_host_tools, @default_rust_toolchains
  |
  +-- @aspect_tools_telemetry//:extension.bzl "telemetry"
        -> creates @aspect_tools_telemetry_report
```

### Key Constraint: RS_HOST_CARGO_LABEL

The crate extension does `load("@rs_rust_host_tools//:defs.bzl",
"RS_HOST_CARGO_LABEL")` at line 4. This label points to a cargo binary
discovered by the `toolchains` extension. The extension then calls
`mctx.path(RS_HOST_CARGO_LABEL)` to get the filesystem path to cargo.

In real Bazel, `RS_HOST_CARGO_LABEL` is a `Label("@repo//:bin/cargo")` and
`module_ctx.path(Label)` resolves it. In kuro, `module_ctx.path()` accepts
strings. This means either:
- The toolchains extension must execute and produce a real label, OR
- `module_ctx.path()` must handle Label objects

## Repository Taxonomy: What Bazel Actually Does

In Bazel 9 there are exactly two categories of repos that the build tool itself
provides. Everything else comes from extension execution.

### 1. Build-tool builtins (shipped with the binary)

| Repo | What it is | How Bazel provides it | How kuro provides it |
|------|-----------|----------------------|---------------------|
| `@bazel_tools` | Build tool's own rules, tools, platforms | Embedded in Bazel binary | **Bundled cell** (`ExternalCellOrigin::Bundled`) at `cells.rs:566-573` — ships in `kuro/bazel_tools/` |
| `@local_config_platform` | Host platform auto-detection (`//:host`) | Built-in repository rule, no extension | **Bundled cell** (`ExternalCellOrigin::Bundled`) at `cells.rs:576-584` |

These are correct as-is. They are NOT in `synthetic_repos.rs` and don't need to
change.

### 2. Extension-generated repos (everything in synthetic_repos.rs)

Every other repo — including `bazel_features`, `local_config_cc`,
`rules_rust_tinyjson`, `rust_toolchains`, `crates`, etc. — is created by
a module extension executing `.bzl` code. There is no middle category.

**`bazel_features` is not special.** Its `version_extension` is a standard
module extension that calls `version_repo` (a repository rule). That rule does
`rctx.file("version.bzl", "version = '" + native.bazel_version + "'")`. If kuro
implements `native.bazel_version` (it already does, returning `"9.0.0"`), the
extension works. There is no reason to hardcode this in Rust.

**`local_config_cc` is not special.** It's created by `rules_cc`'s
`cc_configure_extension`, which probes the host C++ compiler. That's a standard
extension calling `repository_ctx.execute()` and `repository_ctx.which()`. If
kuro implements those (it does), the extension works.

**`rules_rust_tinyjson` is not special.** It's created by rules_rust's internal
extension, which calls `http_archive()` to download tinyjson from crates.io.
The `RepoSpec` capture mechanism records this as a lazy download. There is no
reason to embed 210 lines of Rust source code for a JSON parser in
`synthetic_repos.rs`.

### Conclusion

`synthetic_repos.rs` should be deleted entirely. The bundled cell system
(`@bazel_tools`, `@local_config_platform`) is a separate, correct mechanism
that stays. Everything else executes via DICE.

## Design Principles

### CRITICAL: No More Synthetic Stubs for Extension Repos

**The previous approach of writing hardcoded Rust code to simulate what
extensions do is fundamentally wrong.** It creates:

1. **Maintenance burden** — Every rules_rust version change breaks stubs
2. **Semantic drift** — Stubs generate wrong target types (e.g., `rust_library`
   vs `rust_crate`), wrong deps, wrong features
3. **Chicken-and-egg problems** — Stubbing one extension breaks others that
   depend on its output
4. **Dead code** — 800+ lines of Cargo.lock parsing, registry source copying,
   BUILD file generation that reimplements what rules_rs already does

**The correct approach is to let the real extension code run.** Kuro already has
the infrastructure for this. The only thing needed is to stop intercepting
extensions in `synthetic_repos.rs`.

### All Synthetic Repos Must Go

The synthetic repo system was a stopgap. ALL extensions should execute via DICE:

| Extension | Currently | Target |
|-----------|-----------|--------|
| `bazel_features` version_extension | Synthetic | **Let execute** |
| `rules_cc` cc_configure_extension | Synthetic | **Let execute** |
| `rules_cc` compatibility_proxy | Synthetic | **Let execute** |
| `rules_java` compatibility_proxy | Synthetic | **Let execute** |
| `rules_rust` internal `"i"` | Synthetic | **Let execute** |
| `rules_rust` cargo internal `"i"` | Synthetic | **Let execute** |
| `rules_rust` crate_universe `"i"` | Synthetic | **Let execute** |
| `rules_rust` test `"rust_test"` | Synthetic | **Let execute** |
| `rules_rust` `"rust"` toolchain | Synthetic | **Let execute** |
| `rules_rust` `"rust_host_tools"` | Unrecognized (stub) | **Let execute** |
| `rules_rs` `"toolchains"` | Unrecognized (stub) | **Let execute** |
| `rules_rs` `"crate"` | **Already None** | **Let execute** |
| `rules_python` internal | Synthetic | **Let execute** |
| `rules_python` toolchain | Synthetic | **Let execute** |
| LLVM toolchain | Synthetic | **Let execute** |

### Incremental Strategy

We remove interception in phases to maintain a buildable state at each step.
Each phase builds on the previous one. The end state is that
`generate_synthetic_repos_for_extension` is deleted entirely — it should not
exist.

## What We're NOT Doing

1. **Not implementing full toolchain resolution** — host-detected stubs remain as
   fallbacks for `ctx.toolchains[]` until real toolchain resolution is wired
2. **Not implementing sandboxed execution** — extensions run with full host access
3. **Not implementing lockfile-based caching initially** — extensions re-execute
   on daemon restart (lockfile support exists but may need debugging)

## Phase 0: Clean Up Investigation Artifacts

### Overview
Remove all temporary debug logging and hacky stub code added during the
investigation session.

### Changes Required

#### 1. Remove debug file writes from extension_execution_dice.rs
**File**: `app/kuro_bzlmod/src/extension_execution_dice.rs`
Remove the `/tmp/kuro_ext_debug*.log` file writes and restore the original
`MODULE_EXTENSION_EXECUTOR_IMPL.get()` match arms.

#### 2. Remove debug logging from module_extension_executor_impl.rs
**File**: `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`
Restore original `tracing::debug` (not `tracing::warn`) for the import_path log.
Restore original `.buck_error_context()` error handling (not the expanded match).

#### 3. Revert error propagation change
**File**: `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`
Restore the graceful fallback to empty specs on extension execution failure.
This is needed because some extensions (e.g., test-only ones) may legitimately
fail to load, and hard-failing would break builds that don't need those repos.

#### 4. Remove stub defs.bzl generation hacks
**File**: `app/kuro_bzlmod/src/synthetic_repos.rs`
Remove `generate_stub_defs_bzl()`, `find_tool_in_path()`, and the
`generate_stub_repo()` helper. Revert `generate_stub_repos_for_extension()` to
its original form (just BUILD.bazel stubs).

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes with no new warnings
- [x] `cargo test -p kuro_bzlmod` — all tests pass
- [x] No references to `/tmp/kuro_ext_debug` in codebase
- [x] No `find_tool_in_path` or `generate_stub_defs_bzl` in codebase

---

## Phase 1: Remove ALL Synthetic Extension Interception

### Overview
Delete `generate_synthetic_repos_for_extension` entirely. Every extension
goes through DICE execution. The `_` catch-all stub handler is also removed —
if an extension's repos aren't needed by the build, they won't be accessed and
DICE won't execute them. If they are needed, DICE will execute the real `.bzl`
code.

### Changes Required

#### 1. Delete `generate_synthetic_repos_for_extension` and all generator functions
**File**: `app/kuro_bzlmod/src/synthetic_repos.rs`

Remove the entire match dispatch and all functions it calls:
- `generate_synthetic_repos_for_extension()` — the match dispatcher
- `generate_bazel_features_repos()`, `generate_bazel_features_version_repo()`,
  `generate_bazel_features_globals_repo()`
- `detect_msvc_bin_dir()`, `detect_host_platform()`, `HostPlatformInfo`
- `generate_rules_cc_repos()`, `generate_local_config_cc_repo()`,
  `generate_local_config_cc_toolchains_repo()`
- `generate_cc_compatibility_repo()`
- `generate_java_compatibility_proxy()`
- `generate_rules_rust_internal_repos()`, `generate_rules_rust_tinyjson_repo()`
- `generate_rules_python_internal_repos()`,
  `generate_rules_python_internal_config_repo()`
- `generate_rules_python_toolchain_repos()`, `generate_pythons_hub_repo()`
- `generate_rules_rust_toolchain_repos()`
- `generate_llvm_toolchain_repos()`
- `generate_stub_repos_for_extension()`, `generate_stub_repo()`,
  `generate_stub_defs_bzl()`, `find_tool_in_path()`

Make `collect_synthetic_repos_with_root()` return an empty Vec (or delete it
and update callers). Remove `materialize_synthetic_repos()` if no longer
called.

#### 2. Update callers in cells.rs
**File**: `app/kuro_common/src/legacy_configs/cells.rs`

`generate_synthetic_extension_repos()` calls
`collect_synthetic_repos_with_root()`. With no synthetic repos to generate,
this function should return an empty Vec. The extension repos from
`pre_compute_extension_repo_cells()` will handle all extension-generated cells.

#### 3. Clean up the module
**File**: `app/kuro_bzlmod/src/synthetic_repos.rs`

After removing all generators, the file should contain only the `SyntheticRepo`
struct definition (if still needed by other code) and `materialize_synthetic_repos`
(if still needed). If nothing references them, delete the entire file and remove
it from `mod.rs`.

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes
- [x] `cargo test -p kuro_bzlmod` — all tests pass (157 pass)
- [x] No `generate_synthetic_repos_for_extension` in codebase
- [x] No `generate_bazel_features` in codebase
- [x] No `generate_rules_cc` in codebase
- [x] No `generate_rules_rust` in codebase
- [x] No `generate_rules_python` in codebase
- [x] No `generate_llvm_toolchain` in codebase
- [x] `synthetic_repos.rs` is either empty/minimal or deleted

---

## Phase 2: Fix module_ctx.path() to Handle Labels

### Overview
The crate extension calls `mctx.path(RS_HOST_CARGO_LABEL)` where the value is
a Bazel `Label` object (from `Label("@repo//:bin/cargo")`). Currently
`module_ctx.path()` only accepts `&str`. It needs to also accept Label values.

### Changes Required

#### 1. Accept Label values in path()
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

Change the `path()` method signature from `path: &str` to `path: Value` and
handle both string and Label types:

```rust
fn path<'v>(
    this: &ModuleContext,
    #[starlark(require = pos)] path: Value<'v>,
    heap: Heap<'v>,
) -> starlark::Result<Value<'v>> {
    // If it's a Label, resolve it to a filesystem path
    // If it's a string, resolve relative to working_dir (existing behavior)
    let path_str = if let Some(label) = path.downcast_ref::<Label>() {
        // For labels like @repo//:bin/cargo, resolve via cell paths
        label.to_string()  // Will need actual resolution
    } else {
        path.unpack_str().ok_or_else(|| {
            anyhow::anyhow!("path() requires a string or Label, got {}", path.get_type())
        })?.to_string()
    };
    // ... existing resolution logic
}
```

The exact Label resolution depends on how the extension context resolves labels.
In real Bazel, `module_ctx.path(Label)` looks up the file in the already-
materialized repo. This may require the referenced repo to be materialized first.

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes
- [x] `module_ctx.path("some/path")` still works (string case)
- [x] `module_ctx.path(Label("@repo//:file"))` doesn't crash

---

## Phase 3: Test Extension Execution End-to-End

### Overview
With all rules_rust/rules_rs extensions un-intercepted, run `kuro build
//sdk:sdk` in zeromatter and iterate on any failures.

### Expected Failure Points

1. **Extension `.bzl` load failures** — Missing cells, unresolvable imports
2. **Repository rule execution failures** — `http_archive` download issues,
   missing `sha256` attributes
3. **Starlark API gaps** — Methods or attributes the real extension code uses
   that kuro doesn't implement
4. **Label resolution in module_ctx** — `path(Label)`, `which()`, `execute()`
   edge cases

### Approach
This is iterative. Run the build, read the first error, fix it, repeat. Each
fix should be minimal and targeted.

### Progress (2026-03-20)

#### Bugs Fixed (multi_package example now builds):
1. **AttrValue::Dict → Starlark struct (should be dict)**: `repository_ctx.rs` used `AllocStruct`
   for dict attrs, breaking `rctx.attr.globals.items()`. Fixed: use `AllocDict`.
2. **Starlark errors silently fell through to stub**: `repository_execution.rs` logged warnings
   and fell through to native executor. Fixed: propagate error.
3. **Canonical name prefix wrong for transitive modules**: `pre_compute_extension_repo_cells`
   and `build_canonical_names` always used root module name. Fixed: extract owning module from
   extension ID (`@bazel_features//...` → `bazel_features`).
4. **Missing top-level Bazel globals**: `CcSharedLibraryHintInfo`, `CcSharedLibraryInfo`,
   `PackageSpecificationInfo`, `RunEnvironmentInfo` were only in `cc_common` module, not
   top-level. Fixed: added `register_bazel_provider_globals()` and created `CcSharedLibraryHintInfo`.
5. **`module_extension()` required named `implementation`**: `rules_cc` passes it positionally.
   Fixed: removed `require = named` constraint.

#### Bugs Fixed (zeromatter progress, 2026-03-24):
6. **Tuples not captured as lists in RepoSpec attrs**: `starlark_to_repo_attr_value()` only checked
   `ListRef::from_value()` which doesn't match tuples. Added `TupleRef::from_value()` handling.
   This fixed `bazel_features` globals_repo where `attr.string_list_dict()` values were tuples.
7. **Canonical name prefix wrong for cross-module extension usage**: `pre_compute_extension_repo_cells`
   used the *using* module's name, not the *owning* module's name. E.g., `bazelrc-preset.bzl` using
   `@bazel_features//private:extensions.bzl` got prefix `bazelrc-preset.bzl` instead of `bazel_features`.
   Fixed: extract owning module from `extension_bzl_file` (`@bazel_features//...` → `bazel_features`).
8. **Module name in aggregation used cell name with version**: `parsed_modules` key was cell name
   `bazel_features+1.42.0` instead of declared module name `bazel_features`. This caused extension ID
   mismatch between self-referencing (`//path:ext.bzl`) and cross-referencing (`@bazel_features//path:ext.bzl`)
   modules. Fixed: use `dep_parsed.module.name` as key.
9. **Missing `py_internal` global**: `rules_python` .bzl files reference `py_internal` which is a Bazel
   native global. Added `PyInternalStub` struct with stub attribute methods.
10. **Label tag values returned as strings**: `SerializedTagValue::Label(s)` was allocated as a plain
    string, breaking `type(value) == "Label"` checks and `str()` canonical form. Fixed: allocate as
    `BazelLabel::parse(s)` so `type()` returns `"Label"` and `str()` includes `@@` prefix.

#### Current blocker (2026-03-24):
- `Select | dict` merge operation not supported. `rules_python` py_executable.bzl uses
  `default | config_settings` where `default` is `select({...})` and `config_settings` is a dict.
  This requires implementing dict union on Select values in Starlark.

#### Verified:
- `kuro build //app:calculator` in `examples/multi_package` — **BUILD SUCCEEDED**
- Extension repos materialized with real content:
  - `bazel_features+version_extension+bazel_features_version/version.bzl`: `version = '9.0.0'`
  - `bazel_features+version_extension+bazel_features_globals/globals.bzl`: real `globals = struct(...)`
  - `rules_cc+compatibility_proxy+cc_compatibility_proxy` created via real DICE execution

### Success Criteria

#### Automated Verification:
- [ ] `kuro build //sdk:sdk` in zeromatter progresses past extension execution
- [x] Extension repos are materialized in `bazel-external/` with real content
- [ ] `@crates` hub repo contains `defs.bzl` with real crate dependency data

#### Manual Verification:
- [x] Verify extension execution logs show real `.bzl` evaluation (not stubs)
- [ ] Verify downloaded crate sources match Cargo.lock versions
- [ ] Compare generated BUILD files with what `bazel build` produces

---

## Anti-Patterns to Avoid

### DO NOT write synthetic Rust code that reimplements extension logic

If an extension fails to execute, the fix should be one of:
1. **Fix the kuro Starlark interpreter** — add missing method, fix type handling
2. **Fix the kuro module_ctx** — add missing attribute or method
3. **Fix the kuro repository rule executor** — handle new repo rule type
4. **Fix cell resolution** — ensure the extension's dependencies are resolvable

The fix should NEVER be:
- Writing Rust code that parses Cargo.lock
- Writing Rust code that generates BUILD files for crates
- Writing Rust code that detects host toolchain paths
- Writing Rust code that simulates what a `.bzl` extension does
- Adding hardcoded symbol exports to stub repos

**If you find yourself writing >20 lines of Rust to produce content that an
extension `.bzl` file would normally produce, STOP.** You are going down the
stub rabbit hole. The correct fix is to make the real extension execute.

### DO NOT create synthetic repos for ANY extension

`generate_synthetic_repos_for_extension` should not exist. It was a stopgap.
If an extension fails to execute, the fix is to make it execute — not to add
another hardcoded Rust workaround. No exceptions.

Before proposing a synthetic repo, ask: "Does Bazel hardcode this in Java?" The
answer is always no (except `@bazel_tools` and `@local_config_platform`, which
are bundled cells, not synthetic repos). If Bazel runs `.bzl` code to produce
it, kuro must too.

### DO keep graceful degradation for non-critical extensions

Test-only extensions, telemetry extensions, and similar non-critical repos
should fail gracefully (empty stubs from the `_` catch-all in the old code
are now handled by DICE returning empty specs) rather than blocking the build.
The `execute_extension` fallback to empty specs in
`ConcreteModuleExtensionExecutor` is correct for these cases — but it should
be the DICE execution failing gracefully, not a synthetic repo being
pre-materialized.

## References

- Extension execution DICE: `app/kuro_bzlmod/src/extension_execution_dice.rs`
- Module context: `app/kuro_interpreter_for_build/src/module_ctx.rs`
- Extension executor: `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`
- Repo spec capture: `app/kuro_bzlmod/src/repo_spec.rs`
- Repository rule hook: `app/kuro_interpreter_for_build/src/repository_rule.rs:403-424`
- Synthetic repos: `app/kuro_bzlmod/src/synthetic_repos.rs`
- Cell registration: `app/kuro_common/src/legacy_configs/cells.rs`
