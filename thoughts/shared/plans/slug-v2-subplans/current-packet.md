# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-context-attribute-namespace-implementation-r2`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: independently accepted repository-context architecture `47c937942`,
terminally accepted module-loaded native-context implementation `1f9433600`,
accepted complete thirteen-kind repository-rule attribute carrier `0c3a172ed`,
accepted effective repository Host inputs `64878a1be`, and accepted generic
canonical repository Host capability `26a68d61c`. All unrelated dirty analysis,
loading, core, and REAPI work remains parked and read-only.

The recoverable uncommitted R1 candidate changes only the two allowed Rust
owners and measures 290 production, 197 proof, and 487 aggregate net lines. Its
focused context and selected-effect tests pass, but root review rejects
acceptance because it does not prove the frozen ordinary/innate key-level A/B/A
matrix. Preserve the candidate while correcting proof capacity; no R1 Rust is
accepted or staged by this replan.

## Observable result

Implement the independently accepted bounded contract for Bazel 9.2's
value-bearing `repository_ctx` category: direct immutable `name` and
`original_name` fields plus one read-only `attr` namespace over all thirteen
already-admitted repository-rule attribute kinds. The same authenticated input,
effective-value precedence, recursive allocator, and evaluator value must serve
ordinary module-extension and innate `use_repo_rule` generated repositories.

The successor must advance two fresh rules_rust replays through the authentic
`@@bazel_features+//private:globals_repo.bzl:25` use of
`rctx.attr.globals.items()`. The requested downstream
`@@rules_cc++compatibility_proxy+cc_compatibility_proxy//:symbols.bzl` route is
an integration discriminator only. Do not implement `cc_common`, `cc_internal`,
C++ rules, parser behavior, `set`, or a ruleset-specific repository shortcut.

## Semantic authority and source trace

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority:

- `StarlarkRepositoryContext.java:123-172` publishes `name`, `original_name`,
  `workspace_root`, and `attr`. `original_name` falls back to canonical `name`
  when null or empty. `StarlarkBaseExternalContext` owns `os` and effect methods.
- `RepoDefinition.java:42-66` is the immutable attribute structure. Lookup is
  special `name`, then explicitly instantiated value, then declaration default,
  then absence. Its field inventory is declaration schema union `name`; unknown
  access fails and Bazel adds a spelling suggestion when available.
- `AttributeUtils.java:61-119` discards every Starlark `None` before unknown,
  mandatory, conversion, and default processing. A mandatory value supplied as
  `None` is therefore missing; an optional `None` falls through to its declared
  or implicit default, and even an unknown `None` is ignored.
- `RepoRule.java:92-119` retains only explicitly supplied non-`None` values in
  `RepoSpec`. `RepoDefinitionFunction.java:250-272` re-instantiates imported or
  registry-produced `RepoSpec`s through the same `RepoRule.instantiate` path,
  so a retained `OverrideAttributeValue::None` from any producer also means
  absence rather than a published explicit value.
- `AttributeUtils.java:105-120`, `Attribute.java:1835-1844`, `Type.java:334-680`,
  `BuildType.java:48-132`, and `Types.java:33-53` establish typed default
  publication: scalar labels/outputs default to null, strings to `""`, booleans
  to `False`, integers to `0`, and list/dict families to empty containers.
- `RepoDefinitionFunction.java:250-272` supplies canonical repository `name` and
  the module-extension internal generated name as `originalName`; module-backed
  repository definitions may supply no original name and use the fallback.

There is no narrower upstream test that covers the complete cross-product of
all thirteen kinds, both generated-repository owners, and Slug's DICE
restoration boundary. Pinned Bazel source establishes the contract; focused
Slug tests cover the representation and the real rules_rust replay covers the
first public consumer. No new checked-in oracle fixture is justified.

The two fresh post-`1f9433600` rules_rust replays no longer contain the former
`native.bazel_version` error. Both execute the authenticated `bazel_features`
`globals_repo` through line 25 and fail because Slug's `repository_ctx` lacks
`attr`. The outer missing compatibility repository is aggregate fallout from
that earlier generated-repository failure.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
**concept/test only** guidance. Its observed repository-rule execution host
uses one invocation capability, a separate read-only `repository_ctx.attr`
value, special `attr.name`, explicit-before-default lookup, and adjacent
`name`/`original_name` fields. This corroborates category and ownership shape.
Copy no Zig code, representation, preflight, effect implementation, cache, or
compatibility claim.

