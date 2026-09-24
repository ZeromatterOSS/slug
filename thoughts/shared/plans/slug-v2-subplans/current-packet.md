# Current Slug V2 Work Packet

Packet: WP-7-122-m7a-build-genrule-loading-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the authenticated protobuf v33.4 BUILD's one bare `genrule` at
`@@protobuf+//:BUILD.bazel:654-661` as a Loading declaration. The rebuilt
authentic `slug build //:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must
clear the missing-`genrule` stop; any later failure is separate. Exact Bazel
9.2 slice: BUILD-only global, selected `name`, `srcs`, `outs`, `cmd` and
`testonly` binding; bool or integer 0/1 conversion and `None` inheritance;
package default and explicit override; canonical labels, output owner and
generated-file publication. Slug-native: compact structural package identity
and diagnostics. Unsupported/deferred: arbitrary genrule attributes or
selectors, query/configured genrule projections, Make variables, actions,
execution and exact Bazel configuration/output identity. An admitted Loading
declaration does not claim a configured or executable genrule.

## Source, owner and decision

Pinned Bazel 9.2 `BazelGenRuleRule.java:47-55` registers the rule,
`GenRuleBaseRule.java:66-113` declares selected attributes,
`BazelStarlarkEnvironment.java:245-264,275-302` distinguishes `.bzl`
`native` members from bare BUILD globals, `BaseRuleClasses.java:66-74,264-267`
provides inherited `testonly` with package default, and `Type.java:409-433`
converts non-None bool or integer 0/1 only. The ignored-RC/offline Bazel 9.2
oracle
`/tmp/slug-wp822-genrule-9p-oracle` proves the selected bare call loads,
publishes a generated output and builds; `testonly=1/0`, omitted package
default, explicit `None` inheriting a later default, and explicit zero
overriding a true package default discriminate the effective bit. Invalid
`testonly=2`, string, malformed `srcs`/`cmd` fail. The authentic protobuf
source uses `testonly=1`, one src, one output and a
plain command; the following loaded `proto_library` consumes the output.
No remote service, BuildBuddy credential or NativeLink is required by this
Loading packet.

Slug already owns imported `native.genrule` through `PackageRecorder::genrule`
and one `GenruleDeclaration` retained inside `PackageTargetKind`; it has
generated-file publication and explicit Query/Analysis rejection. Add a
BUILD-only callable to BUILD globals, reusing the recorder and coercion;
do not place it in shared `package_globals` or expose a bare `.bzl` symbol.
Extend the one declaration with a one-byte `Option<bool> testonly` staging
value: an explicit bool/0/1 binds `Some`, omission or Starlark `None` stages
`None`, and package finalization replaces every `None` with
`Some(final package default)` before
publication. Every retained genrule therefore has `Some(effective bool)`;
the optional state does not survive as an unresolved inherited value. This
also honors a `package()` declaration after a rule. No second
attribute map, rule schema, synthetic query/configured carrier, DICE key,
lock, cache or fallback. The immutable package value's existing structural
equality and observed BUILD/loaded-source fingerprints own invalidation and
A/B/A restoration; the evaluator and recorder remain attempt scratch and
failed attempts publish no package. Existing `CompactString`, canonical
labels, `Arc` slices and `Allocative` remain the compact representation;
Stage 9's imported-genrule no-extraction decision applies. Although
`package.rs` is large, its existing global binding and recorder are the
single natural owner; splitting this one cold callable would add a second
semantic path. No performance claim or benchmark for this cold path.

## Evidence and validation

Before production changes, turn the protected bare-BUILD negative into a
selected source-shaped test that fails at the missing global on the base.
Assert canonical src/output and generated owner, exact command and effective
`testonly` true; explicit 0, omitted/None true package default (including a
later `package()` call), and same-DICE default 0/1/0 and explicit 1/0/1
restoration must discriminate retained equality/invalidation. Keep
`.bzl` bare-global absence and invalid 2, string, malformed input,
unsupported keyword and output-conflict negatives. Existing imported
`native.genrule` positives/negatives and Query/Analysis fail-closed tests
remain protected. Do not add a Stage 1 fixture; the temporary oracle is
discriminating source evidence. Reuse accepted external-package loading and
generated-owner tests.

Resolve pinned-nightly Cargo. Compile selected Loading and directly affected
Query/Analysis test binaries with JSON `--no-run`; batch exact nonignored
preflight selectors and run focused/protected tests serially. Direct Loading,
Query, Analysis, Core and CLI compile and CLI rebuild precede one normalized
`PATH` authentic smoke. That approximately 35-second smoke is necessary to
prove transitive closure advancement that focused tests cannot establish.
Check/clean `slugd` before and after. Rust format, diff, plan and archive
checks are required. Independent DESIGN and FINAL review cover semantic
identity, package default, BUILD-only scope and fail-closed consumers.

## Scope and stops

Base clean main `85c44c1f6`. Allowlist: this manifest, canonical Current,
compact Stage 4/9 owner notes after acceptance; production
`app/slug_loading_v2/src/package.rs`; selected tests in
`app/slug_loading_v2/tests/build_file_loading.rs`, with a Query test literal
update only if required by the retained-field extension. A focused Loading
same-DICE test may live in existing host tests if the integration helper
cannot discriminate restoration. Rough growth 50 production/85 proof lines
is a review trigger, not permission to widen configured behavior. REPLAN if
the oracle contradicts the selected binding, a package default cannot be
retained structurally, `.bzl` isolation fails, or Query/Analysis cannot stay
fail-closed. A later authentic error cannot expand this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the one-declaration effective
`testonly` representation and BUILD-only boundary. The selected source-shaped
Loading test compiled on the base, passed exact preflight 1, then failed at
`Variable genrule not found` (exit 101). Final pinned-nightly JSON `--no-run`
compiled Loading, Query and Analysis test binaries; exact preflights selected
5, 2 and 1 nonignored tests respectively, and all eight passed serially in
under 0.1 second combined. Direct Loading/Query/Analysis/Core/CLI `cargo
check` and the required CLI rebuild passed. The normalized-PATH authentic
smoke (exit 2, about 34 seconds) clears protobuf's BUILD `genrule` and stops
separately at `@@rules_license+//rules:license.bzl:124`, where a Starlark rule
rejects `testonly = 0` as a non-bool. Pre/post checks found no `slugd`.
Rust format, diff, plan and archive checks pass; no remote transport or
BuildBuddy credentials were used. The 116 touched production and 148 proof
lines exceed the rough 50/85-line trigger because the shared helper moves
existing native coercion and the proofs separately cover source shape,
late defaults, structural A/B/A and protected failures; final review accepted
that proportionality. Packet wall time from the accepted predecessor through
final review was about 25 minutes; Cargo preparation and the authentic smoke
dominated test time.

## Immediate predecessor

WP821 accepted at `85c44c1f6`: the typed proto-compiler Loading declaration
and configured guard clear the protobuf `.bzl:81` stop; the rebuilt authentic
smoke reaches this separate bare BUILD `genrule` stop. Five focused tests,
direct checks, CLI rebuild, plan checks and independent reviews passed.
