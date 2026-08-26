# Current Slug V2 Packet

Packet: `WP-4-7A-rust-analyzer-detect-sysroot-rule-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: selected defining-module mapping, shared Label resolver and frozen rule schema
Base: `61cb0ad0`

Result: load and recursively freeze the exact
`rust_analyzer_detect_sysroot = rule(...)` declaration at accepted rules_rust
`rust/private/rust_analyzer.bzl:475-484`. Resolve its two raw apparent-self
toolchain strings through the defining module's already-retained selected
mapping and preserve the resulting canonical mandatory requirements in source
order. Stop before target invocation, `ctx.toolchains` or actions.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `BazelModuleContext` immutably retains every evaluated `.bzl` module's label,
  repository mapping and loads. `LabelConverter.forBzlEvaluatingThread` selects
  the innermost executing Starlark function's module context rather than the
  importing evaluator's outer module.
- `LabelConverter.convert` delegates each string to
  `Label.parseWithPackageContext` using that defining package and mapping.
  Both raw `@rules_rust//...` strings therefore resolve through the explicit
  module-local self-name entry; canonical identity is not inferred from text.
- `StarlarkRuleClassFunctions.createRule` supplies that converter to
  `parseToolchainTypes`. A plain string becomes a mandatory
  `ToolchainTypeRequirement`. Its `LinkedHashMap` projection preserves the
  first occurrence of each distinct Label and combines duplicates with the
  strictest policy before producing the ordered immutable set.
- `StarlarkRuleClassFunctionsTest.testRuleAddToolchain` authenticates plain
  strings as mandatory, `testRuleAddToolchain_duplicate` authenticates strictest
  deduplication, and `testRuleOrderedRequirements` authenticates source order.
  The fixed two labels are distinct, so only string conversion, mandatory
  policy and order are active.
- The accepted rules_rust implementation body at lines 431-473 is merely
  retained as the rule implementation function during declaration evaluation.
  No `ctx.toolchains` lookup, fail branch, provider field, path operation,
  declared output, JSON write or `DefaultInfo` construction runs.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The fixed declaration's ordered strings are
`@rules_rust//rust:toolchain_type` and
`@rules_rust//rust/rust_analyzer:toolchain_type` at lines 478-479. Its
`dedent(...)` call yields an already-admitted string documentation value.

Commit `61cb0ad0` already retains the selected route's ordered
`Arc<[(ApparentRepoName, CanonicalRepoName)]>` on each recursive external
`BzlModuleIdentity`, includes it in equality/hash/fingerprinting, and selects
the complete defining identity through native-call source provenance with
`DefInfo` fallback. The pure bounded resolver in `starlark_label.rs` resolves
`@name//package:target` only through that mapping and rejects missing or
duplicate/conflicting entries. `rule_toolchain_requirement` is the remaining
gap: canonical strings have a fixed handoff, but every other string still uses
the older root-only package parser, which rejects raw apparent repositories.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `generic_label_value.zig` keeps the selected apparent mapping with the
immutable defining module and exposes a pure shared Label-resolution leaf.
That supports reusing one explicit-input mapping owner and returning a thin
canonical Label. Its `toolchain_declaration_resolution.zig` handles a native
BUILD `toolchain(...)` declaration, not Bazel's `rule(toolchains = ...)`, so it
is not behavior authority and contributes no conversion semantics. Do not copy
Zig code, representation, mapping rules, evaluator behavior or DICE relations.

The Buck2 utility-reuse audit selects the existing mapping Arc,
`CanonicalLabel`, ordered `Vec` collection and frozen Arc slice. Do not add a
map, tree, interner, cache, hash domain or lookup owner.

## Decision and exact boundary

Expose the existing pure `.bzl` label resolver crate-locally without changing
its admitted syntax. In `rule_toolchain_requirement`, preserve the accepted
canonical `@@...` branch and existing relative-string behavior. Add only a raw
apparent `@name//package:target` branch that calls the shared resolver with the
innermost defining `BzlModuleIdentity` already selected by the evaluator
context.

Retain the two resulting `CanonicalLabel` values in the existing ordered
`RuleDefinitionGen.required_toolchains` and `FrozenRuleDefinition` Arc. Both
plain strings are mandatory in Bazel; Slug's currently admitted frozen owner
represents only mandatory requirements, so no new policy field is needed.
Distinct inputs remain distinct and source ordered. Do not admit Label objects,
optional requirements or duplicate/strictest behavior in this packet.

