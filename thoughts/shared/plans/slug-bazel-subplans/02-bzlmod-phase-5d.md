# bzlmod Phase 5d: DICE Integration for Repository Rule Execution

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)
> **Parent Phase**: [Phase 5 Overview](./02-bzlmod-phase-5-overview.md)

## Status: COMPLETE

**Completed:** 2026-01-29
**Commit:** d9fe62d - Implement DICE integration for repository rule execution (Phase 5d)

### Implementation Summary

| Component | File | Description |
|-----------|------|-------------|
| Invocation Registry | `slug_bzlmod/src/repository_invocations.rs` | Thread-local registry for recording rule invocations |
| DICE Execution Key | `slug_bzlmod/src/repository_execution.rs` | `RepositoryRuleExecutionKey` implementing `Key` trait |
| Repository Executor | `slug_bzlmod/src/repository_executor.rs` | Actual execution logic for http_archive, git_repository, local_repository |
| Lockfile Integration | `slug_bzlmod/src/lockfile.rs` | `RepositoryRuleLockEntry` for caching |

### Key Features Implemented

- **http_archive**: Download with URL fallback, SHA256/SRI verification, tar.gz/zip extraction, strip_prefix
- **git_repository**: Clone, fetch specific commit/tag/branch
- **local_repository**: Symlink to local paths
- **Stub generation**: Unknown rules get minimal BUILD.bazel
- **Completion markers**: `.slug_repo_complete` prevents re-execution

---

## Overview

Enable repository rules (like `http_archive`) to actually execute during module resolution, downloading and extracting dependencies. This bridges the gap between repository rule definitions (which exist) and actual repository materialization.

**Why this phase is critical:** Repository rules are currently defined but never invoked. When `http_archive(name = "foo", ...)` is called, it logs the invocation but doesn't download anything. This phase makes repository rules functional.

---

## Current State

**What exists:**
- `repository_rule()` global creates `FrozenStarlarkRepositoryRule` (`repository_rule.rs:224-300`)
- `repository_ctx` has full I/O methods: download, file, execute, symlink, etc. (`repository_ctx.rs:764-1392`)
- `FrozenStarlarkRepositoryRule::invoke()` is a stub that logs and returns None (`repository_rule.rs:306-327`)
- Bzlmod resolution completes through `ResolvedGraph` but doesn't invoke repository rules

**What's missing:**
- DICE key for repository rule execution
- Registry to collect repository rule invocations during MODULE.bazel parsing
- Invocation of implementation function with proper `repository_ctx`
- Integration with existing download/cache infrastructure
- Cell registration for executed repositories

---

## Bazel Caching Behavior (Reference)

Per Bazel's design, repository rules use a two-tier caching system:

**1. Repository Cache (shared, content-addressable):**
- Location: `~/.cache/bazel/_bazel_$USER/cache/repos/v1/` (configurable via `--repository_cache`)
- Structure: `content_addressable/sha256/{hex_hash}`
- Stores raw downloaded archives only
- Shared across all workspaces and Bazel versions
- Cache key: SHA256 hash of downloaded file

**2. External Directory (per-workspace):**
- Location: `$(bazel info output_base)/external/{repo_name}/`
- Contains materialized repositories where `repository_ctx` operates
- This is the working directory for repository rule execution
- Cleaned with `bazel clean --expunge`

**Slug Alignment:**
- Download cache: `~/.cache/slug/downloads/{integrity}` (SRI format - compatible)
- Working directory: `bazel-external/{repo_name}/` (per-workspace)
- Cell registration: `@repo_name` resolves to working directory

---

## Architecture

### 1. Repository Rule Invocation Registry

**File**: `slug_bzlmod/src/repository_invocations.rs` (new)

When parsing MODULE.bazel or extension .bzl files, repository rule invocations are recorded:

```rust
#[derive(Debug, Clone)]
pub struct RepositoryInvocation {
    /// The repository name (from `name` attribute)
    pub name: String,
    /// The repository rule being invoked (e.g., "http_archive")
    pub rule_name: String,
    /// Path to .bzl file containing the rule definition
    pub rule_source: CellPath,
    /// Attribute values passed to the invocation
    pub attrs: HashMap<String, AttrValue>,
    /// Source location for error reporting
    pub location: Option<FileSpan>,
}

/// Thread-safe registry for collecting invocations during parsing
pub struct RepositoryInvocationRegistry {
    invocations: Mutex<Vec<RepositoryInvocation>>,
}
```

### 2. DICE Key for Repository Rule Execution

**File**: `slug_bzlmod/src/repository_execution.rs` (new)

Follow the `GitFileOpsDelegateKey` pattern from `app/slug_external_cells/src/git.rs:377-408`:

