---
name: slug-agent-orchestration
description: Run Slug V2 implementation-plan or roadmap goals through bounded, reviewed work packets. Use for `/goal follow the implementation plan`, next-packet selection, delegation, model routing, integration, or routing records.
---

# Slug Agent Orchestration

The root owns priority, architecture, worktree safety, integration, status, and
commits.

## Start

1. Read `AGENTS.md`, canonical **Live Status**, and **Current packet**.
2. Search the owner plan for that packet ID. Read only its active section and
   matching Stage 9 rows; histories are not routine context.
3. Check `git status --short --branch` and overlapping dirty diffs.
4. Continue one clearly owned packet. Check the oracle-harness growth
   checkpoint before adding fixture breadth.
5. Inspect agents/Cargo/`slugd` only before overlapping work, retries, or
   daemon-sensitive validation.

Use the routing table below. Read `references/routing-guide.md` only when the
route is unclear and `references/routing-log.md` only when a recent analogous
packet may change it.

## Route

| Work | Route |
|------|-------|
| Small read-only/mechanical | Root |
| Audit, oracle, focused tests, one abstraction | Terra medium |
| Approved difficult Rust/DICE/Starlark/query | Terra high |
| Reserved decision or risky final review | Sol low |
| Concrete unresolved architecture miss | Sol high |

Ordinary packets use at most one write worker and one reviewer. Add read-only
audits only for distinct unresolved semantic questions; parallel writers must
own disjoint files. Correction rereviews inspect only the correction diff and
prior blocker. Never run parallel Cargo commands on one target directory.

## Packet

Use `references/implementation-worker.md`. Always name:

- one owner and observable result;
- exact allowed files and exclusions;
- accepted Bazel 9.2 oracle or pinned-source regression;
- focused validation and stop conditions; and
- residual risk.

Add conditional sections only when used: fixture growth/hygiene for oracle
work; DICE identity/equality for semantic keys; Stage 9/Buck2/V1 reuse for
representation changes; downstream coverage for public interfaces; platform
and lifecycle evidence for daemon/platform work.

Reuse accepted discriminating evidence. Add an oracle only for an evidence
gap. Use a separate design packet only for decisions reserved to the root:
new DICE keys/locks, public or cross-crate APIs, identity/ownership models,
formatter semantics, regex engines, stage boundaries, and destructive actions.
Obtain one Sol pre-review for those decisions. Otherwise keep design,
implementation, and evidence in one logical packet and one terminal rollup.

Workers edit named files and run focused tests. The root inspects the diff,
adds a discriminating case for identity/equality/invalidation/order/formatting
when needed, and owns broader validation and commits.

## Accept

Check only applicable risks:

- Bazel success/failure, diagnostics, order, and output;
- structural identity, ownership, equality, and invalidation;
- create/edit/delete/recreate for incremental state;
- DICE-owned discovery without direct filesystem/fresh-graph bypass;
- named-surface-only activation and compact retained representation; and
- downstream behavior for changed interfaces.

Use `references/design-reviewer.md` for reserved decisions or risky patches;
ordinary packets receive one implementation review. Allow one focused
correction after a concrete miss. A second material correction is `REPLAN`.

For oracle growth, review fixtures before the sixth accepted packet or at
+100 files/+10,000 text lines since the last checkpoint. Preserve provenance,
hermeticity, isolation, and exact outputs while pruning only material proven
redundant or nondiscriminating.

## Validate and close

- Docs/instructions: source, structure, and diff checks.
- Oracle-only: focused harness plus changed/protected fixtures.
- Private Rust: owner tests and direct dependents.
- Public/DICE/daemon/platform changes: affected downstream, daemon, and
  cross-target gates.

Run broad suites only when the interface or bug class requires them. Run Rust
formatting when Rust changes and always run `git diff --check`.

At terminal `ACCEPT`, `REPLAN`, or genuine stop:

1. update the owner plan once with compact evidence;
2. update canonical Live Status only for a scheduling change;
3. add one bounded routing-log row, rotating history only when needed; and
4. commit accepted work, folding status into it when practical.

Do not publish per-audit/correction status commits or duplicate live routing
rows into history.
