# Plan 62: Opus Review Findings — Round 5

> Created: 2026-06-08
> Model: anthropic/opus4-8 (3 parallel subagents, 2 completed, 1 timed out)
> Scope: Full DICE replay correctness audit, lockfile parity audit, resolution correctness audit

## CRITICAL Issues

### C1: `compute_bzl_transitive_digest` v1 doesn't hash file content (stale cache risk)

**File**: `extension_execution_dice.rs:2455-2466`

The v1 digest only hashes the extension ID string, not any `.bzl` file content. If an extension's implementation changes on disk while the extension ID remains the same, DICE will produce a stale cache hit.

**Impact**: `.bzl` file changes that don't change the extension ID will NOT invalidate cached extension results.

**Status**: ALREADY DOCUMENTED as SLUG-PRIVATE DIGEST in Phase 14b. The production path through `ExtensionBzlTransitiveDigestKey` hashes actual file content. The v1 fallback is only used when `file_states` is empty. Risk is real but mitigated by the production path.

## HIGH Issues

### H1: `enforce_error_mode` is dead code — never called from production paths

**File**: `lockfile.rs:1159-1225`

In error mode, extension cache misses (digest drift, recorded-input drift, facts drift) silently trigger re-evaluation instead of erroring. Bazel's `SingleExtensionEvalFunction` converts these into hard errors in error mode.

**Impact**: `--lockfile_mode=error` does not fully guarantee that the build will fail rather than proceed with changed resolution results.

### H2: `enforce_error_mode` is incomplete even if called

Does not check: (a) module extension bzl/usages digest drift, (b) recorded inputs drift, (c) facts drift, (d) non-reproducible extension enforcement — all of which Bazel checks in error mode.

### H3: LateBinding globals accessed from DICE compute are untracked dependencies

Four `LateBinding` globals (`BZLMOD_CLEAN_GRAPH_IO_IMPL`, `MODULE_EXTENSION_EXECUTOR_IMPL`, `REPOSITORY_MATERIALIZATION_STATE_READER_IMPL`, `STARLARK_REPO_RULE_EXECUTOR_IMPL`) are accessed within DICE compute methods. DICE cannot track these as dependencies.

**Impact**: If any implementation were swapped between DICE versions, stale results would be served. In practice, these are initialized once at startup and are immutable within a DICE session.

### H4: `validity() -> false` on 5 keys masks untracked dependencies

Five keys return `validity() -> false` as a workaround for the LateBinding problem. This forces equality checks on every cache lookup and can mask incorrect invalidation if the equality check itself is wrong.

## MEDIUM Issues

### M1: Ambient state reads in DICE compute
- `std::env::temp_dir()` in extension eval
- `record_bzlmod_event` increments global AtomicU64 counters
- `LAST_RECORDED_BZLMOD_RESOLUTION_DIGEST` is a mutable global

### M2: InjectedKey data bypasses DICE tracking — invalidation may not be precise

### M3: `ExtensionSpokesKey` compares by digest fields, not full aggregated content — hash collision risk

## LOW Issues

### L1: `RepoSpec.local` field silently lost on lockfile round-trip (matches Bazel behavior, undocumented)
### L2: No merge-conflict detection in parse errors
### L3: Error message for unsupported version omits `--lockfile_mode=update` flag
### L4: Missing explicit `validity` implementations in dice_graph.rs (default `true` is optimistic)
### L5: Test-only filesystem reads outside DICE tracking
