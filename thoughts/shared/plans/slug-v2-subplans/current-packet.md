# Current Slug V2 Work Packet

Packet: WP-7-109-m7a-frozen-bzl-select-attribute-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit a top-level loaded `.bzl` `select()` value, including the selected
two-part `select(...) + select(...)` form, as an explicit configurable
Starlark-rule BUILD attribute. Authenticated rules_kotlin 1.9.6 defines
`_DEBUG_SELECT` in `kotlin/internal/toolchains.bzl:285-298` with ordered
trace and timings string-list selectors, each defaulting to `[]`, then
passes it as `_kt_toolchain(debug = _DEBUG_SELECT)` at `:320-325`;
`debug` is declared `attr.string_list` at `:157-163`. The rebuilt
normalized-PATH authentic `slug build //:probe` must pass the current
`frozen select values are not valid BUILD attribute values` Loading stop.
A later independently typed stop is acceptable; Kotlin analysis/actions,
target success and M7A close are not claimed.

Exact Bazel 9.2 slice: for an ordinary declared configurable `string_list`
attribute, the selected loaded `.bzl` selector list with non-`None`
list-of-string branches freezes and crosses into the caller BUILD without
losing ordered concatenation parts, resolved condition Label identity and
order, or the empty default branch. The same rule intake accepts an
equivalent fresh BUILD-created selector. Nonconfigurable attributes and
invalid branch values retain their existing errors. Slug-native: the
existing raw/canonical condition intake representation, structural
`CoercedAttributeValue::{Selector, Concatenation}` values and their
package/Bzl-source DICE equality;
no exact Bazel configuration, `bazel-out` or numeric identity is claimed.
Unsupported/deferred: configured branch choice, rule implementation/toolchain
analysis, `None` or nested-selector branches in a frozen selector, frozen
selectors for kinds other than the selected `string_list`, exact invalid
diagnostics and downstream Kotlin.

## Source, owner and decision

Pinned Bazel 9.2 `SelectorList.java:90-115,123-150,228-287` allows
`select()` in BUILD and `.bzl` and preserves ordered selector-list elements;
`BuildType.java:225-246,263-301,321-340` converts them for configurable
declared attributes and rejects nonconfigurable attributes without a
frozen-origin distinction; `SelectTest.java:81-115,138-146` exercises
concatenation and type failures. The authenticated Kotlin source supplies
the selected two-part shape and an independent fresh inline boolean selector
in the same `_kt_toolchain` call. An offline Bazel 9.2 scratch oracle at
`/tmp/slug-wp809-frozen-select-x7jbjg`, run with `--ignore_all_rc_files`,
downloads disabled and two fresh output bases, passes a frozen two-part
`.bzl` global to `attr.string_list` and matches a fresh BUILD-select control
in all four `--define` combinations: empty, `trace`, `timings`, and
`trace,timings` output bytes. A third fresh output base preserves the
both-flags positive while a top-level frozen selector passed as built-in
`tags` fails with `attribute "tags" is not configurable`; `tags` reaches
Slug's shared coercer as a nonconfigurable `string_list`. Bazel does not
permit `attr.string_list(configurable = False)` declarations, so that would
not be a valid negative. Scratch stays outside this checkout and proves
Bazel configured outputs only, not Slug configured parity.

`app/slug_loading_v2/src/package.rs::coerce_starlark_value` is the sole
attribute intake owner. It already detects `SelectorValueGen<Value>` and
`SelectorValueGen<FrozenValue>`, rejects the latter before any branch
conversion, then recursively coerces live parts to the declared kind and
builds ordered structural `Selector`/`Concatenation` values. Reuse this
same branch/default/condition/prefix/suffix conversion for both variants,
converting frozen member handles to evaluator `Value` through the existing
`ValueLike::to_value` route. A generic borrowed-parts helper is preferred
over allocating an intermediate thawed selector. Gate the frozen variant
to `AttributeKind::StringList` before calling it; other frozen kinds retain
their unsupported error while fresh values keep their accepted behavior.
For the frozen path, reject a nested selector branch, prefix or suffix before
recursive coercion; Bazel requires each selected branch to convert directly
to the declared string-list type. The live path retains its prior behavior.
Preserve the early nonconfigurable check and all recursive type/allowed-value
checks. Do not add a separate frozen coercer, mutable global selector cache, retained
representation, new DICE key, lock or analysis-time branch choice. Existing
loaded-Bzl manifests, package source/route observations and structural
attribute values own source changes, errors, replay and A/B/A restoration.
The large `package.rs` remains cohesive because this is one variant of its
existing select attribute intake, not a new subsystem.

