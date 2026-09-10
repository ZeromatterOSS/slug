# Current Slug V2 Work Packet

Packet: WP-5-7A-authentic-sentinel-demand-probe-r1

Status: preflight STOP before implementation/staging/compilation or evaluation.
Required strace is absent from PATH and standard executable locations; dpkg-query
reports no installed strace package. Harmless user/network namespace plus prlimit
preflight passes. Request a provided executable or approval to install strace;
do not remove tracing. Resume this same reviewed contract once tooling exists.
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

Implement an opt-in ignored integration test in
app/slug_cli_v2/tests/payload_demand_probe.rs and one bounded driver
tools/v2_oracle/run_payload_demand_probe.sh. <=600 total added implementation/
proof/driver lines,0 production lines. No Cargo/dependency changes; inspect the
CLI Cargo manifest and existing test helpers first, stop if a new dependency or
public interface would be needed. Prefer existing dependencies/shell utilities.
Do not append to oversized cli.rs or add broad scaffolding.

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

Compile only the named ignored integration test: timeout60 cargo test -q
-p slug_cli_v2 --test payload_demand_probe --no-run; serialize Cargo, no full suite.
The driver selects the compiler-reported test executable, never a stale guessed
binary. No slug CLI rebuild is needed because no CLI executable is invoked.

Before evaluating, require available unshare, prlimit and strace plus a working
unprivileged user/network namespace. Preflight with a harmless child only.
Failure is an isolation/tooling stop, not permission to drop the boundary.
Run the test executable --ignored --exact authentic_sentinel_demand --nocapture
inside that network namespace under prlimit AS=2GiB/CPU=15seconds/FSIZE=16MiB and
a15second wall-clock process-group deadline; KILL/reap the entire scoped group
on deadline and clean any remaining group members on every exit. No daemon.
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

Writable: the2 named new files, this manifest, canonical Live Status and relevant
Stage5/6 status; routing only for REPLAN; <=120 added doc lines outside manifest,
PROGRESS.md<=500. Source caches/preserved candidates read-only; all edits apply_patch.
Run formatting/shell syntax, diff/R2 hash/apply and archive checker (known3 only).
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
