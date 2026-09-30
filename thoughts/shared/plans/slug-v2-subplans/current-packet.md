# Current Slug V2 Work Packet

Packet: WP-7-155-m7a-cpp-grte-top-projection-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Advance the unchanged authentic Rust-toolchain cquery past the missing
`cpp.grte_top()` at rules_cc cc_helper.bzl:844. Expose exactly that zero-argument,
nullable Label method through the existing private C++ fragment facade, using
already owned `grte_top` state. Default/default-reset returns None; a visible
converted label returns the existing canonical Starlark Label. Preserve the
current-caller private API check even for a bound method transferred from an
allowed module. Do not fabricate None for nondefault or unmodeled state.

Method shape, nullable label projection, converted package `:everything`,
preservation of converted canonical labels and caller authorization are exact
for the admitted slice,
anchored in pinned Bazel 9.2 CppConfigurationApi.java:173-179,
CppConfiguration.java:754-769, CppOptions.LibcTopLabelConverter and
builtin_exec_platforms.bzl:300-339. Structural configuration/DICE identity,
Rust valid-Unicode diagnostics and path/display tokens remain Slug-native.
Full cc_toolchain sysroot/C++ execution and authentic Rust-toolchain closure
remain unsupported/deferred. Linux remains the platform priority; Windows
required functionality may remain minimal stubs or explicit refusal. This
packet introduces no Windows implementation or execution evidence.

## Ownership and scope

CppFragmentProjection borrows the sole immutable SlugConfiguration owner.
Add a validated getter using its existing cpp_label("grte_top"); conversion,
canonical label identity and nullable/error semantics remain in Configuration.
The existing exec projection helper must install grte_top from retained
host_grte_top for both exec entry points, matching builtin_exec_platforms.bzl339.
The default host value is None; typed nondefault host values remain structural.
HostGrteTop command admission stays deferred. LibcTopLabelConverter accepts only
absolute //package spellings or default; external/relative command spellings
remain rejected, with no generic LabelConverter substitution.
Validate the getter before the projection is published. RetainedCppFragment
continues retaining the complete structural configuration and its Slug-native
occurrence identity; publication equality compares the configuration with
occurrence pairing while evaluator equality preserves occurrence identity;
CppFragmentValue allocates only the result Label on the current evaluator heap.
Use the existing check_default_allowlist path at invocation, not binding time.
No DICE key, lock, side cache, duplicated retained state, parser, label resolver,
provider, command repair or Host read. Existing root/subrule fragment lifecycle,
configuration equality/invalidation and docs/developers/dice.md contracts apply.
No representation change or performance experiment; existing Allocative/Dupe
ownership is unchanged. cpp_fragment.rs and analysis_fragments.rs remain bounded
cohesive facade owners; large existing test modules only receive focused tests.
The existing >2,000-line configuration.rs receives only the bounded mapping arm
in its existing exec
projection helper plus a test-module registration; this avoids a new transition
owner or unrelated split.

Allowlist: Configuration src/native/{cpp_fragment.rs,configuration.rs,
cpp_grte_top_tests.rs}; Loading src/analysis_fragments.rs and src/analysis_fragments/toolchain_provider_tests.rs;
Analysis tests/subrule.rs and tests/subrule/grte_top.rs; this
manifest and canonical Current; compact Stage 4/6 notes after acceptance.
No command admission, ConfigurationField inventory, unrelated exec semantics,
Build API state, catalog, ruleset modifications, sysroot validation or action.
Estimated production growth is under 30 lines. Independent DESIGN review gates
implementation and FINAL review gates acceptance.

## Evidence and validation

Adapt pinned StarlarkCcCommonTest.testExpandedCppConfigurationApiBlocked to the
focused invocation and transferred-method denial. CppSysrootTest host override
source backs the target/exec discriminator; full upstream sysroot/action tests
are skipped because C++ execution and toolchain sysroots are deferred.
Reuse accepted libc_top converter/field and structural native option evidence;
this is a new facade getter, not a new converter. Pin the source in tests and
check default/label/default, absolute package rewrite, retained canonical
identity and retained-state error refusal. Exercise target and both admitted exec projections
from retained host_grte_top, including default reset and typed host override;
discriminate different retained configurations/equality. Protect the existing
LibcTop configuration_field projection on target and exec from divergence.
Loading tests check callable/zero arity, Label type and attributes, nullable
behavior, allowlisted/nonallowlisted calls, and current-caller denial of a
transferred bound method. Analysis reuses its observed subrule fixture for real
root/subrule invocation, configuration A/B/A and existing undeclared/expired
fragment controls. No new copied external fixtures or public oracle scaffolding.

