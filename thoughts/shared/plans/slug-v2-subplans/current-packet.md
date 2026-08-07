# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-output-semantics-probe-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline-reviewed output and aggregate-prime discriminator.

## Goal and required design

Add only library/CLI/tests named
`buildbuddy_build_cache_prime_output_semantics_probe.py` under their existing
directories, capped at 190/30/260 and 480 total lines. Do not edit existing
gate/probes, configuration, targets, fixtures, or docs.

Reuse the exact one-prime command, retained descriptors, strict BEP and
replacement-aware execution readers, parsers, output scan, anchors, RC-disabled
shutdown, dual-root cleanup, Git, and no-`slugd`. Check output first and do not
read BEP/execution if scanning fails or exactly one executable is not materialized.
Then parse both private artifacts and apply only the existing opaque prime
predicate. Emit no values, counts, paths, digests, runners, errors, or content.

The exact record uses the established schema/process pairing and fixed stages:
`NOT_RECORDED`, `PRECHECK_REJECTED`, `SETUP_REJECTED`, `PROCESS_NONZERO`,
`POST_RUN_ANCHOR_REJECTED`, `OUTPUT_SCAN_REJECTED`,
`OUTPUT_MATERIALIZATION_REJECTED`, `POST_OUTPUT_ANCHOR_REJECTED`,
`BEP_DESCRIPTOR_REJECTED`, `BEP_PHASE_REJECTED`,
`EXECUTION_DESCRIPTOR_REJECTED`, `EXECUTION_SPAWN_REJECTED`,
`POST_PARSE_ANCHOR_REJECTED`, `PRIME_SEMANTICS_REJECTED`, or `PRIME_READY`.

## Stops and budget

Offline mocks cover exact argv; output missing/non-executable/link/multiple/scan
failure and no-reader ordering; every descriptor/parse/semantics stage; root/
phase/output/read/shutdown swaps; cleanup/setup/process failures; schema/privacy
and clean lifecycle. Run focused and related tests, compile/caps/scope/diff, and
independent review only. No Bazel, home RC, network, service, or live artifact.
Existing-file edit, public expansion, ambiguous ownership, or second material
correction is `REPLAN`. A later packet owns one transported result; cache/RBE
and the 43-test gate remain open.
