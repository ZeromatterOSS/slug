# Plan 14c: On-demand Spoke Materialization (Phase 7.2 proper)

> Parent: Plan 10 Phase 7 follow-up

## Problem

After Plan 10 Phase 7 parallelized the eager spoke-materialization loop in
`module_extension_executor_impl.rs:411`, the loop still exists and still
runs. For the `crate` extension, that means 1230+ spoke repos materialize
(run their `repository_rule` implementations) whenever the extension
executes — even when the current build touches only a handful of them.

Observed disk and time waste in `bazel-external/` for
`@llvm-project//llvm:config`: 214 repos materialize, many unrelated
to the LLVM cc_library target. Kotlin toolchain archives, Swift NIO
libraries, Java JDK tarballs — all downloaded when building LLVM.

## Bazel Parity

Bazel materializes external repositories **on first demand** via the
`RepositoryDirectoryValue` Skyframe key. A repo is fetched only when some
target or another repo references a file inside it. The per-extension
wrapper (`ModuleExtensionEvalValue`) just captures `RepoSpec` objects —
execution is deferred.

When a Starlark `module_ctx.path(Label("@repo//..."))` call in extension
B references repo from extension A, Bazel's Skyframe:
1. Starts B's extension eval
2. Hits `path(Label(@A_repo))` — requires @A_repo's directory
3. Requests `RepositoryDirectoryValue(@A_repo)` from Skyframe
4. If not ready, returns `null` → Skyframe suspends B and evaluates A
5. After A completes, re-runs B's extension from scratch

Kuro can't do Skyframe restarts because Starlark eval is synchronous.
Current workaround: eagerly materialize all of A's repos when A's
extension completes, so B finds them on disk.

Better workaround: pre-compute the set of repos B references before
running B's Starlark impl. Materialize only those.

## Design

Three sub-phases, each independently shippable.

### Phase 14c.1: Add lazy-safe Label resolution with pre-scan

Extract the set of cross-extension Label references from each
extension's `.bzl` module at parse time. Store alongside
`AggregatedExtension`. Before running B's Starlark impl, materialize the
subset of other extensions' repos that B references.

**Location**: `app/kuro_bzlmod/src/extensions.rs` `AggregatedExtension`
struct.

Add field:
```rust
pub cross_extension_label_refs: Vec<String>,  // repo names only
```

Populate during parse: scan the extension's .bzl AST for string literals
matching `Label("@REPO//")` and `@REPO//` patterns in known-callsites
(`ctx.path(...)`, `ctx.execute(...)`, `ctx.read(...)`,
`ctx.file(...)`, `ctx.download(...)`, etc.). Extract the repo name.

Scanner: use starlark's AST visitor. Implement
`extract_label_refs_from_bzl(ast: AstModule) -> Vec<String>`.

### Phase 14c.2: Drop the eager spoke loop

Once pre-scan reliably identifies referenced repos, the eager loop in
`module_extension_executor_impl.rs:411` becomes unnecessary for the
cross-extension-reference case.

Keep dynamic-cell registration (line 427-430): it's cheap and informs
cell resolution. Remove the `try_compute_join` over all specs
(line 452-483).

Add new pre-materialization step: before entering
`try_execute_starlark`, iterate `cross_extension_label_refs`, look up
the owning extension for each, and `ctx.compute` an
`ExtensionRepoExecutionKey` for the specific target repo.

**Location**: `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`
`execute_extension` method.

### Phase 14c.3: Make `get_file_ops_delegate` fully lazy

When a file is read from an extension cell and the repo isn't
materialized, `get_file_ops_delegate`
(`app/kuro_external_cells/src/extension_repo.rs:407`) already computes
`ExtensionRepoExecutionKey` on demand. Verify this still works after
the eager loop is removed — specifically, that the pre-compute in
Phase 14c.2 hasn't regressed the lazy path.

## Touchpoints

