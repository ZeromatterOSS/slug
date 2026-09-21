# Current Slug V2 Work Packet

Packet: WP-7-56-m7a-solib-construction-r1
Status: accepted

## Outcome and evidence

Implement the demanded native dynamic_library_symlink(actions, library,
solib_directory, False, False) constructor, retaining a distinct structural
SolibSymlink action and returning its regular derived File. WP755 accepted at
245f67b63 identifies two generated allocator-library inputs; reuse
[solib-demand.json](./solib-demand.json) and the pinned sources/tests in
[solib-construction-inventory.md](./solib-construction-inventory.md).
No new oracle workspace or production build is needed.

Bazel 9.2 source at 8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.java:204–221; SolibSymlinkAction.java:158–205,228–321;
Actions.java:45,85,191–225,376; OutputDirectories.java:127–148;
RuleContext.java:711,730. Reuse unchanged rules_cc helper
_construct_dynamic_library_to_link NODEPS_DYNAMIC_LIBRARY callsite. The public
constructor takes the base action factory; its wrapper is not required for this
leaf proof. Full cc_library prerequisites remain unsupported.

## Compatibility and semantic ownership

Exact admitted mechanics: False/False escaping, lib prefix, SHA256 long-name
hashing preserving final extension at >255 UTF-16 units for valid Unicode,
single regular input/output, SolibSymlink mnemonic, no progress message and
platform-independent native action semantics. Generated regular library inputs
only; use the pinned native shared-library filename predicate, not the narrower
public create_library_to_link predicate.

Slug-native naming: existing ordinary derived output.path includes the package
within Slug's configured root. Naming-only input root-relative projection is
output.path for the main repository, external/<canonical input repository>/output.path
for external repositories. This does not rewrite the retained input artifact.
Output is bin-relative solib_directory/mangled_name, without consumer package or
repository prefix, matching default nonsibling shared bin-root placement.
Slug owns no Bazel mnemonic/ST-history field: omit ST suffix as an explicit
Slug-native configuration/path projection. Retain full input configured owner
and full consumer configuration. Different configured inputs colliding on the
same projected output fail closed; never share by path alone. Solib-derived
inputs (output.path beginning _solib_) are rejected; no heuristic prefix stripping.
This packet supports canonical relative solib directories only.

Unsupported/deferred: true flags, source/tree inputs, runtime-basename/custom-path
constructors, exact Bazel configuration/output bytes, ownerless alias equivalence,
ActionKey/aquery projection, native Solib execution/materialization and REAPI,
wrap_link_actions, and complete C++ compile/link/toolchain support.

BuildAPI owns SolibSymlinkSpec(input AnalysisArtifact, output ActionOutput), a
separate ActionPayload/ActionKind and accessor. Never expose it via symlink_spec().
AnalysisActionSink and active AnalysisActions carry the constructor into the
existing synchronous analysis sink; no evaluator value survives publication.
Same-owner identical native registration is idempotent through the existing
registry owner lookup; nonidentical duplicates retain conflict behavior.
Core ValidatedActionClosure permits cross-owner duplicates only for structurally
equal native specs and empty action execution properties/group. Retain every
configured owner; ignore native owner platform facts only for native sharing.
Mixed families remain unsupported. Existing scalar FileWrite sharing is unchanged.
Prerequisite/runtime and REAPI entrypoints must still reject native Solib.

All new state is DICE-retained through existing configured analysis/actions and
closure values, with structural Eq and Allocative. Existing source/configuration
dependencies own invalidation/restoration; no key, discovery bypass, extra cache,
interner, async work or lock across compute. Existing canonical labels, shared
configured identity and ActionOutput storage are reused (Stage 9 utility row).
Naming/hash buffers are construction scratch; no performance improvement claim.

## Scope and work division

Baseline 245f67b63, clean. Root owns scheduling, integration, Core sharing/runtime
gates and tests. BuildAPI worker owns actions/solib_symlink.rs (new), spec.rs,
ctx_actions.rs, reapi_projection.rs, mod.rs, lib.rs, Cargo.toml/BUILD.bazel and the narrow Cargo.lock
dependency-list update if workspace sha2 reuse requires it. Loading/Analysis
worker owns cc_common.rs and its new child test module, subrule_invocation.rs,
starlark_rule.rs and new starlark_rule/solib_tests.rs, reusing unchanged
rustc-map-each fixture source. Root owns the two Loading/Analysis BUILD.bazel
entries declaring that reused source as test compile_data, with package-private
source visibility; no new fixture copy or build-graph semantic dependency.
Root may edit runtime/configured_action_closure.rs and new bounded test module
under runtime/action_prerequisites plus its module registration; docs allowlist
is this manifest, canonical Live Status, bootstrap-readiness, inventory and
Stage 9 adopted reuse row. Exhaustive enum consumers may receive only required
fail-closed match arms, reported before edits. No fixture-copy growth.

Large spec.rs/starlark_rule.rs remain existing dispatch/sink owners; isolate
naming logic and new tests in child modules. Estimate <300 production and <400
test lines; exceeding these is a cohesion review trigger, not a hard stop.
Independent design review precedes implementation; same reviewer checks final
diff and invariant-to-test evidence. Other workers use disjoint files.

## Validation and closure

