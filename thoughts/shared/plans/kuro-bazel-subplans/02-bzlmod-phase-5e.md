# bzlmod Phase 5e: Module Extension Execution - Deferred Model

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)
> **Parent Phase**: [Phase 5 Overview](./02-bzlmod-phase-5-overview.md)
> **Depends on**: Phase 5d (DICE Integration) - COMPLETE

## Overview

Execute module extensions using **deferred execution** model matching Bazel:
- Extensions capture `RepoSpec` objects (NO downloads during extension evaluation)
- Repository rules execute lazily via DICE when repos are first accessed
- Temporary working directory for `module_ctx` (deleted after extension completes)

**Why this phase is critical:** Module extensions are the primary way modern Bazel modules create repositories (e.g., `pip.parse()` creates Python package repos, `go_deps` creates Go module repos). The deferred model ensures efficient builds by only fetching repositories that are actually used.

---

## Current State

**What exists:**

| Component | File | Status |
|-----------|------|--------|
| `module_ctx` Starlark object | `kuro_interpreter_for_build/src/module_ctx.rs` | ✅ Complete interface, stub methods |
| Extension execution framework | `kuro_interpreter_for_build/src/extension_execution.rs` | ✅ `build_module_context()`, tag serialization |
| Repository executor | `kuro_bzlmod/src/repository_executor.rs` | ✅ http_archive, git_repository execution |
| DICE infrastructure | `kuro_bzlmod/src/repository_execution.rs` | ✅ `RepositoryRuleExecutionKey` |
| Lockfile entries | `kuro_bzlmod/src/lockfile.rs` | ✅ `RepositoryRuleLockEntry` |
| Repository invocations | `kuro_bzlmod/src/repository_invocations.rs` | ✅ Thread-local registry |
| `ExtensionExecutor` placeholder | `extension_execution.rs:292-328` | ❌ Stub only |

**What's missing:**

1. ~~`RepoSpec` type for deferred execution (captures rule + attrs, no execution)~~ - DONE (Phase 5e-1)
2. ~~DICE key for extension execution (`ModuleExtensionExecutionKey`)~~ - DONE (Phase 5e-2)
3. ~~DICE key for lazy repo execution (`ExtensionRepoExecutionKey`)~~ - DONE (Phase 5e-3)
4. Temporary working directory for `module_ctx` (deleted after extension)
5. Cell registration for extension-generated repositories (pending until accessed)
6. Lockfile integration with `generatedRepoSpecs` format

---

## Architecture

### Deferred Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Extension Evaluation                          │
│  1. Load extension .bzl file                                     │
│  2. Build module_ctx with aggregated tags + temp working dir     │
│  3. Run implementation(module_ctx)                               │
│  4. Capture RepoSpec for each repository rule call               │
│  5. Return ModuleExtensionResult with generatedRepoSpecs         │
│     (NO downloads, NO repo materialization yet)                  │
│  6. Delete temp working directory                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Cell Registration                             │
│  Register canonical names for all generated repos:               │
│  _main~{extension}~{repo_name} → (pending materialization)       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ (only when repo is accessed)
┌─────────────────────────────────────────────────────────────────┐
│                    Lazy Repository Execution                     │
│  When @repo_name is accessed in a build:                         │
│  1. DICE key: ExtensionRepoExecutionKey for the RepoSpec         │
│  2. Execute repository rule (download, extract, etc.)            │
│  3. Materialize to bazel-external/{canonical_name}/              │
│  4. Cache result in DICE and lockfile                            │
└─────────────────────────────────────────────────────────────────┘
```

### Key Insight: module_ctx vs repository_ctx

Both share `StarlarkBaseExternalContext` I/O implementations in Bazel, but differ in lifecycle:

| Aspect | module_ctx | repository_ctx |
|--------|-----------|----------------|
| Working dir | **Temporary** (deleted after extension) | **Permanent** (becomes the repository) |
| Purpose | Compute RepoSpecs | Materialize repository |
| When deleted | `shouldDeleteWorkingDirectoryOnClose()` = true | Never (is the output) |

---

## Data Structures

### RepoSpec (Captured, Not Executed)

**File**: `kuro_bzlmod/src/repo_spec.rs` (new)

```rust
use std::collections::HashMap;
use std::sync::Arc;

