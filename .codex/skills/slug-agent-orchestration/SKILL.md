---
name: slug-agent-orchestration
description: Run Slug V2 implementation-plan or roadmap goals through bounded, reviewed work packets. Use for `/goal follow the implementation plan`, next-packet selection, delegation, model routing, integration, or routing records.
---

# Slug Agent Orchestration

The root owns priority, architecture, worktree safety, integration, status, and
commits.

## Start

1. Read `AGENTS.md` and
   `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.
2. Compare its packet ID with canonical **Current packet** using a targeted
   search. If they differ, stop and report the mismatch to the root; only the
   root may reconcile scheduling documents.
3. Check `git status --short --branch` and overlapping dirty diffs.
4. Continue the manifest's packet. Read owner-plan context only for a reserved
   decision or unresolved contradiction, and Stage 9 only for retained
   representation or reuse work. Read evidence files only as the task needs.
5. Inspect agents/Cargo/`slugd` only before overlapping work, retries, or
   daemon-sensitive validation.

Read `thoughts/shared/plans/slug-v2-plan-authoring-guide.md` before creating or
materially revising a packet. Read
`thoughts/shared/plans/slug-v2-subplans/zabel-adoption-roadmap.md` only when the
current packet selects one of its workstreams; it never overrides Live Status.

Read `references/routing-log.md` only when a recent analogous packet may change
an unclear route. Read `references/parity-source-anchors.md` only when the
packet touches one of its listed surfaces.

## Route

| Work | Route |
|------|-------|
| Small read-only/mechanical | Root |
| Simple exact-oracle formatter/CLI/query slice | Root |
| Audit, oracle, focused tests, one abstraction | Terra medium |
| Approved difficult Rust/DICE/Starlark/query architecture | Terra high |
| Reserved decision or risky final review | Sol low |
| Concrete unresolved architecture miss | Sol high |

Ordinary packets use at most one write worker and one reviewer. Add read-only
audits only for distinct unresolved semantic questions; parallel writers must
own disjoint files. Correction rereviews inspect only the correction diff and
prior blocker. Never run parallel Cargo commands on one target directory.

Spawn workers and reviewers with `fork_turns="none"` unless prior conversation
is essential. Pass the packet path, diff base, compact validation summary, and
specific question; agents read shared files from disk.

When the user identifies token pressure, default to root-only serial work and
no implementation delegation. Do not run speculative audits, reconstruct
accepted evidence, or delegate mechanical work. Prefer the next observable
vertical slice over substrate breadth. Keep simple query, formatter, and CLI
slices on the root exact-oracle path unless they cross a reserved boundary.

## Packet

The manifest is the complete contract for a root-only packet. When delegating,
use `references/implementation-worker.md` and name:

- one owner and observable result;
- exact allowed files and exclusions;
- accepted Bazel 9.2 oracle or pinned-source regression;
- focused validation and stop conditions; and
- residual risk.

A new or materially revised packet is not ready until it passes the
plan-authoring checklist: learned facts and non-decisions, upstream Bazel and
applicable Buck2 tests, exact/Slug-native/deferred classification, natural
producer/key ownership, request/revision behavior, memory lifetime, fixture
provenance, fallback deletion, complexity triggers, allowlist/caps, and stops.
Use only the applicable items, but do not silently omit an applicable risk.

Add conditional sections only when used: fixture growth/hygiene for oracle
work; DICE identity/equality for semantic keys; Stage 9/Buck2/V1 reuse for
representation changes; downstream coverage for public interfaces; platform
and lifecycle evidence for daemon/platform work.

Reuse accepted discriminating evidence. Add an oracle only for an evidence
gap. Keep design, source/oracle evidence, implementation, and tests in one
logical packet when they cover one abstraction and behavior family under a
small allowlist. Use a separate design packet only for a new shared public
boundary, DICE key/lock or ownership model, cross-crate identity, destructive
action, or a decision the canonical plan explicitly reserves. Obtain one Sol
pre-review for those decisions.

Workers edit named files and run focused tests. The root inspects the diff,
adds a discriminating case for identity/equality/invalidation/order/formatting
when needed, and owns broader validation and commits.

## Accept

Check only applicable risks:

- Bazel success/failure, diagnostics, order, and output;
- structural identity, ownership, equality, and invalidation;
- applicable create/edit/delete/recreate, environment, lockfile,
  repository-mapping, and materialized-output transitions for incremental state;
- DICE-owned discovery without direct filesystem/fresh-graph bypass;
- named-surface-only activation and compact retained representation; and
- downstream behavior for changed interfaces.

When a packet touches a demonstrated hot path, require exact output/RPC
invariants and balanced control/candidate measurements with declared metrics
and thresholds; record rejected experiments compactly. When a touched
production file exceeds the guide's complexity trigger or mixes semantic,
presentation, persistence, and transport ownership, require either a bounded
split or a concrete cohesion decision. These are review gates, not automatic
authorization for cleanup outside the packet.

Use `references/design-reviewer.md` for reserved decisions or risky patches;
require independent review for reserved architecture, DICE/ownership/identity,
retained representation, public wire/schema changes, lifecycle risk, `REPLAN`,
and milestone close. Exact-oracle changes within an existing formatter/CLI
boundary may use root review. Under token pressure, batch one independent
review across two to four such slices. Allow one focused correction after a
concrete miss; a second material implementation/contract correction is
`REPLAN`.

Every oracle packet reuses deterministic scaffolding and removes
nondiscriminating copied assets, mutations, manifests, fields, and assertions.
Repository/materialization tests compare the current writer helper or manifest
rather than hard-coded marker formats.

For oracle growth, review fixtures before the sixth accepted packet or at
+100 files/+10,000 text lines since the last checkpoint. Preserve provenance,
hermeticity, isolation, and exact outputs while pruning only material proven
redundant or nondiscriminating.

## Validate and close

- Docs/instructions: source, structure, and diff checks.
- Oracle-only: focused harness plus changed/protected fixtures.
- Private/local Rust: focused owner tests plus one direct compile dependent.
- Public/cross-crate Rust: focused owner tests plus named direct dependents.
- DICE/daemon/platform: relevant lifecycle and cross-target gates.

Run broad repository suites once at a milestone/integration checkpoint, not
after each packet or correction. Reviewers inspect recorded output and rerun
only missing, stale, or suspect evidence. Use quiet validation where supported;
review handoffs include command, exit status, test count, and relevant failure
output, not passing logs. Multiple fresh-root Bazel replays are required only
for repository, materialization, or non-hermetic behavior. Run Rust formatting
when Rust changes and always run `git diff --check`.

At terminal `ACCEPT`, `REPLAN`, or genuine stop:

1. update the owner plan only when milestone/gate state, a blocker, a reusable
   architecture decision, or `REPLAN` changes;
2. update the manifest and canonical Live Status only for a scheduling change;
3. add a routing-log row only for `REPLAN`, an unusual route/parallel layout,
   a model-route change, or a reusable routing lesson; and
4. commit accepted work, folding status into it when practical.

Git history of the manifest and implementation commit records ordinary
`ACCEPT` packets. Do not publish per-audit/correction status commits or
duplicate ordinary acceptance evidence in routing history.
