# Current Slug V2 Packet

Packet: `WP-4-5-7A-recursive-analysis-value-provider-implementation`

Milestone: M7A category 5, one recursive retained analysis-value/provider
boundary before selected toolchain implementation analysis.

Base: `5ce967d55`, the independently accepted category-5 architecture. Implement
only that frozen contract under the exact file/cap table below. A material
identity, ownership, value-kind, hashability, semantic-immutability, depset or
publication-equality change is `REPLAN`, not an implementation adjustment.

Observable result: implement and prove one general, heap-independent provider-
value boundary that retains any number of named `ToolchainInfo` fields whose
values belong to the admitted graph, user-provider fields and future builtin-
provider payloads without a per-builtin value shape. No selected implementation
is analyzed and `ctx.toolchains` is not cut over in this packet.

## Learned facts and research basis

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority.

- `ToolchainInfo.java#copyValues` copies any number of keyword fields into a
  key-sorted Starlark map. `ToolchainInfoTest#toolchainInfoConstructor` proves
  string and configured-target fields; `toolchainInfo_equalsTester` proves
  structural value equality but does not independently vary construction
  order. Sorted-name behavior comes from `copyValues` and `StructImpl`, not
  that test.
- `StructImpl` defines provider-instance equality as provider identity plus
  field names and Starlark-equal values, and hashes fields in sorted-name
  order. `StarlarkInfoNoSchema` and `StarlarkInfoWithSchema` retain only present
  fields and expose sorted field names.
- `StarlarkInfoTest#equivalence` proves same provider plus equal fields is
  equal while distinct provider identities, values or present-field sets are
  unequal. Its concatenation tests remain evaluator behavior, not a retained
  store operation in category 5.
- `StarlarkProviderTest#schemafulProvider_withDepset` proves `None`, strings,
  arbitrary provider fields and typed depsets share the provider boundary;
  its immutable-provider tests prove mutable evaluator lists must not escape
  analysis publication.
- Bazel configured targets do not supply structural value equality; their
  Starlark identity is the configured target occurrence. Their provider
  payload must nevertheless participate in Slug publication equality so a
  DICE equality cutoff cannot retain stale materialization data.
- Bazel/Starlark numbers include arbitrary integers and floats. Equality and
  hashing equate an integer with an exactly equal integral float, equate both
  zero signs, and treat all NaNs consistently. The retained graph must not use
  Rust's derived `f64` equality or an `i64` narrowing.
- `Depset.java` retains one nullable top-level `ElementType` token (`empty`
  when no nonempty direct or transitive member fixes it), rejects a mismatched
  direct/transitive type, preserves order compatibility, and delegates equality
  and hashing to its underlying `NestedSet` occurrence. Empty depsets are
  shared per order and a constructor optimized to one unchanged transitive
  occurrence reuses that occurrence; independently constructed nonempty
  depsets remain unequal even when their flattened elements or DAG shape match.
  `DepsetTest#testEmptyGenericType`, `testHomogeneousGenericTypeTransitive`,
  `testBadGenericTypeTransitive`, `testEmptyDepsetInternedPerOrder` and
  `testSingleNonEmptyTransitiveAndNoDirectsUnwrapped` discriminate those cases.
- `StarlarkValue#checkHashable`, `StarlarkList`, `Dict`, `Tuple`,
  `AbstractConfiguredTarget` and `Depset` distinguish immutability from
  Starlark hashability. Frozen lists/dicts are never direct dictionary keys;
  tuples recurse through key hashability; configured targets and depsets are
  identity-hashable; and an exported immutable provider/struct uses its
  structural hash and may remain hashable while containing a frozen list or
  dictionary. Strict depset admission is different again: it requires
  semantic immutability, then independently rejects a top-level list/dict and
  enforces one top-level element type.
- Builtin `ToolchainInfo` extends `StructImpl` but does not override
  `StarlarkValue#isImmutable`, so its immutable Java/Rust storage does not make
  it semantically immutable: it is not a dictionary key or strict depset leaf,
  and a tuple/struct/user-provider field graph that recursively exposes it is
  likewise non-immutable. Frozen list/dictionary immutability remains Bazel's
  container-level exception and does not recursively inspect their children.

