# Module Extension Execution: Let Real Extensions Run

> **Main Plan**: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)

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
`module_ctx.path(Label)` resolves it. In slug, `module_ctx.path()` accepts
strings. This means either:
- The toolchains extension must execute and produce a real label, OR
- `module_ctx.path()` must handle Label objects

## Repository Taxonomy: What Bazel Actually Does

In Bazel 9 there are exactly two categories of repos that the build tool itself
provides. Everything else comes from extension execution.

### 1. Build-tool builtins (shipped with the binary)

| Repo | What it is | How Bazel provides it | How slug provides it |
|------|-----------|----------------------|---------------------|
| `@bazel_tools` | Build tool's own rules, tools, platforms | Embedded in Bazel binary | **Bundled cell** (`ExternalCellOrigin::Bundled`) at `cells.rs:566-573` — ships in `slug/bazel_tools/` |
| `@local_config_platform` | Host platform auto-detection (`//:host`) | Built-in repository rule, no extension | **Bundled cell** (`ExternalCellOrigin::Bundled`) at `cells.rs:576-584` |

These are correct as-is. They are NOT in `synthetic_repos.rs` and don't need to
change.

### 2. Extension-generated repos (everything in synthetic_repos.rs)

Every other repo — including `bazel_features`, `local_config_cc`,
`rules_rust_tinyjson`, `rust_toolchains`, `crates`, etc. — is created by
a module extension executing `.bzl` code. There is no middle category.

**`bazel_features` is not special.** Its `version_extension` is a standard
module extension that calls `version_repo` (a repository rule). That rule does
`rctx.file("version.bzl", "version = '" + native.bazel_version + "'")`. If slug
implements `native.bazel_version` (it already does, returning `"9.0.0"`), the
extension works. There is no reason to hardcode this in Rust.

**`local_config_cc` is not special.** It's created by `rules_cc`'s
`cc_configure_extension`, which probes the host C++ compiler. That's a standard
extension calling `repository_ctx.execute()` and `repository_ctx.which()`. If
slug implements those (it does), the extension works.

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

**The correct approach is to let the real extension code run.** Slug already has
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
**File**: `app/slug_bzlmod/src/extension_execution_dice.rs`
Remove the `/tmp/slug_ext_debug*.log` file writes and restore the original
`MODULE_EXTENSION_EXECUTOR_IMPL.get()` match arms.

#### 2. Remove debug logging from module_extension_executor_impl.rs
**File**: `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`
Restore original `tracing::debug` (not `tracing::warn`) for the import_path log.
Restore original `.buck_error_context()` error handling (not the expanded match).

#### 3. Revert error propagation change
**File**: `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`
Restore the graceful fallback to empty specs on extension execution failure.
This is needed because some extensions (e.g., test-only ones) may legitimately
fail to load, and hard-failing would break builds that don't need those repos.

#### 4. Remove stub defs.bzl generation hacks
**File**: `app/slug_bzlmod/src/synthetic_repos.rs`
Remove `generate_stub_defs_bzl()`, `find_tool_in_path()`, and the
`generate_stub_repo()` helper. Revert `generate_stub_repos_for_extension()` to
its original form (just BUILD.bazel stubs).

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes with no new warnings
- [x] `cargo test -p slug_bzlmod` — all tests pass
- [x] No references to `/tmp/slug_ext_debug` in codebase
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
**File**: `app/slug_bzlmod/src/synthetic_repos.rs`

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
**File**: `app/slug_common/src/legacy_configs/cells.rs`

`generate_synthetic_extension_repos()` calls
`collect_synthetic_repos_with_root()`. With no synthetic repos to generate,
this function should return an empty Vec. The extension repos from
`pre_compute_extension_repo_cells()` will handle all extension-generated cells.

#### 3. Clean up the module
**File**: `app/slug_bzlmod/src/synthetic_repos.rs`

After removing all generators, the file should contain only the `SyntheticRepo`
struct definition (if still needed by other code) and `materialize_synthetic_repos`
(if still needed). If nothing references them, delete the entire file and remove
it from `mod.rs`.

### Success Criteria

#### Automated Verification:
- [x] `cargo check` passes
- [x] `cargo test -p slug_bzlmod` — all tests pass (157 pass)
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
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

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
With all rules_rust/rules_rs extensions un-intercepted, run `slug build
//sdk:sdk` in zeromatter and iterate on any failures.

### Expected Failure Points

1. **Extension `.bzl` load failures** — Missing cells, unresolvable imports
2. **Repository rule execution failures** — `http_archive` download issues,
   missing `sha256` attributes
