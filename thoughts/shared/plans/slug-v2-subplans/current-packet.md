# Current Slug V2 Packet

Packet: `WP-4-5-7A-recursive-analysis-evaluator-adapter-implementation-r3`

Milestone: M7A category 5, one recursive retained analysis-value/provider
boundary before selected toolchain implementation analysis.

Base: `500f0f038`. Category 4 is accepted in `568b0c698`; the retained
category-5 graph and its two equality domains are accepted in `5ce967d55`.
The R1 and R2 implementation candidates are uncommitted. R2 terminal rereview
returned `REPLAN`: no more Rust may be changed until this corrected R3 contract
passes independent architecture review.

Observable result: implement and prove one heap-independent retained value
graph plus one lossless evaluator adapter family. Fresh, frozen and dependency-
rematerialized structs, exported user providers, builtin `ToolchainInfo` values
and depsets must participate in the same Bazel-visible equality/hash domain.
Dependency depsets remain shared DAGs, compose transitively without flattening,
validate when `depset()` is called and flatten only when `to_list()` is called.
Configured targets expose every provider variant already retained by their
`ProviderCollection` through an admitted exported provider callable. No
selected toolchain implementation is analyzed and `ctx.toolchains` is not cut
over in this packet.

## Why R1 was rejected

The R1 retained graph, structural configuration identity, publication
equality, numeric payloads, dictionary order and depset topology/alias
comparison remain useful candidate work. Its evaluator adapter is not
accepted:

- fresh user providers, `ToolchainInfo` and depsets used pointer/default
  evaluator behavior while rematerialized values used unrelated wrapper
  classes, breaking symmetric equality and hash behavior;
- rematerialized depsets were eagerly flattened, could not be used as
  transitive inputs to `depset()`, and delayed composition errors until result
  lowering;
- configured targets retained the complete provider collection but exposed
  only user-provider occurrences; and
- proof omitted fresh/frozen/rematerialized symmetry, dependency-depset
  recomposition, lazy `to_list()`, construction-time failures and builtin
  provider lookup.

These are explicit no-flatten/lossless-round-trip stops in the accepted
architecture, so R2 changes the evaluator adapter ownership rather than adding
another wrapper layer.

## Why R2 was rejected

R2 corrects the shared evaluator classes, authenticated provider lookup, typed
views, owner checks, alias materialization, iterative deep-DAG conversion and
most shared depset construction/traversal behavior. Focused build-api, loading
and analysis suites pass. Terminal correction rereview nevertheless found one
exact retained-representation miss and one scope violation:

- for a sole compatible different-order, nonsingleton transitive child with no
  directs, the builder decrements depth but retains a new
  `Transitive(child)` successor. Pinned `NestedSet` instead dereferences its
  sole physical successor, so the requested-order root shares the child's
  internal successor array without the extra node; and
- the vendored struct patch changes an existing JSON field-order assertion
  from `foo,bar` to `bar,foo`. JSON serialization is unrelated to the admitted
  structural-hash barrier and this packet has no authority to change or weaken
  that regression.

Because publication equality deliberately observes exact DAG topology and
alias partition, flattening equality is insufficient proof for the first miss.
R3 is limited to the canonical sole-successor representation/proof, restoration
of the untouched JSON assertion, and the unchanged R2 contract. No R1/R2 Rust
is accepted or committed.

## Learned facts and research basis

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority.

- `StructImpl#equals` compares provider identity, field names and field values;
  `hashCode` hashes provider plus fields sorted by name. `StarlarkInfoNoSchema`
  and `StarlarkInfoWithSchema#isImmutable` require an exported provider and
  semantically immutable fields. Their equivalence and immutable-provider
  tests prove construction representation is not part of equality.
- Bazel's special struct/provider hash barrier is intentionally different from
  ordinary Starlark key admission. A frozen list or dictionary is not itself a
  dictionary key, and a tuple recursively rejects it, but an exported immutable
  struct/provider may hash such a field through Java structural `hashCode`.
  The vendored Buck2-derived `StructGen::write_hash` currently delegates to the
  child's ordinary Starlark hash, so it incorrectly rejects that admitted
  frozen-container case. `StructGen::equals` also recognizes only its own
  native struct class; rematerializing with `AllocStruct` therefore supplies
  the one symmetric class boundary.
