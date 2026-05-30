export const meta = {
  name: 'plan61-burn-down',
  description: 'Plan 61: survey ALL live bzlmod bridge surfaces (items 2,3,5,6,7), rank a slice queue, auto-implement only low-risk/small slices via plan61-implement-slice, queue the rest for human approval',
  whenToUse: 'Run to make autonomous forward progress on Plan 61. Auto-implements only est_risk=low AND est_size=S non-conflicting slices (each still gated by adversarial review inside plan61-implement-slice); everything med/high or conflicting is returned as an approval queue. Honors the plan 2-consecutive-no-shrink halt rule and stops conservatively on any build/test/review failure.',
  phases: [
    { title: 'Survey', detail: 'one read-only Explore agent per remaining-work item (2,3,5,6,7) locates live production bridge surfaces' },
    { title: 'Rank', detail: 'de-conflict + synthesize ranked, ready-to-implement slice specs; partition auto vs approval-queue' },
    { title: 'Auto-implement', detail: 'sequentially run each auto slice through plan61-implement-slice (shared cargo build lock); stop on failure/blocker/no-shrink' },
    { title: 'Report', detail: 'emit auto results + approval queue + halt reason' },
  ],
}

const PLAN = '/var/mnt/dev/slug/thoughts/shared/plans/slug-bazel-subplans/61-true-dice-bzlmod.md'

// Cap autonomous slices when no token budget is set, so the loop can't run away
// rebuilding slug for every ranked slice. Overflow goes to the approval queue.
const MAX_AUTO = 0
// Each plan61-implement-slice run does a cargo build + pytest guardrail; budget
// it generously so we stop before starving the run mid-slice.
const PER_SLICE_BUDGET = 180_000

// Remaining-work items with a still-live production bridge surface. Item 1 (legacy
// resolution bridge) and item 4 (extension .bzl digest) are done per the plan and
// are deliberately excluded. Item 8+ (status/guardrails) is not a code surface.
// NOTE (2026-05-29): items 1, 4, registry content-addressing (sub-plan 01), and the
// out-of-project DICE-READ paths for items 2 & 3 (sub-plan 02 A.1-A.3) are DONE.
// EXCLUDE anything that requires the file watcher / 61-02 follow-ons (the synchronous
// direct read_absolute_text_file_input non-DICE parse-path reads, and Phase B inotify)
// -- those are intentionally held. Find sliceable bridge-shrinks in items 3-resid,5,6,7.
const ITEMS = [
  {
    n: 3,
    title: 'Lockfile replay completeness (residual)',
    focus: 'EXCLUDE the out-of-project hidden lockfile poll (now a watched DICE input). Look for: facts, selected-yanked versions, registry-file-hashes, recorded inputs, and lockfile-mode that are still bundled inside a lockfile value / extension replay instead of explicit DICE keys; any lockfile policy path that still relies on process-global state or an injected non-DICE value. Reject pure decomposition that does not remove a real bridge.',
  },
  {
    n: 5,
    title: 'Extension spoke / generated-repo out of process globals',
    focus: 'Alias compatibility fallback still uses process-global transitional plumbing. Remaining root/external cell-name adapter calls that should pass an explicit resolver/cell-graph owner instead of a process-global root helper. Materialized generated-repo cell-graph ownership and final materialization output state not yet fully DICE-owned.',
  },
  {
    n: 6,
    title: 'Bazel 9 directive semantics',
    focus: 'Remaining dev_dependency surfaces, single_version_override(registry/patches), multiple_version_override(registry), archive_override, git_override, override_repo validation, inject_repo validation, and isolated extension usages that are not yet Bazel-9-grounded with negative tests. Surfaces here often add a directive edge rather than remove a bridge; only count a slice if it removes/replaces a transitional resolver policy surface.',
  },
  {
    n: 7,
    title: 'Repository execution replay-correctness',
    focus: 'Marker-file trust not yet fully replaced by a manifest value owning the full repository output-tree identity. Remaining repository_ctx.watch/watch_tree/env/repo-mapping/label-path/download/source-identity/patch/overlay/generated-file inputs not yet recorded as DICE-owned replay inputs. Non-cacheable local repository classes still trusting markers.',
  },
]

