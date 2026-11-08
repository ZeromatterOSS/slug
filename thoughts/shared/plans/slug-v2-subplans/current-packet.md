# General sequential repository workspace and process APIs

Packet: WP-7-425-m7a-general-repository-process

Status: INACTIVE_IMPLEMENTATION_IN_PROGRESS; production execution remains gated.
Baseline: d202a77e29b4fda077e69bc17ea4874c4635e22a.
The [selected implementation design](wp1225-general-repository-workspace-design.md)
owns the concrete protocol, rooted IO, quotas and file split.
[Fixed backing evidence](wp1225-repository-root-backing-2026-11-07.json) proves
three-launch rolling attachment and scaled same-backing quota shrink; it closes
no production, confinement or public-command gate.
Accepted inactive components cover anchored resolution, rooted IO/archive/delete/
manifest operations, immutable workspace replay/epochs, and rooted source
observations. The [physical-root ownership receipt](wp1225-physical-root-acceptance-2026-11-07.json)
adds actual quota-backed root attestation, consuming complete-tree sealing and
read-only source leases with descriptor/retained/backing credits. Its fixed Rust
native fixtures prove namespace-exit lifetime and rejection boundaries, not
untrusted execution, full archive exhaustion or whole-command memory fit.

Inactive native progress now covers actual ordered File/Template/Read/Exists/
Delete mutations and complete credited tree snapshots; see the
[mutable-progress receipt](wp1225-mutable-progress-acceptance-2026-11-07.json).
The [rolling-lease receipt](wp1225-rolling-lease-acceptance-2026-11-07.json)
adds an inactive linear token primitive and fixed Rust three-launch handoff proof.
The [archive-progress receipt](wp1225-archive-progress-acceptance-2026-11-07.json)
adds verified same-FD archive handoff, pinned source-scope/cumulative decode
accounting, and shared backing-credit lifetime through root and token owners.
The [command/quota receipt](wp1225-quota-command-acceptance-2026-11-07.json)
adds a consuming whole-command owner, strict epoch publication and first-Execute
history/tree admission, actual same-backing Process quota shrink and atomic shared
credit reduction. The [owned-runner receipt](wp1225-owned-runner-acceptance-2026-11-07.json)
adds actual early Rust CLI PID1 setup, installed payload/supervisor policies,
bounded capture, namespace-wide reap and closed completion back into the command.
Native gates cover sequential arbitrary argv, env/cwd/nonzero effects, exact
replay, Process-token mutation, confined Host/proc/device/socket/FD access,
resources, detached closed-stdio writers, drop/deadline/overflow and exec failure.
These remain inactive. The [native source registry receipt](wp1225-native-source-registry-acceptance-2026-11-08.json)
adds closed full-history/spec adoption, globally fresh identities/empty epochs,
exact selected leases, physical anchored observations/digests, descriptor-backed
InputImage and owned source readers. Native source/chain transports now carry
selected leases through CAS and publication; protected Host/legacy behavior,
transitive legacy symlink data lifetime and real NativeLink corruption/cache/
runfiles checks pass. Physical probes prove the actual root/anchor/digest/reader/
image seams; their SourceArtifact metadata helper does not activate original
Loading. Non-Linux native source reading/upload is explicitly unsupported;
pure metadata remains portable. Cross-repository manifests and full native
selected-view admission now join ordinary semantic Execute in the inactive owner.
The [original Loading join receipt](wp1225-original-loading-acceptance-2026-11-08.json)
adds the inactive policy/Need/replay route, actual original cquery with two fresh
commands and normal process records, same-root adoption, anchored generic source
producers, binary output delivery and command retirement through queued results
and bootstrap/payload cancellation. The [command scope receipt](wp1225-command-source-scope-acceptance-2026-11-08.json)
adds trusted crate-internal configuration on ordinary Native services: fresh
command scopes on one persistent bank, original data deadlines, foreign-scope/
bank refusal, Args/RPC stop signals and bounded retirement. Default CLI activation
remains Disabled. The [selected-source bridge receipt](wp1225-selected-source-bridge-acceptance-2026-11-08.json)
adds inactive Workspace, Local, selected BCR tar.gz and pinned Builtin snapshots
through original repository Starlark. Digest-only Source data keeps its original
scope; fresh commands recapture through the existing producers. Original-route
physical CAS/runfiles proof and profile removal are the next frontier, alongside
the complete activation gates below. Existing production process routes
remain replacement-required. The selected design links owner
contracts and evidence; prior component chronology stays in receipts and Git.
Predecessor [BuildInfo acceptance](wp1224-general-build-info-transform-acceptance-2026-10-18.json)
is committed and post-verified. Inherited dirty work remains protected.

## Outcome and compatibility