- `Depset#fromDirectAndTransitive` validates direct immutability/top-level
  list-or-dict rejection, exact top-level element type, compatible order and
  depth while composing. Empty depsets are shared per order; one unchanged
  transitive child with no directs is reused. `Depset#equals`/`hashCode`
  delegate to the underlying `NestedSet` occurrence, and `to_list()` is the
  explicit flattening operation. `DepsetTest#testTransitiveIncompatibleOrder`,
  `testBadGenericTypeTransitive`, `testEmptyDepsetInternedPerOrder` and
  `testSingleNonEmptyTransitiveAndNoDirectsUnwrapped` discriminate these
  behaviors.
- The existing shared Rust `Depset` is not yet an exact traversal/depth owner.
  Pinned `DepsetTest#testToStringWithOrder` requires a topological parent with
  direct `[3, 4, 5]` and a default-order child `[2, 4, 6]` to flatten as
  `[3, 5, 6, 4, 2]`, while the current eager child-direct walk returns a
  different order. Bazel implements topological order as right-to-left
  postorder followed by reversal and preserves each mixed-order child's
  representation. `NestedSetTest#getApproxDepth` also distinguishes two
  distinct singleton children at depth one, which produce depth two, from
  empty children, repeated identical children and a sole unchanged child,
  which are eliminated or reused and do not add depth. These semantics belong
  in the shared generic DAG, not in an analysis-only wrapper.
- Pinned `NestedSet` lines 253-267 canonicalize the physical successor count
  after hoisting: for `n == 1`, `children` becomes `children[0]` and depth is
  decremented. This is not limited to same-order `Depset` object reuse. A sole
  compatible different-order nonsingleton child therefore produces a distinct
  requested-order root that shares the child's internal successor array. An
  extra `Transitive(child)` node is noncanonical and changes Slug publication
  equality even when `to_list()` agrees.
- `AbstractConfiguredTarget#getIndex`, `containsKey` and `get(Provider.Key)`
  accept an exported declared-provider constructor, synthesize `DefaultInfo`
  and otherwise query the target's provider key. Lookup is by provider identity,
  not by printable name or by whether the payload happens to be a user-provider
  occurrence.
- `ToolchainInfo#copyValues`, `StructImpl` and `ToolchainInfoTest` retain zero or
  more sorted named fields and use structural equality. `ToolchainInfo` does
  not become semantically immutable/hashable merely because its Rust storage is
  immutable.
- Bazel/Starlark numbers include arbitrary integers and exact floats. The
  accepted retained graph's numeric cross-kind Starlark equality/hash and its
  exact-payload publication equality remain unchanged.

Live Slug facts:

- `slug_build_api_v2::analysis_value` in the R1 candidate already implements
  the opaque Arc graph, complete configured-target payload, key hashability,
  depset occurrence/publication domains and deep publication comparison. It
  must be corrected, not replaced with evaluator values or a global store.
- `slug_loading_v2::provider` owns both provider constructors and `depset()`.
  It is therefore the natural owner of evaluator provider/depset classes shared
  by loading-time construction, frozen module constants and analysis-time
  rematerialization.
- `slug_analysis_v2::starlark_rule` owns prepared dependency/result conversion,
  but its R1 materializer/lowerer mixed that conversion with the rule context
  and action host. The R2 candidate split conversion into an analysis-owned
  module; R3 preserves that split and changes no DICE key or lock.
- `ProviderCollection` retains typed `DefaultInfo`, `OutputGroupInfo`,
  `RunEnvironmentInfo`, `FilesToRunProvider` and `PlatformInfo` plus general
  occurrences. Typed payloads remain their existing operational owners; an
  evaluator view may project their already-retained fields in phase scratch
  but may not duplicate them in DICE state.

