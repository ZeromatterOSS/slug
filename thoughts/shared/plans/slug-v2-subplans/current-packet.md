# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one transport-delivered fixed-enum one-prime stage result.

## Goal and required evidence

From the clean scheduling commit, an in-memory, non-repository Python wrapper
invokes exactly one child with `cwd` at the repository root, `env` omitted,
`shell=False`, and anonymous `TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_prime_stage_probe.py
```

The wrapper checks stderr emptiness by size only, rejects stdout over 2 KiB,
validates UTF-8 JSON through the probe's exact normalizer/canonical serializer,
and emits no child bytes unless valid. Its closed envelope contains only fixed
transport class `DELIVERED|REJECTED`, child process `ZERO|NONZERO`, child stderr
`EMPTY|NONEMPTY`, and the normalized probe record only when delivered.

Inherit the environment unchanged so Bazel alone may consume ordinary/home RC.
The terminal caller retains and polls any returned session ID; it never starts
a second command. Never inspect or persist RC/token, `HOME`, raw child output,
BEP/execution contents, invocation data, or remote-service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, outer
failure/non-envelope output, `REJECTED`, nonempty child stderr, sanitizer result,
retained state, Git/daemon drift, or cleanup failure is `REPLAN`. `DELIVERED`
plus a fixed child record accepts transport only. Route its stage as specified;
`PRIME_READY` schedules a separate replay-stage probe. Cache/RBE, the 43-test
expansion, and Stage 10 remain required.
