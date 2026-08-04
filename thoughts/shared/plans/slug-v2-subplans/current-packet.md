# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-route-package-horizon-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only route-aware breadth-first package-horizon design
Evidence: accepted direct-local route/source/inspection owners in `e5e2c55d`
and `8aae11d6`; the private `ExternalRepositoryPackageLookupKey` in
`42ef64cd`; the accepted selected-source/loading migration in `9b5246af`; the
existing root horizon batching shape; and pinned Bazel 9.2 include-horizon
ordering. Add no new oracle unless the design proves a missing discriminator.

Do not edit or format Rust. Design one private breadth-first package-preflight
adapter over the accepted `DirectLocalModuleInspection` route and ordered
`NonrootIncludeRequest` occurrences. Decide whether the bounded owner is a DICE
key over the accepted inspection or a reusable crate-private horizon helper
plus one private key, without adding a second inspection, route, package-policy,
or lookup graph. Freeze the exact implementation file allowlist and measured
production/test/total caps; the live likely seam is `source_preparation.rs`
plus `host_include.rs`, but that observation is not implementation authority.

The design must retain the route and parse every occurrence with its raw label
and `LogicalSpan` diagnostic provenance. Derive canonical
`PackageIdentifier`s from `route.canonical_repo()`, deduplicate only package
dependency requests within the current horizon in deterministic first-seen
order, and compute the private `ExternalRepositoryPackageLookupKey` once per
unique package. The complete value preserves every occurrence and source order,
including duplicates; dependency deduplication must not deduplicate execution
occurrences or erase raw diagnostics.

Freeze the exact terminal order. Reject the first malformed include in source
order before package requests. After parsing succeeds, request all unique
package dependencies before selecting any terminal package result. Union and
forward every `SourcePreparationNeeds` from the inspection and package lookups;
if any package is incomplete, return the whole horizon Need before a complete
package error. Once all packages are complete, walk original occurrence order
and select the first `InvalidPackageName`, `Deleted`, `NoBuildFile`, typed
operational lookup error, or lookup-compute error, restoring that occurrence's
raw label/span. Only a horizon with every package successful returns its ordered
occurrence value. No path/source/`FileBytes` request for an included fragment is
allowed in this packet or its implementation.

Specify exact key/value/error identity, display, source chains, and complete-
only equality. The value must retain only route identity plus ordered parsed
occurrences needed by the later fragment-read packet, using existing compact/
shared carriers; it must not retain duplicate lookup results, event batches,
fragment bytes, evaluation state, or public activation data. Every transient
Need is invalid and self-unequal. The horizon owns no local event batch and
does not copy child policy batches; routed REPO events remain child-owned and
are selected only through DICE activation.

Freeze lifecycle evidence for duplicate-package single request versus repeated
ordered occurrences; first-error source order independent of dependency
completion order; exact unioned Needs; raw-label/span diagnostics; DirectLocal
MODULE include reorder/edit/absence/recreate; package deletion/ignore/REPO and
BUILD-marker create/delete/recovery; route A-to-B-to-A; warm reuse; and
captured/uncaptured child policy events with no horizon-local data.

Stops: no Rust, fragment path or byte reads, occurrence-preserving recursive
closure, include-cycle behavior, module compilation/evaluation, empty-key
defaults/validation/print changes, contextual mappings, registry/MVS/JVM
transport, public export/caller/activation, root-horizon semantic change,
direct filesystem IO, fixture/oracle, or speculative implementation cap.
Finish by recording an accepted bounded implementation packet or `REPLAN` in
the owner/canonical/manifest/routing records. Do not run Cargo or Bazel.
