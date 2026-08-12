# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-root-local-order-oracle-implementation`
Milestone: M5 expansion
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: admit declaration-ordered FileWrite actions owned by one literal root.

## Observable slice

Keep the accepted one main-repository literal request and public wire. For its
sole analyzed requested target, emit one or more supported FileWrite actions in
retained per-owner declaration order. Resolve each root action against the full
build action closure, but exclude actions owned by dependencies or semantic
support nodes. Format every block with exactly two trailing LF bytes and keep
the accepted one-action output unchanged.

Exact behavior covers root-local action order, direct-literal dependency
exclusion, action block shape, and per-block framing. Configuration/output-root
and `SlugActionToken` bytes, progress silence, invalidation counts, and errors
remain Slug-native. Cross-owner order, `deps()` activation, shared actions
across distinct owners, aspects, other action kinds/formats, and Bazel identity
bytes remain unsupported/deferred.

## Oracle and implementation

Add one five-file `filewrite-aquery-root-order` Bazel 9.2 fixture. Its root
declares `z-root.txt` then `a-root.txt` and depends on left/right owners with
a shared diamond leaf. Direct-literal A/B/A rows must prove declaration rather
than lexical order and dependency exclusion. One oracle-only `deps(//:root)`
row proves owner membership and single diamond ownership without asserting
cross-owner order. Pinned formatter source plus raw oracle evidence own Bazel's
two-LF fact; normalized patterns own order/exclusion.

Implement root-only semantic view selection in the retained
`BuildCommandEvaluation`; use the complete closure only for platform and
constraint resolution. Format all root views without sorting. Fail closed for
zero/multiple requested analyses, unsupported root actions, and every existing
semantic integrity failure.

## Allowlist and caps

Only:

- the five files in
  `tests/v2_oracle/fixtures/filewrite-aquery-root-order/`: `fixture.toml`,
  `workspace/MODULE.bazel`, `workspace/BUILD.bazel`, `workspace/defs.bzl`, and
  `expected/oracle.json`;
- `app/slug_core_v2/src/runtime/{dice.rs,file_write_aquery_text.rs}`;
- focused `app/slug_cli_v2/tests/cli.rs`; and
- bundled Stage 8/current/canonical bookkeeping.

Caps: 70 production / 220 tests / 290 total Rust net lines; five fixture files /
350 fixture text lines; bookkeeping excluded.

## Validation and stops

Run the new pinned Bazel 9.2 fixture and protected one-action evidence. Prove
core root-only order/exclusion/full-closure resolution and CLI default/explicit,
one-shot/daemon, framing, dependency exclusion, retained A/B/A restoration, and
stable daemon PID. Run direct compile dependents, rustfmt, archive, and diff
checks; clean stale `slugd`; require independent final review.

Add no command/wire fields, query functions, cross-owner ordering, action
reconstruction, DICE key/state, execution, file contents, other action
kinds/formats, retained identity changes, exact Bazel identity bytes, JVM/Java,
REAPI, or CI. One material correction maximum; a second is `REPLAN`.
