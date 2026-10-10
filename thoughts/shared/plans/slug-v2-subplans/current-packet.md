# WP1096 exact no-op Need-map sharing

Packet: WP-7-308-m7a-no-op-request-map-union-r1
Status: accepted — FINAL gates pass; next normal producer allocation work under source review
Baseline: 60ed16f5630f5354e8de19d8551816295aec5151 (WP1095 accepted)
Acceptance: [WP1096 receipt](wp1096-no-op-request-map-union-2026-10-10.json).
25 fresh focused passes; production/native-profile/FINAL gates pass. ONE native
7.843s supervisor226.69/phase227.55MiB under256, same source252/WorkDeadline;
no frontier advance, speed or full-workload fit claim. First Core preparation
60s cap preserved unproved; remaining stage17.188s passes with original profiles.
Owner: Bzlmod SourcePreparationNeeds::try_union and its existing three immutable
Arc<SmallMap> fields. Loading ordinary external sibling Need accumulation is the
unchanged caller. Compatibility: exact existing request/Need/equality/error/order
semantics; Slug-native storage reuse. Linux priority; Windows deferred.

Outcome: avoid rebuilding unchanged repository materialization, selected-download
and execute maps during normal producer Need unions. Research
[target/wp1095/next-work-frontier.md](../../../../target/wp1095/next-work-frontier.md),
SHA 4e50744a5393656c429b729eb33e3a90d38ee1aa8a598ad49347197f9befb34b,
names source_preparation.rs:336–430 and Loading bzl_module.rs:4916–4950. The
existing path-only sibling accumulation reconstructs three empty maps on its
second Need. This is source-established avoided work, not measured native
frequency, exclusive CPU attribution or a guaranteed speedup. WP1095 native
WorkDeadline detector is diagnostic.rs:296 before batch acquisition; it does not
identify the preceding expensive key or next source. Root-compute windows dominate
the observed prefix; no cap raise or identical probe replay is selected.

Decision needs independent DESIGN: one private borrowed helper returning an
optional original map Arc only for proven no-op cases, then narrow dispatch
around each original typed fallback. Preserve original left on right-empty,
same-map or full-value-equal right subset; both empty retain left. Empty left with
nonempty right shares original right. A proper right superset must execute the
original fallback to preserve left insertion order and first equal request Arc.
Missing/unequal values execute unchanged clone/right-iteration/typed-conflict/
insertion/Arc construction loops. Use existing SmallMap lookup and full PartialEq;
no shortened key/digest/body comparison or whole-Need early return. Minimal generic
trait bounds are implementation detail; no public helper/error abstraction.

Preserve full request identities: workspace/canonical id/RepoSpec label, mapping,
name, attrs and original Local/Immutable/ordered generated plan; download complete
owner/URL/digest/output/mode; execute owner/ordinal/call id/generated repository/
argv/Which provenance/raw environment/cwd/options/complete prefix plan. Existing
semantic owners and DICE equality/validity/invalidation/dependencies remain exact.
Preserve scratch -> environment -> path -> repository -> download -> execute ->
bootstrap validation/error priority, earliest conflicting right entry within each
map and original appended-right order. Earlier families still validate even when
maps can be shared; later bootstrap conflicts still return. Reuse original left
values for pointer-distinct but fully equal incoming requests.

Source allowlist: app/slug_bzlmod_v2/src/source_preparation.rs only. Private helper,
narrow three-family dispatch and adjacent existing owner-test proofs; no Loading
or Core source change, fixture tree, dependency, profile, public API/key/lock,
source vector or host/archive/environment capture. Estimate 20–60 runtime lines
and 120–200 cohesive proof lines; formatting/growth triggers review, not a quota.
No new retained field/String/Vec/body/cache/interner/pool/counter. Borrow, move or
share original strings; use existing scoped interners when scope/cleanup fit.
Original Arc/SmallMap/Dupe/Allocative lifetime remains the owner. Maps retire with
last original/union handles; no special new creditbank or graph-retirement claim.
Stage 9 retained compact map/Arc ownership and WP1094 no-op map reuse are concept
anchors; no donor import. docs/developers/dice.md remains the ownership authority.

