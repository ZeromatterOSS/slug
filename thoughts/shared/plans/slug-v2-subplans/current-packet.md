# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-isolated-observable-candidate-oracle-generation`
Milestone: M3 query / Stage 4 loading evidence
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: generate and independently replay the complete isolated Bazel 9.2
ordinary-`attr()` observable-candidate fixture.

## Background and boundary

The reviewed design selects `query-attr-observable-candidates`, a Bazel-only
payload fixture absent from the Rust projection allowlist and every CLI/server
case. It has five virtual source files and five directory records. Exactly 18
ordinary-query rows contain 170 globally unique positive/negative atom pairs;
no positive label is reused within a row. This evidence does not activate Slug
`attr()`, native-toolchain graph projection, or a generic-external production
path. Production remains permanently Rust-native with no JVM/Java integration
or Bazel semantic delegation.

## Required generation

- Add only root `MODULE.bazel`, `attr/defs.bzl`, `attr/BUILD.bazel`, local
  `modules/ext/MODULE.bazel`, and `modules/ext/leaf/BUILD.bazel` to the new
  canonical payload workspace. The root directory is not a Bazel package.
- Implement all exact atoms in the accepted Stage 4 18-lane table using names
  `//attr:lNN_aMMM_yes` and `_no`. Every atom has a distinct pair, anchored
  whole-value regex, and exact stdout containing its positive once and no
  negative. The lane pair counts are
  `13/8/8/4/3/3/3/6/11/12/18/3/23/5/9/17/15/9`, totaling 170.
- Keep direct lane-9 controls direct and instantiate its legacy-macro controls
  only through the named legacy macro. Positively prove generator name,
  function, location, and direct empty fields. Use a real base string setting
  in `//attr` as the identity transition output and positively prove lane 12's
  function-transition allowlist.
- Independently reproduce the exact generic external candidate
  `@@ext+//leaf:label`. The stopped draft is not accepted evidence; stop rather
  than weaken the anchored regex if the isolated update differs.
- Add no physical source leaf, registry, lockfile, mutation, action, copied or
  generated `@bazel_tools` content, configured analysis, or toolchain
  resolution.

## Files

Change only:

- new
  `tests/v2_oracle/fixtures/query-attr-observable-candidates/fixture.toml`;
- its new `expected/oracle.json`;
- `tests/v2_fixture_payload/fixtures.payload` for the named five-file
  projection;
- `tests/v2_oracle/test_v2_oracle.py` for derived global count/body-byte/SHA
  and the new Python projection hash; and
- `tests/v2_fixture_support/src/lib.rs` for derived global SHA and the
  275-to-285 entry-count assertion.

Do not add the new fixture to Rust `PROJECTIONS`, CLI/server cases, or any
production consumer. Existing fourteen projection hashes remain byte-exact.
Do not change runner, BUILD wiring, production Rust, Cargo/lockfile, plans,
graph, DICE, or regex code.

## Growth and validation

From hygiene reset `51540963`, cap the payload-expanded corpus at +7 regular
files, +5 directories, zero links, exactly 18 rows, and 2,400 newline-counted
source/TOML/expected lines. The encoded payload grows by ten entries, so its
global entry/directory pair must become `(285, 117)`; generate body bytes and
hashes rather than predicting them. Require a fresh hygiene review before any
later fixture packet.

Use ordinary Bazel RC discovery without inspecting, printing, or copying the
private home RC. Run one explicit update and one clean distinct-root replay of
all 18 rows. Run frozen Python payload inventory/projection/metadata tests,
Rust global payload conformance with no new projection, the protected 29-row
CLI and two generated-kind CLI/server cases, and `git diff --check`, serially
where Cargo state is shared. Obtain independent oracle-evidence review before
acceptance.

## Stops

Stop and `REPLAN` on a sixth virtual source, any link/mutation/registry/
lockfile/new tools content, more than 18 rows or 2,400 logical lines, a Rust
projection or semantic consumer, protected projection/output drift, missing or
reused atom pairs, a nonexact external token, a need for configured analysis,
or any production Rust, graph, DICE, regex, JVM/Java artifact, or Bazel-
delegation change.
