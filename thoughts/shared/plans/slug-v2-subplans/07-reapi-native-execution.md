# Stage 7: REAPI-Native Execution

## Goal

Make REAPI the primary and routine execution boundary for Slug V2.

## Scope

- REAPI `Command`, input tree, CAS upload, `Action`, AC lookup/update,
  `Execute`, output materialization, and evidence logging.
- NativeLink local service bootstrap for local and CI validation.
- actiond as an optional backend behind the same REAPI surface.
- direct-local execution only for narrowly scoped debugging, never as parity
  proof.
- remote cache identity as `ActionDigest -> ActionResult`.

## V1 Extraction Candidates

- Plan 34 NativeLink smoke harness.
- Plan 31 persistent action-cache tests.
- what-ran and what-uploaded evidence patterns.
- materializer and stale-entry handling where it reuses Buck2 RE contracts.

## Acceptance Criteria

- A one-action shell fixture executes through NativeLink REAPI with zero
  direct-local actions.
- A generated-output fixture uploads inputs, materializes outputs, and can feed
  a downstream action.
- Remote Action Cache hit proof survives Slug daemon restart and local persistent
  cache deletion.
- Hosted CI cannot silently skip the local REAPI proof on Linux.

## Validation

```bash
slug-v2-oracle run --fixture shell-action-reapi
slug-v2-oracle run --fixture reapi-action-cache-hit
slug-v2-oracle run --fixture reapi-generated-output-reupload
```