## Existing semantic owners and shared handoff

Both routes already converge before repository-rule execution:

| Route | Authenticated producer path | Shared retained repository |
|---|---|---|
| Ordinary module extension | `HostSelectedExtensionOwnerCertificateKey` -> pure invocation receipt -> `instantiate_request` -> validation | `HostInstantiatedModuleExtensionRepository` |
| Innate `use_repo_rule` | `HostSelectedExtensionOwnerCertificateKey` -> `HostPureInnateRepositoryOwnerKey` -> `instantiate_innate_request` -> validation | `HostInstantiatedModuleExtensionRepository` |

`HostSelectedExtensionOwnerCertificate::repository(ordinal)` erases that route
difference and returns the same retained repository. It owns the generated
name, canonical `CanonicalRepoName`, authenticated `RepositoryRuleCallRecord`,
and canonicalized `RepoSpec`. `RepoSpec.attributes` is the sole explicit-value
owner after repository mapping and label coercion. The authenticated
`call.definition.attributes` slice is the sole schema/kind/mandatory/default
owner. `HostSelectedRepositoryFileEffectKey` and its observation key already
depend on the complete certificate and therefore remain the sole DICE effect
owners; add no key or cache.

## Frozen implementation architecture

### Invocation input and failure boundary

Add one loading-private `RepositoryRuleInvocationInput`, constructed in
`module_extension_repository_file_effect.rs` only after `authenticate_rule`
succeeds and before `invoke_repository_rule` creates its evaluator or effect
builder. It owns only cheap immutable handles and names:

- canonical repository name copied into one `CompactString` from
  `CanonicalRepoName::as_str()`;
- `Option<CompactString>` original name, populated from the generated name for
  ordinary and innate extension repositories and permitting `None` for the
  Bazel module-backed fallback shape;
- a cloned `Arc<SmallMap<CompactString, OverrideAttributeValue>>` from the
  authenticated `RepoSpec`; and
- a cloned `Arc<[RepositoryRuleAttribute]>` from the authenticated definition.

Its fallible constructor first treats every retained
`OverrideAttributeValue::None` as absent, matching Bazel before any unknown or
mandatory check. It then preflights schema-name uniqueness, membership of each
remaining explicit field, mandatory presence, and every remaining
explicit/default value's recursive shape against its declared kind. A missing
mandatory field never falls through to a kind default. It does not coerce, map,
stringify, sort, filter into a second map, or copy a nested container. A
projection error returns before `RepositoryRuleInvocationState` exists, so no
file effect can be staged. The authenticated instantiation path should make
this error unreachable in production, but the boundary remains fail-closed for
any future `RepoSpec` producer.

`invoke_repository_rule` receives this input by value and installs it in the
evaluation-owned `RepositoryRuleContext`. `name` is the canonical string.
`original_name` is the nonempty supplied original or canonical `name` when the
option is absent/empty. `attr.name` is the same canonical string.

### Single non-retained value view and allocator

Do not convert declared defaults into `OverrideAttributeValue`, build an
effective attribute map, or introduce a second retained publication identity.
Define one private, copyable `RepositoryAttributeValueRef<'a>` view whose
variants describe `None`, bool, int, borrowed string, borrowed canonical label,
borrowed iterable, and borrowed ordered map. Small iterator adapter enums expose
the existing recursive shapes without allocation:

- explicit source: `&OverrideAttributeValue` and its `SmallMap` entries;
- declared-default source: `&CoercedAttributeValue` and its typed `Arc` slices;
  selectors/concatenations are rejected by preflight because repository-rule
  defaults are already resolved; and
- implicit source: one static scalar or empty iterable/map view selected from
  `AttributeKind`.

