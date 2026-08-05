# Current Slug V2 Packet

Packet: `WP-6-m2-root-action-closure-boundary-design`
Milestone: M2 recursive configured-target action closure
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design only the configuration-opaque command-local closure that retains
actions from requested roots and their recursively analyzed dependencies.
Predecessor: accepted integrated toolchain resolution/context implementation
`1533569f` and accepted recursive target-owned action evidence.

The live gap is exact: each `AnalysisResult` owns its target-local actions and
configured dependency identities, but `BuildCommandEvaluation` retains only
requested-root analyses. Its action count and REAPI consumer therefore omit
actions owned by recursively analyzed dependencies.

Design an ordered, duplicate-safe closure from existing build roots through
`AnalysisResult::direct_dependencies`. Decide single-root, multi-root, shared
diamond, and configuration-distinct membership and ordering; preserve each
`AnalysisResult` as the sole owner of its own actions. Specify Need/error
precedence, same-DICE lifecycle and A-to-B-to-A equality, dependency
edit/delete/recreate pruning, and exact analysis/event ownership without a
second configured graph or duplicate child evaluation.

Reuse `recursive-custom-rule-providers-actions`, specifically the accepted
`aquery_recursive_target_owned_writes` evidence showing distinct parent and
two-leaf `FileWrite` actions. Use only the existing `ConfiguredTargetKey`
identity internally; do not format or invent Bazel configuration identity.

Documentation allowlist:

- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`

Caps are zero production lines, zero test lines, 320 documentation lines, and
320 total net lines. Return either one exact implementation packet or `REPLAN`
if a truthful closure requires unmodeled public configuration identity.

Stop for any Rust, test, fixture, or generated-oracle edit; configuration
checksum/short ID, configured output paths, execution-platform or action-key
identity, cquery/aquery formatting, general transition/configuration
substrate, external mapping, patterns, toolchain action breadth,
execution/REAPI behavior, or any new DICE key/cache/global/lock.
