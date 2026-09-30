# Current Slug V2 Work Packet

Packet: WP-7-156-m7a-cpp-implementation-deps-option-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance the unchanged authentic Rust-toolchain cquery past the missing
cpp.experimental_cc_implementation_deps() at rules_cc cc/common/semantics.bzl:114.
Expose that private zero-argument Boolean method through the existing C++
fragment and admit its named Boolean command option through the closed typed
command carrier. Default True, explicit False, last-wins restoration and both
exec projections must reflect the sole retained CppOptions Boolean. Nonboolean
retained state fails before projection publication; invocation-time caller
checks apply even to a transferred bound method. No hard-coded default bridge.

Method shape, default/conversion/exec propagation and private authorization are
exact for this slice, pinned by Bazel 9.2 CppConfiguration.java:866-883,
CppOptions.java:970-979, builtin_exec_platforms.bzl:303 and the blocked expanded
C++ API test in StarlarkCcCommonTest. Pinned Converters.java:38-68 owns true/false lexical forms;
actual Java null is distinct from command text "null", which is invalid. rules_cc 0.2.17 semantics._check_can_use_implementation_deps
is the authentic caller. Structural configuration/DICE identity, occurrence
identity, valid-Unicode diagnostics and path/display tokens remain Slug-native.
Full implementation_deps C++ context/link propagation, general C++ actions and
Rust-toolchain closure remain unsupported/deferred. Linux remains the priority;
Windows required functionality may remain minimal stubs or explicit refusal.
No Windows implementation or execution evidence is added.

## Ownership and scope

The existing native descriptor already owns experimental_cc_implementation_deps
as a Boolean defaulting True. CppFragmentProjection adds one validated getter
using native_bool on that descriptor; SlugConfiguration remains its sole
immutable owner. Append the matching NativeCommandOption discriminant, named
lookup, canonical name and Boolean classification, preserving existing ordinal
values. Existing Commands occurrence capture keeps raw text and Configuration
performs conversion and preparation into the same structural option vector.
The shared textual Boolean converter currently conflates literal "null" with
actual Java null. Correct convert.rs::boolean to reject null text; the existing
defaults.rs materializer independently owns actual-null defaults and must still
produce False for a nullable Boolean default. Correct only the matching existing
primitive conversion regression and preserve its existing explicit
default-domain control.
Protect an existing Boolean command option alongside the new one through the
same converter. No extra command-side validation or option-specific parser.
Both existing exec paths already preserve this field, matching pinned propagate;
add no new exec mapping. Existing configured keys and publication own equality,
invalidations and Need/error behavior; no new DICE state, lock, cache, parser,
raw string fallback, fragment store, resolver or command repair. The evaluator
checks current source-provenance authorization at invocation and publishes the
primitive Boolean. RetainedCppFragment's existing occurrence identity and
configuration-based publication pairing remain unchanged. Link inherited
contracts in docs/developers/dice.md and accepted WP855 at 37ba64711.

No new retained collection or representation; existing Allocative/Dupe and
structural configuration ownership cover the extra closed enum variant. This
is semantic admission, not a performance experiment. Bounded existing facade
owners stay cohesive. The large configuration.rs receives only test-module
registration; focused private-state tests live separately. The larger tests.rs
receives only the correction of its existing primitive converter case and preserves the
existing default-domain control, keeping the source-derived assertion with
its owner. No unrelated
function activation or owner split.

Allowlist: Configuration src/command.rs, src/native/cpp_fragment.rs,
src/native/configuration.rs only for test registration, src/native/convert.rs
only Boolean textual-null correction, src/native/tests.rs only the matching
primitive conversion regression/default control, and new
src/native/cpp_implementation_deps_tests.rs; Loading src/analysis_fragments.rs
and src/analysis_fragments/toolchain_provider_tests.rs; Commands src/common.rs
only its test section; Analysis tests/subrule.rs only module registration and
new tests/subrule/implementation_deps.rs; this manifest, canonical Current and
compact Stage 4/6 notes after acceptance. No native registry inventory change,
Build API state, new exec semantics, sysroot/platform owner, C++ provider/action
or ruleset/catalog modification. Estimated production growth under 25 lines.
Independent DESIGN gates implementation; FINAL gates acceptance.

## Evidence and validation

