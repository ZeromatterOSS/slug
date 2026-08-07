# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-lifecycle-guard-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one lifecycle-guarded prime stage result.

## Goal and required evidence

From the clean scheduling commit, first confirm only fixed counts: zero direct
`slug-buildbuddy-prime-*` temporary entries, clean Git, and no `slugd`. Use the
accepted in-memory wrapper to invoke exactly one child with repository cwd,
`env` omitted, `shell=False`, and anonymous `TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_lifecycle_guard.py
```

The wrapper bounds stdout at 2 KiB, requires empty stderr, validates exact
normalized/canonical guard JSON, and emits only the fixed transport envelope.
The terminal caller retains and polls one session ID and never starts another
command. The outer wrapper, guard, frozen output-semantics child, and Bazel all
inherit the environment unchanged, so only Bazel may consume private ordinary/
home RC. Never inspect raw child, RC/token, BEP/execution, temporary contents,
invocation, or service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, invalid
envelope, nonempty guard stderr, sanitizer result, cleanup/Git/daemon drift, or
raw exposure is `REPLAN`. `DELIVERED` accepts transport only. Only
`LIFECYCLE_CLEAN` permits routing by the normalized nested stage;
`ROOT_RESIDUE_REMOVED` requires a cleanup repair before another semantic attempt,
and every other non-clean lifecycle is `REPLAN`. Recheck only fixed root/process/
daemon/Git counts afterward. Cache/RBE and the 43-test gate remain open.
