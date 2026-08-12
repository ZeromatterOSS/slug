# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-deps-owner-set-platform-oracle-implementation`
Milestone: M5 expansion
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: activate the bounded aspect-free `deps()` FileWrite owner-set surface,
including a one-candidate action-bearing selected toolchain implementation, or
record `REPLAN`.

## Scope

Implement the accepted literal/deps shared expression scope and closure-wide
FileWrite selection. Preserve the raw daemon wire and direct-literal bytes.
For a configured FileWrite action, use the existing selected toolchain platform
when present. Only for an action-bearing selected toolchain implementation with
no toolchain selection, accept exactly one retained candidate execution
platform and derive the action view from that configured platform key.

Relax the selected-implementation postguard only enough to retain actions,
declared outputs, and non-empty built-in `DefaultInfo`; exact candidate
topology, exactly built-in `DefaultInfo` plus `ToolchainInfo`, and no
diagnostics remain required. Do not add a platform field, toolchain selection,
DICE key, or action reconstruction.

## Evidence and tests

Extend only the existing five-file `filewrite-aquery-root-order` fixture with
one registered execution platform. Discriminate an action-bearing selected
toolchain implementation, ordinary diamond, alias/generated producer, and
transitioned owner; use an actionless second toolchain for the transitioned
owner so shared equivalent actions across distinct configurations stay
deferred. Keep literal order/exclusion and order-agnostic deps membership.

Add focused analysis/result/parser/command/core/server/CLI tests. Prove exact
one-candidate derivation, zero/multiple/no-topology failures, provider/topology
postguards, mixed actions, raw-wire revalidation, default/explicit and one-
shot/daemon equality, and stable-PID dependency-edge A/B/A restoration.

## Allowlist and caps

Edit only:

- `app/slug_analysis_v2/src/{dice.rs,result.rs}` and existing analysis tests;
- `app/slug_query_v2/src/{expr.rs,lib.rs}` and existing query tests;
- `app/slug_commands_v2/src/aquery.rs` and existing command tests;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/aquery.rs` and existing CLI tests;
- `app/slug_server_v2/src/{lib.rs,server.rs,tests.rs}`;
- the existing five files under
  `tests/v2_oracle/fixtures/filewrite-aquery-root-order/`; and
- canonical/current-packet/Stage 8 bookkeeping.

Cap Rust growth at 280 production, 380 tests, and 660 total net lines. Keep the
fixture at five files and at most 420 text lines. Cap bookkeeping at 180 lines.
One material correction maximum; a second is `REPLAN`.

## Validation and stops

Run expanded pinned Bazel 9.2 and protected literal/identity evidence; focused
analysis/query/commands/core/server/CLI tests; direct dependents; rebuilt
`slug_cli_v2`; retained-daemon A/B/A; rustfmt, archive, and diff checks.
Require independent final review and clean stale `slugd` before/after.

Add no ordinary zero-toolchain action support, zero/multiple-candidate platform
choice, depth/wrapper/general query activation, external/package/multi-root
shape, command/wire field, recursive toolchain selection, new DICE key/state,
action reconstruction/execution/contents, other aquery action kind/format,
retained identity representation, exact Bazel identity bytes, JVM/Java
artifact, REAPI reuse, or CI. Shared equivalent actions owned by distinct
configured owners remain deferred.
