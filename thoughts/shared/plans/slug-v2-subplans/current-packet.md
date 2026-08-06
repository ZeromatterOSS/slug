# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-bzlmod-caller-location-expectation-correction`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: the Bzlmod library is clean at 278/278 with its existing Bazel-exact
caller-location producer unchanged.

## Goal

Correct only the four stale expected spans in
`records_exact_proxy_tag_and_innate_call_spans` to the pinned Bazel 9.2
opening-parenthesis points.

## Required design

Keep the existing `LogicalSpan` helper and logical file assertion. Replace the
expected locations with line 2 columns `22–22` for `use_extension`, line 3
columns `10–10` for `proxy.tag`, and line 5 columns `5–5` for both the innate
proxy and its tag. These are zero-width half-open encodings of Bazel's 1-based
caller `Location` points. Do not alter production code or add a new test.

## Allowed paths

- `app/slug_bzlmod_v2/src/module_eval.rs` (test module only)
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the focused exact-location Cargo test, then serial full
`cargo test -p slug_bzlmod_v2 --lib`; all 278 cases must pass. Run formatting,
archive, exact test-only scope, cap, credential-pattern, stable-lock, and
`git diff --check` gates and obtain independent latest-diff review. The absent
crate-mode Bazel target is mapped only in the next packet; no Bazel test or
Windows compile is required for four platform-independent expected literals.

## Stop conditions

Stop with REPLAN on any change to `nonroot_span`, `LogicalSpan`, retained
values/equality/finalization, AST/source retention, include identity,
DICE/source preparation, BUILD/Cargo/lock/fixture/generated source, or any
expectation other than the four pinned point locations. Do not couple query,
cquery, aquery, execution/cache, host tools, fixtures, self-hosting, Java/JVM
delegation, Bazel 8, WORKSPACE, rc, or credentials.

## Diff budget

- Zero production lines, at most eight changed test lines, at most 80 net
  documentation lines, and at most 90 net total lines.
