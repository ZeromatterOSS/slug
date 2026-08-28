# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-package-label-context-prerequisite-r3`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `14ddab17b`.

Result: make canonical repository BUILD evaluation resolve string labels in
the loaded package's full package/repository context, retain only final
canonical identities, and convert the existing external-query projection from
provisional-root assumptions to those producer-owned identities. This unblocks
the bounded selected-nonroot native toolchain closure required by the parked
registration-consumer cutover without moving label conversion, mappings or
source discovery into analysis.

## Replan cause and predecessor

The uncommitted `WP-4-5-7A-expanded-registration-consumer-cutover` correction
proved both expansion families can be consumed with cross-family
outer-before-Need-before-semantic polarity, but its mandatory selected-nonroot
success fixture exposed a missing prerequisite. `HostPackageAttemptInput`
already carries a full `PackageIdentifier`, and the canonical route already
owns the selected repository mapping, but `PackageRecorder::new_host` drops
both to a package-path string. Consequently `package_context_label` constructs
provisional main-repository labels for `:` and `//` inputs and rejects every
`@` input, including exact canonical `@@//:type`. Canonical native platform,
constraint and toolchain references therefore cannot form a repository-aware
configured closure.

Repairing those identities in analysis would violate the active packet's stop
against analysis-side mapping/context repair and would leave every other
loaded-package consumer with incorrect labels. The registration-consumer diff
remains parked and outside this prerequisite's allowlist. Resume it only after
this packet is accepted.

The first implementation passed the complete loading suite, then the required
direct-dependent query suite exposed another provisional-root consumer:
external restricted visibility rejected the now-correct `@@dep+//:group` as a
named-repository edge. The same query projection uses that old assumption for
filegroup sources, aliases, test-suite members and package-group includes.
Correct the whole existing consumer category here; accepting both identities
or repairing them back to root would create a path-only compatibility shim.

R2's complete query validation then found one pre-existing precedence fixture
whose external BUILD used unmapped apparent `@dep` solely to reach the query
projection's different-repository rejection. Final package-context conversion
correctly rejects that spelling during loading, before query projection. R3
adds only that existing test file to the proof allowlist and spells the case as
an explicit different canonical repository so it continues to prove its
query-layer ordering without weakening the loading boundary.

## Learned facts and research basis

Pinned Bazel 9.2 is compatibility authority:

- `Label.parseWithPackageContext` and `computeRepoNameWithRepoContext` resolve
  relative and repository-less labels in the current package repository,
  preserve explicit `@@canonical` identity, and consult the current
  repository mapping only for apparent `@repo` spellings;
- `LabelConverter` owns one package context plus a conversion cache, and
  `BuildType.LabelType.convert` delegates BUILD string-label conversion to it;
- `LabelConverterTest` discriminates mapped apparent, repository-relative
  absolute and package-relative conversion; and
- `LabelTest` discriminates canonical `@@repo` parsing and explicit main
  repository identity. Existing Slug canonical-route, repository-package and
  package-carrier tests already prove selected mapping ownership, observation
  order, cancellation, equality and A/B/A behavior. Add no Bazel fixture.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Its `build_single_host_capture` keeps BUILD package lowering responsible for
repository mapping, while `generic_label_value` consumes an already-selected
mapping and retains canonical label identity. Reuse that ownership idea only;
copy no Zig code, cache, packed identity, diagnostic or compatibility claim.

## Decision and ownership

The existing canonical repository package evaluator is the natural owner.
Thread its already-owned `PackageIdentifier` and immutable selected mapping
`Arc` into `PackageRecorder` as synchronous evaluator scratch. Resolve every
string label through one pure package-context helper:

1. `:target`, bare target and repository-less `//pkg:target` use the current
   package's canonical repository;
2. explicit `@@repo//pkg:target`, including `@@//` main-repository spelling,
   preserves that canonical repository without a mapping lookup;
3. apparent `@repo//pkg:target` uses the already-selected mapping and fails
   closed when the apparent name is absent; and
4. Bazel's absolute `//conditions` and `//visibility` packages remain in the
   main repository.

`LoadedPackage` retains only resolved `CanonicalLabel`s. It retains no mapping,
route, package context or evaluator value. Root package evaluation continues
to use its current root context and empty external mapping; adding the missing
root Bzlmod package mapping is a later packet, not a fallback here.

The external query graph consumes these final identities by requiring both the
selected canonical repository and package path for same-package references.
Its visibility context adapter accepts an already-canonical label only in that
same repository and continues to rebind genuinely provisional-root package
spec state. A different canonical repository remains a named-repository
deferred edge; equal package paths never authorize it.

