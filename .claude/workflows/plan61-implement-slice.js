export const meta = {
  name: 'plan61-implement-slice',
  description: 'Plan 61: implement ONE approved slice TDD-style, verify focused tests, adversarially review the bridge burn-down, emit a plan checkpoint',
  whenToUse: 'Run after a human approves a slice spec produced by plan61-propose-slice. Pass the approved spec as args.',
  phases: [
    { title: 'Regression', detail: 'add the owning-abstraction test; confirm it FAILS as expected' },
    { title: 'Implement', detail: 'single-track: implement the DICE-owned edge, make the test pass, cargo build -p slug' },
    { title: 'Verify', detail: 'run focused test packages sequentially (shared cargo build lock)' },
    { title: 'Review', detail: 'adversarial bridge-burn-down + guardrail review of the diff' },
    { title: 'Checkpoint', detail: 'emit a compact plan-style checkpoint note' },
  ],
}

const PLAN = '/var/mnt/dev/slug/thoughts/shared/plans/slug-bazel-subplans/61-true-dice-bzlmod.md'

const slice = args && args.slice
if (!slice) {
  throw new Error('plan61-implement-slice requires args.slice (an approved slice spec from plan61-propose-slice)')
}
const SLICE = JSON.stringify(slice, null, 2)

const TEST_RESULT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['fails_as_expected', 'test_file', 'observed_output', 'command'],
  properties: {
    fails_as_expected: { type: 'boolean' },
    test_file: { type: 'string' },
    command: { type: 'string', description: 'exact command run to exercise the new test' },
    observed_output: { type: 'string', description: 'the actual failure marker/message observed (truncated)' },
  },
}

const IMPL_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['build_ok', 'target_test_passes', 'files_changed', 'bridge_surface_disposition', 'diff_summary'],
  properties: {
    build_ok: { type: 'boolean', description: 'cargo build -p slug succeeded' },
    target_test_passes: { type: 'boolean', description: 'the regression test now passes' },
    files_changed: { type: 'array', items: { type: 'string' } },
    bridge_surface_disposition: { type: 'string', description: 'EXACTLY what happened to the live bridge surface: removed / made cfg(test) / replaced by <key> / made unreachable. Cite file:line.' },
    diff_summary: { type: 'string' },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['all_passed', 'results'],
  properties: {
    all_passed: { type: 'boolean' },
    results: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['command', 'passed', 'summary'],
        properties: {
          command: { type: 'string' },
          passed: { type: 'boolean' },
          summary: { type: 'string', description: 'e.g. "405 passed plus doctest" or first failure' },
        },
      },
    },
  },
}

const REVIEW_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['lens', 'verdict', 'findings'],
  properties: {
    lens: { type: 'string' },
    verdict: { type: 'string', enum: ['pass', 'concerns', 'block'] },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['severity', 'detail'],
        properties: {
          severity: { type: 'string', enum: ['blocker', 'major', 'minor'] },
          detail: { type: 'string', description: 'path:line: problem. fix.' },
        },
      },
    },
  },
}

// ---- Regression: write the failing test first ----
phase('Regression')
const reg = await agent(
  `Plan 61 TDD step 1 of the approved slice. Plan: ${PLAN}.

Approved slice spec (JSON):
${SLICE}

Add ONLY the regression test described in regression_test_plan — do not implement the fix yet. Place it in the specified file (or the closest correct location; report where). Then run it and confirm it FAILS with the expected_initial_failure marker. If it passes immediately, the test does not yet pin the bridge bug — strengthen it until it fails for the right reason. Report the exact command and observed failure output. Make no other production changes.`,
  { label: 'write-failing-test', phase: 'Regression', schema: TEST_RESULT_SCHEMA }
)

if (!reg || !reg.fails_as_expected) {
  return {
    status: 'halted',
    where: 'Regression',
    reason: 'New regression test did not fail as expected; cannot proceed TDD without a red test.',
    regression: reg,
  }
}
log(`Red test in place: ${reg.test_file}`)

// ---- Implement: single-track ----
phase('Implement')
const impl = await agent(
  `Plan 61 TDD step 2. Plan: ${PLAN}. Follow the project CLAUDE.md error-handling conventions (slug_error, ErrorTag, no anyhow).

Approved slice spec (JSON):
${SLICE}

The red regression test is in ${reg.test_file} (command: ${reg.command}). Implement implementation_sketch: give the intended_owner_key the real DICE dependency edge and REDUCE the live bridge surface named in bridge_surface — remove it, make it cfg(test)-only, replace it with the named DICE key, or make it unreachable in non-test production code. Then:
1. Make the regression test pass.
2. Run cargo build -p slug.
3. Confirm git diff --check is clean.
Report exactly what happened to the bridge surface (cite file:line), files changed, and a diff summary. Single-track work: you are the only writer.`,
  { label: 'implement', phase: 'Implement', schema: IMPL_SCHEMA }
)

