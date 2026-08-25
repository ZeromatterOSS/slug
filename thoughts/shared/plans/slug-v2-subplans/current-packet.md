# Current Slug V2 Packet

Packet: `WP-4-5-6-generated-repository-file-effect-handoff-application-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md`,
`05-bzlmod-and-repository-graph.md` and
`06-analysis-toolchains-and-actions.md`
Base: accepted producer record `b360be14` and retained Rust candidate

Result: freeze one demand-only selected-effect route/request/application
vertical without adding a DICE key or changing native repository behavior.

## Frozen authority baseline

Future Rust authority is exactly:

| Path | Baseline lines | SHA-256 |
|---|---:|---|
| `app/slug_bzlmod_v2/src/host_module.rs` | 4,825 | `185ec7685abd51851c570762e393df1d59892596854cf6c826603d00a2703c39` |
| `app/slug_bzlmod_v2/src/source_preparation.rs` | 16,811 | `7357686adb4171ebe0a2c7ddadf518f22c67ac36aebcda471c2e393f7dc5e878` |
| `app/slug_bzlmod_v2/src/host_package.rs` | 5,001 | `bca9046c7f0088102c0c2b7e7c1a7607788056d3ef48ffbe739506fab14f0ac0` |
| `app/slug_core_v2/src/runtime/generated_repository_definition.rs` | 4,027 | `368dd75d6f9d8c52f9858b176a09820660ea1920777408bf61641a5067e48359` |
| `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs` | 1,729 | `a1cf060405c4a5d7be26acc4b23dda542c7c0fad20325fd6fa4b7369f8dc1f3a` |
| `app/slug_core_v2/src/runtime/generated_package_route.rs` | 591 | `27e6ee70e2b95c3b1e48bb6fcca8795fd2ba763cb6b0867ffd7fc9ba87f90818` |
| `app/slug_core_v2/src/runtime/repository_io.rs` | 5,523 | `f6a8ecb870b460fd06dcf8edbcd589dd1edd4809b77cdbdb66976a7ecdcaa19e` |

All other Rust, tests and fixtures are retained and non-writable. Independent
review accepts this exact authority, representation and proof contract.

## Structural handoff

Add `RepositoryMaterializationKind::GeneratedFileEffects` containing the exact
`GeneratedRepositoryFileEffectPlan`. Do not add a field to
`RepositoryMaterializationRequest`: all existing constructors remain unchanged,
full request equality already includes `kind`, and canonical request ID remains
the physical namespace. Same-ID/different-plan requests therefore conflict
rather than alias.

Retain the plan directly in `RootRepositorySource::Generated` and require it in
`RootRepositoryRoute::for_generated_repo_spec`. Carry it through one private
materialization-flavor field on `HostRepositorySourceCapability`; do not change
the public `HostRepositorySourceCapabilitySource::{Builtin, RepoSpec}` shape.
Include the plan in manual route and capability hashing. Project a generated
capability directly to the new materialization kind; never pass its custom
repository rule through the local/http/git `request_kind` classifier.

Do not expose loading certificate internals. At core's existing authenticated
demand -> certificate join, retain `demand.owner().clone()` beside certificate
and unique ordinal in `HostGeneratedRepositoryDefinition`. Forward only a
core-private borrowed `{ owner, ordinal }` seed through canonical and root-
apparent definition views. Perform no second canonical-name scan.

## Demand owner and observation order

`GeneratedPackageRouteKey` and its observed sibling remain the sole natural
demand-side owner. Only after mapping and root-apparent definition resolve to
Generated, compute the accepted loading effect key over workspace, retained
owner and ordinal. Merge observations left-first in exact order:

`mapping -> definition/certificate -> selected effect`.

Forward Need carrierlessly. An effect observed outer remains an opaque route
outer; an effect semantic error becomes a typed generated-route terminal that
retains the exact producer error. Loading retains its invocation print batch;
route/source parents publish none. Non-Generated definitions remain fallback-
neutral Missing and never activate the effect key.

Construct the route only from a successful exact plan. Downstream package and
source keys remain unchanged: they project one generated immutable request,
return it through the existing materialization Need, and after success observe
the existing immutable observation instance.

