# Phase 9: Unified Execution Architecture

> **Parent Plan**: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)
> **Research**: [Sync Extension Executor Architecture Analysis](../../research/2026-02-18-sync-extension-executor-architecture-analysis.md)

## Motivation

Slug currently has **4 distinct .bzl loading paths**, a **synchronous pre-DICE extension executor** with a subset of Starlark globals, and **deep dependency on `.buckconfig` files** for cell definitions. This diverges from Bazel's unified Skyframe-based architecture where:

1. Canonical repo names are pre-computed from the module graph topology alone (no extension execution needed)
2. All extension execution happens lazily inside Skyframe
3. A single `.bzl` loading mechanism handles all contexts
4. `.bazelrc` handles configuration, `MODULE.bazel` handles dependency management

Additionally, Slug's lockfile format diverges from Bazel 9.0's actual format in several ways that break cross-tool compatibility.

This plan unifies Slug's execution environments to match Bazel's architecture and ensures lockfile format compatibility.

---

## Architecture Overview

### Current State (Problems)

```
MODULE.bazel parsed
       │
       ▼
Lockfile check for extension results
       │
       ├── HIT: use cached RepoSpecs → register cells
       │
       └── MISS: try_execute_extension_sync()
                   │
                   ├── Root-local extension → DiskFileLoader (subset globals, no external loads)
                   └── External extension → FAILS silently
       │
       ▼
CellResolver built (pre-DICE) → DICE starts
       │
       ▼
Extension repos accessed → DICE-based execution (full globals)
```

**Problems:**
- `DiskFileLoader` has ~4 globals vs ~20+ in normal path
- `@cell//...` loads fail in sync mode
- Thread-local state (`CURRENT_BZL_CONTEXT`) doesn't compose with concurrent evaluation
- `.buckconfig` read before `MODULE.bazel` for cell definitions
- Two separate module caches (Mutex<HashMap> vs DICE)
- Same .bzl file evaluated twice (sync then DICE)
- Lockfile format diverges from Bazel 9.0

### Target State (Bazel-Like)

```
MODULE.bazel parsed (all modules in dep graph)
       │
       ▼
Pre-compute ALL canonical names from module graph topology
  (deterministic: _main~{ext_name}~{repo_name} from use_repo() declarations)
       │
       ▼
Register ALL cells (bzlmod deps + extension repo placeholders) in CellResolver
       │
       ▼
DICE starts with complete CellResolver
       │
       ▼
Extension repo first accessed → SingleExtensionExecutionKey::compute()
       │
       ├── Lockfile HIT → return cached RepoSpecs (no .bzl loading)
       └── Lockfile MISS → load extension .bzl via DICE → execute → cache in lockfile
       │
       ▼
Repo rule execution → StarlarkRepoRuleExecutor (DICE-based, full globals)
```

**Key insight**: `use_repo(pip, "numpy")` in `MODULE.bazel` tells us the canonical name `_main~pip~numpy` WITHOUT running the pip extension. We just need the module graph topology.

---

## Sub-Phase Index

| Sub-Phase | Title | Description |
|-----------|-------|-------------|
| 9a | Lockfile Format Compatibility | Match Bazel 9.0's exact lockfile JSON format |
| 9b | Pre-Computed Canonical Names | Register extension repo cells from `use_repo()` declarations alone |
| 9c | DICE-Only Extension Execution | Move all extension execution inside DICE, remove sync executor |
| 9d | `.buckconfig` Elimination for Cells | Move cell definitions entirely to `MODULE.bazel` |
| 9e | Configuration Migration | Move build config from `.buckconfig` to `.bazelrc`/`MODULE.bazel` |
| 9f | Cleanup and Unification | Remove dead code, unify .bzl loading paths |

---

## Sub-Phase 9a: Lockfile Format Compatibility

### Goal

Make Slug's `MODULE.bazel.lock` format match Bazel 9.0's lockfile format exactly. A lockfile written by Slug should be parseable by Bazel, and vice versa.

### Current vs Bazel 9.0 Format