if (!impl || !impl.build_ok || !impl.target_test_passes) {
  return {
    status: 'halted',
    where: 'Implement',
    reason: 'Build failed or regression test still failing after implementation.',
    regression: reg,
    implementation: impl,
  }
}
log(`Implemented; bridge disposition: ${impl.bridge_surface_disposition}`)

// ---- Verify: sequential (one agent; cargo build lock is shared, pytest needs a built slug) ----
phase('Verify')
const focused = (slice.focused_tests && slice.focused_tests.length
  ? slice.focused_tests
  : [
      'cargo test -p slug_bzlmod',
      'cargo test -p slug_common bzlmod',
      'cargo test -p slug_external_cells',
      'TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short',
    ])
const verify = await agent(
  `Plan 61 verify step. Run these focused tests IN ORDER (they share the cargo build lock; the pytest run needs target/debug/slug built first — run cargo build -p slug before pytest if not already built). Report each command's pass/fail and a one-line summary (counts or first failure). Do not edit code.

${focused.map((c, i) => `${i + 1}. ${c}`).join('\n')}`,
  { label: 'verify', phase: 'Verify', schema: VERIFY_SCHEMA }
)

// ---- Review: parallel, read-only, adversarial ----
phase('Review')
const REVIEW_LENSES = [
  {
    key: 'bridge-burn-down',
    prompt: `Adversarially audit whether this slice actually SHRANK a live production bridge surface per Plan 61's Bridge Burn-Down Operating Rule (${PLAN}). Read the git diff (git diff, and git diff --stat). The claimed disposition: "${impl.bridge_surface_disposition}". Verify with rg that the named surface is now gone / cfg(test)-only / unreachable in non-test production code. Default to "block" if it is mere hardening, if the surface is still reachable in production, or if a new untracked-input/process-global/marker-trust bridge was introduced.`,
  },
  {
    key: 'guardrails-parity',
    prompt: `Adversarially audit this slice against Plan 61's Strong Guardrails and Non-Negotiables (${PLAN}). Read the git diff. Check: (a) no ordinary build/query/audit path now writes a visible lockfile; (b) cache hits remain auditable from key inputs; (c) the Bazel 9 anchor "${slice.bazel9_anchor}" is actually matched by the change (read the cited Bazel behavior if needed); (d) the new test is a genuine negative/owning-abstraction test for its replay-input class. Default to "block" on any guardrail regression.`,
  },
]
const reviews = await parallel(
  REVIEW_LENSES.map((l) => () =>
    agent(l.prompt, { label: `review:${l.key}`, phase: 'Review', schema: REVIEW_SCHEMA })
  )
)

const blockers = reviews
  .filter(Boolean)
  .flatMap((r) => r.findings.filter((f) => f.severity === 'blocker').map((f) => ({ lens: r.lens, ...f })))

// ---- Checkpoint: compact plan-style note ----
phase('Checkpoint')
const checkpoint = await agent(
  `Write a compact Plan 61 "Current Checkpoint" bullet for this completed slice, matching the terse dated style already in ${PLAN} (e.g. "- 2026-XX-XX <slice name>: <what moved to DICE ownership / what bridge surface is now gone-or-test-only>. Guardrails: <commands + pass counts>."). Do NOT invent a date — use the placeholder <DATE>. Base it strictly on:

Slice: ${SLICE}
Bridge disposition: ${impl.bridge_surface_disposition}
Files changed: ${JSON.stringify(impl.files_changed)}
Regression: ${reg.test_file} (was red: ${reg.observed_output})
Verify results: ${JSON.stringify(verify && verify.results)}

Output only the bullet text.`,
  { label: 'checkpoint', phase: 'Checkpoint' }
)

return {
  status: blockers.length ? 'needs-attention' : verify && verify.all_passed ? 'slice-complete' : 'tests-failing',
  slice_title: slice.title,
  bridge_surface_disposition: impl.bridge_surface_disposition,
  files_changed: impl.files_changed,
  regression: { file: reg.test_file, was_red: reg.observed_output },
  verify: verify && verify.results,
  all_tests_passed: verify && verify.all_passed,
  review_blockers: blockers,
  proposed_plan_checkpoint: checkpoint,
}
