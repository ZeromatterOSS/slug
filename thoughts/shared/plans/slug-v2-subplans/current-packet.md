# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-carrier-promotion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: selected-carrier visibility audit / `a7d9ffcc`

## Goal and docs authority

Design exactly one doc-hidden Bzlmod -> core visibility surface for the accepted
canonical selected-definition observation key, carrier and opaque outer. Freeze
effective-visibility projection and an external compile smoke without changing
the private driver or adding a caller. Do not implement Rust or activate
canonical selected/generated composition.

Docs write authority is exactly the canonical plan, this packet, Stage 6 and
the orchestration routing log at net caps <=40/<=180/<=220/<=30 and <=470
aggregate. Every Rust file, test, fixture, oracle, Cargo/BUILD target, API,
export and caller is read-only.

## Learned visibility frontier

Commit `a7d9ffcc` accepts the callerless private selected owner in
`selected_repo_spec.rs:2386-2606`. Its observation key and `new` are private.
Its private carrier's `result()` returns private alias
`SelectedDefinitionResult`; its private outer enum directly exposes private
`HostSelectedModuleRoutesObservationError`. Bzlmod's crate root exports only
the legacy selected key/value/error/view surface. Core therefore cannot name
the observed Key Value, carrier or outer.

`HostCanonicalRepositoryDefinitionKey` in core is the sole future semantic
consumer. It currently imports and computes only the legacy selected key at
`generated_repository_definition.rs:21,523`, then computes generated only
after selected Missing at 562. The accepted generated observation key/carrier/
outer is already private in this same core module, so it needs no promotion.

Canonical definition has exactly two production upper consumers: non-root
apparent mapping at generated-definition line 799 and root apparent definition
at line 310 of its module. Root route/source/public/command/bootstrap are later,
and source/command scans contain no direct selected/generated observation
consumer. Core depends one way on Bzlmod; a hidden Bzlmod -> core carrier is the
natural boundary. Moving the owner, reverse dependency, canonical activation or
an adapter key is larger than promotion.

## Decisions to freeze

Promote exactly these three nominal types as `#[doc(hidden)] pub`, and design
exactly three crate-root reexports:

1. existing `HostCanonicalSelectedModuleDefinitionObservationKey` plus public
   `new(NormalizedAbsolutePath, CanonicalRepoName) -> Self`;
2. existing `ObservedHostCanonicalSelectedModuleDefinition` plus public
   borrowed concrete `result()` returning
   `&Arc<Result<HostCanonicalSelectedModuleDefinition,
   HostCanonicalSelectedModuleDefinitionError>>` and public
   `observations() -> &PathObservationEpoch`; and
3. a public field-private opaque
   `HostCanonicalSelectedModuleDefinitionObservationError`.

Keep every field and `SelectedDefinitionResult` private. Preserve exact
derives, workspace/canonical identity, Complete-only equality/validity and
Display. Add no Result/outcome alias, terminal inspector, wrapper constructor,
conversion, fourth reexport, adapter or caller.

Rust effective visibility requires the established projection pattern. Design
must rename the current private outer enum to
`CanonicalSelectedModuleDefinitionObservationError`, preserve its sole
`Routes(HostSelectedModuleRoutesObservationError)` variant and derives, and
keep the private driver on it. The public nominal outer is a tuple struct with
one private inner field and matching derives. Only the observed key projection
wraps `Complete(Err(inner))`; Need and success remain unchanged. There is no
unwrap path: the later core owner must carry this outer without inspection.

Freeze one new external test
`canonical_selected_definition_observation_surface_is_cross_crate_usable` in
`tests/canonical_selected_definition_observation_api.rs`. It:

- constructs only the key for `/workspace` and canonical `dep+`, asserting
  `observed-host-canonical-selected-module-definition:"/workspace":@@dep+`;
- type-checks the exact associated Key Value as
  `SourcePreparationOutcome<Result<
  ObservedHostCanonicalSelectedModuleDefinition,
  HostCanonicalSelectedModuleDefinitionObservationError>>`; and
- type-checks both borrowed carrier accessors against the exact public Result
  Arc and epoch types through nonexecuted function pointers.

The smoke cannot construct a carrier/outer, inspect a terminal, compute the key,
name private aliases/routes types, import core or activate any semantic owner.
Reuse the accepted private owner proof and the two existing Bzlmod observation
API smokes; visibility has no Bazel-visible oracle gap.

## Prospective boundary, validation and terminal

Prospective authority is exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 12,524 physical
  with tests at 4,668, <=80 production and <=40 colocated proof;
- `app/slug_bzlmod_v2/src/lib.rs`, baseline 415 physical and <=10 semantic;
- new
  `app/slug_bzlmod_v2/tests/canonical_selected_definition_observation_api.rs`,
  <=70 proof.

Aggregate semantic authority is <=200 and physical caps are 12,645/425/70.
Allow only a semantic-neutral wrapper adjustment to the existing
`observed_canonical_selected_definition_identity_scan_and_terminal_algebra`
under 200 lines; every new smoke/helper remains below 100. The large selected
file remains cohesive because it owns the inner driver/carrier/projection;
splitting a visibility-only wrapper would expose or duplicate private state.
No hot-path measurement is warranted.

Prospective serial validation is focused observed-selected owner tests, the new
external smoke, existing definition/evaluation observation API smokes, full
`cargo test -p slug_bzlmod_v2`, direct dependent
`cargo check -p slug_core_v2`, formatting and `git diff --check`.

Existing selected values/errors/dispositions/scan/order/views/equality/
invalidation and events remain exact Bazel 9 compatibility. The doc-hidden
cross-crate key/carrier/opaque outer and Result-Arc/transaction-local epoch are
Slug-native. Canonical/generated observation composition, root/publication/
command/bootstrap activation and exact Bazel configuration/output/ActionKey
bytes remain unsupported/deferred.

Design ACCEPT may schedule exactly one visibility-only implementation, then
return only to a docs-only canonical selected/generated owner design. STOP
implementation in this packet, semantic compute or core edit, public field/
alias/terminal, fourth type/reexport, second key/carrier/adapter, reverse
dependency, selected semantics/event/equality/retention change, Cargo/BUILD,
fixture/oracle, third production file, cap waiver, upper activation, milestone
closure, M8/M7B or identity-byte work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Commit `a7d9ffcc` changed only `selected_repo_spec.rs` at +909/-72 and
accepted all private semantic, event, retention and lifecycle proof. This
packet cannot reopen it beyond visibility projection.
