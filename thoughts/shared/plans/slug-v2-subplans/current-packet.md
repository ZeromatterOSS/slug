# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-definition-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `08524062`

## Goal and authority

Implement one private callerless observation owner for
`HostRootApparentRepositoryDefinitionKey`. Share its existing semantics through
one Legacy/Observed driver over exactly two sibling-nameable children: observed
canonical apparent mapping first, then observed canonical definition only for a
nondeferred target. Preserve exact behavior and lower event ownership while
retaining one local Result Arc plus transaction-local observation epoch.

Authority is only
`app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`, baseline
1,079 physical/tests 372 and SHA-256
`58759d7662285abf8b3debce6f0a2f64649e5c7218faf097ade1eee7e2658715`.
Every other file, export, caller, fixture/oracle and Cargo/BUILD surface is
read-only.

## Frozen owner and driver

Add private `HostRootApparentRepositoryDefinitionObservationKey` as a nominal
wrapper over the legacy key. Its `new(NormalizedAbsolutePath,
ApparentRepoName) -> Option<Self>` preserves root-name rejection. Display is
`observed-{legacy}`; `/workspace` and `@first` render exactly
`observed-host-root-apparent-repository-definition:"/workspace":@first`.

Add private `ObservedHostRootApparentRepositoryDefinition` with private local
Result-Arc and `PathObservationEpoch` fields plus borrowed accessors. Add one
private `HostRootApparentRepositoryDefinitionObservationError` with exactly:

- `Mapping(HostCanonicalRepositoryApparentMappingObservationError)`;
- `Definition { mapping: HostCanonicalRepositoryApparentMapping,
  error: HostCanonicalRepositoryDefinitionObservationError }`; and
- `Merge { mapping: HostCanonicalRepositoryApparentMapping,
  error: ObservedPathFrontierError }`.

Use one private `RootApparentRepositoryDefinitionMode::{Legacy, Observed}` and
shared driver returning Need, typed outer, or local Result Arc plus epoch.
Legacy Key delegates to it, requires an empty epoch and projects the existing
outcome. Observed Key wraps only successful local results in the carrier;
`complete_eq` and `is_complete` remain its equality/validity laws.

## Exact order, terminals and epochs

The driver order is fixed:

1. Compute canonical apparent mapping for root context first: legacy child in
   Legacy mode, promoted observed child in Observed mode.
2. Mapping Need returns carrierless Need. Observed mapping outer returns typed
   `Mapping` with no parent carrier. DICE compute failure remains the existing
   local `MappingCompute` semantic terminal with an empty epoch.
3. A completed mapping semantic error becomes the existing local `Mapping`
   terminal with the mapping epoch. A successful mapping supplies the target.
4. Main target immediately returns existing `MainDeferred`; `bazel_tools`
   immediately returns `BuiltinDeferred`. Both retain only the mapping epoch
   and never request definition.
5. Only another target computes canonical definition second. Definition Need
   is carrierless. Its observed outer becomes typed `Definition`, retaining
   the successful mapping value but no parent carrier. DICE compute failure is
   existing local `DefinitionCompute` with the mapping prefix epoch.
6. On completed definition, merge mapping observations left-first with
   definition observations right. Equal duplicate demand/results retain the
   mapping/left Arc. A valid same-demand differing-result conflict is typed
   `Merge { mapping, error }`. Parent Merge does not manufacture
   OperationMismatch; lower OperationMismatch remains inside typed Mapping or
   Definition outers. Add no malformed epoch/hook proof.
7. Only after successful merge, project the existing definition semantic
   Missing/Definition, ContextMismatch or success terminal with the full
   merged epoch. Preserve exact target/canonical/mapping-context checks and all
   existing error payloads/views.

The Result Arc is newly allocated only for the local terminal exactly as the
shared driver requires. Need and typed outers carry no carrier or epoch.

## Events, lifetime and nonactivation

The parent emits no `EvaluationEvent` or `EventBatch`. Mapping and conditional
definition children retain exact event ownership and order; mapping-only
terminals expose no definition event. Warm parent and all warm lower rows are
batchless and replay nothing.

