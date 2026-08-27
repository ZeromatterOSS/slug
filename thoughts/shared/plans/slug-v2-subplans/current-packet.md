# Current Slug V2 Packet

Packet: `WP-4-5-7A-registration-expansion-prerequisite-owners`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `ae46e1cf5`.

Result: implement the identity-owned contextual canonical target-pattern
projection and extract one general Root/Canonical repository package-inventory
DICE owner beneath the current behavior-preserving package-policy adapter. Do
not add, consume or activate a registration expansion key.

## Immediate predecessor

Commit `ae46e1cf5` independently accepts the shared registration-expander
architecture. It freezes one grammar and contextual canonical projection, one
general package inventory, independent toolchain/execution-platform family
keys, exact stable-postorder recursive priority, warning-bearing equality and
the downstream configured boundary. This packet implements only the two
prerequisite owners selected as step 1.

## Learned facts and research basis

Pinned Bazel 9.2 remains compatibility authority:

- `TargetPattern.Parser.parse` delegates absolute syntax to
  `LabelParser.Parts`, preserves `all`, `*` and `all-targets`, and creates the
  same-named explicit-target conflict candidate for every absolute package
  wildcard.
- `TargetPattern.Parser.createPackageIdentifierFromParts` uses `currentRepo`
  only when no repository token is present. An explicit `@//pkg` performs
  `RepositoryMapping.get("")`; `Module.getRepoMappingWithBazelDepsOnly` adds
  that empty mapping only for the root module. Thus `//pkg` means the declaring
  canonical repository, root `@//pkg` maps to main, and nonroot `@//pkg` is a
  not-visible apparent repository rather than an alias for self.
- Canonical `@@repo//...` syntax bypasses apparent mapping. The command-facing
  Slug `TargetPattern` intentionally remains an apparent boundary and must
  continue rejecting it; MODULE contextual parsing admits it through the same
  factored grammar.
- `TargetsInPackage.getWildcardConflict` checks the explicit candidate only
  after package load. Parsing therefore retains the wildcard spelling and
  candidate but performs no inventory lookup.
- `ModuleFileGlobals.checkAllAbsolutePatterns` already rejects relative MODULE
  rows. This identity leaf admits only absolute exact/package/recursive
  patterns and does not implement command-relative path interpretation.

Applicable upstream regressions are `TargetPatternTest.validPatterns_*`,
`testAbsolutePatterns`, `testInterpretPathAsTarget`,
`ModuleFileFunctionTest.testRegisterToolchains_singlePackageRestriction_underDir`
and the package-wildcard conflict tests. Existing Slug identity tests protect
the apparent command boundary; add focused contextual cases for declaring,
mapped, canonical and empty-apparent spelling rather than a new Bazel oracle.

The live checkout establishes these implementation facts:

- `HostSelectedRegistrationPatterns` already retains compact route/pattern
  ordinals over the sole complete final mapping. Its view currently republishes
  a full mapping iterator although contextual parsing needs only one borrowed
  point lookup.
- `RepositoryPackageLoadKey` currently owns source selection, recursive `.bzl`
  evaluation, BUILD evaluation, `LoadedPackage`, local events and observation
  epochs, then applies restrictions inherited from an earlier configured
  consumer. Those restrictions reject useful general inventories after the
  evaluator has already produced them.
- Legacy and observed package-load siblings already share one driver and use
  complete-only equality. The split can preserve all source/load/evaluation
  terminals and exact epoch `Arc`s without another evaluator or filesystem
  read.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept/test guidance
only: its selected-row owner and package-fact split reinforce borrowed mapping
resolution and producer-owned inventory. Copy no Zig code, packed layout,
identifier or compatibility claim.

Buck2/V1 utility review selects the existing V2 representation. Keep
`SmallMap` on the sole selected mapping, `Arc` package carriers and
`Allocative` retained keys/values. Add no `HashMap`, `HashSet`, `String` copy of
the mapping, interner, cache, collection dependency or Stage 9 donor import.
The new projection must prove `Arc::ptr_eq` for accepted packages, so the
temporary adapter adds a key/value shell but no second `LoadedPackage`.

## Decision and non-decisions

### Shared contextual syntax leaf

Factor `pattern.rs` into one private absolute-pattern grammar with repository
spelling `Current`, `Apparent` or `Canonical`. Both public projections consume
that grammar:

1. `TargetPattern::parse` preserves its current apparent representation,
   display and rejection of canonical spelling.