Reuse accepted BooleanConverter and private fragment/current caller evidence.
Configuration tests discriminate default/False/True, accepted explicit Boolean
spellings, no/bare forms, last occurrence wins, invalid text including literal null/NULL, whitespace and illegal no=value,
malformed retained state, structural identity and restoration, both exec values
and selected-platform idempotence. Commands test raw captured occurrences and
use its public parser flow to Configuration for valid conversion/refusal;
conversion must stay out of capture. Existing ForcePic command-null refusal
protects the shared converter owner. The actual-null default control preserves
nullable default semantics. Existing config_setting matching must use the same
Boolean state and reject null text for both the new and protected existing
option; no other converter behavior changes. Loading tests discriminate Boolean type,
zero arity, allowed/nonallowed invocation, exported bound-method denial and
retained-source value over different consumer configuration. Exercise the real
pinned rules_cc guard with empty/nonempty implementation_deps and true/false
through the current facade, while leaving C++ dependency semantics deferred.
Analysis reuses observed subrule helpers for real root/subrule method calls,
configuration and source A/B/A on the same DICE, target/both exec preservation,
zero actions and inherited undeclared/expired/private controls. No new copied
external fixture or public oracle scaffolding. Upstream full CcCompilation/
link-context tests remain deferred; this packet only activates the option getter.

Resolve pinned nightly-2025-09-14 tools; serialize Cargo. JSON --no-run compile
Configuration, Commands, Loading and Analysis selected executables, batch exact
nonignored preflight and execute all selected gates serially. Focused runtime
should remain a few seconds. Each preparation invocation is bounded to 60s.
If completed codegen survives an interrupted link, inspect finalized incremental
work products before one unchanged retry, as WP855 established; a timeout itself
never proves a gate. Rebuild slug_cli_v2 before the unchanged authentic one-shot replay, no
output_base. The lost /tmp fixture is byte-exactly reconstructed in persistent
target/wp856/authentic-workspace; all three WP855 receipt hashes match. Check slugd before/after
in the actual execution environment; supervisor cleans the process group.
Use the existing 180-second cap justified by WP855's 171.782-second replay;
this is an operation cap, not a user deadline. Record exact fixture hashes and
first new stop/success, keeping full closure unclaimed. Rustfmt, diff, archive
and plan-status gates apply; root owns integration and accepted commit.

REPLAN only if the existing Boolean option owner cannot express source-backed
semantics or current-caller authorization needs a new provenance boundary.
Invocation, compile, resource preparation and test corrections stay in scope.

## Immediate predecessor

WP855 at 37ba64711 accepted nullable private grte_top and host option exec
projection. Independent DESIGN/FINAL and 19 exact focused tests pass. Rebuilt
unchanged authentic cquery clears grte_top and first reaches the separate
experimental_cc_implementation_deps getter after 171.782 seconds. M7A remains
partial. Evidence resides in target/wp855 and Git history.

## Acceptance receipt (2026-09-30)

Independent DESIGN and FINAL ACCEPT. Twenty-six exact nonignored focused tests
pass (Configuration 9, Commands 4, Loading 7, Analysis 6), total runtime 0.378s.
Pinned nightly preparation: Configuration initial 1.71s/final 0.59s, Commands
14.33s, Loading 22.36s, Analysis 6.58s; CLI build 24.40s. Rustfmt, diff, archive
and scheduling checks pass. The production change is 21 added lines and one
Boolean lexical correction; no new retained owner or DICE key.

The rebuilt authentic `cquery //:probe --output=label` exits 2 in 83.944s,
clearing the getter and first reaching `analysis_ctx.coverage_instrumented()`
at rules_cc cc/common/cc_helper.bzl:304 via cc_library.bzl:73. The environment
continuation lost /tmp; exact BUILD/MODULE text and a committed-root-lock
projection restore the original three bytes, verified against WP855 SHA-256
receipts before and after. Only cwd moves to persistent target storage.
The failed missing-script launcher never ran a query and proves no semantic
gate. Supervisor cleanup and actual-environment process checks pass.
Raw compile/preflight/test/replay/recovery evidence resides in target/wp856.
Evidence storage began 05:27:33 UTC; close around 07:00 UTC includes the
interrupted environment gap and recovery, not measured active work or a
workflow speedup. Full implementation_deps context/link semantics, coverage,
C++ actions, toolchain closure and M7A/M8 acceptance stay unproved.
