# Current Slug V2 Packet

Packet: `WP-6-7A-host-selected-extension-definition-load-requests-observation-carrier-promotion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `e82057f2`
Accepted predecessor: `e82057f2`

## Design authority

This packet is docs-only. Write authority is exactly the canonical plan, this
manifest, the Stage 6 owner plan and the orchestration routing log, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
Cargo/BUILD metadata, APIs, exports and callers are read-only.

The ordinary terminal rollover changes canonical/current/Stage only. Routing
remains unchanged unless the design reaches formal `REPLAN` or records a
reusable routing lesson.

## Audited decision and source facts

The accepted observed definition-load-request key, carrier and typed outer are
private to `app/slug_bzlmod_v2/src/selected_repo_spec.rs`. The crate root
already doc-hidden reexports the legacy request value, error and key.
`slug_loading_v2` already depends one way on Bzlmod and imports that legacy
surface; Bzlmod does not and must not depend on loading.

`HostLoadedModuleExtensionDefinitionsKey` is the eventual natural owner in
loading. It consumes ordered requests and, per request, parses the root Bzl
target, computes the Host Bzl module, selects the named export and projects the
heap-independent module-extension definition. Its sole direct semantic
consumer is prepared inputs. The accepted crate-local
`HostBzlModuleObservationKey` already owns the complete source/recursive-load
epoch and its local `EventBatch`.

The audit therefore selects one visibility-only prerequisite. Loading cannot
compute the observed request child, inspect its Result/epoch or preserve its
typed outer until Bzlmod exposes a minimum doc-hidden carrier. Moving the
loaded-definition owner into Bzlmod would reverse ownership and dependency.
Combining promotion with loaded-definition implementation would cross two
independently reviewable boundaries.

## Design question

Freeze the minimum Bzlmod -> loading API for the already accepted observed
definition-request owner. Determine exactly:

- the doc-hidden constructible observed key and its stable identity;
- the doc-hidden observed carrier accessors needed to borrow/dupe the exact
  local request Result Arc and cumulative `PathObservationEpoch`;
- one opaque doc-hidden typed observation error usable in loading without
  exporting extension-mapping observation internals;
- the exact crate-root reexports and visibility changes; and
- the smallest compile/proof gate demonstrating cross-crate usability while
  preserving every accepted private behavior.

Prefer a public opaque wrapper with a private internal kind if the existing
private error enum would otherwise expose the mapping-observation graph. Do not
make private fields public when constructors/accessors suffice. Do not export
the evaluation-input observation carrier: loaded definitions does not consume
it.

## Frozen semantics and evidence

This is a visibility design, not a new owner. Preserve the accepted observed
request computation exactly: mappings first; Complete forwards the exact
request Result Arc and epoch; Need and typed outer are carrierless; mapping
compute becomes the existing empty-prefix semantic request error; mapping
semantic and pure request terminals retain the accepted prefix. The parent is
eventless, warm reuse is silent, poll-drop publishes nothing and A -> B -> A
restores the accepted Result/epoch association.

The implementation design must reuse the existing focused and full Bzlmod
proof. Add only the smallest discriminator required to prove the doc-hidden
surface is usable from the dependent loading crate. Do not add an oracle: the
promotion changes no Bazel-visible value, order, error, event or identity.

Exact compatibility remains the existing request semantic values, errors,
ordering and lower child events. The hidden observation key/API/carrier,
including key identity and Result/epoch association, opaque typed outer and
shared-Arc epoch are Slug-native and remain unchanged. Loaded definitions and
every upper semantic owner remain deferred.

## Later-owner handoff (non-authority)

After promotion acceptance, a separate loaded-definition design may add one
matching Legacy/Observed loading owner. It must preserve request -> per-request
label -> Host Bzl module -> export -> wrong-kind/success order; merge the
request epoch and each Complete Bzl epoch left-first before the corresponding
semantics; retain earliest duplicate Arcs; and stop without scanning later
requests or unioning Need.

That later parent remains eventless while observed Bzl children own their
ordered batches. Only its local Result Arc, cumulative epoch and semantic
request/manifest/definition state may be retained. Child carriers, frozen
handles beyond existing ownership, merge/export/event scratch, maps, caches,
locks and tasks stay compute-local or forbidden. This paragraph authorizes no
loading change.

## Terminal and stops

Reach exactly one independently reviewed terminal:

1. one minimum carrier-promotion design authorizing at most one implementation
   successor; or
2. formal `REPLAN` proving that cross-crate use requires a new semantic adapter
   or an unbounded transitive API and naming one next packet.

STOP Rust/test/fixture/oracle/Cargo/BUILD changes; loaded-definition design or
implementation; a new adapter/key/owner; reverse crate dependency; public
fields where accessors suffice; mapping-observation internals; evaluation-input
carrier export; caller or upper activation; generic graph/route/mapping APIs;
event movement; proof/cap waiver; milestone closure; M8/M7B or exact identity-
byte work. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

The audit activated by `52df2e5c` over accepted Rust `e82057f2` establishes the
natural loading owner and selects this uniquely smaller seam. The observed
request carrier is
complete and already proven but private to Bzlmod; loading's accepted observed
Bzl carrier is already crate-local. Prepared/pure/instantiated/validated,
root-mapping, generated and public owners are later or parallel consumers.
