# Current Slug V2 Work Packet

Packet: WP-7-17-m7a-builtin-catalog-bazel-inputs-r1
Status: accepted; exact embedded catalog inputs and direct adapter Bazel build pass

## Outcome and compatibility

Make all 49 existing verbatim Bazel 9.2 `@bazel_tools` catalog files declared
inputs to `//app/slug_bzlmod_v2:slug_bzlmod_v2`'s Rustc action, then rebuild
the directly blocked `//app/slug_reapi_v2:slug_reapi_v2` target. This repairs
Bazel packaging only. Runtime source bytes, catalog paths/modes/digests,
Slug DICE ownership and compatibility classification remain unchanged; adapter
compilation cannot by itself prove CLI buildability or M7A readiness.

WP-7-16 at `869dfa4e5` built the new cache-leaf library but stopped in the
`slug_bzlmod_v2` Rustc action: its parent `compile_data = glob(["builtin/**"])`
sees only `MODULE.bazel`. All other 48 embedded files exist below nine pinned
upstream BUILD package boundaries. Bazel cannot load one of those verbatim
upstream BUILD files as a package in this checkout because its labels are
relative to the upstream repository.

## Packaging decision and invariants

Keep the 49 pinned files byte-for-byte and mode-for-mode unchanged. Add one
local `BUILD.bazel` sidecar beside each of the nine upstream `BUILD` files.
The sidecars select Bazel package metadata in this checkout without changing
the `BUILD` bytes that Slug embeds. A shared parent-package macro creates one
filegroup per sidecar, containing that package's pinned files (including its
upstream `BUILD`) and excluding the sidecar itself. A parent aggregate
filegroup combines those nine groups with the root catalog `MODULE.bazel`;
both `rust_library.compile_data` and the catalog test's `data` use it. No
undeclared host path, source copy, symlink, generated substitute, or runtime
catalog extension is admitted.

Update the checked-in catalog test to assert exactly the 49 pinned source
paths plus the nine named sidecars in the checkout. Its existing per-file
digest/mode checks continue to validate the pinned bytes, and the Bazel
compile proves the inputs enter Rustc's sandbox. Sidecars are build metadata,
never `BuiltinBazelToolsSnapshot` entries or materialized `@bazel_tools`
content. No DICE key, action semantics or source acquisition path changes.

## Scope and validation

Allowlist: `app/slug_bzlmod_v2/BUILD.bazel`, a new parent
`catalog_bazel_inputs.bzl`, `tests/builtin_bazel_tools.rs`, nine new
`builtin/bazel_tools/**/BUILD.bazel` sidecars adjacent to existing upstream
BUILD files, this manifest, canonical Live Status, Stage 10's live inventory
delta and bootstrap-readiness's first row. The original 49 catalog files,
Cargo manifests/locks, module locks, and unrelated code are frozen.

First verify the static path inventory and original-file Git diff. Build only
`//app/slug_reapi_v2:slug_reapi_v2` with the pinned nightly toolchain and
`--lockfile_mode=error`, under a 60-second TERM/short KILL cap. This is the
necessary compile continuation after WP-7-16; do not repeat the unchanged
failed graph. If compilation passes, run only the exact catalog-assets test,
with compile preparation separate from its short runtime. Keep test runtime
as small as possible; investigate any run over a few seconds and scrutinize
one over roughly 30 seconds. Record selected/passed count, elapsed time,
output and tracked status. A new compiler or packaging error requires a new
diagnosis, not an identical retry. Run `python3 scripts/v2_plan_status.py` and
`git diff --check`; no broad suite or CLI-root build is selected.

Commit and push only after the original 49-file inventory and focused
compile/test gates pass. If the sidecar technique cannot preserve the exact
catalog assertion and Bazel sandbox inputs, stop and record `REPLAN` for an
alternative source-package boundary.

## Acceptance receipt (2026-09-16)

The original 49 catalog files have no tracked diff; the checkout contains
exactly those files plus the nine named `BUILD.bazel` sidecars. A no-fetch
Bazel query of `deps(//app/slug_bzlmod_v2:builtin_bazel_tools_files)` exited 0
in 1.61 seconds and returned 59 labels: 49 exact catalog file labels, nine
nested filegroups, and the parent aggregate. No sidecar label entered the
filegroup closure. Query output SHA-256 is
`269fadbbf5616816f355c47b837ffc108c682038b8b155ab1589eeae1b4ba961`.
The first query exposed two packages with empty non-BUILD globs; setting
`allow_empty = True` corrected that local Starlark declaration before the
successful query.

The sole post-fix adapter compile used:

```text
ulimit -f 131072
/usr/bin/time -f 'elapsed_seconds=%e\nexit_status=%x' -o target/wp717/build_time.txt timeout -k 3s 60s bazel --ignore_all_rc_files --batch build --lockfile_mode=error --@rules_rust//rust/toolchain/channel=nightly //app/slug_reapi_v2:slug_reapi_v2 > target/wp717/build_stdout.txt 2> target/wp717/build_stderr.txt
```

It exited 0 in 40.67 seconds (seven processes, 1,354 action-cache hits),
producing both `libslug_bzlmod_v2-1540755102.rlib` and
`libslug_reapi_v2-2512936149.rlib`. Stderr SHA-256 is
`bba62c11bd9575d9a43b3ea4320035a8f0ed5b0a8708ace0b4d42ea471c9a5e0`;
stdout is empty. Only this direct adapter compile is proved; the CLI root is
not built.

The changed catalog test executable compiled in 6.11 seconds, and after a
runfiles-path correction recompiled in 6.17 seconds. The initial direct
invocation lacked Bazel runfiles; a subsequent Bazel launcher run exposed the
old test's source-checkout-only path. Neither failure reached a catalog-byte
assertion. Final preflight selected one exact, nonignored test. The final
`bazel test` passed that one test in 0.00-second test runtime (3.88 seconds
including Bazel startup), with three tests filtered out; test-log SHA-256 is
`2e64e99ac60c269bea60d73a073086c5d2298518211c78b8cd0f0d1366c219d6`.
The test runfiles manifest contains the 49 original files and zero sidecars.
Ignored `target/wp717/` retains command streams and timing. No Cargo, module,
lock, pinned catalog byte or runtime semantic owner changed; M7A remains open.
