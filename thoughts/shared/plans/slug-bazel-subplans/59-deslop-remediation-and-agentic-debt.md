# De-slop Remediation and Agentic Debt Burn-Down

> **Main Plan**: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)

## Why this subplan exists

The previous de-slop note identified hotspots, but this plan integrates the work into the existing Bazel-9 parity subplan framework and adds concrete execution tracks.

This plan focuses on code that appears to have accreted during migration and needs systematic cleanup while preserving strict Bazel 9 parity behavior.

## Scope and categorization model

We classify issues by **origin** so cleanup targets the right layer:

1. **Agent-added debt (post-fork migration additions)**
   - Commonly in `app/slug_*` crates and recent migration modules.
   - Includes temporary disables, placeholder services, and duplicated "bridge" logic.
2. **Inherited Buck2 debt (pre-existing upstream patterns)**
   - Commonly in `starlark-rust`, some core infrastructure, and imported tests.
   - Includes legacy TODO/FIXME and panic/expect invariants that predate migration.
3. **Mixed debt (agent touched inherited code)**
   - Existing Buck2 components modified during Bazel migration where temporary behavior was introduced.

### Practical heuristic for triage

- If file introduced/renamed after initial fork commit (`78f4e300`), classify as **agent-added** unless provenance says otherwise.
- If path is upstream/inherited (`starlark-rust`, imported proto/test fixtures) and markers predate migration, classify as **inherited**.
- If inherited file has new local TODO-disable scaffolding, classify as **mixed** and track exact edits.

## Severity-ranked findings and actions

## F1 (SEV-1): explain/what-ran event projection duplication (must fix now)

**Observed**
- `app/slug_server_commands/src/explain_code.rs` still has inline event-parsing + action reconstruction and explicit dedup TODO.
- Parallel logic exists in what-ran paths, creating drift risk in action correlation semantics.

**Origin**: **Mixed**, but migration-local duplication is primarily **agent-added**.

**Why severe**
- Directly impacts user-visible diagnostics and action provenance.
- Divergent implementations can silently disagree on invalidation/file-change interpretation.

**Execution plan (P0)**
1. Extract shared projection library (`slug_event_observer::projection`):
   - span/action assembly,
   - command reproducer attachment,
   - file watcher event extraction.
2. Migrate both `explain` and `what-ran` to consume the shared model.
3. Add parity tests asserting identical projected action sets from same event log.
4. Remove local TODO and forbid duplicate projectors via lint/check.

**Exit criteria**
- No custom projection loop in `explain_code.rs`.
- One shared projector consumed by both call sites.
- Golden parity tests pass.

---

## F2 (SEV-1): action implementation boilerplate + invariant drift (must fix now)

**Observed**
- Repeated "single input/output by construction" invariants, CBP destination logic, and materialization patterns across action impls (`copy`, `symlinked_dir`, etc.).

**Origin**: Mostly **agent-added** in migration-era action implementations.

**Why severe**
- Semantic drift risk in core action execution path (Bazel parity sensitive).
- Boilerplate obscures behavior differences and slows auditability.

**Execution plan (P0/P1)**
1. Introduce shared internal helpers in `slug_action_impl`:
   - `SingleInput`/`SingleOutput` typed wrappers,
   - common CBP-aware destination resolver,
   - common artifact declaration/materialization helpers.
2. Migrate `copy` and `symlinked_dir` first (highest similarity).
3. Replace runtime `expect` for structural invariants with constructor validation + typed wrappers.
4. Add shared unit test matrix for cardinality + CBP edge cases.

**Exit criteria**
- `copy` and `symlinked_dir` use shared helpers.
- Duplicated invariant snippets removed.
- New action implementations must use helper APIs.

---

## F3 (SEV-2): placeholder/stub surface in health checks and adjacent modules

**Observed**
- `#![allow(dead_code)]` plus "future diff" and "placeholder" markers in health-check-related modules.

**Origin**: Largely **agent-added** scaffolding.

**Execution plan (P1)**
1. Move non-shipping health-check paths behind explicit feature gates.
2. Split runtime core vs experimental modules.
3. Add CI check blocking new unconditional file-level `allow(dead_code)` in shipping crates.
4. Track each placeholder with issue ID + removal milestone.

**Exit criteria**
- No shipping module uses file-wide dead-code suppression as placeholder strategy.

---

## F4 (SEV-2): test suite misalignment (Buck2 legacy vs Bazel parity)

The current state mixes:
- inherited Buck2 behavior tests,
- migration placeholders,
- Bazel-9 parity gaps.

### Required direction

1. **Remove tests that validate Buck2-only semantics** where those semantics are explicitly out-of-scope for slug.
2. **Add/expand tests for Bazel features** required by parity charter.
3. **Migrate renamed-equivalent Buck2 features** into Bazel-semantics tests instead of deleting coverage.

### Test migration matrix

- **Delete**: Buck2-only API expectations (non-Bazel surface, removed native language rule expectations, WORKSPACE-era behavior).
- **Migrate**: tests whose intent is still valid but names/surface differ (e.g., old Buck2 naming of semantics now expressed as Bazel rule/module behavior).
- **Add**: Bazel-9 parity regressions:
  - native symbol removals and error shape,
  - bzlmod lockfile semantics,
  - rule loading/error behavior (`rules_cc`/`rules_python`/`protobuf` paths),
  - output/action parity constraints.

**Origin**: Mostly **inherited** tests with **mixed** migration edits.

**Execution plan (P1/P2)**
1. Build inventory: each test tagged `buck2-only`, `migratable`, or `bazel-parity`.
2. Remove `buck2-only` group.
3. Port `migratable` group to Bazel-surface assertions.
4. Backfill missing Bazel parity tests before deleting large legacy chunks.

**Exit criteria**
- Test suite default signal tracks Bazel 9 parity, not Buck2 historical behavior.

---

## F5 (SEV-3): panic/expect policy drift

**Observed**
- Runtime paths still contain numerous `expect/panic` usage.

**Origin**: Mixed inherited + agent-added.

**Execution plan (P2)**
- Policy: runtime paths return typed errors except documented impossible invariants.
- Enforce with lint profile for non-test targets and incremental cleanup in hot paths first.

## Integration with existing subplans

This subplan links to and tightens existing work:
- **Plan 12**: stub cleanup naming and real implementations.
- **Plan 15**: Bazel 9 parity guardrails.
- **Plan 18**: BEP parity (projection consistency impacts event consumers).
- **Plan 47/48**: parity-gap closure and completion.

## Milestones

1. **M0 (Immediate)**: land shared projector design doc + helper API sketch for action impl consolidation.
2. **M1 (1-2 weeks)**: complete F1 projector unification and parity tests.
3. **M2 (2-3 weeks)**: complete F2 helper migration for copy/symlinked_dir + follow-on action adopters.
4. **M3 (parallel)**: test inventory and Buck2-only test retirement plan.
5. **M4**: placeholder and panic-policy hardening in CI.

## Non-goals

- No Bazel 8 compatibility shims.
- No resurrection of WORKSPACE-era behavior.
- No "temporary" divergence from Bazel 9 error shapes.
