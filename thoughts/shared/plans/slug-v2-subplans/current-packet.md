# Current Slug V2 Work Packet

Packet: WP-5-7A-authentic-sentinel-library-probe-r1

Status: independent review ACCEPTS the read-only audit and revised target contract.
Implement only this library-target packet; no original integration-target retry.
User-installed strace6.8 passes harmless isolated preflight outside tool sandbox.
The integration-target compile stopped at60seconds with no executable. Installed
Cargo docs require binary builds for integration targets; core/CLI fingerprints
show two profiles consistent with abort/unwind variants, not timed attribution.
Use the same native test as a cfg(test) library module to avoid selecting a CLI
binary build. No profile/dependency/production semantic change or longer deadline.
This is a different selected target, not a warmed retry of the timed-out command.
No production semantics, complete/partial R2 restoration, acquisition, Bazel,
CLI conflict-suite rerun or checkout-wide/bounded replay.

## Observable result and source basis

Run exactly one isolated native sentinel evaluation with authentic registry
metadata and the two already-evidenced prerequisite archives. Determine whether
it opens the absent abseil mirror path, completes analysis without that demand,
or stops earlier. Earlier errors, missing inputs and resource limits are
inconclusive about abseil; preserve their exact first boundary instead of retrying.

Predecessor WP-5-7A-configured-cli-payload-demand-review-r1 proves metadata/spec
construction is separate from capture: selected_repo_spec.rs:1241-1370,1495-1525
reads/project specs; core runtime/dice.rs:7161-7213 materializes only emitted Needs.
source_preparation.rs:5226-5252 emits a Need for an absent/mismatched result;
repository_io.rs:1489-1509,1664-1680 dispatches http_archive capture. Actual
configured traversal remains conditional, not proved required/nonrequired.
Stage5 records exact implicit-registration/launcher/source anchors. No source
change or runtime was performed in that review.

## One diagnostic and fixture identity

Implement the same opt-in ignored native test in
app/slug_cli_v2/src/payload_demand_probe.rs, included only via a <=3-line
#[cfg(test)] module declaration in app/slug_cli_v2/src/lib.rs, plus one driver
tools/v2_oracle/run_payload_demand_probe.sh. <=600 total added implementation/
proof/driver lines,0 production semantics. Inspect lib.rs and its test-module
shape before editing; stop if this needs a different owner/public interface,
a broad harness rewrite, or adding to a file over2000 lines.
No Cargo/dependency/profile/rustflags changes. Use existing dependencies/utilities.
Do not append to oversized cli.rs or restore the old integration-test path.

Reuse the unaccepted draft at /tmp/slug-sentinel-draft.Lln8y0/candidate.patch,
SHA256 8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2,
as a draft only; keep that patch and adjacent validation.txt unchanged. Preserve
its exact API/input/trace behavior, correct the known exception cleanup defect,
and relocate only the ignored test's Cargo target. R2 stays untouched.

The test performs exactly R2 sentinel_outputs' native API call, using
BuildRequest::parse with --registry=file://<scratch>/registry and //:root,
then evaluate_workspace_build_command_with_bzlmod_inputs with the resulting
policy/registry fields and the same default environment/configuration projection
as R2. Project to TerminalOutput and publish through the same accepted-command
boundary; report native errors and published exit status. Do not write sentinels,
execute configured actions, invoke a CLI, or start slugd.
Name the test authentic_sentinel_demand; it is ignored in ordinary test runs.
This is a Slug-native demand diagnostic, not Bazel parity or actual CLI acceptance.

Use a new mktemp directory /tmp/slug-sentinel-demand.XXXXXX owned by the driver:
workspace/{MODULE.bazel,BUILD.bazel,defs.bzl},registry/,mirror/,logs/.
Copy only the preserved R2 root BUILD/defs and MODULE root name, root-local
platform/toolchain registrations and platforms1.0.0 dependency. Remove only
fake-platform local_path_override and generated fake-platform bodies. No semantic
root simplification, source/version override, stub, explicit platform flag,
command root change or conflict mutation. Candidate patch stays untouched.

Fixture metadata is a bounded temporary input superset, not a selected graph:
from pinned Bazel8220c6198837d5c13d53fea211cf3282aa12408a default lock
src/test/tools/bzlmod/MODULE.bazel.lock, SHA-256
d7cbba1d746f5522d7dde4a2f7ea7a24d8f0befdf23d7cb4984689b48781049a,
stage only its183 MODULE/source.json rows from exact verified Bazel CAS paths,
preserving bytes and relative registry paths. Hash-check regular inputs first;
<=1MiB/file and1MiB total fixture input bytes. No lockfile injection or module
evaluation during staging. The184th original registry-config row is NOT copied:
write explicit test-owned bazel_registry.json with only
{"mirrors":["file://<scratch>/mirror"]}. This is transport policy, not fabricated
module/source metadata. Record both this generated config and its purpose.

Stage exactly these3 hash-verified payloads, no other archive or patch:
- platforms1.0.0 archive3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8
  (7879bytes), from the exact Bazel CAS hash/file locator;
- rules_shell0.6.1 archivee6b87c89bd0b27039e3af2c5da01147452f240f75d505f5b6880874f31036307
  (23916bytes), from Stage5's exact Slug downloads filename;
- rules_shell version patch5f0700eaa9a33770aae4ae8b06bec8e433f518eb50711378c8cd3a5d7854ff2d
  (320bytes), from the exact Bazel CAS hash/file locator.
