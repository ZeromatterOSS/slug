# Current Slug V2 Work Packet

Packet: WP-7-123-m7a-starlark-bool-rule-call-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit Bazel 9.2's direct integer 0/1 rule-call values for declared Boolean
attributes of ordinary Starlark rules. The authenticated rules_license 1.0.0
`_license` call at `@@rules_license+//rules:license.bzl:124-135` passes
`testonly = 0`; the rebuilt authentic `slug build //:probe` in
`/tmp/slug-wp797-authentic-2YQmSt` must clear that stop. Exact slice:
direct scalar `False`/`True`/`0`/`1` binding for inherited `testonly` and
ordinary `attr.bool` values, with 0/1 normalized to the retained Boolean
and explicit provenance; 2, -1 and strings fail. Slug-native structural
Loading identity and diagnostics remain. Unsupported/deferred here:
integer-valued `select()` branches, rule initializers, native-rule Boolean
overrides, `attr.bool(default=0/1)` (which Bazel itself rejects), built-in
`testonly=None` inheritance and late `package()` default timing, configured
effects not already admitted, and exact Bazel configuration/output identity.
This packet does not claim all Boolean conversion contexts.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkRuleClassFunctions.java:159-165` inherits
`BaseRuleClasses.java:264-267` Boolean `testonly`;
`RuleFactory.java:58-105`, `RuleClass.java:2057-2072`,
`AttributeProvider.java:219-280`, `BuildType.java:165-208,226-249` and
`Type.java:409-428` convert rule-call Boolean values from bool or exactly
0/1. `StarlarkAttrModule.java:1078-1091` binds `attr.bool`'s declaration
default as a Java Boolean before rule-call conversion. Authenticated
rules_license `rules/license.bzl:32-66,124-135` defines and calls the private
Starlark rule with `testonly=0`; sibling `rules/package_info.bzl:117` does
likewise. Ignored-RC/offline Bazel 9.2 oracle
`/tmp/slug-wp823-rule-bool-oracle` proves direct 0/1 and booleans for both
inherited `testonly` and ordinary `attr.bool`, with the typed values visible
in query XML and build implementation. Direct 2/string fail; Boolean
constructor defaults of integer 0/1/2 fail earlier. The oracle also proves
None and integer select branches, recorded as distinct deferred contexts
rather than silently claimed.

Slug's `FrozenRuleDefinition::invoke` in `package.rs:8212-8280` routes an
explicit value to `coerce_starlark_value`; the raw integer is then rejected
by the shared `coerce_raw_value(Boolean)` branch. Normalize only direct
scalar integer values at this Starlark rule-call boundary, before shared
raw coercion. Do not widen `coerce_raw_value`: it also handles attribute
definition defaults, selectors and rule initializer values. Keep existing
`CoercedAttributeValue::Boolean`, `AttributeProvenance::Explicit`, frozen
rule/schema and package structural equality. The observed BUILD and loaded
source fingerprints already own DICE invalidation and A/B/A restoration;
no key, lock, cache, representation, sidecar, evaluator borrow or new
memory owner. Native/genrule call paths remain untouched. The large
`package.rs` is the existing rule-call semantic owner; a small branch there
is more cohesive than a separate coercion framework. This cold Loading
conversion has no performance claim or benchmark.

## Evidence and validation

First add a source-shaped loaded `license` wrapper/rule test that fails
on the unmodified branch at `testonly=0`, with a true package default set
before the call so explicit false is discriminating. Add ordinary Boolean
attribute direct 0/1 and existing bool controls; assert retained typed
value/provenance and same-DICE 0/1/0 structural restoration. Reject 2,
-1, string and integer `attr.bool(default=0)` without treating a failure
in the wrong phase as proof. Preserve selected Boolean default, selector,
native override, configured and Query tests; do not add a Stage 1 fixture.
Reuse the offline Bazel oracle.

Resolve pinned-nightly Cargo. Compile selected Loading and direct Query/
Analysis dependent test binaries with JSON `--no-run`, batch exact nonignored
preflight and run focused/protected tests serially. Direct Loading, Query,
Analysis, Core and CLI compile and CLI rebuild precede one normalized-`PATH`
authentic smoke. This about-35-second replay is necessary to prove
transitive source closure advancement; focused tests cannot. Check/clean
`slugd` before and after. Rust format, diff, plan and archive checks, and
independent DESIGN and FINAL review of the context boundary are required.

## Scope and stops

Base clean main `51ded99c1`. Allowlist: this manifest, canonical Current
and compact Stage 4 note after acceptance; production
`app/slug_loading_v2/src/package.rs`; focused tests in
`app/slug_loading_v2/tests/build_file_loading.rs` or existing
`host_package_load_tests.rs`, plus a directly affected test only if needed.
Rough growth 20 production/80 proof lines is a review trigger, not a license
to change Boolean defaults, selectors or configured behavior. REPLAN if
the oracle contradicts direct conversion, normalization cannot be isolated
from constructor/default paths, or semantic equality/negative contexts
cannot be preserved. A later authentic error cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the direct scalar rule-call
boundary. The source-shaped loaded-license test compiled on the clean base,
passed exact preflight 1, then failed (exit 101) at `_license(...testonly=0)`
with `attribute must be a bool`. Final pinned-nightly JSON `--no-run` compiled
Loading, Query and Analysis test binaries; exact preflights selected 4, 1
and 1 nonignored tests, and all six passed serially (Loading under 0.03
seconds, Query under 0.01, Analysis about 0.02). Direct Loading/Query/
Analysis/Core/CLI `cargo check` and required CLI rebuild passed. The
normalized-PATH authentic smoke (exit 2, about 41 seconds) clears the
rules_license Boolean stop and next fails separately at
`@@protobuf+//:BUILD.bazel:356` through rules_cc `cc_binary` because rule
initializer input `copts` contains `select()`. Pre/post checks found no
`slugd`; Rust format, diff, plan and archive checks pass. No remote
transport or BuildBuddy credentials were used. The 23 touched production
and 191 proof lines exceed the rough 20/80-line trigger because source
shape, typed/default/negative contexts and independent same-DICE Boolean
fields need separate proofs; final review accepted proportionality.

An extra older `scalar_and_string_attr_descriptors_retain_typed_values`
test was run but failed on its unchanged optional-label `None` provenance
assertion (expected Explicit, actual Default). The existing ordinary-None
filter yields Default on both base and candidate; the new direct-integer
branch cannot reach that path. This is an unresolved preexisting
inconsistency, not a WP823 pass or regression, and is not an acceptance
gate. Packet wall time from predecessor acceptance through final review was
about 18 minutes; Cargo preparation and the authentic smoke dominated
test time.

## Immediate predecessor

WP822 accepted at `51ded99c1`: one BUILD-only bare genrule Loading
declaration, effective inherited testonly and generated output owner.
Eight focused/protected tests, direct checks and rebuilt authentic smoke
cleared the protobuf BUILD stop and reached this separate rules_license
Boolean coercion failure.