use allocative::Allocative;
use serde::Deserialize;
use serde::Serialize;

use crate::repository_invocations::AttrValue;

/// A captured repository specification from extension execution.
///
/// This represents the intent to create a repository WITHOUT executing
/// the repository rule. Actual execution happens lazily when the repo
/// is first accessed during a build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Allocative)]
pub struct RepoSpec {
    /// Repository rule identifier.
    /// Format: "@@{module}//path:file.bzl%{rule_name}"
    /// Example: "@@bazel_tools//tools/build_defs/repo:http.bzl%http_archive"
    pub repo_rule_id: String,

    /// All attributes passed to the rule EXCEPT 'name'.
    /// The name is stored separately in the containing map.
    pub attributes: HashMap<String, AttrValue>,
}

impl RepoSpec {
    /// Create a new RepoSpec.
    pub fn new(repo_rule_id: String) -> Self {
        Self {
            repo_rule_id,
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute.
    pub fn with_attr(mut self, key: String, value: AttrValue) -> Self {
        self.attributes.insert(key, value);
        self
    }

    /// Compute a hash for cache invalidation.
    pub fn compute_hash(&self) -> String {
        use base64::Engine;
        use sha2::Digest;
        use sha2::Sha256;

        let mut hasher = Sha256::new();
        hasher.update(self.repo_rule_id.as_bytes());

        let mut keys: Vec<_> = self.attributes.keys().collect();
        keys.sort();
        for key in keys {
            hasher.update(key.as_bytes());
            if let Some(value) = self.attributes.get(key) {
                hasher.update(format!("{:?}", value).as_bytes());
            }
        }

        let hash = hasher.finalize();
        format!(
            "sha256-{}",
            base64::engine::general_purpose::STANDARD.encode(hash)
        )
    }
}

/// Thread-local registry for capturing RepoSpecs during extension execution.
///
/// During extension implementation execution, repository rule calls are
/// intercepted and recorded as RepoSpecs rather than executed immediately.
#[derive(Debug, Default)]
pub struct RepoSpecRegistry {
    /// Collected specs: internal_name -> RepoSpec
    specs: std::cell::RefCell<HashMap<String, RepoSpec>>,
}

impl RepoSpecRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a RepoSpec for a repository.
    pub fn record(&self, internal_name: String, spec: RepoSpec) {
        self.specs.borrow_mut().insert(internal_name, spec);
    }

    /// Take all collected specs.
    pub fn take(&self) -> HashMap<String, RepoSpec> {
        std::mem::take(&mut *self.specs.borrow_mut())
    }
}

// Thread-local for extension execution context
thread_local! {
    static REPO_SPEC_REGISTRY: std::cell::RefCell<Option<RepoSpecRegistry>> =
        const { std::cell::RefCell::new(None) };
}

/// Set up a RepoSpec registry for extension execution.
pub fn with_repo_spec_registry<R>(f: impl FnOnce() -> R) -> (R, HashMap<String, RepoSpec>) {
    REPO_SPEC_REGISTRY.with(|cell| {
        *cell.borrow_mut() = Some(RepoSpecRegistry::new());
    });

    let result = f();

    let specs = REPO_SPEC_REGISTRY.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|r| r.take())
            .unwrap_or_default()
    });

    REPO_SPEC_REGISTRY.with(|cell| {
        *cell.borrow_mut() = None;
    });

    (result, specs)
}

/// Record a RepoSpec in the current extension context.
/// Returns false if no registry is active (not in extension execution).
pub fn record_repo_spec(internal_name: String, spec: RepoSpec) -> bool {
    REPO_SPEC_REGISTRY.with(|cell| {
        if let Some(registry) = cell.borrow().as_ref() {
            registry.record(internal_name, spec);
            true
        } else {
            false
        }
    })
}

