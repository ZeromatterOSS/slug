# Current Slug V2 Packet

Packet: `WP-4-7A-current-rust-analyzer-toolchain-rule-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: selected repository route, recursive Bzl module context and frozen rule schema
Base: `7b9845cd`

Result: load and recursively freeze the exact
`current_rust_analyzer_toolchain = rule(...)` declaration at accepted
rules_rust `rust/private/rust_analyzer.bzl:423-429`. Resolve its explicit
apparent-self Label through the innermost defining module's already-selected
repository mapping and retain the resulting canonical one-element toolchain
requirement. Stop before target invocation or toolchain selection.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `BazelModuleContext` retains each evaluated `.bzl` module's label,
  repository mapping and loads. `LabelConverter.forBzlEvaluatingThread` reads
  the innermost executing Starlark function's module context, so an imported
  function keeps the exporter's mapping rather than acquiring the importing
  module's mapping.
- `LabelConverter.convert` delegates to `Label.parseWithPackageContext` with
  that defining package and mapping. An apparent self-name therefore resolves
  through a real mapping entry; it is not inferred from canonical spelling.
- `BzlLoadFunctionTest.testLoadBzlFileFromBzlmod` proves the defining `foo`
  module records both `foo -> foo+` and `bar_alias -> bar+` lookups in order,
  while unresolved repositories fail as not defined. Label and Starlark string
  representation tests prove `str(Label(...))` emits the unambiguous `@@...`
  canonical spelling used by the fixed handoff.
- `StarlarkRuleClassFunctions.createRule` passes
  `LabelConverter.forBzlEvaluatingThread` to `parseToolchainTypes`. That helper
  accepts a toolchain requirement, Label or string, preserves first-label
  order in a `LinkedHashMap`, combines duplicates with the strictest mandatory
  policy and makes plain Label/string entries mandatory. The fixed declaration
  exercises one canonical string and therefore retains one mandatory
  requirement without entering duplicate combination.
- `StarlarkRuleClassFunctionsTest.testRuleAddToolchain`,
  `testRuleAddToolchain_duplicate` and `testRuleOrderedRequirements` prove
  conversion, strictest deduplication and order. The implementation function
  at rules_rust lines 404-421 remains retained but unexecuted while the rule at
  lines 423-429 is created and exported.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The exact expression is
`str(Label("@rules_rust//rust/rust_analyzer:toolchain_type"))` at line 427
inside the one-element list at lines 426-428.

Slug's selected-registry route already owns an ordered
`Arc<[(ApparentRepoName, CanonicalRepoName)]>` mapping. It is part of route
equality/hash, and `selected_bzl_load_route` constructs each recursive child's
route from that child's own selected definition and mapping. The external Bzl
DICE key owns the complete route, but `BzlModuleIdentity`, `BzlLoadManifest`
and `BzlEvaluationContext` currently discard the mapping. The shared Label
therefore rejects every explicit repository. Separately,
`rule_toolchain_requirement` reparses an already-canonical external source with
an extra `@@` prefix and accepts no canonical string handoff. These are the
first absent facts; the selected route is their one bounded producer.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `generic_label_value.zig` retains the defining module's canonical
repository and selected apparent-to-canonical mapping as immutable module
context, then lets the shared Label builtin consult the currently executing
module. Its toolchain declaration projection reinforces canonical Label
identity and a narrow declaration-owned result, although that native
`toolchain(...)` surface is not the same Bazel behavior as `rule(toolchains)`.
Follow only the explicit-input, defining-module and thin-projection lessons.
Do not copy Zig code, storage, mapping rules, evaluator behavior or DICE
relations; Bazel 9.2 remains authoritative.

The Buck2 utility-reuse audit selects the route's existing Arc slice,
`CompactString`, `CanonicalLabel` and manifest Arc slices. Do not introduce a
second map, tree, interner, cache, hash domain or lookup owner.

## Decision and exact boundary

Expose one hidden clone of the selected route's existing mapping Arc and carry
it in `BzlModuleIdentity` beside canonical label and workspace path. Local,
direct-local, generated and built-in routes supply an empty mapping until a
later packet authenticates their producers. Because mapping changes affect
module semantics, include the ordered entries in identity equality/hash and
the existing manifest fingerprint stream.

Change the recursive evaluation context's filename projection from canonical
Label alone to the complete module identity. The accepted typed native-call
source remains first and `DefInfo` remains fallback; the chosen identity now
provides both defining Label and defining mapping. Missing or ambiguous source
filenames continue to fail closed.

Extend the shared `.bzl` `Label` constructor only for one apparent repository
form `@name//package:target`. Resolve `name` by exact lookup in the selected
defining module's retained mapping, reject missing or duplicate/conflicting
entries, and construct the existing `CanonicalLabel` owner. Preserve admitted
`//...`, `:...` and Label idempotence. Do not guess self aliases or consult
files, Bzlmod, DICE or routes from the builtin.

The fixed `str(Label(...))` yields a canonical `@@...` string. Admit that
canonical string only in `rule_toolchain_requirement`, validate that it is a
direct target, and retain it in the existing ordered
`RuleDefinitionGen.required_toolchains` / `FrozenRuleDefinition` Arc. Existing
relative toolchain strings remain definition-package relative. Direct apparent
strings such as `rule(toolchains = ["@rules_rust//..."])`, Label objects,
optional requirements and duplicate/strictest behavior remain deferred because
the fixed call does not exercise them.