Live Slug facts:

- `slug_build_api_v2::providers` owns `ProviderId`, `ProviderCollection` and
  configured-result provider values, but `ToolchainInfo` is one marker string
  and `UserProvider` fields are `SmallMap<CompactString, CompactString>`.
- `slug_analysis_v2::starlark_rule` reconstructs dependency providers on an
  evaluator heap and lowers only string user providers, marker
  `ToolchainInfo`, and direct-only `DefaultInfo` depsets.
- `slug_loading_v2::provider` already retains authenticated exported provider
  identity and evaluator-generic schemaful/schemaless field values. The
  configured string fast path is the narrowing seam, not a second semantic
  owner.
- `Depset<T>` is already an immutable `Arc` shared DAG with all four admitted
  orders and a depth bound, but its derived equality is structural and it has
  no retained top-level Starlark element type. Category 5 must therefore wrap
  its storage rather than expose `Depset<AnalysisValue>` directly.
  `SlugConfiguration::canonical_bytes()` is the full collision-free structural
  configuration identity; its BLAKE3 projection and display spelling are not
  suitable configured-target value identity.

Prior art classification:

- Buck2's `ProviderCollectionGen` supplies **concept/test only** guidance for
  authenticated provider IDs, `SmallMap`, borrowed-key lookup, `Hashed`,
  `Dupe` and `Allocative`. Its retained values are `FrozenValue` backed by a
  `FrozenHeap`, so that storage design is explicitly **avoided**.
- Existing V2 `Depset`, `SmallMap`, `CompactString`, immutable `Arc` slices,
  `Dupe`, arbitrary `BigInt` and `Allocative` are **leaf reuse**. No new
  interner, weak hash, packed store or Buck/V1 import is authorized.
- Clean Zabel commit
  `0795445f3ab60f4e49070bdd0b94425c5610f73a` is **concept/test only** peer
  guidance. Its producer-owned value roots, separation of semantic provider
  keys from dense row IDs, shared depset references, evaluator-lowering
  boundary, dictionary alternating storage and rejection of provider-specific
  ordinal shortcuts inform this design. Copy no Zig layout, arena/store ID,
  packed row, scheduler, fingerprint or compatibility claim.

## Frozen decision

### Opaque value and occurrence handles

`slug_build_api_v2` owns these public retained concepts:

- `AnalysisValue`: an opaque, `Dupe`/`Allocative` cheap-clone handle. Its first
  implementation is an immutable `Arc` graph, but callers receive accessors and
  a borrowed kind view rather than the backing enum. A future measured dense
  owner store may replace the internals without changing provider consumers.
- `AnalysisValue::is_starlark_immutable()`: the Bazel semantic predicate,
  explicitly separate from immutable retained storage. Scalars, labels,
  targets, artifacts and depsets are true; retained frozen lists/dictionaries
  are true without recursing; tuples recurse; structs/exported user providers
  recurse through fields; builtin `ToolchainInfo` is false. No caller may infer
  this predicate from Arc ownership or Rust mutability.
- `AnalysisNumber`: arbitrary `BigInt` or exact `f64` bits with Bazel/Starlark
  numeric equality and hash behavior, including integral int/float equality,
  signed zero and NaN.
- distinct immutable list and tuple sequences; an insertion-ordered dictionary
  with order-independent mapping equality; and a key-sorted string-field map
  shared by structs and provider occurrences.
- `AnalysisArtifact`: source identity by canonical label, or derived identity
  by full configured owner identity plus the existing `ActionOutput`. This is
  Slug's structural artifact identity and makes no Bazel output-path claim.
- `ConfiguredTargetValue`: full configured-target identity plus an immutable
  provider collection. Production configuration identity copies the complete
  canonical byte slice, never a checksum, projection or display token. The
  existing legacy configuration variant receives a separately tagged complete
  test-only encoding.
