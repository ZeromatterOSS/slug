# Current Slug V2 Packet

Packet: `WP-5-m1-query-host-migration-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted dormant typed query command root; accepted Host path,
directory, package-boundary, glob, and root-loading owners
Validation tier: reserved DICE identity and source-discovery migration design

Design inspection surfaces:

- `app/slug_query_v2/Cargo.toml`
- `app/slug_query_v2/src/graph.rs`
- `app/slug_query_v2/src/loading_environment.rs`
- `app/slug_query_v2/src/evaluator.rs`
- `app/slug_query_v2/src/lib.rs`
- focused query tests and the accepted Host directory/discovery APIs they use

Design result: freeze the exact implementation allowlist that migrates the
typed root query environment's remaining subtree package discovery and
build-companion lookup from eager workspace projections to Host
`PathOutcome`/preparation ownership. Preserve ordering and lazy traversal;
Need must remain typed and must not become `QueryError`.

Add no Rust, Cargo, fixture, oracle, legacy-path replacement, core/CLI/server
caller, runtime activation, external-repository breadth, new query surface,
JVM, Java bytecode, or Bazel delegation. Reuse existing query and Host
lifecycle evidence. Stop if the migration requires eager workspace scanning,
changes a legacy key, or combines build activation.

Validate targeted source/caller/dependency/forbidden-path scans,
`git diff --check`, and exact scheduling-doc scope. Obtain one independent
reserved-boundary design review.
