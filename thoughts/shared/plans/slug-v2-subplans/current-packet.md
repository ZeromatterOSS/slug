# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-all-build-publication-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted implementation: `95002997`
Result: freeze the bounded native publication consumer of the observed
singleton root-package-all build carrier.

## Audit scope

Audit only the private `NativeCommandRoot` boundary, generic native
attempt/terminal-selection owner, sole production
`WorkspaceRuntime::build_command_with_bzlmod_inputs` constructor/caller, and
the accepted-command event-preserving projection seam. Confirm that this is
the uniquely smallest complete owner above `BuildCommandRootKey`, or record one
smaller prerequisite/`REPLAN`.

The candidate slice is structurally exactly one root-repository
`TargetPattern::PackageAll` admitted by the existing observed-key constructor.
Empty, Starlark, exported, multi-target, external and cquery identities stay
legacy and unsupported/deferred for this frontier.

## Required design decisions

Freeze how the observed carrier remains live through native terminal closure
selection and acceptance while the public return type remains the existing
semantic `AcceptedCommand<Arc<Result<BuildCommandEvaluation,
BuildCommandError>>>`. Decide the minimum private, infallible consuming
projection that preserves the accepted event buffer.

Freeze carrier-versus-selected-snapshot validation immediately after terminal
selection/preparation and before any selected snapshot commit, request-revision
finalization or irreversible publication. The proof must cover the complete
selected path epoch, canonical demand order, semantic values and exact result
Arcs, plus the absence of repository requests/validation scopes. Missing,
extra, unequal or pointer-distinct observations must fail closed without
publishing a snapshot or event.

Decide whether selected snapshot construction can preserve the already shared
observation Arcs through `PathObservationEpoch::from_shared`; do not authorize
reconstructed Host reads or another observation store. Anchor-then-package
execution order remains a separate accepted driver invariant from canonical
epoch demand order.

Freeze Need retry, typed observed outer error, semantic error, cancellation,
selection/injection/materializer failure, event replay and restoration
polarity. The observed singleton has no request-revision/source-certificate,
empty-root or unavailable-root relaxation. All non-admitted callers must keep
the identical legacy driver.

## Retention and proof

The public accepted value may retain only its existing semantic Result Arc and
event buffer; the accepted native snapshot may retain only its existing compact
Arc-backed path epoch. The observed carrier is attempt/acceptance-local and its
epoch drops after successful consuming projection. Add no lock, task, cache,
interner, collection, graph, event owner or Host read.

Require discriminating successor proof for public semantic/output parity;
carrier/selected exact-Arc equality; pointer-distinct, missing, extra and value
mismatches; Need/outer/error/cancellation/failure aborts; child-event order and
warm replay; strict observed singleton and non-singleton legacy activation;
warm/edit/delete/recreate/A-B-A; and post-return retained-state shape. Reuse the
accepted package-pattern evidence; no fixture or oracle is authorized.

## Authority and caps

This packet is docs-only. Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Against `95002997`, caps are 40 canonical, 200 manifest, 120 Stage 2 and 300
aggregate net lines. Require source/ownership audit, exact compatibility
classification, bounded successor allowlist/caps, `git diff --check` and
independent reserved-design review.

## Compatibility boundary

Existing singleton package/output/event behavior remains exact. Carrier versus
selected-snapshot association, exact shared-Arc validation and fail-closed
outer errors are Slug-native. Analyzed/exported/multi-target/external/cquery
publication, repository/materializer breadth, native-Windows raw-byte ordering
and exact Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on Rust, Cargo, fixture or oracle writes; any public/caller activation;
partial epoch validation; changed terminal, output, event, retry, selection,
publication or restoration behavior; direct/reconstructed Host reads; another
retained structure/owner; repository/materializer breadth; or cap excess.

`REPLAN` if the observed carrier cannot be compared before irreversible
acceptance, event-preserving projection needs a public API, selected snapshot
identity cannot preserve the exact Arcs, the singleton path selects repository
demands, or generic native-driver changes cannot remain bounded.

## Immediate successor

On independent acceptance schedule exactly one bounded singleton publication
implementation using `95002997` plus the design commit. Do not combine another
build/cquery/repository frontier or milestone close.
