# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-test-target-attribute-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: docs-only dependency-attribute declaration architecture audit
Base: `50205fb3`

Result: audit accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:218-243`, identify the exact unsupported boundary in
the fixed `targets = attr.label_list(...)` descriptor and select one bounded
declaration-only loading packet or `REPLAN`. Make no Rust, fixture, oracle or
behavior change.

## Accepted starting point and source order

Commit `50205fb3` freezes the third rustfmt aspect with its advertised
`RustfmtTestInfo` producer identity and complete recursive required-aspect
objects. Implementations remain lazy and no aspect is applied.

Source order then reaches `rustfmt_test = rule(...)`. The implementation
function is already lazy, `test = True` and rule documentation are accepted,
and `LINT_TEST_COMMON_ATTRS` was frozen in the preceding lint-test packet. The
`attrs = dict(LINT_TEST_COMMON_ATTRS, **{"targets": ...})` expression must be
checked against the live evaluator for merge order, duplicate handling and
descriptor identity. The fixed `targets` descriptor is a label list with:

- a string `doc`;
- provider alternatives `[[rust_common.crate_info],
  [rust_common.test_crate_info]]`;
- `aspects = [_rustfmt_test_aspect]`; and
- `cfg = platform_transition` imported from `lint_test.bzl`.

Slug's `attr.label_list` currently accepts only `mandatory`, `configurable`
and `default`; `doc` is the first unknown named argument. The audit must decide
whether all four fixed facts form one bounded declaration schema or require a
smaller source-order packet/`REPLAN`.

## Required authorities

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive.

Inspect at minimum:

- `StarlarkAttrModuleApi.labelListAttribute` and
  `StarlarkAttrModule.labelListAttribute`/`createAttributeFactory` for doc,
  provider, aspect and `cfg` contracts;
- `buildProviderPredicate`, provider-key conversion, `convertCfg` and aspect
  export validation, including empty/duplicate/wider/invalid cases;
- `StarlarkRuleClassFunctions.rule` and attribute builder/freeze ownership for
  retained dependency metadata and implementation laziness;
- Starlark `dict` construction and `**` merge behavior for the exact
  `LINT_TEST_COMMON_ATTRS` overlay; and
- focused Bazel tests for provider predicates, attached aspects, Starlark
  attribute transitions, docs and frozen descriptors.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect `src/starlark_host/engine/build_rule_declaration.zig`'s single
`AttrDefinition` owner for optional providers/aspects/cfg, module freeze and
export validation. Inspect
`src/starlark_host/engine/build_invocation_capture.zig` only to understand the
later detached provider/aspect/transition provenance boundary. Determine
whether Slug can retain the fixed declaration facts in its existing
transient/frozen rule attribute schema without raw evaluator values, a side
registry or consumer reconstruction. Do not copy Zig code, representation,
evaluator behavior, cache, analysis algorithm or compatibility conclusions;
Bazel remains sole behavior authority.

## Audit questions and compatibility classification

Answer all before selecting implementation:

1. Does Bazel validate and snapshot `doc`, provider predicates, attached
   aspects and custom cfg during descriptor construction, rule construction or
   freeze, and what exact producer identity survives each step?
2. What are Bazel's flat/nested provider predicate normalization, empty,
   duplicate, unexported and native-provider behaviors for this fixed call?
3. Must `_rustfmt_test_aspect` already be exported when attached, and does an
   importer or rule declaration ever rebind its first-export identity?
4. Does `cfg = platform_transition` retain the complete transition object,
   producer identity or another structural projection, and can it reuse
   Slug's existing frozen `TransitionDefinitionGen` safely?
5. Does the exact `dict(base, **overlay)` preserve insertion position for a
   replaced key, reject a duplicate between positional and keyword inputs, or
   use another rule relevant to the fixed merge?
6. Can one existing `RuleAttributeSchemaGen` owner represent all admitted
   facts without applying the aspect/transition or advancing configured
   dependency analysis? What exact invalid breadth must fail closed?
7. If the fixed descriptor freezes, what is the next unsupported expression
   or phase, and does it require target invocation or configured analysis?

Classify every selected behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. At minimum, aspect application/propagation,
transition evaluation, configured dependency provider matching, target
invocation, `ctx.attr`, analysis/actions, M8/M7B and exact Bazel
configuration/output identity remain deferred unless a later packet separately
authenticates them.

## Ownership, utility and scope gates

Prefer the existing `RuleAttributeSchemaGen`, `ProviderId`, frozen aspect and
transition values, canonical labels, compact strings and Arc slices. Preserve
each producer's defining module and first export. An importer, rule consumer,
BUILD package or configured-analysis command must not reconstruct those
identities. Retained declaration values must freeze with the recursive Bzl
module and borrow no evaluator heap or request scratch.

The Buck2 utility audit is required before selecting implementation because
provider alternatives and aspect sequences alter retained collections and
clone/memory accounting. Record reuse or `REPLAN`; authorize no new interner,
hash family, registry or side cache. No DICE key, mapping, source observer,
I/O, lock, async task or command result may change in this audit.

## Docs-only allowlist and validation

Only these files may change from base `50205fb3`:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

Validate both pinned commits, accepted archive provenance, source order, the
live Slug signature, canonical/manifest packet-ID agreement, document
structure, `git diff --check` and exact file allowlist. A selected
implementation packet must define exact discriminating proof, file hashes and
physical/addition/function caps, serial validation, residual risk and STOP
conditions. Independent terminal review is mandatory before commit.

## STOP / `REPLAN`

STOP and `REPLAN` if no bounded declaration-only design can preserve provider,
aspect and transition producer identity in the existing frozen rule schema; if
the fixed call requires aspect application, transition execution, configured
provider matching, target invocation or analysis/actions; if it requires raw
evaluator-value retention, a new DICE key, registry, cache, I/O path, mapping,
interner, hash or lifetime owner; if proof requires a new oracle, fixture,
network request or Java/JVM component inside Slug; or if behavior depends on
Zabel rather than pinned Bazel 9.2.