## Atomic generated-plan application

Extend only the existing core repository session/materializer. The native
dispatcher recognizes `GeneratedFileEffects` without reloading a rule. Before
creating or writing a published root:

- revalidate every path as nonempty normalized relative valid Unicode;
- reject duplicates and file/ancestor collisions such as `a` with `a/b`;
- compute a domain-separated, length-framed SHA-256 source association from
  ordered path bytes, content bytes and executable polarity; and
- allocate one fresh private `TempDir`, create parents, create each file once,
  write exact bytes, flush and set POSIX mode `0755` for executable or `0644`
  for non-executable.

Do not retain the framing scratch bytes. Any validation, create, write, flush or
mode failure drops the private root. Only after I/O completes may the existing
post-I/O session-token validation assign an observation instance and add the
root to provisional session state. Existing selection acceptance atomically
retains selected roots and releases every other provisional root; cancellation,
stale token and abort discard the candidate.

Map generated completion to the existing Immutable success and source-
observation path. The private source association is not Bazel ActionKey,
configuration checksum, REAPI digest or route identity. The complete plan stays
structural in route/request DICE equality; the digest only identifies actual
generated source content.

## Proof, caps and compatibility

Proof must cover:

- exact two-file bytes/order/default executable true and explicit false mode;
- route/capability Eq and manual Hash include path/content/order/mode; full
  request Eq includes the plan; result-key hashing stays ID-only; epoch/full-
  request comparison distinguishes changed plans and restores A/B/A;
- exact retained owner+ordinal with no rescan or non-Generated effect activation;
- Legacy/Observed mapping -> definition -> effect order, Need/outer/semantic
  polarity, left-first epoch association, child-only events and cancellation;
- generated request projection bypassing native rule classification while
  local/http/git/Builtin behavior stays byte-identical;
- invalid/duplicate/ancestor collision preflight before writes, create/write/
  flush/mode failures, source-association framing and no scratch retention;
- session-token race, discard, selection, warm reuse, changed-plan root
  replacement and A/B/A restoration; and
- immutable-instance source/package reads plus the accepted fixture after CLI
  rebuild, or an exact newly exposed typed successor boundary.

Implementation caps are <=500 production, <=850 proof and <=1,350 aggregate
added Rust lines. Physical ceilings in table order are <=5,000/17,000/5,110/
4,160/1,780/900/6,000. Add no `rustfmt::skip`.

The admitted fixture's ordered ASCII bytes and executable polarity are **exact
Bazel 9.2**. Valid-Unicode paths, owner/ordinal, structural plan/request
identity, source association, staging and immutable-root publication are
**Slug-native**. Other repository-context members, overwrite/delete/symlink/
download/execute effects, nonroot rule definitions, Label/StarlarkPath, non-
POSIX mode behavior, broader platform paths, public query breadth and exact
Bazel configuration/output bytes remain **unsupported/deferred**.

Pinned Bazel 9.2 is behavioral authority. Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept-only guidance for a
natural selected producer, private output owner and immutable effect handoff.
Copy no Zig code, representation, scheduler, digest, manifest/root layout or
output vector.

Implement only this seven-file vertical. Run formatting; focused Bzlmod/core
proof; full Bzlmod, loading and core serially;
rebuild `slug_cli_v2`; clean `slugd` before/after the retained fixture; run the
fixture, archive status, diff/scope/hash/accounting and independent review.

`repository_io.rs` may add one private generated-effect I/O trait/native
implementation and colocated `cfg(test)` scripted implementation, following the
existing archive-materializer pattern. It exists only to prove root/parent/
create/write/flush/chmod failures and must retain no handle or enter DICE/API
identity. Add no other injection seam.

STOP a request field, plan bytes in RepoSpec attributes, side table keyed by
canonical repo/request ID, owner/ordinal in physical request ID, public
capability-source variant, certificate-internal export, core rule reload,
parent event replay, non-Generated effect call, new key/store/cache/lock/task,
Bzlmod -> loading dependency, retained Starlark/root/I/O handle, direct DICE
filesystem write, fixture edit, public behavior, Java/JVM, M7 closure, M8/M7B
and identity-byte work. `REPLAN` before widening or exceeding a cap.
