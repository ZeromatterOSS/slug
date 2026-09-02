# Current Slug V2 Packet

Packet: WP-4-7A-native-builtin-label-like-parameter-category-design-r1

Milestone: M7A bootstrap-critical generic Starlark/loading closure. Audit and
freeze the complete admitted native-builtin direct-parameter category in which
Bazel 9.2 accepts either a string spelling or an already-constructed `Label`.

Status: terminally `ACCEPTED`. Initial architecture review returned `REVISE`
on three bounded contract details; R1 corrected the source owners, package-
metadata duplicate contract and ordinary-label grammar. Terminal implementation
review returned one bounded proof-only `REVISE` for nested `None` and exact
wildcard-like target-name assertions. The focused correction rereview returns
`ACCEPT` with production/proof/total additions of 186/315/501 lines.

Immediate predecessor `WP-4-5-7A-repository-source-glob-routing-category-
implementation-r2` is terminally accepted in `bf509cd8b`. Its source-routed
catalog traversal passes complete loading/Bzlmod/query/cross-target/replay
gates. The authentic rules_rust replay clears `GlobUnsupported` and reaches
this independent generic boundary when verbatim `@@bazel_tools//tools/res`
passes a `Label` to native `toolchain(toolchain_type = ...)` while Slug's
hand-written direct adapter requires `str`.

The predecessor recorded an unrelated dirty edit to
`app/slug_loading_v2/src/registration_expansion_tests.rs`; it is absent from
this clean checkout. If that parked edit reappears, verify that its SHA-256 is
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Trigger and learned facts

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` owns this category:

- `BuildType.java` `LabelType.convert` returns an existing `Label` unchanged
  and resolves only strings through the caller's `LabelConverter`;
- the same file's `LABEL_LIST`, `NODEP_LABEL_LIST` and
  `LABEL_KEYED_STRING_DICT` recursively use that scalar conversion and reject
  distinct keys that canonicalize to the same label;
- `PackageArgs.java` `processParam` converts `default_package_metadata` with
  `LABEL_LIST` and rejects duplicate canonical labels before setting defaults;
- `Attribute.java`, `RuleClass.java` and the generic native rule invocation
  path apply those declared types before native target publication;
- `StarlarkNativeModule.java` applies `BuildType.LABEL_LIST` independently to
  `package_group.includes`, while `PackageGroup.java` retains the converted
  labels; and
- `BaseRuleClasses.java`, `Alias.java` (nested `AliasRule`),
  `ConfigRuleClasses.java`,
  `TestSuiteRule.java`, `ConstraintSettingRule.java`,
  `ConstraintValueRule.java`, `PlatformRule.java`, `ToolchainType.java`
  (nested `ToolchainTypeRule`) and `ToolchainRule.java` own the admitted native
  attribute types.

Pinned `BuildTypeTest` covers an already-typed label in a label-keyed string
dictionary, mixed strings/typed labels in nested label lists, package-relative
string conversion and canonicalized-key collision rejection. The accepted
package-context label work in `5f9f9a98a` already owns borrowed string syntax,
mapping, `@//`/`@@//` separation and the single `CanonicalLabel` result. The
accepted repository-source packet supplies a live discriminator for one typed
scalar without making `tools/res` or rules_rust semantic authority.

Slug's generic raw attribute route already matches the upstream architecture.
`RawAttributeValue::Label`, `RawLabelContext::Package::label` and
`coerce_raw_value` preserve an existing `StarlarkLabel`, resolve strings through
`PackageRecorder::dependency_label`, recurse through lists and dictionaries,
and reject canonical label-key collisions. `UnpackVisibility`,
`filegroup.srcs`, `target_settings`, and all label-typed native kwargs already
use that behavior.

The gap is the hand-written direct parameter layer in `package.rs`. The BUILD
global and `native.*` facades duplicate string-only signatures before values
reach the complete generic coercer. The complete changed inventory is:

- `package.default_package_metadata` (`LABEL_LIST`);
- `package_group.includes` (`LABEL_LIST`);
- `alias.actual` (`LABEL`);
- `test_suite.tests` (`LABEL_LIST`);
- `config_setting.flag_values` keys (`LABEL_KEYED_STRING_DICT`) and
  `config_setting.constraint_values` (`LABEL_LIST`);
- `constraint_value.constraint_setting` (`LABEL`);
- `platform.constraint_values` (`LABEL_LIST`); and
- `toolchain.toolchain` (`NODEP_LABEL`), `toolchain.toolchain_type` (`LABEL`),
  `toolchain.exec_compatible_with` (`LABEL_LIST`) and
  `toolchain.target_compatible_with` (`LABEL_LIST`).

`constraint_setting.default_constraint_value` is the positive scalar control:
it already accepts `Label`, string or `None`. `filegroup.srcs`, visibility,
`platform.parents`, `platform.required_settings`,
`platform.allowed_toolchain_types`, `constraint_setting.refines_constraint_value`
and common native label/list attributes are generic-coercer controls rather
than new work. `exports_files.srcs`, `package_group.packages`, licenses, tags,
glob patterns, names and output attributes have distinct path/string/output
grammars and are not in this category.

