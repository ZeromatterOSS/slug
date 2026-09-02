# Current Slug V2 Packet

Packet: WP-4-7A-analysis-test-transition-loading-declaration-design-r1

Milestone: M7A bootstrap-critical generic Starlark/loading closure. Implement
the complete bounded Bazel 9.2 loading declaration for the BZL-only
`analysis_test_transition(settings = ...)` global without claiming configured
analysis-test execution.

Status: design `ACCEPTED`. The complete source/owner audit is accepted. Initial
review identified and the packet corrected two bounded source-contract defects:
native option existence belongs to configured analysis, and identical raw
dictionary keys cannot reach transition duplicate validation. A focused source
check also preserves the first-phase absolute-label requirement while varying
only analysis-test native-option policy. Focused rereview returns `ACCEPT`;
Rust is authorized only within this packet's deliberately narrow publication
boundary, retained-value lifetime, proof, allowlist, caps and stops.

Immediate predecessor
`WP-4-7A-native-builtin-label-like-parameter-category-design-r1` is terminally
accepted in `36ee4f124`. Its authentic rules_rust replay clears native
String-or-Label conversion and stops while loading
`@@bazel_skylib+//lib:unittest.bzl`, whose function body refers to the missing
predeclared `analysis_test_transition` name. The reference is resolved when the
module is compiled even though the function is not invoked.

## Audit result and upstream ownership

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` owns this category:

- `ConfigGlobalLibraryApi.java` declares one top-level BZL global with exactly
  one required named-only `settings` argument;
- `ConfigGlobalLibrary.java` requires a BZL module context, casts dictionary
  keys to strings while retaining arbitrary Starlark values, validates each
  key as a transition output, permits experimental and incompatible native
  options, and records the defining module's repository mapping and location;
- `StarlarkDefinedConfigTransition.java` gives the result a fixed patch shape,
  no callback, no inputs or splits, canonical/sorted outputs, literal changed-
  settings identity and repr `<analysis_test_transition object>`;
- `StarlarkAttrModule.java` admits the object at dependency-attribute `cfg`
  positions and marks the descriptor `HAS_ANALYSIS_TEST_TRANSITION` rather
  than `HAS_STARLARK_DEFINED_TRANSITION`;
- `StarlarkRuleClassFunctions.java` rejects an analysis-test transition on a
  rule not declared with `analysis_test = True` using
  `Only rule definitions with analysis_test=True may have attributes with analysis_test_transition transitions`;
- `StarlarkRuleFunctionsApi.java`, `FunctionTransitionUtil.java`,
  `RuleConfiguredTargetBuilder.java` and `CoreOptions.java` separately own
  configured analysis-test rules: implied test status, action prohibition,
  required `AnalysisTestResultInfo`, the always-distinct analysis-test
  configuration marker, nested-analysis-test rejection and the default 2000
  transitive-label cap; and
- `TestingModuleApi.java` plus `StarlarkTestingModule.java` own the separate
  BUILD-only `testing.analysis_test` target factory. It is not the global
  audited here.

Pinned `StarlarkIntegrationTest` covers successful option application,
same-value transitions remaining configuration-distinct, ordinary-rule
rejection, nested analysis tests and the dependency cap. The integration shell
test covers `allow_analysis_failures`. The audit found no analysis-test-specific
allowlist: Bazel's function-transition allowlist tracks the separate regular
transition property.

## Slug inventory and bounded decision

Slug already has a complete callback-backed regular transition declaration,
attribute attachment and configured execution route. It is the wrong semantic
type for this fixed literal patch. Its callable, closure-source identity,
input/output protocol, split behavior and idempotent configured re-entry must
remain unchanged.

`AttributePropertyFlag::HasAnalysisTestTransition` exists only as catalog text
today. `rule()` has no `analysis_test` parameter. `AnalysisTestResultInfo` and
`testing.analysis_test` deliberately reject construction/invocation until
configured semantics are admitted. Although the configuration registry
contains pinned descriptors for `analysis_testing_deps_limit` and
`evaluating for analysis test`, Slug does not yet own the configured graph
rules that consume them.

Implement one distinct evaluator/frozen declaration value for
`analysis_test_transition`. Register it only in BZL globals. It retains:

1. the original settings dictionary value on its evaluator or frozen module
   heap, including arbitrary Starlark values and original key spellings;
2. canonical, Bazel-ordered `Arc<[TransitionSetting]>` outputs produced by the
   existing transition-setting validator/canonicalizer under an explicit
   analysis-test policy; and
3. the defining `BzlModuleIdentity` needed for label and repository-mapping
   interpretation.

The object has no implementation callable, inputs, source-closure inventory or
final configured transition projection. It must not be converted into
`attrs::TransitionDefinition`.

Extend the transient `AttributeDefinitionGen<Value/FrozenValue>` descriptor to
retain this distinct object through local construction, freeze, import and
dictionary reuse. `set_attribute_cfg` accepts either the existing regular
transition or this analysis-test transition, never both. Macro and subrule
consumers preserve their existing fail-closed transition restrictions.

When `rule()` consumes an attribute descriptor containing the new object, it
must stop before `RuleAttributeSchema`, `FrozenRuleDefinition`, target or
package publication with Bazel's ordinary-rule diagnostic above. This packet
does not add `rule(analysis_test = True)`. Consequently the arbitrary literal
map never becomes package/DICE/configuration semantic state, where pointer
identity or omission would be unsound. A later configured packet must design a
V2-owned structural literal value before lifting this stop.

This generic boundary is sufficient for the motivating bazel_skylib module:
the missing name is resolved while compiling `_make_analysis_test`, but the
constructor and `rule(analysis_test = True)` are reached only if that function
is invoked. Add no bazel_skylib, rules_rust, unittest, toolchain, C++,
`cc_common` or `cc_internal` consumer branch.

## Compatibility classification

Admit as **exact** for the named Bazel 9.2 loading declaration surface:

- presence only in `.bzl` evaluation, including ordinary and Bzlmod-routed BZL
  modules, and absence from BUILD globals;
- exactly one required named-only `settings` dictionary argument;
- string-only keys, arbitrary frozen Starlark values, empty and nonempty maps,
  absolute build-setting labels, repository mapping, and
  syntactically well-formed native `//command_line_option:` names including
  unknown, experimental and incompatible names;