2. A public pure contextual API returns `CanonicalTargetPattern` as an exact
   `CanonicalLabel`, a package wildcard containing its canonical
   `PackageIdentifier`, exact wildcard kind and optional same-named conflict
   `CanonicalLabel`, or a recursive canonical package plus wildcard policy.

The contextual API receives the declaring `CanonicalRepoName` and a borrowed
lookup closure for one `ApparentRepoName`. Missing apparent names return a
typed parse/resolution error string; they never fall back to root or declaring
repo. It constructs canonical values directly, retains no mapping provenance,
and performs no DICE, package, evaluator or filesystem work.

Add `mapping_target(&ApparentRepoName) -> Option<&CanonicalRepoName>` to
`HostSelectedRegistrationPatternView`. Keep the full iterator for current
callers; do not copy, normalize or republish the mapping. Focused tests prove a
nonroot row resolves `//` to its owner, resolves a visible apparent name by
borrowed point lookup, admits `@@`, and rejects nonroot `@//` when the empty
mapping is absent.

Do not parse registration rows eagerly, retain parsed rows, alter MODULE
declarations/mappings, accept relative syntax, change command target patterns,
or add the registration expander.

### General package-inventory owner and policy projection

Introduce crate-private `RepositoryPackageInventoryKey` and its observed
sibling with the same `HostRepositorySourceRoute + PackagePath` structural
identity as the current key. Move the existing compute driver to that owner.
It alone owns:

- selected BUILD source and recursive `.bzl` dependencies;
- BUILD parsing/evaluation and the complete `LoadedPackage`;
- local BUILD evaluation `EventBatch`; and
- the merged `PathObservationEpoch` for the observed sibling.

The inventory terminal reuses `RepositoryPackageLoadError` for existing
source, encoding, parse, load, evaluation and glob limitations, but never
constructs `LoadedTargetKind` or `LoadedStarlarkRule`; those are consumer-policy
errors.

Keep public `RepositoryPackageLoadKey` and
`RepositoryPackageLoadObservationKey` as thin policy projections. Each
computes the matching inventory sibling, preserves outer error/Need/semantic
precedence and epoch identity, validates a borrowed `&LoadedPackage`, returns
the exact incoming result `Arc` on success or allocates only the old typed
policy error on rejection, and stores no local event batch. Child inventory
events therefore remain the sole operational owner and replay once. Need and
cancellation publish neither inventory nor adapter event data.

The dependency shape becomes
`RepositoryPackageLoad{Observation}Key -> RepositoryPackageInventory{Observation}Key -> existing source/.bzl owners`.
Prove it for root-route and canonical-route inputs, including warm reuse,
source and child Need/error prefixes, A/B/A restoration, cancellation recovery,
captured/uncaptured events and observed epoch `Arc` reuse. The general owner
must accept at least an alias and a multi-target/dependency-bearing loaded
Starlark inventory that the old adapter rejects with its unchanged error.

Root workspace packages remain owned by `RootPackageLoadKey`; this packet
generalizes the already accepted Root/Canonical external source route and does
not merge root-workspace loading into it.

## DICE, request/revision and memory contract

Both inventory keys hash only route plus package. The route already contains
the immutable workspace/canonical source projection; source, repository
mapping, file and epoch changes remain explicit child dependencies. Legacy and
observed siblings retain their current request selection, final observation
validation, complete-only equality, and independent overlapping-request
behavior. No manual lock crosses a compute and no direct filesystem or
fresh-graph bypass is admitted.

The inventory key/value and `Arc<Result<LoadedPackage, ...>>` are DICE-retained
semantic memory. Loaded modules remain evaluator-owned frozen values retained
through `LoadedPackage`; no result borrows command or evaluator scratch. Parse
ASTs, prepared-load vectors and validation scans remain compute scratch and are
released on completion or cancellation. Event batches remain request-selected
operational evaluation data stored only by the inventory key. Epoch maps retain
their existing shared `Arc`s. The temporary policy adapter costs one key/value
shell and shared result reference per requested package; it duplicates neither
package data nor events and is removed by the fallback condition below.

## Fallback ledger

`RepositoryPackageLoadKey` remains a temporary consumer-policy adapter.

- Violated invariant: a general loading producer should not encode one
  configured/query consumer's accepted-target restrictions.
