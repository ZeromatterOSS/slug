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

#### Additional bugs fixed (2026-03-24 continued):
11. **Select | dict merge**: Implemented `bit_or` for StarlarkSelector.
12. **Transitive deps of overridden modules not resolved**: archive_override modules' deps were
    never queued for BFS resolution. Fixed: queue overridden modules' deps after override resolution.
13. **Stub repos for failed extensions**: When extensions fail gracefully, create stub repos with
    BUILD.bazel + defs.bzl so dependent extensions can still load.
14. **module_ctx.read() watch parameter**: starlark macro keeps underscore prefix. Renamed `_watch` to `watch`.
15. **download() output param must be positional**: Changed from named-only to positional.
16. **DownloadToken for async downloads**: When `block=False`, return token with `.wait()` method.
17. **BazelModuleTags unknown tag classes**: Return empty list for classes not used by module.
18. **TagInstance with None defaults**: Missing tag attrs now return None instead of erroring.
19. **module_ctx.execute() Label objects**: Convert Labels to strings via `to_str()`.

#### Bugs fixed (2026-03-25):
20. **`mctx.execute()` with RepositoryPath args**: `mctx.path(Label)` returns a `RepositoryPath` object
    whose Display format is `<repository_path /path>`, not a valid path. Fixed: `execute()` now checks
    for `RepositoryPath` type and extracts the path string via `path_str()`.
21. **Tag class defaults not applied**: Missing tag attrs returned `None` instead of declared defaults
    (e.g., `attr.string_list_dict()` should default to `{}`). Fixed: extract defaults from
    `FrozenStarlarkTagClass.attrs()` and apply to `SerializedTag` before evaluation. For attrs without
    explicit defaults, synthesize type-appropriate empties (list→[], dict→{}, string→"").
22. **`set.difference_update()` missing**: Starlark `set` type lacked `difference_update()`. Added
    in-place method to `starlark-rust/starlark/src/values/types/set/methods.rs`.
23. **`set.intersection_update()` missing**: Similarly added `intersection_update()` method.

#### Bugs fixed (2026-03-25 continued):
24. **`Missing parameter proc_macro_deps`**: `bazel_features.globals.macro` evaluated truthy (kuro has
    `macro()` builtin), causing rules_rust to use `_symbolic_rule_wrapper` which requires
    `proc_macro_deps` as a required param. Fixed: `FrozenStarlarkMacroCallable::invoke()` now stores
    the `attrs` dict from `macro()` and applies attr defaults for missing parameters.
25. **tar.zst decompression**: LLVM toolchain downloads use `.tar.zst` (Zstandard). Added `zstd` crate
    and `extract_tar_zst()` to `fetch.rs`, `repository_ctx.rs`, and `repository_executor.rs`.
26. **Dynamic cell resolution for extension spoke repos**: Crate spoke repos (e.g.,
    `crates__tempfile-3.26.0`) aren't in `use_repo()` so they weren't registered as cells. Added:
    - Global dynamic cell registry (`DYNAMIC_EXTENSION_CELLS`) populated during extension execution
    - `CellAliasResolver::resolve()` fallback: tries `X+Y+{alias}` for extension repo contexts
    - `CellResolver::get()` fallback: checks dynamic registry (exact + suffix match) and scans
      `bazel-external/` directory for matching repos
    - Extension repos register all RepoSpec repos in the dynamic registry during materialization

#### Current blocker (2026-03-25):
- **Spoke repos not being created during extension execution**. Investigation revealed:
  - The `crate` extension executes and produces the hub repo `rules_rs+crate+crates` with
    real aliases (7752 lines of BUILD.bazel). The hub references `@crates__tempfile-3.26.0//:tempfile`.
  - But spoke repos (individual crate downloads) are NOT materialized. The eager materialization
    loop in `execute_extension()` never runs because the extension execution path goes through
    `get_file_ops_delegate()` → `ModuleExtensionExecutionKey::compute()` which returns the
    `ModuleExtensionResult` to `get_file_ops_delegate`. That function only materializes the
    SPECIFIC repo being requested (the hub), not all repos from the extension.
  - The eager materialization in `execute_extension()` fires from `MODULE_EXTENSION_EXECUTOR_IMPL`
    which runs the extension's `.bzl` code. But analysis doesn't wait for all spoke repos —
    it only triggers lazy materialization for repos that are actually `ExtensionRepoCellSetup` cells.
  - **Root cause**: Spoke repos are not in `use_repo()`, so they have no `ExtensionRepoCellSetup`.
    They can't be lazily materialized because the cell resolver doesn't know about them. The
    dynamic cell resolution finds the directory but there's no content because the extension's
    `ModuleExtensionResult` has the RepoSpecs but they're never executed.
  - **Fix applied**: `get_file_ops_delegate()` now iterates ALL repos from the extension result
    and materializes them via `ExtensionRepoExecutionKey`. This creates 1234 spoke repos from
    the crate extension. But BUILD.bazel generation inside `crate_repository` rule still fails.
