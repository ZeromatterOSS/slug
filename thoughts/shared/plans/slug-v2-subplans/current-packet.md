# Current Slug V2 Work Packet

Packet: WP-7-177-m7a-rust-toolchain-captured-helper-implementation-r1
Status: implementing on local review branch; independent DESIGN accepted; FINAL held; M7A remains partial

## Outcome and evidence

Accepted baseline `d9c1de64a` (WP878), including WP876 `64f52f1c4`. Implement WP875's finite four-capture Rust callback
through publication AND complete source-owned helper invocation; activation is
coupled. The authentic ToolchainInfo publication currently refuses nested
make_ccinfo. Consumer arguments remain label/actions/allocator_library/std.
Execute original make_libstd_and_allocator_ccinfo, not a None/precomputed CcInfo
adapter, generic closure serializer or consumer export lookup. WP876 supplies
source-library True/True Solib construction required by the dynamic branch.

Pinned Bazel9.2 `8220c619` ToolchainInfo.java58–99 and
CcToolchainProviderTest.java199–226 establish loaded-function publication/new-thread
invocation (global lower bound, not nested-capture proof). Original rules_rust
0.73.0 toolchain body43,390 bytes SHA c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e:
_rust_toolchain_impl387–663, make_ccinfo560–570, constructor590, field606.
Actual helper is @@rules_rust+//rust/private:rust_allocator_libraries.bzl,
11,932 bytes SHA ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e.
Its ordered library/panic graph, allocator forms, std modes, dynamic and
self-contained branches define invocation behavior. Existing full sources are
Loading test constants; move them byte-exact into shared existing fixture assets,
replace constants with include_str and retain old source-hash/line-count proofs.

Named helper behavior after successful argument/authority binding is source-exact;
function occurrence/publication and configuration/artifact/root identity are
Slug-native. Four ordinary required parameters use existing ParametersSpec binding.
Then a Slug-native genuine/current/live-actions guard precedes helper validation:
actions_to_context rejects foreign evaluation/expired handles, require_active also
rejects suspended ones. Do not require supplied label==consumer owner; original
label semantics remain authoritative. Generic functions, raw frozen code carriers,
full callable reflection/equality parity, Solib/REAPI/link execution and Windows
remain unsupported/deferred. Original helper None case remains valid, not a stub.

## Semantic and lifetime owners

BuildAPI defines a distinct immutable RustToolchainCallable model with source
Arc authority, original occurrence Arc/name hash and four owned AnalysisValues:
cc_toolchain, feature_configuration, experimental_link_std_dylib, rust_std.
Validate outer shapes only (None/provider, None/feature configuration, Boolean,
ConfiguredTarget); preserve helper provider-membership validation timing. Authority
retains caller source plus complete helper manifest: canonical labels, normalized
workspace paths, repository mappings, ordered direct children, reachable source
digests and fingerprint. Keep Starlark/code/heap/frame/token/actions out of BuildAPI.
Arc/Dupe/Allocative, existing compact maps and separate publication occurrence
bijection apply; compare every authority/capture and cross-field alias in one state.
Public occurrence equality/hash remain distinct from semantic publication cutoff.
No global cache/interner or donor import. Stage9 existing callable/frozen-module
rows are the reuse basis; record the distinct model/phase binding disposition.

Loading adds one finite preparation API below bzl_module, using existing private
canonical repository route and ExternalBzlModule eval/observation keys. Request
Producer(observed caller provenance) versus Retained(full authority), mode
Legacy/Observed. Reload caller through its DICE owner and validate full provenance,
fixed canonical label and SHA; don't trust public caller path/mapping fields.
Load fixed helper through the same canonical route, authenticate root SHA/public
export and derive actual complete helper manifest. Retained request requires exact
fresh authority equality. Need and observation/frontier errors remain distinguished;
no new key, transaction, source read, mapping reconstruction or lock across await.
Prepared binding owns FrozenBzlModule/export and source metadata as phase memory;
installation adds the defining heap reference. Code handles are lifetime-only.

