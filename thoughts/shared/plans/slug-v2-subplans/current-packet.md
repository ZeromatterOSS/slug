# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-query-unit-ignore-observation-correction`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: the query library is clean at 28/28 with its production Need/restart
and one-route canonical fake-caller semantics unchanged.

## Goal

Correct only the stale observation epoch in
`external_restricted_visible_uses_canonical_fake_caller_without_a_second_route`
by declaring the routed dependency's absent `.bazelignore` file.

## Required design

Add `"/workspace/dep/.bazelignore"` to the existing list whose Host Lstat
observations are `Missing`. Preserve every other observation, the one real
`@dep -> dep+` materialization, both request-local fake callers, visibility
expectations, and direct environment path. Do not add a retry loop: the missing
input, not production Need propagation, is the defect.

## Allowed paths

- `app/slug_query_v2/src/loading_environment.rs` (test module only)
- the canonical plan, Stage 10 owner, and this manifest
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Required validation

Run the focused exact query test, then serial full
`cargo test -p slug_query_v2 --lib`; all 28 cases must pass. Run formatting,
archive, exact test-only scope, cap, credential-pattern, stable-lock, and
`git diff --check` gates and obtain independent latest-diff review. The absent
crate-mode Bazel target is mapped only in the next packet; no Bazel test or
Windows compile is required for one platform-independent missing-path literal.

## Stop conditions

Stop with REPLAN on any production change, expectation change, additional
observation, second route/materialization, retry loop, caller/filesystem/
fresh-graph bypass, DICE key/equality/Need change, lock across a DICE compute,
test filter, BUILD/Cargo/lock/fixture/generated-source change, or coupling to
query/cquery/aquery formatting, execution/cache, host tools, nested fixtures,
self-hosting, Java/JVM delegation, Bazel 8, WORKSPACE, rc, or credentials.

## Diff budget

- Zero production lines, at most two changed test lines, at most 80 net
  documentation lines, and at most 90 net total lines.
