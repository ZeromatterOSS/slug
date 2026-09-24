# Current Slug V2 Work Packet

Packet: WP-7-119-m7a-label-empty-aspects-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit explicit `attr.label(aspects=[])` during Loading as the same
zero-attached-aspect declaration as omitted `aspects`. The rebuilt authentic
`slug build //:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must clear the
rules_python Label-builder unexpected-keyword stop at
`attr_builders.bzl:615/89`; any later stop is separate. That stack frame
identifies a builder family, not one unique target attribute.

Exact Bazel 9.2 slice: explicit empty list and omission attach no aspect to
the scalar label dependency. Slug-native: diagnostics and existing structural
Loading/schema identity. Unsupported/deferred: nonempty attached scalar-label
aspects and configured propagation, empty tuple, other attr-family aspects,
and exact Bazel configuration/output identity. Nonempty input must fail closed,
not be accepted then discarded; direct `aspects=None` and wrong types remain
errors.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkAttrModuleApi.java:439-445` declares a sequence of aspects defaulting
to `[]`; `StarlarkAttrModule.java:717-773` forwards it through the shared
factory, whose `:504-509` loop attaches one per element. Offline ignored-RC
oracle `/tmp/slug-wp819-label-aspects-1gwkgJ` accepts omitted and `[]` with
identical no-aspect configured output; singleton and two distinct aspects
produce different nonempty output. Direct `None`, integer and string element
fail at declaration. All runs disabled downloads, isolated output bases and
left no scratch Bazel server. This packet claims only literal empty-list
behavior, not nonempty propagation. Authenticated rules_python 1.7.0
`python/private/attr_builders.bzl:598-615` supplies default `[]` through
`builders_util.bzl:86-90`, then forwards it at `attr_builders.bzl:86-89`.
Inspected production Label constructors omit aspects and do not append; the
builder test `tests/builders/attr_builders_tests.bzl:169-182` asserts its
empty default.

Slug `app/slug_loading_v2/src/package.rs:7113-7151` does not declare the
named `aspects` argument for `attr.label`, hence rejects the keyword before
the body. Add that argument only there and explicitly validate omitted or
list-typed empty as a no-op. Reject nonempty and malformed input. Do not
reuse the label-list parser and drop its result: singleton input has real
Bazel semantics. `AttributeDefinitionGen.attached_aspect` already initializes
to `None` and the frozen rule schema retains that value; existing scalar
label default/cfg/provider/executable/coercion behavior remains unchanged.
Loaded Bzl source/manifest and frozen rule-schema owners already carry
equality, invalidation, export/replay, Need/error and A/B/A restoration. No
DICE key, retained representation, cache, lock, Analysis or Query change;
the argument is evaluator scratch and no nonempty semantic fact is discarded.

## Evidence and validation

First add a focused Loading regression for omitted and explicit empty list,
including source-shaped `kwargs={'aspects': []}; attr.label(**kwargs)`, and
assert `attached_aspect.is_none()` in the frozen schema. Demonstrate the
pre-fix unexpected-keyword failure. Direct `None`, scalar, wrong-member list
and exported singleton must remain errors. Protect existing label default,
mapping and dependency-schema/aspect validation tests. Reuse the offline
oracle; no Stage 1 fixture growth or configured aspect claim.

Resolve pinned-nightly Cargo; compile selected Loading lib tests with JSON
`--no-run`, exact nonignored selector preflight and serial focused/protected
execution. Direct Core/CLI compile and `cargo build -p slug_cli_v2` precede
one normalized-`PATH` authentic smoke. That roughly 30-second smoke is
necessary to establish the transitive rules_python closure advances; the
focused tests cannot establish it. Check/clean `slugd` before/after. Run
Rust format, diff, plan and archive checks. Independent DESIGN and FINAL
reviews cover the named public API boundary. No broad suite or remote
transport; never inspect BuildBuddy authentication.

## Scope and stops

Base clean main `069535ebe`. Allowlist: this manifest, canonical Current
paragraph, a compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs` only; tests in
`app/slug_loading_v2/src/host_package_load_tests.rs`. Rough growth 12
production/45 proof lines is a review trigger, not permission to broaden
scalar aspects. `package.rs` is large, but the existing attr method is the
signature and semantic owner; a new abstraction offers no cohesion for an
empty-only check. REPLAN if selected builder input is nonempty, if Bazel
empty differs from omission, or if the frozen schema cannot preserve that
fact. A later error cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the exact empty-only scalar
label API boundary. The selected test first failed on unmodified production
with the expected `aspects` unexpected-keyword diagnostic (exit 101). After
the named parameter and early guard, omission, explicit `[]` and
`**{'aspects': []}` all freeze `attached_aspect=None`; direct `None`, scalar,
wrong-member list and exported singleton remain errors. Pinned-nightly JSON
Loading `--lib --no-run` compiled, exact preflight selected five nonignored
tests, and all five passed serially. They protect label defaults/mapping,
dependency schemas and generic aspect rejection. Core/CLI `cargo check` and
CLI rebuild passed. The normalized-`PATH` authentic smoke clears the Label
keyword stop and next fails independently at `config.exec(value)` with an
extra positional argument in rules_python `attr_builders.bzl:307` (exit 2,
about 35 seconds). No `slugd` was present before or after. Rust formatting,
diff, plan and archive checks pass. No remote transport or BuildBuddy
credentials were used. Packet wall time through final review was about 7
minutes; five final tests ran in under 0.1 second combined, with Cargo
preparation and the authentic smoke dominating elapsed time.

## Immediate predecessor

WP818 accepted at `069535ebe`: empty `attr.label_keyed_string_dict` aspects
now freeze no attachment and clear its keyword stop. The authentic build
next reaches this independent scalar Label keyword stop.
