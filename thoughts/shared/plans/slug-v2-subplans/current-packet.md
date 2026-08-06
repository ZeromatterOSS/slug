# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-fixture-payload-compile-input-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: accepted exact Cargo/Bazel compile-time payload embedding evidence—or
terminal REPLAN—before the sole-canonical fixture migration is rescheduled.

## Goal

Prove one rules_rust 0.73/Bazel 9.2 mechanism by which a shared Rust source
module can embed the declared canonical payload at compile time under both
Cargo and Bazel, while the payload and helper are absent from runtime runfiles.

## Required evidence

Use an isolated temporary repository package with the proposed final relative
layout: consumer source, sibling `v2_fixture_support/src/lib.rs`, and sibling
`v2_fixture_payload/fixtures.payload`. Exercise a helper-source-relative
`include_bytes!` path with the helper label in `rust_test.srcs` and payload
label in `compile_data`; do not reuse the rejected execroot-relative
`rustc_env = "$(execpath ...)"` argument as the macro path.

The same helper and payload bytes must compile and run under a standalone
temporary Cargo manifest and the repository's pinned credential-free Bazel
9.2/rules_rust nightly lane. Inspect the Bazel compile action and test runfiles
to prove the payload is a declared compile input but not runtime data. Repeat a
compile-only GNU-Windows target check if the evidence mechanism has a
platform-specific branch. Remove every temporary file before committing docs.

Record the exact working source expression and BUILD attributes, source-file
resolution basis, Cargo and Bazel results, compile-action input, runfiles
absence, remote-sandbox reasoning, and whether the mechanism can be applied
unchanged to all four future test targets. Then either reschedule the atomic
payload implementation with a corrected independently reviewed manifest or
return terminal REPLAN.

## Allowed paths

- temporary `tests/v2_fixture_compile_probe/**`,
  `tests/v2_fixture_support/**`, and `tests/v2_fixture_payload/**`, all removed
  before terminal commit
- the canonical plan, Stage 10 owner, this manifest, and August routing history

## Required validation

Run the standalone Cargo probe; credential-free Bazel 9.2 test; `aquery` of the
Rust compile action; runfiles tree/manifest inspection; applicable GNU-Windows
compile; archive, structure, scope, cap, credential-pattern, and
`git diff --check` gates. Obtain independent latest-text review of the exact
mechanism and successor packet.

## Stop conditions

Stop with REPLAN on an absolute or compile-sandbox path, runtime lookup or
runfile, missing `compile_data`, helper absent from `srcs`, different Cargo and
Bazel payload bytes, undeclared remote input, generated checked-in source,
Cargo execution from Bazel, persistent probe file, Cargo/lock change, Windows
exclusion, production/fixture/application change, or coupling to query/cquery/
aquery semantics, execution/cache semantics, self-hosting, Java/JVM, Bazel 8,
WORKSPACE, rc, CI, or credentials.

## Diff budget

- At most 260 net documentation lines. No persistent Rust, Python, BUILD,
  Cargo, lock, fixture, payload, generated source, CI, deletion, or unrelated
  change.