- `AnalysisDepset`: an opaque occurrence handle over the existing shared DAG,
  with its order and a canonical top-level `AnalysisValueType` token retained
  separately. `Empty` unifies with the first nonempty direct/transitive type;
  otherwise types must match exactly. The token follows the evaluator's
  top-level Starlark type rather than a Rust enum discriminant or provider
  identity. Lowering preserves source occurrence reuse and DAG sharing without
  flattening; materialization memoizes by this handle so two references to one
  occurrence stay equal while independently constructed equal-shaped
  occurrences stay unequal.
- `ProviderIdentity`: builtin provider name or authenticated exported
  `ProviderId`; and `ProviderOccurrence`: that identity plus one key-sorted
  field map. Empty occurrences are valid.

The first admitted value kinds are `None`, Boolean, integer, float, string,
canonical label, configured target, artifact, list, tuple, dictionary, struct,
provider occurrence and depset. This is the full category-5 shape; adding a
future host builtin does not add a new retained field struct. Functions,
mutable sets, opaque evaluator values, actions, command lines and unowned
native objects remain unsupported/deferred until a packet names their
identity, lifetime and evaluator semantics. The evaluator-global `set`
builtin remains outside this store.

`ProviderValue` retains the existing operationally typed builtin providers
(`DefaultInfo`, `OutputGroupInfo`, `RunEnvironmentInfo`,
`FilesToRunProvider`, and `PlatformInfo`) and one general
`ProviderOccurrence` variant. Both builtin `ToolchainInfo` and exported user
providers use the general occurrence path. `ProviderCollection` becomes an
opaque Arc-backed compact map keyed solely by `ProviderIdentity`; duplicate
identity remains an error and `DefaultInfo` validation remains unchanged.
Existing typed builtin internals are not generalized in this packet, but no
new builtin-specific payload representation is permitted.

### Two explicit equality domains

`AnalysisValue`'s Bazel-visible equality/hash domain follows Starlark:

- numeric cross-kind equality follows Bazel/Starlark rather than enum tags;
- list/tuple order and type are significant;
- dictionaries compare mappings independent of insertion order while
  preserving insertion order for iteration;
- structs and provider occurrences compare fields independent of construction
  order; provider identity is significant;
- configured targets and artifacts compare their complete occurrence
  identity; and
- depsets compare and hash only by retained occurrence identity. Reusing one
  occurrence is equal; separately constructed occurrences are unequal even
  when type, order, DAG and leaves are otherwise identical. Flattening still
  deduplicates leaves with the same Starlark equality domain. The first Arc
  implementation uses handle identity for this process-local Starlark
  equality/hash only; its address is never publication identity or serialized
  semantic state.

Separately, retained-publication equality recursively includes every owned
payload needed to materialize the value again. In particular, two configured
target values with equal occurrence identity but different provider
collections are Starlark-equal but publication-unequal. `ProviderCollection`
and therefore `ConfiguredNodeResult` use publication equality for DICE cutoff.
Dictionary publication equality compares the ordered key/value sequence, not
only the mapping: equal mappings with different insertion order are
Starlark-equal but publication-unequal because iteration is observable.
Depset publication equality performs an unflattened, bidirectional graph-
isomorphism comparison over order, top-level type, ordered direct leaves,
ordered transitive edges, deeply publication-equal leaf payloads and the
occurrence/alias partition. Thus the same child referenced twice is not
publication-equal to two distinct equal-shaped children, while independently
allocated A and A' graphs with the same complete topology may cut off safely.
Comparison scratch is pair-memoized and phase-local; no process ordinal,
pointer address or traversal-assigned ID becomes semantic identity. No
weak/precomputed hash is semantic identity; any later `Hashed` use is
lookup-only.

### Hashability and depset admission

The retained API exposes a fallible Starlark-key hash separately from a total
internal structural hash used to hash an otherwise hashable struct/provider
field graph. For the admitted graph, exact dictionary-key behavior is:

- `None`, Boolean, integer, float, string, label, configured target, artifact
  and depset are accepted; configured targets and depsets use occurrence hash;
