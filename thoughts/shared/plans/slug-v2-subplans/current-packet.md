# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-prime-normal-rc-sanitized-stderr-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one fixed token-safe diagnosis of the normal-RC prime exit two.

## Goal and required design

From clean implementation commit `ec8ec2d7…`, run exactly once with the
inherited process environment:

```text
python3 tools/v2_oracle/buildbuddy_prime_diagnostic.py
```

Only the Bazel child may consume ordinary/home RC. Do not inspect/expand home
RC or read stdout/BEP/execution/private files. Review only the CLI process
status, empty CLI stderr, and its single closed JSON object.

Accept only CLI exit zero with `schema_version=1`, classification
`NORMAL_RC_PRIME_DIAGNOSED`, and exactly one of the five frozen
`CHECKED_IN_OPTION_*` identifiers. That result diagnoses one public checked-in
prime flag but proves no cache/RBE/test behavior.

## Stops and budget

Any other status/class/schema, CLI stderr, private-path retention, Git/daemon
drift, or cleanup failure is `REPLAN`. Do not retry, inspect raw/home data,
bisect, change code/config/profile/backend, or make a cache/RBE claim. After the
run only owner/canonical/current docs may record the fixed result, at most 100
changed lines; a diagnosed ID schedules one separately reviewed repair.
