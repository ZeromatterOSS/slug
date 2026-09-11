# Current Slug V2 Work Packet

Packet: WP-5-7A-native-probe-observer-attempt-design-r1

Status: SELECTED, DESIGN ONLY. The observer implementation is independently
ACCEPTED, but this packet does not authorize an authentic run.

## Baseline

The default-off `native-probe-observer` Core/CLI feature is accepted with a
fixed 512-byte sealed memfd, six independent DICE counters, double-buffered
phase/activity frames, post-process-tree-quiescence decoding, and exact native
API/fixture/publication ownership. Production/proof/gross additions are
564/about715/1279 within 650/1100/1750. Observer, mapping, test and driver owners
are 299/300, 158/160, 486/700 and 158/250 lines. Existing production runtime
call sites remain 120/120; the extra `runtime/mod.rs` lines are test-only proof.

Terminal review accepts FD validation/closure, mapping/Arc lifetime, no-wait
callback work, real DICE change/dependency-check events, semantic activation
tracking, one-shot API teardown order, bounded accounting, feature confinement,
decoder failures and supervisor cleanup. Focused tests pass 8/8 in 0.08 seconds;
the whole cached command was 5.53 seconds. Final feature Core no-run compilation
was 19.07 seconds, default CLI check 2.23 seconds, intended non-test feature
rejection 4.59 seconds, and final CLI feature compile-only 0.73 seconds after
self-checks. Latest harmless supervisor evidence is under
`/tmp/slug-sentinel-demand.sHLUmW/logs`.

No authentic observer run has executed. The prior killed diagnostic remains
INCONCLUSIVE and supplies no demand, phase, cause, performance, deadlock or
historical-14-GB attribution. Complete output-conflict R2 remains preserved and
unaccepted; no part may be restored by this packet.

## Goal

Design the smallest reviewable contract for at most one authentic invocation of
the already accepted `run_payload_demand_probe.sh` path. The design must decide
what a success, semantic failure, wall deadline, missing installation, corrupt
frame, incomplete cleanup and output failure permit us to conclude. It must make
UNKNOWN explicit for unsampled time and must not treat counters as a coherent
snapshot or phase samples as causal attribution.

The design may select one later execution only if it preserves the existing
exact ignored library test, staged input inventory, file-registry/mirror policy,
network namespace, process-group finalizer and limits: compiler at most 60
seconds; authentic native wall/CPU at most 15 seconds; AS 2 GiB per process;
FSIZE 16 MiB; stdout/stderr 8192 bytes each; trace 57344 bytes; raw plus decoded
telemetry at most 8192 bytes. No automatic retry, cap increase, daemon, broad
suite, Bazel execution, acquisition or fallback fixture is admissible.

## Authorized work

- Read the accepted ten-file observer implementation, its focused proof and the
  latest compile-only/self-check evidence.
- Read only the prior single killed-probe receipt and the exact staged-inventory
  recipe needed to freeze comparison claims. Do not scan caches or enumerate
  additional dependencies.
- Write a docs-only execution/review contract in this manifest, canonical Live
  Status and Stage 5 summary. Outside this manifest, add at most 80 lines.
- Obtain independent terminal review of the exact one-attempt contract before
  changing this packet to execution-authorized status.
- Keep `~/PROGRESS.md` current and below 500 lines.

## Prohibited work and stops

Do not compile, test, invoke the authentic probe, attach `strace`, replay any
network request, inspect credentials, acquire sources, alter Rust/Perl/fixtures,
restore output-conflict R2, or push an execution packet without independent
acceptance. Any need to change the accepted driver, staged inputs, exact test,
limits, source policy or output interpretation is REPLAN, not a correction here.

Preserve `/tmp/slug-conflict-r2.XZJWwv/candidate.patch` at SHA-256
`90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e`
and `/tmp/slug-sentinel-draft.Lln8y0/candidate.patch` at SHA-256
`8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2`.
