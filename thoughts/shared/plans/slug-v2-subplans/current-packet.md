# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-canonical-fixture-payload-migration-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted sole-canonical-payload migration design—or terminal
REPLAN—for the remaining nested-fixture test boundary.

## Goal

Determine whether replacing the 14 source workspace trees with one canonical
byte-exact payload can preserve every oracle, Cargo, and Bazel consumer while
removing both nested-package ownership and duplicate-snapshot drift.

## Required design

Inventory every consumer of the 14 `fixture/workspace` paths, including Python
fixture discovery/copy/template expansion/mutations, fixture validators and
provenance, CLI/query/server Cargo tests, plan/archive checks, and fresh Bazel
9 generation/replay. Freeze a deterministic human-auditable canonical format
for 112 directories and 163 files, exact modes/paths/arbitrary bytes and empty
directories, no-follow generation/extraction, initial provenance hashes, and a
reversible migration that deletes no source until all consumers use the same
payload. Preserve each queried workspace graph byte-for-byte. Define shared
Cargo/Bazel extraction, compile-time declared-input embedding, Python harness
materialization, native-Windows and remote behavior, atomic packet splits, and
measured caps. Do not implement or delete anything.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Record exact source/consumer/mutation/provenance inventories; format and
generator authority; byte/path/mode/symlink lifecycle; oracle regeneration and
distinct-root replay set; Cargo and Bazel equivalence; archive/prune accounting;
remote and native-Windows analysis; packet split and line arithmetic. Obtain
independent oracle-fixture, Cargo/Bazel, destructive-migration, and platform
review. Run structure, scope, cap, credential-pattern, archive, and
`git diff --check` gates.

## Stop conditions

Stop with REPLAN on any remaining duplicate source, unreviewable or
nondeterministic payload, followed link, byte/path/mode loss, optional drift
owner, changed queried graph/output, mutation/template mismatch, lost fixture
provenance, non-reversible deletion, runtime runfiles, Windows exclusion,
undeclared remote input, ambient tool/path, Cargo execution from Bazel,
package-local export, partial consumer migration, or coupling to core host
tools, query/cquery/aquery expansion, execution/cache semantics, self-hosting,
Java/JVM delegation, Bazel 8, WORKSPACE, rc, CI, or credentials.

## Diff budget

- At most 460 net documentation lines. No Rust, Python, BUILD, Cargo, lock,
  fixture, payload/archive, generated source, generator/tool, CI, deletion, or
  unrelated change.
