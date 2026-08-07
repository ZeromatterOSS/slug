# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-prime-execution-artifact-contract-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a pinned-source execution-artifact replacement contract.

## Goal and required design

Add only `buildbuddy_build_cache_execution_artifact_probe.py` as library (190 lines), CLI (35), and tests (270) under the matching existing directories: 495 lines maximum.

Bazel 9.2 `ExpandedSpawnLogContext` lines 106-130 and 291-316 at pinned commit `8220c619…` require JSON conversion, delete a preexisting output, and create the
final output while closing the converter. `ExecutionOptions` lines 420-436 define it as executed-spawn records, so an empty final file is possible. Reuse the
exact frozen prime command but accept this source-required execution-file inode replacement only inside the retained private root/phase.

Precreate mode-0600 execution evidence, then inspect only its final direct-child
metadata through the retained phase descriptor. Symlink, hardlink, directory,
bad mode, or lost root/phase identity is `NOT_ANCHORED_PRIVATE`; a single-link
regular mode-0600 file is `ANCHORED_PRIVATE_NONEMPTY` or
`ANCHORED_PRIVATE_EMPTY`. Never open/read/hash content or expose exact metadata.

The closed deep-normalized record has exactly schema version one, fixed mode
`buildbuddy-build-cache-prime-execution-artifact-probe`, classification
`PROBE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and the execution
enum above. Rejection uses fixed conservative values. Reuse descriptor-safe
shutdown/original-inode cleanup and clean-Git/no-`slugd` guards.

## Stops and budget

Offline mocked tests cover exact command reuse; retained/replaced empty/nonempty
regular files; missing/symlink/hardlink/bad-mode/directory; phase/root swaps;
no-open/read enforcement; hostile schema; cleanup/shutdown/Git/daemon failures;
and secret suppression. Run focused tests, compilation, caps/diff, and
independent review only. Do not invoke Bazel, normal/home RC, remote services,
or edit the accepted gate/probes/config/targets/docs.

Stop on any need to accept links, relax mode/single-link constraints, read
bytes, expose metadata, or make a cache/RBE claim. A separate packet owns one
live probe. Structured cache/RBE, 43-test expansion, and Stage 10 remain open.
