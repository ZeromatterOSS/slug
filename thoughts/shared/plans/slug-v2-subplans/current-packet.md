# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-bep-stage-probe-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transport-delivered fixed-enum BEP-only stage result.

## Goal and required evidence

From the clean scheduling commit, use the accepted in-memory wrapper to invoke
exactly one child, with repository cwd, `env` omitted, `shell=False`, and
anonymous `TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_bep_stage_probe.py
```

The wrapper checks stderr size only, rejects stdout over 2 KiB, validates exact
normalized/canonical JSON, and emits no child bytes unless valid. Its envelope
contains only `DELIVERED|REJECTED`, child `ZERO|NONZERO`, stderr
`EMPTY|NONEMPTY`, and the normalized record when delivered. The terminal caller
retains and polls one session ID until completion; it never starts another
command. Inherit the environment unchanged so Bazel alone may consume private
ordinary/home RC. Never inspect raw/home/BEP/execution/service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, invalid
envelope, `REJECTED`, nonempty child stderr, sanitizer result, cleanup/Git/daemon
drift, or raw exposure is `REPLAN`. `DELIVERED` accepts transport only. Route
`BEP_READY` to an execution-only discriminator; route a fixed BEP rejection to
its narrow format/source design. Cache/RBE and the 43-test gate remain open.
