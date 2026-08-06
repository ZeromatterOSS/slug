# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-server-loadfiles-package-fixture-correction`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: the two preexisting server `loadfiles` scratch tests have valid Bazel
package fixtures and the full 34-case server library is green.

## Goal

Correct only the package setup in two existing server scratch tests so every
absolute `.bzl` load names a real Bazel package before the atomic fixture
payload migration is retried.

## Required implementation

In `app/slug_server_v2/src/tests.rs`, create empty `BUILD.bazel` files in
`shared`, `root`, `leaf`, and `alternate` before constructing each test daemon.
Keep every existing `.bzl` byte, query, output, invalidation count, lifecycle,
and assertion unchanged. This is test setup only: Bazel absolute load labels
require their target directories to be packages.

## Allowed paths

- `app/slug_server_v2/src/tests.rs`
- canonical plan, Stage 10 owner, this manifest, and August routing history

## Required validation

Run the two exact focused server tests, then serial
`cargo test -p slug_server_v2 --lib` with all 34 source cases passing. Run Rust
formatting, archive, exact scope/cap, credential-pattern, stable-lock, process
cleanup, and `git diff --check` gates. Obtain independent latest-diff review.

## Stop conditions

Stop with REPLAN on production code, changed `.bzl`/query/output/lifecycle or
assertion, payload/helper/fixture/TOML/BUILD metadata, dependency/Cargo/lock,
ambient tool, platform exclusion, or coupling to query implementation, DICE,
the separate broken-Bzl diagnostic baseline, Bazel targets, execution/cache,
self-hosting, Java/JVM, Bazel 8, WORKSPACE, rc, CI, or credentials.

## Diff budget

- Exactly four test setup lines and at most 100 net documentation lines. No
  production, dependency, lock, fixture corpus, generated, CI, deletion, or
  unrelated change.
