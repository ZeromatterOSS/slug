# Slug Routing Guide

Read this only when the SKILL routing table does not decide the route.

| Work | Route |
|------|-------|
| Small read-only/mechanical | Root |
| Audit, oracle, focused tests, one abstraction | Terra medium |
| Approved difficult Rust/DICE/Starlark/query | Terra high |
| Reserved decision/risky final review | Sol low |
| Concrete unresolved architecture miss | Sol high |

Guardrails:

- Use one write worker and one reviewer by default.
- Add parallel read-only audits only for distinct unresolved questions.
- Reuse accepted Bazel 9.2 evidence; add an oracle only for a gap.
- Public interfaces need production-wrapper coverage.
- Identity/order/equality/invalidation/formatting need a discriminating case.
- Match validation to risk; broad, daemon, and cross-target gates are not
  defaults.
- Check process ownership before retrying Cargo or daemon validation.
- Run fixture hygiene before packet six or +100 files/+10,000 text lines.
- Close with one owner update and one live routing row; rotate history only
  when the live log is full.
