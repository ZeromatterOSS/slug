# Current Slug V2 Packet

Packet: `WP-4-5-7A-module-bzl-native-context-implementation`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: independently accepted native-context architecture `612126b6f`, accepted
module-extension `native.existing_rule[s]` family `cc93ce4e7`, and accepted
complete Bazel repository package `3023718a0`. All unrelated dirty analysis,
loading, core, and REAPI work remains parked and read-only.

## Observable result

Implement the accepted exact loading-context architecture needed to complete
Bazel 9.2's module-loaded native family with
`native.bazel_version == "9.2.0"` while
keeping that field absent from BUILD-loaded `.bzl` modules. The design must
separate BUILD and Bzlmod evaluation in root and external recursive DICE keys,
not infer context at attribute access or publish the value universally.

This packet must make two fresh rules_rust replays advance
beyond `@@bazel_features+//private:globals_repo.bzl:22` and stop identically at
the next authentic unsupported boundary or succeed. That boundary is evidence,
not authorization to widen the packet.

## Learned facts and semantic authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole
semantic authority:

- `BzlLoadValue.KeyForBuild` and `KeyForBzlmod` are distinct SkyKeys. Their
  `getKeyForLoad` methods preserve the same context through every transitive
  `.bzl` load.
- `BzlLoadFunction.getAndDigestPredeclaredEnvironment` selects the BUILD or
  Bzlmod predeclared environment from that key class.
- `BazelStarlarkEnvironment.createUninjectedBuildBzlNativeBindings` omits
  `bazel_version`; `createUninjectedModuleBzlNativeBindings` adds the supplied
  product version.
- `RegularRunnableExtension`, `InnateRunnableExtension`, and
  `RepoDefinitionFunction` request `BzlLoadValue.keyForBzlmod`, including the
  Bzlmod-bootstrap repository-definition path.
- There is no focused upstream test that discriminates this field and key
  context. Pinned source plus the real rules_rust/Bazel Features consumer is
  stronger than adding a synthetic checked-in fixture.

Both fresh post-`cc93ce4e7` rules_rust replays advance beyond
`native.existing_rule` and stop identically at:

```text
Object of type `native` has no attribute `bazel_version`
@@bazel_features+//private:globals_repo.bzl:22
```

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
**concept/test only** guidance. Its separate BUILD/MODULE native namespaces,
recursive module-evaluation identity, and complete module native member set
(`existing_rule`, `existing_rules`, `bazel_version`) corroborate the ownership
shape. Copy no Zig code, evaluator, store, digest, or compatibility claim.

The Buck2-derived utility review retains the current Rust DICE key structs,
`Arc` values, `Dupe`, and `Allocative`. Add one compact enum directly to key
identity; add no side map, interner, cache, string allocation, strong hash, or
new dependency. Stage 9's Stage 4/5 loading rows remain unchanged because this
is a V2-owned contextual-key correction, not donor utility adoption.

## Accepted architecture

1. Add a loading-private two-variant context enum, `Build` and `Bzlmod`, to
   root and external `.bzl` evaluation keys, observation keys, and cycle
   identities. Existing ordinary constructors remain BUILD-context; explicit
   Bzlmod constructors are used only by module-extension/repository-definition
   owners. Display strings include the context for diagnostics and audits.
2. Propagate the parent's context unchanged through every root and external
   transitive load. Cycle recovery uses the same contextual identity. Never
   down-convert a Bzlmod child to a BUILD key or share a frozen module across
   contexts.
3. Keep BUILD and Bzlmod native namespaces as two concrete Starlark value
   types. Populate their common native method table once; only the Bzlmod type
   adds the immutable `bazel_version` attribute. `hasattr`, `getattr`, `dir`,
   and direct access therefore observe true absence under BUILD rather than a
   context error or fabricated value.
4. Derive the field from
   `slug_bzlmod_v2::BuiltinBazelToolsSnapshot::CURRENT.bazel_version()`.
   The single-variant pinned snapshot owns `"9.2.0"`; do not duplicate a raw
   version literal in loading or source it from the Slug package version,
   environment, CLI, RC file, or daemon mutable state.
5. Route ordinary and selected module-extension definitions, innate
   `use_repo_rule` definitions, and module-extension repository-definition
   file effects through explicit Bzlmod constructors in both legacy and
   observed paths. BUILD/root/external package loading keeps the existing BUILD
   constructors.
6. Keep the accepted `native.existing_rule[s]` methods in the shared method
   table. Their invocation-local marker behavior is unchanged; only
   `bazel_version` is a static Bzlmod namespace field.

## Compatibility and non-decisions

- **Exact:** distinct BUILD/Bzlmod recursive load identity; BUILD absence and
  Bzlmod presence of `native.bazel_version`; string value `"9.2.0"`; ordinary,
  selected, innate, repository-definition and bootstrap Bzlmod routing; field
  behavior under direct access, `hasattr`, `getattr`, and `dir`; unchanged
  module-extension `existing_rule[s]` values.
