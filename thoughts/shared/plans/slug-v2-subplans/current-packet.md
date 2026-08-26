# Current Slug V2 Packet

Packet: `WP-4-7A-rules-rust-keyword-only-arguments-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 Bazel BUILD/`.bzl` dialect and production parse boundaries
Base: `2f373248`

Result: docs-only. Audit the bare-`*` keyword-only definition syntax now
exposed by the accepted rules_rust root, decide whether the retained
starlark-rust parser/resolver/evaluator is sufficient behind one centralized
Bazel-loading dialect, and select one bounded implementation or `REPLAN`.
Do not edit Rust in this packet.

## Authority and live terminal

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority. Read its `Resolver`, `Parameter`,
`StarlarkFunction`, `ResolverTest.testParameterOrdering`, and
`FunctionTest` keyword-only rows. Run a fresh disposable oracle only if source
and retained tests do not discriminate an observed edge.

The exact accepted rules_rust 0.73.0 source reaches
`rust/platform/triple_mappings.bzl:5`:

```starlark
def _support(*, std = False, host_tools = False):
```

Fresh Slug query and build both stop before evaluation with
`* keyword-only-arguments is not allowed in this dialect`. This is the first
honest terminal after commit `2f373248` accepted selected-BCR materialization;
do not revisit repository transport or realization.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Inspect it with `git show`, especially its injected/session Starlark-semantics
projections: relevant evaluators consume one complete typed semantics owner,
instead of reconstructing policy at individual calls. Use that guidance when
choosing a Slug dialect owner, but copy no Zabel code, representation,
fingerprint, scheduler or behavior. Bazel remains syntax/call authority.

## Required read-only audit

1. Inventory every production `AstModule::parse*` boundary for root/external
   BUILD and recursive `.bzl` loading in `slug_loading_v2`. Separate fixtures,
   MODULE evaluation and dormant core evaluators from the command path.
2. Trace the exact parse/evaluate route for the rules_rust file and determine
   the smallest owner that prevents dialect drift across root, external and
   recursively loaded files while preserving `StringEncoding::BazelInternal`.
3. Audit retained starlark-rust `Dialect`, validation, parameter unpacking,
   function compilation and call binding. Prove whether setting only
   `enable_keyword_only_arguments = true` admits bare `*`, required/defaulted
   keyword-only values and `*args`-followed keyword-only values without
   enabling top-level statements, types, positional-only parameters or other
   extended syntax.
4. Authenticate Bazel ordering and calls: required/defaulted keyword-only
   values, positional rejection, missing-argument behavior, `*args` interaction,
   bare `*` with no following parameter, duplicate/multiple-star ordering and
   the real `_support` evaluation. Diagnostics are exact only where existing
   compatibility requires them; otherwise preserve typed loading failure.
5. Determine the exact implementation/test file set, base hashes, line caps
   and validation. Prefer existing Stage 4 tests plus a focused real-route
   fixture; do not vendor rules_rust or the downloaded archive.

## Compatibility decision

- **Exact candidate:** Bazel 9.2 acceptance, binding and evaluation of the
  audited bare-`*`/keyword-only parameter forms across BUILD and `.bzl` loads,
  including the real rules_rust definition and unchanged loaded-module order.
- **Slug-native:** Rust evaluator storage, valid-Unicode source ingestion,
  internal error representation and nonrequired diagnostic wording.
- **Unsupported/deferred:** positional-only `/`, types, f-strings, new
  top-level forms, unrelated dialect flags, MODULE-language widening, generic
  Python syntax, later rules_rust providers/toolchains/actions and M8/M7B.

## Documentation authority and STOP

This packet may change only the canonical plan, this manifest, Stage 4 and the
small Stage 5 routing/acceptance note. Caps are <=40 canonical, <=180 current,
<=220 Stage and <=30 routing additions, <=470 aggregate. Record inspected
source symbols, exact scope, compatibility class, file hashes/caps, tests and
one implementation successor; obtain independent review before activating
Rust.

STOP on dirty overlap, a need to patch parser/evaluator behavior beyond an
existing dialect field, production parse routes that cannot share one lawful
owner, MODULE semantics coupling, unrelated syntax activation, source vendoring,
Java/JVM, or a scope above the caps. `REPLAN` rather than widening.
