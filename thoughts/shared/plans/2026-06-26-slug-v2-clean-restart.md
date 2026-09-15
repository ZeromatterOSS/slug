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

### Current packet

Packet: WP-4-7A-configured-conflict-path-observation-dice-event-audit-r1
Status: docs-first design pending independent review

Preserve the corrected but unaccepted R2/execution-group/computed-default/alias
stack and its independently accepted initializer and diagnostic checkpoints.
The accepted diagnostic chain narrowed cold evaluation through exact key counts,
a post-sharding census, frontier batching and installed-merge shard exposure.
The sole shard replay produced a valid `RootCompute` cutoff: 209 installed
merges changed 438 shards while adding 462 demands, with 1,528 prior residents
in changed shards. Maxima were 18 changed shards and 112 prior residents; the
exposure buckets were `[49, 124, 21, 15, 0]`. All observer, arithmetic,
PID/reap, output and cleanup gates passed.

The accepted merge-to-commit audit then recorded 218 produced and committed
handoffs, 1,591 total potentially exposed prior residents, maximum 85 and
buckets `[52, 125, 24, 17, 0]` at a valid `RootCompute` cutoff. Its corrected
133-line production cap and augmented 109/140 proof passed independent
rereview without another CLI replay. That result proves only that aggregate
potential exposure reached successful injection commits.

The sole armed-window replay then reached the 12-second deadline after
12.0113899707794 seconds in `RootCompute`. It recorded 221 installed merges,
221 matching commits and 220 closed windows with the final window active, but
all six exact `PathObservationShardKey` event counters were zero. Pending,
flags, reserved state, overflow, dropped samples and activity claim were zero;
PID/reap, output and cleanup gates passed. Independent result review accepts
this as a valid negative across 221 coherent windows. It is consistent with an
injected key producing no ordinary DICE compute event and proves no global
absence outside these atomic window edges.

The selected successor reuses that exact lifecycle, requires the final window
active with `closed = committed - 1`, and counts the six event variants only
for `PathObservationKey`, the immediate ordinary key whose
compute reads the shard key. Words 51--63 and the 100/130/120 temporary
production/proof/scratch caps remain fixed. One exact helper proof, supervisor
self-check including every consumer-arm partial, frozen artifact set,
independent preexecution review and one exact configured-conflict-selector
replay remain under the existing 12/15-second bounds. A positive
result proves only aggregate direct-dependent event membership in the armed
windows. A zero result ends this diagnostic chain; do not broaden it to
`ResolvedPathObservationKey`.
Independent design rereview returned `ACCEPT` for the corrected cutoff,
consumer-arm proof and exact replay gates.

Normal run registry propagation is accepted at `47163df7b`, typed registration
error identity at `a06f3ddfc`, bounded observer at `64c7d2475`, and source
observation diagnostics at `ac6140f41`. Their old implementation checkpoints
are historical, not pending work. The partition change preserves the sole full
accepted epoch and exact values; direct-epoch DICE callers retain their existing
lookup path.

The reconciled selected-request/output-conflict R2 candidate remains unaccepted
at local commit `f3c90ea46` on
`integration/selected-request-output-conflict-r2`, based on `2b3fedf76`. The
older source remains at `27e9e9c0c` on `review/output-conflict-r2`, based on
`97dffd5d4`. Preservation is not integration. Do not activate either candidate
partially or integrate `review-evidence/`.

### Ready and blocked queue

| Order | Result | State / dependency |
|---|---|---|
| 1 | Combined R2-based named/automatic execution-group runtime | ready; freeze the configured-target-owned contract against reconciled R2, implement on that branch, and pass joint gates before atomic acceptance |
| 2 | Authentic configured CLI fixture | F2 accepted through 28 objects; replay F3 once after combined R2/group activation |
| 4 | Remaining M7A action/input-tree/REAPI capabilities and shared cache core | select demanded rows in [bootstrap readiness](./slug-v2-subplans/bootstrap-readiness.md); Stage 11 owns the library boundary |
| 5 | Stage 10.3 graph comparison, then 10.4 fixed point | blocked on finite M7A closure; use reviewed typed comparison contract |
| 6 | Standalone remote/disk cache library | blocked on M8; [Stage 11](./slug-v2-subplans/11-bazel-compatible-cache-library.md) owns release gates |
| 7 | M7B mixed-language/command breadth, then M9 exact identity/inspection | separate functional ruleset and exact projection admission |

Tests retain the user's 12-second deadline/15-second absolute ceiling.
Compile/preparation is separate and bounded to 60 seconds; split/check smaller
units when necessary. An unattributed timeout is not a semantic failure.
Cumulative path-epoch fanout remains a separate measured concern: the recorded
retry 116 recomputed 109 path keys and checked 163 module/192 registry keys.
It is not evidence of a semantic loop or authority for checkout-wide replay.

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