27. **`repository_ctx.execute()` drops Label/RepositoryPath args**: The args list used
    `filter_map(unpack_str)` which silently drops non-string values. Fixed: now handles
    `RepositoryPath` (via `absolute_path()`) and `Label` (via `resolve_label_to_path()`).
28. **`resolve_label_to_path` improved for kuro repo layout**: Now checks dynamic cell registry,
    `bazel-external/` directory, and scans for versioned repo names. Needed for `run_toml2json`
    which calls `ctx.execute([Label("@toml2json_linux_amd64//file:downloaded"), toml_file])`.

#### Bugs fixed (2026-03-25, session 2):
29. **Incomplete materialization check**: `get_file_ops_delegate()` checked `!source_path.exists()`
    (directory existence) instead of `.kuro_repo_complete`. Partially materialized repos (source
    downloaded but no BUILD.bazel) were skipped on re-execution. Fixed: check `.kuro_repo_complete`.
30. **`repository_ctx.read()` only accepted strings**: `ctx.read(repo_path)` failed when passed a
    `RepositoryPath` (from `ctx.path()`). Fixed: accept `RepositoryPath` (via `absolute_path()`)
    and `Label` (via `resolve_label_to_path()`).
31. **`repository_ctx.path()` didn't resolve label-like strings**: When `rctx.attr.build_file`
    returns `"@@rules_rust//path:file"` (a string, not a Label object), `path()` stored it
    literally. Fixed: detect `@@`/`@` + `//` patterns and resolve via `resolve_label_to_path()`.
32. **`resolve_label_to_path()` used workspace_root (repo dir) instead of project root**: For repos
    inside `bazel-external/`, workspace_root is the repo dir, not the project root. The scan for
    versioned directories failed. Fixed: prioritize `DYNAMIC_PROJECT_ROOT` for all scans.
33. **Native http_archive `build_file` label not resolved**: The native executor used naive string
    manipulation for label-like `build_file` values. Fixed: added `resolve_build_file_label()` that
    scans `bazel-external/` for matching repos.
34. **`package_metadata` not accepted as rule parameter**: rules_rust's `_symbolic_rule_wrapper`
    passes `package_metadata` to rules. Fixed: added as internal attribute (ID 15).
35. **Bare filename in `attr.label()` default rejected in `.bzl` files**: `attr.label(default="LICENSE")`
    in rules_license failed in strict mode. Fixed: allow bare names to fall through to relative
    label resolution even in strict mode.
36. **`stripPrefix` camelCase not accepted**: `download_and_extract(stripPrefix=...)` from rules_perl
    failed. Fixed: added `stripPrefix` parameter as alias for `strip_prefix`.
37. **Empty `integrity` string caused "Invalid integrity format"**: `get_optional_string("integrity")`
    returned `Some("")` instead of `None`. Fixed: filter empty strings.

#### Verified:
- `kuro build //app:calculator` in `examples/multi_package` — **BUILD SUCCEEDED**
- **`@crates` hub repo generated with real content** — `defs.bzl` contains `all_crate_deps`,
  `aliases`, `data.bzl` contains `DEP_DATA` with real crate dependency data from Cargo.lock.
- **1230/1235 spoke repos materialized with BUILD.bazel** (5 failures: local/patched crates)
- **`rules_rust_tinyjson` and `rrra` extension repos now generate BUILD.bazel** via
  `crates_vendor_remote_repository` Starlark repo rule
- Extension repos materialized with real content:
  - `bazel_features+version_extension+bazel_features_version/version.bzl`: `version = '9.0.0'`
  - `bazel_features+version_extension+bazel_features_globals/globals.bzl`: real `globals = struct(...)`
  - `rules_cc+compatibility_proxy+cc_compatibility_proxy` created via real DICE execution

38. **BCR overlay files not applied**: BCR modules use `overlay` field in `source.json` to add
    BUILD.bazel and other files on top of extracted source archives. Added `overlay` field to
    `SourceInfo` and `apply_overlays()` to `SourceFetcher`. Files are fetched from
    `{registry}/modules/{name}/{version}/overlay/{path}`.

