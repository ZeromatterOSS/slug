# Current Slug V2 Packet

Packet: `WP-4-7A-post-rust-clippy-source-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: authenticate the remaining evaluated declarations in rules_rust
`rust/private/clippy.bzl`, identify the first unsupported expression, and
select one bounded loading packet or `REPLAN`. This is docs-only.

## Accepted starting point and audit horizon

Base is `993ba5e4` (`Load OutputGroupInfo declaration global`). The exact
`rust_clippy_aspect`, `_rust_clippy_rule_impl` and `rust_clippy` declaration
freeze through line 461 of rules_rust 0.73.0. The source SHA-256 is
`a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`.

Audit the remaining evaluated declarations in source order, lines 463-596:

- documented two-field `RustClippyTestInfo` provider;
- `_CLIPPY_OUTPUT_GROUPS = ["clippy_checks", "clippy_output"]`;
- lazy aspect/rule helpers using imported lint-test functions;
- `_rust_clippy_test_aspect` requiring the accepted clippy aspect;
- `rust_clippy_test`, which merges imported `LINT_TEST_COMMON_ATTRS` with one
  provider-constrained, aspect-bearing, platform-transitioned label-list;
- `capture_clippy_output` and `clippy_output_diagnostics`, each using an
  already-shaped Boolean build-setting descriptor.

Stop at the first unsupported evaluated expression. Do not assume the tail
closes merely because individual constructor shapes resemble accepted slices.

## Authorities and required audit

Bazel 9.2 clean commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority. Reuse
the already-pinned provider schema, aspect requirement/provides, rule attribute,
dictionary merge, transition and Boolean build-setting evidence only after
showing the live tail uses the same contract. Add source anchors only for a
demonstrated gap.

Authenticate the selected rules_rust 0.73.0 archive and the defining modules
for imported `LINT_TEST_COMMON_ATTRS`, `platform_transition`,
`lint_test_aspect_impl` and `lint_test_rule_impl`. Trace export/alias identities,
load order and whether every imported module already freezes on the live Slug
surface. Function bodies remain lazy but their global names must resolve at
compile time, as the preceding `OutputGroupInfo` stop demonstrated.

Inspect the live Slug owners for:

- provider doc/schema identity and arbitrary documented field values;
- immutable string-list top-level values;
- complete frozen aspect requirements/provides;
- label-list provider/aspect/transition metadata and dict overlay ordering;
- test rule capability and Boolean build-setting declaration loading;
- fail-closed target invocation before configured provider/aspect/transition or
  unsupported build-setting semantics could be dropped.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture guidance only.
Consult its declaration-owned provider definitions, named attributes and
aspect/rule schemas solely to evaluate owner reuse and detachment boundaries.
Copy no Zig code, layout, source behavior, provider value, configured capture,
transition, action or diagnostic. Bazel 9.2 decides compatibility.

## Compatibility questions

Classify any selected closure explicitly:

- **Exact candidate:** source-order loading/freeze of declarations whose Bazel
  contract and imported producer identities are fully authenticated.
- **Slug-native candidate:** existing Rust frozen-value/Arc ownership and any
  already-admitted fail-closed invocation boundary.
- **Unsupported/deferred:** implementation execution; `OutputGroupInfo`
  construction/equality/values; configured aspect application; provider
  matching; transition execution; test runner/actions; build-setting configured
  values not already admitted; any expression after the proven stop.

## Allowlist and deliverable

Only these documentation files may change:

- `.codex/skills/slug-agent-orchestration/references/routing-log.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

The deliverable must record:

1. exact selected-source hashes/lines and recursive imported identities;
2. the first unsupported evaluated expression or proof that the tail freezes;
3. Bazel source/test anchors and existing Slug owners for every claimed shape;
4. the precise Zabel guidance used and excluded;
5. Buck2 utility/retained-memory review if representation changes are proposed;
6. one bounded implementation packet with file hashes, caps, discriminating
   proof, serial validation and independent review, or `REPLAN`.

No Cargo, daemon, oracle or smoke run is required. Run `git diff --check` and
`scripts/v2_archive_status.sh`; only its three known archive-only misses may
remain.

STOP and `REPLAN` for Rust/test edits; dirty authority; Java/JVM work; helper
execution; configured provider/aspect/transition/test/action work; constructed
OutputGroupInfo; copied Zabel content; invented parity; skipped source order;
another source module without authenticated traversal; or an unbounded packet.

## Immediate predecessor

`993ba5e4` accepted the fixed `.bzl` OutputGroupInfo declaration token and exact
`rust_clippy` source closure with full validation and terminal review.
