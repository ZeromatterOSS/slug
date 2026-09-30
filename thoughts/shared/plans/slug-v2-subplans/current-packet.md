# Current Slug V2 Work Packet

Packet: WP-7-160-m7a-cpp-start-end-lib-option-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance unchanged authentic Rust-toolchain cquery past cpp.start_end_lib()
at rules_cc cc/private/link/finalize_link_action.bzl:135. Admit the private
zero-argument Boolean getter and --[no]start_end_lib using the already retained
CppOptions.start_end_lib descriptor, True default and shared Boolean converter.
Exact named slice: Bazel9.2 8220c619 CppConfiguration.java:463-473,
CppOptions.java:150-165, builtin_exec_platforms.bzl:305 and Boolean converter
Converters.java:38-68. CcModule.java:189-191/BuiltinRestriction.java:181-183
pin the default private allowlist. No direct getter test found; downstream
CcCommonTest.java:392/425 and LinkCommandLineTest.java:276/300/323 cover
feature+enabled and disabled-option use. Full link execution tests remain
unadmitted; the pinned getter/options and focused source regression suffice.
Getter checks current invoking private caller, not
getter-extraction provenance. Exec carries the target Boolean unchanged.
The rule's actual use is a conjunction with supports_start_end_lib; this packet
admits option observation, not linker feature/action execution. Linux priority;
Windows-specific work deferred under user instruction. Existing valid Unicode,
structural configuration/path/action identity remains Slug-native.

## Ownership and scope

SlugConfiguration retained typed option records remain sole semantic owner.
Append NativeCommandOption::StartEndLib (existing repr ordinals preserved),
name/canonical Boolean admission; existing command capture/preparation owns
conversion/last-wins/negation and structural configuration input. No new
option descriptor or alternate converter. CppFragmentProjection validates
native_bool(CPP_OPTIONS, start_end_lib) during construction and projects it.
Loading zeroarg method uses existing invocation-time CppFragmentValue.check;
transferred method retains original config but checks current caller. Existing
private allowlist/missing fragment/root/subrule token contracts apply.

No DICE key, lock, cache, repair, toolchain lookup, action or environment read.
Existing configured keys track structural configuration/source and publish
primitive bool results; true/false change identity, restoration uses same DICE.
Exec preserves retained option via existing mapping, not a new override.
Projection is existing evaluator scratch backed by shared config; bool adds no
retained memory. docs/developers/dice.md and accepted WP856 private Boolean
owner contract apply. New metadata enum variant uses existing encoding; no
Bazel checksum or mnemonic identity claim. Existing large common.rs and
analysis_fragments.rs receive only bounded test/registration deltas; semantic
getter stays in existing small cpp_fragment.rs. Production estimate below35
lines, test growth below200; keep tests cohesive/use existing helpers.

Allowlist: Configuration src/command.rs, src/native/cpp_fragment.rs,
src/native/configuration.rs only focused test-module registration plus new
src/native/cpp_start_end_lib_tests.rs; Loading src/analysis_fragments.rs and
src/analysis_fragments/toolchain_provider_tests.rs; Commands src/common.rs
only tests; Analysis tests/subrule/implementation_deps.rs existing private
Boolean fixture; this manifest/canonical Current and compact Stage6 note.
Baseline d40eb4c87. No linker, archive/start-lib action activation, new public
oracle fixture, Windows implementation or donor change. Independent DESIGN
before implementation, FINAL before acceptance.

## Evidence and validation

Reuse pinned registry/default and WP856 shared Boolean conversion evidence.
Focused owner checks cover True default, false/true/last-wins structural
identity/restoration, independent implementation-deps vs start_end_lib values,
legacy/selected exec carry and idempotence, malformed retained values rejected
at construction, config_setting observing same Boolean, invalid/null text.
Command tests exercise actual public parse/capture/preparation/negation with
no extra native descriptor. Loading tests cover bool callable/zeroarg binding,
private direct/ transferred invocations checked at current caller, original
bound configuration and target/exec values. Protect existing fragment/lifecycle
checks. Analysis extends existing root/subrule fixture with independent
primitive Boolean fields: crossed values discriminate wrong-option aliasing,
source/config A/B/A in both same-DICE routes and exec restoration. Preserve
prior implementation-deps assertions. No fixture growth or copied assets.

Pinned nightly serial Cargo JSON --no-run Config/Commands/Loading/Analysis,
separate60s preparation caps; batch exact nonignored preflight then focused
runtime expected under a fewseconds. Core direct-dependent check and CLI
rebuild before unchanged originalthreehash authentic cquery. Supervised180s
operationcap justified by observed81s fresh publicsource preparation; no smaller
fixture establishes actual finite closure frontier. Actual environment Slug
process checks and supervisor cleanup before/after. Rustfmt/diff/archive/status.
Root integrates/commits. Bounds are operation caps, not user limits. REPLAN
if getter needs new config/action ownership or source contradicts mapping;
routine invocation/compile/test corrections remain in scope.

## Immediate predecessor

WP859 d40eb4c87 admits True SONAME on genuine wrapper with exact Linux
basename and Slug-native lifetime checks. DESIGN/FINAL,18focused checks and
Core/CLI pass. Unchanged authentic cquery81.071s clears soname then first
reaches start_end_lib, originalthreehashes unchanged, cleanup pass. Evidence
target/wp859. False naming/non-Linux/full link closure/M7A/M8 remain deferred.

## Acceptance receipt (2026-09-30)

Independent DESIGN/FINAL ACCEPT. Twenty-nine exact
nonignored focused checks pass: Configuration11/Commands4/Loading8/Analysis6,
total runtime0.463s. Final serial preparationConfig0.49s, Commands13.00s,
Loading20.78s, Integration6.31s, Core8.68s, CLI23.45s. Initial test-only None
inference and missing Linux exec-host fixture errors were corrected; final
all-gate evidence applies. Production17lines plus4test registration lines;
net tests200lines (Config97/Loading50/Commands+Analysis53), no new public
fixture. Existing tests were parameterized without dropping prior rows;
independent crossed Booleans, both exec paths, original transferred config and
current caller gates, real command parse/preparation, malformed state and
same-DICE source/config restoration are discriminating.

Unchanged rebuilt authentic cquery exits2 in82.489s, clears start_end_lib and
first reaches cc_internal.intern_seq at finalize_link_action.bzl:161 on the
PIC archive path. Original three hashes unchanged. Supervisor cleanup and
actual-environment process checks pass; format/diff/archive/status pass.
Evidence target/wp860; measured evidence window08:15:17UTC to after08:27:22UTC
includes review/test-fixture corrections/preparation/replay. No performance
speedup claim. Full link/toolchain closure, M7A/M8 and Windows execution
remain unproved.