- invalid label syntax, invisible repositories and distinct dictionary keys
  that canonicalize to the same build-setting label rejected through the
  existing transition-output policy and diagnostic order; raw duplicate keys
  remain the Starlark dictionary constructor's boundary;
- canonical Bazel ordering, defining-module identity, value/object freeze and
  repr `<analysis_test_transition object>`;
- use as `cfg` on supported dependency attribute constructors through local,
  frozen and imported attribute descriptors; and
- pre-publication rejection when such a descriptor is consumed by an ordinary
  Slug rule, with the pinned Bazel diagnostic.

Keep **Slug-native**:

- Rust/starlark-rust source spans and incidental type-rendering details outside
  the named exact diagnostics;
- evaluator/frozen heap layout, compact collection choices and BZL module DICE
  ownership; and
- Slug's already-accepted collision-safe canonical repository identity.

Keep **unsupported/deferred**:

- `rule(analysis_test = True)`, analysis-test rule invocation and target
  publication;
- literal patch application, typed option/build-setting conversion, the
  always-distinct analysis-test configuration marker and action-conflict
  behavior;
- `AnalysisTestResultInfo` construction/validation, action prohibition,
  nested-analysis-test prevention and transitive dependency counting/caps;
- BUILD-only `testing.analysis_test`, its dynamic rule definition and
  invocation behavior;
- analysis-test attributes in macros/subrules, configured transition
  preparation/execution, query/cquery/aquery projection and execution; and
- Java object identity, exact impossible-state diagnostics, legacy toolchain
  resolution and all unadmitted ruleset/action breadth.

The exact claim ends before rule-schema or package publication. Do not describe
this packet as complete analysis-test support or configured transition support.

## Identity, revision and memory

The BZL evaluation key and imported-module graph already own source bytes,
repository mapping, imports, evaluation success and frozen heap lifetime.
Constructor success publishes only a frozen module value. Changing settings
source or imported inputs invalidates and recomputes that module through the
existing graph; cancellation or failure publishes no partial descriptor.

Within the accepted boundary, the original dictionary and arbitrary values are
owned by the live/frozen Starlark heap and traced/frozen exactly once. Canonical
outputs use the existing compact `TransitionSetting` plus immutable `Arc`
slice. No copied value tree, repr-derived identity, ordinal, pointer-derived
package identity, map, interner, cache, registry, DICE key, lock, I/O or
fallback is added. The object and descriptors release with module invalidation,
eviction or service shutdown.