// Survey output: candidate bridge surfaces per item (shape shared with plan61-propose-slice).
const SURVEY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['item', 'candidates'],
  properties: {
    item: { type: 'string', description: 'remaining-work item label, e.g. "3 lockfile replay"' },
    candidates: {
      type: 'array',
      description: 'distinct candidate slices, smallest/most-tractable first',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['title', 'bridge_surface', 'still_live', 'intended_owner_key', 'bazel9_anchor', 'regression_test', 'est_risk', 'est_size'],
        properties: {
          title: { type: 'string' },
          bridge_surface: { type: 'string', description: 'EXACT file:line locations + what the live production bridge does (direct std::fs poll, untracked-input flag, per-transaction invalidation, marker trust, process-global, etc.)' },
          still_live: { type: 'boolean', description: 'true only if reachable in NON-TEST production code (verify with rg / cfg(test) check)' },
          intended_owner_key: { type: 'string', description: 'target DICE key/value from the plan Target Shape' },
          bazel9_anchor: { type: 'string', description: 'specific Bazel 9 source path/symbol or observed 9.0.1 behavior the slice must match' },
          regression_test: { type: 'string', description: 'smallest owning-abstraction regression to add and the file it belongs in' },
          est_risk: { type: 'string', enum: ['low', 'med', 'high'] },
          est_size: { type: 'string', enum: ['S', 'M', 'L'] },
          notes: { type: 'string' },
        },
      },
    },
  },
}

// One ready-to-implement slice spec. The non-meta fields here are a drop-in for
// plan61-implement-slice's args.slice; the extra fields drive ranking/partition.
const SLICE = {
  type: 'object',
  additionalProperties: false,
  required: ['title', 'item', 'rationale', 'bridge_surface', 'intended_owner_key', 'bazel9_anchor', 'regression_test_plan', 'implementation_sketch', 'focused_tests', 'same_daemon_proof', 'risks', 'est_risk', 'est_size', 'conflicts_with'],
  properties: {
    title: { type: 'string' },
    item: { type: 'string' },
    rationale: { type: 'string', description: 'why this is a smallest viable slice that actually SHRINKS a live production bridge surface (not just hardens around it)' },
    bridge_surface: { type: 'string', description: 'exact file:line surface being removed / made test-only / made unreachable' },
    intended_owner_key: { type: 'string' },
    bazel9_anchor: { type: 'string' },
    regression_test_plan: {
      type: 'object',
      additionalProperties: false,
      required: ['file', 'test_name', 'asserts', 'expected_initial_failure'],
      properties: {
        file: { type: 'string' },
        test_name: { type: 'string' },
        asserts: { type: 'string' },
        expected_initial_failure: { type: 'string', description: 'the failure message/marker the test should produce BEFORE the fix' },
      },
    },
    implementation_sketch: { type: 'string', description: 'concrete edits: which key/value gains which dependency edge; what becomes cfg(test)' },
    focused_tests: { type: 'array', items: { type: 'string' }, description: 'cargo/pytest selectors to run in verify' },
    same_daemon_proof: { type: 'string', description: 'counter/log or pytest scenario that proves same-daemon invalidation/replay' },
    risks: { type: 'string' },
    est_risk: { type: 'string', enum: ['low', 'med', 'high'] },
    est_size: { type: 'string', enum: ['S', 'M', 'L'] },
    conflicts_with: { type: 'array', items: { type: 'string' }, description: 'titles of other queued slices that touch the same files/keys and must not run concurrently or before this one' },
  },
}

const RANK_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['queue'],
  properties: {
    queue: { type: 'array', description: 'ranked best-first; smallest/lowest-risk genuine bridge-shrinking slices first', items: SLICE },
  },
}

