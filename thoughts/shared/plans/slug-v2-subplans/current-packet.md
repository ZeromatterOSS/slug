# Current Work Packet

Packet: `WP-6-7A-action-owner-context-absence-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Type: docs-only design correction
Scheduling base: `460dea72`
Rust base: `51127df8`
Failed semantic design: `460dea72`
Owner: `06-analysis-toolchains-and-actions.md`

Result: formally REPLAN the retained immutable configured-action candidate,
freeze the missing no-required-toolchain execution-context state, then resume
the same implementation. This packet authorizes no Rust or oracle write.

## Exact authority and budgets

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md` <=40 net;
- this manifest <=200 net;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
  <=160 net; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` <=30 net.

Aggregate docs net <=430. Retain the dirty eleven-file Rust candidate exactly
as found and treat it as non-writable throughout this design packet.

## Formal REPLAN evidence

The accepted design requires every configured action row to point to a context
containing a concrete `ToolchainSelection` and selected `ToolchainInfo` marker.
The live implementation proves that shape is not total for existing exact
behavior:

- ordinary action-producing rules without a required toolchain return
  `Ok(None)` from `root_rule_execution_platforms` and construct no
  `PreparedToolchain`;
- a selected toolchain implementation can itself register actions while its
  own analysis key has no required toolchain; and
- `evaluate_loaded_rule` consequently passes zero contexts, so the finalizer
  rejects existing FileWrite/executable actions with `configured action has no
  matching exec-group context`.

Full `slug_analysis_v2` validation discriminates the regression with six
existing failures. Fabricating a selection, marker, label, platform or
configuration would invent semantic identity and is forbidden. Making those
tests expect failure would silently regress an accepted exact slice.

The candidate also leaves `resolve_root_toolchain` above the frozen 200-line
touched-helper limit. That is a bounded cohesion correction in the same owner,
not grounds for a new DICE producer.

## Frozen correction

Keep one immutable configured-action row and one shared owner-context Arc per
owner/group, but make the execution state explicit rather than synthesizing a
toolchain:

1. `SelectedToolchain` retains the already designed selected Platform fact,
   ordered constraints and exact compact `ToolchainSelection` plus marker.
2. `SelectedPlatformOnly` retains the selected Platform fact and ordered
   constraints with an explicit absent-toolchain state. It is available only
   where the existing topology has exactly one candidate execution platform;
   analyze that Platform through the matching legacy/observed family before
   rule evaluation. Need/typed outer/semantic failure suppresses evaluation.
3. `UnresolvedDefault` is an explicit Slug-native compatibility state for an
   existing action owner with neither a toolchain requirement nor a uniquely
   selected execution platform. It retains no guessed platform, constraints,
   toolchain marker or execution properties.

All states retain the structural configured owner, explicit Default/Named
group and absent-aspect provenance. Equality/invalidation includes the state.
Intrinsic `ActionSpec` value and registration order remain exact in every
state. Production still passes exactly one Default context; it may be selected
or explicitly unresolved, never missing. Named contexts remain private proof.

The finalizer still moves each `ActionSpec` into exactly one configured row,
shares the matching context Arc and retains no lookup map. Missing/duplicate/
wrong-owner/named mismatches remain errors. `ConfiguredActionView` exposes the
state without reconstruction. Platform/toolchain access is optional at the
type boundary; FileWrite semantic resolution requires a selected platform and
therefore preserves the previous behavior: a uniquely selected candidate is
usable, while an unresolved/ambiguous action remains an exact intrinsic action
but cannot be projected as a configured FileWrite.

Split the oversized root-toolchain routine into bounded helpers for selected
Platform analysis/constraint projection, selected implementation projection,
and orchestration. Each touched helper must remain below 200 lines. Add no key,
store, cache, interner, lock, task, direct Host read or retained child Result
Arc. Selected Platform and implementation Result Arcs remain compute-local.

All other contracts in `460dea72` remain frozen: matching-family DICE
selection; Platform-before-implementation order; exact selected properties;
toolchain marker sharing; action finalization after evaluator/provider checks;
no topology reconstruction in core; unchanged aquery/REAPI production; one
configured-action slice only; and no public named-group/aspect breadth.

## Retry authority and caps

After independent ACCEPT, schedule exactly
`WP-6-7A-immutable-configured-action-owner-context-implementation-retry` with
the same eleven Rust files and no twelfth file. Correct only these caps:

- `slug_analysis_v2/src/result.rs`: <=300 production, <=730 physical;
- original remaining per-file caps stay unchanged.

The corrected aggregate is <=848 production, <=860 tests, <=1,708 semantic
and <=25,895 physical. The retained candidate is charged in full against the
retry. STOP/REPLAN again on any additional file or cap increase.

## Required proof and compatibility

Add discriminating proof for selected-toolchain, selected-platform-only and
unresolved-default equality/restoration; exact intrinsic zero-toolchain action
parity; single-candidate projection parity; unresolved projection rejection;
Platform Need/typed outer/semantic suppression before implementation/rule;
matching-family isolation; exact context Arc sharing; properties/constraints/
marker pointer behavior; identity/aquery/REAPI selected-path parity; and no
retained topology/result reconstruction. Full analysis/core/REAPI validation
must include the previously failing six tests.

Exact Bazel 9.2 compatibility remains existing ActionSpec values/order,
selected-platform/toolchain semantics, configured FileWrite identity/aquery/
REAPI output and existing diagnostics. Slug-native is the explicit immutable
execution-state enum and unresolved compatibility row. Unsupported/deferred
remains public exec groups, action `exec_group=`, target/group property
admission, applied aspects, broader action kinds/rules_rust and exact Bazel
identity bytes.

STOP this packet on Rust/Cargo/BUILD/fixture/oracle/public writes, fabricated
selection data, another owner/key/file, M7A/M8/M7B/M9 closure or direct retry.
After correction-design ACCEPT, the retry is the sole successor.
