# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-frontier-certificate-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design the smallest app-internal multi-observation loading-frontier
certificate and its one-way loading/Bzlmod-to-core ownership boundary. This
packet is documentation-only and must select one future bounded consumer or
record a prerequisite `REPLAN`.

## Fixed predecessors and audit result

Commit `f0849151` accepts the sole-root exported-source bridge over the
private request-revision family in `207fe438`. Commit `ea36fdcc` activates
the independent next-consumer audit. That audit finds no remaining bounded
one-observation consumer.

The least-broad apparent candidate, selected root BUILD loading, is already a
source-selection frontier. `BuildCommandRootKey` reaches
`RootPackageLoadKey`, which first anchors the root module and computes
`RootPackageSourceKey::for_build`. `HostRootPackageLookupKey` probes
`BUILD.bazel` then `BUILD` across configured package roots through
`ResolvedPathKey`; only afterward does `HostFileBytesKey` read the selected
file through `PathObservationKey`. The parsed BUILD may recursively compute
`HostBzlModuleEvalKey` children. A selected-file certificate therefore cannot
reject a concurrent precedence, package-root, boundary, or loaded-`.bzl`
change.

Root `MODULE.bazel` dynamically expands an `include()` horizon and retains
evaluation effects. One `.bzl` recursively expands its load closure and cycle
state. Direct-local external exported source adds route, repository result,
materialization, package discovery, and source observations. The existing
core-private one-file type cannot be produced across a crate boundary; moving
it downward without a design would either expose a generic public framework or
create the reverse dependency that the audit forbids.

Existing root MODULE, BUILD, `.bzl`, and direct-local behavior remains
unchanged. The audit records `REPLAN`: representation, visibility, complete
frontier aggregation, and batch final validation must be designed before a
second consumer.

## Design objective

Freeze exactly one non-user-facing `LoadingFrontierCertificate` concept that
can be produced in the existing loading/Bzlmod-to-core dependency direction
and associated with one complete terminal. The design must choose its natural
owner and visibility:

- prefer the lowest existing crate that can represent every required Host
  observation without depending on core;
- decide whether an internally public but sealed app type is acceptable;
- otherwise select an existing lower shared owner without turning it into a
  generic user API; and
- `REPLAN` rather than introduce a loading/Bzlmod dependency on core or a
  second graph/store.

Name the exact future type/module owners, dependency edges, construction sites,
terminal carriers, and core consumption sites. Do not implement them here.

## Complete frontier algebra

The certificate must be a deterministic compact collection of exact Host
observation demand/result pairs with explicit provenance. It includes both
chosen source bytes and every negative or positive probe that controls source
selection: package roots, BUILD precedence, package boundaries, include/load
selection, and dynamically discovered children admitted by the chosen
consumer.

The design must specify:

- complete-only construction for both success and completed errors;
- Need and cancellation paths that publish no partial certificate;
- deterministic ordering, structural equality/hash, duplicate coalescing, and
  conflicting-result failure;
- exact demand containment and workspace/root identity;
- how recursive MODULE includes or `.bzl` loads union child frontiers without
  retaining evaluators, transactions, repository results, or snapshots;
- how negative selection probes remain reobservable rather than being reduced
  to the chosen file;
- Arc/clone boundaries, maximum retained lifetime, memory accounting, and
  release on retry/error/cancellation; and
- whether dynamic cardinality needs a per-command cap and its typed overflow.

A certificate is evidence for one terminal, not accepted semantic state.
`AcceptedNativeDemandSnapshot` remains only native
inputs/repository/path selection and never becomes certificate authority.

## Carrier and ordering design

For the first future consumer, map success, every completed source/evaluation
error, Need, event, and cancellation ordering. Freeze the exact carrier path
from the lower owner through loading to the core terminal without changing
public results, error text, event batches, or output bytes.

Certificate construction must finish before terminal sealing. Selection,
activation closure, event/demand capture, full updater preparation, and
repository validation remain outside the request-revision owner. The selected
terminal token remains the sole provisional-effect authority.

Choose exactly one first consumer. Root MODULE, selected BUILD, one `.bzl`,
and direct-local external may be compared, but a future implementation packet
may not combine them.

