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
   daemon-sensitive validation. Run `python3 scripts/v2_plan_status.py` after
   scheduling edits; the root reconciles any mismatch before delegation.

Read `thoughts/shared/plans/slug-v2-plan-authoring-guide.md` before creating or
materially revising a packet. Read
`thoughts/shared/plans/slug-v2-subplans/zabel-adoption-roadmap.md` only when the
current packet selects one of its workstreams; it never overrides Live Status.

Read `references/routing-log.md` only when a recent analogous packet may change
an unclear route. Read `references/parity-source-anchors.md` only when the
packet touches one of its listed surfaces.

## Route

The root may complete a cohesive packet directly. Delegate only a concrete
independent implementation or review task; give concurrent writers disjoint
files/worktrees. Use the user's selected model and delegation preferences;
there is no mandatory model-name or effort downgrade. Under token pressure,
default to root-only work and reuse accepted evidence.

Obtain independent review for a new shared public boundary, semantic
identity/ownership or DICE locking change, lifecycle risk, and milestone close.
Use a design checkpoint within the implementation packet when the outcome and
ownership are bounded; split out a design packet only when an unresolved choice
prevents defining an implementable contract. Review consequential decisions
before implementing them, then review the final diff. Label reviews `design` or
`final`; reuse the reviewer for the final delta where practical. Routine invocation,
test-selector, formatting, and compiler corrections do not need a new reviewer.
For correction rereviews, inspect the correction and affected evidence only.

When delegating, pass the packet path, baseline, validation summary, and
specific question with `fork_turns="none"` unless conversation is essential.
Never run concurrent Cargo commands sharing a target directory.

## Packet

The manifest is the complete contract for a root-only packet. The author uses
[the plan-authoring guide](../../../thoughts/shared/plans/slug-v2-plan-authoring-guide.md#required-packet-record)
as the single readiness checklist, recording applicable invariants and linking
inherited contracts. When delegating, use `references/implementation-worker.md`
with the manifest and role-specific evidence; workers need not reread the guide
unless they are authorized to revise the contract.

Reuse accepted discriminating evidence; add an oracle only for an evidence gap.
Keep design, evidence, implementation and tests together when they cover one
abstraction and behavior family under a bounded allowlist. A new boundary or
DICE/identity decision requires review, not automatically another packet.
Canonical reserved decisions still require resolution before dependent work.

Workers edit named files and run focused tests. The root inspects the diff,
adds a discriminating case for identity/equality/invalidation/order/formatting
when needed, and owns broader validation and commits.

## Accept

Review the packet's material invariants and inherited contracts against the
actual diff and discriminating evidence. Check for applicable risks omitted by
the packet using the guide's required record, complexity and performance gates;
read only those sections when needed. This does not authorize adjacent cleanup.

Use `references/design-reviewer.md` when independent review is required.
Review the actual change and discriminating evidence for each material invariant;
line counts, hashes, and a passing test count do not establish semantics.
A second correction calls for reassessment, not automatic `REPLAN`. Keep
correcting within the accepted contract when the design remains valid.

## Recovery and candidate ownership

User instructions and session authorization take precedence over packet-local
stops. Preserve explicit resource and permission limits. The historical
12-second test deadline/15-second absolute ceiling remains in force unless
the user changes it; compile separately, with preparation bounded to 60 seconds.
A packet must distinguish inherited user limits from its own estimates.

- **Invocation/environment failure:** launcher failure, zero selected tests,
  or an attributed environment problem leaves a gate unproved. Correct the
  invocation within existing permissions and run only the missing gate.
- **Implementation failure:** diagnose and correct within the same owner,
  scope, compatibility class, and acceptance criteria; invalidate affected
  evidence. Do not repeat an identical failure without a new hypothesis.
- **Design/prerequisite failure:** use `REPLAN` only when evidence contradicts
  the contract or completion requires a new ownership/compatibility decision,
  unavailable prerequisite, or authorization. Name the precise decision.
  REPLAN is not itself a user-permission request; the root may resolve
  decisions already covered by the user's task and existing authorization.
- **Resource limit:** stop that operation and preserve its evidence. Split
  preparation from runtime or select a smaller discriminating check; do not
  silently raise a user ceiling or treat timeout as a semantic test failure.

Keep unfinished substantial candidates on a local review branch/worktree with
base revision and validation receipt. Do not restore and reconstruct code for
routine validation failures. A preservation commit is not acceptance and must
not be merged as such. Reconcile landed prerequisites before continuing;
record evidence against the source, toolchain/features and environment tested.
Preserve passing results unless relevant inputs change. Keep coupled semantic
activation atomic at integration; no partial landing to bypass an unmet gate.

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

Before Rust validation, resolve the pinned toolchain from `rust-toolchain` using
`rustup which --toolchain <channel> cargo` (and rustc/rustdoc/rustfmt) rather
than assuming the PATH launcher works. Compile with `--no-run` and Cargo JSON
output when selecting Rust test executables. Batch all selected ordinary tests
for each executable into one preflight:
`python3 scripts/v2_test_preflight.py <executable> --exact <name1> <name2> ...`.
Reuse that selection evidence while the executable is unchanged; rebuilding or
replacing it invalidates the preflight, and added selectors need coverage before
execution. Preflight does not replace execution/pass counts; zero tests never
prove a gate. The helper rejects ignored tests; an explicitly selected ignored
probe needs the packet's supervised invocation and ignored-test selection check.
For daemon tests, check the required Unix-socket capability in the actual execution environment and use the normal
approval mechanism if a sandbox exception is necessary.

Run broad suites once at milestone/integration checkpoints. Partition suites
into named tests or bounded groups when needed to preserve user deadlines;
all required tests must still be accounted for. Reuse baseline failure records
only for their recorded base/environment and do not waive unattributed failures.
Reviewers rerun only missing, stale, or suspect evidence. Handoffs contain
command, exit status, selected/pass count, elapsed time, and failure output.
Use fresh-root Bazel replays only for state/non-hermetic behavior. Format changed
Rust and always run `git diff --check`.

At acceptance or a genuine blocker, update the owner contract only for changed
architecture/gates and update manifest/canonical status only for scheduling.
Run the plan status checker. Commit accepted work with its compact evidence;
do not publish a new status commit per audit or invocation correction.
Routing logs record only reusable routing lessons or unusual delegation.

Record packet elapsed time, compile/test/review time where available, replan
reason, and observable gate advanced in the acceptance receipt or commit.
Do not claim workflow speedups without measurements, or impose a separate
metrics/reporting packet on routine work.