/// Check if we're currently in extension execution context.
pub fn in_extension_context() -> bool {
    REPO_SPEC_REGISTRY.with(|cell| cell.borrow().is_some())
}
```

### ModuleExtensionResult (Return Value)

```rust
/// Result of module extension evaluation.
///
/// Contains captured RepoSpecs but NO materialized repositories.
/// Repositories are created lazily when accessed.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleExtensionResult {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,

    /// Hash of extension inputs (tags from all modules) for cache invalidation.
    pub input_hash: String,

    /// Generated repository specifications (NOT materialized).
    /// Keys are internal names (e.g., "numpy"), values are RepoSpecs.
    pub generated_repo_specs: HashMap<String, RepoSpec>,

    /// Canonical name mapping.
    /// Maps internal_name -> canonical_name (e.g., "numpy" -> "_main~pip~numpy")
    pub canonical_names: HashMap<String, String>,
}

impl ModuleExtensionResult {
    /// Get the canonical name for a repository.
    pub fn canonical_name(&self, internal_name: &str) -> Option<&str> {
        self.canonical_names.get(internal_name).map(|s| s.as_str())
    }

    /// Get a RepoSpec by internal name.
    pub fn get_repo_spec(&self, internal_name: &str) -> Option<&RepoSpec> {
        self.generated_repo_specs.get(internal_name)
    }
}
```

---

## Implementation Phases

### Phase 5e-1: RepoSpec Capture Infrastructure - COMPLETE

**Goal**: Create infrastructure to capture RepoSpecs during extension execution.

1. [x] Create `kuro_bzlmod/src/repo_spec.rs`:
   - [x] `RepoSpec` type with `repo_rule_id` and `attributes`
   - [x] Thread-local `RepoSpecRegistry` for capture during execution
   - [x] `with_repo_spec_registry()` for scoped capture
   - [x] `record_repo_spec()` for recording during rule invocation
   - [x] `in_extension_context()` to check execution context
   - [x] Unit tests for all functionality

2. [x] Modify `FrozenStarlarkRepositoryRule::invoke()` in `repository_rule.rs`:
   - [x] Check if in extension context via `in_extension_context()`
   - [x] If true: capture RepoSpec (NOT execute), return None
   - [x] If false: fall back to existing behavior (record RepositoryInvocation)

3. [x] Update `kuro_bzlmod/src/lib.rs`:
   - [x] Export `RepoSpec`, `record_repo_spec`, `in_extension_context`, `with_repo_spec_registry`

**Files modified**:
- `kuro_bzlmod/src/repo_spec.rs` (new)
- `kuro_bzlmod/src/lib.rs`
- `kuro_interpreter_for_build/src/repository_rule.rs`

### Phase 5e-2: Module Extension DICE Key

**Goal**: Create DICE infrastructure for extension evaluation (captures specs, no downloads).

1. Create `kuro_bzlmod/src/extension_execution_dice.rs`:

```rust
/// DICE key for module extension evaluation.
///
/// When computed, this:
/// 1. Loads the extension's .bzl file
/// 2. Builds module_ctx from aggregated tags
/// 3. Executes implementation(module_ctx)
/// 4. Captures RepoSpecs (NO downloads)
/// 5. Returns ModuleExtensionResult
#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative, Dupe)]
#[display("ModuleExtensionKey({})", extension_id)]
pub struct ModuleExtensionExecutionKey {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,
    /// Hash of input tags for cache invalidation.
    pub input_hash: Arc<str>,
}

#[async_trait]
impl Key for ModuleExtensionExecutionKey {
    type Value = kuro_error::Result<Arc<ModuleExtensionResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        // 1. Create temporary working directory
        let temp_dir = create_temp_extension_dir(&self.extension_id)?;

        // 2. Load extension .bzl file
        // 3. Build module_ctx with temp working dir
        // 4. Execute with RepoSpec capture
        let (result, specs) = with_repo_spec_registry(|| {
            execute_extension_impl(ctx, &self.extension_id, &temp_dir)
        });

