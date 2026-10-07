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

Packet: WP-7-231-m7a-normal-rustc-configured-producer-readiness-r1
Status: Offline scoped source guard accepted; normal public Rust source readiness continues

The [current manifest](./slug-v2-subplans/current-packet.md) moves toward a normal
retained Slug Rustc producer using the unchanged Bazel9.2/rules_rust0.73 public
probe fixture and existing Spawn view/query APIs. First prove exact offline source,
generated mapping and provider readiness, then normal configured action ownership.
Historical oracle bytes do not certify current sources; missing prerequisites stay
typed, with no acquisition, source16/17/18/F3/fullroot retry or invented providers.
Accepted source prerequisites are indexed in the current manifest: the scoped
[offline guard](./slug-v2-subplans/wp932-offline-source-guard-2026-10-06.json),
[explicit alias](./slug-v2-subplans/wp934-explicit-root-alias-2026-10-06.json),
[finite source reads](./slug-v2-subplans/wp935-explicit-local-source-2026-10-06.json),
[direct imports](./slug-v2-subplans/wp936-direct-local-imports-2026-10-06.json),
[native parser ownership](./slug-v2-subplans/wp937-native-source-memory-2026-10-06.json),
[initial offline command](./slug-v2-subplans/wp938-normal-offline-source-prerequisite-2026-10-06.json)
and [borrowed attribution](./slug-v2-subplans/wp939-borrowed-registry-attribution-2026-10-06.json).
Detailed chronology remains in those receipts and this plan at commit4ddc6c29a;
representative parsing fits the memory envelope, while module/provider/full graph
readiness remains unproved and all spent refusals remain closed.
[WP940](./slug-v2-subplans/wp940-exact-offline-registry-prerequisite-2026-10-06.json)
records the sole complete platforms1.1.0 MODULE request at offline refusal, under
unchanged caps. [WP941](./slug-v2-subplans/wp941-platforms-local-module-readiness-2026-10-06.json)
proves evaluated root declaration/forward URL correspondence and observes one local
platforms MODULE416B with partial literal1.1.0 declaration/dependencies. Seven
checks pass (one new test case); first source read0.052s/18,572KiB, corrected preparation18.581s
and Core check6.834s pass. Its partial candidate facts do not certify registry/MVS provenance.
[WP942](./slug-v2-subplans/wp942-native-local-module-readiness-2026-10-06.json)
accepts genuine native426B root MODULE loading/evaluation and preserves the normal
mapping's classified offline registry transport prerequisite. Five focused tests and one
native observer pass; Core preparation9.537s and CLI check7.358s pass. Native0.226s peaks
66,816KiB under256MiB, with original source/recorder/scope Weak and buffer-credit retirement.
The first retained raw URL spells modules/bazel_features/1.50.0/MODULE.bazel, but
sole-offline qualification is false and its selected typed caller/MVS provenance remains unproved. Next ground that
association or exact local availability before another source operation. Normal platforms
mapping/certification, generated prerequisites and retained public Rustc production remain
open. Representative memory gates permit this broader readiness work; whole-workload fit
and all spent source/acquisition/full-root limits remain unchanged.
[WP943](./slug-v2-subplans/wp943-borrowed-module-registry-association-2026-10-06.json)
accepts a borrowed selected-discovery name/canonical-version/outer registry request on
the existing mapping-error owner.26 production/189 test lines add no retained strings or
DICE behavior; six owner gates pass in0.727s/30,940KiB, owner preparation5.636s and direct
Core/CLI check8.203s pass. Next ground exact current source availability or a separately
frozen typed request observation. No new native/source/acquisition operation was run;
current selected identity/provenance, module/provider/producer and whole-fit remain open.
[WP944](./slug-v2-subplans/wp944-bazel-features-local-source-2026-10-06.json)
observes one187B bazel_features+ link and its exact MODULE346B/SHA2083ef9c… with
partial literal1.50.0 declaration. Two source operations0.031s/17,652KiB and0.042s/18,776KiB
pass their bounds; fifteen preserved controls reused, no Rust/test/fixture growth.
Two unparsed directives and selected identity/provenance remain unknown. Next freeze
normal native Host/local-materializer readiness with this exact fourth source and borrowed
actual error reporting; no native/import/acquisition/spent-source operation selected.
Module/mapping/provider/public producer and whole-fit gates remain open.
[WP945](./slug-v2-subplans/wp945-native-borrowed-dependency-readiness-2026-10-06.json)
accepts the opt-in four-root normal native observer and actual borrowed bazel_skylib1.8.2
discovery request. Six controls1.191s/55,480KiB and native0.241s/67,312KiB pass; original
root426B/hash/Weak and zero final credits qualify readiness. Four parent source brackets
cover5669B; sibling refusal precedes bazel_features native source/evaluation. Separate
Core preparation16.238s/CoreCLI check7.334s pass, no production/fixture changes. Next ground
that actual request's local source and continue normal loading. Mapping/provider/public
Rustc/M7A/full-workload gates remain open; no new source/acquisition/retry selected.
[WP946](./slug-v2-subplans/wp946-native-skylib-source-readiness-2026-10-06.json)
accepts exact skylib MODULE389B availability and shared opt-in five-root observer.
Seven controls1.396s/56,656KiB and native0.240s/67,360KiB pass; five parent bindings6058B,
root426B/hash/Weak and zero final credits succeed. Both added native sources remain unseen;
actual borrowed semantic prerequisite is rules_cc0.2.4/outer59B MODULE request. Separate
Core37.015s/CoreCLI7.265s pass. Representative runtime memory fits unchanged caps; continue
source-specific normal loading toward public Rustc with borrowing/scoped interning discipline.
Mapping/provider/public producer/MVS/provenance/whole-workload gates remain open.
[WP947](./slug-v2-subplans/wp947-native-local-version-readiness-2026-10-06.json)
accepts the explicit Local0.2.17 rules_cc body1181B after preserving failed0.2.4 registry
correspondence and reviewing normal empty-effective-version semantics. Version1control,
8Corecontrols1.602s/56,112KiB and native0.241s/64,864KiB pass; six parent bindings7239B,
root/hash/Weak and final credits succeed. All3extras remain unseen in the native epoch.
Separate Core10.095s/CoreCLI7.324s pass. Actual next request is rules_license1.0.0/outer64B.
Continue its normal source owner under borrowing/scoped interning and unchanged caps;
mapping/provenance/provider/public Rustc/full-workload fit remain open.
[WP948](./slug-v2-subplans/wp948-native-license-source-readiness-2026-10-06.json)
accepts exact license753B/literal1.0.0 local availability and shared Seven observer.
Nine controls1.774s/53,924KiB and native0.242s/67,012KiB pass;7parent bindings7992B,
root426B/hash/Weak and zero credits succeed. All4extras remain unseen in the epoch.
Separate Core9.705s/CoreCLI7.309s pass; prior version evidence is reused0newruns.
Actual next typed request is rules_shell0.6.1/outer62B MODULE. Continue its source-specific
normal owner under existing memory/borrowing/interning limits; no mapping/providers/public
Rustc/MVS/provenance/full-workload fit admission.
[WP931](./slug-v2-subplans/wp931-generated-compiler-env-files-2026-10-06.json)
accepts446 current generated env-file reads (190,700B) and466 ordered bindings to411
saved consumers.13 controls, a2.233s observation and two FINAL reviews pass. Shared
vectors/19 report key names and per-consumer retirement keep conservative peak RSS
120,588KiB under256MiB; traced post-drop4,848,642B retains compact report/helper leaves.
The observations establish file/OUT_DIR subset correspondence, not Slug whole-memory
fit or inherited effective environment/producer freshness. All67 insertedOUT_DIR values
retain pwd uncertainty. [WP930](./slug-v2-subplans/wp930-effective-compiler-arguments-2026-10-06.json)
keeps all513 argument projections qualified. The accepted native header capability
[WP929](./slug-v2-subplans/wp929-native-build-info-header-publication-2026-10-06.json)
has no saved root consumer selecting more header/C++ execution. M7A/full-root/whole-fit,
authentic compiler execution/cache/REAPI and prior memory/source limits remain open.

