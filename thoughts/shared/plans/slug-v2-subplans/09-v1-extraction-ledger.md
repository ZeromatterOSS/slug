# Stage 9: V1 Extraction Ledger

## Goal

Track every deliberate extraction from Slug V1 into V2 so useful work is
preserved without importing V1 architectural debt blindly.

## Extraction Rule

An extraction is acceptable only when it has:

- a named V2 owner stage;
- the V1 source path or test path;
- the Bazel source/test oracle or oracle fixture that justifies it;
- a clear import mode: copy, port, rewrite from behavior, or reject;
- validation evidence after landing.

## Ledger

| Status | V2 Stage | V1 Surface | Import Mode | Oracle / Validation |
|--------|----------|------------|-------------|---------------------|
| Proposed | Stage 5 | `app/slug_bzlmod/src/parser.rs` | Port selectively | `MODULE.bazel` parser fixtures against Bazel |
| Proposed | Stage 5 | `app/slug_bzlmod/src/extension_execution_dice.rs` | Rewrite from behavior plus selective port | module extension replay fixtures |
| Proposed | Stage 5 | `tests/core/bzlmod/test_plan61_guardrails.py` | Port tests after fixture cleanup | bzlmod same-daemon replay fixtures |
| Proposed | Stage 7 | `tests/plan34/test_reapi_local_executor_smoke.py` | Port harness | NativeLink REAPI fixture |
| Proposed | Stage 7 | Plan 31 persistent action-cache tests | Port tests and materializer checks | REAPI AC hit/stale-entry fixtures |
| Proposed | Stage 6 | depset/provider/rule implementation tests | Port tests | Bazel depset/provider oracle probes |

## Rejection Template

Use this when a V1 surface should not be imported:

```text
Surface:
Reason rejected:
Replacement V2 owner:
Bazel oracle:
Cleanup needed in V1 archive docs:
```

## Validation

The ledger is documentation, but any entry moved out of `Proposed` must cite the
landed validation command in the owning stage plan.