        // 5. Delete temporary working directory
        std::fs::remove_dir_all(&temp_dir).ok();

        // 6. Build result with canonical names
        let canonical_names = build_canonical_names(&self.extension_id, &specs);

        Ok(Arc::new(ModuleExtensionResult {
            extension_id: self.extension_id.clone(),
            input_hash: self.input_hash.to_string(),
            generated_repo_specs: specs,
            canonical_names,
        }))
    }
}

/// Build canonical names for extension repos.
/// Format: _main~{extension_unique_name}~{internal_name}
fn build_canonical_names(
    extension_id: &str,
    specs: &HashMap<String, RepoSpec>,
) -> HashMap<String, String> {
    let ext_name = extract_extension_name(extension_id);
    specs.keys()
        .map(|internal| {
            let canonical = format!("_main~{}~{}", ext_name, internal);
            (internal.clone(), canonical)
        })
        .collect()
}
```

2. Wire up in extension aggregation:
   - After collecting `use_extension()` calls
   - Execute each extension via `ModuleExtensionExecutionKey`
   - Collect `ModuleExtensionResult` for cell registration

**Files to modify**:
- `kuro_bzlmod/src/extension_execution_dice.rs` (new)
- `kuro_bzlmod/src/lib.rs`
- `kuro_interpreter_for_build/src/extension_execution.rs`

### Phase 5e-3: Lazy Repository Execution - COMPLETE

**Goal**: Execute repository rules on-demand when repos are accessed.

1. [x] Create `ExtensionRepoExecutionKey` in `repository_execution.rs`:
   - [x] DICE key with canonical_name, extension_id, spec_hash, repo_spec fields
   - [x] Manual Hash/Eq implementation (since RepoSpec contains HashMap)
   - [x] compute() implementation with logging and stub execution
   - [x] Helper function `repo_spec_to_invocation()` for conversion
   - [x] Helper function `extract_rule_name_from_id()` for parsing repo_rule_id

2. [x] Export from `kuro_bzlmod/src/lib.rs`:
   - [x] `ExtensionRepoExecutionKey`
   - [x] `repo_spec_to_invocation`

3. [x] Unit tests:
   - [x] `test_extension_repo_key_creation` - basic key creation
   - [x] `test_extension_repo_key_from_arcs` - Arc-based creation
   - [x] `test_extension_repo_key_display` - Display format
   - [x] `test_extension_repo_key_hash_stability` - hash determinism
   - [x] `test_repo_spec_to_invocation_basic` - basic conversion
   - [x] `test_repo_spec_to_invocation_with_complex_attrs` - complex attributes
   - [x] `test_repo_spec_to_invocation_no_attrs` - empty attributes
   - [x] `test_repo_spec_to_invocation_invalid_rule_id` - error handling
   - [x] `test_extract_rule_name_from_id` - rule name extraction

**Code sample** (original plan for reference):

```rust
/// DICE key for lazy execution of extension-generated repositories.
///
/// This key is computed when @repo_name is first accessed in a build.
/// It takes a RepoSpec and materializes the repository.
#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative, Dupe)]
#[display("ExtensionRepoKey({}, {})", canonical_name, spec_hash)]
pub struct ExtensionRepoExecutionKey {
    /// Canonical repo name (e.g., "_main~pip~numpy")
    pub canonical_name: Arc<str>,
    /// Extension that generated this repo
    pub extension_id: Arc<str>,
    /// Hash of RepoSpec for cache invalidation
    pub spec_hash: Arc<str>,
    /// The RepoSpec to execute (serialized)
    pub repo_spec: Arc<RepoSpec>,
}

#[async_trait]
impl Key for ExtensionRepoExecutionKey {
    type Value = kuro_error::Result<Arc<RepositoryRuleResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        tracing::info!(
            "Lazily executing repository '{}' from extension '{}'",
            self.canonical_name,
            self.extension_id
        );

        // Convert RepoSpec to RepositoryInvocation
        let invocation = repo_spec_to_invocation(
            &self.canonical_name,
            &self.repo_spec,
        )?;

