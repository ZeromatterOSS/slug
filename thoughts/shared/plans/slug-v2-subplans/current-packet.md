# Current Slug V2 Packet

Packet: `WP-10-m8-host-bzl-parse-diagnostic-parity-implementation`
Milestone: M8 Bazel developer graph prerequisite
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: the Host `.bzl` parse route emits Bazel 9.2's pinned compilation
summary and the full CLI fixture target returns to green.

## Goal

Repair only `HostBzlModuleError::Parse` presentation so malformed root-repo
`.bzl` files retain parser detail and append the exact logical module summary
already used by the legacy loader.

## Required implementation

In `bzl_module.rs`, derive `package/target` from the existing validated private
`HostRootBzlLabel` fields, omitting the slash for the root package. Change only
the `Parse` display arm to append a newline and
`compilation of module '<logical-path>' failed` after the existing
`parsing <label>: <parser-message>` text.

In the existing Host retained-lifecycle test, keep the malformed `a.bzl`
transition and current assertion, then require the complete outer load context
and `compilation of module 'pkg/a.bzl' failed` fragment. Keep its later recovery
and all missing/load-label/cycle/evaluation/freeze behavior unchanged. In the
existing Host load-label unit test, construct the root-package `Parse` display
from the resolved `:a.bzl` label and require `compilation of module 'a.bzl'
failed`, proving that the logical path has no leading slash. Do not edit the
pinned CLI/oracle expectation.

## Allowed paths

- `app/slug_loading_v2/src/bzl_module.rs`
- `app/slug_loading_v2/src/host_package_load_tests.rs`
- canonical plan, Stage 10 owner, this manifest, and August routing history

## Required validation

Run the exact Host load-label and lifecycle cases, full loading library,
53-case loading-query integration, exact broken-Bzl CLI case, then the full
39-case CLI integration and 34-case server library serially. Run loading GNU-
Windows check, Rust formatting, archive, exact scope/cap, credential-pattern,
stable-lock, process cleanup, and `git diff --check` gates. Obtain independent
latest-diff review.

## Stop conditions

Stop with REPLAN on error variant/equality/key/event/DICE changes, absolute
checkout paths in the summary, external-repository or BUILD parse formatting,
missing/load-label/cycle/evaluation/freeze changes, parser/language/query/CLI/
server code, fixture/oracle/assertion weakening, payload migration work, new
dependency, BUILD/Cargo/lock, platform exclusion, execution/cache,
self-hosting, Java/JVM, Bazel 8, WORKSPACE, rc, CI, or credentials.

## Diff budget

- At most 20 production, 12 test, 60 documentation, and 92 total net lines.
  No fixture, oracle, dependency, lock, generated, payload, CI, deletion, or
  unrelated change.
