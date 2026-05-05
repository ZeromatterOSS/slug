# Plan 40: bazel_lib `relative_file` produces paths missing leading `/`

## Status: COMPLETE (2026-05-05)

Fixed by bumping `KURO_BAZEL_VERSION` from `9.0.0` to `9.0.1`. That
flips `bazel_features.external_deps.repo_rules_relativize_symlinks`
to `True`, routing rules_rs's `relative_symlink` helper to the
`rctx.symlink(target_path, link_path)` branch (which kuro implements
correctly with absolute paths) instead of the
`ln -sf relative_file(...)` fallback (which produces broken
`var/...`-prefixed targets in kuro's stubbed bazel_lib).

The `bazel_features+version_extension+bazel_features_version` repo
caches `native.bazel_version` via `local = True` repository_rule, but
its `.kuro_repo_complete` marker prevents kuro from re-running on
daemon restart. Workaround for this build: deleted the marker so the
repo rule wrote `version = '9.0.1'`. Long-term fix: respect the `local
= True` flag and re-run such rules on every server restart so the
cached output stays in sync with `native.bazel_version`.

## Context

After Plan 39 phases 1/1.5/1.75 unblocked git_repository materialization
and `git worktree add` for crate spokes, zeromatter's
`//sdk:sdk_contents` advances to a new failure: every git-sourced
crate's `crate_git_repository` impl runs through OK until it tries to
read `Cargo.toml`, which doesn't exist in the spoke directory.

Tracing reveals the spoke directory contains broken symlinks pointing
at paths missing a leading `/`:

```
ln -sf var/mnt/dev/zeromatter/bazel-external/<master>/<sub>/Cargo.toml ./Cargo.toml
                                                  ^^ missing leading slash
```

The symlinks are produced by rules_rs's `relative_symlink` helper
(`bazel-external/rules_rs+override/rs/private/symlink_utils.bzl:18-23`):

```python
result = rctx.execute([
    "ln",
    "-sf",
    relative_file(str(target_path), str(link_path)),
    str(link_path),
])
```

`relative_file` is loaded from `@bazel_lib//lib:paths.bzl`. In kuro,
`bazel_lib` is registered as a cell but its `paths.bzl` is either
unmaterialized or stubbed; the function call returns a string with the
absolute target's leading `/` stripped.

The `bazel_features.external_deps.repo_rules_relativize_symlinks`
guard would route to `rctx.symlink` (which kuro implements correctly)
on Bazel 9.0.1+. ZeroMatter currently sees the feature as `False`,
falling into the `ln -sf` branch.

## Investigation needed

Before fixing:

1. Verify `bazel_lib` actually materializes in the build — check
   `bazel-external/bazel_lib+*/lib/paths.bzl` exists and look at the
   real `relative_file` definition.
2. Determine why `relative_file(absolute_target, absolute_link)`
   returns a path missing the leading `/`. Two suspects:
   a. `bazel_lib`'s implementation has an off-by-one bug for absolute
      paths that share a common prefix (both start with `/`, the
      common-prefix logic strips the slash).
   b. Kuro's `rctx.path(string)` doesn't preserve the leading `/` when
      the string is an absolute path.
3. Inspect the exact strings passed to `relative_file` — `str(rctx.path(target))`
   and `str(rctx.path(link_name))` — to see whether the input or the
   computation is at fault.

## Likely fix paths

- **If kuro's `RepositoryPath::Display` strips a leading `/`**: fix
  the Display impl. Spot-check the `RepositoryPath::with_base_dir`
  flow — when path is absolute, .path stays absolute; Display writes
  it verbatim. Looks safe but worth confirming with a unit test.
- **If `bazel_features.external_deps.repo_rules_relativize_symlinks`
  is the wrong default in kuro's `bazel_features` stub**: flip it to
  `True` so zeromatter's flow takes the `rctx.symlink` branch (which
  produces correct absolute symlinks). Easier than reimplementing
  `relative_file` semantics.
- **If `relative_file` in upstream bazel_lib genuinely has the bug
  for absolute paths**: rules_rs would have hit it on Bazel too — so
  this is unlikely.

The shortest path to unblock zeromatter is probably the
`bazel_features` stub flip, with a unit test for symlink correctness.
A more thorough fix is to implement `relative_file` natively in kuro's
bazel_lib bundled cell so rules_rs and other consumers don't depend
on a half-stubbed external repo.

## Verification

- Re-run zeromatter `//sdk:sdk_contents` after the fix; expect
  `Cargo.toml` reads to succeed and the build to advance into the
  next layer of `crate_git_repository` analysis (toml2json, etc.).
