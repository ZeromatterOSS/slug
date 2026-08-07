# Current Slug V2 Packet

Packet: `WP-10-m8-actiond-release-local-reapi-build-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one verified-release, authentication-free actiond build of Slug.

## Goal and required design

From a clean scheduling commit retaining accepted code ancestor `7f58f3bc…`,
create one private 0700 top root and require clean Slug/actiond checkouts and
unbound port 8980. Download only these immutable GitHub release assets:

- `https://github.com/hermeticbuild/actiond/releases/download/v0.0.6/SHA256.txt`:
  479 bytes, SHA-256
  `639b31e99c2d9236b43e18ab03f6368625c346cab364386f8487ab6dea3a649a`;
- `https://github.com/hermeticbuild/actiond/releases/download/v0.0.6/linux-actiond_linux_x86_64`:
  15,905,480 bytes, SHA-256
  `006dc798d4363596fe8ab997606fc93766a0cc427c2d005cf4fc1765fa4c2052`.

Verify both exact sizes/digests and require the manifest's exact binary
digest/name row before changing the binary to mode 0500. The sibling tag must
remain commit `4bdf3e8899ead4eafad54943a18063e6ff0a2637`. Do not use a
latest redirect, mirror, source retry, alternate release, commit switch, or
fallback.

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

Return `REPLAN` on download/verification/startup/analysis/execution/output/
evidence or cleanup failure; do not retry. Always shut down the private Slug
Bazel server, TERM then KILL/reap the worker group if needed, verify port
closure, make only the exact private root owner-writable if deletion requires
it, delete the root/logs, and recheck both repositories.
Record the exact clean Slug run HEAD and actiond commit. Do not change
code/config/locks/targets, use home RC or BuildBuddy, persist
actiond state, or claim Stage 7/backend/BuildBuddy-cache acceptance. On success
only owner/canonical/current docs may record the fixed summary, at most 120
changed lines.
