# Current Slug V2 Packet

Packet: `WP-6-7A-immutable-configured-action-owner-context-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and evidence base: `9ab2fa4a`
Accepted Rust base: `51127df8`
Result: freeze the smallest immutable analysis-owned configured-action row and
one bounded implementation packet.

## Exact docs-only authority and caps

Write exactly:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`,
2. this manifest, and
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`.

Caps against `9ab2fa4a`: canonical <=40 net, this manifest <=220, Stage 6
<=240 and aggregate <=500. Stage 1/7/8/10, the accepted fixture and live Rust
are read-only. Rust, Cargo, BUILD, fixture/oracle, generated evidence and
public/caller changes are forbidden.

## Accepted just-in-time evidence

The Bazel 9.2 `exec-groups-action-platform` oracle is generated and cleanly
replays six rows for two actions of one configured owner. The default action
omits Starlark's `exec_group` argument and stays on `default_platform`; the
named `compile` action stays on `compile_a` cold/warm. Editing only a property
on that same platform changes only the compile action's opaque ActionKey and
restoration recovers its prior token. Reordering compatible compile platforms
moves only that action to `compile_b`; restoring order returns its platform and
opaque token. Exact ActionKey bytes remain M9.

The evidence pins `RuleContext#getActionOwner(execGroup)`: Bazel derives the
per-group owner from configuration, aspect descriptors, merged exec properties
and selected execution platform. This fixture admits explicit absent aspect
provenance; applied aspects remain unsupported.

## Design authority

Freeze one analysis-owned immutable configured-action representation. Preserve
build-API `ActionSpec` as intrinsic action material; do not add configured
analysis types to the lower crate and do not create a DICE key. The retained
row (or authenticated compact projections with identical lifetime) must own:

- configured target owner and complete structural configuration identity;
- explicit default or named exec-group identity;
- the group-selected execution platform;
- deterministic merged platform/target/group exec properties;
- the selected group toolchain context required by the action; and
- explicit absent aspect provenance now, with a fail-closed extension point
  for later applied-aspect evidence.

Decide the exact construction seam. Current Starlark evaluation registers
intrinsic specs through `CtxActions`; `finish_analysis` separately has the
configured key, prepared toolchain selection, candidate platforms and computed
platform facts. The design must bind every accepted action to a precomputed
matching group context before the `ConfiguredNodeResult` becomes immutable.
It may finalize the registry into configured rows at that boundary, but later
`configured_file_write_actions`, aquery or REAPI may not infer platform,
properties or toolchains from the owner's current topology.

The default group is data, not `None`. Named-group lookup must fail on unknown,
missing or mismatched contexts before retaining a result. Two actions of one
owner may bind different groups/platforms/properties. A `cfg = "exec"` tool or
dependency must use the selected group's exec configuration. Preserve Bazel's
output-conflict and action-construction error precedence.

Replace, rather than duplicate, the current retained action slice where
possible. Inventory exact clone/allocation cost, `Allocative`, equality and
invalidation inputs. Retain no second action collection, topology snapshot,
resolver map, registry, evaluator, lock, cache, interner or task after analysis.
No lock may span DICE. Use the Buck2 utility-reuse review for any new retained
collection/string representation.

Freeze how `ConfiguredNodeResult`, recursive build action closure,
FileWrite semantic identity, text aquery and `FileWriteReapiPlan` consume the
same row without re-resolution. Exact public FileWrite values/order and REAPI
wire semantics remain unchanged; named-group exposure is proof/internal until
a later separately admitted command/action packet.

## Proof and future packet

The design must name exact Rust files, measured baselines, semantic/physical
caps and cohesive helper limits. Required discriminators include:

- default and named actions of one owner retain distinct group/platform/
  property/toolchain rows in declaration order;
- C0/C1/C0, platform A/B/A, property A/B/A and toolchain registration/provider
  edits invalidate and restore structural equality;
- unknown/missing/mismatched group contexts and ambiguous platforms fail
  before result retention with unchanged diagnostic precedence;
- exact configured-owner/action Arc and clone behavior; no projection-time
  topology reconstruction;
- aquery identity/text and REAPI derive from the identical retained row and
  preserve the accepted FileWrite public output;
- recursive action closure has one owner row per action with deterministic
  order and no duplicated collection;
- `Allocative`/retention cleanup and zero new Host read/cache/interner/lock/task;
  and
- applied aspects, broader action kinds and public named-group behavior remain
  nonactivated.

Classify exact Bazel semantics, Slug-native representation/identity and every
unsupported surface. End with one implementation packet or formal REPLAN.

STOP Rust/Cargo, fixture/oracle changes, a new DICE owner, dependency inversion,
projection-time reconstruction, duplicate retained graphs, guessed aspect
semantics, public named-group/action activation, rules_rust breadth, execution
backend changes, M8, M7B, M9, JVM production code, cap excess or multiple
successors.
