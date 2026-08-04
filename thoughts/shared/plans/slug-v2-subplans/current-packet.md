# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-external-package-policy-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only route-aware external package-policy and lookup design
Evidence: accepted nonroot package-policy and repository path-state oracles;
the live sparse path producer/retry owner; accepted root package-policy,
REPO/ignore, lookup, and include-horizon owners; and the accepted direct route,
source, and inspection handoff.

Do not edit Rust. Design the smallest private route-keyed package-policy and
package-lookup boundary required before any direct-local included-fragment
read. Its semantic identity must retain the accepted `RootRepositoryRoute`
plus canonical external `PackageIdentifier`; raw include spelling/span remain
adapter diagnostics and do not enter package-key identity.

Freeze the exact dependency order: validate the package and check global
canonical deleted-package membership first; only a non-deleted package may
obtain route-local materialization/path authority, read/evaluate that
repository's `REPO.bazel`, read its `.bazelignore`, and select `BUILD.bazel`
before `BUILD` by resolved regular-or-special kind without reading marker
bytes. It must propagate exact `SourcePreparationNeeds`, retain typed route/
materialization/path/policy failures, use complete-only equality, and own
marker-conditional evaluation data only where Bazel evaluates policy.

Decide whether existing root policy projections can be split to expose only
global canonical deletion inputs or require a new minimal projection; root
package roots, vendor policy, root `REPO.bazel`, and root `.bazelignore` must
not leak into external equality. Decide how the existing external package-load
path will later consume the same owner so an include-only parallel policy graph
cannot land. Freeze the exact implementation packet split, Rust file allowlist,
production/test/total caps, error/event boundary, and lifecycle matrix for
BUILD priority and create/delete/recovery, route retarget, symlink/error,
deleted-package, `.bazelignore`, and `REPO.bazel` changes. Deleted-package
evidence must prove the short circuit requests no route materialization or
route-local policy Need and stores/emits no route-local policy event.

Keep the serial successors explicit: route-aware package horizon, occurrence-
preserving closure acquisition, then the empty-key evaluator correction and
nonregistry event producer. Stop with **REPLAN** on root package lookup reused
as external policy, BUILD-marker byte reads, direct filesystem IO, a second
materialization/path owner, include-fragment acquisition, cycle rejection,
evaluator changes, contextual mappings, registry/JVM transport, public
activation/export, or any new oracle without a demonstrated discriminator.
Do not edit/format Rust, run Cargo/Bazel, or change a fixture in this design
packet.