39. **Bare source paths with slashes in label coercion**: Paths like `"crypto/aes/file.pl"` used as
    `attrs.dict(attrs.dep(), ...)` keys need label construction via `TargetNameRef::unchecked_new()`
    since the pattern parser rejects slashed target names.

#### Current blocker (2026-03-25, session 2):
- `rules_rust_tinyjson//:src/parser.rs` — dep coercion succeeds for slashed source file paths,
  but analysis fails with "Unknown target" because Buck2 doesn't create implicit source file
  targets like Bazel does. Source files in `srcs = glob(["src/*.rs"])` get coerced as dep labels
  instead of source files. Fix requires either:
  1. Implicit source file targets during package evaluation (Bazel parity)
  2. Making `one_of(dep, source)` prefer source for slashed bare names

### Success Criteria

#### Automated Verification:
- [x] `kuro build //sdk:sdk` in zeromatter progresses past extension execution
- [x] Extension repos are materialized in `bazel-external/` with real content
- [x] `@crates` hub repo contains `defs.bzl` with real crate dependency data

#### Manual Verification:
- [x] Verify extension execution logs show real `.bzl` evaluation (not stubs)
- [ ] Verify downloaded crate sources match Cargo.lock versions
- [ ] Compare generated BUILD files with what `bazel build` produces

---

## Phase 4: Label-to-Path Resolution in module_ctx

### Overview

In Bazel, `module_ctx.path(Label("@repo//:file"))` and
`module_ctx.execute([Label("@repo//:tool"), arg1])` resolve Label arguments to
**filesystem paths** by triggering materialization of the referenced repository.
This is how the `crate` extension from `rules_rs` runs `toml2json` — it calls
`ctx.execute([Label(toml2json), toml_file])` where `toml2json` is a Label
pointing to a tool inside `@rules_rs`.

Currently, kuro's `ModuleContext` has no knowledge of project root, cell paths,
or how to materialize repos. Label arguments get converted to canonical strings
like `@@rules_rs//rs/private:toml2json` which aren't valid filesystem paths.

### How Bazel Does It (Reference Architecture)

In Bazel, both `module_ctx` and `repository_ctx` inherit label resolution from
`StarlarkBaseExternalContext`:

```
path(Label("@repo//:bin/cargo"))
  → getPathFromLabel(label)
    → RepositoryUtils.getRootedPathFromLabel(label, env)
      → env.getValue(PackageLookupValue.key(@repo//))
        → PackageLookupFunction requests RepositoryDirectoryValue(@repo)
          → RepositoryFunction runs: downloads, extracts, runs repo rule
          → Returns materialized path: output_base/external/@repo/
        → Returns RootedPath: output_base/external/@repo/bin/cargo
      → If null (not yet computed): throws NeedsSkyframeRestartException
        → Skyframe re-queues extension evaluation after repo is ready
```

Key behaviors:
1. **Implicit repo materialization**: accessing a Label from another repo
   automatically fetches/materializes that repo
2. **Skyframe restart**: if the dependency isn't ready, the extension is
   suspended and re-run (kuro can't do this — see Design section)
3. **Labels must be source files**: only files that exist in the fetched repo
   tree are valid; build outputs are NOT supported
4. **Same code path for `repository_ctx` and `module_ctx`**: they share the
   implementation via `StarlarkBaseExternalContext`

### Design: Kuro's Approach

Kuro cannot do Skyframe-style restarts because Starlark evaluation is
synchronous within `eval_function()`. Instead, we use a **pre-resolution +
on-demand materialization** approach:

1. **Thread project root + cell path map into `ModuleContext`**: Before entering
   Starlark evaluation, `try_execute_starlark()` obtains the `CellResolver` from
   DICE and builds a map of `cell_name → filesystem_path`. This map, along with
   the project root, is stored on `ModuleContext`.

2. **Resolve Labels via cell path lookup**: When `module_ctx.path(Label)` is
   called, extract the repo name from the label, look it up in the cell path
   map, and construct the filesystem path.

3. **On-demand extension repo materialization**: If the repo is an extension
   repo (lives under `bazel-external/`) and hasn't been materialized yet (no
   `.kuro_repo_complete` marker), trigger synchronous materialization. This is
   the kuro equivalent of Bazel's `RepositoryDirectoryValue` dependency.

4. **Same resolution for `execute()` and `path()`**: Both methods use the same
   label resolution function, matching Bazel's shared `StarlarkBaseExternalContext`
   pattern.

