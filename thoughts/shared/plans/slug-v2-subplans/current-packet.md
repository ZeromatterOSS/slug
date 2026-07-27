# Current Slug V2 Packet

Packet: `WP-8-m3-query-java-pattern-functions-contract`
Milestone: M3, ordinary loading query
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Headings: `Current M3 status`, `Java Pattern feasibility audit accepted
(2026-07-23)`, and `java_regex 0.1.0 qualification rejected (2026-07-23)`
Stage 9: `slug-v2-subplans/09-v1-extraction-ledger.md`,
`Stage 8 loading-query thin vertical — approved extraction plan`
Evidence: pinned Bazel 9.2/OpenJDK sources and oracle `5e78abc1` named by the
owner headings; current V2 query graph/metadata through `72aece4d`
Validation tier: docs/instructions

Allowed files:

- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`
- terminal scheduling updates to this manifest and the canonical plan

Result: freeze one bounded Rust-owned implementation contract for the exact
Java-compatible boolean `Pattern.compile` plus `Matcher.find` substrate shared
by `attr`, `filter`, and regex-based `kind`, including exact production/test
allowlists, retained query metadata, diagnostics, UTF-16 semantics, resource
bounds, and a discriminating Bazel gate. If no bounded Rust path exists,
record `REPLAN` instead of authorizing JVM bytecode, an embedded JVM, Bazel/Java
delegation, a parity subset, or three separate function packets.

Validation: source/heading/allowlist consistency and `git diff --check`. Do
not edit Rust, Cargo, dependencies, fixtures, generated oracles, routing logs,
loading/glob files, or activate any query function in this contract packet.
