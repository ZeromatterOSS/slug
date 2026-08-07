# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-artifact-probe-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one sanitized metadata-only prime artifact result.

## Goal and required evidence

From the clean scheduling commit, invoke exactly once:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_artifact_probe.py
```

Inherit the environment unchanged so only Bazel consumes ordinary/home RC.
Never inspect, print, copy, or persist that RC/token, `HOME`, terminal or
artifact contents, invocation URLs, or BuildBuddy UI data. Review only CLI
status, empty stderr, and one compact normalized JSON record. Accept the probe
only for exit zero, fixed mode/schema, and `PROBE_RECORDED`.

The only payload values are process `ZERO|NONZERO` and BEP/execution
`PRIVATE_REGULAR|NOT_PRIVATE_REGULAR`. The latter requires a nonempty retained
private identity but exposes no exact metadata. The probe owns RC-disabled
shutdown, original-inode cleanup, Git cleanliness, and no-`slugd`.

## Stops and budget

Do not retry, inspect artifacts, diagnose a flag, modify code/config, or claim
cache/RBE. Any CLI/schema/stderr/lifecycle failure is `REPLAN`. Route a recorded
result only by its four fixed values: `NONZERO` plus any unusable artifact goes
to a user-owned token-free environment decision; `NONZERO` plus two usable
artifacts to a separately designed failure-detail sanitizer; `ZERO` plus any
unusable artifact to `REPLAN`; and `ZERO` plus two usable artifacts to a
separate strictly allowlisted parser-discriminator design.

Afterward only owner/canonical/current docs may record the fixed result, at most
100 changed lines. Structured build-only cache/RBE evidence, the full 43-test
expansion, and the rest of Stage 10 remain required.