Resolve pinned nightly-2025-09-14 tools; serialize Cargo. JSON --no-run compile
Configuration, Loading and Analysis focused executables, batch exact-name
preflight then execute selected ordinary tests serially. Compile operations
bounded to 60 seconds; expected focused runtime under a few seconds. Rebuild
slug_cli_v2 before authentic unchanged /tmp/slug-wp797-authentic-2YQmSt replay.
Check stale slugd before/after. The one-shot replay has a supervised 180-second
operation cap because accepted WP853/854 fresh source preparation took roughly
79-88 seconds; smaller tests cannot establish real closure advancement. This is
an operation cap, not a user deadline. Record fixture hashes and first new stop
or success, leaving full closure unclaimed. Run rustfmt, git diff --check,
archive and plan-status checks. Commit accepted work with compact evidence.

REPLAN only if the getter requires a new label/configuration owner, current
caller restriction cannot reuse the admitted source-provenance owner, or pinned
source contradicts the nullable getter contract. Invocation/compiler/test
corrections remain in this packet.

## Immediate predecessor

WP854 at ca8a43939 accepts the pinned installed def_parser package and native
filegroup direct constraint selector. Independent DESIGN/FINAL and 29 focused
tests pass; rebuilt unchanged authentic cquery clears def_parser and reaches
cpp.grte_top() in rules_cc cc_helper.bzl:844 after 79.064 seconds. M7A remains
partial. Evidence is under target/wp854 and Git history.

## Acceptance receipt

Independent DESIGN and FINAL reviews: ACCEPT. The private zero-argument
method returns the existing nullable typed Label, checks the current caller
on each invocation (including transferred bound methods), and performs no
libc target lookup or action. Constructor validation rejects malformed and
nonvisible retained labels. Both existing exec paths install retained
host_grte_top, including default None and typed host override, in agreement
with the existing LibcTop configuration_field. Command admission is unchanged.
Retained occurrence identity and structural publication/configuration ownership
are preserved; no Build API or DICE state was added.

Nineteen exact selected tests pass after batched preflight: Configuration 9/9,
Loading 6/6 and Analysis 4/4. They discriminate converter/default/Label behavior,
argument refusal, direct and transferred caller restrictions, target and both
exec projections, malformed/nonvisible target/host state, field agreement,
protected occurrence identity, real root/subrule calls without a libc package,
and same-DICE configuration/source A/B/A on Legacy and Observed routes.
Focused runtime is 0.822 seconds total. Final JSON --no-run preparation:
Configuration 7.31 seconds, Loading 12.33 (initial 38.99), Analysis 2.76.

The initial Analysis module registration required an explicit path; that
invocation was corrected. Subsequent preparation hit its 60-second cap after
finalizing incremental codegen. Read-only inspection found finalized work
products; the unchanged retry reused them and finished in 2.76 seconds. No
failure or timeout counted as a passed test. An initial Loading assertion
incorrectly expected separate facade occurrences over equal configurations
to compare equal. It now preserves established occurrence semantics, and the
protected retained-fragment identity test passes with the corrected gate.

Current-source offline CLI build passes in 27.23 seconds. The unchanged
/tmp/slug-wp797-authentic-2YQmSt one-shot cquery exits 2 after 171.782 seconds,
inside the 180-second supervised cap, with identical BUILD/MODULE/lock hashes.
It clears cpp.grte_top and first stops at the separate missing
cpp.experimental_cc_implementation_deps() at rules_cc cc/common/semantics.bzl:114,
called from cc_library.bzl:47. This is authentic bootstrap advancement,
not full Rust-toolchain success. The process group is cleaned and actual
execution-environment process checks show no remaining slug or slugd.
Rustfmt, diff, archive and plan-status gates pass. Raw evidence remains under
target/wp855/. The measured implementation/evidence window beginning at
04:56:54 UTC is approximately 14 minutes; design setup and review time were
not separately measured. M7A remains partial; full C++ closure and Windows
execution remain deferred under the user's Linux-first priority.
