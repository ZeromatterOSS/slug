# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-loading-source-address-implementation-r2`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `09de9af9e`.

Result: implement the accepted Stage B source-address contract so canonical
repository BUILD files, external subtree traversal and recursive `.bzl` loads
consume the shared Root/Canonical carrier without an apparent alias, a
fabricated absolute path for embedded content or a second semantic route.

## Accepted basis

Commit `fa896aca4` accepts Stage A: Bzlmod REPO, ignore, package lookup,
boundary and BUILD-source policy share `HostRepositorySourceRoute`; canonical
policy is alias-free; and the temporary canonical source/listing wrappers are
deleted. Root behavior and dependency order remain exact. Built-in canonical
package selection currently stops at its authenticated catalog-relative
address because `RepositoryPackageSource` still requires a Host absolute path.

Commit `e47d5d4c8` freezes the independently reviewed Stage B architecture:

- retain `HostRepositorySourceRoute` as the sole Root/Canonical semantic
  carrier;
- distinguish Host absolute source addresses from built-in catalog-relative
  source addresses;
- retain producer-owned byte `Arc`s without copying;
- derive parser/evaluator source names from semantic identity rather than
  readable access or presentation paths;
- generalize loading-owned subtree, package, recursive `.bzl` and cycle
  identities over that carrier; and
- resolve canonical mapped child loads through the final canonical mapping and
  the child canonical load-route owner before observing child source.

The first implementation preflight stopped before Rust edits. The canonical
route exposes point lookup for parsed `load()` labels, but the generic
evaluator also requires the complete final mapping for Starlark `Label()`
construction inside BCR modules. Reconstructing it from observed loads would
be incomplete, while copying route state into loading would create a second
owner. R2 therefore admits one read-only projection on the existing canonical
route owner and no route-production change.

The existing `<output_base>/external/<canonical>/...` package directory and
BUILD-file paths remain an explicitly Slug-native evaluator/publication
projection. They are not source authority and may not be used for source IO.

This is generic BCR Starlark loading architecture. Bazel 9 BCR Starlark owns
all rule definitions and rule control flow, including `cc_internal`. Slug's
Rust implementation supplies reusable evaluator and host-ABI capabilities;
`cc_common` is one demanding consumer, not a Rust C++ parser, native rule
implementation or C++ rule engine. Builtins remain organized by reusable
capability category so later host modules do not force parser/loading churn.

Zabel's separation of semantic source root, access, canonical runtime identity
and parsed scratch informs the design. Zabel is peer architecture and
optimization guidance only; Bazel 9.2 behavior and pinned source remain the
compatibility authority.

## Exact implementation allowlist

Change only these production files:

1. `app/slug_bzlmod_v2/src/builtin_repository.rs`
2. `app/slug_bzlmod_v2/src/canonical_repository_route.rs`
3. `app/slug_bzlmod_v2/src/host_package.rs`
4. `app/slug_bzlmod_v2/src/lib.rs`
5. `app/slug_loading_v2/src/bzl_module.rs`
6. `app/slug_loading_v2/src/external_subtree_package_set.rs`

Change only these proof files:

7. `app/slug_bzlmod_v2/src/host_external_package_boundary/tests.rs`
8. `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`
9. `app/slug_loading_v2/src/host_package_load_tests.rs`
10. `app/slug_loading_v2/src/external_subtree_package_set_tests.rs`

The two active plan ledgers may record selection and completion. No Cargo,
BUILD, dependency, fixture, oracle, lockfile, query/core/module-extension
production or other source file is admitted.

## Bzlmod source-result implementation

Replace the built-in-only deferred package-source terminal with one explicit
repository source address discriminant owned by `host_package.rs`:

- `Host(NormalizedAbsolutePath)` for all existing root/materialized sources;
  and
- `BuiltinCatalog(repository-relative path)` for embedded catalog sources.

