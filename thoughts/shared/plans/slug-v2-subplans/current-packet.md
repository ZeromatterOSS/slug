# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-post-string-list-frontier-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 recursive ruleset loading and the first missing typed semantic owner
Base: `68e458b4`

Result: authenticate the first rules_rust source-order stop after complete
String/Boolean/StringList descriptor-definition loading and select exactly one
bounded implementation packet or `REPLAN`. This packet is read-only and
changes no Rust.

## Accepted basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Commits `573c25c7`, `6811fa84`, and `68e458b4`
accept the live named `.bzl` String, Boolean, nonrepeatable StringList and
repeatable StringList descriptor definitions. Repeatability is structurally
retained through recursive freeze/equality while both list forms select a
list-typed `build_setting_default`. Boolean and every StringList target still
fail before `PackageRecorder`; only the earlier String target/analysis slice is
admitted.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The descriptor inventory includes every `config.string_list` form in
`rust/private/rustc.bzl`, `clippy.bzl`, and `unpretty.bzl`; line 3120 was the
first repeatable occurrence. Source inventory, not the generic public wrapper,
must determine what recursive evaluation or BUILD loading reaches next.

Fresh disposable query/build after removing the separately parked wildcard
registration still return the established public `query_error` exit 7 and
`build_runtime_error` exit 2 with `repository session failed`. Unmodified query
still stops earlier on the registration-label boundary. These are public
wrappers, not evidence of the internal first semantic stop.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its recursive module projection validates one complete typed rule declaration
against invocation, and its configuration design keeps declaration, effective
value, and consumer projections separate. Use those ownership lessons to
reject evaluator-local markers, side registries, or command-side repairs.
Copy no Zabel code, representation, runtime, scheduler or behavior; exact
claims require pinned Bazel 9.2.

## Audit and decision rule

Trace the accepted archive in deterministic recursive load order beyond all
StringList descriptor definitions. Separate:

- `.bzl` parse/global/call-shape or freeze failures;
- rule-definition schema failures;
- BUILD invocation/default coercion and target-publication failures;
- configured analysis/`ctx.build_setting_value` failures; and
- command/repository-session presentation that masks a lower typed terminal.

Inventory every candidate occurrence before choosing one owner. If the first
stop is Boolean/StringList target invocation, trace Bazel `RuleClass`, default
coercion, `StarlarkRuleContext.getBuildSettingValue`, configuration lookup and
the exact rules_rust defaults/consumers. Determine whether a definition-only,
invocation-only, or invocation-plus-analysis slice is semantically complete;
do not publish a target whose configured consumer cannot fail closed at the
next boundary.

If the first stop is another global, rule parameter, provider, attribute,
transition, toolchain or repository surface, trace its pinned Bazel producer,
retained identity, immediate consumers and discriminating tests instead. Select
the smallest source-ordered semantic owner, not a convenient adjacent feature.

Inspect Slug's producer, frozen/retained value, equality/invalidation path,
request/publication boundary and fail-closed behavior. Reuse accepted evidence
before adding an oracle. If no bounded Rust-native slice preserves the declared
compatibility class, record `REPLAN`.

## Ownership, memory and prior art

No semantic owner changes during this audit. For the selected implementation,
name the producer, retained value, schema/analysis projections, request-local
facts, invalidation and publication boundary. Classify any memory as evaluator
scratch, DICE-retained semantic state, command state or async transfer and name
release/cancellation behavior.

Apply the Buck2 utility-reuse skill if the selected packet changes retained
data, hashing, compact collections/strings, interning, clone cost or memory
accounting. Record a Stage 9 decision only when reuse/import changes; do not add
a collection, hash domain, interner or allocation without evidence. Classify
Zabel as concept/test only unless a separately reviewed leaf exists.

## Files, proof and validation

This audit may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

Any selected implementation must record exact base hashes, final line
ceilings, addition caps, file allowlist, observable result, exclusions,
compatibility classes, proof and `REPLAN` stops. Existing large-file ownership
must be justified or split under the authoring guide.

Run `git diff --check`, plan/current alignment and
`scripts/v2_archive_status.sh`, preserving only its known three-path thoughts
classification if unchanged. Read-only source tracing and fresh disposable
smokes are allowed; no Cargo, Bazel oracle, fixture, network mutation or daemon
change is required unless a demonstrated evidence gap demands it.

## Compatibility and STOP

- **Exact:** accepted `.bzl` String/Boolean/StringList descriptor definitions,
  including structural repeatability and list schema; the audit may
  authenticate but not implement the next Bazel surface.
- **Slug-native:** retained Rust representations, valid-Unicode handling,
  fail-closed nonadmitted boundaries and nonrequired diagnostics.
- **Unsupported/deferred:** Boolean/StringList targets and configured values,
  CLI parsing/accumulation, transitions/config matching unless selected after
  audit; later rules_rust/toolchain/action surfaces, M8/M7B and exact output
  bytes.

STOP on Rust changes, an inferred terminal based only on the public wrapper,
target publication without its complete fail-closed consumer boundary,
behavior sourced from Zabel, BUILD/global widening without Bazel authority,
new oracle work without a gap, dirty overlap, or inability to state one
bounded implementation/`REPLAN` with exact evidence and caps.