Prior art classification:

- Existing V2 `SmallMap`, `CompactString`, immutable `Arc` slices, `Dupe`,
  `Allocative`, arbitrary integer payloads and the shared `Depset` DAG are
  **leaf reuse**. A small process-local Arc occurrence token may preserve
  Bazel-visible depset identity; it is never publication, DICE, serialized or
  cross-process identity.
- Buck2's `ProviderCollectionGen`, `Hashed`, borrowed-key lookup and frozen
  provider tests are **concept/test only**. Its `FrozenValue`/`FrozenHeap`
  retained storage is explicitly **avoided**.
- Clean Zabel commit
  `0795445f3ab60f4e49070bdd0b94425c5610f73a` is **concept/test only** peer
  guidance. `generic_struct_object.zig` uses one canonical-field object for
  construction and materialization; `build_rule_declaration.zig` uses one
  authoritative provider identity for direct and rematerialized instances and
  caches structural hashes only after freeze; `analysis/depset.zig` retains
  typed shared child references without flattening. These ideas support the
  owner split and proof matrix, but no Zig layout, arena/store ID, dense index,
  ordinal shortcut, hash, scheduler, diagnostic or compatibility claim is
  copied.

## R3 decision

### Retained graph and equality domains

Preserve the accepted build-api contract:

- `AnalysisValue` is an opaque `Dupe`/`Allocative` cheap-clone handle over an
  immutable Arc graph with admitted `None`, Boolean, arbitrary integer, float,
  string, canonical label, configured target, artifact, list, tuple,
  insertion-ordered dictionary, struct, provider occurrence and depset kinds.
- `ProviderIdentity` is a builtin provider name or authenticated exported
  `ProviderId`; `ProviderOccurrence` is that identity plus one canonical
  field-name map. `ProviderCollection` is an Arc-backed compact map keyed only
  by `ProviderIdentity` and rejects duplicates.
- Bazel-visible equality/hash remains distinct from retained-publication
  equality. Configured targets and depsets use occurrence equality for
  Starlark, while publication equality includes every rematerialization payload,
  dictionary iteration order, depset order/type/DAG topology and alias
  partition. No pointer, ordinal or weak hash is DICE identity.
- `AnalysisDepsetOccurrence` is an opaque cheap-clone process-local token shared
  by an evaluator depset and its retained `AnalysisDepset`. It exists solely to
  preserve Bazel-visible occurrence equality/hash across freeze/lower/rematerialize.
  Publication comparison ignores token addresses and continues to compare the
  complete unflattened graph. Empty occurrences are shared per order.

The candidate's exact numeric-payload publication correction and identity-keyed
`ProviderCollection::get/contains` correction are retained. Any semantic change
to the admitted value kinds, publication domain or configuration identity is a
new `REPLAN`.

### One shared exact depset DAG owner

Correct `slug_build_api_v2::Depset` before adapting evaluator values. It is the
only traversal, structural-sharing and approximate-depth implementation used
by typed providers and `AnalysisDepset`:

- construction deduplicates direct members in insertion order, ignores empty
  transitive inputs, rejects incompatible non-default orders, and preserves
  ordered nonempty child edges;
- the builder returns the existing child Arc for one same-order nonempty
  transitive with no direct members, and for the Bazel singleton/matching-
  direct case; after singleton hoisting and deduplication, any sole physical
  successor is dereferenced. A compatible different-order nonsingleton child
  keeps the requested root order but shares that child's internal successor
  array rather than retaining an extra transitive wrapper node;
- the generic builder reports that last case as one explicit dereference result,
  not by copying child edges or teaching analysis/loading a second algorithm.
  The build-api root creates a new requested-order node with the child's
  `Arc` successor slice, while the evaluator adapter creates a new occurrence
  and delegates its retained `AnalysisDepset` to the same shared-slice path;
- empty is depth zero, a leaf is depth one, and a materialized non-leaf is one
  plus its deepest retained successor after those builder optimizations;