Analysis prepares producer birth even with no incoming callback, and recursively
discovers retained authorities in values passed to both materializers (dependencies,
Targets/provider fields, toolchains/groups, hidden configured attributes, containers,
dictionary keys+values, depset successors, CC variables/captures and Rust captures).
Deduplicate authority requests independently from callable occurrences. Preparation
occurs before synchronous finish_analysis in both routes. Pass phase bindings to
both materializers and evaluator context; rematerialization requires matching full
authority, adds heap references and reconstructs captures. Merge actual source
provenance collision-safely: deduplicate identical rows, refuse conflicting filename
or canonical identity/digest/mapping. Private calls see actual helper/transitive
source frames and consumer root/token; no forged global caller privilege.

At genuine ToolchainInfo construction Loading authenticates observed AND evaluated
caller SHA, canonical source, unique filename, direct _rust_toolchain_impl frame
and constructor callsite (zero-based589). Compare field function to directly calling
make_ccinfo local, then read four sibling locals via native_call_local_value. This
API unwraps cells but grants no source authority. Separate traced registry roots
original Def, descriptor and capture values through GC/freeze; snapshot only after
evaluation stops. Missing/malformed capture/source/binding fails closed. Runtime
proxy traces/freezes captures and owns referenced helper code through phase-binding
and producer disposal. Preserve original/proxy function equality/name hashing and
alias partitions across republication. Invoke actual helper with four captures plus
consumer args; never retain producer ctx/actions or relower untracked foreign heaps.

## Scope, gates and recovery

Allowlist: BuildAPI new rust_toolchain_callable.rs/tests, analysis_value.rs, lib.rs;
Loading bzl_module.rs (module declaration only), new bzl_module/rust_toolchain_helper.rs
and tests, new rust_toolchain_auth.rs/runtime proxy/tests, evaluator_roots.rs,
provider.rs, subrule_invocation.rs, lib.rs; Analysis analysis_value.rs/new tests,
dice.rs, starlark_rule.rs and focused rustc_map_each integration modules;
shared fixture toolchain.bzl/rust_allocator_libraries.bzl plus existing pinned
triple.bzl/semver.bzl/incompatible.bzl leaf relocation, sources.rs/sources.json/
fixture.toml and Loading host_package_load_tests.rs include_str/source tests;
Core runtime/tests/cquery_command_tests.rs for two genuine terminal-policy
discriminators, new runtime/tests/rust_toolchain_callback_tests.rs,
rust_toolchain_callback_fixture.rs and compact rust_toolchain_callback_sources.json,
and one
#[cfg(test)] child hook in runtime/dice.rs for the coupled proof migration; manifest/canonical/Stage6/Stage9 contracts. No CLI/executor/engine changes or
third-party body edits. Large existing files get local hooks/signatures only; new
source/preparation/runtime/tests live in cohesive owner modules. Estimates production
under900 lines, proof under800 plus byte-exact source relocation; review triggers.

Independent DESIGN before implementation and FINAL against actual diff/gates.
BuildAPI proves every authority/manifest/capture field, same/split occurrence and
cross-field capture aliases, invalid shapes and complete publication A/B/A.
Loading proves exact caller/frame/callsite/helper/private authority refusals, traced
registration GC/freeze, ordinary4 binding invalids and helper code lifetime.
Analysis proves genuine original/proxy equality/hash, producer heap disposal,
nested aliases/republication, source/helper/import/config restoration in both routes
and consumer-owned actions/current-root guards. Full source helper invocation gates:
nonempty library ordering/panic filters, allocator cc_info vs implementation info,
std/no_std_with_alloc, original invalid-provider/mode error order after guard,
empty None, dynamic std Solib and self-contained real objects with enabled archive
AND nodeps-dynamic action configs/tools. Do not change helper defaults or substitute
empty objects to waive a branch. Conditional interface/LTO/stamp prerequisites and
Core resource_set execution refusal remain named unsupported action boundaries;
if a required construction gate hits one, resolve it before coupled activation.

Reuse existing CC callable lifecycle/source fixtures and original full source.
Source-asset growth is five moved files with old bodies removed; review provenance
and hashes. Serial pinned no-run Cargo JSON prep60s, exact nonignored preflight,
focused executable groups20s; direct Core/CLI compile and rebuilt authentic query
with unchanged workspace hashes/supervised cleanup. Authentic >30s is justified:
it is the only existing genuine full Rust toolchain producer/publication boundary;
query alone never proves callback invocation. Broader suites only at integration.
Fmt/diff/plan/archive and actual pre/post process checks. No performance claim.