### Changes Required

#### 1. Add project root and cell path map to `ModuleContext`
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

Add fields to `ModuleContext`:
```rust
pub struct ModuleContext {
    modules: Vec<SerializedModule>,
    root_module_has_non_dev_dependency: bool,
    working_dir: Option<Arc<PathBuf>>,
    delete_on_close: bool,
    // NEW: for Label-to-path resolution
    project_root: Option<PathBuf>,
    cell_paths: HashMap<String, PathBuf>,  // cell_name → absolute path
}
```

Add a builder method:
```rust
pub fn with_label_resolution(
    mut self,
    project_root: PathBuf,
    cell_paths: HashMap<String, PathBuf>,
) -> Self {
    self.project_root = Some(project_root);
    self.cell_paths = cell_paths;
    self
}
```

#### 2. Build cell path map in `try_execute_starlark()`
**File**: `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`

After obtaining the `CellResolver` (line ~186), build the cell path map:
```rust
let cell_resolver = ctx.get_cell_resolver().await?;
let io = ctx.global_data().get_io_provider();
let project_root = io.project_root().root().to_path_buf();

// Build cell_name → absolute_path map for Label resolution
let mut cell_paths = HashMap::new();
for cell_name in cell_resolver.cells() {
    if let Ok(cell_instance) = cell_resolver.get(cell_name) {
        let rel_path = cell_instance.path().as_project_relative_path();
        cell_paths.insert(
            cell_name.as_str().to_owned(),
            project_root.join(rel_path.as_str()),
        );
    }
}
```

Then in `execute_extension()`, chain this onto the module_ctx:
```rust
let module_ctx = build_module_context(aggregated, root_module_name)
    .with_temp_working_dir(working_dir.clone())
    .with_label_resolution(project_root, cell_paths);
```

This requires restructuring so the cell_paths are built in `execute_extension()`
(which has DICE access) rather than after `build_module_context()` is called.

#### 3. Implement `resolve_label_to_filesystem_path()` on `ModuleContext`
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

Add a method to `ModuleContext` that resolves a label string or BazelLabel to
an absolute filesystem path:

```rust
fn resolve_label_to_filesystem_path(&self, label_str: &str) -> Option<PathBuf> {
    let project_root = self.project_root.as_ref()?;

    // Parse @@repo//pkg:target format
    let stripped = label_str.trim_start_matches('@');
    let (repo, rest) = stripped.split_once("//")?;
    let (pkg, target) = rest.split_once(':').unwrap_or((rest, rest.rsplit('/').next()?));

    if repo.is_empty() {
        // Root repo: project_root/pkg/target
        Some(project_root.join(pkg).join(target))
    } else {
        // External repo: look up in cell_paths
        // Try exact match first, then try with version suffix patterns
        self.cell_paths.get(repo)
            .or_else(|| {
                // Try matching cell names that start with repo+ (versioned)
                self.cell_paths.iter()
                    .find(|(k, _)| k.starts_with(&format!("{}+", repo)))
                    .map(|(_, v)| v)
            })
            .map(|repo_path| {
                if pkg.is_empty() {
                    repo_path.join(target)
                } else {
                    repo_path.join(pkg).join(target)
                }
            })
    }
}
```

#### 4. Update `path()` to use cell-based resolution
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

Replace the current `resolve_label_to_path()` call with the new method:
```rust
// In the Label branch of path():
if let Some(resolved) = this.resolve_label_to_filesystem_path(&label_str) {
    let path_str = resolved.to_string_lossy().to_string();
    return Ok(heap.alloc(RepositoryPath::with_base_dir(path_str.clone(), resolved)));
}
// Fallback to working_dir-relative resolution for compatibility
```

#### 5. Update `execute()` to resolve Label arguments to paths
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

In the `execute()` method, when building the args list, resolve Labels:
```rust
let args: Vec<String> = if let Some(list) = ListRef::from_value(arguments) {
    list.iter()
        .map(|v| {
            if v.get_type() == "Label" {
                let label_str = v.to_str();
                this.resolve_label_to_filesystem_path(&label_str)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(label_str)
            } else {
                v.unpack_str()
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| v.to_str())
            }
        })
        .collect()
} else { ... };
```

#### 6. Handle on-demand extension repo materialization (optional, may defer)
**File**: `app/kuro_interpreter_for_build/src/module_ctx.rs`