Do not add a DICE key, mapping copy in analysis, source lookup, path inference,
second parser, interner or process cache. The existing canonical route remains
the sole mapping owner and the package inventory remains the sole loaded
package owner.

## Proof obligations

Prove:

- a canonical package rebinds `:local`, bare and `//same_repo` native
  references to its own canonical repository;
- explicit `@@//` remains main and explicit `@@other+` remains that canonical
  repository rather than rebinding;
- apparent labels use the supplied selected mapping, while an absent mapping
  fails closed before a package result is published;
- `//conditions` and `//visibility` retain main-repository identity;
- two canonical repositories with the same package path produce distinct
  internal labels;
- external query filegroup, alias, test-suite, package-group and restricted
  visibility consumers accept same-package labels only when canonical
  repository identity also matches, and continue to reject cross-package and
  different-repository shapes;
- root package behavior and its existing external-label rejection remain
  unchanged; and
- observed route/inventory event ownership, Need/outer polarity, cancellation,
  warm equality and A/B/A remain owned and proved by existing keys.

Use code-local package/inventory regressions. No `fixture.toml`, oracle asset or
external process is required.

## Compatibility classification

- **Exact:** the admitted Bazel 9.2 package-context conversion rules above and
  final canonical repository/package/target identity.
- **Slug-native:** private Rust helper/layout, evaluator-scratch mapping `Arc`
  transport, query projection layout, and error wording not covered by the
  cited tests.
- **Unsupported/deferred:** root BUILD apparent external labels until the root
  package owner receives its Bzlmod mapping, mapping-dependent selector and
  alias breadth, external toolchain context indexing, general external
  configured graphs, and broader rule/provider/action semantics.

## Request, revision and memory behavior

The selected canonical route already derives its immutable mapping from the
request's Bzlmod graph and participates in DICE equality/invalidation. Package
evaluation consumes that exact `Arc` inside the same key computation; no
mutable host state or historical snapshot is consulted. Overlapping requests
retain independent DICE transactions and route values.

The mapping and package context are evaluator/compute scratch and drop after
the loaded package is built or evaluation is cancelled. Only resolved label
values enter the existing DICE-retained `LoadedPackage`. No lock spans a DICE
compute, no evaluator heap escapes, and no new publication/equality boundary
is introduced.

## Allowlist, complexity and caps

Production:

1. `app/slug_loading_v2/src/package.rs`
2. `app/slug_loading_v2/src/bzl_module.rs`
3. `app/slug_loading_v2/src/visibility.rs`
4. `app/slug_query_v2/src/graph.rs`

Proof:

5. `app/slug_loading_v2/src/host_package_inventory_tests.rs`
6. `app/slug_query_v2/tests/loading_query.rs`

The parked analysis files, identity/Bzlmod/core/CLI/Cargo/BUILD/fixture/
oracle/Zabel files and all other plans are excluded after this scheduling
commit. Caps: 360 net production lines, 360 net proof lines, 720 total; no new
or materially rewritten function over 120 lines.

Both production files exceed the 2,000-line review trigger but remain the
existing cohesive BUILD evaluator/loaded-package owners. This packet adds one
pure conversion helper and threads two existing immutable inputs through one
synchronous recorder. STOP if conversion requires another evaluator, retained
mapping field, key family or general root-package route redesign.

`graph.rs` also exceeds the trigger but remains the cohesive external-query
projection owner. Its bounded correction changes the shared same-package
predicate and existing tests together; it adds no discovery, graph owner,
route lookup or mapping. `visibility.rs` keeps its existing context adapter and
adds only same-repository idempotence for already-final labels.

## Validation

Run serially:

1. focused package-context and canonical inventory regressions;
2. complete `slug_loading_v2` and `slug_bzlmod_v2` suites;
3. `slug_query_v2`, then `slug_analysis_v2`, allowing only the parked analysis
   diff already present in the worktree;
4. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and archive status against
   its recorded three-file baseline; and
5. independent terminal review before acceptance.

## Stops

STOP and `REPLAN` for an analysis-side label repair; a new DICE key or route;
filesystem, source or mapping discovery in the evaluator; retaining a mapping
in `LoadedPackage`; changing root-package mapping behavior; accepting an
unmapped apparent repository; path-only repository identity; aliases,
selectors, target settings or general external configured admission; query
source discovery or acceptance of a different canonical repository; new
rule/builtin/action semantics; Rust ownership of BCR rules or `cc_internal`;
a C++ parser/rule engine; Zabel treated as authority; files outside the
allowlist; or cap overflow.