## Decision and natural ownership

Replace only the inventoried string-only direct adapters with one shared
evaluator-local String-or-Label conversion route. Reuse the existing
`RawAttributeValue`/`RawLabelContext::Package`/`coerce_raw_value` semantics or a
thin private projection over them; do not create another label parser or
mapping policy.

For each direct value:

1. an already-constructed `StarlarkLabel` contributes its existing
   `CanonicalLabel` without display-string conversion or caller-context
   reinterpretation;
2. a string is parsed by `PackageRecorder::dependency_label` in the current
   BUILD package and repository mapping;
3. lists preserve input order unless the already-declared native schema owns
   order-independent canonicalization;
4. label-keyed dictionaries preserve value association and reject different
   raw keys that canonicalize to the same label; and
5. package metadata rejects repeated canonical labels even when distinct raw
   String/Label values produce the collision; and
6. invalid scalar, container, nested element or dictionary-value kinds fail
   before the target or package defaults are published.

All inventoried values use ordinary Bazel attribute-label grammar. Target
names `all`, `all-targets`, `*`, `...` and `sub/...` are valid labels when the
declared type is `LABEL`/`NODEP_LABEL`; remove the current
`native_toolchain_label` lexical target-pattern rejection on these attribute
paths. Preserve recursive and wildcard rejection only in APIs whose input is
actually a target pattern, such as registration or command target-pattern
parsing.

The BUILD package's `PackageRecorder` remains the sole caller-context owner.
The existing `CanonicalLabel` remains the sole semantic value. Existing
`PackageState`, `PackageTargetKind`, `NativeToolchainTarget`,
`ConfigSettingTarget`, native override slots and package defaults retain the
converted values and therefore own package equality and downstream semantic
references. The root/repository package-load DICE keys continue to own source,
mapping, evaluation, equality cutoff and invalidation. Add no DICE key,
projection, side registry, cache, interner, lock, filesystem read or fallback.

Do not stringify a typed label and feed it back through the current BUILD
package. A typed `Label(":typed")` created in a defining `.bzl` package and a
raw `":raw"` string passed by the same macro intentionally retain different
owners: the former stays in the defining package; the latter resolves in the
calling BUILD package.

## Compatibility classification

Admit as **exact** for the named Bazel 9.2 loading surface:

- String and `Label` acceptance for every inventoried scalar, list element and
  label-keyed dictionary key;
- current-package/repository-mapping resolution of strings and identity
  preservation of already-canonical labels;
- ordinary attribute labels whose target names are `all`, `all-targets`, `*`,
  `...` or `sub/...`, without importing command/registration target-pattern
  semantics;
- mixed String/Label collections, input ordering, declared
  order-independent normalization, default/explicit provenance and duplicate
  canonical-key/default-package-metadata rejection; and
- final loading-time native target/package-default values and semantic
  references for the already-admitted native rule classes.

Keep **Slug-native**:

- Rust/starlark-rust type-error wording and source spans, DICE key/value
  layout, compact collection choices and package equality cutoff; and
- the already-accepted collision-safe canonical repository/mapping identity.

Keep **unsupported/deferred**:

- selectors or configurable-expression breadth not already accepted on a
  named native attribute, and all configured matching/toolchain selection;
- unrepresented native rule classes, rule implementation semantics, legacy
  toolchain resolution, registration, execution platforms, actions and
  execution;
- output/path/file grammars, exact impossible-state Java diagnostics and Java
  object identity; and
- rules_rust, rules_cc, `@@bazel_tools//tools/res`, C++, `cc_common` and
  `cc_internal` behavior beyond ordinary consumption of the generic loading
  result.

This packet does not claim that every future native parameter accepts a
`Label`; only the inventoried Bazel `LABEL`, `NODEP_LABEL`, `LABEL_LIST`,
`NODEP_LABEL_LIST` and label-keyed dictionary positions are admitted.

## Revision, equality and memory

No request option or mutable Host observation is added. A package evaluation
converts invocation values completely before publication. Its existing source
and repository-mapping dependencies determine the immutable DICE request;
overlapping requests share only complete package results through existing
keys. Need, cancellation or conversion failure publishes no partial target.

Equivalent string and typed-label inputs that resolve to the same
`CanonicalLabel` may equality-cut off to the same complete `LoadedPackage`.
Changing a raw mapping or typed canonical label must change the retained
package value and downstream semantic references; restoring it must restore
the prior value without stale representation state. No lock is held across a
DICE computation.

Starlark `Value`, temporary raw lists/dictionaries and conversion vectors are
evaluator/phase scratch and drop after invocation. Existing compact strings,
`CanonicalLabel`s and `Arc` slices/maps remain DICE-retained package semantic
memory and release on invalidation/eviction or service shutdown. No command,
service-cache, transfer-owned async or task memory is added. There is no hot-
path or retained-representation change requiring a benchmark or Stage 9/Buck2
extraction update.

## Evidence and proof