- default/postorder and preorder retain their source-defined left-to-right
  traversals; topological uses the Bazel `LINK_ORDER` right-to-left postorder
  plus final reversal, including mixed default/topological child orders,
  rightmost diamond tie-breaking and set-node identity deduplication; and
- flattening remains lazy. The shared core may memoize no eager leaf list and
  exposes the same DAG handles to publication/alias comparison.

Pinned `DepsetTest#testToString`, `testToStringWithOrder`, the four-order
constructor cases, and `NestedSetTest#getApproxDepth` are source regressions for
this owner. `AnalysisDepset` delegates order traversal, builder reuse and depth
to it; loading and analysis may add Starlark type/immutability and occurrence-
token behavior but must not implement a second depset algorithm.

### One evaluator class per semantic value family

`slug_loading_v2::provider` owns the shared evaluator provider/depset classes:

- Fresh, frozen and rematerialized exported user providers all use
  `LoadingStarlarkUserProviderGen<V>`. The class implements identity-plus-field
  structural equality and hash, independent of schemaful/schemaless storage or
  construction order. Rematerialization allocates the same frozen class with
  canonical fields; it does not allocate `MaterializedProvider`. Its field hash
  calls the same vendored struct-field helper as native structs, so the Bazel
  frozen-container barrier has one implementation.
- Fresh, frozen and rematerialized builtin `ToolchainInfo` all use
  `StarlarkToolchainInfoGen<V>`. It implements structural equality and field
  access but remains directly unhashable and semantically non-immutable. It
  does not use a general user-provider wrapper. The existing marker-era
  `AnalysisToolchainInfo` class is deleted: until category 6 removes the marker
  bridge, `AnalysisToolchains::at` allocates this shared loading-owned class
  with the prepared toolchain's canonical `marker` field.
- Every evaluator depset uses `StarlarkDepsetGen<V>` with order, top-level type,
  one occurrence token, ordered direct values and ordered transitive depset
  values. A retained dependency is rematerialized recursively into this same
  class, memoized by retained occurrence, with no flattened leaf cache.
  `depset()` accepts fresh, frozen or rematerialized transitive children,
  validates direct immutability/type, transitive type/order and depth before
  returning, shares empty values per order and returns the unchanged sole
  transitive value when Bazel does. `to_list()` alone traverses and deduplicates
  lazily in the declared order. Lowering a rematerialized depset returns its
  original retained handle; lowering a fresh/frozen DAG preserves its token,
  sharing and child edges.
- Rematerialized structs use native `AllocStruct`, the same class as fresh and
  frozen `struct()` values. The bounded vendored evaluator correction adds an
  internal struct-field structural hash and one doc-hidden structural-value
  capability for custom exported providers. The capability reports Bazel
  semantic immutability and a total structural hash separately from ordinary
  key hashability. The helper checks immutability at a direct struct/provider
  field, treats a frozen native list/dictionary as Bazel's non-recursive
  immutability barrier, and recursively total-hashes native list/dict/tuple/
  struct children. A custom `ToolchainInfo` supplies a total hash for use behind
  such a barrier while reporting non-immutable and remaining directly
  unhashable. Top-level list/dict and ordinary tuple key admission stay
  unchanged; mutable direct children reject. No global mutable state, new
  Starlark type or replacement `struct` builtin is added.

The vendored change is limited to that structural-hash capability and focused
hash/key tests. Restore the pre-existing `json.encode(struct(foo = 42,
bar = "some"))` field-order assertion byte-for-byte; no serialization behavior
or assertion changes are admitted.

Provider/depset equality must be symmetric because both operands downcast to
the same class. Cross-class equality hooks and analysis-only wrapper classes are
forbidden.

### Analysis conversion and complete configured-target view

Move conversion out of the already mixed `starlark_rule.rs` owner into
`slug_analysis_v2::analysis_value`:

- The lowerer recognizes all admitted native containers, shared loading-owned
  provider/ToolchainInfo/depset classes, canonical loading labels and
  analysis-owned target/artifact views. It memoizes evaluator identity only in
  phase scratch, rejects cycles/unsupported values at the first path and
  publishes no `Value`, `FrozenValue`, heap or scratch identity.
