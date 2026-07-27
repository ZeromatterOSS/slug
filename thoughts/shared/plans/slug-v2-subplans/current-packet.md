# Current Slug V2 Packet

Packet: `WP-8-m3-query-label-output`
Milestone: M3, ordinary loading query
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Evidence: Bazel 9.2 `LabelOutputFormatter.java`, `OutputFormatters.java`, and
observed invalid-format behavior; accepted `query-path-topology` fixture and
existing default label renderer
Validation tier: public/cross-crate Rust plus three protected oracle rows

Allowed files:

- `tests/v2_oracle/fixtures/query-path-topology/fixture.toml`
- `tests/v2_oracle/fixtures/query-path-topology/expected/oracle.json`
- `app/slug_commands_v2/src/common.rs`
- `app/slug_commands_v2/src/query.rs`
- `app/slug_commands_v2/tests/commands.rs`
- `app/slug_cli_v2/src/commands/query.rs`
- `app/slug_cli_v2/tests/cli.rs`
- `app/slug_server_v2/src/lib.rs`
- `app/slug_server_v2/src/tests.rs`
- terminal owner, canonical, manifest, and exceptional routing updates

Result: accept explicit Bazel 9.2 `query --output=label` through the same
renderer as default query output in one-shot and daemon paths. Remove the
prototype-only public `--output=text`; it must fail at parse time with Bazel's
invalid-format message and exit class. Keep the internal default-output
representation request-local and keep query graph/DICE behavior unchanged.

Add three rows to the existing path fixture: explicit label auto, explicit
label full, and rejected text with the Bazel 9 valid-format list. This is
oracle packet five after checkpoint `e2cc891d`; no growth review is due until
before the next oracle packet. Validate one pinned Bazel generation, one exact
Slug replay of all protected rows, focused command/CLI/server tests, direct
three-crate tests, formatting, `git diff --check`, archive status, and daemon
cleanup.

Do not add dependencies, query functions, query-graph/loading state,
target-pattern breadth, external repositories, JVM/regex code, fixtures, or
workspace assets. Stop if explicit label cannot reuse the accepted default
renderer or removing public text requires a wire/schema compatibility shim.
