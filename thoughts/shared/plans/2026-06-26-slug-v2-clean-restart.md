# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This is the canonical Slug implementation plan after the V1 archive decision.
The January roadmap and numbered V1 subplans remain valuable reference material,
but new implementation work should start from this plan and the V2 subplans
under [slug-v2-subplans](./slug-v2-subplans/).

Slug keeps its name and repository. The archived V1 implementation is a Buck2
fork migrated toward Bazel compatibility. V2 keeps the proven lessons and
selected code from V1, but the active trunk is a Bazel-shaped Rust
implementation from the first architectural boundary.

## Operating Decision

Use the existing repository for continuity, but restart the implementation
shape:

1. Preserve V1 through a tag and archive branch before root-level replacement.
2. Keep V1 source as extraction/reference material, not as the default build
   graph for V2.
3. Build V2 around Bazel 9 semantics, Bazel source/test oracle fixtures, DICE,
   starlark-rust, and REAPI-first execution.
4. Import V1 code only after a small oracle fixture or focused regression proves
   the behavior matches the V2 boundary.

## 2026-07-22 Direction Reset

The immediate goal is not broader build execution. It is one trustworthy,
incremental semantic graph that can reproduce Bazel 9 analysis and expose that
graph through `query`, `cquery`, and `aquery` in increasing order of depth.

The governing order is:

1. Pin all new oracle work to Bazel 9.2.0 at
   `8220c6198837d5c13d53fea211cf3282aa12408a`. The sibling `../bazel`
   checkout may move to Bazel 10 or later; use the tag/commit, not its current
   `HEAD`, for parity evidence.
2. Replace split one-shot evaluation and fallback workspace scanning with one
   daemon-owned DICE graph whose injected inputs cover files, directory
   listings, environment and command policy, repository mapping, loading,
   configured targets, and action declarations.
3. Make configured-target analysis real: recursively analyze dependencies,
   execute rule implementations with prepared Bazel-shaped contexts, consume
   returned providers, and retain declared actions without executing them.
4. Implement full unconfigured `query` over the loading graph, then `cquery`
   over configured targets, then exact `aquery` over the same action graph
   Stage 6 produces.
5. Treat matching `aquery` output as the execution handoff. Only after this
   gate should new execution/cache breadth, `run`, `test`, or broad ruleset
   conformance control the next milestone.
6. Maintain a Bazel 9 build graph for Slug itself so Bazel plus BuildBuddy can
   accelerate development. After analysis, action graph, and execution are
   correct, prove a Bazel-built Slug can build Slug and then reach a Slug-built
   fixed point.

The already-landed first-build and NativeLink-backed REAPI fixtures remain
valuable regression tests. They prove a narrow vertical slice; they do not
prove the DICE ownership, configured-target graph, query surface, or bootstrap
architecture described above.

### Integration-first freeze

- Do not expand Stage 5 with more standalone parser/key/value substrate unless
  the packet is required by the analysis/query/aquery path.
- Do not expand Stage 7 cache, materializer, or backend breadth until the
  `aquery` gate is accepted, except to preserve an already-landed regression or
  to enable the Bazel/BuildBuddy developer build.
- Do not use a real-world build as structural acceptance evidence. Convert each
  discovered gap into a focused Bazel 9 oracle first.
- Historical checkpoint sections remain evidence of what landed. The latest
  priority/gate section in this plan and each owning subplan is authoritative
  when older checkpoint prose says `pending`, `next`, or `first`.

Do not physically move the whole V1 tree into `v1-archive/` unless the tag and
branch archive is not enough. A full in-tree archive makes search, codegraph
indexing, and new-agent orientation worse. If a physical archive is required,
exclude it from active build metadata and codegraph indexing.

## 2026-06-29 Branch Review And Remediation Gate

Review of `codex/slugv2` found that the clean-restart archive sequence has not
actually been completed in this checkout:

- `scripts/v2_archive_status.sh` fails because `slug-v1-archive` and
  `v1-archive` are missing, even though Stage 0 docs recorded them.
- `codex/slugv2` adds V2 scaffolding on top of the full V1 root instead of
  resetting the active tree into a clean V2 root. Relative to `main`, the branch
  adds hundreds of files and no root cleanup.
- `Cargo.toml` still includes the V1 `app/slug*` workspace members beside the
  new `app/slug_*_v2` crates, and the active tree still tracks V1-heavy paths
  such as `app/`, `buck2/`, `prelude/`, and `tests/`.
