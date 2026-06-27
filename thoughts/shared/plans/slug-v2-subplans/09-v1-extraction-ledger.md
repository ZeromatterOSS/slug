# Stage 9: V1 Extraction Ledger

## Goal

Track every deliberate extraction from Slug V1 into V2 so useful work is
preserved without importing V1 architectural debt blindly.

## Extraction Rule

An extraction is acceptable only when it has:

- a named V2 owner stage;
- the V1 source path or test path;
- the Bazel source/test oracle or oracle fixture that justifies it;
- a clear import mode: copy, port, rewrite from behavior, reference-only, or
  reject;
- validation evidence after landing.

## Workflow

1. Open the V2 owner plan and identify the exact behavior needed.
2. Inspect V1 source and tests, plus the Bazel oracle source/test.
3. Choose an import mode:
   - `copy`: code is infrastructure with no V1 semantic leak;
   - `port`: code is useful but names/types/path assumptions must change;
   - `rewrite from behavior`: keep tests/lessons, not implementation shape;
   - `reference-only`: external implementation or backend contract only;
   - `reject`: document why the V1 surface should not enter V2.
4. Add or update an oracle fixture before landing the extraction.
5. Update this ledger and the owner plan with validation evidence.

## Review Checklist

Before moving an entry out of `Proposed`, answer:

- Does it contain Buck cell identity, `buck-out`, `BUCK`, `.buckconfig`, or
  direct-local assumptions?
- Does it rely on process-global state or fallback scanners?
- Which DICE key owns the semantic value in V2?
- Which Bazel source/test proves the behavior?
- Which command proves the extraction after landing?

## Ledger

| Status | V2 Stage | V1 Surface | Import Mode | Oracle / Validation |
|--------|----------|------------|-------------|---------------------|
| Proposed | Stage 5 | `app/slug_bzlmod/src/parser.rs` | Port selectively | `MODULE.bazel` parser fixtures against Bazel |
| Proposed | Stage 5 | `app/slug_bzlmod/src/extension_execution_dice.rs` | Rewrite from behavior plus selective port | module extension replay fixtures |
| Proposed | Stage 5 | `tests/core/bzlmod/test_plan61_guardrails.py` | Port tests after fixture cleanup | bzlmod same-daemon replay fixtures |
| Proposed | Stage 7 | `tests/plan34/test_reapi_local_executor_smoke.py` | Port harness | NativeLink REAPI fixture |
| Proposed | Stage 7 | `tests/plan34/validate_reapi_evidence.py`, `tests/plan34/test_ci_gate.py`, `.github/actions/setup_plan34_nativelink/action.yml`, `.github/actions/run_plan34_reapi/action.yml`, `.github/workflows/plan34-reapi.yml` | Rewrite behavior into V2 oracle harness; copy only small validator schema ideas | `shell-action-reapi` plus evidence validator |
| Proposed | Stage 7 | `app/slug_execute/src/execute/action_digest.rs`, `app/slug_execute/src/execute/action_digest_and_blobs.rs` | Port concepts, rewrite implementation around Bazel action declarations | REAPI action identity fixtures |
| Proposed | Stage 7 | `app/slug_execute_impl/src/executors/re.rs`, `app/slug_execute/src/re/client.rs`, `app/slug_execute/src/re/remote_action_result.rs`, `app/slug_execute_impl/src/re/download.rs` | Selective rewrite; reject Buck executor/config assumptions | upload/execute/download REAPI fixtures |
| Proposed | Stage 7 | Plan 31 persistent action-cache tests | Port tests and materializer checks | REAPI AC hit/stale-entry fixtures |
| Proposed | Stage 7 | `app/slug_execute_impl/src/sqlite/action_cache_db.rs`, `app/slug_execute_impl/src/sqlite/tables/action_cache_table.rs`, `app/slug_execute_impl/src/executors/action_cache.rs`, `app/slug_execute_impl/src/executors/caching.rs`, `app/slug_server/src/daemon/state.rs`, `tests/plan31/test_persistent_re_action_cache.py` | Port schema/value semantics and stale-entry behavior; Stage 3 owns output/cache layout | durable RE `ActionDigest -> ActionResult` fixtures |
| Proposed | Stage 7 | Plan 34 paramfile/generated-output fixtures | Port fixtures after Stage 6 can declare matching action graphs | `reapi-paramfile-input-tree` and `reapi-generated-output-reupload` |
| Proposed | Stage 7 / Stage 8 | Plan 34 `cc_actions` and `rules_cc` fixture themes | Stage 7 owns execution evidence; Stage 8 owns ruleset conformance breadth | `rules-cc-reapi-basic` plus Stage 8 rules_cc fixtures |
| Reference only | Stage 7 | NativeLink source checkout | Backend contract reference, not Slug import | same REAPI evidence with `remote_service=nativelink` |
| Reference only | Stage 7 | actiond source checkout | Optional REAPI backend validation only | same REAPI evidence with `remote_service=local_actiond` |
| Proposed | Stage 6 | depset/provider/rule implementation tests | Port tests | Bazel depset/provider oracle probes |
| Proposed | Stage 3 | V1 label/repo mapping helpers | Rewrite from behavior | Bazel label/output path oracle fixtures |
| Proposed | Stage 4 | V1 `slug_interpreter_for_build` globals/loading tests | Port tests, rewrite loading boundary | Bazel `PackageFunction`/`BzlLoadFunction` fixtures |
| Proposed | Stage 6 | V1 `cc_common` feature and provider work | Port selectively | rules_cc public fixture plus Bazel analysis oracle |
| Proposed | Stage 8 | V1 public ruleset smoke fixtures | Port tests after sanitizing versions and paths | ruleset oracle fixtures |
| Proposed | Reject by default | Direct-local executor success, copied-output bridge hits, Buck output-root spelling, and old compiled NativeLink/actiond adapter artifacts | Reject | Does not prove V2 Bazel REAPI parity |
| Proposed | Reject by default | V1 Buck cell graph and fallback cell machinery | Reject unless Stage 5 proves a Bazel equivalent | Stage 3/5 identity and bzlmod tests |
| Proposed | Reject by default | V1 BXL user surface | Defer as Slug extension | Not part of Bazel compliance |

## Evidence Template

Use this when updating a ledger row after landing:

```text
Status:
V2 commit:
V1 source inspected:
Bazel oracle:
V2 fixture:
Expected evidence artifact:
Implementation summary:
Validation:
Residual risk:
```

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

Doc-only validation:

```bash
git diff --check -- thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md
```
