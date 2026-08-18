# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-all-build-publication-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted implementation: `95002997`
Scheduling commit: `2f496c3a`
Result: freeze the bounded native publication consumer of the observed
singleton root-package-all build carrier.

## Selected natural owner

The private generic `NativeCommandRoot` boundary is the smallest complete
owner. Its native driver selects the exact terminal closure, prepares the
selected snapshot and then owns finalization/commit. The sole public
`WorkspaceRuntime::build_command_with_bzlmod_inputs` constructor already owns
admission of the command identity. A private consuming projection on
`AcceptedCommand` is the minimum seam that can preserve its event buffer while
removing the observation carrier from the public result.

This cutover admits only the existing observed-key constructor's structurally
exact singleton root-repository `TargetPattern::PackageAll`. Empty, Starlark,
exported, multi-target, external and cquery identities keep their identical
legacy path. No smaller prerequisite or `REPLAN` is required.

## Frozen root and terminal contract

Implement the private `NativeCommandRoot` trait for
`BuildCommandRootObservationKey` with `ObservedBuildCommandRoot` as its
terminal. Add a private default optional `observations(&Terminal)` hook to the
generic trait and override it only for this observed sibling. The shared native
driver remains the single attempt/selection/acceptance owner; neither legacy
nor observed root computes the other.

Observed Need remains Need. `Complete(Ok(carrier))` is the only successful
observed terminal. `Complete(Err(observed_outer))` becomes the existing typed
native computation/session failure and aborts the attempt. Semantic build
errors remain inside the carrier's semantic Result Arc. The observed sibling
does not add a request revision, source certificate, unavailable-root or
empty-root relaxation.

## Frozen selected-epoch validation

Change only selected path-epoch construction in `selected_snapshot` to
`PathObservationEpoch::from_shared`, preserving the already selected exact
result Arcs in canonical `SortedMap` demand order. Canonical epoch order is not
producer execution order; the accepted observed root separately proves
anchor-then-package execution and union order.

Immediately after successful `prepare_accept`, while the selected updater is
still uncommitted and before request-revision finalization, revision
finalization or `commit_prepared_native_demand_snapshot`, compare the complete
selected path epoch with the terminal's optional observed epoch. Require equal
length, identical canonical demand sequence, semantic Result equality and
`Arc::ptr_eq` for every result. Also require empty selected repository request
and repository validation sets for this bounded path. Missing, extra,
value-unequal or pointer-distinct observations fail closed as a typed session
error, never a panic/assert, and publish no snapshot or event.

No Host read, reconstructed observation, second store or partial certificate is
authorized. On failure, token drop plus the existing guard abort restores the
previous accepted state.

## Frozen public projection and retention

Add a private, infallible, consuming `AcceptedCommand::map_terminal`. It moves
the carrier's exact semantic Result Arc into the existing public
`AcceptedCommand<Arc<Result<BuildCommandEvaluation, BuildCommandError>>>`,
preserves the accepted event buffer unchanged and drops the carrier epoch after
successful acceptance.

The public constructor selects the observed sibling only after existing
constructor validation admits exactly singleton root-repository `PackageAll`.
Every other identity constructs the same legacy key and uses the same legacy
driver. The public accepted value retains only its semantic Result Arc and
event buffer; the accepted native snapshot retains only its existing compact
Arc-backed path epoch. Attempt values, validation scratch and the carrier are
local. Add no lock, task, cache, interner, collection, graph, event owner or
Host read.

## Required implementation proof

Require discriminating tests for successful and semantic-prefix exact-Arc
equality; equal-valued but pointer-distinct, missing, extra and value mismatch;
Need and typed observed outer retry/abort; cancellation and
selection/injection/materializer failure with no publication; unchanged child
event order and warm replay; warm/edit/delete/recreate/A-B-A; public singleton
observed identity and non-singleton legacy isolation; empty repository
selection scopes; and post-return retention of only the existing accepted
compact epoch plus public semantic Arc/events.

Existing package-pattern evidence is sufficient. No fixture or oracle is
authorized.

## Compatibility boundary

Existing singleton package/output/event behavior remains exact. Carrier versus
selected-snapshot association, exact shared-Arc validation and fail-closed
observed outer errors are Slug-native. Analyzed/exported/multi-target/external/
cquery publication, repository/materializer breadth, native-Windows raw-byte
ordering and exact Bazel identity bytes remain unsupported/deferred.

## Authority and caps

This design packet writes only this manifest and
`thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.
Against `95002997`, the docs caps remain 200 manifest, 120 Stage 2 and 300
aggregate net lines. Require `git diff --check` and independent reserved-design
review.

The future implementation writes only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/events.rs`.

Against `95002997`, future caps are 150 production plus 300 test net lines and
14,151 physical lines for `dice.rs`; 16 production plus 30 test net lines and
1,640 physical lines for `events.rs`; and 496 aggregate net Rust lines.

## STOP / REPLAN

STOP on any other file; Cargo, fixture or oracle writes; broader caller or
repository/materializer activation; partial epoch validation; changed public
terminal, output, event, retry, selection, publication or restoration behavior;
direct/reconstructed Host reads; another retained structure/owner; public API;
or cap excess.

`REPLAN` if exact selected Arcs cannot survive preparation, complete validation
cannot precede irreversible acceptance, event-preserving projection needs a
public/fallible API, repository selection is nonempty, or the generic driver
cannot keep every non-admitted identity on the unchanged legacy path.

## Immediate successor

On independent acceptance schedule exactly one bounded singleton publication
implementation using `95002997` plus the accepted design commit. Do not combine
another build/cquery/repository frontier or milestone close.
