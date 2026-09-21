# Current Slug V2 Work Packet

Packet: WP-7-58-m7a-native-cc-feature-model-r1
Status: accepted

## Outcome and compatibility

Implement the native C++ feature-model prerequisite: complete eager metadata
validation and owned retention, defaults, requested feature selection, and native
FeatureConfiguration is_requested/is_enabled. WP757 accepted at f921c2d20 found
no direct declared allocator Solib consumers; native Solib execution stays deferred.
This packet narrows that selected successor to its native model boundary. The
unchanged public configure_features entrypoint still requires structural ctx/Cpp
projections and exec_os; it is not activated or claimed by native selection tests.
Those projections precede a later end-to-end public configuration admission.

Exact: admitted Bazel 9.2 native constructor validation, declaration ordering,
defaults, implies/requires/provides selection, membership, fresh-wrapper occurrence
equality and unhashability. Slug-native: valid-Unicode strings, complete structural
publication identity including Artifact ownership, deterministic requested insertion
order without Java request-set cache reuse. Unsupported/deferred: expansion/tool
lookup/artifact-name APIs, public configure_features/ctx projections, exec_os,
compile/actions2ctx, action creation/execution, full C++/Cargo loading and M8.
Admitted execution OS values are LINUX, DARWIN, FREEBSD and OPENBSD; WINDOWS,
UNKNOWN, unrecognized OS values, NUL-containing tool/tools-directory/path-chunk
strings and cyclic flag-group metadata fail closed as unsupported. Diagnostic
checks establish rejection/message shape, not Java stack/location or full error bytes.

## Source contract and invariants

Bazel source 8220c6198837d5c13d53fea211cf3282aa12408a under
src/main/java/com/google/devtools/build/lib/rules/cpp/:
CcStarlarkInternal.java:257–269; CcToolchainConfigInfo.java:60–91;
CcToolchainFeaturesLib.java:65–573; CcToolchainVariables.java:151–240;
CcToolchainFeatures.java:1099–1217,1230–1330 and FeatureSelection.java:115–301.
FeatureConfigurationForStarlark.java:42–90 and StarlarkValue.java:92–121 own
fresh occurrence and unhashable semantics. Internal FeatureConfiguration.equals
omits requested names/tools directory and is NOT Slug publication equality.

Retain every parsed feature, action config, recursive flag/env group and condition,
variable chunk, tool path/origin/Artifact, feature predicate, execution requirement,
artifact pattern, tools directory and admitted exec OS. Constructor eagerly validates
metadata even while expansion is deferred. Features precede actions in defaults and
selection order. Selection does not autoadd defaults; unknown requested names remain
requested but disabled. Requires is OR-of-AND and does not enable dependencies;
implication closure is pruned to the source fixed point, cycles allowed. Preserve
provides multiplicity and reject collisions, including one duplicate provider.

Producer is the existing DICE-owned Starlark evaluation over loaded source,
configuration and provider inputs. BuildAPI owns immutable typed model/selection
payloads and occurrence handles. Loading owns parsing/native facades; Analysis owns
lowering/materialization. Publication compares complete payload plus a bidirectional
occurrence mapping across the entire retained provider graph. Starlark equality is
occurrence-only. No evaluator heap, command scratch or borrowed Artifact view escapes.
No new DICE key/lock/cache, direct filesystem discovery, digest domain or Java runtime.
Read docs/developers/dice.md and the utility skill. Stage 9's existing Arc,
CompactString, SmallMap/SmallSet and occurrence-publication dispositions apply.
New memory is DICE-retained semantic data released with provider/result ownership;
selection/parser buffers are phase scratch. No service cache/interner. Cheap shared
clone and Allocative required. No performance claim or benchmark gate.

## Scope and ownership

BuildAPI worker: new app/slug_build_api_v2/src/cc_features.rs (optional child
modules/tests beneath cc_features/), lib.rs exports only. Typed model, eager
normalization helpers and selection algorithm/tests; coordinate parser API first.
Loading worker: new app/slug_loading_v2/src/cc_features.rs and child parser/tests,
cc_common.rs factory, lib.rs registration/exports only. Native facades, exact provider
parsing and focused source-derived invocation/constructor tests.
Root: app/slug_build_api_v2/src/analysis_value.rs; app/slug_analysis_v2/src/analysis_value.rs
and new child tests for retention/publication. Loading/Analysis BUILD.bazel changes
are limited to compile_data and the package-scoped export of the reused constructor
fixture (required for the new include_str under Bazel). Scheduling docs, bootstrap-readiness
and Stage 9 architecture row only as changed. No other production/fixture/dependency
or lockfile edits. Large existing owners receive dispatch integration only; put new
semantic work and tests in cohesive modules. Estimate 1,500–2,500 production and
500–900 test lines; larger growth triggers responsibility review, not a weaker model.
Independent reviewer owns design checkpoint before code and final actual diff review.

Independent design review ACCEPT: native-only scope is coherent; final review must
verify complete eager metadata retention and explicit fail-closed OS/path handling.

## Evidence and validation

