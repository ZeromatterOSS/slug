# Current Slug V2 Packet

Packet: `WP-10-m8-host-bzl-parse-diagnostic-parity-design`
Milestone: M8 Bazel developer graph prerequisite
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted bounded Host `.bzl` parse-diagnostic parity repair—or
terminal REPLAN—before retrying the atomic fixture payload migration.

## Goal

Design the smallest exact repair by which the production Host Bzl-module route
retains Bazel 9.2's pinned compilation-failure fragment for malformed `.bzl`
files, restoring the final red CLI fixture row without changing successful
loading or other diagnostics.

## Required design

Trace the accepted `broken_bzl_failure` oracle row and CLI assertion through
`HostBzlModuleEvalKey`, its parse error construction, the legacy path which
already adds `compilation of module 'broken_bzl/bad.bzl' failed`, and the final
query diagnostic presentation. Freeze the exact source-relative label/path
bytes, error ownership and chaining, whether the legacy helper can be reused,
and a targeted Host-route regression which distinguishes the missing fragment
without weakening the existing oracle assertion.

Define the exact production/test allowlist, focused loading/query/CLI
validation, downstream diagnostic non-regressions, GNU-Windows shape, and
measured caps. Reuse the existing Bazel 9.2 fixture evidence; add no oracle row.
Do not implement Rust in this design packet.

## Allowed paths

- canonical plan, Stage 4 loading owner, Stage 10 owner, this manifest, and
  August routing history

## Required validation

Record exact live producer/consumer and accepted oracle anchors, error-chain
ownership, unaffected diagnostic classes, target regression shape, downstream
commands, and line arithmetic. Obtain independent diagnostic/parity latest-
text review. Run structure, scope, cap, credential-pattern, archive, and
`git diff --check` gates.

## Stop conditions

Stop with REPLAN on assertion/fixture/expected-output weakening, parser or
Starlark language substitution, path-dependent repository checkout text,
changed success behavior, broad error-wrapper or public diagnostic schema,
new dependency, DICE key/ownership/lock, query semantics, payload/helper/
consumer/deletion work, server fixture work, execution/cache, self-hosting,
Java/JVM delegation, Bazel 8, WORKSPACE, rc, CI, or credentials.

## Diff budget

- At most 240 net documentation lines. No Rust, test, fixture, Python, BUILD,
  Cargo, lock, payload, generated, CI, deletion, or unrelated change.
