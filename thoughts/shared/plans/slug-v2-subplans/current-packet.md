# Current Slug V2 Work Packet

Packet: WP-5-7A-nodep-fixed-point-pruning-implementation-r1

Status: source audit and implementation contract independently accepted; ready to implement.
No Rust changed or runtime/CLI acceptance claimed by the audit milestone.

## Observable result and source basis

The existing selected-module-graph producer must ignore still-unfulfilled
repo_name=None (nodep) edges after discovery convergence, without hiding an
eligible fetch error, losing later-round fulfillment or changing ordinary edges.
This is the concrete Stage 5 prerequisite of the saved Stage 6 conflict CLI proof.
Read Stage 5's "Frozen nodep fixed-point pruning correction (2026-09-10)" for the
complete semantic, ownership, equality, fixture and lifetime contract.

Authority: /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a, not checkout HEAD.
Discovery.java:62-78,146-181,205-217; DiscoveryTest.testNodep_unfulfilled,
testNodep_fulfilled, testNodep_fulfilled_manyRounds, testNodep_fulfilled_withOverride.
Selection.java:225-242,305-343 and retained Slug nodep/multiple-version controls
preserve fulfilled constraints followed by removal of nodep-only final reachability.
Broader compatibility-level selection is not admitted by this correction.

Exact named behavior is final transformed-key pruning. Structural graph identity,
order and Host observation stay Slug-native. Other selection/source/runtime limits
remain unchanged. No source/registry acquisition, invented bazel_tools, platform
flag, executor, command bypass, new DICE owner, donor import or optimization claim.

## Frozen implementation owner

At discover_fixed_point's existing successful previous_keys == keys boundary,
run one synchronous private helper borrowing its existing key set. Stable-retain
only RawModule.nodep_dependencies whose transformed exact key exists. Preserve
Root, module/source Arcs, ordinary/original dependencies and surviving order.
Do not prune earlier, use only names, create another index/set, alter raw source
facts, or change select_graph/resolve_target/error translation to hide failures.

Both HostSelectedModuleGraphKey and its observation wrapper already use this
producer and tracked root files, command policy, effective overrides and discovered
module children. No key/value layout, equality, validity, observation, async join,
cache or retention model changes. Source facts remain equality-participating;
observed equality retains the epoch even when the semantic graph compares equal.
Existing complete error/Need/frontier precedence and cancellation remain intact.
Scratch and retained memory/lifecycle bounds are frozen in Stage 5.

## Exact allowlist and growth caps

Production:
- app/slug_bzlmod_v2/src/selected_graph.rs: private helper plus convergence call only.

Proof:
- app/slug_bzlmod_v2/src/selected_graph.rs: include hook, small existing test-helper
  instrumentation only; no production API/export for tests.
- app/slug_bzlmod_v2/src/selected_graph_nodep_tests.rs: new included pure/DICE tests.
- app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs: only extend
  observed_selected_graph_diamond_cycle_nodep_rounds_are_exact with an absent nodep
  and nonactivation/pruned-edge assertions; preserve its existing controls.

Caps: <=40 gross production / <=650 gross proof / <=690 aggregate Rust additions.
New test file <=600 physical lines; observation-file additions <=35; new helpers
<=80 lines. Existing 2,481-line producer remains cohesive; its 1,324-line production
body gets one bounded helper/call, not another subsystem. Avoid copied lifecycle
scaffolds in either large file. No dependency/harness/oracle-tree/vendored changes.

Docs: this manifest, canonical Live Status, relevant Stage 5/6 owner status;
maintain /home/wgray/PROGRESS.md separately <=500 lines.

## Required proof and validation

First reproduce a producer-level absent-nodep red regression. Use existing inline
in-memory MODULE/registry fixtures and their provenance comments, not a network
registry or fake imported builtin. A test root named bazel_tools remains only the
existing isolated unit scaffold, never proof of the real built-in CLI closure.

- Absent optional name succeeds and is removed; registry spy proves no optional
  metadata lookup even if that metadata is available. Ordinary missing deps fail.
- Later-name two/three-round discovery fetches/retains requested versions. Once
  eligible, missing/failing requested module remains an error rather than pruning.
- Pure exact-key/requested-versus-transformed/Root/empty-version/order/idempotence
  controls; same-name wrong-version is a helper discriminator, not a successful
  converged producer case. Preserve ordinary/source rows.
- Single-version override applies before membership; nonregistry empty-key and
  root/self controls retain existing transformation semantics. Multiple-version
  validation and nodep-only final reachability remain covered by existing tests.
- Same-DICE absent/present/absent with old results held; warm/comment-only semantic
  equality and exact observed-epoch inequality. Preserve the observed graph's
  legacy parity, event order, source prefix, Need/error and poll/drop recovery.
- No parent success from incomplete discovery, no synthetic historical snapshots.

One Cargo command at a time, initially timeout60:
1. cargo test -q -p slug_bzlmod_v2 --lib nodep
2. cargo test -q -p slug_bzlmod_v2 --lib selected_graph
3. At integration, cargo test -q -p slug_bzlmod_v2 --lib once; includes existing
   selected_repo_spec mapping/route lifecycle and graph-only mapping controls.
4. cargo check -q -p slug_loading_v2 -p slug_core_v2
5. cargo build -q -p slug_cli_v2 before the diagnostic below.
6. cargo fmt --all; scripts/v2_archive_status.sh (only three established
   thoughts-path failures); git diff --check; independent actual-diff review.

After the correction passes, one timeout15s network-disabled diagnostic may reuse
/tmp/slug-cli-configured-action-conflicts-1789068063219567278 with the existing
tests/v2_oracle/fixtures/registry-module-discovery-recovery/workspace/registry/first
local registry: aquery 'deps(//:root)' --registry=file://<absolute-existing-registry>.
Read/verify the fixture first; if absent, report and defer this diagnostic rather
than reconstruct it or acquire sources. Rebuild the binary first. No fixture edit,
daemon, external download or archive acquisition is authorized for this diagnostic.
Record the actual next terminal; a later source boundary is not nodep acceptance or
permission to weaken the combined CLI gate. Do not keep retrying it.

Any command exceeding one minute needs investigation; distinguish compilation
from tests and never automatically extend a test timeout. Fifteen minutes is the
absolute maximum. No checkout-wide/authenticated replay or broad query.

## Resume and stops

Preserved combined selected-request/output-conflict candidate:
base9cc3c4a73, /tmp/slug-conflict-candidate.FIkPZj/candidate.patch,
SHA-25672fabf28cf02355cadc7f3a243f81aadd4824f20cc57275f2ab4e2b4f01aba73;
adjacent validation.txt owns exact passed/failed/unrun gates. It is not restored
or accepted here. 718 production/1772 proof/2490 aggregate additions remain within
its reviewed envelope. Do not repeat its general execution-group/source audit.
After accepted nodep correction, schedule the combined implementation successor,
recheck patch applicability/hash, and complete all remaining original gates.

New semantic state, changed selection/compatibility policy, swallowed fetch errors,
early/name-only pruning, output/runtime admission, cap overflow or a second material
correction is REPLAN. No fallback is introduced. Commit/push each accepted milestone;
never partially ship the preserved combined candidate.
