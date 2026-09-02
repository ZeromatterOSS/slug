# Current Slug V2 Packet

Packet: WP-4-7A-applicable-licenses-loading-alias-design-r1

Milestone: M7A bootstrap-critical generic Starlark/loading closure. Implement
the bounded Bazel 9.2 compatibility aliases
`default_applicable_licenses` and `applicable_licenses` by canonicalizing them
immediately into Slug's accepted `default_package_metadata` package field and
`package_metadata` rule-attribute slot.

Status: category audit and architecture `ACCEPTED`; bounded Rust is authorized.
The exact claim is limited to single BUILD-package declarations and admitted
rule instances. It does not widen the existing package-call, REPO.bazel,
symbolic-macro, rules_license or configured-analysis surfaces.

Immediate predecessor
`WP-4-7A-analysis-test-transition-loading-declaration-design-r1` is terminally
accepted in `3f90b41b5`. Its rebuilt authenticated rules_rust replay clears the
missing BZL global and stops while loading
`@@bazel_skylib+//toolchains/unittest:BUILD` at
`package(default_applicable_licenses = ["//:license"])`.

## Learned facts and research basis

Pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` owns the category:

- `PackageArgs.java` sends both `default_applicable_licenses` and
  `default_package_metadata` through the same `LABEL_LIST` conversion and
  `defaultPackageMetadata` builder field. It rejects canonical-label
  duplicates using the supplied spelling and rejects both spellings together
  with the fixed migration diagnostic.
- `PackageCallable.java` exposes those package arguments only while evaluating
  a BUILD file. `RepoFileGlobals.java` exposes the same `PackageArgs` through
  REPO.bazel's distinct `repo()` call.
- `RuleClass.java` names the stored common attribute `package_metadata` and its
  alternate input spelling `applicable_licenses`.
- `AttributeProvider.java` rewrites the alternate spelling before schema
  lookup for rule instances, ignores `None` after checking that the canonical
  schema exists, converts through the canonical attribute, and stores only the
  canonical slot. If both spellings are supplied, ordered keyword traversal
  makes the last non-`None` value win.
- `BaseRuleClasses.java` gives ordinary native and Starlark rules the
  nonconfigurable, no-configuration `package_metadata` label-list attribute,
  whose computed default reads the package's `defaultPackageMetadata`.
- `StarlarkRuleClassFunctions.java` removes `package_metadata` from dependency-
  resolution and materializer base rules. Platform, constraint-setting and
  constraint-value rule classes likewise remove it.
- `MacroClass.java` checks user keywords against the macro schema before its
  shared `AttributeProvider` population step. A macro that declares
  `package_metadata` therefore still rejects `applicable_licenses`; the alias
  is not a symbolic-macro input surface.
- `RuleClass.isPackageMetadataRule` and `AttributeProvider` suppress the
  package default for Starlark rules defined in the canonical repository named
  `rules_license`, avoiding metadata self-edges.
- `RuleClassTest.testPackageMetadataAlternateName` is the focused pinned-source
  regression: alternate input is converted to a label and observed only under
  the canonical slot. `StarlarkRuleClassFunctionsTest` proves the canonical
  common attribute. The accepted Stage 4 package-license matrix already proves
  package-default propagation for native, Starlark and config-setting rules.

Slug already owns one `PackageState.default_package_metadata` field as
`Arc<[CanonicalLabel]>`, one canonical Starlark builtin schema declaration and
one canonical native schema slot. Filegroup, alias, config_setting, test_suite,
toolchain_type and toolchain own that native slot; constraint_setting,
constraint_value and platform intentionally do not. Both BUILD `package()` and
`native.package()` route through `package_global`, and rule labels already use
the package recorder's canonical repository mapping.

Slug's REPO.bazel evaluator currently validates `repo()` call placement but
deliberately discards all keyword values; its file is marked dormant until a
root package-policy activation packet. This is not a lawful place to add one
metadata exception.

## Decision and compatibility classification

Implement as **exact** within the named Bazel 9.2 loading slice:

1. On one otherwise-admitted BUILD package declaration,
   `default_applicable_licenses` is a true input alias for
   `default_package_metadata` in both `package()` and `native.package()`.
   String and `Label` members, package-relative and mapped labels, order,
   duplicate rejection, explicit empty lists and type errors use the existing
   canonical conversion path. Supplying both spellings fails with Bazel's
   pinned migration diagnostic before package-state mutation.
2. On admitted native and Starlark rule instances,
   `applicable_licenses` resolves to `package_metadata` before schema lookup.
   It is accepted only where that canonical slot exists, is coerced and
   defaulted exactly as that slot, and is stored/query-visible only as
   `package_metadata`.
3. Explicit non-`None` canonical or alternate values override the package
   default; an explicit empty list suppresses it. `None` is omission after the
   canonical schema-existence check. If both spellings occur, the last
   non-`None` value in BUILD keyword order wins.
4. A rule class without `package_metadata` rejects the alternate spelling as a
   missing canonical attribute. Symbolic macros continue to reject the
   alternate spelling even when they declare canonical `package_metadata`.

Keep **Slug-native** only the already-admitted Rust/starlark-rust diagnostic
framing, source spans, evaluator layout, collision-safe canonical repository
identity, and incidental query presentation outside the canonical attribute
name and value.

Keep **unsupported/deferred**:

- more than one `package()`/`native.package()` call in a BUILD file and every
  interaction that relies on Slug's currently broader call multiplicity;
- REPO.bazel `repo()` package-argument retention, merge precedence and
  propagation to repository packages;
- the Bazel `rules_license` repository-name special case and metadata-rule
  default self-edge suppression;
- `applicable_licenses` on symbolic macros, repository rules, tag classes,
  aspects or subrules; rule initializers and initializer-returned aliases;
- dependency-resolution/materializer rules and native rule classes outside
  Slug's admitted catalog;
- configured metadata providers, license policy interpretation, action or
  output identity, query/cquery/aquery semantics beyond the already-loaded
  canonical attribute, and any Skylib/toolchain/ruleset special case; and
- Java/HotSpot object identity, legacy toolchain resolution and exact
  incidental diagnostics not named above.

The package and rule aliases are distinct inputs. Do not infer a second
attribute named `applicable_licenses`, a license provider, or full applicable-
license policy from this packet.

## Natural owner, identity and revision behavior

`PackageRecorder::set_package_defaults` and its `PackageState` remain the sole
producer and retained owner of package metadata defaults.
`FrozenRuleDefinition::invoke` and `coerce_native_overrides` remain the sole
Starlark/native rule-input owners and publish only the existing canonical
attribute values. Add one small name-canonicalization helper shared where
useful; do not retain the original alias spelling.

The BUILD source observation and package-load DICE key already own source
bytes, BZL imports, repository mapping, evaluation success and the final
`LoadedPackage`. An alias/canonical source edit invalidates through that same
graph. Equivalent spellings may equality-cut off at the existing canonical
loaded value; failures publish no partial package. Overlapping requests keep
their existing immutable observations and transaction boundaries. No command
overlay, host fallback or historical filesystem inference is added.

All retained memory is the existing DICE-retained `Arc<[CanonicalLabel]>` or
canonical `AttributeValue`. Keyword maps and the supplied spelling are
evaluation scratch and release when invocation returns. No new enum, string,
map, interner, registry, cache, DICE key, lock, task, I/O or async-transfer
memory is permitted. Cancellation, invalidation, eviction and shutdown remain
those of the existing package-load graph.

This immediate canonicalization changes no retained data structure, hashing,
compact collection, clone path or memory accounting, so the Buck2 hot-path
utility skill is not triggered and Stage 9 gains no extraction row.

## Evidence and proof

Reuse the pinned-source regression and accepted Stage 4 package-metadata
evidence rather than adding an oracle fixture. Add focused loading tests that
prove:

- `package()` and `native.package()` accept the alternate default spelling,
  including String/`Label`, relative-label, explicit-empty and canonical-
  duplicate cases;
- both package spellings together produce the pinned diagnostic before any
  target/package publication;
- canonical and alternate package spellings produce equal canonical loaded
  defaults and equivalent rule projection;
- admitted native and Starlark rules accept `applicable_licenses`, publish only
  `package_metadata`, override defaults, distinguish explicit empty, treat
  `None` as omission, and preserve last-non-`None` behavior when both spellings
  are present;
- constraint_setting, constraint_value and platform reject through the absent
  canonical slot, while symbolic macros keep their pre-provider rejection;
- ordinary canonical `default_package_metadata` and `package_metadata`
  behavior remains unchanged; and
- the rebuilt authentic rules_rust replay clears the Skylib package argument
  and selects only the next independent generic boundary.

`RuleClassTest.testPackageMetadataAlternateName` is adapted as a Rust loading
regression. The configured proto-output and materializer tests are skipped
because those phases/rule families remain unsupported. The accepted package-
license oracle is stronger than adding another fixture for default propagation.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/package.rs`.