// ---- Survey: one read-only agent per item ----
phase('Survey')
const surveys = await parallel(
  ITEMS.map((it) => () =>
    agent(
      `You are surveying Plan 61 (True DICE-Owned Bzlmod) remaining-work item ${it.n}: ${it.title}.

Read the plan first: ${PLAN} (the "Remaining Work", "Target DICE/Skyframe Shape", "Bridge Burn-Down Operating Rule", and "Strong Guardrails" sections especially). Then locate the LIVE production bridge surface for this item in the codebase.

Focus area: ${it.focus}

Search the crates named in the plan: app/slug_common, app/slug_bzlmod, app/slug_core, app/slug_external_cells, app/slug_analysis, app/slug_interpreter_for_build. Use rg/grep + Read. For each candidate slice you find:
- Pin EXACT file:line of the bridge surface.
- Determine still_live: is it reachable in NON-TEST production code? A surface guarded by cfg(test) / a test-only helper is NOT live — say so. Verify with rg for cfg(test) and call-site analysis.
- Name the intended owner DICE key from the plan's Target Shape.
- Cite a specific Bazel 9 source path/symbol or observed 9.0.1 behavior the slice must match.
- Propose the SMALLEST owning-abstraction regression test and the file it belongs in.
- Estimate risk (low/med/high) and size (S/M/L) HONESTLY — only mark low+S when the edge is local, the owner key already exists, and the blast radius is one or two files.

Per the Bridge Burn-Down Operating Rule: a valid slice REDUCES a production bridge surface (removes it, makes it test-only, replaces with a named DICE key, or makes it unreachable). Reject pure hardening. Order candidates smallest/most-tractable first. Do NOT edit any files.`,
      { label: `survey:item${it.n}`, phase: 'Survey', schema: SURVEY_SCHEMA, agentType: 'Explore' }
    )
  )
)

const live = surveys
  .filter(Boolean)
  .flatMap((s) => s.candidates.map((c) => ({ ...c, item: s.item })))
  .filter((c) => c.still_live)

log(`Survey: ${live.length} live candidate slice(s) across items ${ITEMS.map((i) => i.n).join(', ')}`)

if (!live.length) {
  return {
    status: 'no-live-surfaces',
    note: 'No still-live production bridge surfaces found across items 2,3,5,6,7. Re-plan from ## Remaining Work or re-confirm with a manual rg sweep.',
  }
}

// ---- Rank: de-conflict + produce ready-to-implement specs ----
phase('Rank')
const ranked = await agent(
  `You are building the Plan 61 burn-down queue. Plan: ${PLAN}.

Here are the LIVE candidate bridge surfaces found by the survey (JSON):

${JSON.stringify(live, null, 2)}

Produce a ranked, ready-to-implement slice queue. Rules:
- Each slice must actually SHRINK a live production bridge surface (remove it, make it cfg(test)-only, replace it with the named DICE key, or make it unreachable) — not harden around it. Drop pure-hardening candidates.
- Read the relevant code yourself to confirm each surface is real and still reachable in NON-TEST production code before including it.
- Set est_risk and est_size HONESTLY. low+S means: owner key already exists, edge is local, blast radius 1-2 files, and a clean owning-abstraction regression can pin it. When unsure, round UP.
- conflicts_with: list titles of other queued slices that touch the same files/keys. Two slices that edit the same key/file MUST list each other.
- regression_test_plan must describe an owning-abstraction test that FAILS before the fix with a concrete marker.
- Order best-first: genuine, low-risk, small, clean-owner, citable-Bazel-9-anchor slices first.

Emit up to 10 slices. Do NOT edit files.`,
  { label: 'rank', phase: 'Rank', schema: RANK_SCHEMA }
)

const queue = (ranked && ranked.queue) || []

// The ranker can legitimately reject EVERY live candidate as non-sliceable (pure
// hardening, blocked on a missing foundational key, or feature work). When that
// happens the survey is still the valuable output — surface it instead of a black
// hole, and flag that the next move is foundational, not a slice.
if (!queue.length) {
  const byItem = {}
  for (const c of live) (byItem[c.item] ||= []).push(c)
  return {
    status: 'needs-foundation',
    note: 'Ranker rejected all live candidates as non-sliceable (pure hardening, blocked on a missing foundational DICE key, or feature work). No auto-implement performed; tree untouched. Next progress requires foundational keys, not bridge-burn-down slices — re-plan from ## Remaining Work.',
    live_candidate_count: live.length,
    live_candidates_by_item: byItem,
  }
}