Use a pinned-source regression rather than a new persistent oracle fixture.
`BuildType.java` supplies one generic conversion contract, and the accepted
verbatim catalog replay already discriminates the motivating scalar. This
avoids copying a matrix whose rows differ only by RuleClass declaration while
still testing every Slug adapter that bypasses the shared conversion owner. No
fixture-growth checkpoint is triggered.

Add focused proof that:

- direct BUILD globals and `.bzl` `native.*` calls both accept the complete
  inventoried category;
- scalar/list/dictionary rows mix strings and typed Labels and publish the
  exact canonical values, semantic references, order and provenance;
- one cross-package macro keeps a typed defining-package label while resolving
  a raw string in the calling BUILD package;
- typed and equivalent raw spellings compare as the same semantic package
  value, while a different typed owner changes and restores the result;
- canonicalized collisions in `config_setting.flag_values`, invalid scalar,
  list element, dictionary key/value and `None` positions fail before target
  or package-default publication;
- global `package()` and `native.package()` reject equivalent String/Label
  duplicates in `default_package_metadata` without retaining either default;
- String and typed-Label values with target names `all`, `all-targets`, `*`,
  `...` and `sub/...` succeed at the inventoried attribute positions, while
  actual target-pattern APIs retain their existing wildcard/recursive
  rejection;
- generic kwargs, visibility, `constraint_setting.default_constraint_value`,
  output/path/string grammars and ordinary valid string behavior remain
  unchanged; and
- the built-in `tools/res` package clears the typed-parameter stop, then either
  publishes through ordinary loading or reaches a later independently
  classified boundary without a catalog/toolchain/ruleset branch.

The rebuilt authentic rules_rust replay must clear the current
`toolchain_type` Label-versus-string diagnostic and select only the next
generic unsupported category, if any. It is downstream evidence, not
authority for ruleset behavior.

## Terminal implementation outcome

The implementation replaces every inventoried direct String-only adapter with
a thin projection through the existing raw package-context coercer. Typed
labels retain their defining identity, raw strings use the calling BUILD
package, canonical collisions and invalid nested values fail before
publication, and ordinary label target names remain distinct from target-
pattern APIs. No parser, DICE owner, retained representation, I/O, fallback or
consumer special case was added.

Serial validation passes 508 loading library tests with one ignored, every
loading integration binary, 596 Bzlmod tests, 55 query tests, the V2 CLI build,
formatting and diff checks. The archive checker passes its refs and structural
gates and reports only the three longstanding thoughts-path allowlist failures;
the parked registration edit is absent. The isolated authenticated rules_rust
configured-query replay clears the former `toolchain_type` Label-versus-string
diagnostic and next stops at missing predeclared `analysis_test_transition` in
`@@bazel_skylib+//lib:unittest.bzl`. No stale `slugd` remains.

Next, audit the complete Bazel 9.2 predeclared `analysis_test_transition`
category docs-first. Do not add a bazel_skylib/rules_rust consumer branch or
silently widen configured-analysis test semantics.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/package.rs`.

Proof Rust may change only:

- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`;
- `app/slug_loading_v2/tests/build_file_loading.rs`; and
- `app/slug_loading_v2/tests/bzl_invalidation.rs`.

Scheduling records may change only the canonical plan, Stage 4 owner and this
manifest. Caps are 240 gross added production Rust lines, 520 proof lines and
760 total. No new function may exceed 120 lines.

`package.rs` is 10,784 lines and exceeds the 2,000-line trigger. It remains the
cohesive owner because it contains `PackageRecorder`, evaluator-local raw
attribute coercion and both mirrored native invocation facades. The patch may
add one private converter beside the existing raw conversion and alter only
the inventoried adapters; moving the converter away would split evaluator
values from package/mapping ownership. Do not move unrelated declarations,
schema storage, presentation, persistence or transport into this file.

The proof files are large existing owner suites. Add only focused rows using
their current scaffolding; do not create copied package trees or broad
snapshot assertions. This category adds no retained collection and is not a
demonstrated hot path, so no benchmark is required.

## Validation and stops

Run serially:

- focused package/raw-coercion, direct BUILD, `native.*`, mapping and A/B/A
  tests;
- focused built-in catalog repository-load proof;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration test;
- `cargo test -p slug_bzlmod_v2 --lib -q`;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after;
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  SHA-256 verification.

Return `REPLAN` before or during Rust if:

- any inventoried value requires a second parser, stringification/reparse of a
  typed label, direct mapping lookup outside `PackageRecorder`, new DICE owner,
  registry, cache, interner, lock, I/O or fallback;
- exact behavior requires selectors/configured evaluation, output/path/file
  widening or a native rule class outside the declared category;
- one facade cannot share the same semantic conversion without changing its
  BUILD-versus-defining-package ownership;
- a failure can publish a partial target/package default or canonicalized
  dictionary collisions are accepted;
- a `tools/res`, rules_rust, rules_cc, toolchain-selection, C++, `cc_common` or
  `cc_internal` consumer special case appears;
- a new oracle fixture becomes necessary without first resolving the current
  fixture-growth checkpoint count; or
- production/proof caps, file allowlists or the bounded `package.rs` cohesion
  decision are exceeded.

Independent architecture review must return `ACCEPT` before Rust begins.
