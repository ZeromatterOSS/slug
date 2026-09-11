# Current Slug V2 Work Packet

Packet: WP-7A-daemon-start-child-status-design-r1

Status: SELECTED after the bounded launch trace reached bind/listen. Design only;
the run-registry implementation remains restored and complete R2 untouched.

## Accepted attribution boundary

The initial sandbox `strace` preflight failed immediately with `PTRACE_TRACEME`
denied, wrote an empty trace and launched no Slug process. The approved
out-of-sandbox diagnostic used the freshly compiled candidate binary only because
its launch/serve code is byte-for-byte represented by the restored source. It
sent no command request, registry value, remote endpoint, network traffic or
credential.

The direct invocation executed `target/debug/slug --serve`, bound and listened on
the fresh short Unix socket, emitted its normal started record, and remained alive
until the fixed five-second SIGTERM. Inner wall was5.00s, peak RSS15812KiB and the
trace was201854 bytes, below2MiB. The traced process and one runtime descendant
were killed by SIGTERM; no slug/strace survivor remained, and the verified socket
was removed. Evidence is `/tmp/slug-daemon-readiness.oZv7p5`: trace SHA-256
`bd19d68de9569087c07fc62ef3e90818546af69a608d805b78c8ea68f5af7bbf`,
stderr `6fb1701220d16a6250b13ea43d5efaa38a0fd05e8c3ca67008af1394c072c4f9`,
time `ebb1ec0e2ca4ce2d5e5fbbcea4bbb321ef90d6527e61e145e9435c1ed43c7517`.

This proves only that the binary can exec, dispatch, bind and remain live with
the diagnostic socket/workspace. It does not reproduce `start_daemon` ownership,
the exact failed output-base/workspace, or the earlier integration failure, and
does not prove a transient, path-length, resource or scheduling cause. That cause
remains unknown. Do not rerun the failed test to classify it.

## Child-status and cleanup design audit

Audit only `app/slug_cli_v2/src/commands/build.rs`, all five CLI callers of
`start_daemon`, the CLI integration daemon cleanup/helper, and existing daemon
launch/error assertions. Freeze the smallest correction that makes a future
failure attributable without retaining daemon output pipes:

- keep the spawned `Child` mutable during the readiness loop;
- before each connect attempt, call `try_wait` and immediately return
  `daemon exited before becoming ready (status: {status})` if it has exited;
- on `try_wait` error, terminate/reap the owned child and return
  `checking daemon process status while waiting for readiness: {error}`;
- on the existing readiness deadline, terminate and reap the owned child, then
  return the byte-identical existing timeout message;
- on successful connect, drop only the parent handle and preserve current daemon
  ownership, PID file, null stdio and caller behavior.

Do not pipe or inherit daemon stdout/stderr: CLI integration callers use
`Command::output`, so a successful background daemon retaining those pipes could
hold the foreground capture open. Add no log file, async task, dependency, public
API, signal protocol or daemon lifetime owner. Termination first calls `try_wait`;
an observed status is already reaped. Otherwise it attempts `kill` and always
calls `wait`, tolerating an already-exited kill race only when `wait` successfully
reaps the child. The primary observation/timeout result stays the reported error;
cleanup detail may be appended but cannot replace its category.

Do not unlink the PID file or socket on a failure path. Their pathname identity
cannot prove they still belong to this child after a concurrent replacement; the
owned `Child` is the only identity-safe cleanup authority. Existing later launch
and fixture cleanup behavior remains unchanged. A separately designed ownership
protocol would be required to remove path artifacts safely.

Implementation may touch only `app/slug_cli_v2/src/commands/build.rs`, adding at
most35 production/90 proof/125 gross lines. Factor a private readiness helper so
a unit proof can pass a controlled quickly exiting child and a nonexistent short
socket, assert the exact early-status message, and finish below1 second without
using the ten-second deadline. Compile that library-test target once under55
seconds; then run only its precompiled named proof under12/15 seconds absolute and
the affected default CLI check under30 seconds. Freeze hashes after compilation;
no edit, retry or timeout extension afterward.

This packet is read-only. No Rust, test, compile, direct server, strace, replay,
network, credential, fixture or R2 action is authorized. Independently review the
design before implementation. Any need to change the five command callers, server
wire/protocol, daemon main loop, deadline, path-artifact ownership or successful-
launch semantics is `REPLAN`. Independent review ACCEPTS the corrected process-
only cleanup boundary; implementation begins only after this checkpoint is pushed.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
