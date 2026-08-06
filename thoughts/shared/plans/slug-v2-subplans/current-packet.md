# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-nested-fixture-ownership-redesign`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted hermetic, incrementally tracked, cross-platform ownership
design—or terminal REPLAN—for the remaining nested-fixture test boundary.

## Goal

Select the smallest source-exact Bazel fixture owner that can serve the 42 CLI
integration cases, 53 loading-query cases, and inseparable 34-case server unit
target without altering the workspaces those tests query.

## Required design

Reconcile every live fixture consumer, mutation, compile-time manifest/binary
path, and expected workspace byte with the 14-workspace/163-file/105-nested-
package Gate C0 inventory and the query/server subsets. Re-evaluate only
mechanisms that can declare all bytes for local sandbox, remote execution, and
incremental invalidation while preserving Windows manifest-only runfiles and
Cargo behavior. If a checked-in immutable snapshot/archive is the sole bounded
mechanism, freeze a deterministic source-exact generator, manifest/hash and
drift check, extraction/scratch lifecycle, platform path contract, and prove
why the duplicate cannot diverge silently or change queried BUILD semantics.
Otherwise freeze the exact supported Bazel owner and consumer API. Partition
CLI binary env, fixture ownership, query loading, and server crate-mode
activation; do not bundle unrelated semantic repairs.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Record exact consumer/file/workspace/mutation inventories, the Bazel 9.2 and
rules_rust ownership/runfiles authority, remote/invalidation and native-Windows
analysis, deterministic byte/path lifecycle, proposed packet split and line
arithmetic, and independent fixture/remote/platform design review. Run
structure, scope, cap, credential-pattern, and `git diff --check` gates; no
fixture, archive, generator, Rust, BUILD, or Cargo implementation is authorized
in this packet.

## Stop conditions

Stop with REPLAN on undeclared recursive source-directory reads, package-local
exports or targets that change queried fixture graphs, ambient repository/home
paths, untracked directory symlinks, source/runfile writes, a snapshot without
deterministic source-to-byte drift enforcement, Cargo execution from Bazel,
Windows exclusion or assumed runfiles tree, nondeterministic metadata, remote
or incremental invalidation gaps, fixture query-output changes, or coupling to
core host tools, query/cquery/aquery expansion, execution/cache semantics,
self-hosting, Java/JVM delegation, Bazel 8, WORKSPACE, rc, or credentials.

## Diff budget

- At most 420 net documentation lines. No Rust, BUILD, Cargo, lock, fixture,
  archive, generated source, generator/tool, CI, or unrelated change.