### Ready and blocked queue

The current packet selects normal retained compiler producer readiness under priority1. Declared
header/C++ capabilities do not select execution without requested-root demand; the
remaining priorities stay queued until concrete prerequisites and owners are ready.

| Order | Result | State / dependency |
|---|---|---|
| 1 | Configured production graph coverage | current36 local owners/353 external normal direct-feature owners and selected generated cfg bytes accepted; saved argument/file-env subsets reconciled; current packet selects normal retained compiler producer readiness; runtime context/current freshness remain qualified |
| 2 | Bazel root compilation and action coverage | current CLI compilation accepted; Slug per-family action coverage remains open |
| 3 | Remaining M7A action/input-tree/REAPI capabilities and shared cache core | native header producer accepted with zero saved root consumers; demanded execution rows in [bootstrap readiness](./slug-v2-subplans/bootstrap-readiness.md) remain open |
| 4 | Stage 10.3 graph comparison, then 10.4 fixed point | blocked on finite M7A closure; use reviewed typed comparison contract |
| 5 | Standalone remote/disk cache library | blocked on M8; [Stage 11](./slug-v2-subplans/11-bazel-compatible-cache-library.md) owns release gates |
| 6 | M7B mixed-language/command breadth, then M9 exact identity/inspection | separate functional ruleset and exact projection admission |

The user treats 12 seconds as a guideline, not an exact test rule. Keep tests
as small as the semantic boundary allows; anything over a few seconds should
run infrequently, and a test over roughly 30 seconds needs serious necessity
review. The one opt-in 30-second F3 proof was independently reviewed, ran once
and is closed without success. Compile/preparation remains separate and must
be split or checked in smaller units when necessary. An unattributed timeout
is not a semantic failure.
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
