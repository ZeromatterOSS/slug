# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-output-semantics-probe-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transported output and aggregate-prime stage result.

## Goal and required evidence

From the clean scheduling commit, use the accepted in-memory wrapper to invoke
exactly one child with repository cwd, `env` omitted, `shell=False`, and
anonymous `TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_output_semantics_probe.py
```

The wrapper checks stderr size only, bounds stdout at 2 KiB, validates exact
normalized/canonical JSON, and emits only the fixed transport envelope. The
terminal caller retains and polls one session ID and never starts another
command. Inherit the environment unchanged so Bazel alone may consume private
ordinary/home RC. Never inspect raw child, RC/token, BEP/execution, invocation,
or service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, invalid
envelope, `REJECTED`, nonempty child stderr, sanitizer result, cleanup/Git/daemon
drift, or raw exposure is `REPLAN`. `DELIVERED` accepts transport only.
`PRIME_READY` routes to a replay-only cache discriminator; any fixed rejection
gets one narrow source/design route. Cache/RBE and the 43-test gate remain open.