Proof in two adjacent owner selectors:
source_preparation::tests::source_preparation_needs_union_reuses_exact_map_owners
and source_preparation::tests::source_preparation_needs_union_preserves_full_identity_and_error_order.
Cover path-only three empty-map pointers; all three actual populated map types for
same/empty/full-Eq-subset reuse; empty-left right sharing; independent equal value
Arcs preserve first left values; proper additions/order/fallback; actual descriptor
identity differences and typed conflicts, mixed family and earliest-right errors,
unchanged earlier environment/scratch and later bootstrap priority. Nonmap fields
still merge. Assert exact map/request pointer identity, representative strong-count
and genuine Weak last-drop retirement, actual size/alignment of Needs/maps/Arcs.
Shared bodies and small existing actual request constructors only; no duplicate
full DICE lifecycle or synthetic image creditbank fixture.

Gates after DESIGN: all source/helper/fixture writers stopped, pinned format and
30-owner source freeze, independent actual source/prelaunch/FINAL review. Serial
nightly-2025-09-14 locked/offline/jobs1 Cargo JSON no-run and current executable
list/batched exact preflight/nonzero pass. Bzlmod two new proofs plus existing
source_preparation_needs_union_environment_names_by_workspace,
immutable_materialization_equality_is_operationally_exact,
selected_extension_download::tests::selected_download_need_union_conflicts_by_owner_and_used_key_provides_request,
repository_execute::tests::execute_need_union_rejects_conflicting_descriptor_and_epoch_is_lease_scoped
plus selected_extension_tool_sources::tests::batch_precharges_union_before_legacy_merge_and_retains_credit_until_last_drop
(7 current exact selectors, verify module prefixes from current list).
Loading fresh existing external_bzl_child_frontier_accumulates_sibling_path_needs,
observed_sibling_need_precedes_later_semantic_error_and_repairs_strict_closure,
earlier_semantic_error_does_not_probe_missing_later_sibling,
external_bzl_synopsis_reuses_across_child_need_and_current_epoch_changes,
observed_prepared_identity_and_finisher_algebra (5 exact current controls).
Core current WP1095 13 protected/generated-file controls, current original-profile
native-feature preparation and production Bzlmod/Loading/Core/CLI check. Loading
ordinary opt0 profile may reuse the accepted test-only WP1094 recovery; native
original Loading opt1 remains unchanged. Preparation 60s, separate uncapped RSS;
split completed dependency builds if needed, preserve failures without gate credit.

Fresh standalone ignored singleton list and ONE changed-source native with original
350 ordered source triples/18 bindings, held noninherited FD brackets, current
caller/observer/source/evidence pins and retired version2 raw64 report. Supervisor/
decoder only ROOT changes from WP1095. Every cap from
[WP1095 receipt](wp1095-local-jdk-file-effects-2026-10-10.json) unchanged:
operation10s/work7/default5/lifetime8/cleanup3; RSS256MiB/AS512MiB;
live-transfer16MiB/decode-write64MiB/source256/registry128/object1MiB/admitted16MiB;
directory128/name1KiB/total16KiB/FSIZE16MiB/log1MiB/prep-report4MiB.
No identical native replay. Record actual frontier and separate supervisor/phase
prefix RSS; partial/invalid process CPU windows remain qualified. Tests establish
avoided map construction, not native speed or full producer memory fit.

REPLAN only if full identities/left ownership/order/error priority or cleanup
cannot fit this private dispatch, or a new owner/key/lock/public API is required.
Routine fixture/compiler/selector corrections stay within contract and preserve
first evidence. Pinned format/archive/diff/plan checks, compact receipt, Stage 9
reuse note and accepted commit. Full producer/time/whole-workload fit/Rustc/actions/
certificates/bootstrap remain open; Goal ACTIVE.

Predecessor WP1095 accepted in 60ed16f56: bounded local JDK File-only/current epoch
test admission; 13 focused, original-profile/native/production and FINAL gates
pass. ONE 7.600s native has supervisor227.727/phase228.320MiB under256, source252,
350 stable held FDs/77 pins/v2 status0 and WorkDeadline at caller7:296. Prior
materialization refusal frame absent/writes+1004; these do not certify complete
JDK/namespace or full producer. Two test-only compile errors corrected with first
failed evidence preserved. No source or resource cap expansion selected here.
