# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-unknown-command-diagnostic-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a frozen secret-safe expansion for the unknown command failure.

## Goal and required design

Audit pinned Bazel 9.2 `FailureDetail` exit-2 category/code pairs and the
current parser. Design the smallest closed classification expansion that can
distinguish the live `UNKNOWN_COMMAND_LINE_ERROR` without exposing raw
messages, category/code/enum names, options, paths, credentials, nonces, or
stderr. Freeze exact allowlists, malformed/general-field behavior, tests,
implementation files/caps, and the boundary for one separately reviewed live
diagnostic packet.

## Stops and budget

Change only the owner plan, canonical plan, and this manifest, at most 120
changed lines total. Do not edit code/config/CI/BUILD/MODULE/locks, run Bazel,
discover or inspect home configuration, read raw artifacts, contact
BuildBuddy, invoke RBE, or make another live attempt. Return `REPLAN` if no
bounded fixed classification can add diagnostic information without widening
the secret boundary.
