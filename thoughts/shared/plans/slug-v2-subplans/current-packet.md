# Current Slug V2 Work Packet

Packet: WP-7-158-m7a-default-link-actions-wrapper-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance the unchanged authentic Rust-toolchain cquery past cc_internal.
wrap_link_actions at rules_cc cc/private/link/
create_linking_context_from_compilation_outputs.bzl:105 via cc_library.bzl:152.
Admit wrap_link_actions(actions, build_configuration=None,
sharable_artifacts=False), all three positional-only, for genuine live actions
in the current evaluator. Return a fresh private wrapper facade preserving the
original sink, token, context name and root/subrule scope. Its already-admitted
ordinary methods reuse the same action binder/declaration/registration owners.
Expose wrapper shareable artifact/directory methods only as explicit deferred
refusals; non-None configuration and sharable True are likewise deferred.
The authentic caller has compilation outputs; this is adapter admission, not
an empty-output or full link-action proof.

Exact admitted binding/default and inherited ordinary-operation semantics are
pinned to Bazel9.2 8220c619 CcStarlarkInternal.java:442-483,
StarlarkActionFactory.java:121 and its inherited mutators, and Param.java:78/94.
CppLinkActionBuilder.java:44-99 owns unadmitted config/sharing construction.
Upstream arbitrary non-configuration arguments fall back to caller config;
that lane is narrowed/deferred here. Upstream wrapper has no own private-caller
or mutability check; Slug's evaluator/live-owner checks and facade/type/identity,
configuration/path/action identity remain Slug-native. Shareable declarations,
alternate configuration, factory-local run_shell helper counters,
linkstamp/LTO/link argument construction and full
CppLink/CppArchive actions/toolchain closure remain unsupported/deferred.
Linux is priority; no Windows implementation or execution claim.

## Ownership and scope

Existing AnalysisActions owns sink/token/context_name/scope. Add a private
phase-local WrappedLinkActions value composed over its unchanged handle;
wrapper method tables reuse analysis_actions_methods plus explicit deferred
extensions. A genuine-actions unpack helper accepts only existing base and
private wrapper, used by ordinary action binders and existing actions2ctx_cheat/
solib consumers so inherited factory use preserves original authorization.
Wrapping reuses actions_to_context for current evaluator, live frame and
registered-root validation; suspended frames remain wrappable, but their
ordinary mutations still require the original top-frame token. Do not create
fresh authorization, a replacement context, action owner/configuration or
registry. Both root/subrule handles preserve scope through nested wrapping.
Existing DICE configured/action publication, Need/errors and dependencies remain
unchanged; docs/developers/dice.md applies. No new key, cache, lock or state
repair. Wrapper construction cannot mutate the sink.

Memory is evaluator-phase scratch: one private facade with constant-time
Arc-backed handle clone, released with its heap; captured handles remain
inactive after their original frame exits. No retained evaluator borrow or
new collection/interner. Preserve Allocative and existing Arc-backed sink/token/
scope; no donor import or performance claim. Reuse Stage9 action-handle and
phase-facade rows (WP778/WP767/WP765), following slug-buck2-utility-reuse.
Existing 2000-line subrule_invocation.rs gets only unpack/module registration
and binder substitution; wrapper logic/tests live in a bounded child module.
Existing cc_common.rs receives one narrow registration and two native-unpack
substitutions. Production growth is105lines including the bounded wrapper/refusal method
table (slightly above the initial100-line estimate), ordinary binder
behavior otherwise unchanged.

Allowlist: Loading src/cc_common.rs, src/subrule_invocation.rs,
new src/subrule_invocation/link_actions.rs and its tests.rs;
Analysis tests/subrule.rs only module registration plus narrow shared harness
materialization injection (existing default helper unchanged) and new
tests/subrule/link_actions.rs; this manifest/canonical Current and compact
Stage6/Stage9 acceptance notes. No native linkargs/soname/action activation,
provider or generic spawn extension. Baseline55a802f38. Independent DESIGN
precedes implementation, FINAL gates acceptance.

## Evidence and validation