**Bazel 9.0 actual format** (`lockFileVersion: 26`):
```json
{
  "lockFileVersion": 26,
  "registryFileHashes": {
    "https://bcr.bazel.build/bazel_registry.json": "sha256-hex",
    "https://bcr.bazel.build/modules/rules_cc/0.0.9/MODULE.bazel": "sha256-hex"
  },
  "selectedYankedVersions": {},
  "moduleExtensions": {
    "@@rules_python+//python/extensions:pip.bzl%pip": {
      "general": {
        "bzlTransitiveDigest": "base64-encoded-sha256",
        "usagesDigest": "base64-encoded-sha256",
        "recordedInputs": [
          "REPO_MAPPING:rules_python+,bazel_tools bazel_tools",
          "FILE:@@rules_python+//MODULE.bazel sha256-hex"
        ],
        "generatedRepoSpecs": {
          "numpy": {
            "repoRuleId": "@@rules_python+//pip:pip.bzl%pip_install",
            "attributes": {
              "version": "1.24.0"
            }
          }
        },
        "moduleExtensionMetadata": null
      }
    }
  },
  "facts": {}
}
```

**Slug current format** (`lockFileVersion: 24`):
```json
{
  "lockFileVersion": 24,
  "moduleFileHash": "sha256-base64",
  "registryFileHashes": {},
  "selectedYankedVersions": {},
  "moduleDepGraph": { ... },
  "moduleExtensions": { ... },
  "repositoryRules": { ... }
}
```

### Differences to Fix

| Aspect | Slug Current | Bazel 9.0 | Fix |
|--------|-------------|-----------|-----|
| `lockFileVersion` | `24` | `26` | Update constant |
| `moduleFileHash` | Present | **Removed** (Bazel 8.0+) | Remove field |
| `moduleDepGraph` | Present | **Removed** (Bazel 8.0+) | Remove field |
| `repositoryRules` | Present (Slug-specific) | **Not present** | Remove or move to separate file |
| `facts` | Missing | Present (may be `{}`) | Add field (empty initially) |
| `recordedInputs` | Missing from extensions | Present as string array | Add field |
| `moduleExtensionMetadata` | Missing from extensions | Present (nullable) | Add field |
| Extension ID separator | `~` (e.g. `@@rules_python~//...`) | `+` (e.g. `@@rules_python+//...`) | Update separator |
| Registry file hashes | SRI format (`sha256-base64`) | Hex format (`sha256-hex-string`) | Match Bazel's format |
| `recordedInputs` format | N/A | `REPO_MAPPING:mod+,name canonical`, `FILE:@@mod+//path sha256-hex`, `ENV:VAR_NAME` | Implement |

### Implementation

#### Step 1: Update lockfile version to 26

**File**: `app/slug_bzlmod/src/lockfile.rs:60`

- [x] Change `LOCKFILE_VERSION` from `24` to `26`

#### Step 2: Remove deprecated top-level fields

**File**: `app/slug_bzlmod/src/lockfile.rs`

Bazel 8.0+ removed `moduleFileHash` and `moduleDepGraph` from the lockfile. Slug should match.

- [x] Remove `module_file_hash` from `Lockfile` struct (marked deprecated with `#[serde(default, skip_serializing)]`)
- [x] Remove `module_dep_graph` from `Lockfile` struct (marked deprecated with `#[serde(default, skip_serializing)]`)
- [x] Remove `LockfileModuleNode` struct entirely (kept struct for backwards compat, removed impl)
- [x] Remove `from_resolved_graph()` and `to_resolved_graph()` methods
- [x] Remove `is_valid_for()` (depends on `module_file_hash`)
- [x] Add backwards compatibility: still deserialize old lockfiles with these fields (use `#[serde(default)]`)

The module dep graph information was used for lockfile-based module resolution. After this change, module resolution always runs fresh (which is fast — just MODULE.bazel parsing + MVS). The lockfile's primary purpose becomes extension result caching.

#### Step 3: Remove `repositoryRules` from lockfile format

**File**: `app/slug_bzlmod/src/lockfile.rs`

`repositoryRules` is a Slug-specific addition not in Bazel's format. Move to a separate Slug-specific cache file if still needed.

- [x] Remove `repository_rules` field from `Lockfile` struct (marked deprecated with `#[serde(default, skip_serializing)]`)
- [x] Remove `RepositoryRuleLockEntry` and `DownloadedFileLockEntry` structs (kept for backwards compat deserialization)
- [x] Remove all `repository_rule_cache` methods
- [ ] If repo rule caching is still needed, create a separate `MODULE.bazel.lock.slug` or use DICE-based caching

