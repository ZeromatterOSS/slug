# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-full-cache-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transported manifest-aware BuildBuddy cache prime/replay record.

## Goal and required evidence

From the clean scheduling commit containing accepted `b1b64f41…`, invoke
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once through anonymous
private stdout/stderr transport with inherited environment and retained-session
polling. Cap stdout at 8 KiB, require empty stderr and exact canonical normalized
JSON, and emit only fixed `DELIVERED|REJECTED`, child status, and public record.

## Stops and budget

Accept only outer/child zero, `DELIVERED`, and `PROVED_CACHE_ONLY`. Both phases
must report process/BuildFinished/build/output one and completion/pass/run 43;
prime/replay cached counts 0/43 and persistent hits zero. Eligible action counts
must be equal/nonzero with equal digest hash: prime only local/worker/sandbox
misses, replay only remote-cache hits, and every error/other runner zero.

Before and after require clean Git, zero `slugd`, zero roots prefixed
`slug-buildbuddy-cache-`, and zero processes with an argument beginning
`--output_base=/tmp/slug-buildbuddy-cache-`. Poll only the returned session and
do not retry. Any transport/schema/cache/target/lifecycle failure is `REPLAN` and
stops before RBE. Do not inspect raw output, private artifacts, home RC, effective
options, BuildBuddy UI/service, targets, or credentials; do not edit code/config.
