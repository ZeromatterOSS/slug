# Plan 39: Preserve `.git` from `git_repository` for `git worktree` consumers

## Status: PARTIAL (2026-05-05)

Phases 1, 1.5, 1.75 implemented and verified. `git worktree add` runs
successfully; zeromatter advances to a downstream stub (`bazel_lib`'s
`relative_file` returns paths missing the leading `/`, producing
broken `ln -sf var/mnt/...` symlinks). Phase 2 (Starlark override of
native built-in `git_repository`) is still open. The relative_file
issue belongs in a follow-up plan (40).

## Context

After Plan 38 unblocked spoke registration, zeromatter's `//sdk:sdk_contents`
advances to the analysis of git-sourced crates and fails:

```
* bazel-external/rules_rs+override/rs/private/crate_git_repository.bzl:83
ctx.patch(patchfile, strip)
error: fail: fatal: not a git repository: '.git'
```

Several zeromatter crates come from `git+...` Cargo sources (e.g.
`crates__diplomat-tool-0.15.0`, `crates__diplomat-0.15.0`,
`crates__ts-rs-12.0.1`). rules_rs handles them by:

1. Creating one `git_repository` per unique `git+...` URL (a master
   clone shared across all crates from that source).
2. For each crate, declaring a `crate_git_repository` that runs
   `git --git-dir=<master_clone>/.git worktree add ...` to project a
   subset of the master clone into the crate's spoke dir.

rules_rs ships its own `rs/private/git_repository.bzl` that comments:

> Fork upstream git_repository to not delete the .git; we need that to
> derive worktrees.

## Root cause

Two layered issues:

### 1. Native `execute_git_repository` deletes `.git`

`app/kuro_bzlmod/src/repository_executor.rs:982-986`:

```rust
let git_dir = working_dir.join(".git");
if git_dir.exists() {
    std::fs::remove_dir_all(&git_dir).ok();
}
```

This kills the `.git` directory immediately after cloning, leaving a
plain working tree. Anything downstream that needs to `git worktree
add` from the clone fails because the master is no longer a git repo.

### 2. `BUILTIN_REPO_RULES` shadows user-defined Starlark rules

`app/kuro_bzlmod/src/starlark_repo_rule_executor.rs:43-50` lists
`"git_repository"` and `"new_git_repository"` as built-ins. Any rule
invocation with that name dispatches to the native code in
`repository_executor.rs::execute_git_repository`, **even when the user
has loaded a Starlark `git_repository` from a different .bzl file**.

rules_rs's custom `git_repository` (defined as a `repository_rule(impl =
_git_repository_impl)` in `rules_rs+override/rs/private/git_repository.bzl`)
is never executed; kuro silently swaps in the native impl.

The `_external_repo_for_git_source` repos that Cargo crates depend on
are therefore created by kuro's native code with `.git` stripped, even
though the project's effective rule says to keep it.

## Proposed fix

Multi-phase change. Phase 1 unblocks the immediate `.git` strip bug;
Phase 1.5 and Phase 1.75 close two label/materialization gaps that
zeromatter's flow stumbles into next; Phase 2 is the correctness fix that
prevents this class of bug from re-emerging.

### Phase 1 — Stop deleting `.git` in `execute_git_repository`

Remove the unconditional `.git` removal block in
`repository_executor.rs:982-986`.

Justification:
- Bazel's `@bazel_tools//tools/build_defs/repo:git.bzl` does *not*
  unconditionally remove `.git`; the `git_worker.bzl` it delegates to
  leaves the clone intact. (rules_rs's "fork to keep .git" comment
  refers to a much older Bazel behavior; modern Bazel keeps it.)
- Downstream rules (worktree-based fan-out, in-tree git ops, vendor
  stamping) all need `.git`.
- The cost is disk space — bounded by the number of distinct git
  sources, not by the number of crates per source.

Open question: do any Bazel projects rely on `.git` being gone (e.g.,
hashing the working tree without it)? Spot-check rules_jvm_external,
rules_python, rules_oci before landing.