- Exact deletion condition: every direct command, query and configured-analysis
  consumer depends on the general inventory and applies its policy at its
  natural owner, while all accepted old restriction/error regressions remain
  protected.
- Owning migrations: later command/query/configured packets selected by the
  Stage 6 owner after the shared registration expander consumes the inventory.
- Permanence prevention: this packet adds dependency-shape and divergent
  inventory-versus-adapter regressions; the expander packet must downcast/prove
  the inventory key directly and must not depend on the adapter.

## Compatibility classification

- **Exact:** admitted Bazel 9.2 absolute contextual `//`, `@apparent` and
  `@@canonical` exact/package/recursive syntax; the `@//` root/nonroot
  distinction; wildcard spelling and explicit-conflict candidate identity;
  every existing public repository-package adapter result, restriction,
  diagnostic content/order, Need/error precedence, event content/replay and
  observed epoch behavior.
- **Slug-native:** canonical result enum/API, private grammar shape, Rust
  error strings outside accepted fixtures, inventory/policy key layout,
  structural hashes, event transport and shared-`Arc` memory accounting.
- **Unsupported/deferred:** actual registration expansion, ambiguity package
  lookup/warning, recursive ordering, family filtering/deduplication,
  configured provider/settings/alias resolution, CLI option patterns,
  relative command parsing beyond the accepted command surface, new evaluator
  semantics, additional builtin categories, rules and actions.

## Exact allowlist, caps and validation

Production files:

1. `app/slug_identity_v2/src/pattern.rs`
2. `app/slug_identity_v2/src/lib.rs`
3. `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
4. `app/slug_loading_v2/src/bzl_module.rs`

Proof files:

5. `app/slug_identity_v2/tests/pattern.rs`
6. `app/slug_loading_v2/src/host_package_load_tests.rs`

No Cargo manifest/lockfile, query/core production, BUILD, fixture, oracle,
other test or plan file is admitted after this scheduling commit. The selected
view test remains inline in its existing cohesive owner.

Caps: at most 950 net production lines, 1,400 net proof lines and 2,350 net
total lines; at most 650 net lines in `bzl_module.rs`, 300 in `pattern.rs`, 90
in `selected_repo_spec.rs`, and 120 lines for any new/touched function. The
10k-line loading owner and 14k-line Bzlmod owner trigger complexity review, but
the relevant private key drivers and sole selected-mapping owner remain
cohesive; splitting either physical module would cross adjacent accepted
families. New grammar/validation helpers must stay bounded and independently
named. STOP before unrelated cleanup.

Run serially:

1. focused identity contextual/apparent-boundary tests;
2. focused selected-registration borrowed-point tests;
3. focused package inventory/policy divergence, dependency, event, epoch,
   Need/error, cancellation and A/B/A tests;
4. complete `slug_identity_v2`, `slug_bzlmod_v2` and `slug_loading_v2` tests;
5. direct dependent `slug_query_v2` and `slug_core_v2` tests or their named
   affected loading/runtime suites if a complete crate gate is impractical;
6. `cargo fmt --all --check`, allowlist/cap checks, `git diff --check`, packet
   and canonical ID agreement, and `scripts/v2_archive_status.sh` against its
   recorded baseline.

No new oracle fixture is needed because pinned source plus existing accepted
wildcard evidence discriminates this prerequisite. Rebuild `slug_cli_v2`
before any later test invokes `SLUG_V2_BIN`; this packet plans no binary oracle
or smoke. Require independent terminal review of the public contextual API,
DICE ownership/equality, event single ownership, `Arc` sharing, old-policy
parity, mapping semantics and complexity/caps before acceptance.

Residual risk: the adapter temporarily adds one DICE node per requested
external package, exact future ambiguity diagnostic text is not yet exercised,
and the next packet must prove stable-postorder expansion against package
inventories. These do not authorize retained parsed rows, duplicate evaluation
or activation.

## Stops

STOP and `REPLAN` for a second grammar, mapping owner, BUILD evaluator or source
read; apparent spelling in canonical identity; `@//` silently resolving to a
nonroot owner; retained parsed patterns; mapping copies; a package restriction
left in the inventory driver; cloned `LoadedPackage`; duplicate/local adapter
event storage; epoch reconstruction; direct filesystem IO; a lock across DICE;
query/core production edits; registration expansion or activation; scope/cap
overflow; Rust ownership of BCR rules or `cc_internal`; a C++ parser/rule
engine; or treating Zabel as compatibility authority.
