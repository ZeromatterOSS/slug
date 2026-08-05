# Current Slug V2 Packet

Packet: `WP-6-m2-native-toolchain-declaration-loading-design`
Milestone: M2 successful toolchain/platform selection
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: read-only design for the smallest native declaration/loading prerequisite
Predecessor: accepted ordered root registration retention `4a3af8df`.

Inspect the accepted first-compatible Bazel 9.2 fixture, pinned source, and live
Stage 4/6 owners. Freeze the smallest exact serial implementation boundary for:

1. native `constraint_setting`, `constraint_value`, `platform`,
   `toolchain_type`, and `toolchain` declarations in the fixture's root BUILD;
2. the one mandatory `rule(toolchains = ["//:demo_type"])` requirement;
3. typed labels, ordered registration/declaration semantics, constraint mapping,
   and structural equality/invalidation; and
4. `platform_common.ToolchainInfo(marker = <string>)` ownership, deciding
   explicitly whether it belongs with loading or a later selected-implementation
   analysis/context packet.

The design must identify exact production and test owners, DICE/loading handoff,
caps, lifecycle evidence, source anchors, utility reuse, and stop conditions.
Prefer serial prerequisites when semantic values can be retained truthfully
without inventing a consumer. Return `REPLAN` if any proposed slice would only
wire dormant digest/event scaffolding, represent a builtin provider as a user
provider, add a second graph/evaluator, or claim diagnostics absent from the
accepted positive oracle.

Do not edit Rust, Cargo, fixtures, harness, commands, or public behavior in this
packet. Keep real toolchain selection, selected implementation analysis,
prepared `ctx.toolchains`, configuration identity, command-line registration,
patterns, external repositories, aliases, host fallback, optional/multiple
types, target constraints, exec groups, actions, aquery, REAPI, and failure
diagnostics deferred. Direct-local MODULE dependency cycles remain the
user-approved unsupported boundary for later.

After parallel pinned-source, live-owner, and retained-utility audits, obtain a
reserved boundary review before authorizing Rust.