One recursive `allocate_repository_attribute_value(view, heap)` function is
the sole evaluator projector. It allocates strings, lists, dicts, `None`, bool,
int, and existing `StarlarkLabel` values directly on the invocation module
heap. Label values and label dictionary keys use their already-canonical
`CanonicalLabel`; no repository mapping or display-string round trip occurs.
Iterable order and each existing `SmallMap`/typed-map iteration order are
preserved exactly. Allocation is infallible after preflight and no evaluator
value escapes `invoke_repository_rule`.

### Effective lookup and namespace behavior

Add one `RepositoryRuleAttributes` Starlark value holding the same immutable
invocation input handles. Lookup order is exact and does not memoize:

1. `name` -> canonical repository string;
2. non-`None` explicit `RepoSpec.attributes[name]`; retained `None` continues;
3. authenticated declaration's typed default;
4. the kind-specific implicit value below;
5. unknown -> no attribute.

Its field inventory is declaration order plus `name`, with duplicate `name`
removed. Starlark-rust sorts the combined `dir()` result, so direct access,
`hasattr`, `getattr`, and `dir` agree without a sorted retained index. Unknown
access fails and the runtime derives any spelling suggestion from that exact
inventory. Success/failure and suggestion candidate are exact; Rust type names
and punctuation remain Slug-native.

`RepositoryRuleContext::get_attr` adds only `name`, `original_name`, and `attr`
beside the existing `os`; `dir_attr` returns those four direct fields. Existing
method discovery is merged and sorted by starlark-rust. The values are immutable
because neither context nor attribute namespace exposes mutation operations.

### Complete thirteen-kind publication matrix

In every row, a non-`None` explicit value wins, a retained explicit `None`
means absence, a declared typed default wins over the implicit value, and
container order is source iteration order.

| `AttributeKind` | Explicit/default source shape | Published Starlark value | Implicit when no declared default |
|---|---|---|---|
| `String` | string | string | `""` |
| `Boolean` | bool | bool | `False` |
| `Integer` | signed `i32` | int | `0` |
| `Label` | explicit canonical label; default canonical label or `None` | `StarlarkLabel` or default `None` | `None` |
| `Output` | explicit canonical label; default canonical label or `None` | `StarlarkLabel` or default `None` | `None` |
| `StringList` | ordered strings | list of strings | `[]` |
| `LabelList` | ordered canonical labels | list of `StarlarkLabel` | `[]` |
| `OutputList` | ordered canonical labels | list of `StarlarkLabel` | `[]` |
| `StringDict` | ordered string -> string | dict of string -> string | `{}` |
| `StringListDict` | ordered string -> ordered strings | dict of string -> list of strings | `{}` |
| `StringKeyedLabelDict` | ordered string -> canonical label | dict of string -> `StarlarkLabel` | `{}` |
| `LabelKeyedStringDict` | ordered canonical label -> string | dict of `StarlarkLabel` -> string | `{}` |
| `LabelListDict` | ordered string -> ordered canonical labels | dict of string -> list of `StarlarkLabel` | `{}` |

Any retained explicit `OverrideAttributeValue::None` means absence before
membership, mandatory, lookup, or kind dispatch. A mandatory attribute then
fails as missing; an optional attribute uses its declared default or the matrix
implicit value; an unknown `None` is ignored. This applies equally to generated,
imported, registry, and future authenticated producers. Any other
explicit/default shape mismatch is a pre-invocation projection error, never an
implicit default.

## Identity, revision, memory, and effects

The certificate's existing structural equality already includes canonical
name, generated name, authenticated call definition/defaults, and `RepoSpec`.
The accepted `RepoSpecPublicationIdentity` additionally preserves nested
dictionary order while making top-level attribute insertion order
nonsemantic. Thus explicit-value, schema/default, nested-order, and name changes
invalidate the existing effect key naturally; A/B/A restores the original
semantic value with no new key field or side dependency.

Repository source/mapping/lockfile observations remain upstream dependencies of
the certificate. This packet adds no request overlay, direct filesystem read,
historical lookup, final-validation phase, retry mode, or overlapping-request
policy. Existing DICE dependency recording and equality cutoff apply unchanged.