Reuse pinned wrapper implementation (no direct upstream wrapper test found).
Loading tests exercise all default positional forms, fresh wrapper occurrence,
nested wrapping, genuine base/wrapper unpack, missing/fake/foreign/expired
handles/registration, alternative config and sharing refusal, invalid arity/
named/type binding, and deferred shareable declaration methods. Preserve root
context recovery and suspended frame semantics; wrapper mutation must reject
outer actions inside a child and expired bound methods. Wrapper extensions
must leave declaration/action state unchanged. Existing base action method
availability must not grow from the wrapped extensions.

Real Analysis root/subrule integration must prove shared ordinary declaration/
registration owners via an admitted FileWrite (not a new link action), shared
File identity/conflict handling, nested wrapper actions2ctx recovery, preserved
root/subrule action scope and source/configuration A/B/A on the same DICE in
Legacy/Observed routes. Use existing FileWrite/action registry evidence; no
remote run needed. Root/outer tokens remain locked in child frames; no wrapped
handle publication is admitted. Existing extraction/lifecycle and solib/native
action tests protect unpack changes. No copied public fixture. The declared local rules_cc module preserves private
accessor provenance. Its materialization receipt is included with the existing
builtin receipt before eager source preparation via one shared test-harness
helper; no duplicated snapshot/policy setup or production owner bypass.

Pinned nightly serial Cargo JSON --no-run compile Loading and Analysis named
executables within separate60s preparation caps. Batch exact nonignored
preflight then focused runtime expected a fewseconds; Core direct-dependent
check and CLIrebuild before unchanged authentic cquery from persistent
target/wp856/authentic-workspace, exact WP855threehashes. Supervised180s
operationcap justified by fresh public source preparation; check slugd
before/after in actualenvironment and supervisorcleanup. First newstop/success
is observable outcome, not full linkclosure. Rustfmt/diff/archive/status apply.
Root owns integration/acceptedcommit. REPLAN if default wrapper needs a new
semantic config/action owner or cannot preserve token/scope/heap lifetime.
Routine invocation/compile/test corrections remain in scope.

## Immediate predecessor

WP85755a802f38 accepts coverage-disabled instrumentation with genuineTarget/None
and lifetime validation. Independent DESIGN/FINAL and12focusedchecks pass;
rebuilt unchanged authentic cquery clears it in82.989s and next stops at
wrap_link_actions. Evidence target/wp857. M7A/M8 unproved; Windows deferred.

## Acceptance receipt (2026-09-30)

Independent DESIGN/FINAL ACCEPT. Seventeen exact nonignored focused checks pass:
Loading11 and Analysis6, total runtime0.346s. They prove fresh/nested facade
identity, unchanged sink/token/scope, genuine/current/live owner checks,
suspended and expired token refusal, shared FileWrite declarations/registration,
root/subrule scope, source/configuration A/B/A in both DICE routes, conflicts,
wrapper-only deferred extensions and provider-publication refusal. Existing
base lifecycle, solib, coverage and action-restoration protections pass.
Pinned serial preparation: finalLoading22.67s, finalIntegration1.59s,
Corecheck15.26s and CLIbuild23.95s. Initial test-only compiler/selector errors
and eager-source fixture acquisition failure were corrected without changing
production semantics. The shared harness now includes the declared local
rules_cc materialization receipt before initial source preparation; existing
callers use empty additional receipts and retain their prior setup. No source
identity spoof or private-accessor bypass. Rustfmt/diff/archive/status pass.

Production grows105lines, including a bounded private facade and explicit
refusal table. It clones existing constant-time Arc-backed handles without a
new retained semantic owner/key/lock or evaluator borrow. The unchanged rebuilt
authentic `cquery //:probe --output=label` exits2 in81.433s, clears wrapping and
first reaches dynamic_library_soname at rules_cc cpp_link_action.bzl:193 via
cc_linking_helper.bzl:635 on the PIC static-library path. Original three hashes
remain unchanged. Supervisor cleanup and actual-environment process checks
pass. Evidence target/wp858; measured packet evidence window07:22:44UTC to
around07:49UTC includes review/test-harness corrections/preparation/replay.
Full shareable/alternate-config factories, helper counters, link actions and
toolchain closure/M7A/M8 remain unproved; no Windows execution claim.