When `resolve_label_to_filesystem_path()` returns a path under `bazel-external/`
and the directory doesn't exist or lacks `.kuro_repo_complete`, we need to
trigger materialization. Since we don't have DICE access inside the Starlark
eval, there are two options:

**Option A (simpler, recommended for initial implementation):**
Require that all referenced repos are already materialized. Extension repos
that are needed by other extensions should be materialized as part of the DICE
dependency chain — i.e., the extension execution for the dependency should
complete before the dependent extension starts.

For the `crate` extension's use of `Label("@rules_rs//rs/private:toml2json")`:
this is a **source file** inside the `rules_rs` repo (fetched via
`archive_override`). It should already exist at
`bazel-external/rules_rs+override/rs/private/toml2json`. No extension repo
materialization is needed — the repo is a regular bzlmod cell, not an extension
repo.

**Option B (full Bazel parity, future work):**
Store a callback/handle that can trigger synchronous DICE execution for repo
materialization. This would require threading a `tokio::Runtime` handle and a
DICE transaction reference into `ModuleContext` so the synchronous Starlark
code can block on an async materialization. This is complex and should only be
done if Option A proves insufficient.

### Key Insight: The toml2json Case

The current blocker `ctx.execute([Label(toml2json), toml_file])` is actually
**Option A** — `toml2json` is a source file within the `rules_rs` repo, not an
extension-generated repo. The `rules_rs` module was fetched via
`archive_override` and should already exist at
`bazel-external/rules_rs+override/`. The only missing piece is that
`ModuleContext.resolve_label_to_filesystem_path()` needs to know where
`rules_rs` lives on disk. Threading the cell path map solves this.

### Additional changes made during Phase 4 implementation

Beyond the planned changes, the following were also needed:

20. **`use_repo_rule()` invocations in MODULE.bazel now recorded**: `RepoRuleProxy::invoke()`
    was a no-op. Now records invocations in `ParsedModuleFile.repo_rule_invocations`.
    `cells.rs` processes them into cells and materializes via `execute_repository_rule()`.
    This enabled `http_file(name="toml2json_linux_amd64", ...)` to create real repos.
21. **Eager materialization of ALL extension repos via DICE**: After an extension executes,
    all its RepoSpecs are materialized via `ExtensionRepoExecutionKey::compute()`. This
    ensures repos created by one extension (e.g., `cargo_linux_x86_64_1_92_0` from
    `toolchains`) are on disk when referenced by another extension (e.g., `crate`).
22. **Repository rule attr defaults applied to RepoSpec**: `coerced_attr_to_repo_attr_value()`
    extracts default values from the rule's `attrs` definition. Fixes `cargo_repository`
    which relies on `urls = attr.string_list(default = DEFAULT_STATIC_RUST_URL_TEMPLATES)`.
23. **`http_file` puts file in `file/` subdirectory**: Matches Bazel convention for
    `Label("@repo//file:downloaded")`. Fixed executable permission via `get_bool()`.
24. **`repository_ctx` underscore param names fixed**: `_watch`, `_canonical_id`, `_auth`,
    `_headers`, `_block` all kept underscore in Starlark param name. Renamed.
25. **`download_and_extract` params allow named usage**: `sha256`, `strip_prefix` changed
    from positional-only to allow named.
26. **`path.exists` is now an attribute**: `#[starlark(attribute)]` annotation makes
    `ctx.path(file).exists` work without `()` (Bazel uses structField property).
27. **`readdir()` returns `RepositoryPath` objects**: Was returning strings, breaking
    `.basename` and `.get_child()` on results.
28. **`report_progress()` stub on module_ctx**: No-op stub for progress reporting.
29. **tar.xz decompression support**: Rust toolchain archives use `.tar.xz`. Added
    `extract_tar_xz()` using `xz2` crate.
30. **`resolve_label_to_filesystem_path` fallback scan**: When repo not in cell_paths,
    scan `bazel-external/` for `*+repo_name` directories (extension repos not in `use_repo()`).

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes
- [x] `cargo test -p kuro_interpreter_for_build` — all tests pass
- [x] `module_ctx.path("some/path")` still works (string case)
- [x] `module_ctx.path(Label("@repo//:file"))` resolves to correct filesystem path
- [x] `module_ctx.execute([Label("@repo//:tool")])` resolves label to path before exec

#### Manual Verification:
- [x] `kuro build //sdk:sdk` in zeromatter progresses past `toml2json` execution
- [x] The `crate` extension can locate and run `toml2json` from `@rules_rs`
- [x] Extension repos that depend on other extension repos' source files work

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
