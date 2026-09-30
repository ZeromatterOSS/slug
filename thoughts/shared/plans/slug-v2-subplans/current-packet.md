# Current Slug V2 Work Packet

Packet: WP-7-163-m7a-immutable-link-args-r1
Status: accepted bounded slice; corrected independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome, compatibility and ownership

Advance unchanged authentic Linux Rust-toolchain cquery past cc_internal.
get_link_args at rules_cc finalize_link_action.bzl:352, using existing native
feature/variable expansion and generic Args action publication. Baseline
WP862 a07544523. Pinned Bazel9.2 8220c619 CcStarlarkInternal.java:408-440
requires four named-only arguments action_name:string, genuine FeatureConfiguration,
genuine CcToolchainVariables build_variables, parameter_file_type:string|None;
no caller/context/action gate. Args.java:119-240 returns immutable unhashable
Args with all five mutations rejected, not a list or mutable args.
LinkCommandLine.java:93-153 expands features without toolpath, filters EVERY
argument containing linker_param_file converted by the existing native string
projection even with None (empty filters all). Lookup/string conversion precedes
feature expansion and preserves malformed-parameter error precedence; Boolean/File
values reuse admitted conversion, sequence/struct errors remain visible. It
then optional always-use policy uses first literal LINKER_PARAM_FILE_PLACEHOLDER,
escapes percent before substitution. Retained flags must pass existing shared
exactly-one-%s validator; multiple literal placeholders are explicitly deferred
rather than sent to the renderer that assumes validation. LinkCommandLineTest:170/187/202/218/275-337
covers library/search expansion, param use, static split/filter/no-split.
Direct get_link_args/FrozenArgs tests not found; adapt pinned regression using
existing small constructors and Analysis fixture. No new copied public fixture.

Exact admitted binder/type/immutability, flag order/filter and supported policy, with registration snapshot timing
classified Slug-native below:
None, UNQUOTED and SHELL_QUOTED reuse existing serializers. GCC_QUOTED and WINDOWS
explicit unsupported/deferred, invalid names/type rejected. Windows stays minimal.
Existing Slug-native output paths/structural identity/Unicode/repr contracts apply.
Full native archive/link actions, callbacks/resource_set, runtime mapped paths,
tree expansion and execution closure remain deferred. A different actual requested
format is a recorded boundary, not permission to approximate its serializer.

Own one immutable traced/freezable evaluator LinkArgsGen<V> retaining existing
CcFeatureConfiguration (shared immutable model), action CompactString and original
variables Value/FrozenValue plus optional existing param format/flag. None does
not expand on construction; string mode expands now to compute policy, then
expands again at consumption, preserving shallow-live/dotted-cache behavior
between construction and registration. None-lazy/string-eager construction timing
is source-exact; expansion into scalar recipes at action registration is inherited
Slug-native snapshot timing. Upstream execution-time deferred expansion and
mutations after registration remain unsupported/deferred.
Freeze uses existing variable freezer; no borrowed heap/scratch retained, new
semantic store/cache/key/lock/global/interner/donor or retained action representation.
Evaluator values release with heap; existing module freeze owns any retained seed.

At Loading write/run/run_shell, AFTER existing receiver/binding/lifecycle guards,
normalize only genuine LinkArgs through current caller heap into existing scalar
EvaluatorArgsSnapshot/StarlarkArgs before unchanged Analysis sink. This adapter
must not expose a new mutable value to Starlark. Shared Args mutation binder
rejects LinkArgs before validation. Later sink-owned output/scope override error
precedence for multiple-invalid requests remains Slug-native at this snapshot
phase; every error prevents registration. Existing mutable Args and wrong-typed entries
preserve behavior. Existing lower_args_snapshot, RetainedArgsRecipe and parameter
policy own action publication, structural equality/invalidation/source/config A/B/A.
Source-exact immutable behavior is bounded to all mutation rejection and module
freeze. Nested struct/provider immutable classification and identity hashing are
unsupported/deferred; no incomplete structural capability or new occurrence
owner is added. No new sink API, action family or repair. DICE owner/locking rules from
 docs/developers/dice.md apply. Stage9 existing generic Args owner remains authority;