REPLAN for raw code in retained semantic data, unmodeled helper authority/lifetime,
source fallback, changed compatibility class or unavoidable unadmitted action
prerequisite. Preserve substantial unfinished candidates on a local review branch
with base/receipts; don't activate publication to bypass missing invocation proof.
Root resolves authorized architecture choices; no user permission needed.

## Predecessor

WP876 `64f52f1c4`: source/generated Solib and all flag pairs, current consumer
owner/root, complete identity/conflict and source/configuration restoration;
independent DESIGN/FINAL,17 tests0.169s/Core/CLI/hygiene pass. Runtime Solib remains
unsupported. WP875 finite callback owner accepted at8c427a879; remaining seam
research target/wp876/callback-seam-notes.md, no runtime evidence claimed there.

## Candidate checkpoint (not acceptance)

Preserve the substantial implementation on local branch
`review/wp877-rust-toolchain-callback`, base `64f52f1c4`; no main activation.
Independent source review reports no material code blocker. Growth exceeded the
900/800 estimates (approximately1,350 production/1,800 proof lines): reviewed as
cohesive BuildAPI semantic model, Loading preparation/authentication/runtime and
Analysis traversal/materialization obligations. Two original source assets moved
byte-exact with their old raw bodies removed; no helper implementation copied.

Pinned nightly2025-09-14 serial compile/exact-selector receipts in `target/wp877`
record25 distinct passing checks: BuildAPI4, Loading preparation/source6,
Analysis ownership4, Loading authentication/runtime/lifecycle9, actual helper
branches2. The lifetime check passed after correcting its public-export probe;
retain earlier failure receipts. Core/CLI compile passes, including rebuild after
removing the temporary diagnostic (12.697s). These focused checks do not establish
the genuine coupled publication/invocation gate.

## Remaining coupled integration gates

Accepted WP878 d9c1de64a is merged into this review branch at13a854000.
Its archive correction resolves the exact rules_jvm_external6.7 relative
.bazelversion link refusal. The rebuilt unchanged authentic producer query now
fails final effect selection: unavailable ConfiguredNodeAnalysisObservationKey
root DiceNodeId(engine1,node1094),142.717s/exit2. Temporary root-lifecycle
instrumentation observed transient completion during retries, but reached the
180s operation cap before terminal selection. A reviewed terminal-only replay
then completed144.837s/exit2: actual cquery terminal Ok, unchanged strict policy,
and the same unavailable analysis root at selection. This rules out a hidden
terminal query error but does not identify the invalid dependency/persistence
cause. Both diagnostics were restored byte-exact; clean CLI rebuilt12.697s.
Workspace BUILD/MODULE/lock hashes remain unchanged.
Receipts: target/wp877/authentic-activation-closure-failure.* and
activation-diagnostic-summary.json and authentic-terminal-only-diagnostic.*.
Strict selection policy is unchanged; DICE propagates child transience even when
a parent returns a successful value, so swallowing an invalid child is a current
source-investigation hypothesis, not an established cause.

Five original body relocations are byte-exact with old constants removed;
three additional leaves pass protected Loading checks3/3 in0.008s. The actual
normal-preparation paths.bzl transitive import A/B/A passes both routes1/1
in0.565s, with genuine caller provenance, manifest membership, stale Retained(A)
refusal under B and exact authority restoration. Original direct-helper/source
branch checks remain complementary to genuine coupled invocation.