The input's cloned `Arc`s and names are phase/evaluator scratch released when
`invoke_repository_rule` returns. Nested semantic values remain DICE-retained
only through their existing certificate/`RepoSpec` owners. Published Starlark
values are invocation-heap scratch. No retained value borrows that heap. Add no
interner, cache, lock, async transfer, task, cancellation, join, eviction, or
shutdown owner.

The Buck2-derived utility decision is **retain existing utilities**: use
`SmallMap` for compact deterministic repository maps, `CompactString` for the
two copied names, `Arc` plus `Dupe`/cheap clones for shared immutable carriers,
and preserve `Allocative` on owned structs. Add no `HashMap`, `BTreeMap`, owned
nested `Vec`, stringified projection, precomputed weak hash, strong hash,
global interner, dependency, or Stage 9 donor import. The Stage 4/5 extraction
ledger stays unchanged because this is a V2-owned evaluator adapter over
already-adopted utilities.

Effect staging remains transactional: `RepositoryRuleInvocationState` owns the
builder, `finish` publishes a plan only after a `None` result, and every error
drops the state. Preflight projection errors happen before state creation;
unknown-field or other evaluation errors may occur after calls in source order
but no staged plan escapes. No new rollback or filesystem operation is needed.

## Compatibility classification

- **Exact:** Bazel 9.2 membership and values for `name`, `original_name`, and
  `attr`; original-name fallback; `attr.name`; explicit/default/implicit
  precedence including pre-validation `None` elision; all thirteen kinds;
  recursive order; canonical label objects and keys; direct access/`hasattr`/
  `getattr`/`dir`; unknown-field failure and
  suggestion candidate; ordinary/innate parity; unchanged transactional file
  effects.
- **Slug-native:** Rust helper/type names, non-retained iterator adapter shape,
  evaluator allocation mechanics, runtime type-name/diagnostic punctuation,
  retained `Arc`/`SmallMap` layout, and DICE display wording.
- **Unsupported/deferred:** `workspace_root`; repository path objects and
  filesystem reads; `download`, `download_and_extract`, `execute`, `extract`,
  `patch`, `template`, `symlink`, `delete`, `read`, `path`, watch/progress and
  other effect methods; `repo_metadata`; Windows repository execution; exact
  Java object identity/UTF-16 edges; parser/`set`; `cc_common`, `cc_internal`,
  C++ providers/toolchains/actions; and the next boundary after fresh replays.

Do not expose repository attributes in module-extension or BUILD contexts,
make the namespace writable, infer values from the demanded ruleset, flatten
containers, use display strings for labels, or claim the full `repository_ctx`
API from this category.

## Implementation proof matrix

| Proof | Ordinary | Innate | Required observation |
|---|---:|---:|---|
| All kinds | yes | shared allocator | non-`None` explicit scalar/label/list/map values and all implicit defaults match the table; declared defaults override implicit values |
| `None` normalization | yes | yes | optional `None` falls through to declared/implicit default, mandatory `None` fails missing, and unknown `None` is ignored before projection |
| Label identity/order | yes | shared allocator | scalar/list/map-key/map-value/nested labels remain `StarlarkLabel`; list and nested-map order appear in written content |
| Namespace/reflection | yes | yes | `name`, fallback/nonfallback `original_name`, `attr.name`, direct access, `hasattr`, `getattr`, sorted `dir`, immutability, and unknown/suggested failure agree |
| Explicit values A/B/A | yes | yes | warm same-DICE changes to explicit scalar and collection values produce A/B/A plan restoration |
| Declaration/default A/B/A | yes | yes | changing only declaration kind/default invalidates, changes publication, and restores A/B/A |
| Nested order A/B/A | yes | yes | reordering only nested map entries invalidates and restores; reordering top-level kwargs remains equal |
| Name A/B/A | yes | yes | canonical and generated/original-name changes participate in the existing certificate/effect identity and restore |
| Projection failure | yes | yes | malformed test-only explicit/default shape fails before invocation-state construction and yields zero effects |
| Evaluation failure | yes | shared state | a file call followed by unknown attribute/error publishes no plan |
| Owner convergence | yes | yes | both certificate kinds hand the same input type to the same evaluator value and allocator; no route branch exists in the context |
| Real replay | yes | as reached | two fresh roots pass `globals_repo.bzl:25`, agree on next authentic terminal, and introduce no ruleset-specific code |