Public cross-crate tier. Resolve pinned nightly-2025-09-14 tools explicitly;
serialize Cargo, compile --no-run JSON separately (60s preparation operation cap),
preflight exact named tests once per unchanged executable. Focused BuildAPI
proofs cover escaping/threshold/hash/extension, main/external projections,
input/config structural identity, invalid inputs, idempotence/conflict and REAPI
rejection. Analysis proofs cover active token, binder flags/types, bin-relative
declarations and retained input/output. The unchanged private helper proof runs
in Loading with the actual private binder and a recording sink; Analysis tests
connect the same typed AnalysisActions bridge to the actual production sink.
This segmented proof avoids exporting private production APIs solely for tests;
it is not a full cc_library loading or DICE integration claim.
Core proofs cover cross-owner sharing, differing configured input rejection,
mixed-family rejection, owner-context independence and requested-output runtime
gating. Compile direct dependents slug_loading_v2, slug_analysis_v2,
slug_core_v2 and slug_cli_v2; no binary oracle invocation is planned.
Tests expected seconds; no daemon or broad suite required for this constructor.
Run formatting, git diff --check, plan status and archive checks; record exact
selection/pass counts, elapsed times and final review, then commit accepted work.

Correct implementation/invocation failures within this contract. REPLAN only if
source contradicts the selected path/ownership model or safe admission requires
new configuration provenance, runtime execution or full C++ prerequisites.
M7A remains partial and M8 unproved. Packet began approximately 17:54 UTC.
Independent design review ACCEPT: full typed identity, naming projection and
separate execution gates accepted; final review protects exact filename predicates.

Preparation note: initial combined Core no-run preparation reached its 60s cap
after compiling dependencies and starting Core code generation; no compiler error
was emitted. The wrapper originally killed Cargo but left rustc briefly alive;
it exited before the targeted cleanup. Further operations supervise a dedicated
process group. Split Loading/Analysis test preparation from Core's now-warm
dependency graph, retaining the same 60s operation cap. This is a resource stop,
not test evidence or a semantic failure. Test growth exceeds the initial estimate:
separate binder/sink scaffolding and negative matrices stay in bounded child test
modules; no production test export or copied fixture corpus is introduced.

## Validation receipt

All commands use the resolved pinned toolchain, RUSTC/RUSTDOC paths, offline
locked Cargo and serialized target/debug preparation. Final no-run commands:
cargo test --offline --locked -p <crate> --lib --no-run --message-format=json.
BuildAPI exit0 4.183s plus final-source 0.465s; Analysis exit0 45.018s;
Core exit0 32.541s after the resource split; Loading exit0 31.983s and final
test-scaffold correction 3.823s. Earlier Loading compile exit101 36.287s was a
test-only OwnedFrozenValue accessor error, corrected to a retained handle/value().
Its first test run passed2/3; the unchanged helper's unused depset global was
missing from authored scaffolding, corrected to an inert fail-if-invoked binding.
No implementation assertion or unchanged source was weakened.

Final preflight-selected tests all pass: BuildAPI4/4 0.00s, Loading3/3 0.00s,
Analysis3/3 0.00s, Core5/5 0.01s. Each harness was preflighted after its last
rebuild with scripts/v2_test_preflight.py <executable> --exact <all named tests>,
then executed as <executable> --exact <same names> under a 10s cap.
Exact selection is the four tests in actions::solib_symlink::tests, the two
cc_common::solib_tests tests plus
subrule_invocation::native_solib_token_tests::inactive_native_solib_actions_never_reach_sink,
the three starlark_rule::solib_tests tests, and the three
runtime::action_prerequisites::tests::solib_tests tests plus protected
runtime::configured_action_closure::tests::{configured_action_conflicts_compare_raw_platform_not_merged_owner_properties,
configured_action_conflicts_unresolved_long_unicode_and_distinct_configuration}.
Harnesses: slug_build_api_v2-18e71ad104d60d52,
slug_loading_v2-db3f96e2264887f3, slug_analysis_v2-6df0dd9f40d73962,
slug_core_v2-1bb9a93f43ab610d under target/debug/deps.

cargo check --offline --locked -p slug_cli_v2 --message-format=json passes exit0
26.300s, covering named direct dependents plus Query/Commands/REAPI/Server/CLI.
No Slug binary, daemon, Bazel query/build or broader suite ran. Cargo emitted only
existing unused perf-event patch notices. Command/compile logs and JSON receipts
remain under target/wp756; this receipt preserves compact durable evidence.

Independent final review ACCEPT. Pinned rustfmt checks on all 14 changed Rust
files, git diff --check, plan status and archive checks pass. No new fixture
files; the existing authenticated source is explicitly declared as test compile
data. Approximately 21 minutes elapsed (17:54–18:15 UTC); compile preparation
including resource-stopped/error attempts and CLI check totals about 241s,
focused test harness time 0.01s, independent reviews approximately six minutes
overlapped with implementation/validation. No semantic REPLAN.

Observable gate advanced: production-demanded native False/False construction
and structural action sharing now exist; execution remains explicitly rejected.
Next resolve downstream consumption of the two observed Solib outputs from
existing evidence or one bounded input-filtered production action observation.
That selects native execution admission versus the separately missing C++
analysis prerequisites without assuming registration proves execution demand.
Do not open full allocator compilation or a broad production build implicitly.
M7A remains partial and M8 unproved.