- `app/kuro_bzlmod/src/extensions.rs` —
  `AggregatedExtension::cross_extension_label_refs` field,
  population during parse
- `app/kuro_bzlmod/src/module_extension_executor.rs` — pass refs
  through to executor
- `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`
  — pre-materialize referenced repos, drop eager loop
- `app/kuro_interpreter_for_build/src/ast_scan.rs` (new) — AST visitor
  for extracting Label references
- Tests: a small extension fixture that references another extension's
  repo via Label; verify referenced repo materializes but unreferenced
  peers don't

## Success Criteria

### Automated

- `cargo check -p kuro_interpreter_for_build` clean
- `cargo check -p kuro_bzlmod` clean
- `cargo test -p kuro_bzlmod --lib` — existing tests pass
- New test: extension A generates repos R1, R2, R3. Extension B
  references `Label("@R2")` only via `ctx.path`. After B executes,
  only R2 is materialized; R1 and R3 are not.

### Manual

- zeromatter: `kuro build //sdk:sdk` still works (crate extension
  finds `rs_rust_host_tools` and `toml2json` via pre-scan)
- LLVM cquery: count of repos in `bazel-external/` drops from 214 to
  expected ~30-50 (only cc toolchain + rules_cc + bazel_tools + direct
  bazel_deps)

## Risks

1. **Label-ref scanner misses dynamic constructions**: A Starlark
   extension that builds a label from variable strings
   (`Label("@" + repo_name + "//path")`) wouldn't match a static
   pattern. Mitigation: keep a fallback that, on
   `module_ctx.path(Label)` failure (path doesn't exist), falls back to
   triggering materialization via `get_file_ops_delegate`. This means
   synchronous block-on from Starlark, which is the risk in (2).

2. **Synchronous block-on from Starlark deadlocks tokio**: If the
   fallback triggers `tokio::task::block_in_place(|| handle.block_on(...))`
   while holding DICE context mid-computation, we may deadlock the
   runtime. Mitigation: use Bazel's actual approach instead — defer the
   `ctx.execute([Label,...])` call by splitting eval into phases. Far
   more invasive; if needed, abandon 14c.3 and live with the eager
   fallback for dynamic labels.

3. **Cross-extension Label refs that go through aliases**: Real
   extensions may reference via module alias
   (`use_repo(pip, "numpy" = "py_numpy")`), requiring apparent-name →
   canonical-name resolution in the scanner. Mitigation: use
   `ExtensionCellDefinitions.aliases` to translate. Load from
   `pending_repo_cells.rs`.

4. **Extensions that use `load()` to pull another repo's .bzl at
   extension-eval time**: a load() inside a module_extension() body can
   trigger arbitrary cells. This is different from `ctx.path` and not
   covered by the scanner. Mitigation: the extension-execution path
   already handles this via DICE (get_loaded_module); it's not
   eager-spoke-loop related.

5. **crate extension regression**: If the scanner misses a real label
   reference that crate makes, `ctx.execute([Label(@rs_rust_host_tools)])`
   fails because the target repo isn't on disk. Mitigation: extensive
   testing against zeromatter's crate extension. Keep the eager loop
   behind a feature flag during rollout; toggle on per-extension.

## Not Addressed

- Within-extension spoke parallelism (Phase 7 fix, stays as is even if
  eager loop is removed): the *set* of pre-materialized repos is smaller,
  but those that do materialize still run in parallel.
- Lockfile-cache hits don't re-run the scanner (results persist). If the
  scanner logic changes, lockfile entries become stale. Add a scanner
  version to the lockfile entry.

## Est. Effort

- Phase 14c.1 (pre-scan): 2 days — requires AST visitor, thorough
  testing against rules_rs crate extension
- Phase 14c.2 (drop eager loop): 1 day — direct change, relies on 14c.1
- Phase 14c.3 (verify lazy path): 0.5 day — mostly review + testing
- Total: ~1 week

Risk/reward is highest of the three 14-plan options. Correctness
problems are possible if the scanner misses cases.
