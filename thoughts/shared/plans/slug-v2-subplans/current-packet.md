# Current Slug V2 Packet

Packet: `WP-6-m2-process-host-native-capture-source-boundary-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs/source-evidence-only native process-host capture boundary.

## Goal

Pin the exact bounded Rust equivalence contract for Bazel/HotSpot native
process-host capture before any implementation.

## Required design record

Establish property precedence, mutation, and lossless platform strings; OS/CPU/
path-policy class initialization and failures; physical-memory,
available-processor, container/cgroup semantics; and RAM-before-CPU,
post-completion timing on every supported platform. Decide exact native source
APIs and error/latching mappings.

This is source evidence only. It authorizes no Rust, Cargo, fixtures, probes,
artifacts, DICE, driver, bridge, or configured-target work. Stop with REPLAN or
Unsupported on any unprovable boundary or need for JVM/delegation; user approval
for that architecture is absent. The user-approved configured-target-cycle
deferral remains unchanged.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Pin source anchors and complete platform/lifetime/error evidence. Run archive,
scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Do not edit Rust or Cargo, create probes/artifacts, add capture, driver, bridge,
DICE, command/configured-target behavior, fixtures, or generated output. Stop
and REPLAN or Unsupported on an unprovable native/JVM boundary.

## Diff budget

- Documentation: at most 180 net lines.
- Total: at most 180 net lines; no Rust, Cargo, fixture, generated, baseline,
  or unrelated changes.
