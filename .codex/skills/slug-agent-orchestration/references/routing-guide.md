# Slug Routing Guide

Use this summary for normal packet selection. Historical logs are optional.

| Work | Route |
|------|-------|
| Small read-only/mechanical task | Root |
| Audit, oracle, focused tests, one abstraction | Terra medium |
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
