# Current Slug V2 Packet

Packet: `WP-6-m2-root-toolchain-registration-retention-implementation`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: retain ordered direct root MODULE registrations through the existing
Need-aware loading anchor
Predecessor: accepted first-compatible Bazel 9.2 toolchain oracle `ed4baf08`.

Implement the accepted serial prerequisite only:

1. Add `RootModuleRegistrations` to `EvaluatedRootModule`, with separate
   immutable ordered execution-platform and toolchain label slices and slice
   accessors.
2. Record root `register_execution_platforms` and `register_toolchains` calls
   in exact argument and call order, with no sort or deduplication.
3. Retain a dev registration exactly when
   `!dev_dependency || !ignore_dev_dependency`, using the existing root command
   policy and ordering validation.
4. Fail closed before retention unless each argument is a direct absolute
   apparent label. Reject recursive, package-wide, and wildcard patterns
   locally; do not extend or depend on target-pattern expansion.
5. Freeze evaluator-local vectors into the semantic root-module value and
   expose registrations through the existing `RootModuleLoadingAnchor` only.

Required tests prove exact multi-call order, root and apparent-repository label
retention, non-string/relative/pattern rejection without claiming public
diagnostic parity, default→ignore→restore dev policy, registration order
A→B→A restoration, unchanged warm structural equality, Host-only event
ownership, and the anchor's existing sole dependency.

Production allowlist:

- `app/slug_bzlmod_v2/src/module_eval.rs`
- `app/slug_bzlmod_v2/src/host_module.rs`
- `app/slug_bzlmod_v2/src/lib.rs`

Test allowlist additionally permits:

- inline tests in `app/slug_bzlmod_v2/src/host_module.rs`
- `app/slug_bzlmod_v2/tests/root_module_dice.rs`

Caps are 220 formatted production net lines, 300 test lines, and 520 total.

Stop and return `REPLAN` for pattern expansion, external repository
materialization or mapping, command-line registrations, registered target
loading, native platform/toolchain declarations, constraint resolution,
`ToolchainInfo`, `ctx.toolchains`, configuration checksum/display changes,
public cquery or formatter behavior, actions, REAPI, a new DICE key, a digest
bridge, or process-global state. Direct-local MODULE dependency cycles remain
the user-approved unsupported boundary for later.

After acceptance, design the next serial owner for the fixture-bounded native
constraint/platform/toolchain declarations before real DICE resolution and
prepared `ctx.toolchains` ownership.