- A focused V2 compile check passed for the new crates, so the branch is useful
  as a prototype and selective patch source, but it is not the V2 trunk shape.

Do not merge or promote the current `codex/slugv2` branch wholesale as the clean
restart. Before implementation proceeds as V2 trunk, do this sequence:

1. Freeze new feature work on the mixed-root branch.
2. Pick the V1 preservation commit from the live checkout, verify the worktree
   state, then create and validate the `slug-v1-archive` tag and `v1-archive`
   branch.
3. Start the active V2 line from a clean root worktree: keep root pointers,
   V2 plans, and intentionally retained infrastructure; remove V1-only source,
   tests, Buck-shaped metadata, and V1 workspace members from the active build.
4. Re-import from `codex/slugv2` one bounded stage at a time. Each import needs
   an owner subplan, an oracle fixture or Bazel source citation, focused
   validation, and a Stage 9 extraction-ledger entry when it came from V1 or
   from the mixed-root prototype.
5. Run `scripts/v2_archive_status.sh`, `git diff --check`, and the touched
   stage validation before calling the root clean.

Use the saved implementer prompt at
[thoughts/shared/prompts/2026-06-29-slug-v2-generic-implementer.md](../prompts/2026-06-29-slug-v2-generic-implementer.md)
for sessions that continue this remediation.

2026-06-29 execution update: the missing local archive refs have been repaired;
`slug-v1-archive^{commit}` and `v1-archive` now both resolve to
`e218054d4c796655939b968d90208b185decb352`. Cargo root metadata now exposes only
V2 app crates as active `app/slug_*` workspace members/dependencies, with V1
app crates removed from that surface.

2026-06-29 clean-root remediation update: the active clean-root branch is
`codex/slugv2-clean-root-remediation`. It removes tracked V1 source/test trees,
root Bazel/Buck metadata, old CI, old docs, old V1 plans, and the unwrapped
`remote_execution` source candidate from the active tree. The retained tracked
root is orientation docs, V2 plans/prompt, Stage 1 oracle harness, V2 crates,
repo-local V2 skills, `docs/developers/dice.md`, and the explicitly retained
infrastructure crates listed in `V1_ARCHIVE.md`. V1 and rejected mixed-root
surfaces remain available through `slug-v1-archive`, `v1-archive`, and
`codex/slugv2` for staged extraction only.

2026-07-22 live-checkout correction: the annotated `slug-v1-archive` tag still
resolves to `e218054d4c796655939b968d90208b185decb352`, but the local
`v1-archive` branch is absent and the archive checker allowlist predates
`app/slug_server_v2`. Stage 0 is therefore not green in the live checkout; its
owner plan records the bounded repair before M0 acceptance.

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
- Slug-local sandbox implementation is deferred until after analysis, exact
  `aquery`, remote execution, and cache correctness. Backend isolation supplied
  by BuildBuddy or actiond does not count as a Slug sandbox implementation.
- Progress is demonstrated by a vertical Bazel-shaped build, not by independent
  identity, parser, DICE-shaped, action, or REAPI data models. A wrapper trait
  or stable-serialization helper is scaffolding until the owner fixture drives
  it through the real runtime boundary.
- V2 output layout targets Bazel-shaped paths. Any deliberate Slug-specific
  divergence must be explicitly documented as an extension, not assumed.
- V1 plans and code are evidence and extraction sources, not the V2 source of
  truth.

## V1 Material Worth Keeping

Preserve and mine these V1 surfaces:

- DICE-owned bzlmod/replay implementation and tests in `app/slug_bzlmod` and
  `tests/core/bzlmod/test_plan61_guardrails.py`.
- REAPI/NativeLink smoke tests, what-ran evidence, upload/materialization
  checks, and remote action-cache tests from Plans 31 and 34.
- Bazel Starlark API work: `rule(implementation=...)`, `attr.*`, providers,
  depset probes, `ctx.actions`, and selected `cc_common` or `proto_common`
  compatibility surfaces.
- Repository-rule and module-extension lessons, especially lockfile replay,
  repo mapping, watched inputs, and materialization guardrails.
- Plan docs as a bug database for known semantic traps.

Do not import these V1 surfaces without redesign:

