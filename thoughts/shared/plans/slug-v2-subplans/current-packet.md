# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-full-gate-driver-reconciliation-design`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one frozen manifest-aware full-cache/full-RBE expansion contract.

## Goal and required evidence

Edit only the canonical, owner, and current-packet documents at respectively 8,
110, and 45 changed/total lines, at most 163 changed lines overall. Preserve the
tracked `slug-buildbuddy-targets-v1` manifest byte-for-byte at SHA-256
`3a717cb4b0a1f5cab06d336e69d2382861a9c21af9a1502ea20c54b990adf6d5`:
one production build label plus 43 sorted green test labels, including
`runtime_test` and excluding the expected-red `cli_fixture_test`.

## Stops and budget

Reconcile the tracked 361-line full-cache library, 29-line CLI, and 361-line
tests with the accepted one-label primitives; do not run that older driver
unchanged. Freeze four separate meanings: one-label `PROVED_BUILD_CACHE`, one-label
`PROVED_BUILD_RBE`, full-manifest `PROVED_CACHE_ONLY`, and distinct full-manifest
`PROVED_RBE`. Cache uses one manifest-aware CLI with fresh prime/replay output
bases and equal nonempty eligible digest multisets; RBE uses a separate CLI and
third fresh output base, all-SpawnExec remote-only proof, and 43 exact test
completions. Do not require cache/RBE cross-profile action equality.

Specify bounded offline implementation/review packets followed by two serialized
live packets: full cache once, then full RBE once. Reuse hardened no-follow
private-artifact, schema, shutdown, and clean-lifecycle primitives while leaving
the accepted one-label drivers unchanged. Return `REPLAN` on manifest/config/
platform drift, required target or credential changes, local-only tests, relaxed
classification, raw/home/UI access, cleanup residue, or any live retry. Validate
scope/caps, scheduling agreement, `git diff --check`, and independent design
review. No code/config/manifest/test/Bazel/network/home/artifact/service access.