Advance authentic configured Rust cquery by replacing both GCC ProbeForm/argv/
expected-effect and Homebrew caller profiles with actual repository Starlark
through general process/workspace APIs. Source preparation, API details,
corrections and genuine integration proof belong to this capability.

Exact pinned Bazel 9.2 slices: original operation order, str/Path/Label arguments,
environment removal/override, cwd/quiet, exit/stdout/stderr Latin1, and sequential
file/read/template/execute effects including nonzero-exit writes. Slug-native:
physical path/structural identity, finite quotas, confined Linux Host read view
and conservative command-fresh nonhermetic process observations. Networking,
async execution, unconstrained Host mutation, cross-command process-result reuse
and unadmitted node/link forms remain unsupported. No tool/caller/source profile
may choose replacement semantics or platform permissions.

## Owners and implementation boundary

Bzlmod owns immutable reservation, operation, complete state, transcript and
realization equality. Loading evaluates originals and supplies Path text and
Need/replay/options/current print semantics; it removes Homebrew selectors.
Core owns the one actual root, ordered mutation, child quiescence, complete
bounded snapshot, adoption and release; it removes GCC/source/effect guards.
No DICE transaction, Starlark evaluation or mutex spans child execution.

Reserve the eventual absolute root and observation instance before original
evaluation. Every operation uses that root and exact predecessor; adoption
transfers the same root without rebasing embedded strings or copying a predicted
effect. A command-fresh empty process epoch is demanded before any prepared or
materialization reuse. Every new operation index performs an actual process
sample; only its exact completed record may replay within the command. DICE
stores immutable observations, never child/root-FD mutation authority.

Retain 128 executes, 1 MiB per output stream, 30-second operational maximum and
64 MiB physical tree plus cumulative retained snapshot-body discipline. Keep
actual modes, directories, file bodies, deletions and admitted literal links.
Timeout/cancel/overflow/setup failure kills and reaps before root disposal;
normal nonzero exits retain their actual result and next tree.

Root owns LIVE, projections, native/Cargo execution and atomic integration.
Workers use disjoint inactive candidates under the selected design.
All coupled semantic activation remains atomic after its complete gates.
Relevant owners: Bzlmod repository_execute/filesystem types; Loading repository
context/path/file-effect leaves; Core repository_process/filesystem_progress/
repository_io/source-generations and small private backing/confinement leaves.
Pinned API semantics: [source contract](wp1225-repository-api-source-contract.md).
Inherited [Stage5](05-bzlmod-and-repository-graph.md),
[Stage9](09-v1-extraction-ledger.md) and [DICE](../../../../docs/developers/dice.md)
contracts apply.

## Decision and evidence gates

Use the proven rolling detached lease: attach incoming lease in its actual
owning user namespace, mint the next clone only from the attached root during
trusted setup, and retain it in exclusive native progress. Next launch waits
for current quiescence, complete snapshot and record acceptance. No keeper,
Host alias, proc reopening, copying or privileged fallback. Sources retain
sealed backing without mount operations. The diagnostic's successful scaled
byte/inode limits do not prove all kernel metadata RAM or the full archive
workload envelope.

[Native process-view proof](wp1225-native-process-views-acceptance-2026-11-08.json)
and [selected admission proof](wp1225-selected-view-admission-acceptance-2026-11-08.json)
cover shared owning namespaces, RO finalization/renewal, full child-visible
manifests and complete native selected-set admission through ordinary Execute.
Data-only history guards preserve source allocations without setup authority;
source bodies never become mutable Process history. The selected-source bridge
admits direct Label/Path text, read/template and Execute; derived Source Path
operations/listings, reserved string-read roundtrips, Legacy LocalTar and
cross-source links remain unsupported. Full Archive/public Rust, original-route
physical CAS/runfiles and equivalent-memory gates remain. No production profile
replacement is activated by this evidence.

Inactive protocol, anchored resolver, rooted observation/mutation and native
reader candidates may proceed. Before untrusted activation, prove actual
pre-payload lease transfer, full archive/process quota transition and retention
admission, uniform syscall/mount/socket/device/FD confinement and lifecycle.
Every trusted file/archive/delete/manifest operation must use the same rooted
FD route, including when the display path is absent or names a foreign Host tree.

Independent final review of the implementation and complete production proof
precede activation. Reuse existing launcher FD/timeout/cancel/descendant tests and pinned
Bazel source. Discriminate unrelated sequential originals, absolute path bytes
after adoption, nonzero writes/deletes, actual env/cwd/quiet, Host helper mutation
before warm reuse, exact/+1 quotas, foreign predecessor/lease, socket/device/FD
escape and last-owner release. Then normal/owned owner/direct-dependent checks,
broader integration once, fresh default CLI and justified public Rust cquery at
a fresh unused destination under existing lifecycle/equivalent-memory gates.
Internal or incomplete probes do not close public/bootstrap/whole-fit gates.
