# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-canonical-fixture-payload-migration-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one atomic final-state migration of the 14 shared fixture workspaces to
one canonical payload; three Bazel targets are green and the CLI target retains
only the exact user-deferred cyclic-Bzl negative baseline.

## Goal

Make `tests/v2_fixture_payload/fixtures.payload` the sole canonical byte source
for the 14 Gate C fixture workspaces. Switch Python oracle and all Rust/Cargo/
Bazel consumers to fresh scratch extraction, activate the four test targets,
and delete the old 163 workspace files in the same accepted commit.

## Required implementation

Implement the exact `slug-fixture-payload-v1` path alphabet, grammar,
projection, conformance vectors, inventory, and hashes frozen in Stage 10. Add
a standard-library Python POSIX pack/portable parse/extract owner and one shared
standard-library Rust test-only module. Each selected TOML explicitly names its
payload workspace and initial tree hash while preserving every existing Bazel
provenance field; all other fixtures keep directory-backed loading and the
existing post-materialization template/mutation lifecycle.

Use the accepted source-relative compile mechanism verbatim. The Rust helper
contains `include_bytes!("../../v2_fixture_payload/fixtures.payload")`; Cargo
includes that module with repository-relative `#[path]`. Each of the four
Bazel `rust_test`s lists the helper label in `srcs` and the payload label only
in `compile_data`. Add no `rustc_env`, custom cfg, runtime path lookup, payload
`data`, or runtime runfile. The helper returns a fresh owned scratch guard under
`TEST_TMPDIR` for Bazel or OS temporary storage for Cargo, validates before
writes, rejects pre-existing components, and cleans up on drop. Preserve the
CLI binary data edge and `CARGO_BIN_EXE_slug` behavior.

This is one exceptional atomic migration. Payload and old source trees may
coexist only as uncommitted work. Before the sole final commit, prove exact
source-to-payload equality, stage all 163 source deletions, validate the final
tree, and prove no direct old-workspace read remains. Do not land a payload-
only, consumer-only, or deletion-only commit.

Keep the 57-row CLI test body, cycle assertion, TOML row, and expected JSON
unchanged. Do not add a negative wrapper, fifth target, skip, ignore, filter,
split, or manual tag. The CLI Cargo/Bazel test remains visibly red only at
`bzl_cycle_failure`; pair it with the all-command Slug oracle negative gate
rather than treating an arbitrary nonzero exit as acceptance.

## Allowed paths

- `tests/v2_fixture_payload/**` and `tests/v2_fixture_support/**`
- the 14 selected `tests/v2_oracle/fixtures/*/fixture.toml` files and their
  exact 163 `workspace/**` files, for selectors/hashes and deletion only
- `tools/v2_oracle_lib/{fixture,payload,runner}.py`,
  `tests/v2_oracle/test_v2_oracle.py`, and `tests/v2_oracle/README.md`
- `app/slug_cli_v2/{BUILD.bazel,tests/cli.rs,tests/graph_output.rs}`
- `app/slug_query_v2/{BUILD.bazel,tests/loading_query.rs}`
- `app/slug_server_v2/{BUILD.bazel,src/tests.rs}`
- canonical plan, Stage 9 historical path wording, Stage 10 owner, this
  manifest, archive checker only if exact accounting requires it, and August
  routing history

## Required validation

- Before deletion, pack and extract every workspace and compare ordered
  path/type/logical-mode/byte manifests, the exact 14/112/163/24,939 inventory,
  full payload digest, and all 14 projection hashes. Reject source links.
- Preserve the clean committed 64-command Slug oracle projection. Exclude only
  operational paths/durations and canonicalize numeric fields inside the sole
  cycle `DiceNodeId` by replacing the complete
  `DiceNodeId { engine: [0-9]+, node: [0-9]+ }` fragment with
  `DiceNodeId { engine: <id>, node: <id> }`. Serialize the ordered records with
  Python `json.dumps(records, sort_keys=True, separators=(",", ":"))`, UTF-8
  and no trailing newline. Each record contains only name, argv, exit code,
  normalized stdout/stderr, manifest, and mutations; the resulting SHA-256 is
  `eb217429572083716f41e133cb68c67e8ee3237d2524c65c21d7a5f472709cb4`.
  The migrated replay must have the same digest and exactly one comparison
  failure: `bzl_cycle_failure: stderr did not match /cycle detected in extension
  files/`. Its exit remains 7, stdout empty, and stderr the typed unavailable-
  root-DICE-node terminal. Every other command, including rows after it, passes.
- Run identical Python/Rust canonical and malformed conformance vectors; Python
  oracle and validator suites; fixture listing; archive and packet gates.
  Preserve generic directory-backed fixture tests.
- Run the graph-output, loading-query, and server Cargo/Bazel targets green;
  source-owned counts remain 3/53/34 and each final executable also runs the two
  shared helper conformance cases. The CLI source target remains exactly 38/39
  and its final executable 40/41, with only the unchanged 57-row test failing at
  the cycle row. Build that Bazel target successfully before separately proving
  its expected runtime failure; require exact test counts/name/log, not exit 3
  alone.
- For each Bazel target, `aquery` must list consumer, helper, and payload on the
  Rust compile action; runfiles must omit helper and payload. Keep Cargo and all
  three lock hashes stable. Compile the GNU-Windows branches without running
  them; make no native Windows runtime or hostile race claim.
- Run all 14 fixtures/403 commands against Bazel 9.2 in a fresh root and a
  distinct replay root. Compare normalized exit/stdout/stderr/manifest/command
  results with existing expected evidence; expected JSON must not change.
- Prove no selected direct `workspace` source path remains in live Python or
  Rust. Run formatting, structure, scope, cap, credential-pattern, archive,
  staged-deletion, process-cleanup, and `git diff --check` gates, then obtain
  independent destructive/platform latest-diff review before the one commit.

## Stop conditions

Stop with REPLAN on any committed duplicate/partial state, payload mismatch or
nondeterminism, followed source link, path/type/mode/byte/empty-directory loss,
changed graph/oracle result, missing provenance, runtime lookup/runfile,
undeclared compile input, pre-existing extraction component, Windows exclusion
or native race claim, ambient traversal/archive tool, Cargo execution from
Bazel, Cargo/lock change, package-local fixture export, incomplete consumer
migration, non-reversible deletion, cycle diagnostic/row/assertion change,
arbitrary expected-red acceptance, or coupling to query/cquery/aquery semantics,
core host tools, execution/cache semantics, self-hosting, Java/JVM, Bazel 8,
WORKSPACE, rc, CI, or credentials.

## Diff budget

- Exactly one generated 50,103-byte/1,424-line payload with SHA-256
  `d4a5a0f05866908934725209649897fc7b3cf1dfc3f91aad2f5a9d7725bb5566`.
- Delete exactly 163 files/24,939 bytes/992 logical lines. At most 1,200 added
  handwritten lines under the Stage 10 component allocations, 38 non-generated
  touched files, 3,900 total changed lines, and 1,650 final net lines. No Cargo,
  lock, generated Bazel source, expected JSON, CI, or unrelated change.