### Phase 1.5 — `resolve_label_to_path` handles `@repo` (no `//`)

`crate_git_repository.bzl` passes a label of the form
`@https___github.com_..._6dc...` — no `//pkg:target`, just `@<repo>`.
Per Bazel convention this is shorthand for `@<repo>//:<repo>`.

`resolve_label_to_path` (in `repository_ctx.rs`) currently parses
label strings as:

```rust
let (repo, rest) = if let Some(idx) = stripped.find("//") {
    (&stripped[..idx], &stripped[idx + 2..])
} else {
    ("", stripped)              // BUG: treats @foo as //foo
};
```

When `//` is absent, the parser misclassifies `@foo` as a root-cell
label `//foo`, returning the literal string instead of resolving
through the cell map. Fix: when `//` is absent, treat the input as
`@<stripped>//:<stripped>`.

`rctx.path` (line 1356) also has its own `@`-detection that requires
`//` to be present; mirror the fix there too.

### Phase 1.75 — `rctx.path(Label)` triggers lazy materialization

Plan 36 made `mctx.path(Label)` trigger lazy spoke materialization.
`rctx.path(Label)` does not. The current implementation
(`repository_ctx.rs:1349-1373`) only constructs a `RepositoryPath`
object — it does not arrange for the labeled repo to be on disk
before downstream commands try to read it.

In zeromatter's flow, `crate_git_repository_implementation` does:

```python
repo_dir = rctx.path(rctx.attr.git_repo_label).dirname
... rctx.execute(["git", "--git-dir=" + str(repo_dir.get_child(".git")), ...])
```

Even with Phase 1's `.git` preserved, this fails because the master
clone repo (`rules_rs+crate+https___...`) has never been materialized.
`rctx.path()` builds a path object pointing at a directory that
doesn't exist on disk.

Fix: when `rctx.path` receives a `Label` (or `@`-prefixed string),
synchronously materialize the referenced external repo before
returning the path. Reuse `kuro_bzlmod::materialize_spoke_sync` (Plan
36's sync bridge from extension-time Starlark to async DICE
materialization) — extending it as needed to handle non-spoke
extension repos.

### Phase 2 — Don't override Starlark `git_repository` definitions

`is_builtin_repo_rule` should check whether the rule is being invoked
from a non-Bazel-tools origin. If the rule's defining `.bzl` lives in a
non-`@bazel_tools` cell, prefer the Starlark execution path.

Implementation sketch:
- `RepositoryInvocation` already carries `rule_source` (the bzl path).
  Inspect it in `execute_repository_rule`: if it points outside the
  bazel-tools-equivalent cells, route to the Starlark executor instead
  of the native dispatcher.
- The native built-ins remain as a fallback for the
  `@bazel_tools//tools/build_defs/repo:git.bzl%git_repository` and
  similar canonical paths — they're still useful when no user override
  exists.

This dovetails with Plan 28's broader builtin-vs-Starlark resolution
strategy. Land Phase 1 now to unblock zeromatter; revisit Phase 2 with the
broader cleanup.

## Verification

- `cargo test -p kuro_bzlmod --lib`
- After Phase 1: re-run zeromatter `//sdk:sdk_contents`. Expect git-sourced
  crate spokes to materialize via `git worktree add`. The next blocker
  will likely be either:
  - `rctx.path(@external_repo)` resolution behavior (the
    `git_repo_label` indirection in `crate_git_repository.bzl:66`), or
  - `ctx.execute(["git", "worktree", "add", ...])` permissions /
    working-dir semantics.
- Confirm `examples/multi_package` still builds (no regression for
  http_archive or local_repository paths).

## Files (anticipated)

- `app/kuro_bzlmod/src/repository_executor.rs` — drop `.git` removal
  block
- `app/kuro_bzlmod/src/starlark_repo_rule_executor.rs` — Phase 2
  builtin-override logic
- (Possibly) `app/kuro_bzlmod/src/repository_invocations.rs` — surface
  rule_source in dispatch decision