- Buck cell identity and fallback cell graph machinery.
- `buck-out` or Buck-shaped output-root assumptions.
- Direct-local executor shortcuts used as compatibility proof.
- Process-global semantic registries, hidden bridges, or fallback scanners that
  bypass DICE ownership.
- BXL or other Buck-derived user surfaces unless deliberately scoped as Slug
  extensions after Bazel compatibility is stable.

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
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Query after loading/analysis; execution commands after aquery | `query`, `cquery`, and exact `aquery` pass before ruleset, run, test, and BEP breadth. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |
| 10 | [10-bazel-build-and-bootstrap.md](./slug-v2-subplans/10-bazel-build-and-bootstrap.md) | Bazel developer graph may start now; self-hosting follows exact aquery and execution | Bazel 9 builds/tests Slug through BuildBuddy, then Slug reaches a stage1/stage2 self-build fixed point. |

## Current Milestone Overlay

The numbered stage files are ownership boundaries, not the implementation
order. Use this overlay for scheduling new packets:

| Milestone | Required result | Owning stages | Exit gate |
|-----------|-----------------|---------------|-----------|
| M0: archive and baseline health | V1 refs and clean-root checker are truthful; Bazel/Buck2/actiond sources are pinned | 0, 1, 9 | Archive status is green and every new fixture carries immutable provenance. |
| M1: one semantic spine | One daemon-owned DICE instance and explicit create/edit/delete inputs serve loading, bzlmod, analysis, and commands | 2, 4, 5 | Same-daemon tests explain invalidation without a fallback scanner or fresh per-request graph. |
| M2: analysis graph | Recursive configured targets return real providers and deterministic declared actions without execution | 3, 4, 5, 6 | Focused Bazel analysis fixtures and upstream test themes match. |
| M3: `query` | Full Bazel 9 unconfigured query language evaluates the loading graph | 8, 9 | Parser, function, set-operation, pattern, ordering, and formatter fixtures match. |
| M4: `cquery` | Configured query reads the same configured-target graph as analysis | 6, 8 | Configuration identity, transitions, providers, and Starlark output match. |
| M5: `aquery` | Action query reads the same Stage 6 action graph and implements Bazel 9.2.0's text, commands, summary, textproto, proto, streamed_proto, and jsonproto formatters | 6, 8 | Normalized `ActionGraphContainer` content and human-readable renderings are identical for the gate matrix. |
| M6: execution and caching | Stage 6 actions execute and replay only through REAPI | 7 | BuildBuddy and local actiond evidence prove upload, execute, AC, and materialization with zero direct-local actions. |
| M7: command/ruleset breadth | `build`, `run`, `test`, BEP, and public rulesets use the accepted graph and executor | 8 | Focused public fixtures match; stress projects remain supplemental. |
| M8: bootstrap | Bazel-built Slug builds Slug and reaches a self-hosted fixed point | 10 | Stage1 and stage2 action graphs and declared outputs match. |

M3 progress: implementation commit `61ca25db` lands the first accepted
DICE-backed loading-query thin vertical over the root repository, with
Buck2-derived parser/evaluator/traversal seams and retained-daemon execution.
It passes the Bazel 9.2 `query-parser-and-sets` and
`query-loading-thin-vertical` oracle fixtures through Slug. M3 remains open for
the remaining functions, repositories and patterns, ordering modes, and
formatters; this checkpoint must not be described as full query parity.
Oracle commit `5b7806d7` now pins the next accepted behavior packet for
root-repository subtree patterns, `rdeps`, and
`same_pkg_direct_rdeps`. Implementation commit `cdc5af41` passes that oracle
through the retained daemon with prefix-local package enumeration and
Buck2-derived reverse traversal. M3 remains open for the other 13 loading
functions, repository/pattern breadth, ordering modes, and formatters.
Oracle commit `2b73c08d` now pins the next 43-command packet for `allpaths`
and `somepath`, including bounded arbitrary shortest paths and Bazel's
source-backed root-node `somepath` AUTO-order exception. Implementation commit
`7d851ce9` passes that oracle with direct unbounded reverse-traversal reuse,
Buck2-derived compact BFS/parent reconstruction, exact DICE transitions, and
retained-daemon execution. M3 remains open for the other 11 loading functions,
repository/pattern breadth, ordering modes, and formatters.
Oracle commit `e8e1d9ef` now pins the next 42-command ordinary-query packet for
`some` and the shared signed Java-`int` boundary used by `deps`/`rdeps`.
Implementation commit `b25c8aff` now passes that packet through the retained
daemon: `some` selects from the existing insertion-ordered `TargetSet`, while
the shared FULL renderer deterministically topologically orders the selected
induced graph. It also carries signed `i32` depth/count values through
`deps`/`rdeps` and renders bare-negative syntax safely for UTF-8 input. Worker
and root each passed the six-crate 82-test suite and all five accepted query
fixtures (133/133 rows). M3 remains open for the other ten loading functions,
repository/pattern/order/formatter breadth; `filter` stays deferred until an
exact Java `Pattern` substrate exists.