#### Step 4: Add `facts` field

**File**: `app/slug_bzlmod/src/lockfile.rs`

Add the `facts` top-level field. Initially empty, used by some extensions for metadata.

- [x] Add `facts: HashMap<String, serde_json::Value>` field with `#[serde(default)]`

#### Step 5: Add `recordedInputs` to extension data

**File**: `app/slug_bzlmod/src/lockfile.rs`

Bazel 9.0 stores `recordedInputs` as a list of strings in each extension's general data. Three formats:
- `REPO_MAPPING:<module>+,<apparent_name> <canonical_name>`
- `FILE:@@<module>+//<path> <sha256-hex>`
- `ENV:<VARIABLE_NAME>`

- [x] Add `recorded_inputs: Vec<String>` field to `LockfileExtensionGeneral` with `#[serde(default)]`
- [ ] Populate `recordedInputs` during extension execution (repo mappings used, files read, env vars accessed) — deferred to 9c
- [ ] Use `recordedInputs` for cache validation (in addition to existing digest checks) — deferred to 9c

#### Step 6: Add `moduleExtensionMetadata` to extension data

**File**: `app/slug_bzlmod/src/lockfile.rs`

- [x] Add `module_extension_metadata: Option<serde_json::Value>` field to `LockfileExtensionGeneral` with `#[serde(default)]`

#### Step 7: Update extension ID separator from `~` to `+`

**Files**: Multiple

Bazel 9.0 uses `+` as the separator in canonical names and extension IDs:
- `@@rules_python+//python/extensions:pip.bzl%pip`
- Canonical repo names: `rules_python+pip+numpy`

Slug currently uses `~`:
- `_main~pip~numpy`

- [x] Audit all canonical name construction to use `+` instead of `~`
- [x] Update `build_canonical_names()` in `extension_execution_dice.rs`
- [x] Update `extract_extension_name()` in `extension_execution_dice.rs`
- [ ] Update `pre_compute_extension_repo_cells()` (new in 9b)
- [x] Update `build_use_repo_aliases()` in `pending_repo_cells.rs`
- [x] Update any path construction that uses `~` for canonical names
- [x] Ensure `bazel-external/` symlinks use `+` separator

#### Step 8: Update registry file hash format

**File**: `app/slug_bzlmod/src/lockfile.rs`

Bazel stores `registryFileHashes` as plain hex strings, not SRI format. Verify and fix.

- [ ] Verify Bazel's exact hash format for `registryFileHashes` values — deferred (need real Bazel lockfile to compare)
- [ ] Match the format exactly (hex vs base64, prefix vs no prefix)

#### Step 9: Add deserialization compatibility for old lockfiles

Since we're changing the format, ensure old lockfiles can still be read (and will be regenerated on next build).

- [x] All removed fields have `#[serde(default)]` for backwards compat
- [x] Version check allows reading older lockfiles (regenerates on write)
- [x] Add migration logic: if old-format lockfile is read, it's treated as empty (forces re-resolution)

### Success Criteria (9a)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] `cargo test -p slug_bzlmod` passes (update lockfile tests for new format) — all 158 tests pass
- [x] Lockfile written by Slug matches Bazel 9.0 JSON structure (validate with `jq` or test)
- [x] Old-format lockfiles are read without error (treated as needing refresh)

**Manual:**
- [ ] Generate a lockfile with Slug, validate the JSON structure matches Bazel 9.0
- [ ] Compare field names, nesting, and value formats with a real Bazel 9.0 lockfile
- [ ] Verify Bazel 9.0 can read a Slug-generated lockfile (at least without error)

---

## Sub-Phase 9b: Pre-Computed Canonical Names

### Goal

Register extension-generated repo cells in the CellResolver using ONLY information from `MODULE.bazel` parsing (the `use_extension()` and `use_repo()` declarations), without executing any extensions or consulting the lockfile for cell registration purposes.

### How Bazel Does It

`BazelDepGraphFunction` (`BazelDepGraphFunction.java`) processes all MODULE.bazel files and generates canonical names via `makeUniqueNameCandidate()`:
```
canonical_name = <root_module>+<extension_name>+<repo_name>
```
This is purely deterministic from the module graph topology. No extension execution needed.

