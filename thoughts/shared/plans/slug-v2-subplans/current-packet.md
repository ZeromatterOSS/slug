# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-cli-test-boundary-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an exact, implementation-ready boundary for the CLI library unit test
and its two integration-test binaries over the accepted production graph.

## Goal

Inventory the live `slug_cli_v2` unit and integration tests, their Cargo-only
compile-time environment, binary and fixture runfiles, external/local deps, and
daemon/process cleanup needs. Split them into the smallest honest Bazel test
implementation packets. Do not map tests or edit Rust in this design packet.

## Required design

Reconcile `app/slug_cli_v2/Cargo.toml`, `src/**/*.rs`, `tests/cli.rs`, and
`tests/graph_output.rs` with the accepted `BUILD.bazel`. Freeze target
ownership, exact `CARGO_BIN_EXE_slug` and `CARGO_MANIFEST_DIR` adaptation,
fixture/runfile inventory, environment isolation, serial/daemon-sensitive test
constraints, local and external dependency edges, and validation commands.
Distinguish the small library-unit target from each integration binary and
partition implementation if either integration surface cannot fit one bounded
review packet. Name the later transitive-package test inventory separately.

## Allowed paths

- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Inspect only the live Cargo manifest, accepted Bazel metadata, CLI test source,
and referenced repository fixtures. Prove every compile-time env consumer and
fixture path has one proposed declared Bazel owner. Run documentation, scope,
cap, archive, credential-pattern, and `git diff --check` gates. No Cargo or
Bazel build/test command is needed.

## Stop conditions

Stop with REPLAN if exact test adaptation requires rewriting test semantics,
copying fixtures, using Cargo as a Bazel executor, repository-layout/canonical
external paths, ambient daemon state, rc/credential inspection, or coupling to
M2/M5/M6/self-hosting. Do not add BUILD/source/fixture/lock/Cargo changes, test
targets, WORKSPACE, `.bazelrc`, CI, BuildBuddy/cache/RBE, query, cquery, or
aquery surface.

## Diff budget

- Documentation only: at most 560 net lines. No production, test, BUILD, Cargo,
  lock, fixture, generated-source, CI, or unrelated change.