Proof Rust may change only:

- `app/slug_loading_v2/tests/build_file_loading.rs`.

Scheduling records may change only the canonical plan, Stage 4 owner and this
manifest. Do not change Stage 9, oracle fixtures, Bazel/Skylib sources, query
production, Bzlmod production or Cargo metadata.

Caps are 90 gross added production Rust lines, 230 proof lines and 320 total.
No new function may exceed 100 lines, and no existing function may grow by
more than 20 lines.

`package.rs` is above the 2,000-line trigger and contains large rule/native
invocation functions. It nevertheless remains the cohesive owner because the
alias must disappear at the existing package, Starlark-rule and native-rule
ingress sites before any semantic value is built. Use small helpers and do not
move unrelated declarations or create a second attribute-normalization module.
The test file is also large; extend the existing package-metadata/direct-label
test neighborhood rather than creating new harness or fixture machinery. No
benchmark is required because retained representation and configured hot paths
do not change.

## Validation and stops

Run serially:

- focused package-default, native-rule, Starlark-rule, absent-schema and macro-
  rejection tests;
- `cargo test -p slug_loading_v2 --test build_file_loading -q`;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration test;
- `cargo test -p slug_bzlmod_v2 --lib -q`;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after; and
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  verification.

Return `REPLAN` before or during Rust if:

- either alias survives in `PackageState`, `RuleAttributeSchema`,
  `NativeRuleAttributes`, `LoadedPackage`, DICE identity or query output;
- a second metadata field/slot, source-spelling bit, registry, cache, DICE key,
  lock, fallback, parser, label converter or consumer special case appears;
- REPO.bazel keyword retention, repeated-package-call enforcement,
  rules_license self-edge suppression, macro alias acceptance, rule
  initializers, materializers or configured license semantics become necessary;
- alias handling changes canonical attribute ordering, dependency edges,
  repository mapping, existing package defaults or unrelated `None` policy;
- a new oracle fixture is necessary without a docs-first provenance decision;
  or
- the file allowlist, growth caps or bounded large-file decision is exceeded.
