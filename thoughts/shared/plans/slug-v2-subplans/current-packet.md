# Current Slug V2 Work Packet

Packet: WP-5-7A-configured-cli-source-and-baseline-audit-r1

Status: selected after independently reviewed combined-implementation R2 REPLAN.
Docs/source audit only; both baseline comparisons are complete. No executable
test/runtime reruns or Rust/source-policy changes are authorized.

## Observable result and preserved work

With the two fast core failures attributed to the accepted nodep baseline, freeze
the smallest authentic configured-CLI source prerequisite.
Resume the complete selected-request/output-conflict contract after that
prerequisite, not a reduced CLI assertion or partial implementation.

R2 candidate preserved against 97dffd5d4:
/tmp/slug-conflict-r2.XZJWwv/candidate.patch
SHA-256: 90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e
Adjacent validation.txt owns exact passed/failed/unrun gates. Gross Rust additions:
757 production/2103 proof/2860 aggregate; validator287 production, dedicated
core proof368 lines. Reverse check passed before restoring owned Rust via
apply_patch; forward check passes. Both removed new files remain recoverable.
Original R1 patch and validation remain unchanged at their previously recorded path.

R2 supplied loaded raw/message A/B/A and context-pointer cutoff through both
constructor paths, concurrent root-set conflict/restoration, and bounded CLI pipe
draining. Its loading-owned target exec_properties wiring was independently
accepted as a scoped correction: one phase-scratch map into existing default
constructors, reject dotted/group-qualified keys, leave group maps empty. No new
retained identity or named-group support. Complete combined acceptance is still open.

Focused conflict12 passed in0.44s before the final dotted guard. Full analysis147
and build-api76 pass after that guard; query/server check passes. Helper-only CLI
drainer1 passes in0.00s. Full core311 timed out at60s with multiple failures and no
final summary; no survivors, no longer retry. Two tight in-memory diagnostics:

- duplicate-target terminal-producer test: event-order mismatch in0.04s;
- resolved_run_view test: missing retained Host action-environment facts in0.06s.

Both reproduce identically on clean97dffd5d4 Rust: exit101,1failed/294filtered,
0.04s and0.06s respectively; whole commands took0.60s and0.66s with cached builds.
The first result was recovered by one bounded rerun after confirming no live prior
process. These two failures predate R2; other full-core failures remain unattributed.
Baseline comparisons are complete: do not repeat them. No full-core or actual CLI
acceptance is claimed.

## Learned facts and bounded questions

The accepted nodep correction at97dffd5d4 eliminated false absent-nodep failure.
The spent network-disabled diagnostic then stopped at rules_shell local_repository
source projection (exit2,0.37s,RSS36716KiB). However its local-registry subtree has
only MODULE.bazel/source.json/BUILD.bazel with exports_files([]), not
shell/sh_binary.bzl. Its fixture.toml explicitly calls builtin declarations/stubs
discovery scaffolding, not source evidence. Do not nominate local_repository
admission alone or grow that stub tree to manufacture CLI success.

The original no-override fixture instead stopped at real platforms archive entry
mode. Root-local platform/toolchain registrations do not establish avoidance of
implicit host-platform source discovery. Audit authentic existing inputs first;
neither a new archive-mode policy nor a complete closure is already accepted.

Read Stage6's selected-request and configured-action-closure contracts and R2 stop.
Pinned Bazel authority: /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a, not checkout HEAD. Use only relevant
repository/archive source and tests to establish entry-mode and source provenance;
retain admitted Rust Host observations and structural identity as Slug-native.
Exact permission/source claims require a named pinned-source regression or oracle.
No new exact checksum/path/ActionKey or configured runtime surface is admitted.

Audit:

1. COMPLETE: on clean97dffd5d4 Rust, compared the two named in-memory tests below.
   Both terminal failures match. Do not repeat, waive either failure, rewrite
   expected events or make a semantic fix.
2. Inspect existing authentic source/cache/archive inputs required by the real
   configured fixture. Do not scan credentials, download/acquire inputs, or
   execute archive content. Confirm hashes/provenance, needed entry type/mode,
   source completeness and the current Slug producer before proposing a policy.
3. Identify the existing source-preparation/request-kind and archive-realization
   owners, tracked inputs, observation/certificate and extraction-safety boundary.
   Freeze a minimal successor only when compatibility, source authority, exact
   files/caps and discriminating safety/lifecycle proofs can be named.
4. If no existing hermetic scaffold reaches the real conflict assertion, record
   the concrete missing input/producer decision as REPLAN; do not use fake builtin
   content, a source override, explicit platform flags or broader replay.

No retained-state/layout change is selected; existing DICE/Host publication,
equality, cancellation and source certificates remain mandatory. Buck2 ownership
guidance is concept-only; no utility import, fallback or new cache is authorized.
Any implementation successor must inspect file size/cohesion and pass the plan
authoring checklist and independent reserved-boundary review before activation.

## Exact scope, checks and stops

Writable repo docs: this manifest, canonical Live Status, relevant Stage5/6 owner
status; orchestration routing log only for this REPLAN and bounded rollover.
Keep /home/wgray/PROGRESS.md <=500 lines. Cap <=200 added doc lines outside manifest.
No Rust, dependency, fixture/harness, vendored, registry, archive or source-input edits.
Read only the named test/producer/scaffold evidence and relevant existing cache
inputs needed to settle the concrete questions; never inspect/print/copy ~/.bazelrc.

Completed baseline comparison evidence (serialized, timeout60 each):

- cargo test -q -p slug_core_v2 --lib build_command_root_selects_each_terminal_producer_once_for_duplicate_targets
- cargo test -q -p slug_core_v2 --lib resolved_run_view_reuses_exact_executable_filewrite_relation

No executable tests/runtime commands or reruns remain authorized. Read-only
source inspection and the documentation/preservation checks below remain allowed.

Distinguish compile time from test time. More than one minute requires investigation;
fifteen minutes is absolute maximum, with no automatic longer test retry.
No other Cargo gate, CLI/Bazel/oracle invocation, daemon, authenticated/checkout-wide
replay, repeated spent setup or acquisition. Cached binary is stale relative to
restored sources; it must not be used as baseline evidence.

Validate source/provenance and structure, saved-patch hash/forward applicability,
git diff --check and archive checker (only the three established thoughts paths).
Require independent terminal review. Commit/push accepted audit/REPLAN scheduling
milestones, never a partial combined candidate. New source policy, broad fixture
closure or unmodeled ownership needs an explicitly frozen successor, not an
incidental fix during this audit.
