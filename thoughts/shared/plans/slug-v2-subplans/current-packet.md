# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-nightly-channel-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: the cache gate selects the repository's registered nightly toolchain.

## Goal and required design

In `tools/v2_oracle_lib/buildbuddy_cache.py`, add exactly
`--@rules_rust//rust/toolchain/channel=nightly` once in `command()`, immediately
after `--config=buildbuddy-cache`. Extend the existing command test to pin the
exact spelling, unique occurrence, and order. This aligns the driver with the
sole `nightly/2025-09-14` toolchain registered by `MODULE.bazel` and the
accepted Stage 10 local-command boundary.

## Stops and budget

Change only the driver (5 changed lines) and its focused test (15), at most 20
total. Run only offline unit tests, Python compilation, diff/cap checks, and an
independent REPLAN review. Do not edit MODULE/locks/config/manifest/CLI/CI/
BUILD/targets, run Bazel, discover/inspect home RC, contact BuildBuddy, invoke
RBE, or make a live attempt. A separate reviewed packet owns later evidence,
preferably through sibling `../actiond` first as the user requested.
