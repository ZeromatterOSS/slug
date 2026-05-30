export const meta = {
  name: 'plan61-propose-slice',
  description: 'Plan 61: survey live bzlmod bridge surfaces (items 2,3) and propose ONE ready-to-implement next slice',
  whenToUse: 'Run before implementing a Plan 61 slice. Returns a single recommended slice spec for human approval; makes no edits.',
  phases: [
    { title: 'Survey', detail: 'one read-only agent per remaining-work item locates live production bridge surfaces' },
    { title: 'Synthesize', detail: 'pick the smallest viable next slice and emit a full spec' },
  ],
}

const PLAN = '/var/mnt/dev/slug/thoughts/shared/plans/slug-bazel-subplans/61-true-dice-bzlmod.md'

// Which remaining-work items to survey (user-selected: 2 and 3). Override via args.items.
const ITEMS = (args && args.items) || [
  {
    n: 3,
    title: 'Lockfile replay completeness',
    focus: 'Out-of-project hidden/output-base lockfile content is still directly POLLED by TrackedLockfileContentKey and invalidated per-transaction until a watched-input filesystem key replaces direct polling. Also: any remaining direct lockfile reads on production paths, and selected-yanked / facts / registry-file-hash / recorded-input / lockfile-mode dependencies not yet modeled as explicit DICE edges.',
  },
  {
    n: 2,
    title: 'Module-file DICE inputs',
    focus: 'Out-of-project local-override directory presence is a DICE child metadata key that stays transaction-invalid / marked untracked until absolute paths can use a watched filesystem key. Also: out-of-root registry-cache paths observed inside RegistryFileInputsKey forcing same-key recompute via has_untracked_inputs, and any remaining direct std::fs validity hacks for module source classes.',
  },
]

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
          still_live: { type: 'boolean', description: 'true only if this surface is still reachable in NON-TEST production code (verify with rg / cfg(test) check)' },
          intended_owner_key: { type: 'string', description: 'target DICE key/value that should own this from the plan Target Shape (e.g. LockfileContentKey, ModuleSourceKey)' },
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

const SLICE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['title', 'item', 'rationale', 'bridge_surface', 'intended_owner_key', 'bazel9_anchor', 'regression_test_plan', 'implementation_sketch', 'focused_tests', 'same_daemon_proof', 'risks'],
  properties: {
    title: { type: 'string' },
    item: { type: 'string' },
    rationale: { type: 'string', description: 'why this is the smallest viable next slice that actually SHRINKS a live production bridge surface (not just hardens around it)' },
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
  },
}

phase('Survey')
const surveys = await parallel(
  ITEMS.map((it) => () =>
    agent(
      `You are surveying Plan 61 (True DICE-Owned Bzlmod) remaining-work item ${it.n}: ${it.title}.

Read the plan first: ${PLAN} (the "Remaining Work", "Target DICE/Skyframe Shape", and "Bridge Burn-Down Operating Rule" sections especially). Then locate the LIVE production bridge surface for this item in the codebase.

Focus area: ${it.focus}

Search the crates named in the plan: app/slug_common, app/slug_bzlmod, app/slug_core, app/slug_external_cells, app/slug_analysis, app/slug_interpreter_for_build. Use rg/grep + Read. For each candidate slice you find:
- Pin EXACT file:line of the bridge surface.
- Determine still_live: is it reachable in NON-TEST production code? A surface guarded by cfg(test) / a test-only helper is NOT live — say so. Verify with rg for cfg(test) and call-site analysis.
- Name the intended owner DICE key from the plan's Target Shape.
- Cite a specific Bazel 9 source path/symbol or observed 9.0.1 behavior the slice must match.
- Propose the SMALLEST owning-abstraction regression test and the file it belongs in.
- Estimate risk and size.

Per the Bridge Burn-Down Operating Rule: a valid slice REDUCES a production bridge surface (removes it, makes it test-only, replaces with a named DICE key, or makes it unreachable). Reject pure hardening. Order candidates smallest/most-tractable first. Do NOT edit any files.`,
      { label: `survey:item${it.n}`, phase: 'Survey', schema: SURVEY_SCHEMA, agentType: 'Explore' }
    )
  )
)

const live = surveys
  .filter(Boolean)
  .flatMap((s) => s.candidates.map((c) => ({ ...c, item: s.item })))
  .filter((c) => c.still_live)

log(`Survey found ${live.length} live candidate slice(s) across items ${ITEMS.map((i) => i.n).join(', ')}`)

phase('Synthesize')
const chosen = await agent(
  `You are choosing the next Plan 61 slice to implement. Plan: ${PLAN}.

Here are the LIVE candidate bridge surfaces found by the survey (JSON):

${JSON.stringify(live, null, 2)}

Pick the ONE smallest viable next slice per the plan's Validation Workflow and Bridge Burn-Down Operating Rule. Requirements:
- It must actually SHRINK a live production bridge surface, not harden around it.
- Prefer low-risk / small-size with a clean owning DICE key and a citable Bazel 9 anchor.
- The regression test must be an owning-abstraction test that FAILS before the fix with a concrete marker.

Read the relevant code yourself to confirm the surface is real and still_live before committing to it. Emit a complete, ready-to-implement slice spec. Do NOT edit files.`,
  { label: 'synthesize', phase: 'Synthesize', schema: SLICE_SCHEMA }
)

return { chosen, live_candidate_count: live.length, all_live_candidates: live }
