# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-full-rbe-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transported manifest-aware BuildBuddy managed-RBE record.

## Goal and required evidence

From the clean scheduling commit containing accepted `d7faa2f7…`, invoke
`python3 tools/v2_oracle/buildbuddy_rbe_gate.py` exactly once through anonymous
private stdout/stderr transport with inherited environment and retained-session
polling. Cap stdout at 8 KiB, require empty stderr and exact canonical normalized
JSON, and emit only fixed `DELIVERED|REJECTED`, child status, and public record.

## Stops and budget

Accept only outer/child zero, `DELIVERED`, and `PROVED_RBE`. Require process,
BuildFinished, production completion, and executable output one; completion,
pass, and exact-run counts 43; remotely cached tests and persistent hits zero.
Every SpawnExec must have a valid digest, exact remotable true, cache-hit false,
empty status, exit zero, and runner remote: `remote_execution == count > 0` and
every remote-cache/local/worker/sandbox/other/error count zero.

Before and after require clean Git, zero `slugd`, zero roots prefixed
`slug-buildbuddy-full-rbe-`, and zero processes with an argument beginning
`--output_base=/tmp/slug-buildbuddy-full-rbe-`. Poll only the returned session
and do not retry. Any transport/schema/RBE/target/lifecycle failure is `REPLAN`.
Do not inspect raw output, private artifacts, home RC, effective
options, BuildBuddy UI/service, targets, or credentials; do not edit code/config.
