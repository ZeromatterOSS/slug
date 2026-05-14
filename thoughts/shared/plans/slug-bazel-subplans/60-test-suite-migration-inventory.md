# Test Suite Migration Inventory (Buck2 legacy -> Bazel 9 parity)

This document starts F4 from `59-deslop-remediation-and-agentic-debt.md` by tagging high-signal test surfaces as:

- `buck2-only` (delete)
- `migratable` (port assertions to Bazel surface)
- `bazel-parity` (keep/expand)

## Initial inventory (phase 1)

| Area | Path(s) | Tag | Notes / next action |
|---|---|---|---|
| Interpreter-for-build tests | `app/slug_interpreter_for_build_tests/src/tests.rs` | migratable | Keep behavioral intent, rename/update assertions for Bazel 9 Starlark/module semantics. |
| Build API tests | `app/slug_build_api_tests/src/**` | migratable | Audit Buck2 naming in rule/provider tests; port to Bazel naming and error-shape expectations. |
| Query tests | `app/slug_query/src/query/**/tests.rs` | migratable | Retain graph/query semantics where Bazel-equivalent; drop Buck2-only CLI/output assumptions. |
| Event observer projector tests | `app/slug_event_observer/src/projection.rs` | bazel-parity | Keep and extend: shared projection behavior guards `what-ran` + `explain` parity. |
| Health-check lint test | `app/slug_health_check/tests/dead_code_lint.rs` | bazel-parity | Keep as policy guard for placeholder control in shipping OSS modules. |
| Action impl unit tests | `app/slug_action_impl/src/actions/impls/*` | bazel-parity | Expand around constructor/cardinality invariants and Bazel-consistent action contracts. |

## Immediate follow-up batches

1. **Batch A (delete candidates)**
   - identify tests asserting Buck2-only symbols/surfaces removed by Bazel 9 parity charter;
   - remove after adding equivalent Bazel-coverage where needed.

2. **Batch B (migration candidates)**
   - port rule/provider/interpreter tests that still validate relevant semantics but use legacy Buck2 names.

3. **Batch C (parity backfill)**
   - add targeted regressions for:
     - native symbol removals (`CcInfo`/`PyInfo`/`ProtoInfo` globals),
     - `rules_cc`/`rules_python`/`protobuf` load/error shape,
     - lockfile and module behavior covered by Bazel 9.

## Exit criteria for this inventory

- every touched test file gets one of: `delete`, `migrate`, `keep`;
- no unclassified legacy test remains in modified areas;
- parity backfills land before large delete wave.
