# Current Slug V2 Work Packet

Packet: WP-7-67-m7a-compile-output-declarations-r1
Status: accepted

## Outcome and compatibility

Admit paired native compile-output prefix/declaration entrypoints demanded by the
allocator regular-source path. Genuine configured rules obtain regular derived
Files at supplied-label _objs coordinates, owned by the active ctx and rooted in
the validated supplied configuration. WP766 at0a1d7d943 accepted coherent roots,
315 ordinary plus3 fresh NativeLink tests, CLI check and final ACCEPT; receipt in Git.

Exact admitted nonsibling/non-Windows path semantics: purpose suffix selection,
normalized package-contained declaration, regular File kind, supplied-label versus
active-owner distinction and argument types. Slug-native: full configuration/root/
artifact identity, output tokens, observed Rust Host and diagnostics. Deferred:
alternate-configuration roots, sibling/Windows paths, normalized final paths
containing backslash or NUL (existing registry rejection), configuration-value provider
publication, CppCompile/discovery, tree templates, extra compile natives and full
cc_library/toolchain/Run/bootstrap. Never substitute ctx configuration silently.

## Demand and pinned source

Bazel9.2 source8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.java557–576/994–1008; CppHelper.java69–72/85–93;
AnalysisUtils.java147–154; RuleContext.java744–752;
CachingAnalysisEnvironment.java239–266; PathFragment.java254–298;
ArtifactFactory.java302–338; PackageIdentifier.java182–193.
ObjcLibraryTest.java184–185/211–227 and CcCommonTest.java1597/1624 provide outcome
themes; Java allocation assertions are not imported. No direct native-entrypoint
test was found; use discriminating pinned-source regressions.

Accepted Solib inventory identifies allocator cc_library target/exec owners.
Its BUILD declares allocator_library.cc; cc_library.bzl56–79 calls compile with
purpose "cc_library-compile". Existing unchanged54-body corpus compile.bzl1229
computes the prefix; regular branches1554/1564 declare object/dotd outputs through
private _get_compile_output_file2144–2151. This proves source demand, not new
observed CppCompile rows. No new upstream body or oracle is needed.

Required keyword String purpose: suffix _non_objc_arc maps to non_arc, otherwise
_objc_arc to arc, otherwise empty. Java null/mnemonic fallback is unreachable from
this binder; no Bazel mnemonic reproduction. Declaration joins output_name to
[external/<repo>/]<package>/_objs/<label.name>, normalizes, then checks containment
in the ACTIVE ctx package. Same-package other targets/descendant packages are
valid; label equality is not required. Internal .. is allowed, package escape,
absolute/remaining upward paths rejected. Preserve existing Host path constraints.

## Owners and design checkpoint

Loading owns thin native dispatch and a genuine immutable configuration facade
sharing existing Arc-backed SlugConfiguration, replacing Analysis's coverage-only
facade. Coverage projects from that same configuration. Facade remains phase-local,
not newly admitted to AnalysisValue publication. A borrowed genuine-context
capability routes declaration to the actual Analysis ctx/sink. Analysis checks
liveness/subrule restrictions and full supplied configuration equality BEFORE
declaration mutation; reuse bin-relative storage, root, active owner, kind conflict
and dedup checks. No temporary artifact map, fallback root or alternate owner.

Existing ConfiguredNodeAnalysisKey/package/configuration inputs own results;
read docs/developers/dice.md. No new DICE key, observation, cache, filesystem
discovery or lock across compute. Facades cheaply share configuration; published
Files use existing DICE root/provider/action cutoff/invalidation/eviction lifetimes.
Path buffers are call scratch. Reuse Arc/Dupe/CompactString/Allocative and Stage9's
WP766 row, no donor/interner or performance claim. Independent design review
precedes implementation. Independent design ACCEPT on2026-09-21 freezes this
bounded declaration contract; runtime acceptance still requires the named proofs.

## Scope and routing