## Ownership, revision and lifetime

The selected route remains the sole repository-mapping producer and already
participates in external Bzl key equality. The module identity borrows no
route; it clones the producer-owned mapping Arc into the frozen recursive
manifest. Every reachable module keeps its own mapping, so imported functions
use their defining module's context. The evaluation scratch projection dies
with the evaluator; the mapping Arc, canonical Label and frozen requirement die
with the existing frozen module/package owners.

No new DICE key, compute, lock, filesystem read, registry lookup, request
overlay, service cache or shutdown duty is added. Retaining the complete
selected mapping may over-invalidate relative to Bazel's consulted-entry
recorder; this is a Slug-native representation choice that preserves semantic
identity and never under-invalidates.

## Files and caps

Only these files may change, against the listed base SHA-256:

| File | Base SHA-256 | Final line cap |
|---|---|---:|
| `app/slug_bzlmod_v2/src/host_module.rs` | `bf1da8d3f0c9e83386ea006d91931c4f602a428db1802e92acd19c166cb52eab` | 5,365 |
| `app/slug_loading_v2/src/bzl_module.rs` | `f2bc3b16051a318bd74680570f3114d2bbb787bb7004759816a820556de1b633` | 9,710 |
| `app/slug_loading_v2/src/provider.rs` | `520bc3776dd438575685ab3fee7e31312a84d6d54d0b9e24e3de85cc8f35cf0e` | 625 |
| `app/slug_loading_v2/src/starlark_label.rs` | `9e070bdba46c19cfcd6b3b87bc84de2b6f80ad390152fa230cec2ac150e090e4` | 205 |
| `app/slug_loading_v2/src/package.rs` | `96d1c6ebf5609c1ae727d8498a703fb785f89429e6ce9960fb9025192b18439e` | 5,515 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `bd49cf5a2d884b33b0c91a320ba74e7e8efe8d8a9c2bcf6eab3911673e21fd05` | 4,535 |
| `app/slug_loading_v2/src/host_package_attempt_tests.rs` | `2b1729c2722e5499dbcbc8ed32672b13b56fa75106e428df56b1e9d5a1f55280` | 535 |

Cap production additions at 200, proof additions at 100 and total additions at
300. No touched function may exceed 150 lines. Existing larger functions may
receive only a module-identity field initialization and no other body change.
The large files remain cohesive: `host_module.rs` owns the route projection;
`bzl_module.rs` owns recursive identity/fingerprinting; `provider.rs` owns
caller selection; `starlark_label.rs` owns Label construction; `package.rs`
owns rule requirement retention; and the two existing test modules own their
current recursive/fixture literals. `REPLAN` before an eighth Rust file or any
cap breach.

## Proof and validation

Prove:

- a selected registry module whose root apparent name differs from its
  module-local self-name retains `rules_rust -> dep+`; this discriminates real
  mapping from spelling inference;
- a recursively imported definition evaluates the exact line-427 expression,
  freezes `current_rust_analyzer_toolchain`, does not execute its implementation
  and retains exactly
  `@@dep+//rust/rust_analyzer:toolchain_type` as its one requirement;
- direct and imported Label calls select their own defining module identities,
  while mapping changes alter manifest equality/fingerprint;
- an absent or duplicate/conflicting apparent mapping rejects before rule
  freeze, and existing selected-mapping producer conflict tests remain green;
- canonical `str(Label(...))` is accepted by the rule converter, while a raw
  apparent string in `rule(toolchains = ...)` and non-direct labels remain
  rejected; and
- the accepted fixed aspect, first rust-analyzer rule schema, bounded Label,
  recursive Bzl lifetime and toolchain requirement tests remain green.

Run serially: focused selected-route, recursive manifest/Label and rule-
toolchain tests; `cargo test -p slug_bzlmod_v2`; full
`cargo test -p slug_loading_v2`; `cargo fmt --all -- --check`;
`cargo check -p slug_core_v2 --locked`;
`cargo build -p slug_cli_v2 --locked`; `git diff --check`; and
`scripts/v2_archive_status.sh`. Rebuild the CLI before any binary smoke and
clean stale `slugd` before/after daemon-sensitive tests. Pinned source and the
accepted rules_rust archive suffice; do not run Bazel or add an oracle fixture.

## Compatibility and STOP

- **Exact:** the fixed apparent-self Label resolution from the innermost
  selected-registry defining module, canonical string handoff, one mandatory
  direct toolchain requirement, recursive freeze and producer export identity.
- **Slug-native:** Arc-backed module mapping retention, complete-mapping
  over-invalidation, fingerprint framing and nonrequired diagnostics.
- **Unsupported/deferred:** direct-local/generated/built-in/root mapping
  projection, wider explicit Label forms and APIs, raw apparent/Label/optional
  or duplicate toolchain inputs, target invocation, `ctx.toolchains`,
  toolchain registration/resolution/selection, configured dependencies,
  analysis/actions, `_rust_analyzer_detect_sysroot_impl`, aspect application,
  M8/M7B and exact output bytes.

STOP on guessed aliases, filesystem or Bzlmod lookup from a builtin, a second
mapping owner, target invocation, `ctx.toolchains`, analysis changes, new DICE
computes or locks, Zabel code/behavior adoption, Java/JVM work,
fixture/network/dependency drift, public rules_rust success claims or any cap
breach. `REPLAN` before widening.