The Core coupled test has genuine original rust_toolchain/stdlib,
registered C++ toolchain, native platforms host and fresh consumer assertions,
including current-owner/configuration Solib paths. Original built-in registration
expansion uses the complete rules_shell/shell and C++ compatibility proxy
load closure. The authored inputs use the required linker_inputs parameter,
producer-package .rlib siblings and the genuine libstd-*.so classification
(libstd-fixture.so); no third-party source implementation changes.
No original third-party bodies, built-in registrations or required gates may
be trimmed. Independent DESIGN accepts the private Core-owned relocation in
target/wp877/core-coupled-proof-design.md with observations=None in both routes,
strict ClosureOnly/default acceptance, and explicitly authored minimal module/
registry metadata whose mappings/extensions are derived by the normal owners. Core owns production generated
repository writer/session association; no new public API or Analysis->Core
dependency is allowed. A closed archive-member/mapping inventory must precede
asset work; tracked full archives are reused, without duplicate source bulk in Git.
Root independently validates all6tracked archives/103member hashes. The frozen
47additional members total418603bytes/9954lines in scratch; no new archive blobs
or duplicated third-party implementation bodies enter Git. Required apparent
label mappings (including shell platforms) are normal MODULE dependencies.
The single coupled test/assertions now live in Core; its old Analysis Rust file
and hook are removed after the Core gate passed. The shared authored .bzl probe
remains the single source. No duplicated passing gate
or partial coupled activation. Runtime20s and preparation60s remain fixed. Its observed adapter must use the existing strict DICE closure;
Analysis does not expose a path epoch, so none may be reconstructed.

Two test-only Core cquery discriminators exercise genuine transient Analysis
failure with the existing permitted policy versus late Evaluation failure with
a verified nontransient strict closure. Independent FINAL accepts2/2 exact nonignored checks in0.016s after serial
no-run51.219s (first60s preparation cap preserved separately). These cannot classify
the authentic final terminal. The independently reviewed terminal-only diagnostic ran once under180s,
identified Ok before strict selection refusal, and was removed/rebuilt byte-exact. No engine, key validity or acceptance relaxation is allowed.

FINAL remains held for genuine producer -> retained model -> fresh prepared
helper -> actual consumer invocation in both routes, unchanged authentic producer
success and the subsequent actual consumer replay. Current main admits only
WP878, not callback activation. Windows execution remains deferred. Inherited
oracle growth was reviewed read-only; reset only at an accepted checkpoint.

## Core coupled proof checkpoint (test acceptance; production held)

Independent DESIGN/FINAL accepts the private Core adapter, closed assembler,
source initialization and preserved semantic assertions. Actual text/raw/directory
observations refresh through the existing request revision owner before each
request, without evaluating unrelated semantic roots. CC/features/platforms are
real selected registry sources: deterministic scratch tar.gz archives contain
validated original bodies plus authored minimal MODULE/BUILD metadata, actual
SRI/file URLs and ordinary source associations. Rust and other supported mutable
body sources retain local overrides for actual import A/B/A. Archive TempDir
guards live across the whole request matrix and clean scratch archives afterward.
No new production owner/API/validity policy, generated output, mapping, recipe,
receipt or source certificate is fabricated.

The cold/fresh comparison exposed a real operational input change: only /tmp
Lstat changed in Host and seven materialization namespaces as native writers
created immutable repository roots. The same observed CC module key reevaluated;
only its module-global needs_pic_for_dynamic_libraries occurrence changed among
62 provider fields. Registration/lowering/rematerialization preserved identity.
No semantic equality weakening was justified. A reviewed single genuine request
with complete static assertions establishes the warm-source baseline; first/fresh
requests then require nonempty shared observation coverage, the implicated Host
/tmp Lstat and equality of every shared path value before publication equality.
This proves stable-input reuse, not cold/warm global function identity.

The complete exact nonignored Core gate passes both routes, six aliases, original
helper invocation after producer disposal, fresh static/dynamic consumers,
current-owner/configuration Solib and complete configuration/probe/import A/B/A.
Warm baseline receipt: compile6.076s/exit0,1selected/1passed4.571s.
Temporary module activation audit is removed; strict ClosureOnly/default terminal
selection is unchanged. Final cleaned source compiles6.326s/exit0 and exact
nonignored coupled/policy group passes3/3 in4.321s; the Analysis direct integration
dependent compiles9.286s/exit0 after duplicate removal. Receipts are in
target/wp877/core-callback-relocated-owned-archives-* and
analysis-callback-relocation-dependent-*; earlier attributed failed
receipts remain under core-callback-* (including observation-discriminator).
Authentic producer/consumer replay gates remain required before production
activation. No main merge is authorized by this test checkpoint.
