# Current Slug V2 Work Packet

Packet: WP-7-66-m7a-coherent-output-roots-r1
Status: accepted

## Outcome and compatibility

Give existing generated artifacts one coherent root/location through File, Args,
actions, REAPI and physical publication. Expose immutable File.root, File.owner
and ctx.bin_dir/genfiles_dir to unchanged rules_cc/rules_rust helpers.
WP765 at d6ed76015 accepted context recovery/naming/provider classification with47
tests and Core/CLI checks; roots were explicitly deferred.

Exact: admitted nonsibling path relationships/repository placement, root type,
fields/repr/hashability, merged bin/genfiles, context lifetime and File.owner.
Slug-native: full structural configuration/artifact/root identity, workspace-scoped
logical source-root identity and output tokens, not Bazel physical Root equality
across workspaces. Exact configuration/output bytes remain M9.
Deferred: sibling layout, split genfiles, direct external generated-label target requests, execution path mapping, alternate-root
configuration declarations, compute_output_name_prefix_dir/declare_compile_output_file,
CppCompile/discovery, new action families and full bootstrap. No empty derived
root, fallback root, or dual legacy/rooted path mode.

## Research and design checkpoint

Pinned Bazel9.2 source8220c6198837d5c13d53fea211cf3282aa12408a:
ArtifactRoot.java103–122,159–170,203–220,255–303; Artifact.java455–470,
482–520,636–666; FileApi.java70–82; FileRootApi.java26–37;
OutputDirectories.java127–157; StarlarkRuleContext.java789–797;
PathMapper.java208–220; CoreOptions.java88–95.
Unchanged54-body corpus: cc_helper_internal.bzl365–377,
cc_compilation_helper.bzl35–48, compile.bzl284–285, utils.bzl520–534/954–956,
rustc.bzl920–926/1990–2005. Adapt public source contracts rather than Java
allocation/interner assertions. No new upstream body or oracle is needed for
source-defined relationships; actual transport still needs execution proof.

Stage3's coherent-root section owns the shared contract, frozen after independent
design review. ActionOutput owns a mandatory derived-root descriptor and relative
coordinate; path() is execroot-relative everywhere. Root configuration is separate
from action owner; complete SlugConfiguration supplies both identity and projection.
No opaque-owner-byte decoding, command-side argv rewriting or filesystem inference.
Ordinary external output producers add external/<repo>; Solib remains explicitly
bin-relative and can be external-owned under shared bin/_solib.
Source roots have empty displayed paths but canonical-repo logical identity.

Existing ConfiguredTargetKey/source certificates are the DICE producers; roots
and outputs enter existing configured provider/action values and publication.
Read docs/developers/dice.md. No new key, lock, request policy or historical
snapshot assumption. Existing retry/final source validation and collision claims
remain. Legacy identity-only keys stay; existing production action analysis already
rejects them. Test outputs use explicit genuine structural root configurations.

Independent design ACCEPT on2026-09-21: mandatory rooted outputs, independent
action owners, source/external/Solib projections and hardened staging form one
coherent contract. Acceptance remains gated on the complete transport proof.

## Scope and routing

BuildAPI worker: new artifact_root.rs/tests; analysis_value.rs root/value variants;
actions/{spec,ctx_actions,registry,solib_symlink,spawn_command_line,reapi_projection}.rs;
ctx.rs, lib.rs, runfiles/layout.rs and direct tests in that crate. Generic exhaustive
value dispatch sites receive root hooks only. Shared root code goes in a child
module, not large spec.rs/analysis_value.rs.

Analysis/Loading worker: Loading root facade, subrule_invocation.rs File getters and
exports; Configuration native/configuration.rs merge-genfiles projection only;
Analysis analysis_value.rs root bridge, starlark_rule.rs declaration/ctx
hooks, starlark_rule/cc_context.rs, dice.rs generated reconstruction,
runfiles_support.rs, result.rs same-root action admission; their direct constructor
tests. Add authored roots integration
callers in tests/rustc_map_each, reusing unchanged bodies; do not replace providers,
schemas, callable authority or upstream helpers.