Missing or conflicting apparent mappings must reject before the rule freezes.
Do not guess a self-name, parse canonical identity from apparent spelling, or
consult routes, Bzlmod, files, DICE or I/O from either resolver or converter.

## Ownership, revision and lifetime

The selected route remains the sole mapping producer. Each recursive external
module owns an Arc clone through its frozen identity; imported functions keep
their defining module's mapping. The evaluator context borrows that identity
only for the native call. The shared resolver returns an owned
`CanonicalLabel`, and the existing rule definition/frozen module owners retain
the ordered Arc until those owners die.

No new DICE key, compute, lock, observation, filesystem read, registry lookup,
request overlay, service cache or shutdown duty is added. The already-admitted
complete-map over-invalidation remains Slug-native and cannot under-invalidate.

## Files and caps

Only these files may change, against the listed base SHA-256:

| File | Base SHA-256 | Final line cap |
|---|---|---:|
| `app/slug_loading_v2/src/starlark_label.rs` | `c650c851d16a8f00af54b63cacf9adbec7cf3e34b9fd1abf4613995635a37677` | 190 |
| `app/slug_loading_v2/src/package.rs` | `929afc35507a803856994597a38d9edf042e1f35cfaee8374c1b7aa295e5309e` | 5,510 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `06c99deb45a43f565cfa6a3afc7a6d183339c2de4a613eae943cf5e8f6f7754f` | 4,580 |

Cap production additions at 30, proof additions at 70 and total additions at
100. No touched function may exceed 150 lines. The existing selected-route
test is 142 lines and may receive only one helper call or no body change; keep
new proof in its 30-line focused helper or a separate focused function.
`starlark_label.rs` owns pure defining-context conversion, `package.rs` owns
rule requirement retention and the existing test module owns the selected
route fixture/proof. `REPLAN` before a fourth Rust file or any cap breach.

## Proof and validation

Prove:

- the selected registry mapping still deliberately separates root apparent
  `dep_alias`, module-local self-name `rules_rust` and canonical `dep+`;
- a recursively imported definition evaluates the fixed two raw strings,
  freezes `rust_analyzer_detect_sysroot`, retains exactly
  `@@dep+//rust:toolchain_type` then
  `@@dep+//rust/rust_analyzer:toolchain_type`, and does not execute its
  implementation;
- raw apparent rule strings use the defining module rather than an importer,
  and missing or duplicate/conflicting mappings reject before freeze;
- the accepted canonical current-toolchain handoff, relative rule requirements,
  Label constructor, recursive identity/fingerprint and selected mapping proofs
  remain green; and
- the implementation body's `ctx.toolchains`, fail paths, provider fields,
  path operations, action declaration/write and return value stay unexecuted.

Run serially: focused selected-route and raw-rule requirement tests; the
accepted current-toolchain, Label and recursive provenance tests; full
`cargo test -p slug_loading_v2 --locked`; `cargo fmt --all -- --check`;
`cargo check -p slug_core_v2 --locked`;
`cargo build -p slug_cli_v2 --locked`; `git diff --check`; and
`scripts/v2_archive_status.sh`. Rebuild the CLI before any binary smoke and
clean stale `slugd` before/after daemon-sensitive tests. Pinned source and the
accepted archive suffice; do not run Bazel or add an oracle fixture.

## Compatibility and STOP

- **Exact:** the fixed selected-registry defining-module conversion of the two
  raw strings, mandatory policy, source order, recursive freeze, documentation
  value and producer export identity.
- **Slug-native:** existing Arc-backed complete mapping retention,
  over-invalidation, frozen Arc representation and nonrequired diagnostics.
- **Unsupported/deferred:** Label objects, optional requirements,
  duplicate/strictest behavior, wider Label forms and mapping producers,
  target invocation, `ctx.toolchains`, toolchain registration/resolution/
  selection, configured dependencies, provider access, path semantics, JSON
  FileWrite action, `DefaultInfo`, aspect application, M8/M7B and exact output
  bytes.

STOP on guessed aliases, a second mapping owner, I/O or DICE from conversion,
relative/canonical behavior drift, rule invocation, `ctx.toolchains`, analysis
or action changes, Zabel code/behavior adoption, Java/JVM work,
fixture/network/dependency drift, public rules_rust success claims or any cap
breach. `REPLAN` before widening.