- The materializer allocates immutable evaluator views on the analysis module's
  `FrozenHeap`. Native lists/tuples/dictionaries/structs remain native classes;
  provider and depset values use the loading-owned shared classes. Depset
  materialization recursively memoizes occurrences and never precomputes a
  flattened list.
- Declared and rematerialized artifacts use one analysis-owned evaluator class
  and complete `AnalysisArtifact` identity. The rule owner is known before
  evaluator entry, so declared files no longer need a path-only class that
  compares differently from dependency artifacts.
- One callable-identity adapter recognizes every exported provider callable
  already admitted by loading: authenticated user constructors and existing
  builtin constructors. It does not match printable names and does not install
  a new global.
- A configured-target view carries complete occurrence identity and the full
  `ProviderCollection`. `target[Provider]` and `Provider in target` look up that
  identity. General occurrences use the shared provider classes. Every
  existing typed `ProviderValue` variant receives a phase-only evaluator view
  over its already-retained fields; this adds no second DICE payload. The view
  is exact only for fields already admitted by the typed V2 provider and fails
  closed for unrepresented Bazel fields. `DefaultInfo` lookup remains
  synthesized from the typed V2 value, rather than fabricated as an empty
  general occurrence.

The existing configured-analysis DICE producer remains sole semantic owner.
No key, global provider registry, process value store, cache, fallback lookup,
filesystem observation or lock is added.

## Compatibility classification

- **Exact:** for the admitted graph, Bazel 9.2 numeric/container equality,
  native struct equality and special frozen-container structural hashing,
  exported user-provider and `ToolchainInfo` equality/field behavior,
  provider identity lookup, depset construction-time leaf/type/order/depth
  validation, builder deduplication/hoisting, empty/single-transitive reuse,
  occurrence equality/hash and lazy `to_list()` order/deduplication including
  topological mixed-order and diamond behavior; configured-target lookup/membership for every
  exported callable identity already admitted by Slug; duplicate returned-
  provider rejection.
- **Slug-native:** Rust/Arc layout, process-local depset occurrence token,
  complete structural configuration/artifact identity, phase-only typed-
  provider views, publication-equality API, memory accounting and unproved
  diagnostic wording.
- **Unsupported/deferred:** evaluator functions and sets in retained provider
  payloads, mutable/cyclic result graphs, unowned opaque/native values, exact
  Bazel configuration/output bytes, Bazel provider fields not represented by
  the existing typed V2 providers, installation or construction semantics for
  deferred builtin provider callables, selected implementation analysis and
  `ctx.toolchains` behavior reserved for category 6.

BCR-delivered Starlark owns every rule definition and control path, including
`cc_internal`. `cc_common` and future C++ builtins are generic host-ABI clients
of this category-wide evaluator/value architecture, not parsers or Rust rule
engines. The Buck2-derived parser/evaluator remains the language substrate.

## Ownership, lifetime and revision behavior

`ConfiguredNodeResult` and its `ProviderCollection` retain the semantic graph
under the existing configured-analysis DICE key. Provider occurrences,
configured-target payloads, artifacts, depset occurrence tokens and DAG nodes
are DICE-retained semantic memory and release with result Arcs. No retained
node borrows evaluator or command memory.

Evaluator values, native container allocations, materialization/lowering memo
tables, visiting sets, lazy `to_list()` traversal/dedup sets and typed-provider
views are phase scratch. They drop on success, error or cancellation before
publication. No async work or lock crosses evaluator execution. Existing DICE
dependency recording means dependency provider edits recompute consumers; deep
publication equality prevents an equal target occurrence from cutting off a
changed provider payload. Request overlay/filesystem/lifecycle concerns are
inapplicable because this packet observes no new external input and adds no
request state.

There is no fallback. The R1 `MaterializedProvider`, `MaterializedToolchainInfo`,
`MaterializedStruct`, `MaterializedDepset` and eager flattened cache must be
deleted, not retained behind a compatibility branch.

