# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-evidence-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a docs-only implementation contract for secret-safe authenticated
BuildBuddy cache prime/replay evidence.

## Goal and required design

Freeze the smallest local driver and sanitizer that prove a BuildBuddy remote-cache
prime/replay for the exact 43-green target set without exposing authentication.
Specify the checked-in target manifest, two distinct fresh output bases, cache-read
disable/synchronous prime, cache-enabled replay, cache-only execution strategy,
disposable mode-0700 raw BEP/execution logs outside the checkout, closed sanitized
field schema, digest/runner/cache-hit predicates, failure classes, cleanup, platform
coverage, implementation split, tests, and exact caps. Authentication may be consumed
by ordinary Bazel RC discovery but must never be inspected, expanded, echoed, or
persisted. Preserve the expected-red cycle target as its separate accepted negative
gate and the blocked core unit outside the remote claim.

## Stops and budget

Return `REPLAN` rather than retain raw sensitive artifacts, infer cache behavior from
elapsed text or aggregate process totals, accept a partial/mixed/local replay, invent
an endpoint or credential path, combine RBE, add CI, run an authenticated invocation,
or change configuration/code/locks/evidence/targets/cycle/core/platform behavior.
Zero non-documentation changes; at most 420 authored documentation lines.
