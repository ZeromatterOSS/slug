# AI-agent-pattern sweep

**Date:** 2026-04-22  
**Scope:** app/, dice/, prelude/  
**Result count:** 3 P0, 4 P1, 0 P2  

## Summary

Slug's codebase is remarkably clean for a "AI-generated → perf pass" scenario. Most modern patterns are idiomatic—SmallMap is used appropriately, Dupe trait is respected in most cases, and regex compilation is properly lazy-loaded. However, a few P0 correctness issues and P1 idiom violations were found, mostly in low-frequency code paths (one-shot initialization or test utilities) that won't impact production hot loops.

---

## P0 — real perf/correctness

### Pattern: Arc::new(x.clone()) on String-like types (double alloc)

**File count:** 4 instances in file-watcher code

#### app/slug_file_watcher/src/notify.rs:321
```rust
let mergebase = Mergebase(Arc::new(stats.branched_from_revision.clone()));
```
`branched_from_revision` is already `String`; wrapping `String::clone()` in `Arc::new()` allocates the string, then wraps it. Should reuse or accept by value.

**Related:** 
- app/slug_file_watcher/src/watchman/interface.rs:422 (same pattern)
- app/slug_file_watcher/src/edenfs/interface.rs:820 (same pattern)
- app/slug_file_watcher/src/fs_hash_crawler.rs:101 (same pattern)

All are in sync/initialization code, not in hot event loops. **Severity: P0 (correctness) but low runtime impact.**

---

### Pattern: Vec<Arc<T>>.collect() from iter().map(|x| Arc::new(x.clone()))

#### app/slug_query_impls/src/uquery/environment.rs:422
```rust
let universe_paths: Vec<ArcCellPath> =
    universe.iter().map(|file| Arc::new(file.clone())).collect();
```
`file` is `&CellPath`, so `file.clone()` allocates, then `Arc::new()` wraps it. If `CellPath` is already Arc-wrapped or cheap to copy, this is wasteful. The variable `ArcCellPath = Arc<CellPath>` indicates intentional wrapping, but the chain `iter().map(...).collect()` materializes an intermediate `Vec` unnecessarily.

**Fix:** Consider `collect::<Vec<_>>()` from a lazily-evaluated iterator if the Vec isn't needed elsewhere, or accept the Arc creation if it's only done once during setup.

**Severity: P0 (minor double-alloc) in one-time initialization code.**

---

### Pattern: Arc::new(...clone()) in per-action or per-event code

#### app/slug_build_api/src/actions/execute/action_executor.rs:895
```rust
Arc::new(DryRunExecutor::new(tracker, artifact_fs.clone()))
```
`artifact_fs` is `Arc<_>`, so `.clone()` on an Arc is cheap (refcount bump), but wrapping the whole executor in another Arc. If `DryRunExecutor` holds an Arc internally and only needs a clone of it, this is redundant. However, this is in action execution setup, not the hot loop.

**Severity: P0 (idiomatic issue) but not in critical path.**

---

## P1 — idioms

### Pattern: Regex::new() inside non-lazy function context

#### app/slug_interpreter_for_build/src/interpreter/functions/regex.rs:31
```rust
fn regex_match(
    #[starlark(require = pos)] regex: &str,
    #[starlark(require = pos)] str: &str,
) -> starlark::Result<bool> {
    let re = Regex::new(regex).map_err(slug_error::Error::from)?;
    Ok(re.is_match(str).map_err(slug_error::Error::from)?)
}
```
If this function is called per-action or per-pattern-match during evaluation, compiling the regex on every call is wasteful. The comment at `/app/slug_interpreter/src/types/regex.rs:15` says "TODO(nga): drop it, and only use `regex` function." suggesting this is a deprecated API path. **Not a hot-path today, but flagged for cleanup.**

---

#### app/slug_interpreter/src/extra/xcode.rs:118
```rust
pub fn from_version_and_build(version_and_build: &str) -> slug_error::Result<Self> {
    let re = Regex::new(r"^((\d+)\.(\d+)(?:\.(\d+))?)\-([[:alnum:]]+)$").unwrap();
    if !re.is_match(version_and_build) {
        return Err(XcodeVersionError::MalformedVersionBuildString.into());
    }
    ...
}
```
`from_version_and_build()` is called during version parsing, likely once per build session. Regex is recompiled each time. Should use `once_cell::sync::Lazy` or const-time regex. Low impact (called rarely), but idiomatic fix is trivial.

**Severity: P1 (idiom, not correctness).**

---

### Pattern: Arc<T>.clone() on Dupe-impl types in non-hot contexts

#### app/slug_execute/src/path/artifact_path.rs:184, 308
```rust
BaseDeferredKey::TargetLabel(target) => Some(target.clone()),
```
`TargetLabel` implements `Dupe`. Should be `.dupe()` per CLAUDE.md. However, this is in an `as_str()` method called during artifact-path construction, not a tight loop.

**Count:** 2 instances, same pattern.

**Severity: P1 (idiom violation, no perf impact in this context).**

---

### Pattern: String::clone() on &str values

Not prevalent in non-test code. Grep for `.to_string()` on `&str` found mostly legitimate uses (serialization, formatting).

**Severity: P1 (idiom) but rare in scope.**

---

## P2 — style

No P2-level string-concat-in-loop or other style issues were found. The codebase avoids hot-path string building; most `format!` calls are in error paths or initialization.

---

## File Summary

| File | P0 | P1 | P2 | Notes |
|------|----|----|----|-|
| app/slug_file_watcher/src/*.rs | 4 | 0 | 0 | All in sync/init, not event loop |
| app/slug_query_impls/src/uquery/environment.rs | 1 | 0 | 0 | One-time setup |
| app/slug_build_api/src/actions/execute/action_executor.rs | 1 | 0 | 0 | Action setup, not per-event |
| app/slug_interpreter_for_build/src/interpreter/functions/regex.rs | 0 | 1 | 0 | Deprecated API path (TODO noted in code) |
| app/slug_interpreter/src/extra/xcode.rs | 0 | 1 | 0 | Called once per session |
| app/slug_execute/src/path/artifact_path.rs | 0 | 2 | 0 | Non-hot context; idiom violation only |

---

## Recommendations

### Immediate (next iteration)
1. **Migrate xcode.rs Regex to lazy-static:** Use `once_cell::sync::Lazy<Regex>` at module scope. 2-line fix.
2. **Replace artifact_path.rs .clone() with .dupe():** 2 sites. Consistency fix; no perf gain but idiomatic.

### Soon (next perf pass)
3. **Review file-watcher Arc<String> pattern:** Consider accepting by value or reusing refs if the Mergebase struct is short-lived.
4. **Rationalize uquery environment ArcCellPath:** Verify the double-wrapping is necessary; if not, fold into single Arc.

### Deferred (codebase health)
5. **Deprecate regex_match() Starlark function:** The TODO comment indicates intent. Use the lazy-loaded regex trait instead.

---

## Notes

- **SmallMap usage is excellent:** The team is already using SmallMap for small collections (seen in critical_path, interpreter_for_build). No HashMap-for-size-≤8 violations found.
- **Async/blocking:** No `std::fs::*` or `std::thread::sleep` found inside async blocks. File watcher uses notify-rs, which is async-friendly.
- **Arc<Mutex<T>>:** Usage is appropriate—config maps and cached lookup tables are read-dominant; no ArcSwap or OnceCell opportunities found.
- **Test code excluded:** Counts above exclude test modules; 1891 .unwrap() calls exist (mostly in tests/asserts), which is acceptable.