- tuples are accepted only when every element independently passes this same
  dictionary-key check;
- structs and authenticated exported provider occurrences are accepted when
  `is_starlark_immutable()` is true, including when a field is a retained frozen
  list or dictionary; their sorted-field structural hash uses the total
  internal hash of those children; and
- list, dictionary and builtin `ToolchainInfo` are rejected directly. A tuple,
  struct or user provider that reaches `ToolchainInfo` through semantic-
  immutability recursion (without crossing a frozen list/dictionary) is
  rejected too. Sets/functions/opaque values are already outside the admitted
  graph. No rejected direct kind is admitted merely because retention made its
  storage immutable.

The total internal hash remains consistent with Starlark equality: list/tuple
hashing is ordered, dictionary hashing is mapping-based and insertion-order
independent, and struct/provider fields hash in sorted-name order. It is not a
public permission to place list/dictionary values directly in a dictionary.

Strict depset-leaf admission is a distinct exact matrix. It first requires
`is_starlark_immutable()`, then rejects a top-level list or dictionary. Thus
tuples and exported provider/struct occurrences containing retained frozen
lists or dictionaries remain legal, while builtin `ToolchainInfo` and a
tuple/struct/user provider that reaches it through semantic-immutability
recursion (without crossing a frozen list/dictionary) are rejected. It also
rejects every unsupported kind, incompatible order, a nonempty direct/
transitive `AnalysisValueType` mismatch and the existing depth overflow. Empty
direct and transitive children contribute no type; the first nonempty child
fixes it. This validation occurs while composing the shared DAG and never by
flattening.

### Evaluator boundary

One analysis-owned lowerer converts returned evaluator graphs to
`AnalysisValue` before the evaluator/module heap drops. It:

1. recognizes every admitted scalar and container, loading-owned user-provider
   occurrence, builtin `ToolchainInfo`, analysis label/configured-target and
   declared-artifact adapter, and existing depset occurrence/type/order/DAG;
2. uses evaluator `ValueIdentity` only in phase scratch for a visiting set and
   memo table, preserving acyclic DAG sharing and rejecting cycles;
3. retains field names and dictionary iteration order, validates the frozen
   dictionary-key and depset-leaf matrices above, and reports the first
   unsupported value path; and
4. publishes no raw `Value`, `FrozenValue`, heap, pointer identity, scratch
   memo or evaluator lifetime.

The inverse adapter allocates evaluator-facing immutable views from retained
handles. Provider/struct attribute access and list/tuple/dict iteration plus
depset `to_list()` delegate to the same retained nodes; depsets themselves
remain non-iterable. Its memo table rematerializes each retained depset
occurrence once per evaluator heap, preserving shared-versus-distinct identity.
A configured-target view carries its full identity and provider collection, so
`target[Provider]`, membership and field access do not stringify or copy the
occurrence. Repeated accesses may allocate evaluator wrappers but always
`Dupe` the same retained handle.

Schemaful and schemaless provider constructors keep schema validation in
`slug_loading_v2`, then produce one evaluator-generic occurrence shape. The
configured string shortcut is deleted. `platform_common.ToolchainInfo`
accepts zero or more named fields whose values belong to the admitted graph and
uses the same generic occurrence path; positional arguments and values outside
that graph remain rejected.

## Ownership, lifetime and revision behavior

The existing configured-analysis DICE producer remains the sole semantic
owner. `ConfiguredNodeResult` retains the Arc-backed `ProviderCollection`;
dependency results are prepared before synchronous evaluator entry. No DICE
key, global provider registry, process store, query cache or new lock is added.

Evaluator values, conversion stacks, pointer-identity and publication graph-
comparison memo tables, and wrapper allocations are phase scratch and drop on
success, error or cancellation. `AnalysisValue`, provider occurrences,
configured-target values, depset occurrence handles and DAG nodes are DICE-
retained semantic memory and release with their result Arcs.
No retained node borrows command scratch or an evaluator heap. Existing DICE
dependency recording and request isolation govern edits: a dependency provider
change recomputes its consumers, and publication equality prevents a stale
embedded provider payload from being cut off. No lock spans a DICE compute.

