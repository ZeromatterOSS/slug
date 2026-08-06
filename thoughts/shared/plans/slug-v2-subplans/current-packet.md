# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-nested-fixture-snapshot-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an accepted deterministic immutable-snapshot ownership design—or
terminal REPLAN—for the remaining nested-fixture test boundary.

## Goal

Determine whether one checked-in source-exact payload can serve the 42 CLI
integration cases, 53 loading-query cases, and inseparable 34-case server unit
target after Bazel 9.2's repository watching proved unable to retain no-follow
directory identity.

## Required design

Freeze the smallest cross-platform no-follow generator for exactly 14
workspaces, 112 directories, and 163 regular files. Specify a versioned ordered
directory/file format, normalized modes/metadata, per-entry and whole-payload
hashes, atomic generation, and a source-to-payload drift check that cannot be
silently skipped in the supported Cargo and Bazel developer gates. Preserve
empty directories, arbitrary bytes, the non-ASCII discriminator, ASCII
Windows-safe paths, and exact BUILD/BUILD.bazel workspace graphs. Freeze a
create-new `TEST_TMPDIR` extraction API and compile-time rules_rust
`compile_data` embedding so runtime runfiles remain unnecessary. Partition the
owner, loading-query, server, and CLI binary/test activations and size every
packet. Do not implement the snapshot or helper.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Record the exact generator authority and source enumeration, no-follow and
path-collision behavior, deterministic byte/hash lifecycle, drift-enforcement
entry points, local sandbox and remote declared-input semantics, native-Windows
analysis, Cargo fallback, packet split, and line arithmetic. Obtain independent
fixture/generator/drift/remote/platform review. Run structure, scope, cap,
credential-pattern, archive, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on a drift check that is optional or Bazel-invisible in the
supported developer gate, ambient interpreters/tools/repository paths, followed
links, untracked directory reads, nondeterministic timestamps/owners/modes,
source/runfile writes, path or byte loss, Windows exclusion, runtime runfiles,
Cargo execution from Bazel, package-local fixture exports, queried graph
changes, application activation, CI assumptions, or coupling to core host
tools, query/cquery/aquery expansion, execution/cache semantics, self-hosting,
Java/JVM delegation, Bazel 8, WORKSPACE, rc, or credentials.

## Diff budget

- At most 360 net documentation lines. No Rust, BUILD, Cargo, lock, fixture,
  payload/archive, generated source, generator/tool, CI, or unrelated change.