## Required proof

Reuse the pinned source regressions above; no new Bazel fixture is needed.
Focused Rust proof must discriminate:

- every retained value kind; exact numeric Starlark versus publication domains;
  dictionary mapping equality versus iteration-sensitive publication equality;
  direct/nested key admission, including both a frozen list and a frozen
  dictionary containing `ToolchainInfo` behind the barrier inside both a native
  struct and an exported provider; direct `ToolchainInfo` fields and tuple-
  nested `ToolchainInfo` fields reject, as do top-level list/dict and tuple-
  with-list keys;
- fresh-to-fresh, fresh-to-frozen, frozen-to-rematerialized and fresh-to-
  rematerialized equality in both operand orders, plus equal hash, for native
  structs and exported providers; the same equality matrix and direct
  unhashability for `ToolchainInfo`; marker-era `ctx.toolchains` field behavior
  remains unchanged and its shared-class value compares equal in both operand
  orders to an equivalent directly constructed `platform_common.ToolchainInfo`;
- depset shared occurrence equality/hash, distinct equal-shaped inequality,
  same-order empty reuse, sole-transitive reuse, dependency-rematerialized
  transitive recomposition, no construction-time flatten, lazy `to_list()`,
  all admitted orders, the pinned `[3, 5, 6, 4, 2]` mixed-order topological
  vector, rightmost-conflict and deep-diamond sharing, empty/repeated/sole/two-
  child depth distinctions, direct deduplication and singleton hoisting,
  sole compatible different-order nonsingleton canonicalization with shared
  successor-array identity and publication equality against the equivalent
  direct canonical graph (not merely equal flattening),
  type/order/depth failures at the constructor call, and publication topology/
  alias discrimination; the discriminating topological and depth cases run
  through both a freshly constructed and dependency-rematerialized depset;
- configured-target `target[Provider]` and membership for admitted user,
  `ToolchainInfo`, `DefaultInfo`, `OutputGroupInfo` and `RunEnvironmentInfo`
  callables without printable-name collision; direct adapter tests also prove
  phase-only `FilesToRunProvider` and `PlatformInfo` projection without
  installing a new Starlark global; typed field access is limited to the
  existing retained payload;
- direct dependency round-trip of nested struct/provider/list/dict/depset,
  configured target and artifact values; A/B/A provider edits and same-mapping/
  different-dictionary-order edits; duplicate rejection; and deterministic
  unsupported/cycle errors with no evaluator heap retention; and
- unchanged executable/default-provider, action and marker-era consumers until
  category 6 deletes the marker bridge.

## Allowlist and caps