## Compatibility classification

- **Exact:** the admitted Bazel 9.2 value kinds' Starlark equality/hash shape,
  the dictionary-key and strict depset-leaf matrices above, list/tuple/
  dictionary/struct distinctions, dictionary iteration order, provider
  identity plus field equality, depset element-type/order/occurrence behavior,
  zero or more `ToolchainInfo` field names whose values belong to the admitted
  graph, configured-target provider access, and rejection of duplicate returned
  provider identities under the named source regressions.
- **Slug-native:** Rust layout, Arc graph, complete structural configuration
  bytes used for configured-target occurrence identity, artifact owner/output
  identity, publication-equality API, memory accounting and unproved
  diagnostic wording.
- **Unsupported/deferred:** evaluator functions and sets as retained provider
  values, mutable/cyclic graphs, unowned opaque/native values, exact Bazel
  configuration/output bytes, broader typed builtin-provider internals, and
  selected implementation/`ctx.toolchains` behavior reserved for category 6.

BCR-delivered Starlark owns all rule definitions and control flow, including
`cc_internal`. `cc_common` and future C++ builtins are generic host-ABI clients
of this representation, not parsers or Rust rule engines. The Buck2-derived
parser/evaluator remains the language substrate.

## Implementation evidence

No new Bazel fixture is needed: the pinned source/tests above discriminate the
representation contract, and category 6 owns the public selected-toolchain
oracle. The implementation packet must add focused Rust regressions for:

- every value kind, int/float/zero/NaN equality and hash, list-versus-tuple,
  dictionary same-mapping/different-order Starlark equality plus publication
  inequality, every admitted/rejected direct dictionary-key kind, recursive
  tuple-key rejection, provider-with-frozen-list and provider-with-frozen-
  dictionary key acceptance when the frozen container contains
  `ToolchainInfo`, and direct plus tuple/struct/provider-nested `ToolchainInfo`
  key rejection, sorted struct/provider fields, and shared Arc nodes;
- depset shared occurrence equality/hash, distinct equal-shaped occurrence
  inequality, same-order empty occurrence reuse, single-transitive occurrence
  reuse, empty/direct/transitive element-type unification, heterogeneous and
  incompatible-order rejection, the exact admitted/rejected leaf matrix
  including direct and recursively exposed `ToolchainInfo` plus separately
  proven frozen-list and frozen-dictionary barriers, unchanged DAG sharing
  without flattening, and publication comparison of order/type/topology/
  aliasing/deep leaf payload;
- builtin/user provider identity, empty/admitted-field `ToolchainInfo`, nested
  provider/struct/list/dict/depset values, field-order-independent equality,
  duplicate-provider rejection and publication inequality for changed embedded
  target providers;
- analysis lowering/materialization across a direct dependency, including a
  configured-target field and declared artifact, A/B/A provider edits, an
  A/B/A same-mapping/different-dictionary-order dependency-provider transition,
  depset occurrence reuse versus separation after rematerialization, no
  evaluator heap retention, and deterministic errors for cycles and each
  unsupported kind; and
- unchanged `DefaultInfo`, executable and marker-era direct dependents until
  category 6 deletes the marker bridge.