Archive mirror paths are original descriptor URL host+path under mirror/;
patch is registry/modules/rules_shell/0.6.1/patches/module_dot_bazel_version.patch.
Keep source.json/MODULE/SRI/strip/patch order unchanged. Authentic host_platform
generation/builtin content must execute normally if demanded. No pre-extraction.
Leave the exact abseil mirror path absent, as observed at both cache locators.
Never stage abseil or resume the47-payload inventory in this diagnostic.

## Execution, observation and hard limits

Compile only the library test target once: timeout --kill-after=1 60 cargo test
-q -p slug_cli_v2 --lib --no-run --message-format=json, with pinned
nightly-2025-09-14 toolchain, unchanged profile/rustflags and serialized Cargo.
Capture the complete bounded compiler log plus elapsed time; parse both Cargo
compiler-message errors and final artifact/build-finished, not just stderr.
Select exactly the compiler-reported slug_cli_v2 library-test executable with
profile.test=true and target.kind containing lib, never a guessed cached binary.
If Cargo reports a selected slug binary artifact, stop and investigate routing.
No guarantee this finishes in60seconds; timeout/error/missing executable is a
terminal compilation stop, no retry/profile override or cap increase.
Do not compile again in the driver after a successful build: one driver owner
compiles once and uses its own returned executable. No full suite or CLI rebuild;
no CLI executable is invoked.

Before evaluating, require available unshare, prlimit and strace plus a working
unprivileged user/network namespace. Preflight with a harmless child only.
Failure is an isolation/tooling stop, not permission to drop the boundary.
Run the test executable --ignored --exact
payload_demand_probe::authentic_sentinel_demand --nocapture
inside that network namespace under prlimit AS=2GiB/CPU=15seconds/FSIZE=16MiB and
a15second wall-clock process-group deadline; KILL/reap the entire scoped group
on deadline and clean any remaining group members on every exit. No daemon.
Correct the draft's exception path: a single finally-style bounded kill/reap/close
phase must own normal, timeout, overflow, signal and supervisor-error exits. Keep
subreaper ownership through all descendant waits; report cleanup failure as
inconclusive, never success. No unbounded wait and no dropping children on error.
Before compilation/staging, exercise this same supervisor with harmless children
only: normal exit, wall-deadline termination, and injected supervisor exception
after setsid with a live descendant. Each self-check <=5seconds, group-scoped
cleanup and authoritative no-survivor/reap evidence; separate self-check logs
never count as native demand evidence. No Slug/fixture/network in self-checks.
These lifecycle checks reuse the driver owner, not a second test framework.
These AS limits are per process, not an aggregate-RSS performance claim.
Use a minimal environment; no credential forwarding, RC reads or network access.

Use strace -f with only openat/openat2 and -P selecting the exact absent
mirror/github.com/abseil/abseil-cpp/releases/download/20250814.1/abseil-cpp-20250814.1.tar.gz.
Keep trace <=64KiB; concurrently drain stdout/stderr retaining <=8192bytes each.
Overflow is a stop, not silently truncated passing evidence. Entire invocation/
supervision <=60seconds. Never retry after timeout or enlarge a resource cap.
Record return status, elapsed time, peak RSS where available, trace and complete
bounded diagnostics. Positive demand requires an actual traced child openat/
openat2 syscall targeting the exact abseil path, never strace startup/path-warning
text. Keep harmless preflight traces separate; they never count as probe evidence.
Non-demand requires native evaluation success AND published exit status0 with
no such syscall, and applies only to this exact sentinel/input revision, not
other CLI commands. A nonzero publication, earlier error, timeout, overflow or
resource/isolation failure is inconclusive and leaves demand unresolved.

## Evidence hygiene, ownership and close

The183 small metadata files are temporary discovery inputs, not checked-in
fixture breadth; every file is hash/provenance bounded by the pinned catalog.
Keep only a compact generated inventory/digest and diagnostic logs for handoff;
remove staged trees after recording the result, without deleting any source cache.
The diagnostic must not become a production cache locator or mandatory test in
ordinary CI. Reuse/copy bounded pipe themes only, no whole existing test module.
No new exact-parity fixture; native diagnostic provenance belongs in Stage5.
No DICE/key/lock/retained-state/overlap/fallback changes; buffers are phase scratch,
child lifetime belongs to the driver and source publication stays native-owned.

Writable: the2 named new files plus the cfg(test)-only lib.rs declaration,
this manifest, canonical Live Status and relevant
Stage5/6 status; routing only for REPLAN; <=120 added doc lines outside manifest,
PROGRESS.md<=500. Source caches/preserved candidates read-only; all edits apply_patch.
Run formatting/shell+Perl syntax, driver lifecycle self-checks, diff, preserved
R2 hash/forward-apply and draft hash/relocated-body comparison, plus archive checker
(known3 only). The old draft itself need not forward-apply over its revised driver;
its clean-baseline applicability is already recorded. Preserve R2 applicability.
One diagnostic attempt; independent terminal review; commit/push accepted evidence
or concrete stop and select its first actual prerequisite. No speculative fixes
or input acquisition. Second material correction/new owner/cap overflow is REPLAN.
Never inspect/print/copy ~/.bazelrc or derived secrets.

R2 /tmp/slug-conflict-r2.XZJWwv/candidate.patch remains unaccepted,base97dffd5d4,
SHA-256 90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Its validation.txt owns gates: actual one-shot/stable-daemon conflicts, positive
common execution-view/REAPI sharing and complete relevant gates remain open.
Future complete R2 fixtures must propagate this explicit policy to both sentinel
and every CLI request; no partial restoration/shipping or broad replay.
