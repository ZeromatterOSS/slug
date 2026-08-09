# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-observable-candidate-oracle-generation`
Milestone: M3 query / Stage 4 loading evidence
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: generate and independently replay the accepted 18-lane Bazel 9.2
ordinary-`attr()` observable-candidate fixture.

## Background and boundary

The accepted design extends `query-labels-attribute-metadata` from 39 to 57
Bazel rows. Each new command unions uniquely labeled positive/negative atomic
`attr()` clauses, anchors the whole candidate value, and expects every positive
exactly once with no negative. The 18 lanes cover the accepted RuleClass
ledger's renderers, containers, nulls, ordering, dictionaries, selector
correlation/cross-product, package/macro/test/Starlark defaults, canonical label
leaves, and nine native class additions/removals. Candidate position and equal-
candidate multiplicity are intentionally unobservable and excluded.

This is Bazel-only loading evidence. It does not activate Slug `attr()`, native
toolchain graph projection, a new generic-external graph path, configured
analysis, or any production representation. JVM/Java integration and
production Bazel delegation remain permanently excluded.

## Required generation

- Extend the existing fixture with exactly 18 ordinary-query commands using
  the lane matrix frozen in the Stage 4 owner plan. Preserve distinct positive
  and negative target labels per atomic clause so union deduplication cannot
  rescue an incorrect match.
- Add only `attr/BUILD.bazel`, `modules/ext/MODULE.bazel`, and
  `modules/ext/leaf/BUILD.bazel` to the canonical virtual workspace. Edit only
  its root `MODULE.bazel` for the local module dependency/override and existing
  `pkg/defs.bzl` for the admitted Starlark probes.
- Freeze the actual Bazel 9.2 canonical generic-external label produced by the
  update run. Stop rather than weaken an anchored regex when it differs from
  the pinned-source expectation. Keep `@@bazel_tools` fixed to the admitted
  upstream label; do not copy or invent repository content.
- Expand fixture provenance to the pinned `AttrFunction`, regex filter,
  `TargetUtils`, candidate/default mapper, Starlark base definitions, and nine
  native RuleDefinitions at `8220c6198837d5c13d53fea211cf3282aa12408a`.
- Replay all 57 rows in a clean distinct output root. The first 39 decoded
  argv, exit, normalized stdout, and normalized stderr records must remain
  semantically identical; invocation IDs and absolute run roots are ignored.

## Files

Change only:

- `tests/v2_oracle/fixtures/query-labels-attribute-metadata/fixture.toml`;
- its `expected/oracle.json`;
- `tests/v2_fixture_payload/fixtures.payload` for the five named virtual-source
  additions/edits;
- `tests/v2_oracle/test_v2_oracle.py` for derived global/projection payload
  counts and hashes; and
- `tests/v2_fixture_support/src/lib.rs` for the same derived integrity
  constants and entry-count assertion.

The Python/Rust changes are mechanical test-integrity data, not production
semantics. Do not change runner, BUILD, CLI, server, query/loading production,
Cargo/lockfile, plans, graph, DICE, or generated `@bazel_tools` content.

## Validation

Use ordinary Bazel RC discovery so the private home RC supplies authentication;
never inspect, print, copy, or commit it. Perform one update run and one clean
distinct-root verification. Run the frozen Python payload inventory/projection
tests, Rust payload conformance consumers, the protected 29-row Slug CLI and two
generated-kind CLI/server regressions, and `git diff --check`; serialize shared
Cargo-state commands. Obtain independent oracle-evidence review before
acceptance.

## Stops

Stop and `REPLAN` on more than 18 new commands, more than three new virtual
regular files, any new link/mutation/fixture, more than 1,000 logical fixture/
TOML/expected lines, protected-row semantic drift, an unfrozen/weakened
canonical-label regex, a need for configured analysis or an unbounded registry,
or any production/Rust-semantic, graph, DICE, regex-engine, JVM/Java artifact,
or Bazel-delegation change. Native-toolchain graph projection and any new
generic-external production consumption remain separately reviewed
prerequisites.
