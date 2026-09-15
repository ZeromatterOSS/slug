# Current Slug V2 Work Packet

Packet: WP-4-6-7A-r2-execution-group-rule-initializer-combined-r3
Status: rule-initializer checkpoint accepted; effect attribution selected

## Observable result

Complete one atomic stack containing the unaccepted selected-request/output-
conflict R2 candidate, named and automatic execution groups, the `attr.label`
computed-default and configurable-alias prerequisites, and the rule-initializer
execution now demanded by the authentic configured CLI fixture. Pass the real
CLI consumer gates and replay unchanged F3. Nothing in this stack may reach
`main` separately.

The preserved branch is `integration/selected-request-output-conflict-r2`.
Review checkpoints are R2 `f3c90ea46`, groups `7a2a149af`, compact automatic
identity `7b2fccb76`, qualifiers `498ea2f49`, computed defaults `dab5cb5ea`, and
configurable aliases `fe57809f6`. These remain local, unaccepted preservation
points. The alias checkpoint passes its exact proofs, full loading (563 active
plus one ignored), and full analysis (168/168). Supervised F3 selected one test,
kept valid telemetry and cleanup, and advanced in 10.26 seconds to rules_java
`@@rules_java+//toolchains:BUILD:365` -> `java/toolchains/java_toolchain.bzl:27`,
where `_java_toolchain(**attrs)` reaches the retained initializer guard.

## Compatibility and source authority

Exact for the selected protocol: before final target coercion, each explicitly
supplied, non-`None`, public Starlark-defined attribute is copied and lifted
against its declared type in schema order, then passed with `name`. A raw BUILD
label therefore becomes a typed Label in target-package context. Only names in
the private `_legacy_any_type_attrs` string-list default may bypass this input
type check and reach the initializer as a scalar, list/tuple or dictionary.

The initializer executes once. A whole-function `None` result preserves the
original arguments. A string-keyed dictionary merges returned known public
attributes. Returned `name` must be unchanged. A returned attribute value of
`None` removes that assignment before default installation; it therefore gains
ordinary Default provenance, rather than Explicit provenance. Other returned
values are coerced against the declaration: ordinary label strings and
`Label()` values use initializer-definition context, while output strings use
the target package. `print()` uses existing attempt capture.

The selected rules_java 9.1.0 initializer is
`java/common/rules/java_toolchain.bzl:253-265`, SHA-256
`5ad6511cdef925246961c7e7a9039475c192371fedbf909c63cf92334779e875`.
It checks seven legacy names, replacing only runtime lists, then returns
`kwargs`. `toolchains/default_java_toolchain.bzl:149-178`, SHA-256
`6f963992c933e6cbc48f0c64f3349484422ee06f01830473ea802731b874deea`,
authentically supplies `genclass`, `header_compiler`,
`header_compiler_direct`, `ijar`, `javabuilder` and `singlejar` as scalar typed
Labels and omits `deps_checker`; all six pass through unchanged. Synthetic
evidence separately proves marker-authorized singleton and empty-list rewriting.
No consumer-specific rule path is added.