```rust
#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("RepositoryRuleKey({}, {})", name, rule_name)]
pub struct RepositoryRuleExecutionKey {
    /// Repository name
    name: String,
    /// Rule name for debugging
    rule_name: String,
    /// Hash of attributes for cache invalidation
    attrs_hash: String,
}

#[async_trait]
impl Key for RepositoryRuleExecutionKey {
    type Value = slug_error::Result<Arc<RepositoryRuleResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellations: &CancellationContext,
    ) -> Self::Value {
        execute_repository_rule(ctx, &self.name, cancellations).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok() // Don't cache errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepositoryRuleResult {
    /// Path to the materialized repository
    pub repo_path: ProjectRelativePathBuf,
    /// Hash of repo contents for invalidation
    pub content_hash: Option<String>,
}
```

### 3. Execution Flow

**File**: `slug_bzlmod/src/repository_execution.rs`

```rust
async fn execute_repository_rule(
    ctx: &mut DiceComputations,
    invocation: &RepositoryInvocation,
    cancellations: &CancellationContext,
) -> slug_error::Result<Arc<RepositoryRuleResult>> {
    // 1. Determine working directory
    let working_dir = ctx.get_project_root()
        .join("bazel-external")
        .join(&invocation.name);

    // 2. Check if already materialized (via lockfile or marker)
    if is_repo_materialized(&working_dir)? {
        return Ok(Arc::new(RepositoryRuleResult {
            repo_path: working_dir.clone(),
            content_hash: read_content_hash(&working_dir)?,
        }));
    }

    // 3. Load the .bzl file containing the repository rule
    let module = ctx.load_bzl_file(&invocation.rule_source).await?;

    // 4. Get the frozen repository rule
    let rule = module.get_value(&invocation.rule_name)?
        .downcast::<FrozenStarlarkRepositoryRule>()?;

    // 5. Create RepositoryContext with working directory
    let repo_ctx = RepositoryContext::new(
        invocation.name.clone(),
        invocation.attrs.clone().into(),
        working_dir.clone(),
    );

    // 6. Invoke the implementation function
    let mut evaluator = Evaluator::new(&module);
    evaluator.eval_function(
        rule.implementation(),
        &[repo_ctx.to_value()],
        &[],
    )?;

    // 7. Write completion marker and content hash
    mark_repo_complete(&working_dir)?;

    Ok(Arc::new(RepositoryRuleResult {
        repo_path: working_dir,
        content_hash: compute_content_hash(&working_dir)?,
    }))
}
```

### 4. Integration with Download Infrastructure

**Reuse existing infrastructure:**

- `slug_bzlmod/src/fetch.rs` - `SourceFetcher` for HTTP downloads with retry
- `slug_bzlmod/src/cache.rs` - `ModuleCache` for content-addressable download storage
- `slug_bzlmod/src/integrity.rs` - SRI integrity verification
- `slug_http/src/client.rs` - HTTP client with redirect handling

**Changes to `repository_ctx.rs`:**

The current I/O methods in `RepositoryContext` use synchronous I/O (curl/wget for downloads). For DICE integration:

Option A: Keep synchronous I/O, wrap in blocking executor (simpler):
```rust
// In repository_execution.rs
let io = ctx.get_blocking_executor();
io.execute_io(Box::new(RepositoryRuleIoRequest { invocation, working_dir }), cancellations).await?;
```

Option B: Convert to async I/O using `SourceFetcher` (more consistent):
```rust
// Inject SourceFetcher into RepositoryContext
impl RepositoryContext {
    pub fn with_fetcher(name: String, attr: RepositoryAttr, working_dir: PathBuf, fetcher: Arc<SourceFetcher>) -> Self;
}
```

**Recommendation:** Start with Option A (blocking executor) for simplicity, then optimize to Option B if needed.

### 5. Cell Registration

After repository rule execution, register the repository as a cell:

**File**: `slug_common/src/legacy_configs/cells.rs`

```rust
/// Register a repository rule result as a cell
pub fn register_repository_cell(
    aggregator: &mut CellsAggregator,
    repo_name: &str,
    repo_path: &Path,
) -> slug_error::Result<()> {
    let cell_name = CellName::unchecked_new(repo_name)?;
    aggregator.add_cell(CellRootPathBuf::new(repo_path.to_path_buf()))?;
    aggregator.mark_external_cell(
        cell_name,
        ExternalCellOrigin::RepositoryRule(RepositoryRuleCellSetup {
            repo_name: repo_name.to_owned(),
            source_path: repo_path.to_path_buf(),
        }),
    )?;
    Ok(())
}
```

### 6. Lockfile Caching

