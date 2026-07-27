# Current Slug V2 Packet

Packet: `WP-8-m3-query-package-output`
Milestone: M3, ordinary loading query
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Heading: `Bundled Java Pattern query-functions contract replanned (2026-07-27)`
Stage 9: `slug-v2-subplans/09-v1-extraction-ledger.md`,
`Stage 8 loading-query thin vertical — approved extraction plan`
Evidence: Bazel 9.2 `PackageOutputFormatter.java`; accepted
`query-loading-thin-vertical` fixture and current V2 query graph through
`72aece4d`
Validation tier: public/cross-crate Rust plus three protected oracle rows

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
- terminal owner, Stage 9, canonical, manifest, and exceptional routing updates

Result: add exact Bazel 9.2 `--output=package` over the already-selected query
labels. Main-repository package names omit `//`; results sort
lexicographically, deduplicate, and ignore `--order_output`. Keep the existing
13-function registry and graph/DICE ownership unchanged.

Validation: one pinned generation plus two fresh-root exact replays of the
three new rows, all protected fixture rows, focused and direct four-crate
tests, formatting, `git diff --check`, archive status, and daemon cleanup. Do
not add dependencies, query functions, graph/loading state, target-pattern
breadth, JVM/regex code, fixtures, or workspace assets.