## Batch final-validation design

Extend the existing owner protocol conceptually, without code, for a complete
frontier:

1. prepare and select the complete terminal outside the owner;
2. acquire the existing async, nonreentrant publication owner;
3. compare the prepared updater's existing state with the terminal transaction;
4. exact Host-reobserve every certificate demand as a deterministic batch,
   with no DICE compute, Starlark, repository/materializer work, event callback,
   or lock reentry;
5. when unchanged, add the checked successor revision to the prepared full
   updater and commit once;
6. when changed, replace every changed certificate entry inside the command's
   full epoch, publish that epoch plus successor revision atomically from a
   fresh updater, suppress/reset, and retry; and
7. on version advance, observation error, overflow, nonprogress, cancellation,
   or failure, expose no stale terminal or provisional event and restore or
   fail closed.

Specify changed-many ordering, observation-error algebra, exact counters,
initial-revision interaction, retry cap, reset ownership, and proof that no
compute or callback can reenter the owner. A partial revalidation or a
one-entry overwrite of a full epoch is forbidden.

## Compatibility and evidence

Preserve accepted serial MODULE, BUILD, `.bzl`, external, Need/error/event,
output, and recovery behavior as regression/non-widening invariants. Reuse only
separately accepted exact source-observation evidence; do not infer new parity
from `f0849151`.

Exact Host observation values remain exact only where already admitted.
Frontier aggregation/identity, batch final validation, request revision,
retry/reset, stale-effect suppression, and future overlap are Slug-native.
Directory/glob unions beyond the selected frontier, repository/materialized
certificates, public overlap, historical Host reads, and exact Bazel identity
bytes remain unsupported/deferred.

## Allowlist, caps, and proof contract

Edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 300 current-packet, 260 Stage 2, and 600 total net
ledger lines. Read only `docs/developers/dice.md`; the three relevant Cargo
manifests; core `runtime/{dice,request_revision,events}.rs`; loading
`src/{lib,keys,file_discovery,package,bzl_module,load_label,cycle_detector}.rs`;
Bzlmod
`src/{lib,dice,source_preparation,root_bootstrap,host_module,host_include,host_package,host_file}.rs`;
`host_package_boundary/mod.rs`; and directly referenced focused tests. Missing
named files may be mapped to their live equivalent without widening the owner
families.

For the retained compact representation only, read the matching
Stages-3/6 utility row in
`slug-v2-subplans/09-v1-extraction-ledger.md`, the repo
`slug-buck2-utility-reuse` skill, and retained
`starlark_map/src/{small_map,small_set,sorted_map}.rs`,
`gazebo/dupe/src/lib.rs`, and `allocative/allocative/src/lib.rs`.
This authorizes bounded comparison, not a Stage 9 write or new container.

No Rust, Cargo/BUILD, oracle fixture, generated evidence, or other ledger write
is authorized. Use source inspection and existing accepted evidence only.
Independent review must confirm dependency direction, complete selection
frontier, error/effect ordering, owner nonreentry, memory/lifetime, bounded
first consumer, compatibility, caps, and exact successor allowlist/proof.

STOP on any code or oracle write, public user API/wire/output, reverse core
dependency, generic public certificate framework, new graph/key/store,
snapshot replacement, evaluation/compute/callback under the owner,
repository/materializer activation, partial frontier, historical Host reads,
watcher, JVM work, combining consumers, or cap excess.

`REPLAN` if no one-way app-internal visibility boundary is possible, negative
selection probes cannot be represented exactly, a complete frontier is
unavailable before terminal sealing, batch final validation requires DICE,
Starlark, or repository work under the owner, atomic multi-entry epoch plus
revision cannot reuse the existing owner, or the first implementation consumer
cannot be bounded independently.

## Acceptance and immediate successor

Accept only a design that names one coherent representation/owner, complete
frontier and carrier algebra, compute-free batch finalization, one bounded first
consumer, exact Rust/test/ledger allowlists and caps, focused proof, and all
STOP/`REPLAN` boundaries. Then activate that implementation only after
independent ownership and cleanup review; otherwise activate the smallest
remaining design prerequisite.