- **Slug-native:** Rust enum/type/constructor names, display wording, use of the
  pinned built-in snapshot as the immutable version owner, and DICE's retained
  Rust representation.
- **Unsupported/deferred:** `_builtins` injection and override semantics; `.scl`
  dialect convergence; runtime-selectable Bazel compatibility; BUILD native
  package/module/repository information methods; BUILD/finalizer existing-rule
  snapshots; repository context methods; parser or `set` work; `cc_common`,
  `cc_internal`, rules_cc, and C++ rules/actions; the next replay boundary.

Do not make the field globally visible, inspect call stacks or source labels to
guess context, reevaluate on cache hits, add a command-side repair, or key only
the top-level module while sharing transitive children.

## Ownership, equality, request, and memory

The `.bzl` evaluation DICE key is the natural producer of the frozen module and
therefore owns its predeclared-environment context. Context participates in
derived `Eq`/`Hash` for evaluation, observation, and cycle keys before any
computation. Recursive child edges preserve it structurally, so warm reuse,
invalidation, overlapping requests, and A/B/A restoration cannot cross BUILD
and Bzlmod results.

The exact version is process-immutable binary metadata selected with the
verbatim Bazel-tools snapshot. It has no request overlay, filesystem source,
environment input, lockfile input, final-validation step, or historical-state
problem. A future runtime-selectable compatibility version must `REPLAN` and
become an explicit key/request input rather than mutating this constant.

The context enum is DICE-retained semantic memory and is released with its key;
the static version string and method tables are service-lifetime immutable
data. Evaluator values are frozen-module/evaluation scratch under existing
owners. No lock, async transfer, task, cache, eviction, cancellation, join, or
shutdown policy changes. No retained value borrows evaluator scratch.

## Implementation proof matrix

This packet must prove:

1. the pinned snapshot returns `"9.2.0"` and its route identity remains stable;
2. direct BUILD `.bzl` globals lack the field while Bzlmod globals expose the
   exact immutable string through direct access, `hasattr`, `getattr`, and
   `dir`;
3. root legacy and observed keys distinguish BUILD/Bzlmod for the same label,
   propagate context into a transitive child, and restore BUILD/Bzlmod/BUILD
   A/B/A without cross-context reuse;
4. external legacy and observed keys provide the same recursive distinction,
   including a repository-mapped child;
5. ordinary root and selected module-extension, innate repository-rule, and
   repository-definition effect owners request only Bzlmod constructors while
   package-loading owners remain BUILD-only;
6. the accepted `existing_rule[s]` focused and full loading suites remain
   green; and
7. two fresh rules_rust replays advance beyond `globals_repo.bzl:22` and agree
   on the next boundary.

Reuse colocated DICE/unit scaffolding and the real workspace. Add no checked-in
oracle fixture, copied registry subtree, mutation, manifest, or expected file.

## Implementation allowlist, caps, and stops

This implementation may touch only:

- `app/slug_bzlmod_v2/src/builtin_repository.rs` — snapshot version accessor
  and focused invariant;
- `app/slug_loading_v2/src/package.rs` — shared/BUILD/Bzlmod native namespace
  construction and focused namespace proofs;
- `app/slug_loading_v2/src/bzl_module.rs` — contextual key/cycle identity,
  recursive propagation, globals selection, and DICE proofs;
- `app/slug_loading_v2/src/module_extension.rs` — ordinary/selected Bzlmod key
  constructors and focused owner assertions;
- `app/slug_loading_v2/src/module_extension_innate_repository.rs` — innate
  Bzlmod key constructors only;
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs` —
  repository-definition Bzlmod key constructors only; and
- scheduling documents for architecture acceptance, activation, and closure.

The parked `package.rs` definition-source diff must remain byte-identical and
unstaged. Every other dirty file is excluded. Cap net Rust production growth at
220 lines, tests at 300, and total at 520. No new file, crate, dependency,
unsafe code, public cross-crate type, lock, cache, fallback, or fixture.

`bzl_module.rs` remains the cohesive contextual load-key/evaluation owner
despite its size; splitting the enum or recursive propagation elsewhere would
separate key identity from computation. `package.rs` remains the cohesive
native registry with two concrete namespace values sharing one method table.
Keep every new helper below 150 lines. This is retained semantic state but not
a demonstrated hot-path optimization, so exact equality/lifecycle proofs
replace performance claims.

Validate serially with focused snapshot/native/root/external/owner tests,
`cargo test -p slug_bzlmod_v2`, `cargo test -p slug_loading_v2`,
`cargo build -p slug_cli_v2`, two clean fresh-root replays, formatting/diff/
allowlist/cap checks, `scripts/v2_archive_status.sh`, and independent terminal
review.

`REPLAN` before implementation if context cannot live in every relevant DICE
and cycle key; recursive children cannot inherit it without a public key
surface; a caller cannot be classified from its semantic owner; BUILD package
loading requires the Bzlmod namespace; version bytes are runtime-mutable; key
size requires a new allocation; the dirty `package.rs` hunk overlaps; caps
fail; or one focused correction does not resolve design/terminal review.