`RepositoryPackageSource` retains that address, the selected BUILD name and
the producer-owned `Arc<[u8]>`. Preserve the existing borrowed-Arc byte API or
an equivalently zero-copy crate-private API. A bounded crate-private built-in
value byte-Arc accessor is admitted only to retain the producer allocation;
do not add an interner, cache, content hash or second byte allocation.

Existing root constructors, variants, results, errors, display and dependency
order remain exact. Add a canonical constructor that accepts the already
authenticated `HostCanonicalRepositorySourceInput` through the shared carrier.
Delete `BuiltinSourceAddressDeferred` only as the new discriminant becomes
consumable in loading. Never turn a catalog-relative address into
`NormalizedAbsolutePath`, a workspace path, an execroot path or an output-base
path.

Add one allocation-bounded read-only final-mapping projection to
`HostCanonicalRepositoryRoute`. It returns the already-owned selected or
generated apparent-to-canonical pairs in evaluator input shape and returns an
empty mapping for the built-in route. It may not compute, cache, filter,
reinterpret or become a second mapping owner. Loading uses this projection for
the complete `BzlLoadManifest` repository mapping; parsed child `load()` labels
still use `mapping_target` and the child canonical load-route key.

## Loading implementation

Generalize these retained loading identities from `RootRepositoryRoute` to
`HostRepositorySourceRoute` while preserving existing root constructors:

- external subtree package-set and observed keys;
- repository package-load and observed keys;
- external `.bzl` evaluation and observed keys;
- resolved external child loads; and
- external `.bzl` cycle identities.

Use a pure loading-owned adapter for parser/evaluator source names:

- root Host sources retain their accepted absolute source names;
- canonical BUILD and `.bzl` sources use stable valid-Unicode canonical-label
  names; and
- repository package publication retains its current Slug-native output-base
  presentation paths.

The adapter is stack/scratch state, not a DICE key, retained cache, interner,
filesystem capability or source address. BUILD parsing, external `.bzl`
parsing, evaluation diagnostics and cycle diagnostics must use the same
canonical naming rule. Keep `LoadedPackage.package_dir` and `build_file`
equality unchanged.

Branch root source observation explicitly through the accepted
`HostRepositorySourceFileKey`/observed sibling so root dependency metadata and
order do not change. Branch canonical source observation through the shared
`HostRepositorySourceObservationKey`/epoch sibling. Do not silently migrate
root callers to the shared observation owner.

External subtree boundary and listing demands use the existing root owners for
Root and the accepted canonical constructors for Canonical. Preserve ignored
pruning, deleted-descendant traversal, fail-closed child handling, event order
and retained lexical identity.

For recursive external `.bzl` loads:

1. retain the admitted Bazel label parser and exact root resolution path;
2. keep same-repository canonical loads on the current carrier;
3. resolve a canonical apparent child repository through the current route's
   final mapping;
4. demand the child `HostCanonicalRepositoryLoadRouteKey` or observed sibling
   by workspace and canonical repository;
5. in observed mode merge child route/effect observation before child source;
6. construct the child module key from the returned canonical source input;
   and
7. never synthesize a root apparent repository name.

Need, outer-frontier error, semantic route error, generated-effect error,
source terminal, parse/evaluation terminal and child-module terminal retain
their accepted precedence. Cycle identities contain the exact carrier and
repository `.bzl` label. Cycle completion reobserves source through the
carrier-appropriate owner.

Do not change canonical load-route production, repository registration,
mapping computation, source materialization, package glob behavior, target
pattern expansion, configured analysis, provider/rule declaration semantics,
action creation or execution.

## Proof contract

The allowed tests must prove all of the following without a new oracle unless
a demonstrated Bazel behavior gap exists:

- byte-for-byte root result/error/display compatibility and exact root child
  dependency order;
- canonical built-in BUILD success retaining the exact catalog-relative
  address and the same byte `Arc`, with no invented absolute path;
- alias-free selected-registry package-load success where no root apparent
  mapping exists;
- canonical same-repository and mapped-child `.bzl` success;
- child canonical route/effect observation before child source, including the
  exact merged observation-epoch order;