Primary authority is pinned Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkRuleClassFunctions.java:1516-1526,1765-1881` derives the legacy-any-
type exception from `$legacy_any_type_attrs`, copies and normally lifts explicit
arguments against declarations, removes loading context, invokes child-to-
parent, accepts `None` or a string dict, preserves `name`, merges results and
guards private/native attributes.
`StarlarkRuleClassFunctionsTest.java` covers basic merge, name preservation,
omitted/`None` inputs, collection shapes, return types, attribute guards and
callback errors.

Slug-native restrictions: Slug derives the legacy exception directly from its
retained `_legacy_any_type_attrs` schema/default instead of adding another
field. A nonlisted type mismatch fails before the callback. Slug has no rule-
parent chain, so one initializer is the complete selected chain. Input
selectors, callables, providers and other evaluator-owned shapes and returned
configurable expressions are rejected. Private and built-in results are
rejected because no Bazel builtins allowlist is added.
`native.package_relative_label()` inside initializers, parent chains,
arbitrary-value mutation, private builtins, selector copying, other initializer
surfaces and configured Java behavior remain deferred. Existing later guards
keep precedence after final arguments. No fallback is added.

## Owner, order, and lifecycle

`FrozenRuleDefinition.initializer` remains the sole retained owner. Its frozen
value, definition source and recursive source table preserve identity through
ordinary/Bzlmod re-export. `FrozenRuleDefinition::invoke` is the sole consumer
and calls it before deferred/unknown checks, coercion, outputs or recorder
mutation. Callback inputs/results are attempt scratch. Only the ordinary final
`AttributeValue` slice enters `PackageRecorder` and package equality.

Existing DICE source/manifest dependencies own invalidation. Initializer source
A/B/A recomputes the same package key and restores the exact package. Callback
errors publish no target, generated output or partial package. The callback is
synchronous and adds no cancellation boundary; package-level cancellation and
same-key retry remain inherited from exact selector
`observed_root_package_cancellation_publishes_no_parent_and_recovers`. No
initializer-specific cancellation claim is added. No evaluator value enters
retained state, a key, cache, registry, interner or lock.

A non-`None` returned public value has `AttributeProvenance::Explicit` and
follows normal allowed-value, dependency and generated-output processing.
Returned `None` removes original explicit assignment and installs the declared
default with Default provenance (or the existing missing-mandatory result).
Whole-function `None` and an omitted dictionary key leave original values
unchanged.

## Scope, caps, and stops

Implementation may edit only `app/slug_loading_v2/src/package.rs`,
`app/slug_loading_v2/src/host_package_load_tests.rs`, and scheduling/status
sections in the canonical plan, this manifest, Stage 4, bootstrap readiness and
the configured CLI fixture ledger. No manifest, fixture bytes, key, analysis,
query, server, CLI production or CLI test may change. `package.rs` remains
cohesive because frozen callback, schema, BUILD values, coercion, Bzl identity
and recorder meet in this invocation.

From `fe57809f6`, allow 250 gross production and 300 gross proof Rust lines,
550 total, without deletion credit. The formatted owner-local draft needs
229 production and 300 proof lines. Production is 59 beyond the design estimate
for finite deep copying, legacy-marker admission, two label contexts and reset-
aware merge. The additional 50 proof lines isolate the callback from package
state, execute the exact Java marker through a typed-Label Bzl wrapper, make the
external re-export terminal, count callback events and discriminate computed-
default reset. No new owner or behavior category is added. Preserve inherited
caps from `f3c90ea46`:
2,300 production, 3,600 proof and 5,900 gross Rust additions. Physical caps are
12,250 for `package.rs` and 39,000 for `host_package_load_tests.rs`. Replan for
another owner or production file, retained state, DICE/cache/lock/request
change, parent chain, private/builtin write, selector/evaluator-value copying,
fixture change, cap breach or a new authentic prerequisite.

## Discriminating evidence

Add and preflight exact selectors:

- `rule_initializer_reexport_invokes_copies_merges_and_restores`: ordinary and
  Bzlmod re-export, exact kwargs filtering/order, a raw BUILD label lifted to a
  target-package typed Label, definition-relative returned string/`Label()`,
  target-package-relative returned output, untouched explicit input, absent
  builtin/private/default/`None`, print capture, copied-list isolation, returned
  `None` resetting an explicit value to a nonempty declared default, whole-call
  `None` preserving originals, and one-DICE initializer A/B/A restoration;
- `rule_initializer_failures_publish_no_target_or_output`: earlier declaration
  plus callback failure, non-dict result, non-string key, renamed target,
  unknown/private/builtin result, selector/unsupported input, invalid returned
  type, and a non-marker label attribute passed a list; each publishes no
  package. A no-initializer rule is the control;
- `java_toolchain_initializer_passes_authentic_labels_and_legacy_lists`: the
  six authentic scalar Labels pass unchanged and absent `deps_checker` stays
  absent. A synthetic `_legacy_any_type_attrs` rule separately proves a listed
  singleton list becomes a label, a listed empty list becomes unset/default,
  and a nonlisted list fails before callback. Analysis remains lazy.

Adapt the existing guard proof rather than retain a contradictory failure.
Tests requiring parent classes, builtins allowlisting, arbitrary values or Java
analysis are skipped as unsupported. No fixture is added.

## Validation and atomic acceptance

Use nightly `2025-09-14-x86_64-unknown-linux-gnu`, 60-second compilation and
12-second/15-second test limits. Run format/diff checks, exact loading tests,
full loading unit and integration harnesses, exact inherited package-cancellation
recovery, query compile coverage, unchanged
ten exact CLI selectors, unchanged supervised F3, plan status and all caps.

Final independent review verifies filtering/order, copying, lexical labels,
merge/coercion, atomicity, A/B/A, scope and caps. If F3 finds another semantic
guard, preserve this checkpoint and replan it. Only after every inherited joint
gate and final review may the complete stack merge to `main` and push to
`git@github.com:ZeromatterOSS/slug.git`.

The corrected checkpoint passes all four exact selectors, full loading (565
active plus one ignored and all eight integration harnesses), and query compile
coverage. Independent rereview returned `ACCEPT` for the Bzl-only evaluator,
copy/filter/merge semantics, authentic Java path, exact events, computed reset,
atomic failure and A/B/A. Live additions are 229 production, 300 proof and 529
total; physical sizes are 12,221 and 38,896 lines. The first unchanged CLI
selector advances beyond the initializer and stops during fixture setup at
registration row 8 for `rules_java++toolchains+local_jdk` with
`[diagnostic incomplete: Effect]` after 10.59 seconds. This selects a diagnostic
attribution replan; it does not accept the combined stack or authorize F3.

## Immediate predecessor

`fe57809f6` preserves independently accepted configurable native aliases. It
retains typed expressions, resolves them in Target/Exec configuration, computes
selected/condition nodes in one guarded unioning join, publishes one configured
edge and projects branch/condition query edges. It remains unaccepted because
F3 selected the initializer prerequisite above.
