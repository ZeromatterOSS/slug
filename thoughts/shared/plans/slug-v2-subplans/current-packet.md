# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-semantic-branch-discriminator-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one lifecycle-clean fixed prime semantic branch.

## Goal and required evidence

From the clean scheduling commit, confirm only fixed counts: zero direct
`slug-buildbuddy-prime-*` temporary entries, clean Git, and no `slugd`. Use the
accepted in-memory wrapper to invoke exactly one child with repository cwd,
`env` omitted, `shell=False`, and anonymous `TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_lifecycle_guard.py
```

The wrapper bounds stdout at 2 KiB, requires empty stderr, validates exact
normalized/canonical guard JSON, and emits only the fixed transport envelope.
Retain and poll one session ID; never start another command. The outer wrapper,
guard, output-semantics child, and Bazel inherit the environment unchanged, so
only Bazel may consume private ordinary/home RC. Never inspect raw child,
RC/token, BEP/execution, temporary contents, invocation, or service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, invalid
envelope, nonempty guard stderr, sanitizer result, cleanup/Git/daemon drift, or
raw exposure is `REPLAN`. `DELIVERED` accepts transport only. Only
`LIFECYCLE_CLEAN` permits routing by the nested stage. `PRIME_READY` advances to
replay-only cache evidence. Process/build-finished/output/runner-partition
rejections are contract contradictions and `REPLAN`; each other fixed rejection
gets one corresponding narrow source/design route without retry or cache/RBE
claim. Recheck only fixed root/process/daemon/Git counts afterward.
