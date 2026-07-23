# Slug Agent Routing Log

## Current Recommendations

- Use Terra medium for the default bounded implementation or discovery packet.
- Use Terra high for contained multi-file Rust, Starlark/query, DICE, or
  invalidation work with an approved architecture.
- Use Sol low for concise architecture/parity adjudication and risky-patch
  review.
- Keep trivial tasks in the root; coordination overhead is a real cost.
- Prefer minimal-context forks and one worker before adding concurrency.
- For public API changes, require a downstream-crate compile and a
  production-wrapper regression; owner-crate tests alone are insufficient.

Change these recommendations only from observed results, not model reputation.

## Event Schema

| Date | Packet / task class | Route | Context / parallelism | Tokens / cost | Outcome and validation | Rework / escalation | Next-use recommendation |
|------|---------------------|-------|-----------------------|---------------|------------------------|---------------------|-------------------------|

Use exact token or cost data only when the orchestration surface exposes it.
Otherwise record `not exposed` and one qualitative band: `low`, `medium`, or
`high`.

## Events

| Date | Packet / task class | Route | Context / parallelism | Tokens / cost | Outcome and validation | Rework / escalation | Next-use recommendation |
|------|---------------------|-------|-----------------------|---------------|------------------------|---------------------|-------------------------|
| 2026-07-22 | Create orchestration policy and persistent routing log | Root only | Live repo context; no subagent because delegation overhead exceeded the edit | not exposed; low | Skill scaffold, routing policy, metadata, log, and AGENTS pointer validated locally | none | Keep similarly small policy/metadata edits in root |
| 2026-07-22 | M1 unified DICE spine source audit | Terra medium | `fork_turns=none`; one read-only worker | not exposed; medium | One-pass compact packet identified the fresh root DICE, separate loading graph, scanner deletion gap, and data-only analysis/bzlmod keys; root verified the cited source anchors | none; reserve Sol low for the cross-stage input/ownership decision before edits | Reuse Terra medium for bounded source archaeology with exact paths and a strict output cap |
| 2026-07-22 | M1 unified DICE ownership adjudication | Sol low | `fork_turns=none`; one reviewer after the Terra audit | not exposed; low | Revised the packet to unify root/loading immediately around one `WorkspaceRuntime`, retained `Dice`, injected present/absent file values, and one batch transaction; root verified `changed_to`/`InjectedKey` and cited ownership hazards | no implementation rework; narrowed analysis/query to consumers only | Reuse Sol low after a Terra source map for bounded DICE/cross-stage ownership decisions |
| 2026-07-22 | M1 DICE runtime post-implementation review | Sol low | `fork_turns=none`; one read-only reviewer | not exposed; low | Found an uninitialized missing-`.bzl` input panic path, a downstream analysis compile break, and a daemon create-after-missing gap; root reproduced the compile failure and verified all findings | triggered one focused correction of the Terra-high packet | Require Sol-low review for DICE ownership or cross-crate API patches; its compact defect yield was high |
| 2026-07-22 | M1 unified workspace runtime implementation | Terra high | `fork_turns=none`; one write worker, followed by one correction | not exposed; high | Implemented the retained `WorkspaceRuntime`, caller-owned loading transaction, injected immutable snapshot, and create/edit/delete/read-error regressions; root extended root BUILD fallback/change accounting, replaced the semantic `BTreeMap` with Buck2-derived `SortedMap`, and passed the five-crate suite including CLI | first pass required correction for three review findings; full snapshot remains a measured performance/invalidation residual | Reuse Terra high for approved multi-crate DICE work, but mandate production-wrapper tests and downstream compilation in the initial packet |
