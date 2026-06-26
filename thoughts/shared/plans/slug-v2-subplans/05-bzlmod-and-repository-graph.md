# Stage 5: Bzlmod and Repository Graph

## Goal

Implement bzlmod as DICE-owned semantic state: module parsing, resolution,
repo mappings, repository specs, module extensions, lockfile policy, and
materialization manifests.

## Scope

- `MODULE.bazel` parsing and validation.
- MVS resolution and yanked-version policy.
- Bazel Central Registry and override handling.
- repository mappings for root, module repos, and extension-generated repos.
- module extension usages, aggregation, execution, facts, and generated repos.
- `MODULE.bazel.lock` read/write/update/error modes.
- repository-rule execution and materialization.

## V1 Extraction Candidates

Review and selectively extract from:

- `app/slug_bzlmod/src/parser.rs`
- `app/slug_bzlmod/src/dice_graph.rs`
- `app/slug_bzlmod/src/extension_execution_dice.rs`
- `app/slug_bzlmod/src/lockfile.rs`
- `app/slug_bzlmod/src/repo_mapping.rs`
- `tests/core/bzlmod/test_plan61_guardrails.py`

Each extraction needs an oracle fixture or direct Bazel source citation.

## Acceptance Criteria

- No process-global semantic registry is required for bzlmod correctness.
- Same-daemon create/edit/delete transitions replay for clear DICE reasons.
- Lockfile replay rejects stale repo mappings, stale extension facts, and
  changed watched inputs.
- Generated repositories materialize through auditable DICE-owned state.

## Validation

```bash
cargo test -p slug_bzlmod_v2
slug-v2-oracle run --fixture module-resolution-basic
slug-v2-oracle run --fixture module-extension-lockfile-replay
```
