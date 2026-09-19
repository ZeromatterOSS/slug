# Current Slug V2 Work Packet

Packet: WP-7-53-m7a-cargo-runfiles-args-r1
Status: accepted

## Outcome and source basis

Admit the production-demanded Cargo `_runfiles_map` Args callback as a structural retained
recipe, including its captured fake executable and workspace name. The unchanged pinned
helper must produce correct filtering, ordered mapping strings and forced parameter bytes;
source/capture changes must invalidate and restore the retained result. WP752 is accepted
at 74770f888; its alias execution/publication contract remains there in current-packet.md.

Saved target/wp714/actions.json SHA256
ffc50a44648f85b464988eb5dd6c6dd152269f342121a006839d3e8c7b4c0be8 contains 66
CargoBuildScriptRun actions with 4865 mapping arguments: 4783 external sources, 16 main
sources, 61 external generated files and five main generated files. Each mapping input
resolves to a non-tree artifact; no Directory callback demand is established. The two
SolibSymlink actions require separate C++ native construction and remain deferred.

Pinned rules_rust 0.73 cargo/private/cargo_build_script.bzl SHA256
6147df938723ec58cef646169814c85a72fa0f74080471756e3c1891d02f4630, lines 307–311
`_rlocationpath`, 343–346 `_runfiles_map`, and 351–357 Args construction establish semantics:
filter exact fake_exe artifact; map path + '=' + rlocation, stripping one leading '../'
from short_path for external sources or prefixing workspace_name otherwise. Preserve the
merged depset traversal and forced `--cargo_manifest_args=@%s` parameter-file policy.
Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a Args.java:366–388 permits
explicit allow_closure; StarlarkCustomCommandLine.java:1125–1180 maps in input order and
omits None. Reuse accepted WP720–725 vector options/parameter-file and source-provenance
contracts. This is not general callback or closure admission.

## Contract and owners

Exact named behavior: pinned callback filter, ordering and string algorithm over the
admitted File projections; retention/equality of all captures; existing vector formatting
and forced parameter encoding. Source path/short_path use accepted nonsibling repository
semantics. Derived path/short_path and structural configuration/output identities retain
the existing explicit Slug-native projection, including external generated files. Do not
claim exact Bazel output-root/runfiles bytes or repair artifact paths inside this callback.
Noncallback Directory vectors with expand_directories=False render the literal typed
artifact path; default/true expansion and Directory callback inputs remain unsupported.
Directory expansion, arbitrary closures, add_joined callbacks, full Cargo rule/toolchain
execution, native C++ construction, broad aquery, exact ActionKeys and Run remain deferred.

Authenticate the unique loaded canonical Cargo source row, its observed digest, evaluated
full-source digest, exact add_all callsite, immediate `_create_runfiles_dir` caller and
`_runfiles_map` function before reading captures. The pinned call uses allow_closure=True;
all other callback admission rules remain unchanged. Source edits/relabels/missing or
ambiguous provenance fail closed. An empty source does not bypass authentication.

The real token-checked Analysis context exposes workspace_name as the Bzlmod-only '_main'
runfiles prefix, matching existing RetainedRunfiles. Pinned BazelRuleClassProvider.java:232,
PackageFactory.java:231/299, RuleContext.java:383–384 and StarlarkRuleContext.java:714–716
establish the package-owned fixed prefix. No command option or ambient state participates.

A narrow lifetime-bound evaluator accessor may expose one immediate Starlark caller local,
reusing the existing Def/FrozenDef slot lookup and captured-cell unwrapping behind
native_call_context. No module-scope or unrelated-frame fallback. Keep this accessor
source-agnostic; the Loading owner authenticates the caller. Copy workspace_name and keep
fake_exe in a traced evaluator-local Args snapshot before the caller exits. A Value must
never enter the existing unsafe-ignore/Copy mapper enum. No closure, evaluator, local-frame
reference, arbitrary user callable or heap owner survives action publication.

Analysis lowers captured fake_exe to exact AnalysisArtifact and workspace_name to compact
owned string, and lowers the input depset with the existing shared AnalysisValueLowerer.
Build API owns a focused validated Cargo runfiles recipe retaining ArtifactInputs/depset
DAG and those two captures, with Allocative and structural publication equality. Filter by
artifact identity, never rendered path; same-path different owner/config remains distinct.
Reject nonregular inputs/captured fake exe and FilesToRun in this recipe. Rendered strings
are action-expansion scratch, never retained flattened depset/cache state. Options apply
after mapping/filtering through existing RetainedVectorArg machinery.
Filter only rendered arguments; preserve all_runfiles_files as the action input depset.

Existing configured-analysis DICE values own retained actions and source dependencies;
publication equality includes workspace, fake artifact, input DAG/alias topology and options.
No new DICE key, cache, interner, lock, filesystem read or observation bypass. Existing
need/error and source-revision machinery remains authoritative. Read docs/developers/dice.md
and Stage9 rows for retained Args/depset/CompactString/Allocative and source provenance.
Use the utility-reuse skill; no donor implementation or performance claim is needed.

## Scope and proof

