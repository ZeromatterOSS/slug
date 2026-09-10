# Current Slug V2 Work Packet

Packet: WP-5-7A-nodep-fixed-point-pruning-audit-r1

Status: selected after independently confirmed combined-implementation REPLAN.
Docs/source audit only; no Rust implementation or CLI proof acceptance.
Independent terminal review accepts this stop and bounded audit successor.

## Observable result and priority

Freeze the smallest producer-owned correction for still-unfulfilled Bzlmod
`repo_name=None` (nodep) edges at discovery fixed point, then return to the
combined selected-toolchain-request/output-conflict implementation. This is a
concrete prerequisite of its mandatory local CLI gate, not general Bzlmod breadth.
Named/automatic group activation remains behind the combined implementation.

Predecessor WP-6-7A-selected-request-and-output-conflict-implementation-r1
is preserved against 9cc3c4a73 at:
/tmp/slug-conflict-candidate.FIkPZj/candidate.patch
SHA-256: 72fabf28cf02355cadc7f3a243f81aadd4824f20cc57275f2ab4e2b4f01aba73
Adjacent validation.txt records 718 production/1772 proof/2490 aggregate gross
Rust additions, focused passes, failed setup and remaining obligations. Reverse
check passed; only owned Rust changes were restored via apply_patch, and forward
git apply --check passes. Do not partially ship or reconstruct this candidate.
The original smaller patch remains preserved at its prior recorded path.

Final focused core conflict gate passed 10 tests in 0.03s; selected-request core
3, selected-toolchain analysis 11 and REAPI 18 passed. CLI setup instead hit
implicit platforms archive admission, then timeout60 with the existing local
platform scaffold. Local-only registry diagnostics promptly exposed missing
rules_license and then MissingSelectedModule(bazel_features1.42.1). No CLI child,
daemon, execution RPC or action-output mutation was reached in the failed setup.
Full integration, loaded raw-fact/message cutoff, conflict-specific lifecycle,
positive REAPI sharing and final combined implementation review remain unaccepted.
Rebuild the CLI before later binary proofs; cached binary still contains candidate.

## Learned facts and authority

Read Stage 5's nodep fixed-point prerequisite and Stage 6's combined-implementation
stop. Preserve the accepted Stage 6 selected-request and closure-integrity contracts.

Pinned Bazel source authority is local /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a, not checkout HEAD.
Discovery.java:62-78 repeats rounds until nodep name fulfillment reaches fixed
point; :205-217 gates following nodep edges on prior-round module names; :146-165
removes still-unfulfilled transformed exact module-key edges before selection.
Source tests: DiscoveryTest.testNodep_unfulfilled, testNodep_fulfilled,
testNodep_fulfilled_manyRounds, testNodep_fulfilled_withOverride.

Live app/slug_bzlmod_v2/src/selected_graph.rs name-gates discovery at :640-650,
but validate_and_reachable(include_nodep=true) resolves retained nodep rows
unconditionally at :990-992. An absent optional module incorrectly becomes
MissingSelectedModule. Existing ordinary-dependency registry fixtures expose
this defect; acquiring unrelated modules would mask it, not prove correctness.
The first review's complete-authentic-closure acquisition proposal is superseded
by this pinned-source diagnosis. No new registry/archive acquisition is authorized.

## Decisions to freeze, not implement

- Exact named behavior: prune only unfulfilled transformed nodep edges after
  discovery convergence; retain fulfilled version constraints and ordinary errors.
- Verify exact-key presence, root/self and nonregistry/single/multiple-version
  override interaction, multi-round/later-name fulfillment and deterministic order.
  Do not prune early based only on names or ignore a fetched module's failure.
- Name the existing HostSelectedModuleGraphKey producer, tracked discovery inputs,
  raw/final graph values and natural cutoff boundary. No new DICE key, graph,
  command bypass, fake bazel_tools, explicit platform-flag admission or side store.
- Freeze same-DICE absent/present/absent and unchanged-semantic cutoff proofs,
  observed Need/error/cancellation precedence, and downstream mapping consumption.
  Reuse existing selected_graph unit/DICE scaffolds and Buck2 worker
  when_equal_return_same_instance, test_detecting_changed_dependencies and
  mismatch_epoch_results_in_cancelled_result as concept/test guidance only.
- Preserve immutable compact graph ownership/Allocative. Pruning is phase scratch
  at its natural producer; no retained duplicate graph, global registry or lock
  across awaits. Existing reachable versions/command tokens own retention/release.
- Inspect physical size/cohesion before choosing exact implementation files/caps.
  Prefer a bounded helper in the existing selected-graph producer if cohesive.
- Specify how to reassess the existing local ordinary-module registry scaffold
  after correction, without claiming that the remaining host/platform/toolchain
  source gate will necessarily pass. Do not invent module bodies or broaden
  materialization to force the conflict CLI tests through.

Structural graph identity, order and Host observation remain Slug-native;
exact checksum/path/ActionKey bytes, broader execution and unsupported platform
flags remain deferred. No donor code or new optimization/fallback is selected.

## Scope, validation and stops

Writable repo files: this manifest; canonical Live Status; relevant Stage 5 and
Stage 6 owner status; orchestration routing log and its existing August archive
only for bounded rollover. Maintain /home/wgray/PROGRESS.md separately <=500 lines.
Cap: <=250 added documentation lines outside this manifest. Read only the named
source/tests and existing fixture precedents needed to resolve the frozen questions.
No Rust, dependency, fixture/harness, vendored, source override, archive or registry
input edits; no Cargo, Bazel, network replay, checkout-wide query or oracle run.

Produce a reviewed implementation successor with exact allowlist, gross caps,
source-derived red/green tests, named dependents and stops. Validate source/structure,
scripts/v2_archive_status.sh (three established thoughts-path failures only), and
git diff --check. Require independent reserved-boundary review before activation.
Commit/push the accepted docs milestone, then implement the correction before
restoring the combined candidate. Do not repeat the general execution-group audit.

If fixed-point pruning needs new semantic ownership, broad graph-policy changes,
unmodeled source facts or fixture acquisition, return REPLAN with the concrete
decision. Do not substitute fixture downloads for missing nodep semantics.
Tests in the later implementation start with timeout60; over one minute requires
investigation and fifteen minutes is absolute maximum. No longer automatic retry.
