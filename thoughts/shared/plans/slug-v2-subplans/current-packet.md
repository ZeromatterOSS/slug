# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-phase-explicit-transported-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one fixed paired one-label build-cache record.

## Goal and required evidence

From the clean scheduling commit, confirm only fixed counts: zero direct
`slug-buildbuddy-prime-*` temporary entries, zero matching private-output-base
processes, clean Git, and no `slugd`. Use an in-memory wrapper to invoke exactly
one child with repository cwd, `env` omitted, `shell=False`, and anonymous
`TemporaryFile` stdout/stderr:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_gate.py
```

The wrapper bounds stdout at 4 KiB, requires empty stderr, validates exact
normalized/canonical gate JSON, and emits only a fixed transport envelope plus
the normalized compact record. Retain and poll one session ID; never start
another command. The wrapper, gate, both Bazel phases, and shutdown inherit the
environment unchanged where applicable, so only Bazel may consume private
ordinary/home RC. Never inspect raw child output, RC/token, BEP/execution,
temporary contents, invocation, or service data.

## Stops and budget

Do not reissue, inspect artifacts, or modify code/config. Session loss, invalid
envelope, nonempty child stderr, sanitizer result, retained root/process,
Git/daemon drift, or raw exposure is `REPLAN`. `DELIVERED` accepts transport
only. Accept the cache vertical only with outer and child zero, empty stderr,
exact schema/mode, and `PROVED_BUILD_CACHE`; any other fixed classification is
one bounded route without retry. Recheck only fixed root/process/daemon/Git
counts afterward. This can prove only the one build label; structured RBE and
the 43-target expansion remain separate successors.