        // Execute via existing repository executor
        let repo_path = PathBuf::from("bazel-external").join(self.canonical_name.as_ref());
        execute_repository_invocation(&invocation, &repo_path).await?;

        Ok(Arc::new(RepositoryRuleResult::success(
            self.canonical_name.to_string(),
            repo_path,
        )))
    }
}
```

4. [ ] Integration with cell resolver (FUTURE - Phase 5e-5):
   - When `@repo_name` is accessed and cell is "pending"
   - Look up the extension result for this repo
   - Trigger `ExtensionRepoExecutionKey` computation
   - Update cell path to materialized location

**Files modified**:
- `kuro_bzlmod/src/repository_execution.rs` - DONE
- `kuro_bzlmod/src/lib.rs` - DONE
- `kuro_common/src/legacy_configs/cells.rs` - FUTURE (Phase 5e-5)

### Phase 5e-4: Extension Lockfile Integration - COMPLETE

**Goal**: Cache extension evaluation results with Bazel-compatible format.

1. [x] Update `lockfile.rs` with new format:

```rust
/// Extension lock data matching Bazel's format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionData {
    /// General extension data (not OS-specific).
    #[serde(default)]
    pub general: Option<LockfileExtensionGeneral>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileExtensionGeneral {
    /// Transitive digest of all .bzl files the extension depends on.
    pub bzl_transitive_digest: String,

    /// Digest of all module usages (tags passed to the extension).
    pub usages_digest: String,

    /// Generated repository specifications.
    /// Keys are internal names, values are full RepoSpec data.
    #[serde(default)]
    pub generated_repo_specs: HashMap<String, LockfileRepoSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockfileRepoSpec {
    /// Repository rule identifier.
    /// Format: "@@module//path:file.bzl%rule_name"
    pub repo_rule_id: String,

    /// All attributes (serialized as JSON values).
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}
```

2. [x] Lockfile operations:
   - [x] `get_extension_cache()`: Check for valid cached result
   - [x] `set_extension_cache()`: Store extension result
   - [x] Cache hit validation: compare `bzl_transitive_digest` + `usages_digest`
   - [x] `AttrValue` <-> `serde_json::Value` conversion functions
   - [x] `LockfileRepoSpec::from_repo_spec()` and `to_repo_spec()` conversions
   - [x] Unit tests for all functionality (14 new tests)

**Files modified**:
- `kuro_bzlmod/src/lockfile.rs` - DONE

### Phase 5e-5: Cell Registration for Pending Repos - COMPLETE

**Goal**: Register extension repos as "pending" cells before materialization.

1. [x] Add `ExtensionRepoCellSetup` to `ExternalCellOrigin`:
   - [x] New struct in `kuro_core/src/cells/external.rs` with canonical_name, extension_id, internal_name, spec_hash, materialized fields
   - [x] New `ExtensionRepo` variant in `ExternalCellOrigin` enum
   - [x] Update `Display` impl and match statements in `buck_out_path.rs`

2. [x] Create `kuro_bzlmod/src/pending_repo_cells.rs`:
   - [x] `PendingRepoCell` struct for pending repo definitions
   - [x] `RepoAlias` struct for apparent -> canonical mappings
   - [x] `ExtensionCellDefinitions` to hold cells and aliases
   - [x] `build_extension_cells()` to create cells from `ModuleExtensionResult`
   - [x] `build_use_repo_aliases()` to create aliases from `use_repo()` declarations
   - [x] `build_extension_cell_definitions()` combining cells + aliases
   - [x] `build_all_extension_cells()` for multiple extensions
   - [x] `is_extension_repo_canonical_name()` to detect extension repo names
   - [x] `parse_canonical_name()` to extract components from canonical names
   - [x] Unit tests (10 tests covering all functionality)

3. [x] Export new types from `kuro_bzlmod/src/lib.rs`:
   - [x] `PendingRepoCell`, `RepoAlias`, `ExtensionCellDefinitions`
   - [x] All builder functions

4. [x] Integration with `cells.rs`:
   - [x] Import `ExtensionRepoCellSetup` from kuro_core
   - [x] Add `register_extension_cells()` function for DICE integration

**Files modified**:
- `kuro_core/src/cells/external.rs` - Added `ExtensionRepoCellSetup` and `ExtensionRepo` variant
- `kuro_core/src/fs/buck_out_path.rs` - Handle new variant in match statements
- `kuro_bzlmod/src/pending_repo_cells.rs` - NEW: Cell registration infrastructure
- `kuro_bzlmod/src/lib.rs` - Export new types
- `kuro_common/src/legacy_configs/cells.rs` - `register_extension_cells()` function

**Note**: Full DICE integration (triggering lazy execution when pending cells are accessed) is future work.
The infrastructure is in place; actual extension execution wiring will happen when `ModuleExtensionExecutionKey::compute()`
is fully implemented.

### Phase 5e-6: module_ctx Temporary Working Directory - COMPLETE

**Goal**: Use temporary directory for module_ctx that's deleted after extension.

1. [x] Modify `ModuleContext`:

```rust
pub struct ModuleContext {
    modules: Vec<SerializedModule>,
    root_module_has_non_dev_dependency: bool,
    /// TEMPORARY working directory for I/O during extension.
    /// Deleted when extension completes - NOT the repository output.
    working_dir: Option<PathBuf>,
    /// Whether this working dir should be deleted on close.
    delete_on_close: bool,
}

impl ModuleContext {
    /// Create with a temporary working directory.
    pub fn with_temp_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self.delete_on_close = true; // Key difference from repository_ctx
        self
    }
}
```

2. [x] Implement basic I/O methods that operate on temp dir:
   - [x] `download()`: Downloads to temp dir (for inspection during extension) - stub exists
   - [x] `file()`: Creates files in temp dir - stub exists
   - [x] `execute()`: Runs commands with temp dir as cwd - stub exists
   - These are for extension logic, NOT for creating repos

3. [x] Extension lifecycle (infrastructure complete in extension_execution_dice.rs):
   - [x] Create temp dir at start (`create_temp_extension_dir()`)
   - [x] Run implementation with temp dir (stub, uses `with_temp_working_dir()`)
   - [x] Capture RepoSpecs (`with_repo_spec_registry()`)
   - [x] Delete temp dir at end (cleanup happens after execution)

**Important**: `module_ctx.file()` in extension is NOT how repos are created.
Repos are created by calling repository rules like `http_archive(name="foo")`,
which captures a RepoSpec for later execution.

**Files to modify**:
- `kuro_interpreter_for_build/src/module_ctx.rs`
- `kuro_bzlmod/src/extension_execution_dice.rs`

---

## Key Files Summary

| File | Status | Changes |
|------|--------|---------|
| `kuro_bzlmod/src/repo_spec.rs` | ✅ **Done** | RepoSpec type, thread-local capture registry |
| `kuro_bzlmod/src/extension_execution_dice.rs` | ✅ **Done** | ModuleExtensionExecutionKey (captures specs) |
| `kuro_bzlmod/src/repository_execution.rs` | ✅ **Done** | Add ExtensionRepoExecutionKey (lazy execution) |
| `kuro_interpreter_for_build/src/repository_rule.rs` | ✅ **Done** | Capture RepoSpec in extension context |
| `kuro_interpreter_for_build/src/module_ctx.rs` | ✅ **Done** | Temp working dir, delete_on_close, with_temp_working_dir() |
| `kuro_bzlmod/src/lockfile.rs` | ✅ **Done** | generatedRepoSpecs format |
| `kuro_common/src/legacy_configs/cells.rs` | ✅ **Done** | Pending cell registration |
| `kuro_bzlmod/src/pending_repo_cells.rs` | ✅ **New** | Cell definitions from extension results |
| `kuro_core/src/cells/external.rs` | ✅ **Done** | ExtensionRepoCellSetup + ExtensionRepo variant |
| `kuro_bzlmod/src/lib.rs` | ✅ **Done** | Export new types |

---

## Integration Work (Phase 5e-7) - COMPLETE

Additional integration work done to connect the pieces together:

### 1. ModuleExtensionExecutionKey Enhanced

**File**: `kuro_bzlmod/src/extension_execution_dice.rs`

Changes:
- [x] Added `AggregatedExtension` field to store all extension tags
- [x] Added `root_module_name` field for ModuleContext construction
- [x] Manual Hash/Eq implementation (AggregatedExtension contains HashMap)
- [x] `compute()` logs extension tags and modules for debugging
- [x] Documented the full Starlark execution flow (requires kuro_interpreter_for_build)
- [x] Added `new_minimal()` constructor for backward compatibility

**Note**: Full Starlark extension execution requires cross-crate integration with
`kuro_interpreter_for_build::extension_execution::build_module_context()`. This will be
implemented in a future phase when the interpreter is wired up to DICE.

### 2. ExtensionRepoExecutionKey Wired to Repository Executor

**File**: `kuro_bzlmod/src/repository_execution.rs`

Changes:
- [x] Added `project_root` field for repository materialization path
- [x] `compute()` now calls `execute_repository_rule()` for actual execution
- [x] Supports http_archive, git_repository, local_repository rules
- [x] Added `new_with_cwd()` constructor for testing convenience

This enables lazy repository materialization when extension-generated repos are accessed.

### 3. New Exports from kuro_bzlmod

**File**: `kuro_bzlmod/src/lib.rs`

Added exports:
- `ModuleExtensionError`
- `AggregatedExtension`
- `compute_extension_input_hash`
- `aggregate_extensions`

### What Remains for Full Integration

1. **Starlark Extension Execution**: Wire `ModuleExtensionExecutionKey::compute()` to:
   - Load the extension's .bzl file via Starlark interpreter
   - Call `build_module_context()` from `kuro_interpreter_for_build`
   - Execute `extension.implementation(module_ctx)` via Starlark evaluator

2. **Cell Resolver Integration**: When `@repo_name` is accessed and cell is "pending":
   - Look up the extension result for this repo
   - Trigger `ExtensionRepoExecutionKey` computation
   - Update cell path to materialized location

3. **Lockfile Integration**: Store and retrieve extension results from lockfile cache

---

## Success Criteria

### Core Deferred Behavior
- [ ] Extension evaluation returns RepoSpecs WITHOUT downloading anything (needs Starlark execution)
- [x] Repository rules execute lazily on FIRST ACCESS via DICE (`ExtensionRepoExecutionKey::compute()` calls `execute_repository_rule()`)
- [x] Second access uses DICE cache (no re-execution) - DICE Key infrastructure in place
- [x] Temp working directory infrastructure for module_ctx (cleanup in extension_execution_dice.rs)

### Cell Registration
- [x] Pending cells registered for all extension-generated repos (infrastructure complete)
- [x] Canonical naming: `_main~{ext}~{repo}` format used (`build_canonical_names()`)
- [x] `use_repo()` apparent names resolve to canonical (`build_use_repo_aliases()`)
- [ ] Cell access triggers lazy materialization (needs cell resolver wiring)

### Lockfile
- [ ] Lockfile contains `generatedRepoSpecs` with full RepoSpec data
- [ ] `bzlTransitiveDigest` and `usagesDigest` used for invalidation
- [ ] Second build uses lockfile cache (no extension re-execution)

### Integration
- [ ] Simple extension that creates a filegroup works
- [ ] Extension `download()` works (for inspection, not repo creation)
- [ ] `bazel_features` extension creates version repos correctly
- [ ] Error messages clear when extension fails or repo not found

### Stretch Goals
- [ ] `rules_python` `pip.parse()` extension works
- [ ] Extension execution parallelized where safe
- [ ] Lockfile format fully Bazel-compatible

---

## Testing Strategy

### Unit Tests

1. `RepoSpec` creation and hashing
2. `with_repo_spec_registry()` capture behavior
3. `in_extension_context()` detection
4. Lockfile serialization/deserialization for new format

### Integration Tests

1. Extension captures RepoSpecs without network activity
2. Lazy execution triggered on `@repo_name` access
3. DICE caching prevents re-execution
4. Lockfile cache prevents re-evaluation
5. Temp dir cleanup after extension

### Manual Test Suite

```python
# In MODULE.bazel
bazel_features = use_extension("@bazel_features//private:extensions.bzl", "bazel_features")
use_repo(bazel_features, "bazel_features_version", "bazel_features_globals")