Oracle commit `8c28877b` now lands the 40-command
`query-siblings-build-file-node` packet for `siblings(EXPR)` plus actual
BUILD-file basename behavior. Implementation, DICE/daemon evidence, and Slug
parity against the preceding six query fixtures remain pending, so M3 still
has ten deferred functions; landing siblings would leave nine. `buildfiles`
and `loadfiles` remain separate because they require transitive `.bzl` and
fake-target semantics, while regex, attribute, generated, external,
configured, and action surfaces remain out of scope.

## Two-Tier Work-Packet Contract

Use role boundaries rather than relying on a particular model to infer project
architecture. The default assignment is a Terra or Luna xhigh agent as the
**implementation worker** and a Sol agent as the **design reviewer**; another
model may fill either role only if it follows the same contract.

- The implementation worker executes one bounded packet, makes only local
  implementation choices already implied by its owner plan, edits only the
  named scope, and produces the fixture, patch, validation, and evidence bundle.
- The design reviewer owns choices that change architecture, public or
  cross-crate interfaces, DICE keys/ownership/invalidation/locking, stage
  boundaries, or V1/Buck2 reuse and adaptation boundaries. Read-only discovery
  may precede review, but implementation of such a choice may not.
- Every Stage 2-8 and Stage 10 packet begins with reuse discovery, even when
  its request does not mention an import. Before new implementation, the worker
  must inspect the
  owner plan, the matching Stage 9 candidates, relevant retained Buck2-derived
  crates, and the V1 and mixed-root refs documented by `V1_ARCHIVE.md`, then
  obtain Sol approval for the recorded reuse decisions.
- Sol review is mandatory before any reserved choice and after every validated
  packet. The post-validation result is `accept`, `revise`, or `replan`; only an
  accepted packet may be recorded as completed evidence in its owner plan.

Copy and fill this template before editing. Every field is required; use
`none` with a reason rather than omitting a field. Paths, commands, pass
criteria, and exclusions must be concrete enough that another worker can run
the packet without reconstructing its design.

```text
Work packet ID: WP-<stage>-<short-name>
Owner stage and plan: <stage number and exact V2 subplan path>
Goal and gate link: <one result; current milestone exit gate or narrow independent oracle reason>
Prerequisites and current state: <required prior packets; branch/HEAD; relevant dirty paths; observed baseline>
Oracle-first artifact: <fixture and expected artifact path, or exact local Bazel source citation, created/verified before implementation>
Reuse audit (required for Stages 2-8 and 10):
- Candidates checked: <owner-plan sections and exact Stage 9 rows>
- Sources inspected: <exact retained paths; archive or mixed-root ref@commit:path>
- Prior evidence inspected: <tests, oracle/evidence paths, and relevant results>
- Decision and Sol approval: <adopt/port/rewrite/reference-only/reject for each candidate with reason; approval reference>
Exact scope: <allowed files/symbols plus explicit exclusions>
Decisions reserved for design reviewer: <questions and affected boundary, or none with reason>
Implementation steps:
1. <ordered bounded step>
2. <ordered bounded step>
Focused validation: <exact commands and pass criteria>
Evidence and plan update: <owner-plan section; oracle/result/diff facts to record after acceptance>
Stop conditions: <state mismatch, missing oracle, dirty overlap, scope growth, reserved decision, changed failure class, or other packet-specific stop>
```

For every Stage 2-8 and Stage 10 packet, reuse discovery is required before new
implementation, including apparently greenfield work. The worker records the
Stage 9 rows and owner-plan candidates checked; the exact retained active-tree
paths and archive or mixed-root refs, commits, and paths inspected; prior tests
or evidence read; and an `adopt`, `port`, `rewrite`, `reference-only`, or
`reject` decision with a reason for each candidate. A request that names no
import is not a reason to skip the audit. If discovery finds no reusable
candidate, record the concrete sources checked and why they were rejected or
kept reference-only. Sol must approve the audit before implementation begins.

