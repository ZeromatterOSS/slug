# Current Slug V2 Packet

Packet: `WP-10-m8-actiond-local-reapi-build-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one authentication-free remote-only actiond build of the Slug binary.

## Goal and required design

From a clean scheduling commit that retains accepted code ancestor
`7f58f3bc…`, create one private 0700 top root and require port 8980 unbound.
Build clean sibling actiond commit `8a42c3d4…` locally at `-c opt` from source
in a private output base with private `--symlink_prefix=/`, system/home RC
disabled, no remote config, and exact empty `--bes_backend=`,
`--bes_results_url=`, executor, cache, and disk-cache overrides. Use that same
output base and the same empty overrides for `cquery --output=files` plus
`info execution_root`; require one canonical executable beneath the private
output base. Do not download a binary, switch commits, or fall back.
Start its VM worker in a process group with private state, loopback port 8980,
8192 MiB CAS, 4096 MiB memory, four CPUs, and 180-second startup timeout;
require its exact bridge-listening event and a live PID.

Run one fresh-output-base Bazel build of `//app/slug_cli_v2:slug` with no
system/home RC, explicit nightly, actiond loopback executor/cache, empty remote
instance/BES/results/disk/downloader, remote-only spawn/genrule and no fallback,
no accepted cached actions/local-result upload/cache compression, top-level
downloads, 900-second remote timeout, four jobs, and exact action properties
`libc=glibc2.39` and `requires-bash=`. Capture BEP and execution JSON privately.
Accept only exit/BEP/target success, executable materialization, at least one
SpawnExec, and every spawn runner exactly `remote`, uncached, empty-status, and
exit zero.

## Stops and budget

Return `REPLAN` on source-build/startup/analysis/execution/output/evidence or
cleanup failure; do not retry or switch to a release/commit. Always shut down
both private Bazel servers, TERM then KILL/reap the worker group if needed,
verify port closure, delete private roots/logs, and recheck both repositories.
Record the exact clean Slug run HEAD and actiond commit. Do not change
code/config/locks/targets, use home RC or BuildBuddy, persist
actiond state, or claim Stage 7/backend/BuildBuddy-cache acceptance. On success
only owner/canonical/current docs may record the fixed summary, at most 120
changed lines.