# Test extension
test_ext = use_extension("//:test_extension.bzl", "test_ext")
test_ext.config(name = "my_config", value = "test")
use_repo(test_ext, "test_repo")
```

```python
# In test_extension.bzl
def _test_ext_impl(module_ctx):
    for mod in module_ctx.modules:
        for cfg in mod.tags.config:
            print("Config:", cfg.name, cfg.value)

    # This captures a RepoSpec, does NOT execute immediately
    http_archive(
        name = "test_repo",
        url = "https://example.com/test.tar.gz",
        sha256 = "abc123",
    )

    # This file goes to temp dir, deleted after extension
    module_ctx.file("debug.txt", "extension ran")

test_ext = module_extension(
    implementation = _test_ext_impl,
    tag_classes = {
        "config": tag_class(attrs = {
            "name": attr.string(mandatory = True),
            "value": attr.string(default = ""),
        }),
    },
)
```

**Verification:**
1. Run with network disabled → extension should succeed (no downloads)
2. Access `@test_repo` → download happens NOW
3. Access `@test_repo` again → uses DICE cache, no download
4. Clean and rebuild → uses lockfile, no extension re-execution
5. Verify no temp dir remains after extension

---

## Dependencies

- Phase 5d (DICE Integration) - **COMPLETE**
- Phase 5c (@bazel_tools bundling) - **COMPLETE**
- Phase 5b (Build Integration) - **COMPLETE** for what we need

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Extension I/O security | Medium | High | Sandbox I/O to temp dir only |
| Lazy execution ordering | Medium | Medium | DICE handles dependencies |
| Lockfile format drift from Bazel | Low | Medium | Version lockfile format |
| Extension error messages poor | High | Medium | Add detailed error context |
| Temp dir not cleaned on crash | Medium | Low | Use temp dir in known location |

---

## Bazel Research Reference

### Key Findings

1. **StarlarkBaseExternalContext**: Both `module_ctx` and `repository_ctx` share I/O implementations

2. **Working Directory Lifecycle**:
   - `module_ctx`: Temporary, `shouldDeleteWorkingDirectoryOnClose()` = true
   - `repository_ctx`: Permanent, becomes the repository output

3. **SingleExtensionValue** contains:
   - `generatedRepoSpecs`: Map of internal_name → RepoSpec
   - `canonicalRepoNameToInternalNames`: Bidirectional mapping
   - `lockFileInfo`: For caching
   - `facts`: Persistent data for future evaluations

4. **Lockfile format**:
   ```json
   "@@module~//path:ext.bzl%name": {
     "general": {
       "bzlTransitiveDigest": "...",
       "usagesDigest": "...",
       "generatedRepoSpecs": {
         "repo_name": {
           "repoRuleId": "@@bazel_tools//...%http_archive",
           "attributes": { ... }
         }
       }
     }
   }
   ```

5. **Repository naming**: `{extensionUniqueName}~{internalName}` (e.g., `_main~maven~junit`)

---

## References

**Bazel Source:**
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java` - Extension execution
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionContext.java` - module_ctx implementation
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/LockFileModuleExtension.java` - Extension lockfile
- `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/StarlarkBaseExternalContext.java` - Shared I/O

**Kuro Patterns:**
- `kuro_bzlmod/src/repository_executor.rs` - Repository execution (reuse for lazy execution)
- `kuro_bzlmod/src/repository_execution.rs` - DICE key pattern
- `kuro_bzlmod/src/repository_invocations.rs` - Thread-local registry pattern
- `kuro_interpreter_for_build/src/repository_ctx.rs` - repository_ctx I/O methods
