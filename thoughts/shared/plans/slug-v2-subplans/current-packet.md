# Current Slug V2 Work Packet

Packet: WP-7A-daemon-exact-path-escalated-bind-audit-r1

Status: SELECTED after the ordinary-sandbox direct serve reproduced the bind
failure. Rust is clean at accepted `build.rs`; registry and complete R2 remain
untouched.

## Same-boundary result

The single foreground diagnostic used the exact failed workspace and87-byte
socket path inside the ordinary managed sandbox. It sent no command request,
registry, remote endpoint, network traffic or credential. It exited2 in0.02s,
peak RSS11048KiB, with the exact foreground record
`daemon_serve_error` / `binding daemon socket <exact path>`. No socket was created
and no slug/strace process survived, so cleanup was unnecessary. Aggregate logs
were218 bytes, below64KiB.

Evidence `/tmp/slug-daemon-sandbox-serve.ptko98` has stdout SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`,
stderr `6cbd3d516de1c2237db390fd8aa3bb9de6abb1736948db7736cd520d679179a4`
and time `c65c1f448fbc7c3c4e1ca5469fd0e910d481334fa76b360ed77610a9c431ee10`.

This attributes both registry integration failures to daemon Unix-socket bind
failure in their managed-sandbox environment, not registry propagation. The
current top-level formatting loses the underlying OS error, so it does not yet
prove the sandbox policy is causal rather than the exact path/workspace state.
The prior escalated trace proved the same binary can bind a different short path.

## Exact-path complementary audit

Run one direct foreground `target/debug/slug --serve` outside the managed sandbox,
using the exact same leftover workspace and output-base socket. Enforce a2s wall
deadline plus1s kill grace, capture bounded stdout/stderr/time logs under `/tmp`,
and cap aggregate output at64KiB. Send no test or command request, registry value,
remote endpoint, network traffic or credential. Do not use `strace`; the sole
question is whether exact-path bind/start succeeds when the sandbox boundary is
removed.

Before invocation verify the directories and absent socket. If started, kill/reap
at the deadline and remove only the verified socket. If it errors, retain the
foreground record and do not retry. Check no survivor and source cleanliness.
Any ambiguity, survivor, output overflow, source mutation or time above2s inner
wall is `REPLAN`.

After independent review, a successful complementary bind permits a separately
reviewed registry validation packet to run only the already-compiled named
integration proof outside the sandbox once, still under12/15s and without network
access. It does not authorize reimplementation, broad tests or R2. A failed bind
requires path/error-chain design instead. Preserve complete R2 SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e` and old
probe SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
Never inspect, print, copy or commit `~/.bazelrc` or derived credentials.