Allowed: starlark-rust/starlark/src/eval/runtime/evaluator.rs and focused test child;
Loading subrule_invocation.rs plus focused Cargo capture/provenance helper/tests;
Build API actions focused cargo_runfiles_args child, spec.rs/mod.rs/lib.rs exports and tests;
Analysis starlark_rule.rs routing plus focused lowering/test child and the focused public
workspace_name integration child/registration; a focused authored
source-proof fixture and its provenance, reusing existing rustc-map-each imports when useful.
Root owns manifest/canonical and Stage6/7/bootstrap/Stage9 contract notes.

Prefer focused children. starlark_rule.rs exceeds 2000 lines: add only dispatch there and
keep new recipe/lowering logic in its child. Existing evaluator and Loading owner remain
cohesive for narrow capture access/dispatch. Estimate 250–450 production and 300–650 proof
lines plus only indispensable verbatim source fixture growth; review scope if exceeded.
Do not append test exports to authenticated Cargo source. Use a test-only private symbol
lookup from its unchanged loaded module and real Args snapshot/lowering; classify authored
context scaffolding honestly. Imported helpers keep full source/provenance authentication.
Fixture manifest records source SHA, pinned Bazel anchors and exact/structured comparison.
Inspect load closure and avoid copying unrelated files or another full corpus.

Discriminators: main/external source and generated files, exact fake artifact filtering,
same-path/different-owner retention, depset order/dedup/shared-node identity, workspace/fake
capture changes and A/B/A publication, retained snapshot after caller exit/GC, invalid
captures and Directory rejection, spoofed/mutated source and generic closure rejection.
Prove actual unchanged helper invocation, lowered owned recipe and forced paramfile bytes.
Protect existing native_call_context, Rustc pinned callbacks and ordinary noncallback Args.
Compile direct consumers CLI/server after the public recipe/snapshot shape change. Reuse
existing REAPI paramfile evidence since transfer/expansion code is unchanged.

Independent design then final review. Pinned nightly-2025-09-14 offline compilation separate
from exact-selector tests; serial shared-target Cargo and ancestor-observing tests. Root
coordinates execution. Tests should take a few seconds; longer tests run only if needed,
and >30s needs strict necessity. Preparation cap60s per operation. No Bazel/F3 replay or full
production build. Format/diff/plan/archive checks and target/wp753 receipts; commit and push
at acceptance. REPLAN only if bounded capture ownership/source proof cannot satisfy this
contract, not for routine invocation/compiler corrections. M7A/M8 remain incomplete.

Independent design review ACCEPT: traced captures, source authentication and retained
identity are bounded. The source proof may use documented inert import scaffolding with
the unchanged full Cargo file, real executed typed dependencies and private test lookup;
it does not claim full module loading. The fixed workspace_name getter amendment is independently accepted.

Independent amendment ACCEPT: the unchanged helper first requires
args.add_all([runfiles_dir], expand_directories=False), which failed against the existing
unconditional Directory rejection. Admit literal Directory paths for noncallback sequence
and depset vectors with expansion disabled, retaining artifacts and option equality. Pinned
StarlarkCustomCommandLine.java:328–348,427–439 gates expansion before path rendering. Add
focused public sequence/depset false positives and default/true negatives; keep Directory
callback rejection. Root owns tests/directory_args.rs and its registration. This prerequisite
is necessary to execute the unchanged helper, not broader Directory expansion.

## Acceptance receipt

The unchanged pinned helper now publishes the retained Cargo mapping recipe, preserves
the original action input depset, filters only the exact fake artifact, and emits forced
shell-quoted parameter bytes. The helper leaves Bazel's default SHELL_QUOTED format intact:
Args.java:270–290, ParameterFile.java:82–98 and ShellEscaper.java:58–65,98–110 at the pinned
commit establish single quotes around the '=' mappings. Correcting the initial unquoted
test expectation changed no production behavior.

Twenty exact-selected tests pass (target/wp753 receipts): evaluator-locals-r1 2/2 in
0.007s; cargo-recipe-r2 4/4 in 0.002s; loading-capture-r4 3/3 in 0.011s;
cargo-helper-r3 3/3 in 0.054s; analysis-integration-r2 8/8 in 1.803s. These cover the
immediate-frame accessor, actual GC tracing, structural captures/shared depsets, unchanged
source helper and its authentication/invalid captures, public workspace name and Directory
literal/default/true behavior, source-change restoration and existing Rustc/generic Args.
Pinned offline CLI/server compilation passes in 44.628s (cli-server-build-r1); this is
separate preparation, not test runtime. Final helper preparation passes in 3.522s. Initial
Directory prerequisite and shell-byte assertion failures are preserved with their corrected
receipts. No broad suite, backend test, Bazel replay or full production build was run.

Independent final source review accepts ownership, compatibility and proof growth:
approximately 361 production additions remain within the 250–450 estimate; 1,113 authored
proof lines exceed the 300–650 estimate because evaluator/GC, retained identity, unchanged
source/authentication and public amended surfaces require distinct tests. The only copied
fixture is the indispensable 919-line unchanged Cargo module plus its 202-line license;
inert scaffolding and snapshot workspace mutation are explicitly labeled in fixture.toml.
Full Cargo loading/toolchain/execution and exact Bazel derived paths remain unproved.

Final formatting, diff, plan and archive checks pass. Source and validation hashes are
recorded under target/wp753. The next work is the demanded native C++/Solib construction
prerequisite inventory; WP752 execution support alone cannot register those native actions.
WP746's baseline diagnostic defect remains open; M7A is partial and M8 remains unproved.