### Implementation

#### Step 1: Extract `use_repo()` declarations during MODULE.bazel parsing

**File**: `app/slug_bzlmod/src/parser.rs`

The parser already captures `use_extension()` and `use_repo()` calls into `ExtensionUsage` structs. Verify that all repos from `use_repo()` (both positional and keyword) are captured with their apparent names.

- [x] Verify `ExtensionUsage` captures all `use_repo()` repo names
- [x] Verify this works for transitive modules (not just root)

#### Step 2: Pre-compute canonical names from `use_repo()` declarations

**New function**: `pre_compute_extension_repo_cells()` in `app/slug_common/src/legacy_configs/cells.rs`

For each `use_extension()` + `use_repo()` combination across all parsed modules:

1. Extract the extension name from the extension ID (e.g., `pip` from `@@rules_python//python/extensions:pip.bzl%pip`)
2. For each repo in `use_repo()`:
   - **Positional**: `use_repo(pip, "numpy")` → canonical name `_main+pip+numpy`
   - **Keyword**: `use_repo(pip, np = "numpy")` → canonical name `_main+pip+numpy`, alias `np` → `_main+pip+numpy`
3. Register the cell with path `bazel-external/{canonical_name}` and origin `ExternalCellOrigin::ExtensionRepo(setup)`
4. Register aliases

The canonical name construction must match `build_canonical_names()` in `extension_execution_dice.rs:556-568` (after 9a updates the separator to `+`).

- [x] Create `pre_compute_extension_repo_cells()` function
- [x] Use deterministic canonical name formula: `{root_module}+{ext_name}+{repo_name}`
- [x] Register placeholder cells in CellsAggregator
- [x] Register `use_repo()` aliases
- [x] Handle keyword args in `use_repo()` (alias → canonical mapping)

#### Step 3: Move lockfile check OUT of cell registration

**File**: `app/slug_common/src/legacy_configs/cells.rs`

Currently `resolve_extension_repos_from_lockfile()` (line 1125) both checks the lockfile AND registers cells. Split this:

1. Cell registration uses ONLY `use_repo()` declarations (Step 2 above)
2. Lockfile is consulted ONLY inside DICE when the extension repo is first accessed

- [x] Remove `resolve_extension_repos_from_lockfile()` from `resolve_bzlmod_dependencies()`
- [x] Replace with `pre_compute_extension_repo_cells()` call
- [x] Remove `try_execute_extension_sync()` call from cell registration path

#### Step 4: Update `ExtensionRepoCellSetup` for deferred execution

**File**: `app/slug_core/src/cells/external.rs`

The `ExtensionRepoCellSetup` currently stores the full `RepoSpec` JSON. For deferred execution, it only needs:
- `extension_id`: which extension generates this repo
- `internal_name`: the repo name within the extension
- `canonical_name`: the full canonical name

The actual `RepoSpec` will be resolved inside DICE when the repo is first accessed.

- [x] Simplify `ExtensionRepoCellSetup` to store extension reference, not full RepoSpec
- [x] Add `extension_id` and `internal_name` fields

### Success Criteria (9b)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] `cd tests/manual_test && ../../slug build //:hello_bin` succeeds (extension repos pre-registered)
- [x] `cd tests/manual_test && ../../slug build //:hello_cc_proto` succeeds (313 commands)
- [x] All 158 slug_bzlmod unit tests pass

**Manual:**
- [x] Extension repos are registered in CellResolver purely from `use_repo()` declarations (verified: `pre_compute_extension_repo_cells()` replaces `resolve_extension_repos_from_lockfile()`)
- [ ] Verify that removing the lockfile still allows cell registration (extensions execute lazily inside DICE)

---

## Sub-Phase 9c: DICE-Only Extension Execution

### Goal

Move ALL module extension execution inside DICE. Remove the synchronous pre-DICE executor entirely. Extension execution happens lazily when an extension-generated repo is first accessed during a build.

### How Bazel Does It

`SingleExtensionEvalFunction` runs inside Skyframe, triggered lazily via dependency chain:
```
BUILD references @pip+numpy
  → RepositoryMappingFunction (apparent → canonical)
    → SingleExtensionFunction (validate extension)
      → SingleExtensionEvalFunction (execute extension, check lockfile first)
```

