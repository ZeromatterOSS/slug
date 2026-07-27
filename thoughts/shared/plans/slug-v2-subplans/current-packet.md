# Current Slug V2 Packet

Packet: `WP-5-m1-query-typed-command-root`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted typed query command-root design; public typed root-module
anchor and root-package loading; existing query order/lazy traversal tests
Validation tier: public cross-crate DICE command-root and private restart
control boundary

Implementation files:

- `app/slug_query_v2/Cargo.toml`
- `app/slug_query_v2/src/graph.rs`
- `app/slug_query_v2/src/loading_environment.rs`
- `app/slug_query_v2/src/generic.rs` only if needed to pass the private
  restart sentinel without formatting
- `app/slug_query_v2/src/evaluator.rs`
- `app/slug_query_v2/src/lib.rs`
- `app/slug_query_v2/tests/loading_query.rs`
- `app/slug_query_v2/tests/query.rs`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: add dormant public `RootQueryCommandKey` over normalized workspace,
validated compact source, order, policy, and completion. Always compute the
root-module anchor, use typed root-package loading in the root query
environment, keep typed Needs in its private side channel, and use only an
inert private sentinel to unwind the fixed generic `QueryError` call chain.
Preserve the legacy facade/keys/callers, ordering, and lazy traversal.

Move only `slug_bzlmod_v2` and `slug_workspace_v2` from dev to production
dependencies. Keep subtree discovery and build-companion lookup on their
current eager projections for the separate Host-migration packet.

Add no existing-key replacement, analysis dependency, core/CLI/server caller,
runtime activation, external-repository breadth, recursive Host migration,
new output/query surface, eager preloading, fixture/oracle, JVM, Java bytecode,
or Bazel delegation. Stop on a ninth implementation file, public sentinel
escape, Need text/error conversion, legacy behavior change, or forced lazy
branch.

Validate focused root-query identity/preflight/anchor/Need/non-escape/lazy/
order/equality/lifecycle regressions, full `slug_query_v2`, direct
`slug_core_v2` compile coverage, query/core GNU-Windows no-run linkage,
formatting, `git diff --check`, archive status, and exact allowlist/export/
no-caller/Cargo/dependency/legacy/carrier/IO/blocking/JVM guards. Obtain one
terminal independent implementation review.
