# Current Slug V2 Packet

Packet: `WP-6-7A-immutable-configured-action-owner-context-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `11934418`
Evidence base: `8eecf172`
Semantic design: `460dea72`
Absence correction: `11934418`
Accepted Rust base: `51127df8`
Result: implement the first immutable analysis-owned configured-action row and
migrate the accepted FileWrite semantic consumers to it without public breadth.

## Exact authority and caps

Write exactly:

1. `app/slug_analysis_v2/src/result.rs`: <=300 production net, <=730 physical;
2. `app/slug_analysis_v2/src/dice.rs`: <=260 production, <=3,000 physical;
3. `app/slug_analysis_v2/src/starlark_rule.rs`: <=80 production, <=680 physical;
4. `app/slug_analysis_v2/src/lib.rs`: <=12 production, <=75 physical;
5. `app/slug_analysis_v2/tests/configured_target.rs`: <=300 test, <=850 physical;
6. `app/slug_analysis_v2/tests/starlark_rule.rs`: <=220 test, <=3,500 physical;
7. `app/slug_core_v2/src/runtime/dice.rs`: <=140 production, <=11,850 physical;
8. `app/slug_core_v2/src/runtime/file_write_identity.rs`: <=50 production,
   <=300 physical;
9. `app/slug_core_v2/src/runtime/mod.rs`: <=6 production, <=260 physical;
10. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=240 test,
    <=4,150 physical; and
11. `app/slug_reapi_v2/tests/reapi.rs`: <=100 test, <=500 physical.

Production semantic cap <=848, test cap <=860, aggregate <=1,708 and combined
physical <=25,895. `dice.rs`, the existing Starlark-rule proof and the core
build proof are cohesive owner/proof exceptions; every touched/new helper must
remain below 200 lines. No twelfth file.

## Frozen owner and representation

Preserve `slug_build_api_v2::ActionSpec` as intrinsic action material. In
`slug_analysis_v2`, add immutable `Allocative`, cloneable/equatable configured
action types with this structural content:

- `ConfiguredActionExecGroup` is explicit `Default` or a compact named group;
- `ConfiguredActionAspectProvenance` has only the explicit `Absent` state in
  this packet; no applied-aspect value may be guessed;
- `ConfiguredActionPlatformConstraint` retains the configured constraint-value
  and setting keys in platform declaration order;
- one compact admitted toolchain context retains the exact `ToolchainSelection`
  plus the selected `ToolchainInfo` marker/provider projection, never its full
  analysis/provider Result Arc; and
- one shared `ConfiguredActionOwnerContext` retains the configured target
  owner, group, explicit execution state, and absent aspect provenance; its
  execution state is `SelectedToolchain` with platform/properties/constraints
  and the compact toolchain, `SelectedPlatformOnly` with the same platform
  projection and an explicit absent toolchain, or `UnresolvedDefault` with no
  fabricated platform/properties/constraints/selection/marker; and
- each `ConfiguredAction` owns one intrinsic `ActionSpec` plus an `Arc` to its
  matching immutable context. Actions of the same owner/group share that Arc.

Replace `ConfiguredNodeResult.actions: Arc<[ActionSpec]>` with exactly one
`Arc<[ConfiguredAction]>`; do not retain the old slice. The evaluation-time
`Vec<ActionSpec>` must be moved into configured rows. Retain no context lookup
map after finalization and no platform/toolchain `ConfiguredNodeResult` Arc.
The existing `ToolchainTopology` remains only as the already accepted
rule/native analysis fact and candidate-edge witness; action consumers must not
read it, and this packet adds no second topology/candidate collection.

Configured owner identity already includes the complete structural Slug
configuration. In either selected state, the platform and all constraint keys must carry the
same structural exec configuration. The toolchain selection's execution
platform must equal the context platform; its declaration/type/implementation
remain structurally retained. Equality and invalidation include every field.

Merge execution properties deterministically in platform, target, then group
override order into one sorted unique compact slice. The admitted production
surface supplies platform properties and empty target/group overlays; preserve
the platform fact's exact inner Arc when both overlays are empty. The helper
and pure tests freeze later target/group precedence, but loading/public
target/group-property admission remains deferred. Per-action properties remain
intrinsic `ActionSpec` data and are not silently folded into owner context.

## Creation seam and failure order

Extend the existing mode-aware root toolchain preparation; add no DICE key.
After raw package-based platform/toolchain selection, but before computing the
selected toolchain implementation, compute selected Platform analysis through
the matching legacy/observed family. Validate its exact configured key,
Platform kind, diagnostics, normalized `PlatformSemanticFact`, and ordered
constraint edges. Build the constraint setting keys from the already loaded,
DICE-owned native toolchain packages and the selected platform's exec
configuration. Forward the exact properties Arc; drop the selected Platform
result after the context is built. A Platform Need/typed outer/semantic error
suppresses both selected-implementation analysis and rule evaluation.

For a required toolchain, only after Platform success compute and validate the selected toolchain
implementation as today. Project its exact `ToolchainInfo` marker together
with the selected labels/platform into the compact toolchain context.
`PreparedToolchain` and Starlark `ctx.toolchains` must borrow/share that compact
context; move the same context Arc into configured action rows after evaluation
rather than cloning a second marker or retaining the provider/result Arc.

Where the existing topology supplies exactly one candidate but no required
toolchain, analyze that Platform through the matching family and construct
`SelectedPlatformOnly`; it retains no guessed toolchain binding. Where no
candidate is selected, construct `UnresolvedDefault` without computing or
guessing a Platform. This preserves existing intrinsic zero-toolchain actions
and the former sole-candidate projection boundary.

Pass one compute-local prepared default-group context into
`evaluate_loaded_rule`. Starlark still registers intrinsic actions in
declaration order through `CtxActions`. After provider and action-registry
validation succeeds, but before constructing `ConfiguredNodeResult`, move each
spec through one pure finalizer:

1. `None` group means explicit `Default`; a string means that exact named key;
2. select exactly one matching context and reject missing/duplicate contexts;
3. validate owner, exec configuration, platform/toolchain agreement, sorted
   properties/constraints and explicit aspect state; then
4. produce rows in original registration order, sharing contexts by Arc.

Production passes exactly one default context, selected or explicitly
unresolved. Named contexts are a private
representation/finalizer proof only: Starlark `rule(exec_groups=...)` and
action `exec_group=` remain unactivated. Unknown/named groups therefore fail
before result retention. Existing action registration/output-conflict,
provider, executable-rule and Starlark evaluation errors retain their current
precedence because finalization occurs after those checks. Need/typed observed
outer from selected-platform analysis propagates before rule evaluation and
retains no partial result or context. Split selected-Platform projection,
selected-implementation projection and orchestration into helpers below 200
lines; the retained candidate's monolithic root-toolchain helper is not
acceptable.

## Consumer and memory contract

`ConfiguredActionView` becomes a borrowed view of the retained row. Its owner,
group, explicit execution state, optional platform/properties/constraints/
toolchain and aspect accessors must read only the row/context.
`configured_file_write_actions` retains the existing
FileWrite shape checks but performs no topology/platform reconstruction.
It requires a selected platform: sole-candidate actions remain projectable,
while unresolved/ambiguous actions remain exact intrinsic rows but preserve
the prior semantic-view rejection.

In core, reduce `ResolvedFileWriteSemanticView` to the borrowed configured
action row. Remove `unique_closure_node` platform/constraint resolution and its
temporary constraint vector. FileWrite semantic identity encodes the same
configured owner/output/Write material/group/platform/properties/constraint
grammar from the retained row; add a tagged named-group representation without
activating it publicly. Text aquery continues to reject named/non-FileWrite
surfaces and preserves its exact accepted bytes. `FileWriteReapiPlan` already
consumes the resolved view and must read the identical retained properties;
production REAPI code does not change.

The recursive action closure keeps its existing dependency-owned platform,
constraint and toolchain nodes/order through existing configured edges. It
retains one configured action slice per analysis result, not a second action or
resolved-platform graph. Registry/evaluator/prepared context maps, selected
Platform result, merge scratch and consumer iteration state stay compute-local.
Add no cache, interner, store, lock, task, direct Host read or DICE state. No
lock may span DICE.

## Required discriminating proof

- default and named rows for one owner retain distinct group/platform/property/
  toolchain contexts in declaration order; same-group rows are `Arc::ptr_eq`;
- C0/C1/C0, platform A/B/A, property A/B/A, toolchain registration/provider
  A/B/A and restored values change then restore structural equality;
- the production default row preserves the selected Platform fact's exact
  property Arc, ordered constraint keys, selected `ToolchainInfo` marker and
  configured owner/action clone behavior without a provider/result Arc;
- missing/default/named/duplicate/mismatched owner, platform configuration,
  toolchain platform, constraint configuration and unsorted property contexts
  fail before result retention; output-conflict and Starlark/provider errors
  keep their existing precedence;
- selected Platform Need/typed outer/semantic error prevents evaluation and
  selected-implementation activation and leaves no configured row;
  legacy/observed analysis families stay isolated;
- selected-toolchain, selected-platform-only and unresolved-default rows have
  discriminating equality/restoration proof; zero-toolchain intrinsic action
  value/order remains exact, sole-candidate projection remains usable, and
  unresolved projection fails without invented identity;
- a real configured FileWrite result no longer needs platform/constraint nodes
  to reconstruct its semantic view, while the recursive closure still contains
  those dependency nodes exactly once and in deterministic order;
- semantic identity/text aquery and REAPI use one borrowed retained row,
  preserve exact accepted public FileWrite output/identity/REAPI wire behavior,
  and respond to property/platform/configuration restoration;
- named-group types/finalizer paths are private proof only: zero Starlark,
  command, aquery, REAPI or execution named-group activation;
- `Allocative` and clone/Arc accounting prove one row slice plus shared compact
  contexts, no retained context map/topology duplicate/result Arc/scratch; and
- focused analysis/core/REAPI tests, full affected crates, fmt, diff-check and
  AI-cleanup/Buck2 retention review pass. Record inherited baselines exactly.

## Compatibility and terminal

Exact Bazel 9.2 for the admitted surface: existing default FileWrite values,
action declaration/closure order, configured owner/configuration, selected
platform/toolchain/property semantics, diagnostics, text aquery and REAPI wire
behavior; the accepted oracle's default/named platform/property relationships
remain the evidence boundary.

Slug-native: immutable configured-action/context types, compact Arc sharing,
explicit group/aspect/execution-state enums (including unresolved compatibility
rows), structural identity bytes, and the private named finalizer proof.

Unsupported/deferred: public `rule(exec_groups=...)`, action `exec_group=`,
target/group property ingestion, applied-aspect actions, `cfg = "exec"` tools
outside the selected-context invariant, broader action kinds/rules_rust,
execution backend breadth, and exact Bazel configuration/ActionKey bytes.

On ACCEPT schedule exactly one docs-only M7A next-owner audit. STOP on any
twelfth file, lower build-API configured type, new DICE owner, raw-action slice
retention, consumer topology reconstruction, second retained graph/map,
property/input invention, diagnostic drift, public named-group activation,
Rust/Java delegation, cap excess, partial proof or M7A/M8/M7B/M9 closure.
REPLAN instead of widening any frozen boundary.
