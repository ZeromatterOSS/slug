# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This plan owns milestone priority and acceptance state. The
[current packet](./slug-v2-subplans/current-packet.md) owns the one active
contract; stage plans own subsystem invariants and future acceptance gates.
The orchestration skill owns execution/recovery, and the
[authoring guide](./slug-v2-plan-authoring-guide.md) owns packet readiness.
Historical evidence is indexed in [plan-history.md](./plan-history.md) and
never selects work. This compaction changes no accepted compatibility surface.

## Live Status

| Milestone | Status | Accepted boundary / remaining gate |
|---|---|---|
| M0: archive and baseline | accepted | V1 archive refs and clean-root baseline in `9897e940`; keep archive identities |
| M1: one semantic spine | accepted | shared daemon DICE, immutable requests/observations/source certificates, atomic publication, overlapping isolation and warm reuse; historical acceptance inventory in plan-history |
| M2: analysis graph | accepted, Slug-native identity | recursive configured graph; exact configuration/output bytes remain M9 |
| M3: query | accepted | 16 default functions and admitted text formats, including `attr()` through `ed38f82a`; Sky Query/non-text breadth deferred |
| M4: cquery | accepted | shared configured graph, structural/null identity, admitted topology/formats, error and daemon lifecycle behavior |
| M5: aquery | accepted, bounded FileWrite | owner-complete configured graph and admitted literal/deps output; per-family ActionKey fields have independent exact/Slug-native/deferred classifications |
| M6: execution/caching | accepted, bounded FileWrite | sole resolved semantic input, canonical REAPI SHA-256, selected platform and daemon restoration; zero direct-local actions |
| M7A: bootstrap-critical breadth | partial; current milestone | authentic configured fixture, combined selected-request/conflict acceptance, execution groups and only the remaining bootstrap closure capabilities |
| M8: bootstrap | blocked on M7A | Bazel developer graph and BuildBuddy cache/RBE gates accepted; CI not admitted; Stage 10.3/10.4 self-hosting remains unproved |
| Cache library release | deferred until M8 | Stage 11 standalone Rust consumer, remote AC/CAS and Bazel disk-cache interoperability; bootstrap establishes the shared core |
| M7B: remaining breadth | deferred until M8; mixed-language repository follows cache library release | run/test/BEP, unrelated rulesets, extra formats and product extensions not demanded by bootstrap |
| M9: exact identity/inspection bytes | deferred until functional bootstrap | retain four-domain evidence `f00e99db`; exact configuration/output bytes and unadmitted exact ActionKey projections |

Current platform priority is Linux bootstrap. Windows-specific functionality
may remain minimal stubs or explicit unsupported boundaries until a suitable
platform is available for effective testing; do not expand Windows execution
or compatibility work merely to close a Linux bootstrap packet.

### Current packet

Packet: WP-7-425-m7a-general-repository-process