The DICE-retained carrier owns only the local root-definition Result Arc plus
merged epoch. Child carriers/Result Arcs, target/view values, merge iterator and
event/test scratch are compute-local. Add no map, cache, interner, evaluator,
task, lock, side store or command-lifetime borrow. Overlapping transactions
remain isolated by child observation dependencies; equality cutoff includes
both local Result and epoch. Cancellation publishes neither parent activation
nor dependency row; same-DICE recovery recomputes normally.

Legacy parent, root route/source input/source observation/path input,
repository publication/materialization, public commands and bootstrap remain
inactive. No route visibility, import or caller is authorized.

## Frozen proof

Add exactly three tests:

1. `observed_root_apparent_repository_definition_identity_staging_and_terminal_algebra`
   proves key/hash/Display/root rejection/accessors/equality/validity; mapping-
   first and conditional-definition dependency rows; all mapping/definition
   semantic terminals; carrierless Need/typed outers; lawful left-Arc/equal
   merge and conflicting-result Merge; exact source order and no legacy child.
   ContextMismatch is proved only through synthetic finisher algebra using
   lawful real child values paired under a mismatched synthetic context;
   no real keyed-parent row may claim it.
2. `observed_root_apparent_repository_definition_real_order_events_and_parity`
   proves real selected-registry, selected-nonregistry, generated, Missing,
   mapping failure, MainDeferred and BuiltinDeferred families;
   exact legacy Result parity; observed mapping then conditional definition;
   mapping-only short circuit; lower event vectors and parent eventlessness;
   every warm row batchless.
3. `observed_root_apparent_repository_definition_lifecycle_cancellation_and_nonactivation`
   holds parent and both real child carriers through semantic A-B-A restoration
   and semantic-neutral source metadata changes; proves each child epoch is a
   subset of the parent and parent is a subset of its transaction global,
   equal Result/different epoch invalidation, Arc identity only on Reused,
   poll-drop/no publication, same-DICE recovery and all upper/legacy families
   inactive.

Reuse accepted lower outer/mismatch and Bazel 9.2
`BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping` and `BazelDepGraphFunctionTest`
evidence. Source/dependency proof may establish inaccessible opaque lower
outers; add no private injection, malformed hook, fixture or oracle.

## Caps and validation

Caps are <=300 production, <=720 proof, <=1,020 aggregate semantic and <=2,100
physical lines; at most six production/seven test helpers, exactly three new
tests, shared driver below 180 and every helper/test below 200. The file remains
cohesive because it already owns the legacy key/value/error/views, both child
composition, trackers and fixtures. No hot-path or retained-representation
change applies.

Run serially: the three exact observed-root-definition tests; protected
`request_shape_and_target_precedence_are_total`,
`real_generated_selected_and_deferred_domains_are_structural`,
`lifecycle_identity_and_mapping_precedence_are_structural` and both child
observation suites/smokes; full `cargo test -p slug_core_v2`; direct
`cargo check -p slug_commands_v2`; `cargo fmt --all -- --check`; then exact
one-file allowlist/SHA/accounting/physical/helper/test-size/source-shape and
`git diff --check`.

Root-definition values, target order, errors/views, equality/invalidation and
lower events remain **exact** Bazel 9 compatibility. The private Result-Arc+
epoch carrier and typed outer are **Slug-native**. Carrier visibility, route/
source/public/command/bootstrap observation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

## Terminal and stops

ACCEPT returns only to a docs-only root-definition observation carrier-
visibility/consumer-frontier audit. STOP a second file/key/owner/adapter,
visibility/export/caller, route/source compute, semantic/order/error/event/
equality/retention drift, third child/parallel join, invalid parent
OperationMismatch, malformed injection, retained child/scratch/task/lock,
fixture/oracle, cap/helper/test/format waiver, milestone closure, M8/M7B or
exact identity work. REPLAN before widening or hash drift. M7 remains partial
and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted `08524062` makes both child observation surfaces sibling-nameable and
keeps the legacy root owner and sole upper route consumer unchanged.