Root: Core configured_output.rs, configured_action_closure.rs,
runfiles_manifest/paths.rs, action_output_staging.rs and plan/linux children,
action_chain_staging/{runfiles,symlink}.rs, action_prerequisites/requested_artifacts
projections, dice.rs run projection; REAPI action_chain.rs and binding/artifact_symlink/
output_staging/result children, executor.rs, source_spawn.rs, requested_build.rs,
input_tree.rs as required by rooted paths, with focused tests. Core
file_write_aquery_text.rs renders the declared exec path once while retaining
the root-relative action description for its existing main-target slice. Core
action_chain_execution.rs claims roots before all transport effects. Server reapi.rs
uses declaration-checked configured materialization, never workspace-wide writes;
REAPI exports the typed bridge and Server joins direct-dependent compile coverage.
Other direct call
sites listed by rg for ActionOutput::new/CtxActions::new/RuleContextBuilder::new
in app receive only mandatory signature/expected-coordinate updates.
Root owns scheduling and Stage3/6/9/bootstrap notes, serial Cargo and integration.
Reviewer owns independent design and final actual diff/evidence review.
Estimate500–1000 production lines plus cross-crate updates/proofs; growth is a
review trigger, not permission for partial landing or adjacent cleanup.

## Memory, identity and safety

Use retained Arc/Dupe/Allocative/CompactString/SmallMap utilities; no donor import
or interner. Derived root and ActionOutput are shared immutable owners. Retain one
full output path with a root-relative borrowed projection, not two copied paths.
Share existing SlugConfiguration Arc. Equality/hash uses full root semantics plus
relative path/kind; artifact action owner stays independently structural.
Published roots own data, never evaluator/context scratch. Existing DICE lifecycle
owns release on cutoff/invalidation/eviction; transport maps are attempt scratch.
No performance or memory-reduction claim; test cheap clones/handle size and full
identity. Stage9 retained provider/configuration rows apply.

Physical staging remains beneath the validated configured root; use explicit
root-relative paths there and rooted outputs in capabilities/result reconciliation.
Do not broaden hardened publication to arbitrary workspace writes. Reject foreign
roots before currently same-configuration transport admission. Conflicts compare
structural roots/coordinates; projection collision guards still fail closed.
REAPI outputs/results/inputs and argv use exec paths; raw argv strings are never
rewritten. Runfiles locations/Solib names use explicit appropriate coordinates.

## Validation and acceptance

New discriminators: main/external source roots with equal empty paths but unequal
identity; shared derived roots across owners/repos; differing configurations and
unused payload/A-B-A; three-component roots and exec-role marker; external ordinary
declared/generated Files through producer providers versus shared Solib; existing
main generated-target reconstruction equals its producer artifact; unchanged direct
external generated-label rejection; File.owner and extracted-root
publication/hash/immutability; expired ctx getters; typed false merge projection and foreign
root rejection. Genuine configured callers exercise unchanged root_relative_path,
_repo_relative_path and Rust exec-role/root slicing helpers. Protect WP765 toolchain/
context, Rustc/linker/Cargo Args, Solib, runfiles layout, conflicts/source paths and
generated-output providers. Args, embedded strings, forced params and REAPI agree.

Core namespace/configuration collision/alias/runfiles/stale destination and source
retry gates remain mandatory. REAPI schema rejects wrong rooted paths/kinds;
generated file/tree verified cache/change/restore and selected publication prove
no doubled root and coexisting target/exec roots.
Pinned nightly resolved via rustup; serial offline/locked Cargo JSON no-run for
BuildAPI, Loading, Analysis, Core and REAPI changed targets; exact batched preflight/
direct execution, dependent CLI check.60s preparation cap per operation, expected
seconds/test, no new user deadline. Reuse NativeLink setup for required transport
proof; justify >30s tests and separate preparation. No user BuildBuddy mutation.
No Slug binary invocation unless rebuilt; clean stale slugd only for selected
daemon-sensitive validation. Format/diff/archive/status, independent final ACCEPT,
atomic commit. REPLAN only for contradicted owner/domain contract or unavailable
root/transport prerequisite; normal compiler/fixture corrections stay in packet.
M7A partial/M8 unproved; activate only when the whole path spine agrees.

Design clarification2026-09-21: inspection of the failing new proof found the
preexisting external-target shape guard rejects GeneratedFile nodes before shared
reconstruction. Independent review and root preserve that unsupported boundary,
rather than admit a new target shape as part of a root representation migration.
External generated/predeclared Files still require real producer-provider evidence;
main generated-label requests require exact producer-artifact/root equality.
Split genfiles is likewise not publicly constructible through admitted command or
transition inputs: cover typed false projection plus actual merged/default and
missing/expired-context guards without adding a flag just for a test.

