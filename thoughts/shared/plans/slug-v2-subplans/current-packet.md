# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-developer-gate-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a docs-only design for authenticated cache/RBE evidence after Gate C1.

## Goal and required design

Freeze the smallest Bazel 9.2 developer gate that distinguishes remote unavailable,
cache-only, and execution-enabled BuildBuddy modes through structured, secret-free
evidence before implementation or CI changes. Audit only live repository
configuration, the accepted 43-target Gate C1 graph, and Bazel/BEP surfaces. Never
inspect or reproduce `~/.bazelrc`; credentials remain environment-owned. Define local
fresh/replay commands, cache/RBE discriminators, redaction, failure classification,
platform coverage, review gates, implementation split, and exact caps. Preserve the
blocked core unit target and the user-deferred cyclic-Bzl baseline.

## Stops and budget

Return `REPLAN` rather than infer remote behavior from elapsed text, expose
secrets, add CI, accept silent fallback, change code/locks/evidence, or enter core host
tools, self-hosting, JVM, Bazel 8, WORKSPACE, or cycle semantics. Zero
non-documentation changes; at most 500 authored documentation lines.
