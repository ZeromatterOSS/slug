# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-prime-execution-artifact-contract-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one sanitized execution-artifact replacement result.

## Goal and required evidence

From the clean scheduling commit, invoke exactly once:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_execution_artifact_probe.py
```

Inherit the environment unchanged so only Bazel consumes ordinary/home RC.
Never inspect, print, copy, or persist the RC/token, `HOME`, private contents,
invocation URLs, or BuildBuddy UI data. Review only CLI status, empty stderr,
and its compact normalized record. Accept the probe only for exit zero, fixed
mode/schema, and `PROBE_RECORDED`.

The only result values are process `ZERO|NONZERO` and execution
`ANCHORED_PRIVATE_NONEMPTY|ANCHORED_PRIVATE_EMPTY|NOT_ANCHORED_PRIVATE`.
The probe never reads the artifact and owns anchored shutdown, cleanup of both
original/replacement private roots, Git cleanliness, and no-`slugd`.

## Stops and budget

Do not retry, read artifacts, modify code/config, or claim cache/RBE. Any CLI,
schema, stderr, or lifecycle failure is `REPLAN`. Route only the fixed record:
`ZERO+NONEMPTY` to a separate strict parser-discriminator design; `ZERO+EMPTY`
to a source-consistent no-record stop; `ZERO+NOT_ANCHORED` to `REPLAN`;
`NONZERO` plus an anchored file to a failure-detail sanitizer design; and other
`NONZERO` to a user-owned token-free environment decision.

Afterward only owner/canonical/current docs may record the result, at most 100
changed lines. Structured cache/RBE, 43-test expansion, and Stage 10 remain.