### Implementation

#### Step 1: Add lockfile check to DICE extension execution path

**File**: `app/slug_bzlmod/src/extension_execution_dice.rs`

`ModuleExtensionExecutionKey::compute()` already checks the lockfile (lines 336-371). Verify this path works correctly when the extension hasn't been executed by the sync executor first.

Currently the DICE path requires that cells are already registered. With 9b's pre-computed canonical names, cells ARE registered — but the RepoSpec data may not be available yet. The DICE path must:

1. Check lockfile first (existing behavior)
2. If lockfile miss: load extension .bzl via DICE, execute, capture RepoSpecs
3. Cache results in lockfile
4. Return RepoSpecs to the repo execution key

- [x] Verify `ModuleExtensionExecutionKey::compute()` works without prior sync execution
- [x] Ensure lockfile caching works correctly in DICE-only path
- [ ] Handle the case where extension generates different repos than `use_repo()` declared (validation) — deferred

#### Step 2: Update `ExtensionRepoExecutionKey` for deferred resolution

**File**: `app/slug_bzlmod/src/repository_execution.rs`

When an extension repo is first accessed, `ExtensionRepoExecutionKey::compute()` needs to:

1. Identify which extension generates this repo (from `ExtensionRepoCellSetup`)
2. Trigger `ModuleExtensionExecutionKey::compute()` to get all RepoSpecs for that extension
3. Find this specific repo's `RepoSpec` in the results
4. Execute the repo rule (existing behavior)

- [x] Update `ExtensionRepoExecutionKey::compute()` to trigger extension execution
- [x] Implement dependency: repo execution → extension execution → repo spec lookup
- [x] Handle case where extension doesn't generate expected repo (error message)

#### Step 3: Remove synchronous extension executor

**Files to modify:**
- `app/slug_bzlmod/src/sync_extension_executor.rs` — DELETE
- `app/slug_interpreter_for_build/src/sync_extension_executor_impl.rs` — DELETE
- `app/slug_bzlmod/src/lib.rs` — Remove `SYNC_EXTENSION_EXECUTOR_IMPL` export
- `app/slug_interpreter_for_build/src/lib.rs` — Remove `sync_extension_executor_impl` module and init
- `app/slug_common/src/legacy_configs/cells.rs` — Remove `try_execute_extension_sync()` function
- `app/slug_bzlmod/src/repo_spec.rs` — Remove `CURRENT_BZL_CONTEXT` thread-local, `set_bzl_context()`, `get_bzl_context()`

- [x] Delete `sync_extension_executor.rs` and `sync_extension_executor_impl.rs`
- [x] Remove all references to `SYNC_EXTENSION_EXECUTOR_IMPL`
- [x] Remove `try_execute_extension_sync()` from `cells.rs`
- [x] Remove `CURRENT_BZL_CONTEXT` thread-local from `repo_spec.rs`
- [x] Remove `set_bzl_context()` / `get_bzl_context()`

#### Step 4: Validate extension output matches `use_repo()` expectations