Rust baselines are blobs at `500f0f038`. Untracked R1 files are measured from
absence; the dirty candidate is not a new baseline.

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_build_api_v2/src/lib.rs` | `8091bec244d55c433a5179b8848a0f514a66d58a` / 50 | +25 |
| `app/slug_build_api_v2/src/analysis_value.rs` | absent / 0 | +1,250 |
| `app/slug_build_api_v2/src/depset.rs` | `a122f17aec87716a052181c3a835972d42a82d3c` / 253 | +240 |
| `app/slug_build_api_v2/src/providers/mod.rs` | `b191b11f8b9ee26f45a8558b257d90212c155c81` / 434 | +300 |
| `app/slug_build_api_v2/tests/analysis_value.rs` | absent / 0 | +650 |
| `app/slug_build_api_v2/tests/depset.rs` | `76f6ec58d30279b0d31d797dff63addb08df677f` / 170 | +200 |
| `app/slug_build_api_v2/tests/providers.rs` | `687ff27fee0b4d38fff286185206592a6ea1f872` / 209 | +200 |
| `app/slug_loading_v2/src/provider.rs` | `ab9129d352ce93e68c6d622a892746e4421f67c4` / 1,066 | +650 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `42a057175e094bdc712288062cc3124cb927f15f` / 35,115 | +220 |
| `app/slug_analysis_v2/src/lib.rs` | `9b4469cc2b5b8dc70904e2735e5f26fff148ba4e` / 88 | +15 |
| `app/slug_analysis_v2/src/analysis_value.rs` | absent / 0 | +900 |
| `app/slug_analysis_v2/src/key.rs` | `6a21efd96906bb843f229e3c39295028cf0013ae` / 270 | +35 |
| `app/slug_analysis_v2/src/starlark_rule.rs` | `2d97b0e2c47ca6316890ef00520fc9564f392bc6` / 797 | +250 |
| `app/slug_analysis_v2/src/dice.rs` | `28ae630557dcca41b31f6517050e07a17619c400` / 4,749 | +20 |
| `app/slug_analysis_v2/tests/configured_target.rs` | `3bc8da1ea5f1332e86203c2e6acedb0d74d9c827` / 827 | +45 |
| `app/slug_analysis_v2/tests/root_analysis.rs` | `4f2f443b2d5f2982128d0b6592b769cd1e5c8bc0` / 1,355 | +65 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `4791c4ad1294857bf8162f17f7ce27012a742cc6` / 6,988 | +550 |
| `starlark-rust/starlark/src/values/types/structs.rs` | `7d08daa2203814a43a09dd7c7164d47275fb9938` / 45 | +5 |
| `starlark-rust/starlark/src/values/types/structs/value.rs` | `b5671a957adfdda88cbb71153f91441cbb0e77c1` / 274 | +160 |

Net production growth is capped at +3,470 lines and proof growth at +2,050.
`analysis_value.rs` in build-api remains the sole retained recursive-value
owner; the new analysis module is conversion only. Loading `provider.rs` remains
cohesive because it owns provider/depset constructors and their shared evaluator
classes. The 35,115-line loading and 6,988-line analysis test files are only
integration proof surfaces. The 4,749-line DICE file may change only the
existing marker/accessor handoff. The vendored struct file may change only the
bounded internal structural-hash barrier and its focused tests; its existing
JSON serialization assertions must remain byte-for-byte unchanged.

No other file may change. In particular, do not edit parser/compiler syntax,
loading globals/package registration, configuration, query/server/CLI/action
code, BCR content or Zabel.

## Validation and stops

Run serially:

- `cargo fmt --all -- --check`;
- `cargo test -p starlark --lib values::types::structs::value --no-fail-fast`;
- `cargo test -p slug_build_api_v2 --no-fail-fast`;
- full `cargo test -p slug_loading_v2 --no-fail-fast`;
- full `cargo test -p slug_analysis_v2 --no-fail-fast`;
- `cargo test -p slug_core_v2 --no-fail-fast` as the direct cross-crate gate;
- `scripts/v2_archive_status.sh`, baseline-blob, allowlist, physical/net-cap,
  forbidden-wrapper/flatten-cache searches and `git diff --check`; and
- independent terminal retained-representation/evaluator review.

Rebuild `slug_cli_v2` before any binary smoke; no smoke is required unless an
existing server-observable result changes. Record independently reproduced
pre-existing failures separately and never weaken their assertions.

Return `REPLAN` before further Rust for a retained evaluator value/heap, global
provider/depset store or mutable interner, cross-class provider/depset/struct
equality, eager depset flattening, validation deferred past `depset()`, loss of
occurrence/DAG alias identity, printable-name provider lookup, missing typed
provider view, direct list/dict key admission, tuple-key weakening, per-builtin
arbitrary retained field struct, configuration/display/digest identity
substitution, parser/ruleset control flow, Zabel authority, or inability to
round-trip the admitted graph losslessly. A second material R3 contract
correction after implementation begins is another `REPLAN`.

Residual performance risk is per-node Arc allocation, recursive frozen-heap
materialization and uncached lazy depset traversal. No demonstrated Slug hot
path justifies a benchmark gate yet. The opaque retained handle and canonical
field APIs preserve a later measured dense-store or hash-cache optimization
without exposing Zabel/Buck2 layout as compatibility.