**File**: `slug_bzlmod/src/lockfile.rs`

Add repository rule results to lockfile:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRuleLockEntry {
    /// Rule that created this repository
    pub rule_name: String,
    /// Hash of input attributes
    pub attrs_hash: String,
    /// Hash of downloaded content (for integrity)
    pub content_hash: Option<String>,
    /// URLs that were downloaded (for cache lookup)
    pub downloaded_urls: Vec<DownloadedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedFile {
    pub url: String,
    pub integrity: String,
    pub output_path: String,
}
```

---

## Changes Required

### Phase 5d-1: Repository Invocation Registry

1. Create `slug_bzlmod/src/repository_invocations.rs`
2. Modify `FrozenStarlarkRepositoryRule::invoke()` to record invocations
3. Thread registry through MODULE.bazel parsing

### Phase 5d-2: DICE Execution Key

1. Create `slug_bzlmod/src/repository_execution.rs`
2. Define `RepositoryRuleExecutionKey` implementing `Key` trait
3. Implement `execute_repository_rule()` function

### Phase 5d-3: Blocking I/O Integration

1. Create `RepositoryRuleIoRequest` implementing `IoRequest` trait
2. Wrap repository_ctx I/O in blocking executor calls
3. Connect to existing `SourceFetcher` for downloads

### Phase 5d-4: Cell Registration

1. Add `ExternalCellOrigin::RepositoryRule` variant
2. Implement `register_repository_cell()` function
3. Integrate with cell resolution after execution

### Phase 5d-5: Lockfile Integration

1. Add `RepositoryRuleLockEntry` to lockfile format
2. Cache repository rule results by attrs_hash
3. Skip execution on lockfile cache hit

---

## Key Files

| File | Purpose | Changes |
|------|---------|---------|
| `slug_bzlmod/src/repository_invocations.rs` | Invocation registry | New file |
| `slug_bzlmod/src/repository_execution.rs` | DICE key + execution | New file |
| `slug_interpreter_for_build/src/repository_rule.rs` | Rule definition | Modify `invoke()` to record |
| `slug_interpreter_for_build/src/repository_ctx.rs` | I/O methods | May need async variants |
| `slug_common/src/legacy_configs/cells.rs` | Cell registration | Add RepositoryRule origin |
| `slug_bzlmod/src/lockfile.rs` | Caching | Add repository rule entries |

---

## Success Criteria

### Automated Verification

- [ ] `http_archive(name = "foo", url = "...", sha256 = "...")` downloads and extracts archive
- [ ] Downloaded archives cached in `~/.cache/slug/downloads/`
- [ ] Extracted content in `bazel-external/{repo_name}/`
- [ ] `@foo//:target` resolves to extracted repository
- [ ] Second invocation uses cached download (no network activity)
- [ ] Lockfile contains repository rule execution results
- [ ] DICE caches execution results (no re-execution on rebuild)
- [ ] `repository_ctx.execute()` runs commands correctly
- [ ] `repository_ctx.file()` creates files in working directory
- [ ] `repository_ctx.symlink()` creates symlinks correctly

### Manual Verification

- [ ] Create MODULE.bazel with `http_archive` for a real dependency
- [ ] Verify archive downloaded and extracted correctly
- [ ] Build a target from the extracted repository
- [ ] Delete `bazel-external/`, rebuild, verify re-download
- [ ] Verify lockfile prevents re-download on clean rebuild

---

## Testing Strategy

1. **Unit tests** for `RepositoryInvocationRegistry`
2. **Integration tests** for `RepositoryRuleExecutionKey` with mock downloads
3. **E2E test** with real `http_archive` from BCR
4. **Add to manual test suite** in `tests/manual_test/`

---

## Dependencies

- Phase 5 (repository_rule/repository_ctx implementation) - **COMPLETE**
- Phase 5b (cell registration infrastructure) - **COMPLETE**
- Phase 5c (@bazel_tools bundling) - **COMPLETE**

---

## References

**Bazel Source:**
- `src/main/java/com/google/devtools/build/lib/rules/repository/RepositoryFunction.java` - Repository rule execution
- `src/main/java/com/google/devtools/build/lib/bazel/repository/downloader/HttpDownloader.java` - Download logic
- `src/main/java/com/google/devtools/build/lib/bazel/repository/cache/RepositoryCache.java` - Cache implementation

**Slug Patterns:**
- `app/slug_external_cells/src/git.rs:377-408` - `GitFileOpsDelegateKey` DICE pattern
- `app/slug_build_api/src/actions/calculation.rs:656-683` - `BuildKey` DICE pattern
- `app/slug_bzlmod/src/fetch.rs` - Existing download infrastructure