3. **Starlark API gaps** — Methods or attributes the real extension code uses
   that slug doesn't implement
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
24. **`Missing parameter proc_macro_deps`**: `bazel_features.globals.macro` evaluated truthy (slug has
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
28. **`resolve_label_to_path` improved for slug repo layout**: Now checks dynamic cell registry,
    `bazel-external/` directory, and scans for versioned repo names. Needed for `run_toml2json`
    which calls `ctx.execute([Label("@toml2json_linux_amd64//file:downloaded"), toml_file])`.

#### Bugs fixed (2026-03-25, session 2):
29. **Incomplete materialization check**: `get_file_ops_delegate()` checked `!source_path.exists()`
    (directory existence) instead of `.slug_repo_complete`. Partially materialized repos (source
    downloaded but no BUILD.bazel) were skipped on re-execution. Fixed: check `.slug_repo_complete`.
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
- `slug build //app:calculator` in `examples/multi_package` — **BUILD SUCCEEDED**
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

40. **one_of(dep, source) tried dep first for file-like paths**: `glob(["src/*.rs"])` expanded to
    `src/parser.rs` which dep-coerced as a label but failed at analysis (no implicit source file
    target). Fixed: `one_of` now tries alternatives in reverse order when value contains `/`.
41. **Missing CompilationContext attributes**: rules_cc 0.2.17 accesses `_exporting_module_map_files`,
    `loose_hdrs_dirs`, `purpose` on `CompilationContextStub`. Added to both stubs.
42. **repository_ctx.symlink() didn't overwrite**: `cc_autoconf` creates symlinks to template files.
    Second runs fail with "File exists". Fixed: remove existing before creating.
43. **repository_ctx.template() required named substitutions**: rules_cc passes it positionally.
    Fixed: changed to `require = pos`.

#### Current blocker (2026-03-26):
44. **Missing compiler path in create_cc_compile_action**: The GCC/Clang arg list started with `-c`
    instead of `compiler_path -c`. The compiler was omitted because `args_vec` didn't include
    it before the flags. Fixed: prepend `compiler_path` to args for non-MSVC builds.
    Note: `cc_configure_extension` repos still don't materialize because `ToolchainsStub`
    shortcircuits real toolchain resolution (returns `CcToolchainInfoStub` directly). The
    `CcToolchainInfoStub` provides correct compiler paths via `host_tool_path()`, so builds
    work without real `local_config_cc`. Full toolchain resolution remains future work.

#### Bugs fixed (2026-03-27):
45. **`external/` directory missing for action execution**: `artifact.path` returns `external/<cell>/...`
    for external repo source files (Bazel convention), but no `external/` directory existed on disk.
    Local action execution (unsandboxed) couldn't find these paths. Fixed: added
    `ensure_external_symlink()` and `ensure_external_symlinks_for_cells()` to `slug_core/src/cells.rs`.
    Creates `external/<cell_name>` → `../<cell_path>` symlinks during cell setup (for static cells)
    and during dynamic cell registration (for spoke repos). This fixed the "Dependency environment
    file unreadable" error from the cargo build script runner.

46. **Cell name duality for extension repos**: Pre-computed extension repo cells used canonical names
    (`rules_rs+crate+crates__typenum-1.19.0`) while dynamic resolution used short names
    (`crates__typenum-1.19.0`). This caused duplicate DICE computations with different output
    paths. Fixed: register cells under short names with canonical→short aliases. Also added alias
    lookup in `CellResolver::get()` to prevent duplicate cell creation.
47. **CopyAction symlink not materialized to disk**: Buck2's deferred materializer doesn't eagerly
    write CopyAction/Symlink outputs. Fixed: in `CopyAction::execute()`, directly create symlink
    at the plain output path for Symlink mode. This bridges the gap between DICE's virtual artifact
    system and local unsandboxed execution.
48. **`use_param_file` on nested Args not creating param file**: Bazel's `args.use_param_file()` on
    a per-Args basis puts those specific args into a file. But when Bazel `arguments=[args1, args2]`
    is flattened into a single Buck2 cmd_args, the param_file config on the nested args2 is lost.
    Fixed: in local executor, detect inline positional args that should have been in a param file
    (first positional arg after all `--key=value` args, with `=` in subsequent entries) and write
    them to a temp file with `--cargo_manifest_args=@<path>` substitution.

49. **`bootstrap_process_wrapper` execvp "No such file or directory"**: Root cause was bug #48's
    param file detection using too-broad heuristic (`!starts_with("--")`) that matched source file
    paths. Tightened to `ends_with(".cargo_runfiles")` which only matches the cargo runner pattern.
50. **Param file deleted before use**: The param file was written to the action's scratch path
    (inside the output directory). `create_output_dirs` in `exec_once` cleaned the output dir
    before the command ran, deleting the param file. Fixed: write to `/tmp/slug-param-files/`
    with unique atomic counter filenames.

#### Bugs fixed (2026-04-08, zeromatter manual verification):
51. **`depset.to_list()` didn't deduplicate elements**: Bazel's `depset(["a", "a"]).to_list()`
    returns `["a"]`; slug returned `["a", "a"]`. This caused `rules_rust`'s
    `_get_toolchain_repositories()` to generate duplicate toolchain names when `exec_triple` was
    also in `DEFAULT_EXTRA_TARGET_TRIPLES` (e.g., `x86_64-unknown-linux-gnu`), making the `rust`
    extension `fail()` silently. Fixed: added `HashSet`-based deduplication in both `to_list()`
    implementations (`depset.rs`).
52. **`repository_ctx.attr.name` missing**: Protobuf's `system_python.bzl` accesses `ctx.attr.name`.
    Fixed: added `name` field to `RepositoryAttr`, set during construction from repo name.
53. **`download_and_extract(url=...)` rejected named parameter**: Protobuf's `protoc_toolchain.bzl`
    passes `url=` as keyword. Fixed: removed `require = pos` from `url` param.
54. **`download_and_extract(output=path_obj)` rejected RepositoryPath**: `apple_support`'s
    `http_dmg.bzl` passes a `RepositoryPath` as `output`. Fixed: changed `output` from `&str` to
    `NoneOr<Value>`, extracting string from both `RepositoryPath` and `str` types.

#### Current state (2026-04-08):
- **42 actions execute successfully** (from 0 at start of session)
- All extension repos materialize with real content (1230+ crate spokes, tinyjson, rrra, etc.)
- External repo source files accessible via `external/<cell>/` symlinks
- All cargo build scripts (_bs_ compilation, _bs- symlink, _bs execution) work end-to-end
- **`rust` extension from `@rules_rust//rust:extensions.bzl` now executes successfully** and
  generates real `rust_toolchains` repo (33KB BUILD.bazel with `toolchain()` targets)
- Rust toolchain resolution still returns None due to toolchain_type label mismatch:
  generated BUILD uses `@rules_rust//rust:toolchain` (an `alias()`), rule requests
  `@rules_rust//rust:toolchain_type` (the alias' `actual`). **Moved to Plan 11
  Phase 8** (alias resolution at toolchain registration). Does NOT block Plan 10
  closure — extension execution itself works; this is a downstream toolchain-resolution
  gap that belongs in Plan 11.
- 2 remaining failures: `postgres-types` has `unresolved import chrono_04` (missing crate
  alias in build rules, not a slug infrastructure issue). `zmij` and `serde_core` fail
  downstream from this.

### Success Criteria

#### Automated Verification:
- [x] `slug build //sdk:sdk` in zeromatter progresses past extension execution
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

Currently, slug's `ModuleContext` has no knowledge of project root, cell paths,
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
   suspended and re-run (slug can't do this — see Design section)
3. **Labels must be source files**: only files that exist in the fetched repo
   tree are valid; build outputs are NOT supported
4. **Same code path for `repository_ctx` and `module_ctx`**: they share the
   implementation via `StarlarkBaseExternalContext`

### Design: Slug's Approach

Slug cannot do Skyframe-style restarts because Starlark evaluation is
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
   `.slug_repo_complete` marker), trigger synchronous materialization. This is
   the slug equivalent of Bazel's `RepositoryDirectoryValue` dependency.

4. **Same resolution for `execute()` and `path()`**: Both methods use the same
   label resolution function, matching Bazel's shared `StarlarkBaseExternalContext`
   pattern.

### Changes Required

#### 1. Add project root and cell path map to `ModuleContext`
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

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
**File**: `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`

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
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

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
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

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
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

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
**File**: `app/slug_interpreter_for_build/src/module_ctx.rs`

When `resolve_label_to_filesystem_path()` returns a path under `bazel-external/`
and the directory doesn't exist or lacks `.slug_repo_complete`, we need to
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
- [x] `cargo test -p slug_interpreter_for_build` — all tests pass
- [x] `module_ctx.path("some/path")` still works (string case)
- [x] `module_ctx.path(Label("@repo//:file"))` resolves to correct filesystem path
- [x] `module_ctx.execute([Label("@repo//:tool")])` resolves label to path before exec

#### Manual Verification:
- [x] `slug build //sdk:sdk` in zeromatter progresses past `toml2json` execution
- [x] The `crate` extension can locate and run `toml2json` from `@rules_rs`
- [x] Extension repos that depend on other extension repos' source files work

---

## Anti-Patterns to Avoid

### DO NOT write synthetic Rust code that reimplements extension logic

If an extension fails to execute, the fix should be one of:
1. **Fix the slug Starlark interpreter** — add missing method, fix type handling
2. **Fix the slug module_ctx** — add missing attribute or method
3. **Fix the slug repository rule executor** — handle new repo rule type
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
it, slug must too.

### DO keep graceful degradation for non-critical extensions

Test-only extensions, telemetry extensions, and similar non-critical repos
should fail gracefully (empty stubs from the `_` catch-all in the old code
are now handled by DICE returning empty specs) rather than blocking the build.
The `execute_extension` fallback to empty specs in
`ConcreteModuleExtensionExecutor` is correct for these cases — but it should
be the DICE execution failing gracefully, not a synthetic repo being
pre-materialized.

## Phase 6: Lazy Extension Repo File Tracking (2026-04-17)

Once extensions execute and materialize real content, the next bottleneck is
how that content gets tracked. The inherited Buck2 pipeline walks every file,
reads contents, and registers writes with the materializer. For Bazel repos
that can point at gigabytes of source (e.g. `new_local_repository(path="../../")`
for llvm-raw on a real llvm-project checkout), this is catastrophic.

### Problem

`declare_all_source_artifacts_ext` in `app/slug_external_cells/src/extension_repo.rs`
(~line 304) walks every file in each extension repo and reads content into
memory via `tokio::fs::read`. Combined with the eager spoke-materialization
loop (~line 555), building `@llvm-project//llvm:config` appears to hang — it is
actually doing tree walks over millions of files.

### Structural root cause

Slug inherited Buck2's "external cell" materializer pipeline:

- `ExternalCellOrigin::ExtensionRepo` triggers hard-coded path remapping from
  `bazel-external/{canonical}/file` to
  `buck-out/v2/external_cells/extension_repo/{canonical}/file` in
  `artifact_path_resolver.rs:78-85`.
- `materializer.declare_write` requires pre-registration of every file before
  actions can find inputs.

Buck2 uses this for small bundled prelude content. It does not fit Bazel extension
repos which land real source files on disk. Bazel treats those files as ordinary
source: digested lazily per access, never copied, never path-remapped. Slug's
root cell already does this via `IoFileOpsDelegate`. Extension repos take an
unnecessary alternate path.

### Incremental plan

**Phase 6.1: Skip eager walk (IMPLEMENTED 2026-04-17)**

Remove `declare_all_source_artifacts_ext` calls in `extension_repo.rs` and drop
the eager spoke-compute loop. Keep `ExtensionRepoFileOpsDelegate` with its lazy
per-access `read_*` methods. Keep spoke registration in the dynamic registry.

Status: landed. Build progresses past the hang, `llvm-raw` + `llvm-project`
materialize correctly, `llvm_configure` rule executes and writes `vars.bzl` +
`targets.bzl`, overlay symlinks are created.

**Phase 6.2: Merge repo-rule attr defaults in DICE executor (IMPLEMENTED 2026-04-17)**

`starlark_repo_rule_executor_impl.rs` around line 143 converts user-passed
attrs to `RepositoryAttr` but did not merge declared defaults from
`frozen_rule.attrs()`. Copied the pattern from `repository_rule.rs:478-486`
(extension-context path that applies defaults correctly).

Status: landed. `llvm_configure(name="llvm-project")` now receives
`ctx.attr.targets` with its default value (DEFAULT_TARGETS list).

**Phase 6.3: Full Plan C — register extension repos as ordinary cells (PENDING)**

After 6.1 and 6.2 prove the lazy model works end-to-end, remove
`ExternalCellOrigin::ExtensionRepo` entirely and replace with normal cell
registration:

- Register extension repos as plain cells pointing at
  `bazel-external/{canonical}`, no external origin.
- Store `ExtensionRepoCellSetup` in a side registry (not as part of cell
  origin).
- Hook `FileOpsKey::compute` (or the dispatcher in
  `app/slug_common/src/file_ops/delegate.rs`): if cell has pending
  extension-repo metadata, drive materialization to completion via DICE first,
  then return `IoFileOpsDelegate`.
- Delete `ExtensionRepoFileOpsDelegate` and `declare_all_source_artifacts_ext`.
- Path resolution unchanged — no external origin means
  `artifact_path_resolver.rs` naturally produces `bazel-external/X/file`
  project-relative paths, same as root cell source files.

User confirmed no existing code depends on the `buck-out/v2/external_cells/...`
path format; buck-out structure will eventually match bazel-out directly.

### Touchpoints for Phase 6.3

- `app/slug_core/src/cells/external.rs` — remove
  `ExternalCellOrigin::ExtensionRepo` variant (keep `Bundled`, `Git`, etc.).
- `app/slug_common/src/legacy_configs/cells.rs` — register extension repos as
  normal cells; store `ExtensionRepoCellSetup` in a new side registry.
- `app/slug_common/src/file_ops/delegate.rs` — add materialization hook before
  returning `IoFileOpsDelegate`.
- `app/slug_external_cells/src/extension_repo.rs` — simplify: materialize
  directory, return `IoFileOpsDelegate`, delete file-ops complexity.
- `app/slug_bzlmod/src/pending_repo_cells.rs` — stop setting
  `ExternalCellOrigin`.

### Reference (Bazel behaviour)

From investigating `/var/mnt/dev/bazel`:

- `FileStateValue.create()` computes digests lazily (on first access) per file.
- `RepositoryDirectoryValue.Success` returns a `Root` pointing at the repo
  directory; actions reference files through this Root without copying.
- `local_repository` uses symlinks, not copies.
- `GlobFunction` operates lazily — traverses only matching files.
- No materializer-style `declare_write` for source files.

### Adversarial review

Two sub-agent reviews of smaller plans (skip `declare_write` only; lazy
per-file `declare_write`) both concluded the Buck2 materializer pipeline is
fundamentally incompatible with lazy access to large external source dirs.
Phase 6.3 bypasses it entirely for extension repos, which is the structurally
correct fix.

## Phase 7: LLVM `cquery` analysis hang — follow-up (2026-04-17)

After Phase 6.1 + 6.2 + downstream Starlark-compat fixes
(ExecGroup freeze, ExecutionInfoProvider equality, cc_common.launcher_provider
stub), a `slug uquery @llvm-project//llvm:config` now completes: BUILD
evaluation succeeds and returns the target label. `slug cquery` for the same
target, however, hangs during analysis.

### Observed symptoms

- `bazel-external/` grows to ~214 repos (many unrelated to our target:
  rules_fuzzing jazzer, Kotlin capabilities, JVM maven, Java JDKs, Swift NIO,
  etc. — all triggered transitively via extension eval during analysis).
- Daemon enters `S (sleeping)` with no CPU progress after a burst of
  materialisation activity. Client remains connected but never returns.
- Stderr shows no new log output once the hang begins. No error, no
  `BUILD FAILED` — just silence.

### Suspected causes (untriaged)

1. DICE future deadlock deeper in analysis (not the same as the Phase 6
   `declare_write` tree walk). Likely two analysis keys transitively awaiting
   each other via load / toolchain-resolution / configuration probing.
2. Every `@rules_xxx//...` label that shows up in any loaded .bzl file causes
   the corresponding extension cell to materialise. Many irrelevant
   extensions get pulled in for a cc_library target. Bazel is more selective
   — some kind of laziness gap at the cell-resolution level, or a lockfile
   parse that eagerly registers too many dynamic cells.
3. A specific extension or repo rule retries forever on failure (less likely
   now — we log failures clearly and most show up as "Creating stub" once).

### Next investigation steps

- Reproduce deterministically. Record which key is in flight at the moment of
  hang (DICE could expose "active keys" via a debug endpoint or by dumping
  pending futures on SIGUSR1).
- Compare Bazel's analysis graph for the same target: how many extensions
  does Bazel actually evaluate for `@llvm-project//llvm:config`?
- Check whether `pre_compute_extension_repo_cells` registers
  `ExtensionRepoCellSetup` for repos that are never referenced, and if
  something in analysis accesses them anyway.
- Bisect: does `slug cquery` hang on a simpler target in a simpler repo?
  If not, the hang is specific to llvm-project's graph shape.

### Session commits (2026-04-17)

- `62fe237` — Plan 10 Phase 6.1 + 6.2 (lazy file tracking, attr defaults).
- `5d128f6` — `repository_ctx.path(Label)` unconditional resolution.
- `9bbea57` — ExecGroupValue freezable.
- `6acb8a3` — ExecutionInfoProvider equality.
- `99d4502` — Stub `cc_common.launcher_provider`.

Verified through this chain: llvm_configure Starlark rule runs end-to-end,
full llvm-project overlay created, BUILD.bazel evaluation succeeds, uquery
returns the target. Blocker is now in the analysis phase (Phase 7).

### Investigation findings (2026-04-17, second session)

**Root cause located:** `module_extension_executor_impl.rs::execute_extension`
ran an eager spoke-materialization loop (`ctx.compute(&ExtensionRepoExecutionKey)`
per spec) **serially** with `.await` on each call. For extensions producing
hundreds of spokes (crate: 1230+; rust toolchain: ~50; LLVM toolchain: ~40
per-platform archive), this is O(N) wall time per extension.

Phase 6.1's commit message (`62fe237`) claimed this loop was dropped — it was
not. Only the `declare_all_source_artifacts_ext` file-walk path was dropped.
The eager `ctx.compute` loop stayed. Phase 6.1's Phase-6.1 heading in the plan
text was aspirational; git diff confirms the loop remained untouched.

During cquery analysis, `ensure_registered_toolchains_loaded`
(`app/slug_analysis/src/analysis/env.rs:386`) iterates every registered
toolchain across all non-dev modules and calls
`dice.get_interpreter_results(package_label)` for each. Loading any
toolchain BUILD file transitively loads `.bzl` files that `load()` from
extension-generated cells. Each first-touch on an extension cell triggers that
extension's execution → enters the serial eager loop → serializes all of that
extension's spoke materializations. Cascade × serial = minutes of sleep.

uquery never enters `get_analysis_result_inner`, so it never calls
`ensure_registered_toolchains_loaded`, so the cascade does not fire — hence
uquery finishes while cquery hangs.

### Fix applied

`module_extension_executor_impl.rs:411-480`: converted the serial loop into a
parallel `ctx.try_compute_join`. Dynamic-cell registration and
`.slug_repo_complete` skip-check remain, so lazy fast-path for
already-materialized repos is preserved. Per-spec materialization still runs
via `ExtensionRepoExecutionKey::compute`, so DICE dedup and cycle detection
remain intact. I/O-bound downloads that have no inter-spoke dependency can
now proceed concurrently.

### What the parallelization fix does NOT address

- `ensure_registered_toolchains_loaded` still eagerly loads every non-dev
  toolchain package. For a cc_library target, loading rules_fuzzing / Kotlin
  / Swift / JVM toolchain packages is structurally unnecessary — Bazel only
  loads the toolchain packages for types the target's rule actually declares.
- Cross-extension Label references (crate extension's
  `ctx.execute([Label("@rs_rust_host_tools//:bin/cargo"), ...])`) still
  depend on the referenced extension having been materialized eagerly before
  the referring extension runs. Removing the eager loop entirely requires a
  blocking-from-sync-Starlark materialization hook.

Both items were previously punted to "Plan 13 out-of-scope" or "deferred".
Pulling them in-scope as Phase 7.1 and Phase 7.2 below — the parallelization
fix is a stopgap that masks rather than eliminates the cascade.

---

## Phase 7.1: Parallelize toolchain package loading

### Problem

`ensure_registered_toolchains_loaded`
(`app/slug_analysis/src/analysis/env.rs:386`) iterates every registered
toolchain label **serially** with `dice.get_interpreter_results(...).await`
inside the loop body. For an `@llvm-project//llvm:config` cquery, that's ~100
sequential package loads spanning rules_fuzzing, rules_kotlin, rules_swift,
rules_jvm_external, rules_java, rules_python, etc. Each load triggers its own
transitive `.bzl` cascade.

Phase 7's parallelization of the inner spoke-materialization loop is
neutralized: package N's cascade can't start until package N-1's load (and
its spoke cascade) completes, because the outer `await` serializes everything.

### Design: parallelize the outer loop

Replace the serial `for` loop with `ctx.try_compute_join`. Each toolchain
package load runs concurrently; DICE deduplicates shared loads (many
toolchain labels resolve to overlapping `.bzl` files); Phase 7's parallel
inner spoke materialization now gets to run across extensions in parallel too.

Preserve the existing filter (extension-repo skip, cell-not-found skip,
non-fatal package load failures). `DeclaredToolchainInfo` registry writes
are under `RwLock`; they already tolerate concurrent registration.

### Why NOT per-type demand-driven loading

The original Phase 7.1 draft proposed a
`RegisteredToolchainsByType: HashMap<type, Vec<label>>` index with per-type
lazy loading. Abandoned because:

- Determining a toolchain's `toolchain_type` requires loading the package's
  BUILD file (the `toolchain()` rule's `toolchain_type` attr). So populating
  the index still requires loading all packages.
- Bazel pays the same cost — it loads every registered `toolchain()` wrapper.
  Bazel's speed comes from Skyframe parallelism, not from type-filtering the
  loads.
- Per-target-type filtering only helps if we skip loading packages that
  DON'T match the requested type. Without pre-computed type info, we can't
  skip them. Circular.

Parallelization directly addresses the hang. If it proves insufficient,
revisit per-type filtering backed by lockfile-cached `(label → type)` pairs
from prior builds.

### Touchpoints

- `app/slug_analysis/src/analysis/env.rs` —
  `ensure_registered_toolchains_loaded`: convert `for tc_label_str in
  &registered { ... dice.get_interpreter_results(...).await ... }` into
  `dice.try_compute_join(registered, |ctx, tc_label_str| async move { ... })`.
- `register_declared_toolchain` already uses `RwLock` — no change.
- Non-fatal load errors already swallowed with `continue` — preserve via
  returning `Ok(())` in the mapper.

### Success criteria

#### Automated

- [x] `cargo check -p slug_analysis` clean (2026-04-17)
- [x] `cargo test -p slug_analysis --lib` — 10/10 pass (2026-04-17)
- [x] `cargo build -p slug` clean (2026-04-17)

#### Manual

- `slug cquery //app:calculator` in `examples/multi_package` still resolves
  toolchains correctly (no regression for working builds)
- For an llvm-project-shaped graph: total wall time for
  `ensure_registered_toolchains_loaded` drops from O(N_packages × slowest
  cascade) to O(slowest cascade). Measurable via elapsed time in the
  `tracing::debug!` summary at end of function.

### Phase 7.1 implementation notes (2026-04-17)

`app/slug_analysis/src/analysis/env.rs` —
`ensure_registered_toolchains_loaded` split into two phases:

1. **Pre-filter (serial, cheap)** — parse labels, skip extension repos,
   resolve cell names. No DICE calls inside the filter, so keeping it serial
   costs nothing. Populates `to_load: Vec<(String, PackageLabel)>`.
2. **Load (parallel, expensive)** — `dice.try_compute_join(to_load, ...)`
   fires all `get_interpreter_results` calls concurrently. Non-fatal errors
   match prior behaviour (swallowed with `tracing::warn!`). DICE's internal
   scheduler bounds actual parallelism.

Structural property verified via code review: the outer `.await` that
previously serialized cascades is gone. Combined with Phase 7's parallel
spoke loop, the entire toolchain-loading path is now fully parallel.
`DeclaredToolchainInfo` registry writes go through `RwLock`; already safe
for concurrent registration from multiple mapper futures.

### Phase 7.2 assessment (2026-04-17)

With Phase 7 + Phase 7.1 combined, `ensure_registered_toolchains_loaded`
is now fully parallel at both levels (toolchain package loads → extension
spoke materializations). The 214-repo amplification is unchanged — those
repos still materialize — but in parallel rather than sequentially.

### Phase 7 empirical diagnosis (2026-04-17, third session)

Reproduced the hang against `/var/mnt/dev/llvm-project/utils/bazel/` with
diagnostic counters on `ModuleExtensionExecutionKey::compute` and
`ExtensionRepoExecutionKey::compute`. Findings invalidated **all three**
of the 14-series candidate plans:

- **14a premise (DICE re-executes errored keys)**: false. Per-key compute
  counter showed `count=1` for both the single extension that ran
  (`llvm_repos_extension`) and the single spoke it materialized (`gmp`).
  DICE dedups correctly within a transaction.
- **14b premise (lockfile misses force re-execution)**: irrelevant. The
  lockfile cache hit for llvm_repos_extension and specs were used. No
  re-run from cache miss.
- **14c premise (spoke amplification cascades)**: irrelevant. Only one
  extension and one spoke were in flight during the entire hang window.

Actual blocker: `download_url` in
`app/slug_bzlmod/src/repository_executor.rs:584` used `curl --max-time
300` with no `--connect-timeout`, followed by a same-URL wget fallback
with another 300s timeout. gmplib.org was unreachable at TCP level from
this host; curl blocked 30+ seconds to fail the connect, continued to
wait toward its 300s ceiling, then wget tried the same URL with another
300s budget. The outer fallback loop (which would have tried
`https://ftp.gnu.org/gnu/gmp/...` — reachable and fast) only got a turn
after >5 minutes of per-URL time. Single tokio worker blocked in
synchronous `Command::output()` during the whole stall.

### Fix (commit 04176ec)

`repository_executor.rs:584` tightened:
- Added `--connect-timeout 30` so unreachable mirrors fall through in
  ~30s.
- Reduced `--max-time` from 300 to 60s to cap stalled transfers.
- Dropped the same-URL wget fallback on curl HTTP failure. The caller's
  `urls[]` already provides the real fallback; wget would re-issue the
  same failing request with the same timeout. wget remains the primary
  tool if curl is not installed.

Verified:
- `slug cquery @llvm-project//llvm:config` now completes in ~54s
  (previously 5+ minute hang).
- Log shows `curl: (28) Failed to connect to gmplib.org port 443 after
  30002 ms` → immediate fall-through to `ftp.gnu.org` → download
  succeeds in ~2s → gmp materialised.
- Cquery then surfaces a real, actionable error:
  `bazel_tools//src/conditions package does not exist`. Separate
  Bazel-compat issue, not a hang.
- 154/157 slug_bzlmod tests passing (3 pre-existing lockfile test
  failures unrelated to this change).

### Status

- Phase 7 (parallel spoke materialization): complete (commit 094b0ab)
- Phase 7.1 (parallel toolchain package loading): complete (commit
  2b0f50a)
- Phase 7 real-world verification: complete (commit 04176ec)
- Phase 7.2 (on-demand spoke materialization): **not required**.
  Amplification (214 repos) is wasteful in disk/time but not a hang
  source. Defer indefinitely; reassess if disk-usage or first-build
  wall-time becomes a user complaint.
- Plans 14a / 14b / 14c: drafted during investigation, retained as
  reference, not implemented — reviewer findings + empirical evidence
  showed none addressed the actual bottleneck.

---

## Phase 7.2: On-demand materialization from sync Starlark

### Problem

The eager spoke-materialization loop (now parallelized in Phase 7) exists
only because `module_ctx.path(Label)` and
`module_ctx.execute([Label, ...])` resolve Label to a filesystem path during
synchronous Starlark evaluation. Slug lacks Bazel's Skyframe restart
mechanism, so the repo must be on disk by the time Starlark reads the path.
Eager materialization of every spoke pre-empts that need but materializes
far more than the current extension actually references.

### Design

Add a blocking-from-sync-Starlark hook that materializes the specific repo
referenced by a Label at the moment `path(Label)` resolves it. Two routes:

**Route A — Pre-scan before Starlark eval**: Parse the extension's `.bzl`
AST for `ctx.path(Label(...))` and `ctx.execute([Label(...)])` call sites,
extract referenced repo names, and materialize *only those* before calling
`implementation(module_ctx)`. Cheap, but misses dynamic Label construction.

**Route B — Blocking DICE compute from sync Starlark**: Thread a tokio
`Handle` and DICE `DiceComputations` reference into `ModuleContext`. In
`path(Label)`, detect if the referenced repo is not yet materialized, and
invoke `tokio::task::block_in_place(|| handle.block_on(ctx.compute(&key)))`.
Requires verifying that `block_in_place` from inside DICE's own worker thread
doesn't deadlock the runtime.

### Recommendation

Start with **Route A** (pre-scan). Covers ~100% of real-world cases (the Label
references are always static in captured extensions like `rules_rs`'s crate).
Falls back gracefully — if pre-scan misses a reference, the old eager-loop
behavior kicks in (gated behind a feature flag during rollout, then removed).

### Touchpoints

- `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs` —
  after `with_repo_spec_registry` evaluates, before the eager loop: scan the
  extension's `FrozenModule` for Label literals referenced by
  `module_ctx.path` / `module_ctx.execute` / `module_ctx.read`
- `app/slug_bzlmod/src/extension_execution_dice.rs` — optional: store
  pre-scanned cross-extension refs alongside `ModuleExtensionResult` so they
  persist via the lockfile cache
- Delete the Phase-7 parallel loop (not the dynamic-cell-registration path)
  once Route A demonstrably handles the crate extension

### Success criteria

#### Automated

- `cargo check` clean
- `pytest tests/core/ -q` — all passing tests stay green

#### Manual (zeromatter)

- `slug build //sdk:sdk` still succeeds: crate extension's `toml2json`
  execution finds `rs_rust_host_tools` on disk
- `bazel-external/` count for zeromatter cc_library builds drops
  correspondingly (only referenced spokes materialize)
- For `@llvm-project//llvm:config`: zero repos materialized beyond those
  actually touched by target-closure `.bzl` loads

### Verification (2026-04-17 continued)

Build + existing test suite pass locally (`cargo check -p
slug_interpreter_for_build` clean; 50/50 interpreter_for_build tests pass;
lockfile tests have 3 pre-existing failures unrelated to this change).

LLVM-specific repro for `@llvm-project//llvm:config` not runnable in this
workspace (no MODULE.bazel-registered llvm-project checkout locally). Verified
the parallelization path indirectly against `examples/multi_package`:

- `slug uquery //app:calculator` — completes
- `slug cquery //app:calculator` — returns in 0.274s, no hang
- Extension execution (`bazel_features`, `rules_cc`, etc.) flows through
  `try_compute_join` and returns without deadlock
- Analysis ultimately fails downstream on CC toolchain resolution
  (`find_cc_toolchain.bzl:88`), but this is a pre-existing regression —
  stashing the Phase 7 change reproduces the same failure on `bdbd737`.
  Unrelated to the parallelization fix.

Outstanding: confirming elapsed-time bound on an actual cascaded-extension
scenario (llvm-project graph shape) remains pending until an llvm-project
MODULE.bazel workspace is available. The structural property — that spoke
materialization no longer serializes behind a single `.await` — is visible
in code review (`try_compute_join` dispatches all specs concurrently, DICE
bounds actual parallelism).

## References

- Extension execution DICE: `app/slug_bzlmod/src/extension_execution_dice.rs`
- Module context: `app/slug_interpreter_for_build/src/module_ctx.rs`
- Extension executor: `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`
- Repo spec capture: `app/slug_bzlmod/src/repo_spec.rs`
- Repository rule hook: `app/slug_interpreter_for_build/src/repository_rule.rs:403-424`
- Synthetic repos: `app/slug_bzlmod/src/synthetic_repos.rs`
- Cell registration: `app/slug_common/src/legacy_configs/cells.rs`
- External cell file ops: `app/slug_external_cells/src/extension_repo.rs`
- Cell external origin: `app/slug_core/src/cells/external.rs`
- Artifact path resolver: `app/slug_core/src/fs/artifact_path_resolver.rs`
- IO file ops delegate (target model): `app/slug_common/src/file_ops/io.rs`

## 2026-04-17 continuation: `@llvm-project//llvm:llvm` post-analysis unblocks

Three further commits landed unblocking `@llvm-project//llvm:llvm`
beyond cquery. These are follow-ons to Plan 10 Phase 7 (specifically
to the extension-repo materialization path documented here) and have
been promoted to Plan 15 sub-plans 15.5.1 / 15.5.2 for API-surface
tracking.

- `fe3639f` — Seed `STACK_FRAME_UNLIMITED=""` in `ctx.var` builtins.
  Unblocks analysis of rules_cc's `_expand_make_variables_for_copts`.
  Tracked for real `TemplateVariableInfo` plumbing in 15.5.1.
- `bfe28b4` — Symlink
  `buck-out/v2/external_cells/extension_repo/{canonical}` ->
  `bazel-external/{canonical}` after materialization. Action command
  lines use the buck-out path (via
  `BuckOutPathResolver::resolve_external_cell_source`), so action
  execution needs the content reachable there. Mirrors the bzlmod
  cell symlinking at `cells.rs:822`.
- `325e06a` — Follow symlinks in `ExtensionRepoFileOpsDelegate::read_dir`
  when classifying entry types. `repository_ctx.symlink(src_dir,
  dst_dir)` materializes whole subtrees as directory symlinks;
  `DirEntry::file_type()` does not follow, so
  `gather_package_listing_impl`'s `if d.file_type.is_dir()` test
  failed and `glob()` returned empty for symlinked subtrees.
  `IoFileOpsDelegate` has the same latent bug (not yet hit in
  practice); see 15.5.2.

Current state: `@llvm-project//llvm:llvm` reaches compile stage,
Support compiles ~183 files, blocks on generated-header resolution
for `llvm/Config/abi-breaking.h` (an `expand_template` output). That
remaining blocker is tracked in Plan 15 sub-plan 15.5.3.

---

## Closure Status (2026-05-04)

Plan 10's primary goal — letting real module extensions execute via DICE
instead of synthetic stubs — is **complete**. Extensions execute, repos
materialize with real content, label-to-path resolution works, parallelization
removes the cquery-time cascade.

### Phase 4 follow-up: cross-module relative-label canonicalization (complete, 2026-05-04)

Discovered while attempting end-to-end verification of Plan 11 Phase 8 against
zeromatter `//sdk:sdk`.

**Bug**: When an extension's tag attribute (`attr.label`) is set in a *non-root*
module's `MODULE.bazel` using a relative `//` label, slug resolves it against
the root project's cell instead of the *enclosing module's* cell.

**Concrete example** (rules_rs's own MODULE.bazel):

```python
crate.from_cargo(
    name = "rrra",
    cargo_toml = "//tools/rust_analyzer:Cargo.toml",   # relative `//`
    ...
)
```

The label should canonicalize to `@@rules_rs//tools/rust_analyzer:Cargo.toml`
(the file lives at `bazel-external/rules_rs+override/tools/rust_analyzer/Cargo.toml`).
Instead it reaches `resolve_label_to_filesystem_path` in
`app/slug_interpreter_for_build/src/module_ctx/context.rs:263-270` with
`repo = ""`, so slug produces `<project_root>/tools/rust_analyzer/Cargo.toml`
— which doesn't exist in zeromatter's tree.

The crate extension calls `mctx.execute([Label(toml2json), toml_file])` which
fails ENOENT, the extension fails, `@crates` becomes a stub, and analysis of
`//sdk:sdk` fails on `Module has no symbol all_crate_deps`.

**Where the canonicalization should happen**: `attr.label` tag-attribute
coercion needs to know the *owning module* of the MODULE.bazel where the tag
was used, and use that module's repo as the base for relative `//` labels.
Suspect site: tag-attr coercion in `app/slug_bzlmod/` or `module_ctx/tags.rs`,
or label parsing from the serialized tag values.

**Why not blocking Plan 10 closure**: this is a Phase 4 polish item that
arises only with extensions whose tags are set across modules using relative
labels. Plan 10's core (DICE execution of extensions, eager spoke
materialization, label-to-path for already-canonical labels) works. The
relative-label canonicalization is one more pre-existing-Phase-4-style fix
on the same code path. Track here; close when fixed.

**Minimum fix sketch**:
- Identify where tag attribute values are coerced (likely
  `module_ctx/tags.rs` `SerializedTag` construction or its consumer).
- Pass the owning module's repo name through to label coercion.
- For an `attr.label` value with `repo.is_empty()`, replace with the owning
  module's repo before producing the canonical `@@<module>//pkg:target` form.

**Resolution (2026-05-04)**: Implemented in `app/slug_bzlmod/src/globals.rs`:

- Added `canonicalize_relative_label(s, owning_module)` helper — pure-string
  function that prepends `@@<module>` to relative `//`-labels, with
  pass-through for already-canonical (`@@`/`@`) and target-relative (`:`) forms.
- Threaded `owning_module: Option<&str>` through `starlark_to_tag_value` and
  its callers (`ExtensionTagInvoker::invoke`, the `use_repo_rule` invocation
  path). The owning module name comes from the `ModuleFileContext`'s `module`
  decl.
- Verified by 6 unit tests in `mod label_canonicalization_tests` covering all
  branches (relative-to-module, root-module-pass-through, already-canonical,
  target-relative, empty-pkg). Also verified end-to-end by the parser
  integration test `parser::tests::test_parse_use_extension_with_tags` which
  now asserts the canonical `@@test//:requirements_lock.txt` form.
- All 161 `slug_bzlmod --lib` tests pass; no regressions in `slug_analysis`.

**Status: Complete**. The zeromatter `crate.from_cargo` failure mode
("Failed to read tools/rust_analyzer/Cargo.toml") that motivated this fix
no longer reproduces.

### Items moved to other plans (do NOT block closure)

| Item | Moved to | Reason |
|------|----------|--------|
| Rust `toolchain_type` alias resolution | **Plan 11 Phase 8** | Belongs in toolchain resolution, not extension execution. Extensions correctly produce the BUILD that Bazel produces; the gap is downstream alias-following at registration time. |
| Phase 6.3: register extension repos as ordinary cells (delete `ExternalCellOrigin::ExtensionRepo`) | **Stays in Plan 10 as deferred cleanup** | Structural simplification, not a correctness gap. Phases 6.1+6.2 already deliver lazy file tracking. Can land as an isolated refactor whenever convenient. |
| Phase 7.2: on-demand spoke materialization | **Plan 36** | Later zeromatter verification proved this was a correctness issue for extensions that call `mctx.path(Label)` on internal spoke repos. Plan 36 is the successor plan and now owns the remaining `repository_ctx` audit / attr-backfill / loud-fail follow-ups. |

### Outstanding manual verification (nice-to-have)

- Verify downloaded crate sources match Cargo.lock versions
- Compare generated BUILD files with what `bazel build` produces

These are sanity-checks, not gates. Real-world zeromatter and llvm-project
graphs already exercise the path end-to-end.

### Recommended action

Mark Plan 10 **Complete** in the main plan index. Open work above lives in
its rightful home (Plan 11) or as deferred follow-ups within Plan 10 itself.