Implementation worker: Loading cc_common.rs, cc_context.rs, new cc_compile_outputs
and configuration child modules/tests, lib.rs exports, subrule_invocation.rs only
for required sink/token adapter; Analysis starlark_rule/cc_context.rs and its tests,
starlark_rule.rs sink hooks and context_lifecycle_tests.rs. New algorithms live in
child modules; large dispatch/analysis files receive narrow hooks only.
Estimate150–350 production lines plus tests; growth triggers review, not partial landing.
Integration worker: tests/rustc_map_each/compile_outputs.{bzl,rs}, mod.rs scoped
setup/registration, existing authored naming/toolchain fixtures as needed.
Loading subrule_invocation/context_lifecycle_tests.rs may extend its existing
suspended-root token discriminator for the new operation.
Never modify upstream54 bodies or artificially export private functions.
Root owns scheduling, Stage6/9/bootstrap notes, serial Cargo and integration.
Reviewer owns independent design/final actual diff and evidence, no edits.

## Proof and validation

Owner tests: suffix precedence/case/trailing text, required keyword/binder types;
normalized internal traversal vs escape/absolute/backslash/NUL paths; supplied label other-target/
descendant/sibling/repo; repeated declarations and File/tree conflict; expired/
suspended context; foreign configuration rejection before state mutation. Preserve
coverage projection and root/default ctx behavior.

Configured proof: genuine main/external contexts, target/exec roots, naming label
different from active owner, regular File/root publication and A/B/A including
configuration changes. Authored callers use native entrypoints with genuine values
and preserve public toolchain naming through the existing WP765 protected proof.
The new authored helper loads only same-package cc_internal: root-origin external
cross-package loading remains deferred and cannot be expanded to support a test.
This proves declarations, not new composition with the toolchain naming wrapper.
Private compile.bzl wrapper is source-
backed; existing test-only private-module lookup may invoke the unchanged body
if feasible, with no new production export. Do NOT claim full public compile():
other context/tree/action/discovery prerequisites remain open.

Protect WP765 context/naming, WP766 roots and compile-input/toolchain integration.
Resolve pinned nightly; serial offline/locked Cargo JSON no-run for Loading and
Analysis; batched exact preflight/direct tests and Core/CLI dependent checks.
Preparation60s per operation; seconds/test estimates are not new user deadlines.
No remote/daemon/Bazel gate for this declaration-only change. Format/diff/archive/
status checks, independent final ACCEPT and atomic commit. REPLAN for ownership/
compatibility contradiction or unavailable prerequisite, not ordinary compiler/
fixture correction. M7A remains partial; M8 unproved.

## Closure receipt (2026-09-21; accepted)

Base0a1d7d943, candidate on codex/wp767-compile-output-review. Independent design
ACCEPT and independent final ACCEPT confirm the bounded declaration contract,
ownership, rejection-before-mutation and publication/restoration evidence.
31/31 exact-selected tests pass: Loading13 (four new binder/path/Host/configuration
tests, protected naming/context/root helpers), Analysis4 (real ctx/sink lifecycle
and protected execution-OS projections), configured14 (three new declaration/
restoration/configuration-publication cases plus protected roots/naming/compile
inputs/toolchain). No test-result waiver. Receipts in target/wp767 include:
tests-1790034449455763461 (Loading), tests-1790034119459316910 (Analysis), and
tests-1790034338492239099 (configured), all exit0/selected=pass.

Pinned nightly2025-09-14 offline/locked Cargo JSON no-run and Core/CLI checks pass.
Preparation/check104.657s total, max20.859s per operation; latest successful tests
1.345s. Packet elapsed approximately26 minutes including research/design/review.
Initial fixture failures are preserved: unsupported canonical/cross-package load
routes and analysis-time Label construction were removed from the fixture, not
enabled in production; test-only exec-platform scaffolds now provide explicit
Linux Host independent of the tested missing-Host owner. One compile borrow and
one zero-selection preflight were corrected before execution. Configuration
publication is explicitly tested unsupported. No Slug/daemon/Bazel/remote action
was invoked; configured marker FileWrites are registered, not executed.

Formatting, diff, archive and matching packet-status checks pass. No upstream
body, extra public compile native, action representation, DICE key or cache was
added. Original private compile.bzl forwarding remains pinned-source evidence;
the new runtime proof exercises native declarations, not full public compile().
