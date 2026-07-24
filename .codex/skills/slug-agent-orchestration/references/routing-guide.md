# Slug Routing Guide

Use this summary for normal packet selection. Historical logs are optional.

| Work | Route |
|------|-------|
| Small read-only/mechanical task | Root |
| Audit, oracle, focused tests, one abstraction | Terra medium |
| Periodic fixture-growth inventory and pruning proposal | Terra medium |
| Approved multi-file Rust/DICE/Starlark/query change | Terra high |
| Reserved-decision or risky-patch review | Sol low |
| Concrete unresolved architecture miss | Sol high review |

Keep a high-capability Sol root for long autonomous plan execution when the
surface permits it. Default to one write worker with `fork_turns="none"`;
parallel workers must be read-only or own disjoint files.

Observed guardrails:

- Root inspects diffs and owns downstream/broad validation and commits.
- Public/cross-crate changes need production-wrapper coverage.
- Identity, ordering, equality, invalidation, and formatting need a
  discriminating case beyond happy-path output.
- Check process ownership before retrying Cargo or daemon validation.
- Do not infer semantics from nondiscriminating oracle output.
- After five accepted oracle packets, 100 net fixture files, or 10,000 net
  fixture text lines of growth, route a fixture-hygiene review before another
  oracle packet. Record exact packet IDs and aggregate before/after counts.
  When no checkpoint exists, inventory the accepted tree as the first baseline.
  Prune only exact redundant or nondiscriminating material and replay every
  affected fixture.
- Close a packet with one owner-plan evidence update and one routing-log row.
  Touch canonical Live Status only for a scheduling change, never mirror a live
  row into routing history, and avoid per-phase status commits.