The Buck2 utility review selects the already-adopted row 112 frozen-transition
lifetime pattern only: evaluator values freeze with their owning module and
compact canonical settings use shared immutable slices. It explicitly rejects
reuse of the regular callable transition as semantic identity. Because this
packet stops before a retained package/configured representation, Stage 9 gains
no new adopted extraction row; a future lifting packet must record its
structural literal representation before Rust.

## Evidence and proof

Use a pinned-source regression rather than a new oracle fixture. The source
contract is closed and the motivating authentic replay discriminates BZL name
availability. Add focused tests that prove:

- the name exists in ordinary/Bzlmod BZL globals and is absent from BUILD;
- missing, positional, extra, non-dictionary and non-string-key arguments fail;
- empty and mixed arbitrary-value dictionaries construct, freeze, import and
  preserve exact repr, raw value structure, canonical output order and defining
  package/repository mapping;
- build-setting, mapped-repository, native, experimental and incompatible keys
  succeed, including a syntactically valid unknown native name; malformed and
  invisible labels plus canonical-label aliases fail in pinned order, while
  native option existence and value typing remain deferred to configured
  analysis;
- all supported dependency attribute constructors accept the object as `cfg`,
  and a frozen descriptor survives a two-hop imported dictionary union;
- ordinary rule consumption fails with the exact pinned diagnostic before a
  rule definition, target or package can be published;
- regular `transition()` descriptors retain their existing live/frozen/final
  identity and tests;
- macro/subrule consumers and `testing.analysis_test` retain their existing
  unsupported boundaries; and
- the authentic rules_rust replay clears the missing-global stop and selects
  only the next independent generic boundary.

No A/B/A package-semantic claim is allowed for the literal settings map in this
packet because it is intentionally never published into a package. A focused
module-source change/restore test may prove existing BZL invalidation without
claiming configured semantics.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/package.rs`; and
- `app/slug_loading_v2/src/transition.rs`.

Proof Rust may change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_loading_v2/src/testing_bootstrap_tests.rs`; and
- `app/slug_loading_v2/tests/bzl_invalidation.rs`.

Scheduling records may change only the canonical plan, Stage 4 owner, Stage 9
ledger and this manifest. Stage 9 may change only if review determines that an
extraction row is mandatory before implementation; otherwise leave it
untouched.

Caps are 210 gross added production Rust lines, 330 proof lines and 540 total.
No new function may exceed 100 lines.

`package.rs` is over the 2,000-line trigger but remains the cohesive owner of
BZL globals, transition validation, evaluator/frozen attribute descriptors and
rule declaration. Extracting this loading-only object elsewhere would split
the one evaluator lifetime and force new public plumbing. Do not move unrelated
definitions. `transition.rs` remains the sole setting parser and canonicalizer;
generalize it with a closed regular-versus-analysis-test policy so absolute-
label behavior does not change while analysis-test outputs permit all
syntactically valid native names and regular option rejection remains exact.
Proof must use existing evaluator and host-load scaffolding; do not copy
bazel_skylib or create a fixture tree. No benchmark is required: this adds one
construction-time compact slice and no configured hot path.

## Validation and stops

Run serially:

- focused constructor/global-placement/signature/validation/repr/freeze/import,
  attribute-consumer and ordinary-rule rejection tests;
- existing regular-transition declaration/attachment/loading tests;
- focused BZL invalidation only if that proof is added;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration test;
- `cargo test -p slug_bzlmod_v2 --lib -q`;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after;
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  verification.

Return `REPLAN` before or during Rust if:

- the literal map reaches `RuleAttributeSchema`, `LoadedPackage`,
  `attrs::TransitionDefinition`, a configured dependency or any DICE value;
- arbitrary literal values are stringified, recursively copied, assigned an
  ordinal, compared by pointer, omitted from a semantic equality domain or
  routed through the regular transition callback;
- `rule(analysis_test = True)`, `testing.analysis_test`,
  `AnalysisTestResultInfo`, action restrictions, nested tests, dependency caps
  or configured patch application become necessary to pass an admitted proof;
- BUILD receives the global, ordinary regular-transition behavior changes, or
  macro/subrule restrictions are widened;
- a second transition parser/validator, native-option existence lookup,
  repository mapping, DICE key, cache,
  interner, registry, lock, I/O, fallback or consumer special case appears;
- a new oracle fixture is necessary without first resolving the fixture-growth
  checkpoint; or
- production/proof caps, file allowlists or the bounded `package.rs` cohesion
  decision are exceeded.
