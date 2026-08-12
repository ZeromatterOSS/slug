# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-deps-owner-set-oracle-implementation`
Milestone: M5 expansion
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: activate one bounded aspect-free `deps()` FileWrite owner-set surface
or record `REPLAN`.

## Scope

Accept exactly an unbounded top-level
`deps(<one direct main-repository literal>)` alongside the unchanged direct
literal. Parse a shared typed literal/deps scope independently at the CLI and
daemon boundary while retaining the raw public wire. Reuse the accepted build
DICE evaluation and retained configured action closure. Literal scope remains
sole-root-only; deps scope emits every action-bearing configured owner in the
closure, deduplicated by configured-target identity.

Within the admitted aspect-free graph, owner membership, per-owner declaration
order, block text, and two-LF framing are exact Bazel 9.2 behavior. Cross-owner
order is deterministic Slug-native roots-first breadth-first closure order.
Actionless semantic-support nodes emit nothing; action-bearing resolved
toolchain implementations emit. Mixed non-FileWrite actions fail the entire
request closed.

## Evidence and tests

Extend only the existing five-file `filewrite-aquery-root-order` fixture. Keep
literal dependency exclusion and add order-agnostic deps membership for the
ordinary diamond, action-bearing selected toolchain implementation,
alias/generated producer, and configured transition owner. Prove every shared
configured owner once and preserve raw two-LF framing without claiming
cross-owner order.

Add focused shared-parser/command/server negatives, closure-wide selection and
mixed-action failure tests, and CLI default/explicit plus one-shot/daemon
equality. Retained-daemon A/B/A removes and restores one dependency edge,
proves exact owner membership/token restoration and stable PID, and retains the
direct-literal A/B/A declaration-order regression.

## Allowlist and caps

Edit only:

- `app/slug_query_v2/src/{expr.rs,lib.rs}` and existing query tests;
- `app/slug_commands_v2/src/aquery.rs` and existing command tests;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs,mod.rs}`;
- `app/slug_cli_v2/src/commands/aquery.rs` and existing CLI tests;
- `app/slug_server_v2/src/{lib.rs,server.rs,tests.rs}`;
- the existing five files under
  `tests/v2_oracle/fixtures/filewrite-aquery-root-order/`; and
- canonical/current-packet/Stage 8 bookkeeping.

Cap Rust growth at 250 production, 320 tests, and 570 total net lines. Keep the
fixture at five files and at most 420 text lines. Cap bookkeeping at 170 lines.
One material correction maximum; a second is `REPLAN`.

## Validation

Run the expanded fixture with pinned Bazel 9.2 and the protected
direct-literal/identity evidence. Run focused query, commands, core, server,
and CLI tests; direct compile dependents; rebuilt `slug_cli_v2`; retained-daemon
A/B/A with stable PID; rustfmt; archive; and diff checks. Clean stale `slugd`
before and after daemon validation. Require independent final review.

## Stops

Add no depth/wrapper/general query activation, external/package/multi-root
shape, command/wire field, aspect state, new DICE key/state, action
reconstruction/execution/contents, other action kind/format, retained identity
representation, exact Bazel identity bytes, JVM/Java artifact, REAPI reuse, or
CI. Shared equivalent actions owned by distinct configured owners remain
deferred. Direct-literal output must remain byte-for-byte unchanged.
