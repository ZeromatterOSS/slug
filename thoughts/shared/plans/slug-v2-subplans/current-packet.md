# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-rbe-vertical-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transported structured one-label managed-RBE record.

## Goal and required evidence

From the clean scheduling commit containing accepted `e48213bb…`, invoke
`python3 tools/v2_oracle/buildbuddy_build_rbe_gate.py` exactly once through the
accepted anonymous outer transport with inherited environment and retained-
session polling. The child therefore uses ordinary Bazel RC discovery, including
the private home authentication RC, without the transport reading or exposing it.

## Stops and budget

The transport uses anonymous private stdout/stderr files, caps stdout at 4 KiB,
requires empty stderr, validates the exact normalized compact schema, and emits
only a fixed `DELIVERED|REJECTED` envelope with child status and public record.
Accept only outer and child exit zero, `DELIVERED`, and `PROVED_BUILD_RBE` with
process/BuildFinished/target/output counts one, nonempty spawns,
`remote_execution == count`, and all cache-hit, persistent-cache, field-error,
local, worker, sandbox, and other counts zero.

Before and after, require clean Git, zero `slugd`, zero roots prefixed
`slug-buildbuddy-rbe-`, and zero processes whose arguments contain
`--output_base=/tmp/slug-buildbuddy-rbe-`. Poll the same returned session; do not
reissue. Any transport/session/schema/lifecycle failure stops at `REPLAN`.
Do not inspect raw output, home RC, private artifacts, BuildBuddy UI/service, or
effective options; do not edit code/config or claim the full 43-target gate.