The active capability replaces GCC/Homebrew process profiles with one general
sequential repository workspace and command-fresh actual process observations.
Inactive Core now covers rooted mutation/archive IO, complete snapshots/sealing,
owned confined execution, selected native source IO/transport and full selected
process-view manifests/admission with ordinary semantic records. Its real CLI,
registry and protected transport evidence is linked from the
[current packet](./slug-v2-subplans/current-packet.md).
The inactive original Loading route now joins actual native execution and
same-root adoption across command-fresh cquery requests, with ordered binary
output and cancellation retirement. Its bounded fixture evidence is linked from
the current packet. Trusted internal Native configuration now owns fresh command
scopes while preserving the permanent Host/Registry owner and original data
deadlines. The inactive selected-source bridge now captures Workspace, Local,
selected BCR tar.gz and pinned Builtin sources through ordinary repository
Starlark, with fresh command ownership and read-only digest manifests.
Original General outputs now pass ordinary analysis and actual NativeLink
CAS/runfiles, corruption/restoration and runtime-retirement proof. The distinct
SourceRoot snapshots now pass ordinary Analysis and actual NativeLink transport
for already captured Workspace, Local, selected BCR and Builtin inputs, with
fresh-scope/lifetime proof. [Source-backed links and generic read-only Archive
quota contraction](./slug-v2-subplans/wp1225-source-links-acceptance-2026-11-08.json)
now pass unchanged `cc_autoconf` generation/linked configuration Loading and
linked Analysis/CAS/runfiles under the fixed bank.
[Atomic profile retirement](./slug-v2-subplans/wp1225-profile-retirement-acceptance-2026-11-08.json)
removes both GCC/Homebrew adapters and makes default repository Execute
explicitly unsupported; inactive originals now cover Homebrew environment A/B/A
wrapper changes/restoration. [Read-only Source backing contraction](./slug-v2-subplans/wp1225-source-quota-acceptance-2026-11-08.json)
now closes the warm compiler capacity refusal: four actual commands pass on one
runtime and the unchanged bank. Full archive-envelope, derived Source Path
operations, repeated Workspace publication, public Rust and matching-command
memory gates still precede General production activation.
The [retention-dimension component](./slug-v2-subplans/wp1225-retention-dimensions-acceptance-2026-11-08.json)
separates root capacity128 from byte-bounded complete history and explicitly
bounds command-wide execution. The
[observation-cutoff component](./slug-v2-subplans/wp1225-observation-cutoffs-acceptance-2026-11-08.json)
now preserves exact per-repository dependencies with causal cache/freshness proof.
Same-input bounded diagnostics observe about1.9s less root-compute wall time;
The [owned-verification component](./slug-v2-subplans/wp1225-owned-observation-acceptance-2026-11-08.json)
now keeps full actual byte charges while sharing canonical General bodies.
The [verified Source-placement component](./slug-v2-subplans/wp1225-source-placement-acceptance-2026-11-08.json)
now also replaces read-only MODULE/overlay members safely. Rust passes the
permission refusal. The
[materialization-result cutoff](./slug-v2-subplans/wp1225-materialization-cutoff-acceptance-2026-11-08.json)
now preserves completed results across unrelated publications, with actual-key
RED/GREEN and native admission proof. Bounded family counts leave the Analysis
CapturedSourceSnapshotKey unreached in observed prefixes; its cutoff remains
deferred. Alternating same-input probes reduce result recomputes but
retain variable prefixes and the unchanged work deadline. The
[validated observation union](./slug-v2-subplans/wp1225-observation-union-acceptance-2026-11-08.json)
now reuses no-op maps and full left key/result owners with original RED/GREEN
and native proof. Bounded candidates reuse612 measured maps, but retain the same
recorded epoch and work deadline. Prerequisite serialization is the next
candidate to discriminate; Rust archive extraction, completion, command latency
and RSS acceptance remain unproved.

Authentic Rust analysis, required action comparison, Rust REAPI execution,
complete Slug closure, Stage 10 and whole-goal memory fit remain open.

### Bootstrap command queue

| Order | Observable result | Dependency |
|---|---|---|
| 1 | Authentic Rust cquery through general APIs | General Args accepted; demanded provider/toolchain/repository API replacements remain |
| 2 | Required action comparison and a small authentic Rust REAPI build | Source-derived providers/toolchains, generic Args and real tool inputs |
| 3 | Full Slug CLI closure analyzes and builds through the same owners | Demanded proc macros, build scripts, generated sources and native actions |
| 4 | Stage 10.3 graph comparison and 10.4 stage1/stage2 fixed point | No semantic adapters; exact output and REAPI proof |

Source preparation belongs to the capability it serves. Milestone priority
remains M7A, M8, cache-library release, M7B, then M9.

## Non-Negotiables

- Bazel 9 only. No pre-Bazel-9 behavior, no WORKSPACE support, and no legacy
  toolchain-resolution compatibility.
- Bazel source and Bazel tests are the compliance oracle. A parity claim needs
  a local Bazel source citation or an oracle fixture result.
- DICE owns semantic build state. Do not hide semantic discovery inside
  synchronous Starlark-visible APIs.
- REAPI is the execution boundary. BuildBuddy is the primary scaled remote
  development/CI lane; sibling `../actiond` is the preferred hermetic local
  conformance backend; NativeLink remains a useful regression backend. All sit
  behind the same REAPI boundary.
- Bazel invocations may use ordinary RC discovery and consume the user's
  `~/.bazelrc` for BuildBuddy authentication. Agents and inspection tools must
  never read or copy its contents, and credentials or derived secret material
  must never enter this checkout, logs intended for commit, or Git history.
- Slug-local sandbox implementation is deferred until after analysis, admitted
  `aquery` semantics, remote execution, and cache correctness. Backend isolation supplied
  by BuildBuddy or actiond does not count as a Slug sandbox implementation.
- Progress is demonstrated by a vertical Bazel-shaped build, not by independent
  identity, parser, DICE-shaped, action, or REAPI data models. A wrapper trait
  or stable-serialization helper is scaffolding until the owner fixture drives
  it through the real runtime boundary.
- V2 output layout targets Bazel-shaped paths. Any deliberate Slug-specific
  divergence must be explicitly documented as an extension, not assumed.
- V1 plans and code are evidence and extraction sources, not the V2 source of
  truth.