// Auto-eligible = low risk AND small AND no declared conflict. Everything else is
// for human approval. This is the "hybrid: auto low-risk only" policy.
const autoEligible = queue.filter(
  (s) => s.est_risk === 'low' && s.est_size === 'S' && (!s.conflicts_with || s.conflicts_with.length === 0)
)
const auto = autoEligible.slice(0, MAX_AUTO)
const approvalQueue = queue.filter((s) => !auto.includes(s))

log(`Ranked ${queue.length} slice(s): ${auto.length} auto-eligible (running), ${approvalQueue.length} queued for approval`)

// ---- Auto-implement: sequential, conservative stop + 2-no-shrink halt ----
phase('Auto-implement')
const autoResults = []
let haltReason = null
let consecutiveNoShrink = 0

for (let i = 0; i < auto.length; i++) {
  const slice = auto[i]

  if (budget.total && budget.remaining() < PER_SLICE_BUDGET) {
    haltReason = `Budget guard: ${Math.round(budget.remaining() / 1000)}k remaining < ${PER_SLICE_BUDGET / 1000}k needed per slice. Remaining auto slices moved to approval queue.`
    approvalQueue.push(...auto.slice(i))
    break
  }

  log(`Auto-implement ${i + 1}/${auto.length}: ${slice.title}`)
  const res = await workflow('plan61-implement-slice', { slice })
  autoResults.push({ title: slice.title, item: slice.item, result: res })

  // Conservative stop: anything other than a clean slice-complete means the tree
  // may be dirty/red or review wants human eyes. Stop mutating, escalate the rest.
  if (!res || res.status !== 'slice-complete') {
    haltReason = `Slice "${slice.title}" returned status="${res && res.status}" (not slice-complete). Stopping auto loop; remaining auto slices moved to approval queue for human review.`
    approvalQueue.push(...auto.slice(i + 1))
    break
  }

  // Forcing function: a verified (tests-passing) slice that did not actually shrink
  // a bridge surface — i.e. the bridge-burn-down review still flagged a blocker.
  const shrank = !(res.review_blockers && res.review_blockers.length)
  if (shrank) {
    consecutiveNoShrink = 0
  } else {
    consecutiveNoShrink++
    if (consecutiveNoShrink >= 2) {
      haltReason = `Plan halt rule: two consecutive verified slices did not shrink a bridge surface. Stop normal slicing and re-plan from ## Remaining Work. Remaining auto slices moved to approval queue.`
      approvalQueue.push(...auto.slice(i + 1))
      break
    }
  }
}

// ---- Report ----
phase('Report')
const completed = autoResults.filter((r) => r.result && r.result.status === 'slice-complete')
return {
  status: haltReason ? 'halted' : completed.length ? 'auto-progress' : 'queued-only',
  halt_reason: haltReason,
  auto_completed: completed.map((r) => ({
    title: r.title,
    item: r.item,
    bridge_surface_disposition: r.result.bridge_surface_disposition,
    files_changed: r.result.files_changed,
    proposed_plan_checkpoint: r.result.proposed_plan_checkpoint,
  })),
  auto_results: autoResults,
  approval_queue: approvalQueue.map((s) => ({
    title: s.title,
    item: s.item,
    est_risk: s.est_risk,
    est_size: s.est_size,
    conflicts_with: s.conflicts_with,
    bridge_surface: s.bridge_surface,
    intended_owner_key: s.intended_owner_key,
    rationale: s.rationale,
    spec: s,
  })),
  live_candidate_count: live.length,
  note: 'Approve any approval_queue spec by running plan61-implement-slice with args.slice = that spec. auto_completed checkpoints still need a human to paste them into the plan and rerun the full guardrail suite.',
}