**New**: Add validation in the DICE extension execution path (matching Bazel's `SingleExtensionFunction`):

When an extension finishes executing, validate that:
- All repos referenced by `use_repo()` were actually generated
- Warn (don't error) if extra repos were generated but not referenced

- [ ] Add validation of extension output vs `use_repo()` declarations — deferred
- [ ] Clear error message when `use_repo()` references a repo the extension didn't generate — deferred

### Success Criteria (9c)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] `cd tests/manual_test && ../../slug build //:hello_bin` succeeds with DICE-only extension execution
- [x] `cd tests/manual_test && ../../slug build //:hello_cc_proto` succeeds (313 commands)
- [x] 159 slug_bzlmod unit tests pass

**Manual:**
- [ ] Verify no sync extension execution occurs (check logs for "synchronously" messages)
- [ ] Verify lockfile is written after DICE extension execution
- [ ] Verify second build uses lockfile cache (no re-execution)

---

## Sub-Phase 9d: `.buckconfig` Elimination for Cell Definitions

### Goal

Move cell definitions entirely to `MODULE.bazel`. The `[cells]`, `[cell_aliases]`, `[external_cells]` sections of `.buckconfig` are no longer needed. Project root discovery uses `MODULE.bazel` (already supported).

### Current `.buckconfig` Cell Sections

From the research, these sections define cells:
- `[cells]` / `[repositories]` — cell-to-path mappings
- `[cell_aliases]` / `[repository_aliases]` — cell aliases
- `[external_cells]` — bundled/git origin declarations
- `[external_cell_<name>]` — per-cell git configuration

These are ALL replaceable by `MODULE.bazel`:
- `bazel_dep()` replaces `[cells]` entries for external deps
- `bazel_dep(repo_name=...)` replaces `[cell_aliases]`
- `local_path_override()` replaces local cell paths
- `git_override()` replaces `[external_cells] = git`
- Bundled cells (`bazel_tools`, `local_config_platform`) are auto-registered

### Implementation

#### Step 1: Make `.buckconfig` optional for cell definitions

**File**: `app/slug_common/src/legacy_configs/cells.rs`

In `parse_with_file_ops_and_options_inner()` (line 356):

1. If `MODULE.bazel` exists, skip reading `[cells]` from `.buckconfig`
2. The root cell (`.`) is always implicitly defined from the project root
3. All other cells come from bzlmod resolution

- [x] Skip `[cells]` section when `MODULE.bazel` exists
- [x] Skip `[cell_aliases]` section when `MODULE.bazel` exists
- [x] Skip `[external_cells]` section when `MODULE.bazel` exists
- [x] Always register root cell implicitly from project root

#### Step 2: Derive root cell name from `MODULE.bazel`

**Current**: Root cell name comes from `.buckconfig` `[cells]` (the entry with path `.`).
**New**: Root cell name derived from `MODULE.bazel` `module(name = "my_project")`. If `module()` has no name, use `_main` (Bazel convention).

**File**: `app/slug_bzlmod/src/parser.rs`

- [x] Extract module name from `module()` call in root `MODULE.bazel`
- [x] Use as root cell name (falling back to `_main`)
- [x] Pass root cell name to `CellsAggregator`

#### Step 3: Handle prelude cell discovery without `.buckconfig`

**File**: `app/slug_interpreter/src/prelude_path.rs`

Currently the prelude is found by looking for a `"prelude"` cell alias. This alias must still be registered even without `.buckconfig`.

Options:
1. If a `prelude/` directory exists, auto-register it as the `prelude` cell
2. Register the prelude cell from bzlmod if there's a `bazel_dep(name = "prelude")`
3. For Bazel-mode projects (no Buck2 prelude), skip prelude entirely

- [x] Auto-detect prelude based on directory existence OR bzlmod dep — prelude_path() already returns Ok(None) when no "prelude" alias exists
- [x] Make prelude optional when in pure Bazel mode — already works (prelude_path returns None)

#### Step 4: Update project root discovery

**File**: `app/slug_common/src/invocation_roots.rs`

Already supports `MODULE.bazel` as a project root marker (line 81-84). Ensure `.buckconfig` is checked only as a fallback.

- [x] Prefer `MODULE.bazel` over `.buckconfig` for project root — already implemented in invocation_roots.rs
- [x] `.buckconfig` as fallback for legacy/hybrid projects — already implemented

#### Step 5: Update `tests/manual_test/`

**File**: `tests/manual_test/.buckconfig`

Remove `.buckconfig` or make it empty. All cell definitions should come from `MODULE.bazel`.

- [x] Remove or empty `tests/manual_test/.buckconfig`
- [x] Verify all builds still work with MODULE.bazel-only cell definitions

### Success Criteria (9d)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] `cd tests/manual_test && ../../slug build //:hello_bin` succeeds without `[cells]` in `.buckconfig`
- [x] `cd tests/manual_test && ../../slug build //:hello_cc_proto` succeeds (313 commands)

**Manual:**
- [ ] Verify project works with empty `.buckconfig` (or no `.buckconfig` at all)
- [ ] Verify `slug audit cell` shows all cells from MODULE.bazel

---

## Sub-Phase 9e: Configuration Migration

### Goal

Move build configuration from `.buckconfig` to `.bazelrc` and/or `MODULE.bazel` where Bazel has equivalents. Keep `.buckconfig` as a Slug-specific config file for settings with no Bazel equivalent.

### Configuration Mapping

**Move to `.bazelrc`** (Bazel equivalent exists):
| Current | `.bazelrc` equivalent |
|---------|----------------------|
| `[build] threads` | `build --jobs=N` |
| `[build] execution_platforms` | `build --extra_execution_platforms=` |
| `[parser] target_platform_detector_spec` | `build --platforms=` |
| `[project] ignore` | `.bazelignore` file |
| `[http] *` | `build --remote_*` flags |

**Keep in Slug config** (no Bazel equivalent):
| Setting | Why |
|---------|-----|
| `[slug] daemon_buster` | Slug-specific daemon management |
| `[slug] digest_algorithms` | Slug-specific hash config |
| `[slug] materializations` | Slug-specific materialization strategy |
| `[slug_resource_control] *` | Slug-specific cgroup management |
| `[slug_system_warning] *` | Slug-specific health monitoring |
| `[slug] file_watcher` | Slug-specific file watcher backend |

### Implementation

- [x] Implement `.bazelrc` parser (INI-like with `command --flag=value` syntax) — `app/slug_client_ctx/src/bazelrc.rs`
- [x] Support `common` and per-command flag sections
- [x] Support `import` and `try-import` directives
- [x] Support named configs (`build:opt --flag`, applied with `--config=opt`)
- [x] Load `~/.bazelrc` (user-level) and `<workspace>/.bazelrc` (workspace-level)
- [x] Inject flags right after subcommand (lower precedence than command-line flags)
- [x] Support `--nobazelrc` and `--bazelrc=none` to disable loading
- [x] Support `--bazelrc=PATH` (recognized by clap as startup flag)
- [x] Keep `.buckconfig` as fallback for Slug-specific settings (unchanged)

### Success Criteria (9e)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] 9 unit tests in `bazelrc::tests` pass
- [x] `.bazelrc` with `build --verbose=2` causes verbose build output

**Manual:**
- [x] Verify `.bazelrc` flags take effect (`build --verbose=2` shows per-action lines)
- [x] Verify `--nobazelrc` disables loading (build output less verbose)
- [x] Verify `--bazelrc=none` also disables loading
- [ ] Verify Slug-specific settings still work from `.buckconfig` fallback

---

## Sub-Phase 9f: Cleanup and Unification

### Goal

Remove dead code from the old execution paths and unify the remaining .bzl loading paths.

### Implementation

#### Step 1: Remove `resolve_extension_repos_from_lockfile()` entirely

This function is replaced by 9b's pre-computed canonical names + 9c's DICE-only execution.

- [x] Delete `resolve_extension_repos_from_lockfile()` from `cells.rs` — done in 9c
- [x] Delete `try_execute_extension_sync()` from `cells.rs` — done in 9c
- [x] Delete related helper functions — done in 9c

#### Step 2: Unify .bzl loading paths

After removing the sync executor, Slug should have **2** loading paths (down from 4):

1. **Normal DICE-based loading** — for BUILD files, .bzl files, extension .bzl files
2. **Module extension executor** — uses Path 1 to load .bzl, then invokes `implementation(module_ctx)`

Path 2 is just a thin wrapper around Path 1, which is the correct architecture (matches Bazel's `BzlLoadFunction` with context-specific keys).

- [x] Verify only 2 loading paths remain — confirmed: DICE-based + module extension executor (thin wrapper)
- [x] Remove any remaining references to `DiskFileLoader` — already removed (no references found)
- [x] Remove `build_extension_globals()` (no longer needed) — already removed (no references found)

#### Step 3: Clean up `repo_spec.rs`

- [x] Remove `CURRENT_BZL_CONTEXT` thread-local (replaced by DICE context) — done in 9c
- [x] Remove `set_bzl_context()` / `get_bzl_context()` — done in 9c
- [x] Clean up `RepoSpecRegistry` if no longer needed for sync path — still needed for DICE extension execution (with_repo_spec_registry used in module_extension_executor_impl.rs)

#### Step 4: Remove deprecated lockfile code

- [x] Remove `LockfileModuleNode`, `from_resolved_graph()`, `to_resolved_graph()` if still present — removed
- [x] Remove `RepositoryRuleLockEntry`, `DownloadedFileLockEntry` if still present — removed, fields use serde_json::Value
- [x] Remove any code that reads/writes `moduleDepGraph` or `repositoryRules` sections — fields kept as opaque Value for backwards compat only

#### Step 5: Update documentation and plan files

- [ ] Update `02-bzlmod.md` to reflect new architecture — deferred
- [x] Update main plan to mark this phase
- [x] Update MEMORY.md with new architecture notes

### Success Criteria (9f)

**Automated:**
- [x] `cargo build -p slug` succeeds
- [x] All existing manual tests still pass (hello_bin, hello_cc_proto)
- [x] 159 slug_bzlmod unit tests pass

**Manual:**
- [x] Code review confirms only 2 .bzl loading paths remain (DICE-based + module extension executor)
- [x] No references to sync extension executor in codebase (grep confirms)
- [ ] Lockfile format matches Bazel 9.0 exactly — mostly complete, registry hash format TBD

---

## Implementation Order and Dependencies

```
9a (Lockfile Format Compatibility)
 │
 ▼
9b (Pre-Computed Canonical Names)
 │
 ▼
9c (DICE-Only Extension Execution)
 │
 ▼
9d (.buckconfig Elimination for Cells)
 │
 ▼
9e (Configuration Migration) ← can be deferred
 │
 ▼
9f (Cleanup and Unification)
```

9a should come first since the `+` separator change affects canonical name construction used in 9b. 9b and 9c are the critical path. 9d can start after 9c. 9e is independent and lower priority. 9f is cleanup after everything else.

---

## Risk Assessment

### Low Risk
- **9a**: Lockfile format changes are additive (new fields) and backwards-compatible (old fields default to empty). Format migration is transparent.
- **9f**: Pure cleanup, no behavior change.

### Medium Risk
- **9b**: Pre-computing canonical names is additive — cells that were registered from lockfile/sync execution are now registered from `use_repo()` declarations instead. Same names, same paths.
- **9c**: Removing sync executor means first builds without lockfile must go through DICE. This changes the bootstrapping order but should work since cells are already registered (from 9b).

### Higher Risk
- **9d**: Removing `.buckconfig` cell definitions changes a fundamental assumption. Needs careful testing with the manual test project.
- **9e**: `.bazelrc` parser is a new component. Can be deferred without blocking other phases.

---

## Key Code References

| Component | File | Key Lines |
|-----------|------|-----------|
| Lockfile format | `app/slug_bzlmod/src/lockfile.rs` | entire file |
| Cell resolver construction | `app/slug_common/src/legacy_configs/cells.rs` | 356-573 (main flow) |
| CellsAggregator | `app/slug_common/src/legacy_configs/aggregator.rs` | 48-158 |
| Sync extension executor (DELETE) | `app/slug_interpreter_for_build/src/sync_extension_executor_impl.rs` | entire file |
| Sync executor trait (DELETE) | `app/slug_bzlmod/src/sync_extension_executor.rs` | entire file |
| DICE extension execution | `app/slug_bzlmod/src/extension_execution_dice.rs` | 336-488 |
| Repo execution key | `app/slug_bzlmod/src/repository_execution.rs` | `ExtensionRepoExecutionKey::compute()` |
| Canonical name construction | `app/slug_bzlmod/src/extension_execution_dice.rs` | 556-594 |
| CellResolverKey (DICE) | `app/slug_common/src/dice/cells.rs` | 54-84 |
| ExternalCellOrigin | `app/slug_core/src/cells/external.rs` | 22-34 |
| Pending repo cells | `app/slug_bzlmod/src/pending_repo_cells.rs` | 123-207 |
| MODULE.bazel parser | `app/slug_bzlmod/src/parser.rs` | ExtensionUsage parsing |
| Project root discovery | `app/slug_common/src/invocation_roots.rs` | 72-115 |
| Prelude discovery | `app/slug_interpreter/src/prelude_path.rs` | 41-50 |
| .buckconfig parsing | `app/slug_common/src/legacy_configs/configs.rs` | entire file |
| Config file paths | `app/slug_common/src/legacy_configs/path.rs` | 39-57 |
| Thread-local bzl context (DELETE) | `app/slug_bzlmod/src/repo_spec.rs` | 126+ |