After reuse approval where required, the worker checks the stated current state
and creates or refreshes the oracle artifact. If another reserved question
appears, the worker sends Sol the packet plus the smallest relevant source/plan
evidence and waits for a recorded decision before related edits. After
implementation and focused validation, the worker sends Sol the packet ID,
scoped diff, oracle or Bazel citation, command results, and residual risks.
`revise` returns to the same packet and requires revalidation; `replan` ends it
and requires a replacement packet. Record the accepted result and reviewer
outcome compactly in the owner plan before starting another packet.

## Retained First Real Bazel Build Integration Gate

This was the first integrated implementation proof after the Stage 2 skeleton.
It is owned here because it crosses the Stage 1-7 boundaries; implementation
and detailed evidence remain in their stage owners. As of the 2026-07-22
direction reset it is a retained regression gate, not the current scheduling
gate. The Current Milestone Overlay controls new work.

The gate is:

1. `slug build` opens a real DICE transaction and evaluates a root
   `MODULE.bazel` and `BUILD.bazel` through starlark-rust.
2. A small package resolves a typed label, evaluates one custom rule, and
   produces a provider plus a shared-DAG depset and declared action.
3. The action becomes serialized REAPI `Command`, `Directory`, and `Action`
   protobufs; it uploads, executes through NativeLink, and materializes the
   declared output.
4. The matching Stage 1 fixture has a checked-in Bazel oracle, proves
   `reapi_actions=1` and `direct_local_actions=0`, and compares the declared
   output digest.
5. Once the daemon exists, an edit to the loaded `.bzl` reruns the affected
   computation in the same daemon for named DICE dependencies.

`simple-rule-action`, `shell-action-reapi`, and `load-invalidation` are the
initial fixture chain. A missing-module probe is separate: Bazel 9 creates an
empty `MODULE.bazel` with a warning, so V2 must not treat a missing module file
as a generic WORKSPACE-only failure.

Do not use this narrow build as proof that Stages 5-8 are structurally accepted.
Stage 9 records the concrete V1/Buck2 reuse that made each segment real, and the
analysis/query/aquery overlay now determines what may advance next.

This integration gate is not one implementation packet. Each packet names the
single numbered gate clause and owner stage it advances; detailed evidence
stays in that stage's plan. Cross-stage interface choices require pre-review.
After the contributing packets are accepted, a final integration packet runs
the complete fixture chain and receives Sol review before this gate is marked
complete. Passing substrate-only tests or one stage's isolated fixture cannot
substitute for that integration review.

### Gate status — 2026-07-16

All five clauses have contributing packets accepted:
1. `simple-rule-action` (clause 4, write action via REAPI) — pass
2. `shell-action-reapi` (clause 4, run_shell via REAPI) — pass
3. `bare-remote-executor-reapi` (clause 4, bare executor) — pass
4. `platform-exec-properties-reapi` (clause 4, platform properties) — pass
5. `load-invalidation` (clause 5, same-daemon DICE invalidation) — pass

The fixture chain (`simple-rule-action`, `shell-action-reapi`,
`load-invalidation`) passes end-to-end through the oracle harness with
NativeLink-backed REAPI execution and the `slug_server_v2` daemon. A final
integration review by Sol is required before the gate is marked complete.

## First Commit Scope

The first V2 implementation commit is documentation and ownership only:

- mark this plan as canonical;
- preserve the V1 roadmap as archive/reference;
- create the V2 subplans;
- update `AGENTS.md` so future workers read this plan first;
- avoid moving source code until the V1 archive tag/branch and V2 root policy
  are explicit.

Do not mix source movement, root reset, or implementation code into this commit.

## Validation

For documentation-only changes:

```bash
git diff --check -- AGENTS.md README.md thoughts/shared/plans
```

For the first real implementation slice, use the validation command in that
slice's subplan and record compact evidence in the owning V2 plan.

## Next Agent Prompt

Use
[thoughts/shared/prompts/2026-06-29-slug-v2-generic-implementer.md](../prompts/2026-06-29-slug-v2-generic-implementer.md)
for implementation sessions. The split-specific remediation instructions live
in this plan, Stage 0, Stage 9, and `V1_ARCHIVE.md`, not in the prompt.