## Acceptance receipt (2026-09-21)

Base d6ed76015; candidate preserved as bbf325c71 on local review branch
`codex/wp766-output-roots-review`, followed by a test-only Run correction.
Independent review cleared the production change, including the target FileWrite
aquery coordinate correction, then cleared the actual transport evidence and Run
correction design/source. Independent final verdict: ACCEPT for this bounded
surface and closure receipt. The complete candidate is integrated atomically;
the preservation commit alone was not acceptance. M7A remains partial; M8 unproved.
The legacy Run writer remains declaration-checked, not a newly hardened/no-follow
publisher. Hardened publication claims apply only to the existing fd-relative
Core staging path.

Pinned nightly2025-09-14 offline/locked no-run/check receipts and exact-selection
logs are in `target/wp766/`. Latest distinct outcomes:315/315 ordinary selectors
and3/3 supervised NativeLink selectors pass. Core staging/prerequisites/collisions/
requested/runfiles84, REAPI ordinary44
and integration21, Analysis protected35 plus broad12 and root/reconstruction3,
BuildAPI actions34 plus root/Solib/Cargo/runfiles21, Loading7, Configuration merge
projection and Server materialization/pre-execution guard all pass. Other focused
source/FileWrite/provider checks are included in the aggregate. CLI direct-dependent
check passes through Server/REAPI/Core. Three aquery golden/order/semantic checks
pass; only Slug-native fixture configuration/action display tokens were refreshed.
Preparation/check total417.975s; one60s capped combined REAPI preparation timed
out, then split lib/integration preparations passed. Latest successful test runtime
16.004s including real transport; failed invocations are retained, not counted as
acceptance evidence. The prerequisite-to-closure working window is about one hour
(d6ed76015 at22:21UTC); this includes preparation, implementation and review, not a
performance claim. No Slug binary, daemon or Bazel invocation was used.

NativeLink setup: published v1.5.2 Linux x86_64 musl release matching archived CI
source6e63ef9a567ac49c77ab258f3af9331336868bb0, isolated under ignored
`target/wp766/nativelink-setup`; no global installation. Archive SHA256
`4de04063b2d0c7d44bef98be4ea726b9005db54e041b222557753e05bf7d2e22`;
executable SHA256
`26be65952e402bb43a5d027a8dde2d859d14f267c8195694e9d39c8ea2efc95a`.
The supervised helper reuses `tools/v2_oracle_lib/nativelink.py`, exact ignored
selection, a fresh verifying CAS/AC and local worker per test, with teardown and
process-group cleanup. All3 exit0, selected1/pass1 each:

- `action_chain::tests::nativelink_chain_generated_file_tree_cache_change_restore`
  (2.667s): generated File/Directory topology, bytes/digests, cache A/A/B/A.
- `action_chain::output_staging::requested_publication_tests::nativelink_requested_subsets_publish_file_and_tree_from_distinct_producers`
  (0.965s): selected multi-producer files/trees, modes and unselected exclusions.
- `action_chain::output_staging::requested_publication_tests::nativelink_target_and_exec_roots_coexist_through_cache_change_restore`
  (1.115s): real configured producers with the same relative output name, distinct
  target/exec roots, full wire paths, exact physical bytes and no double prefix.

The extra exploratory legacy Run failure is reconciled, not waived. Baseline
d6ed76015 already added OutputGroupInfo/runfiles support to ordinary analyzed
executables, incompatible with the existing DefaultInfo-only guard. The retained
selector `runtime::dice::tests::build_command_tests::resolved_run_view_reuses_exact_executable_filewrite_relation`
now verifies that real-analysis rejection and separately constructs explicit
minimal legacy owners through the unchanged validator. Rooted paths, borrowed
owner/provider, files/runfiles singleton, executable mode, output, closure size and
owner mismatches are covered. Terminal receipt
`tests-1790032883933280748.receipt.json`: exit0, selected1/pass1,0.021s;
Core preparation7.730s. Original failure receipts remain. No production guard,
public constructor, configuration bypass or ordinary Run activation was added.

Plan status, archive-boundary and diff checks pass. Root/external generated-label
boundary clarification above is explicit; no new upstream bodies, DICE keys,
Java runtime, execution-time path rewriting or output-root fallback were added.
