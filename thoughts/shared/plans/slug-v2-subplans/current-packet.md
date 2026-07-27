# Current Slug V2 Packet

Packet: `WP-8-m3-query-rank-output`
Milestone: M3, ordinary loading query
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Evidence: Bazel 9.2 `MinrankOutputFormatter.java`,
`MaxrankOutputFormatter.java`, and `RankAndLabel.java`; accepted
`query-loading-thin-vertical` fixture and current selected query graph
Validation tier: public/cross-crate Rust plus six protected oracle rows

Allowed files:

- `tests/v2_oracle/fixtures/query-loading-thin-vertical/fixture.toml`
- `tests/v2_oracle/fixtures/query-loading-thin-vertical/expected/oracle.json`
- `app/slug_commands_v2/src/common.rs`
- `app/slug_commands_v2/src/query.rs`
- `app/slug_commands_v2/tests/commands.rs`
- `app/slug_query_v2/src/output.rs`
- `app/slug_query_v2/tests/loading_query.rs`
- `app/slug_cli_v2/src/commands/query.rs`
- `app/slug_cli_v2/tests/cli.rs`
- `app/slug_server_v2/src/lib.rs`
- `app/slug_server_v2/src/tests.rs`
- terminal owner, canonical, manifest, and exceptional routing updates

Result: add exact Bazel 9.2 `--output=minrank` and `--output=maxrank` over the
already-selected query graph. Condense strongly connected components so every
cycle member has one rank. Minrank is shortest SCC-path distance from a root;
maxrank is longest acyclic SCC-path distance. Full order sorts by rank then
Bazel-natural label; auto order preserves Bazel's rank-stable traversal order.
Keep graph/DICE identity and retained representation unchanged.

Extend only the existing fixture with six rows covering a multi-root graph
that distinguishes minimum from maximum rank, a cycle, and auto/full ordering.
This is oracle packet five after checkpoint `e2cc891d`; no growth review is due
until before the next oracle packet. Validate one pinned Bazel generation, one
exact Slug replay of all protected rows, focused and direct four-crate tests,
formatting, `git diff --check`, archive status, and daemon cleanup.

Do not add dependencies, query functions, graph/loading state, target-pattern
breadth, external repositories, JVM/regex code, fixtures, or workspace assets.
Stop if exact auto order needs missing evaluator state or implementation would
change graph identity, DICE ownership, or retained representation.