only evaluator adapter added, no utility import/performance claim.

## Scope and evidence

Allowlist: Loading cc_common.rs narrow binder; cc_variables.rs child registration;
new cc_variables/link_args.rs and focused child tests; subrule_invocation.rs
narrow shared Args receiver/snapshot construction and guarded consumption hooks;
Analysis tests/cc_feature_expansion/mod.rs child registration and existing
cc_configure_features/mod.rs minimal request_with_route helper (old wrapper unchanged),
new cc_feature_expansion/link_args.rs tests reusing workspace_with and exact creator
sources; existing subrule/link_actions.rs remains protected. Manifest/
canonical Current; compact Stage6 Args/link metadata and matching Stage9 owner note.
No production Analysis/sink/build-api serializer/config/fixture/materialization edit.
Large parent files get narrow hooks; new bounded child owns LinkArgs algorithm.
Estimate production below300 lines. Actual proof is286Loading plus~282Analysis
lines, above the initial300 total estimate: separate native construction/freeze
and real action-consumption/restoration phases require both harnesses. They reuse
existing constructors/request/creator sources; no duplicated copied fixtures.
Review actual responsibility boundaries and discriminators.

Loading discriminators: four named-only binding/genuine inputs; None lazy failure
and mutable list changes; string eager failure then second expansion; no toolpath;
all-occurrence substring filter incl absent/empty/nonstring; first placeholder/%
escape/no-placeholder/two-placeholder refusal; supported serializer/default shell without policy; deferred
formats; all five mutation errors/unhashability/freeze and repeated consuming
original values with no new cache. Protect prior live/dotted/frozen variable cases.
Analysis reused feature-creator fixture adds root/subrule wrapped write and generic
spawn that consume returned Args,
verify owned scalar recipes/policy/bytes and existing action guards. Protect prior
subrule FileWrite/soname/immutable-library assertions without editing their fixture. Same-DICE source/config A/B/A both
Legacy/Observed, target/exec restoration prove original action owner remains.

Independent DESIGN before edits and FINAL actual delta/evidence. Pinned nightly
serial Cargo JSON Loadinglib/Analysis starlark_rule and protective subrule no-run60s prep each, batch exact
nonignored preflight and focused runtime belowfewseconds; Core direct-dependent
check, CLI rebuild. Unchanged originalthreehash authentic fixture supervised180s
cap justified by observed86s sourceprep, actual process checks/cleanup. Scoped
rustfmt/diff/archive/planstatus. No hard user12s ceiling. REPLAN if deferred values
cannot use existing current-heap expansion safely, freeze changes original variable/cache ownership,
or implementation requires another retained/action owner. Routine corrections stay.

## Immediate predecessor

WP862 a07544523 projects specific-tool-files through existing compile-policy owner
and typed option. Independent DESIGN/FINAL,30focused checks0.514s and Core/CLI pass.
Authentic unchanged cquery86.208s clears getter, reaches get_link_args:352;
hashes/cleanup pass. Evidence target/wp862. M7A/M8/full linking unproved.

## Acceptance receipt

WP863 accepts the bounded immutable LinkArgs adapter, not full helper closure.
Corrected independent DESIGN/FINAL ACCEPT. Thirty-seven exact nonignored checks
pass: Loading26 (sixnew, realGC/freeze and prior variable/wrapper/sequence proofs),
protective subrule6, Analysis starlark_rule5; runtime0.592s total. Final serial
pinned-nightly preparation Loading7.50s, subrule16.07s, starlark_rule1.40s,
Corecheck14.18s, CLIrebuild21.49s. Initial compile/helper visibility/GC invocation
and new guard-row errors were corrected without changing production ownership;
original scopeNone checks remain protected. Logs retained target/wp863.

Unchanged authentic cquery exits2 in77.565s: missing get_link_args becomes
explicit parameter_file_type=GCC_QUOTED refusal at the same finalize:352 call.
The helper remains partial; the confirmed next prerequisite is GCC serialization.
Originalthreehashes, actual process cleanup, scoped format/diff/archive/status
checks pass. Packet09:13→09:55 UTC. Full linking/M7A/M8 remain unproved;
execution-time expansion/nested identity hashing/Windows stay deferred.