The exact allowlist and physical-growth caps at base `5ce967d55` are:

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_build_api_v2/Cargo.toml` | `da603e2a2a9468e18f67290332e2a9f6f4fb0574` / 15 | +5 |
| `app/slug_build_api_v2/src/lib.rs` | `8091bec244d55c433a5179b8848a0f514a66d58a` / 50 | +20 |
| `app/slug_build_api_v2/src/analysis_value.rs` | absent / 0 | +1,100 |
| `app/slug_build_api_v2/src/providers/mod.rs` | `b191b11f8b9ee26f45a8558b257d90212c155c81` / 434 | +250 |
| `app/slug_build_api_v2/tests/analysis_value.rs` | absent / 0 | +500 |
| `app/slug_build_api_v2/tests/providers.rs` | `687ff27fee0b4d38fff286185206592a6ea1f872` / 209 | +120 |
| `app/slug_loading_v2/src/provider.rs` | `ab9129d352ce93e68c6d622a892746e4421f67c4` / 1,066 | +350 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `42a057175e094bdc712288062cc3124cb927f15f` / 35,115 | +160 |
| `app/slug_analysis_v2/src/key.rs` | `6a21efd96906bb843f229e3c39295028cf0013ae` / 270 | +35 |
| `app/slug_analysis_v2/src/starlark_rule.rs` | `2d97b0e2c47ca6316890ef00520fc9564f392bc6` / 797 | +350 |
| `app/slug_analysis_v2/src/dice.rs` | `28ae630557dcca41b31f6517050e07a17619c400` / 4,749 | +15 |
| `app/slug_analysis_v2/tests/configured_target.rs` | `3bc8da1ea5f1332e86203c2e6acedb0d74d9c827` / 827 | +30 |
| `app/slug_analysis_v2/tests/root_analysis.rs` | `4f2f443b2d5f2982128d0b6592b769cd1e5c8bc0` / 1,355 | +50 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `4791c4ad1294857bf8162f17f7ce27012a742cc6` / 6,988 | +250 |

Net production growth is capped at +1,800 lines and proof growth at +1,000.
The new `analysis_value.rs` is the sole recursive retained-value owner;
`providers/mod.rs` remains the collection/typed-builtin owner. The 35,115-line
loading host test, 4,749-line analysis DICE owner and 6,988-line integration
test cross complexity triggers, but their authority is respectively focused
constructor/depset proof, at most the marker-bridge accessor migration, and
focused direct-dependency/A/B/A proof. Splitting any of them would widen this
packet without moving a semantic owner.

No other file may change. In particular, do not edit the existing generic
`depset.rs`, loading package/global definitions, configuration representation,
core/server/CLI/query/action code, starlark-rust, BCR content or Zabel. Direct
consumer strings that already call `ToolchainInfo` require no fixture edit.
Compilation-demonstrated constructor/accessor migration is confined to the
listed proof files and the two listed analysis production consumers.

Validation after implementation: `cargo fmt --all -- --check`;
`cargo test -p slug_build_api_v2 --no-fail-fast`; full
`cargo test -p slug_loading_v2 --no-fail-fast` because both provider
constructor families and the evaluator depset boundary change; full
`cargo test -p slug_analysis_v2 --no-fail-fast` including direct-dependency/
A/B/A proof; and `cargo test -p slug_core_v2 --no-fail-fast` as the direct
cross-crate gate. Run Cargo commands serially. Rebuild `slug_cli_v2` before any
binary smoke; no smoke is required unless a changed result is server-observable.
Then run `scripts/v2_archive_status.sh`, baseline-blob, allowlist, physical/net-
line-cap and `git diff --check` audits and independent terminal review. Broad
server validation belongs to category 6 unless category 5 changes an existing
server-observable provider result.

## Stops and residual risk

Return `REPLAN` before Rust for a raw/frozen evaluator value, provider heap,
global mutable interner/store, display/checksum/digest identity, missing deep
publication equality, mapping-only dictionary publication equality, structural
Starlark depset equality, missing depset type/alias preservation, conflated
hashability/semantic-immutability/storage-immutability, per-builtin arbitrary
field struct, flattened depset, unbounded copied field graph, parser/ruleset
control flow, Zabel authority, or an implementation that cannot round-trip the
admitted graph without semantic loss. A second material contract correction
after implementation begins is also `REPLAN`.

Residual performance risk is per-node Arc allocation and wrapper
materialization. The opaque handle deliberately permits a later measured dense
store without API churn. Zabel's measurements justify small compact fields and
reject provider-specific shortcuts, but do not establish a Slug performance
claim; no benchmark gate applies until a demonstrated Slug hot path exists.

Immediate predecessor: category 4 was accepted in `568b0c698`, with 50 server
passes plus the unchanged inherited event-replay failure and 291 core passes,
one ignored test and the unchanged inherited external-query event failure.