## Evidence and validation

Add a focused Loading regression beside current select-attribute tests:
freeze a `.bzl` top-level two-part string-list selector, import it into a
BUILD rule call, and assert ordered structural parts, resolved condition
labels, empty defaults and list-of-string branch values. Include the fresh
BUILD-created selector control, a frozen `tags` nonconfigurable built-in
negative, wrong-typed and nested-selector frozen branch negatives, and a
frozen other-kind unsupported control, plus a same-Dice Bzl-source A/B/A
restoration test.
Reuse existing selector and typed-key regressions;
the structural positives use the root Host package loader because the
lightweight local-path repository-package post-validator separately rejects
Starlark-rule selector-key reachable labels, while the authentic external
smoke tests the selected Kotlin route. Do not widen that post-validator in
this packet. Do not add a Stage 1 fixture unless the offline oracle reveals
a concrete coverage gap. Compile Loading library tests with pinned Cargo JSON
`--no-run`, preflight exact nonignored selectors, run selected tests serially,
rebuild `slug_cli_v2` before authentic smoke, and check/clean `slugd`
before and after. Run format, archive, diff and plan-status checks and obtain
independent DESIGN and FINAL reviews. No broad suite or remote-service gate
is selected; no `~/.bazelrc` contents, BuildBuddy credentials, NativeLink or
remote transport enter this Loading packet.

## Scope and stops

Base clean main `5fe759ed5`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 select owner contract after acceptance; production
`app/slug_loading_v2/src/package.rs`; focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs` and only a necessary
local selector unit in `package.rs`. Moving the existing select conversion
into a generic helper and roughly 150 proof lines are review triggers, not
permission ceilings. REPLAN if Bazel rejects the selected frozen `.bzl`
selector, generic value conversion cannot preserve live/frozen branch
semantics, structural attribute identity loses an admitted source input, or
the authentic stop cannot clear without configured Kotlin behavior.

## Immediate predecessor

WP808 accepted at `5fe759ed5`: zero-argument `native.package_name()` in an
ordinary loaded `.bzl` macro returns the caller BUILD package after loaded
source authentication. Five focused tests and the rebuilt authentic smoke
cleared its prior stop and reached this independently typed frozen-select
attribute error at rules_kotlin toolchain registration row 55. M7A remains
partial.

## Acceptance receipt

Independent corrected DESIGN and FINAL reviews returned ACCEPT. The offline
Bazel 9.2 scratch passed frozen/fresh two-part selector equality for four
configured outputs, replayed the empty/both cases in a second fresh base,
and confirmed frozen `tags` nonconfigurability and the both-flags positive
in a third base. Pinned-nightly Loading JSON `--no-run` compiled; exact
preflight verified five nonignored selectors and all five passed serially:
frozen/fresh ordered structural
equality, frozen `tags`/wrong type/other kind/nested negatives, same-Dice
Bzl-source A/B/A restoration and two protected live-selector tests. The
root Host harness proves structural package behavior without changing the
separate local-path repository post-validator. The direct `slug_cli_v2`
rebuild passed, and the normalized-PATH authentic one-shot `slug build
//:probe` cleared the former Kotlin frozen-select stop and failed
independently at rules_swift module-extension `Label()` on registration row
56. No `slugd` remained before or after. Format, archive, diff and
plan-status checks passed. No remote cache, NativeLink or BuildBuddy gate
was selected. `None` branches, other frozen kinds, configured choice,
downstream rules_swift and M7A close remain deferred.