- child route Need, route semantic error, effect error, missing source,
  parse/evaluation error and recursive-cycle polarity;
- canonical external subtree traversal through the shared boundary/listing
  owners;
- key/equality/hash A/B/A discrimination across workspace, canonical name,
  disposition, selected specification, final mapping, generated plan, package
  and `.bzl` label;
- drop-before-publication and same-DICE recovery for canonical package and
  recursive `.bzl` loading;
- complete canonical evaluator mapping, including a mapped `Label()` that is
  not also present in a `load()` statement;
- no production reference to a deleted temporary wrapper, no fabricated
  apparent alias and no fabricated absolute catalog path; and
- retained-size bounds plus `Allocative` coverage for each changed or new
  retained enum, key and value.

Explicitly protect source-name consistency across BUILD parsing, external
`.bzl` parsing, evaluation/cycle diagnostics and published package
presentation. A canonical test must distinguish each domain so an access path
cannot accidentally become a parser name or publication path.

## Compatibility classification

- **Exact:** Bazel 9.2 package-marker selection, repository mapping and
  admitted load-label resolution; BCR catalog bytes; root load results,
  diagnostics, events and dependency order; source observation before
  evaluation; and admitted error/Need precedence.
- **Slug-native:** Root/Canonical carrier and key layout, source-address enum,
  canonical-label parser source names, explicit output-base repository-package
  presentation, structural hashes and retained-memory accounting.
- **Unsupported/deferred:** unadmitted load-label forms, broader glob/package
  traversal, registration expansion, configured semantics, additional host
  builtin categories including `cc_common`, rule/action execution and exact
  output identity.

## Caps, validation and review

Allow at most 1,450 net added production Rust lines, 2,200 proof Rust lines and
3,650 aggregate Rust lines. Within those totals, cap `bzl_module.rs` at 950 net
production lines and any single proof file at 1,200 net lines. Functions are
at most 120 lines. Extract narrow helpers inside the allowlist rather than
expanding large drivers or performing adjacent cleanup.

Terminal accounting must report gross additions and deletions as well as net
growth for `bzl_module.rs`, and review its changed drivers for duplicated
control flow. The net cap may not conceal a broad rewrite or parallel root and
canonical implementations where one carrier-aware helper suffices.

Run Cargo commands serially in one target directory:

1. focused Bzlmod package-boundary/source tests;
2. focused canonical load-route, host package-load and external-subtree tests;
3. full `slug_bzlmod_v2` with `--test-threads=1`, recording the accepted
   pre-existing default-parallel activation-order flake separately;
4. full `slug_loading_v2` and `slug_query_v2` suites;
5. focused core root-apparent and generated-package-route regressions;
6. locked `cargo build -p slug_cli_v2` before any `SLUG_V2_BIN` smoke;
7. formatting, checks, `git diff --check`, scope/cap/function/structural guards;
   and
8. `scripts/v2_archive_status.sh`, permitting only its known three-row
   baseline.

Require independent DICE/loading/retained-representation terminal review.
Review must explicitly adjudicate root dependency compatibility, source-name
domain consistency, zero-copy built-in ownership, canonical child observation
order and the absence of a second route or apparent alias.

## Stops

STOP and `REPLAN` for a second semantic route; a fabricated apparent alias; an
absolute catalog-content path; source IO through a parser or presentation
path; copied source bytes; a new interner/cache/dependency; a lock held across
DICE compute; changed root output/error/event/dependency order; loading-owned
catalog IO or materialization; retained parser scratch; canonical load-route
production changes or a second mapping owner; query/core/module-extension
production scope; Rust
ownership of BCR rule definitions or `cc_internal` control flow; a C++ parser
or rule engine; or activation of registration/configured/rule/action behavior.

On acceptance, freeze the shared registration-expander architecture before
implementation. Additional builtin categories, including `cc_common`, follow
through the same generic evaluator/host-ABI architecture rather than through
language-specific parsing.
