# Slug V2 Current Packet

Packet: `WP-5-m1-loading-pure-host-glob-traversal-owner`
Milestone: M1, private loading
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Heading: `Pure Host glob traversal design`
Stage 9: `slug-v2-subplans/09-v1-extraction-ledger.md`,
`Stage 4 private Host glob segment-candidate owner`
Evidence: accepted traversal oracle `5abff72e`; pinned Bazel 9.2 sources named
by the owner heading
Validation tier: private/local Rust

Allowed files:

- `app/slug_loading_v2/src/host_glob/mod.rs`
- `app/slug_loading_v2/src/host_glob/traversal.rs`
- `app/slug_loading_v2/src/host_glob/traversal_tests.rs`

Result: add the dormant private recursive Host traversal, composing the
accepted segment-candidate and root-package-boundary keys with FIFO error rank,
standalone `**`, operation filtering, raw order/dedup, and same-graph
invalidation/restoration. Keep zero production callers and do not activate
parser, evaluator, callable, query, or consumer surfaces.

Validation: focused host-glob tests, direct bzlmod boundary compile/tests,
loading formatting, `git diff --check`, GNU-Windows no-run only if production
code changed after the last recorded cross-target pass.

Future M3 proposal: `WP-8-m3-query-java-pattern-functions` may bundle an exact
Rust Java-`Pattern` substrate with `attr`, `filter`, and regex-based `kind`.
It is not schedulable until the root freezes its owner, exact allowlist,
Bazel evidence, and validation contract. It must not touch loading/glob files,
execute JVM bytecode, embed a JVM, or delegate production behavior to
Bazel/Java.