Pinned-source regressions adapt CcToolchainFeaturesTest.java implications/pruning/
requirements/cycles (1141–1377), defaults (1558–1579), action/feature links
(1736–1790,1913–1959), duplicate actions/flag-set actions (1961–2012) and provides
(2015–2027); StarlarkCcCommonTest.java groups (3158–3402), tools/OS (3484–3724),
features (4349–4597). Cover nested metadata parsing/invalid inputs, unknown requests,
default ordering, duplicate symbols and fresh occurrences. Expansion tests remain
deferred with the unadmitted phase, not skipped parity.

Use existing exact rules_cc source/constructor definitions in the loading fixture
where practical, authenticated from rules_cc 0.2.17 archive SHA256
283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a.
Authored transport/native calls are explicitly native-boundary proof, not execution
of the whole public configure_features wrapper or production cc_library. No new
fixture corpus, Bazel query/build or Java helper. Existing source-freeze tests do not
prove invocation. No guessed Cpp/context defaults for an end-to-end claim.

Retention regressions dispose source heap, rematerialize and invoke methods; verify
alias versus fresh-instance equality, nested metadata/tools directory/request/Artifact
changes in publication, alias topology changes, rejection from dict/depset before and
after freeze, and GC safety. Native objects remain unhashable even though Rust payloads
are immutable. Native model/configuration publication is atomic with all bridges.

Public/cross-crate tier: serialize pinned-toolchain cargo test --lib --no-run JSON
for BuildAPI, Loading and Analysis; preflight all exact selectors once per unchanged
harness; run focused owner and retention tests. Check direct dependent slug_core_v2
and slug_cli_v2 compilation (no binary invocation). Preparation cap60s per operation;
runtime expected seconds, no long broad suite. These are packet caps/estimates, not
new user ceilings. Root owns Cargo orchestration. Format changed Rust, run diff check,
plan status and archive status. Preserve unfinished candidates if gates remain open.

REPLAN if a complete eager model cannot be represented within the existing ownership
boundary, source contradicts selected compatibility, or activation requires new
semantic configuration/platform decisions. Correct ordinary compile/test failures
within this contract. Acceptance requires independent final review, compact evidence
and a commit; M7A remains partial and M8 unproved.

## Implementation and validation receipt

Complete native parser/model/selection and both retained bridges are implemented.
Immutable defaults, complete recursive metadata, source-order provided-symbol
collision checks, first-seen native predicate normalization, occurrence-only
Starlark equality and full-payload/alias publication are covered. Retained list
storage is boxed behind Arc handles; name indexes and selection buffers are scratch.
No source corpus, dependency, DICE key/lock/cache or action family was added.
The reused constructor fixture is explicitly declared in Bazel compile_data with a
Loading-package-only export. Fixture SHA256 f8418490663f7e188fa060265b215e80b154a5d190b32ef75fcb6d0254017808
matches the exact cc/cc_toolchain_config_lib.bzl archive member.

Pinned nightly-2025-09-14 Cargo --offline --locked --lib --no-run JSON preparation:
BuildAPI succeeds 2.571s; Loading 29.226s; Analysis 24.667s, then 1.668s after the
test-only GC harness correction. All selected harnesses were preflighted once per
unchanged binary. Exact execution: BuildAPI 9/9 native model tests; Loading 6/6
native boundary tests plus 2/2 protected authentic configure/legacy source-freeze
tests; Analysis 4/4 native publication/heap/GC/hash tests plus 1/1 protected
HeaderInfo round trip. Total 22/22, approximately 0.102s summed successful runtime.
Core cargo check succeeds 24.849s; CLI cargo check succeeds 15.186s. No Slug/Bazel
binary, daemon, build action or broad suite was invoked.

Two ordinary compile corrections: terminate borrowed scratch indexes before moving
model data (0.715s failed preparation), and import ValueLike in Loading tests
(13.085s failed preparation). The first Analysis invocation failed in the test's
direct post-evaluation GC call (0.004s); corrected to the evaluator's supported
Assert Auto/Never/Always modes and safe scheduled garbage_collect builtin, then all
five tests passed. No production assertion was weakened. Total preparation/check
time including corrections is 111.967s; every operation stayed below its 60s cap.
Scratch commands, logs and individual receipts are under target/wp758; no failed
receipt was overwritten. Format, diff, plan status and archive checks pass.
Test growth is 1,269 lines, above the 500–900 estimate. Responsibility review keeps
the three cohesive modules: 416 lines model/selection, 491 authenticated parser/API,
and 362 retained publication/GC. No fixture corpus or large existing test owner grew;
the extra integration tests discriminate ownership and hash barriers across phases.

Final review checks the complete diff and these discriminators, including the
immutable-default correction and Bazel compile-data declaration. The selected
successor is structural ctx/Cpp and exec_os ownership plus unchanged public
configure_features admission, still before compile/actions2ctx. No fabricated
configuration defaults or runtime support is implied by this native-only result.

Independent final review ACCEPT. Packet elapsed approximately 28 minutes
(18:27–18:55 UTC), including source/design work; review overlapped implementation
and validation. Gate advanced: native C++ feature metadata and selection survive
provider publication with complete owned payloads. Public configuration, expansion,
compilation and execution remain separately gated. No semantic REPLAN was required.