Use existing test-only module/owner builders and DICE transactions. Ordinary
and innate rows must exercise `HostSelectedRepositoryFileEffectKey`, not merely
the pure allocator. Direct allocator tests may cover the exhaustive matrix once;
both routes still require at least one discriminating nested/default/name
end-to-end case and no-effect failure. Add no copied registry, fixture manifest,
checked-in output, or Bazel mutation.

R2 must reuse the existing test-only
`module_extension_repository_instantiation::tests::transaction_untracked`
source updater and, where its built-in graph seed is required, the existing
`canonical_repository_route_tests::tests::transaction`. Both accept replacement
MODULE/extension source on the same `Dice` instance, so ordinary and innate
tests can drive warm A/B/A without a new helper or production dependency. One
compact transition may change explicit scalar/collection, declaration/default,
nested order, and generated/original name together only if separate assertions
show that each dimension changes the written plan/certificate identity and the
restored A value is structurally equal. Projection/no-effect failures remain
separate for each owner kind.

## Implementation allowlist, caps, validation, and stops

The implementation may touch only:

- `app/slug_loading_v2/src/repository_rule_context.rs` — immutable invocation
  input, preflight, non-retained source views, sole recursive allocator,
  repository context/attribute Starlark values, and exhaustive focused tests;
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs` —
  authenticated input construction, error translation, and ordinary/innate
  DICE/effect proofs; and
- scheduling documents for activation and closure.

No instantiation, certificate, attribute-carrier, Bzlmod, parser, analysis,
package, ruleset, or public API file may change. No new file, crate, dependency,
feature, fixture, unsafe code, public cross-crate type, key, lock, cache, hash,
fallback, or compatibility shim.

Cap net Rust production growth at **300 lines**, proof growth at **1,100 lines**,
and aggregate growth at **1,400 lines**. Count from the accepted `1f9433600`
versions of the two allowlisted files, classifying everything before the first
`#[cfg(test)]` as production. Every new helper stays below 150 lines.
`repository_rule_context.rs` remains the cohesive evaluator adapter.
`module_extension_repository_file_effect.rs` already exceeds 2,000 lines but
remains the cohesive authenticated effect-key owner; restrict its production
change to the small handoff/error translation and place only key-level proofs
there. `REPLAN` rather than widening or splitting unrelated effect behavior.

Validate serially with focused context/allocator tests, focused ordinary and
innate effect-key tests, `cargo test -p slug_loading_v2 --quiet`,
`cargo test -p slug_bzlmod_v2 --quiet`, `cargo build -p slug_cli_v2`, two fresh
rules_rust replays using the rebuilt binary, `cargo fmt --all -- --check`,
`git diff --check`, exact allowlist/cap accounting,
`scripts/v2_archive_status.sh`, clean `slugd` before/after replay, and
independent terminal review. Record command, exit status, test count, and next
terminal; do not retain passing logs in the checkout.

`REPLAN` before or during implementation if any kind cannot use the one
non-retained value view; label publication would remap or stringify; exact
container order needs a second retained identity; ordinary and innate owners
cannot share the input/evaluator value; preflight can occur only after state or
effects exist; a DICE proof needs a new key or direct filesystem bypass; a
retained value borrows the evaluator heap; the two-file allowlist or caps fail;
fresh replays require `workspace_root` or another deferred effect; or one
focused correction does not resolve implementation/terminal review.

## R2 replan state

Commit `47c937942` records independent `ACCEPT` of the corrected R3
architecture. R1 fits production at 290/300 but its 197 proof lines omit the
required ordinary/innate key-level matrix; the original 650 proof cap leaves no
clean room for both owner transitions. R2 changes no semantic decision,
production cap, file allowlist, fixture policy, or validation gate. It raises
only proof/aggregate caps to 1,100/1,400 and freezes reuse of the existing
same-DICE source updaters above. Independent replan review returns `ACCEPT`;
complete the missing proofs without changing R1 production. Root owns
integration, cap/scope accounting, fresh replay, terminal review, scheduling
closure, and commits.
