# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-repository-selection-validation-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `71fad142`
Rust base: `a9270586`
Accepted query design: `44c1b444`
Accepted proof correction: `e22404a8`
Result: freeze only the native repository-selection association policy, then retry the retained query candidate.

## Docs authority and blocker

Write exactly canonical/current/Stage/routing within 40/180/140/30 and 390
aggregate net-line caps. The retained eight-file candidate is non-writable.
Stop Rust, Cargo, fixtures/oracles, public changes and M1 closure.

The stable-parent root query passes, but isolated external query fails closed
with `ObservedTerminalMismatch::RepositoryRequests`. External query correctly
selects repository materialization requests/validations from its DICE closure;
the inherited path-only validator rejects every nonempty repository selection.
This is a production acceptance-boundary design miss, not a lower producer or
test issue.

## Frozen design

Add one private typed `NativeCommandRoot` association policy with two cases:

1. strict path-only selection, the default for every existing root; and
2. closure-selected repositories, overridden only by
   `RootQueryCommandObservationKey`.

The default continues to reject nonempty repository requests and validations.
The query policy permits only sidecars already selected after terminal compute
from the exact DICE activation closure. `selected_snapshot` remains the sole
owner that resolves selected repository epochs, adds validation paths, rejects
conflicting requests and constructs exact validations. Existing materializer
acceptance still consumes those exact selected requests/validations.

Regardless of policy, validate the entire observed versus selected path epoch
by length, canonical demand, semantic value and `Arc::ptr_eq`. The policy may
not skip or weaken path validation. Add no request/validation collection to
`ObservedRootQueryCommand`, no new carrier field, key, cache, store, lock, task,
Host read or event owner. The query carrier stays one Result Arc plus epoch.

Future Rust remains exactly the accepted eight-file retry authority and all
caps remain 1,154 production/1,328 tests/2,482 aggregate semantic and 19,531
physical. The hook/enum belongs in core DICE; proof belongs in the relocated
query proof and existing private strict-root tests. No new file or cap increase.

## Compatibility, proof and terminal

Exact external/root public query results, errors, events, materialization and
all strict-root behavior remain exact. The typed private selection policy is
Slug-native. Exported-source build, multi-build, one-shot query, unsupported
breadth and exact identity bytes remain deferred.

Prove an external query with nonempty selected requests and validations accepts
with exact path/result Arcs; root query keeps empty repository selection;
strict observed roots still reject both RepositoryRequests and
RepositoryValidations; cancellation/abort accepts none; warm/lifecycle/events/
families remain exact. Retain the accepted stable-parent and loading assertion
corrections.

After independent design ACCEPT schedule exactly the same
`WP-2A-m1-loading-query-observed-publication-implementation-retry` with this
additional authority. STOP any extra retained state, unrestricted boolean,
weakened validation, other root opt-in, file/cap/caller/public change or M1
closure. REPLAN if exact closure association cannot fit the private hook.
