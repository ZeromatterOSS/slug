# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-developer-graph-boundary-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a bounded implementation design for the first Bazel 9 Rust target
closure, without starting self-hosting or changing repository metadata.

## Goal

Inspect the live Cargo workspace and pinned Bazel 9.2/rules_rust requirements.
Freeze the smallest `slug_cli_v2` transitive developer graph: package/target
ownership, focused test mapping, toolchain and dependency pins, Cargo/Bazel
synchronization, and generated/build-script/proc-macro treatment. Select an
exact implementation allowlist and validation boundary.

## Required design record

Pin Bazel at `9.2.0`/`8220c619` and select a Bazel-9-compatible rules_rust
boundary without writing it. Keep Cargo supported and define a reviewed lock/
dependency synchronization policy. Record only a credential-safe BuildBuddy
boundary; do not open either repository or home `.bazelrc`, and claim no remote
cache/RBE evidence. The developer graph is independent of M2 configuration;
self-hosting remains gated on M5/M6.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/10-bazel-build-and-bootstrap.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record a complete live Cargo workspace/`slug_cli_v2` closure inventory, pinned
primary source for Bazel/rules_rust choices, target/test ownership, dependency
synchronization, and the credential-safe local/BuildBuddy split. Run source,
archive, scope, cap, no-Cargo, and `git diff --check` gates only.

## Stop conditions

Stop with REPLAN on credential or user-rc inspection, an unbounded Bazel-9/
rules_rust mapping, unresolved generated/proc-macro/dependency-sync ownership
beyond one CLI closure, or coupling to M2/M5/M6 semantics. Do not edit or add
MODULE/BUILD/.bazelversion/.bazelrc files, Rust, Cargo/lockfiles, dependencies,
fixtures, CI, or generated data; do not run Bazel/Cargo, BuildBuddy/RBE, query,
cquery, aquery, execution, materialization, or self-hosting.

## Diff budget

- Documentation and total: at most 220 net lines. No code, metadata, fixture,
  generated, dependency, lockfile, or unrelated changes.
