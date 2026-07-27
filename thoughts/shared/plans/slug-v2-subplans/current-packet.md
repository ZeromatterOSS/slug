# Current Slug V2 Packet

Packet: `WP-5-m1-query-typed-command-root-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted public typed root-loading and root-analysis boundaries;
existing loading-query evaluator, graph keys, order, and lazy traversal tests
Validation tier: reserved public cross-crate DICE command-root design

Design inspection surfaces:

- `app/slug_query_v2/Cargo.toml`
- `app/slug_query_v2/src/graph.rs`
- `app/slug_query_v2/src/loading_environment.rs`
- `app/slug_query_v2/src/generic.rs`
- `app/slug_query_v2/src/evaluator.rs`
- `app/slug_query_v2/src/lib.rs`
- `app/slug_query_v2/tests/loading_query.rs`
- `app/slug_query_v2/tests/query.rs`

Design result: freeze one dormant always-rooted typed query command key and its
exact implementation allowlist. It must consume the accepted preparation
envelope without converting Need to `QueryError`, preserve current result
ordering and lazy traversal, and make even a valid empty query seal a
nonempty exact dependency closure. Decide whether the production bzlmod
dependency is direct or provided through an accepted loading-crate reexport.

Add no Rust, Cargo, fixture, oracle, Host migration, existing-key replacement,
core/CLI/server caller, runtime activation, external-repository breadth, JVM,
Java bytecode, or Bazel delegation. Reuse current query tests and accepted
loading/analysis evidence. Stop if the design requires eager whole-workspace
loading, exposes Need as an error, changes an existing query key identity, or
mixes the later Host-migration packet into this boundary.

Validate targeted source/caller/dependency/forbidden-path scans,
`git diff --check`, and exact scheduling-doc scope. Obtain one independent
reserved-boundary design review.