- New packets and replans follow
  [slug-v2-plan-authoring-guide.md](./slug-v2-plan-authoring-guide.md): name
  learned facts, decisions and non-decisions, exact/Slug-native/deferred
  classification, natural producer ownership, request/revision behavior,
  memory lifetime, upstream tests, fallback deletion, scope, and stops.

## Current Milestone Overlay

Stages are ownership boundaries, not execution order. Preserve
**M7A → M8 → cache library release → M7B → M9**. Linux self-hosting is the first
product milestone. Establish the graph-independent cache core within demanded
M7A execution work; standalone packaging, direct Bazel disk-cache compatibility,
C/Python bindings and mixed-language breadth are post-bootstrap work.
Exact Bazel ActionKeys do not gate functional action admission or self-hosting.
Preserve accepted exact fields, complete semantic identity/invalidation and
exact REAPI/CAS digests; classify each unadmitted inspection field explicitly.
The finite bootstrap readiness matrix owns the
capability-to-target/evidence mapping. Stage 10 comparison uses ordinary graph
owners; no bootstrap-only path, precomputed runtime graph or hidden Cargo/Bazel
executor is admitted.

## Stage Map

| Stage | Owner Plan | Parallelism | Checkpoint |
|-------|------------|-------------|------------|
| 0 | [00-v1-archive-and-clean-root.md](./slug-v2-subplans/00-v1-archive-and-clean-root.md) | Serial | V1 is tagged/branched, V2 root docs and metadata are active, archive policy is clear. |
| 1 | [01-compliance-oracle-harness.md](./slug-v2-subplans/01-compliance-oracle-harness.md) | Parallel | A fixture runner compares Java Bazel and Slug V2 for exit status, outputs, events, and selected diagnostics. |
| 2 | [02-rust-skeleton-and-runtime-substrate.md](./slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md) | Parallel | Minimal Rust CLI/server skeleton uses actual Buck2 runtime crates without exposing Buck semantics. |
| 3 | [03-bazel-identity-and-layout.md](./slug-v2-subplans/03-bazel-identity-and-layout.md) | Parallel after Stage 2 starts | Labels, repositories, packages, target ids, execroot, and output paths are Bazel-shaped. |
| 4 | [04-starlark-loading-and-build-packages.md](./slug-v2-subplans/04-starlark-loading-and-build-packages.md) | Parallel after Stage 3 basics | `BUILD.bazel` and `.bzl` loading work for small packages with Bazel globals. |
| 5 | [05-bzlmod-and-repository-graph.md](./slug-v2-subplans/05-bzlmod-and-repository-graph.md) | Parallel after Stage 3 basics | Starlark-evaluated `MODULE.bazel`, registry, repo mapping, extensions, repo specs, and lockfile policy are DICE-owned. |
| 6 | [06-analysis-toolchains-and-actions.md](./slug-v2-subplans/06-analysis-toolchains-and-actions.md) | Parallel after Stages 4/5 | Configured-target analysis, toolchains, providers, depsets, and action declarations pass focused oracle fixtures. |
| 7 | [07-reapi-native-execution.md](./slug-v2-subplans/07-reapi-native-execution.md) | Parallel with synthetic actions, then after Stage 6 | Shell and ruleset actions execute through REAPI with upload, AC, materialization, and zero direct-local proof. |
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Query after loading/analysis; execution commands after admitted aquery semantics | `query`, `cquery`, and classified `aquery` comparisons pass before the corresponding ruleset, run, test, and BEP breadth. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |
| 10 | [10-bazel-build-and-bootstrap.md](./slug-v2-subplans/10-bazel-build-and-bootstrap.md) | Bazel developer graph may start now; self-hosting follows classified aquery comparison and execution | Bazel 9 builds/tests Slug through BuildBuddy, then Slug reaches a stage1/stage2 self-build fixed point. |
| 11 | [11-bazel-compatible-cache-library.md](./slug-v2-subplans/11-bazel-compatible-cache-library.md) | Core boundary with M7A execution; standalone release after M8 | Slug consumes a graph-independent cache core; external Rust applications share remote and Bazel disk caches. |

## Supporting contracts

- [DICE ownership](../../../docs/developers/dice.md) owns runtime invariants.
- [Stage 9](./slug-v2-subplans/09-v1-extraction-ledger.md) owns retained reuse
  decisions; V1 is archival reference only.
- [Adoption roadmap](./slug-v2-subplans/zabel-adoption-roadmap.md) supplies
  bounded future work, not an alternate scheduler or Bazel compatibility oracle.
- [Plan history](./plan-history.md) preserves original acceptance inventories,
  source audits, and rejected experiments at immutable Git revisions.

Future branding consideration: Rubin (Red Rubin basil) remains a product
choice outside implementation milestones.
