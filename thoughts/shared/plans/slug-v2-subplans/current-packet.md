# Current Slug V2 Packet

Packet: `WP-5-host-discovered-module-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze one Host per-module discovery/evaluation value for the embedded
and registry-backed module categories before selected-graph ownership.

## Accepted predecessor boundary

Commit `3bc745de` accepts the exact callerless embedded MODULE value. Commit
`b875e22a` accepts the selected-graph owner design, whose source audit ends
`REPLAN`: Slug cannot build the sole discovery-to-MVS graph while its
embedded, registry, and nonregistry paths expose different semantic values and
provenance.

Pinned Bazel 9.2 finalizes a requested MODULE into a complete InterimModule
before discovery retains it. Discovery owns override rewriting and the chosen
registry/nonregistry source; MVS consumes the recursively discovered modules.
Only the later selected graph may derive canonical/full mappings, extension
unique names, registrations, RepoSpecs, selected-yanked/hashes, or lockfile
state.

The live Slug boundary is narrower. `BuiltinBazelToolsModuleKey` returns a
complete module with immutable route/hash provenance. `ModuleSourcePreparationKey`
returns registry bytes, selected registry, and ordered URL/SHA-or-absence
attempts, but does not evaluate them. `DirectLocalModuleEvaluationKey` returns
a complete module and route only for a main-repository-visible direct
`local_path_override`; its route owner currently reserves `bazel_tools`
before override routing. It cannot represent arbitrary transitive
nonregistry, archive/Git, or explicit built-in override discovery. Normalized
command `--override_module` state is also absent. Reusing the legacy
handwritten `ResolvedGraph` remains forbidden.

## Accepted design contract

Freeze one crate-private Host discovered-module key/value that joins the
complete evaluator with typed source provenance. Its key identity is a
normalized workspace plus exact `NonrootModuleKey`. It computes root files
first so root Need/error ordering and explicit override category are known
before any embedded lookup.

The admitted successful categories are:

- `bazel_tools@<empty>` with no explicit override, computed from
  `BuiltinBazelToolsModuleKey(BuiltinBazelToolsSnapshot::CURRENT)`; and
- a versioned registry module computed from `ModuleSourcePreparationKey`,
  retaining the selected `RegistryBaseUrl`, every ordered
  `RegistryModuleFileAttempt`, and the complete evaluated
  `EvaluatedNonrootModule`.

Registry bytes use the existing complete nonregistry evaluator as Bazel's
restricted MODULE evaluator. The logical source identity is derived from the
selected registry/module request, not a Host materialization path. Registry
`include()` remains an exact validation failure through the evaluator and
must not request an include file. Evaluation events follow the existing
capture/no-capture transaction contract.

The value pairs the complete evaluated module with exactly one provenance
variant: built-in route identity plus exact MODULE SHA-256, or selected
registry plus ordered attempts. Equality includes the full module and
semantic provenance; request generations remain DICE dependencies rather than
retained semantic fields. `Need` is invalid; complete values and typed errors
are stable.

Fail closed before embedded computation for any explicit `bazel_tools`
override. Also fail closed for every nonregistry override, empty-version
non-built-in request, absent command-override normalization, direct-local
route, archive/Git source, and unsupported include shape. These are
unsupported/deferred, not successful discovery claims. The design must name
the exact implementation allowlist, formatted caps, error algebra, cold/warm
and A/B/A proofs, serial validation, and successor gate.

## Compatibility

Exact: Bazel 9.2 complete MODULE evaluation and ordered selected-registry
provenance for admitted registry modules; the unoverridden built-in empty-key
module and its immutable identity; root-first failure and explicit-override
bypass. Slug-native: DICE type/diagnostic names, compact allocation, logical
source spelling, and non-Bazel identity bytes. Unsupported/deferred:
nonregistry discovery/evaluation through this owner, command overrides,
recursive discovery, MVS, mappings, extension identities, RepoSpecs/yanked/
hash aggregation, lockfile writing, package/BUILD/Bzl loading, configured
toolchains, Test, commands/consumers, execution/results/BEP/coverage, Windows,
JVM/Java, and exact Bazel identity bytes.

## Scope, proof, and stops

Edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Cap formatted net documentation growth at 240 lines. Add no Rust, Cargo/BUILD
metadata, asset, fixture/oracle record, generated file, dependency, public
surface, command behavior, or production representation.

Validation is `git diff --check`, exact-scope/net-line checks, active-layout
archive validation, cross-document packet-name consistency, and independent
latest-diff design review. Inspect exact existing type visibility and evaluator
event behavior before freezing Rust scope. Stop with `REPLAN` on evaluator
semantics changes, lost registry attempt/hash/category identity, computation of
the embedded leaf before override classification, invented nonregistry
breadth, legacy `ResolvedGraph` reuse, root/selected graph or consumer edge,
untracked IO, lock-held DICE compute, public surface, JVM/Java, fifth file, or
cap excess.

Independent source and latest-diff review accept the built-in/registry-only
leaf. Existing source-preparation ownership already provides root-first
override classification, selected-registry bytes and ordered attempt/hash
provenance, the complete evaluator, event capture, and same-file DICE test
infrastructure. No visibility or module-boundary widening is required.

## Active implementation contract

Implement only in `app/slug_bzlmod_v2/src/source_preparation.rs`. Add a
crate-private `HostDiscoveredModuleKey` keyed by normalized workspace and
`NonrootModuleKey`, a crate-private complete value, typed provenance, and
typed errors. Compute `RootModuleFilesKey` first. For unoverridden
`bazel_tools@<empty>`, compute the current `BuiltinBazelToolsModuleKey` and
project its full value. For a versioned request with no nonregistry override,
compute `ModuleSourcePreparationKey`; accept only its registry variant and
evaluate the selected bytes with the existing complete closure evaluator and
no includes. Preserve selected registry and all ordered attempts.

Explicit `bazel_tools` overrides fail before the built-in key is computed.
All nonregistry preparations, empty non-built-in versions, direct-local
routes, and unsupported include/evaluation cases return typed terminals.
Store one event batch only when capture is enabled and the wrapper completes.
`Need` is invalid; complete values and errors compare structurally. Add no
consumer.

The first formatted compile measured the typed one-file seam at 266 production
lines; the original 190-line estimate omitted the complete error/provenance
algebra and root-first DICE branching. Correct the cap to 290 production lines,
360 test lines, and 650 total without changing scope or behavior.

Add no file, public export, Cargo/BUILD metadata, dependency, utility, lock,
cache, interner, process-global state, fixture, or command behavior.
Focused tests must prove exact built-in and registry provenance, selected
registry A/B/A, ordered absence/hash attempts, root-first explicit built-in
override bypass through activation tracking, nonregistry/empty/include typed
failures, separately computed equality, cold `Evaluated` then warm `Reused`,
event/no-event behavior, invalid Need, and structural absence of graph/MVS,
mapping, lockfile-writer, package, or consumer edges.

Run serially `cargo test -p slug_bzlmod_v2 host_discovered_module`, the full
`slug_bzlmod_v2` suite, downstream `cargo check -p slug_loading_v2` and
`cargo check -p slug_core_v2`, `cargo fmt --all -- --check`, and
`git diff --check`. Also run exact-scope/cap, no-public-surface,
credential-pattern, active-layout archive, and forbidden-edge scans. Obtain
independent latest-diff implementation review before commit. Stop with
`REPLAN` on evaluator semantic changes, lost provenance, built-in activation
before root override classification, admitted nonregistry/command breadth,
second graph, consumer, second file, or cap excess.
