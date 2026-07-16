# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This is the canonical Slug implementation plan after the V1 archive decision.
The January roadmap and numbered V1 subplans remain valuable reference material,
but new implementation work should start from this plan and the V2 subplans
under [slug-v2-subplans](./slug-v2-subplans/).

Slug keeps its name and repository. The current implementation is V1: a Buck2
fork migrated toward Bazel compatibility. V2 keeps the proven lessons and
selected code from V1, but the active trunk is a Bazel-shaped Rust
implementation from the first architectural boundary.

## Operating Decision

Use the existing repository for continuity, but restart the implementation
shape:

1. Preserve V1 through a tag and archive branch before root-level replacement.
2. Keep V1 source as extraction/reference material, not as the default build
   graph for V2.
3. Build V2 around Bazel 9+ semantics, Bazel source/test oracle fixtures, DICE,
   starlark-rust, and REAPI-first execution.
4. Import V1 code only after a small oracle fixture or focused regression proves
   the behavior matches the V2 boundary.

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

## Non-Negotiables

- Bazel 9+ only. No pre-Bazel-9 behavior, no WORKSPACE support, and no legacy
  toolchain-resolution compatibility.
- Bazel source and Bazel tests are the compliance oracle. A parity claim needs
  a local Bazel source citation or an oracle fixture result.
- DICE owns semantic build state. Do not hide semantic discovery inside
  synchronous Starlark-visible APIs.
- REAPI is the execution boundary. Local execution is useful only as a REAPI
  service backend, with NativeLink first and actiond optional behind the same
  boundary.
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
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Parallel after Stages 4-7 | Modern rules_cc, rules_rust, rules_python, protobuf, query, run, test, and BEP slices pass public fixtures. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |

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
- Every Stage 2-8 packet begins with reuse discovery, even when its request does
  not mention an import. Before new implementation, the worker must inspect the
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
Goal and gate link: <one result; First Real Bazel Build clause or narrow independent oracle reason>
Prerequisites and current state: <required prior packets; branch/HEAD; relevant dirty paths; observed baseline>
Oracle-first artifact: <fixture and expected artifact path, or exact local Bazel source citation, created/verified before implementation>
Reuse audit (required for Stages 2-8):
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

For every Stage 2-8 packet, reuse discovery is required before new
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

## First Real Bazel Build Integration Gate

The first implementation milestone after the Stage 2 skeleton is a small but
complete Bazel-shaped build. It is owned here because it crosses the Stage 1-7
boundaries; implementation and detailed evidence remain in their stage owners.

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

Do not mark Stages 5-8 accepted, or add broad new substrate-only checkpoints in
those stages, until a change either advances this gate or has a narrow oracle
reason to remain independent. Stage 9 records the concrete V1/Buck2 reuse that
makes each segment real.

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
