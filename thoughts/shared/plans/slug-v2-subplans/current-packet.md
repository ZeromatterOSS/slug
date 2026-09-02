# Current Slug V2 Packet

Packet: WP-2-4-5-7A-repository-label-path-owner-design-r1

Milestone: M7A bootstrap-critical loading/repository execution closure. Admit
the bounded Bazel 9.2 `repository_ctx.path(Label)` slice with a lexical routed
path owner and a lock-safe bridge between synchronous Starlark evaluation and
asynchronous DICE prerequisites.

Status: docs-only design and independent architecture review `ACCEPTED`;
bounded Rust is selected under this frozen contract.

Immediate predecessor `WP-5-7A-repository-context-path-audit` is terminally
`REPLAN` in `08af092a8`. It proves that Slug's existing
`HostRepositoryPathKey` is not reusable because it observes target existence
and resolves symlinks, while Bazel's Label-path operation only package-looks-up
the Label and returns a lexical rooted path.

## Frozen compatibility boundary

Implement as **exact** within the admitted repository-file-effect slice of at
most 256 distinct Label-path addresses per invocation:

1. `repository_ctx.path` accepts an existing Slug `Label` value. Its canonical
   repository/package/target address selects the root workspace or an
   authenticated canonical repository source route.
2. The Label package must exist according to the same BUILD/BUILD.bazel,
   deleted-package and repository-ignore policy used by ordinary loading. A
   missing target underneath an existing package is accepted and produces the
   same path as a present target.
3. The path is the selected package root joined lexically with the Label's
   package and target fragment. Construction does not inspect the target,
   resolve its symlinks or add a target observation. This matches Bazel 9.2's
   default `--incompatible_no_implicit_watch_label=true` behavior.
4. The returned `path` value is immutable and hashable. `str` emits its
   normalized physical path, `repr` quotes the same string, and equality/hash
   compare only those physical path bytes. Observation namespace and source
   routing provenance do not affect Starlark equality.
5. Direct-local, immutable registry/archive and generated external routes use
   the same projection when their existing source owners complete. Root labels
   use the package root chosen by the existing root package lookup, not an
   assumed workspace directory.
6. Route, package and materialization needs restart through existing DICE need
   algebra. Package/source/materialization errors fail before an effect is
   published. A self-generated-repository cycle remains a typed DICE failure;
   it is not bypassed.

Keep **Slug-native** physical temporary/materialization directory bytes,
native-Unicode path representation, diagnostics, retry count and error text,
evaluator sentinel transport, observation carrier representation and DICE
cutoff mechanics. No exact Bazel output-base, Java VFS or HotSpot identity is
claimed.

Keep **unsupported/deferred**:

- string and existing-path arguments to `repository_ctx.path`, including the
  generated repository's working-directory identity and absolute-path rules;
- built-in `@bazel_tools` Label paths while its verbatim in-memory catalog has
  no immutable physical materialization owner;
- `basename`, `dirname`, `get_child`, `exists`, `is_dir`, `realpath`, `readdir`
  and every filesystem observation/watch they can initiate;
- `symlink`, `template`, `read`, `watch`, `watch_tree`, `execute`, `which`,
  delete/rename/download/extract/patch and every other repository effect;
- module-extension path values, native repository rules, remote repository
  execution, lockfile mutation, configured analysis/actions and exact
  generated-repository layout;
- alternate Label grammar/mapping behavior and any rules_cc, rules_rust,
  toolchain, repository-name or platform special case; and
- invocations demanding more than 256 distinct Label paths.

The path value is useful for dictionary storage, equality, hashing,
stringification and later generic repository APIs. This packet does not claim
that the current rules_cc replay completes: its next independent call is
expected to be `repository_ctx.symlink`.

## Natural owner and retained identity

Add one private-module/public-ABI family in
`app/slug_bzlmod_v2/src/repository_label_path.rs`:

- `HostRepositoryLabelPathSource` is either a root package-lookup workspace or
  an existing `HostRepositorySourceRoute`.
- `RepositoryLabelPathAddress` contains only `PackageIdentifier` and
  `TargetName`, projected from `CanonicalLabel`. It deliberately discards
  `RepositoryMappingId`: mapping provenance selected the canonical address but
  is not part of Bazel path identity. This is a filesystem address, not a
  second Label representation.
- `HostRepositoryLabelPath{,Observation}Key` owns source plus address.
  Constructors reject root/external repository mismatches before compute.
- `HostRepositoryLabelPathValue` owns `NormalizedAbsolutePath` and
  `PathObservationNamespace`. Its DICE equality includes both because the
  namespace distinguishes host from immutable materialization observations;
  the Starlark wrapper's equality/hash intentionally uses only the path.
- `HostRepositoryLabelPathError` projects package disposition, package lookup,
  materialization, root mismatch, invalid materialized path and unsupported
  built-in catalog disposition without exposing unrelated private key types.
- `ObservedHostRepositoryLabelPath` owns the ordinary result and merged
  package-marker observation epoch. Observed outer frontier errors remain
  separate exactly like existing Bzlmod observed keys.

The key must reuse `HostRootPackageLookup{,Observation}Key` for root labels and
`ExternalRepositoryPackageLookup{,Observation}Key` for routed external labels.
For root success, use `HostPackage.package_root()`. For external success,
compute the existing `RepositoryMaterializationResultKey` and select local
`source_root`/Host namespace or immutable `generation_root`/materialization
namespace. Widen that result key to `pub(crate)` only if the sibling module
needs it; do not publish it outside `slug_bzlmod_v2`.

After package success, join `package.package()` and `target` directly from the
already-validated identity types. Do not parse their display form. Do not call
`HostRepositoryPathKey`, `ResolvedPathKey`, file-byte keys, directory listing,
BUILD evaluation or direct filesystem APIs for the target. Package lookup may
continue using its already-accepted marker-resolution dependencies.

Built-in source disposition returns a stable typed unsupported error after
package lookup and before fabricating any path. Root package search must retain
the exact selected package root when multiple package roots exist.

No repository-rule definition, call, selected-owner certificate, BZL manifest,
source route, materialization result, generated file-effect plan or published
effect shape changes. The new key/value is retained DICE state and therefore
must derive/implement structural `Eq`, `Hash`, `Allocative`, cutoff and validity
in the existing style.

## Synchronous invocation bridge

Add an invocation-only `PreparedRepositoryLabelPaths` using the existing
`SmallMap<RepositoryLabelPathAddress, HostRepositoryLabelPathValue>`. It is
created inside `HostSelectedRepositoryFileEffectKey::compute`, never published,
and capped at 256 distinct addresses. Exceeding the cap is a Slug-native
terminal invocation error. The first rules_cc consumer requests nine.

`RepositoryRuleInvocationState` borrows or clones that prepared map for one
synchronous attempt and owns one `RefCell<Option<RepositoryLabelPathAddress>>`
unresolved demand. The `path` method:

1. validates the receiver and accepts only `StarlarkLabel`;
2. projects its address without reparsing or retaining mapping provenance;
3. returns a heap-allocated `RepositoryStarlarkPath` on a prepared hit; or
4. records the first unresolved address and returns an evaluator sentinel.

`invoke_repository_rule` examines invocation state after the evaluator is
dropped. When the sentinel corresponds to the recorded unresolved address it
returns a typed `RepositoryRuleInvocationError::LabelPathNeed`; otherwise it
preserves the existing evaluation/result/path/plan errors. It must not turn an
arbitrary evaluation error into a need.

The outer effect driver loops over attempts. On `LabelPathNeed`, it proves that
the address is new, resolves it asynchronously through the root or canonical
load route and the new label-path key, inserts the complete value, then invokes
again. Every evaluator, heap allocation, effect builder, dynamic-environment
vector, invocation state and `RefCell` borrow from the failed attempt is
dropped before any `ctx.compute(...).await`.

Each attempt gets its own print capture; captures, partial file effects and
dynamic environment observations from demand attempts are discarded. Only the
terminal successful attempt publishes them. The prepared map and retry count
are local scratch and never participate in effect identity. The DICE
dependencies computed while filling it do participate normally in the outer
effect key.

Legacy mode computes ordinary route/path keys. Observed mode computes their
observation variants and unions route and package-marker epochs with the
effect's current observations before the next attempt. A need returns through
the existing `SourcePreparationOutcome`; on recompute, child DICE results may
be reused even though the local prepared map starts empty. Cancellation drops
all scratch. No lock or borrow crosses a DICE compute.

## Buck2 utility and memory decision

Reuse the retained Buck2-derived `SmallMap`, `Dupe` and `Allocative` patterns,
plus existing `Arc`, `CompactString`, `NormalizedAbsolutePath`,
`PathObservationNamespace`, `HostRepositorySourceRoute`, `PackageIdentifier`
and `TargetName` values. The prepared map is bounded invocation scratch; the
retained key contains one source and one compact address, while the retained
value contains one normalized path and namespace.

No new interner, arena, global cache, registry, process singleton, string-path
parser or large clone is selected. This is new Slug-owned semantic composition,
not an extraction from V1 or Buck2, so Stage 9 records the reuse decision but
adds no extraction-ledger row. Add size assertions for the address, key and
value; return `REPLAN` if the key exceeds 384 bytes or the value exceeds 128
bytes on the reference target.

## Evidence and proof

Pinned authority remains Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, specifically
`StarlarkBaseExternalContext.getPath/getPathFromLabel`,
`RepositoryUtils.getRootedPathFromLabel`, `StarlarkPath`,
`StarlarkPathTest`, and both implicit-watch cases in
`StarlarkRepositoryContextTest`. The authenticated rules_cc 0.2.18
`resolve_labels` source is a consumer discriminator only.

Add focused Bzlmod proof for:

- root lookup selecting the actual package root, including a non-first root;
- direct-local and immutable external roots and namespaces;
- package absent/deleted/ignored/invalid/error dispositions;
- target present, missing and symlink spellings producing the same lexical
  address and no target observation;
- built-in catalog fail-closed behavior;
- route/repository mismatch and invalid join rejection;
- observed/legacy value parity, exact marker observations, need propagation,
  cancellation and A/B/A materialization-root restoration;
- equality/cutoff/hash and size ceilings.

Add focused loading proof for:

- one and multiple Label path demands resolving through retries;
- repeated demand hits without another resolution and the 256-address cap;
- immutable/hashable path equality, inequality, `str` and `repr`;
- Label mapping provenance not affecting a resolved physical path address;
- root and canonical route selection plus missing package/error projection;
- failed-attempt print/effect/environment discard and successful-attempt
  publication;
- observed route/package epochs, needs, cancellation and A/B/A restoration;
- no target-observation dependency and no lock/borrow across compute.

The rebuilt authentic replay must clear only the missing `path` method and stop
at the next independent generic boundary. Do not add a symlink or rules_cc
special case to make the replay advance farther.

## Allowlist, caps and complexity

Production Rust may change only:

- new `app/slug_bzlmod_v2/src/repository_label_path.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs` only for a private visibility
  handoff when required;
- `app/slug_loading_v2/src/repository_rule_context.rs`;
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`; and
- `app/slug_loading_v2/src/lib.rs` only if a new internal module is required.

Proof Rust may change only the `#[cfg(test)]` sections/modules adjacent to
those owners and one new focused loading test module if keeping proof out of the
2,428-line effect owner materially improves reviewability.

Scheduling records may change only the canonical plan, Stages 2, 4, 5 and 9,
and this manifest. Do not change Cargo metadata, fixtures, Bazel/rules_cc
sources, repository-rule definition/call/certificate shapes, Label parsing,
source-route/materialization shapes or generated file-effect shapes.

Caps are 520 gross added production Rust lines, 650 proof lines and 1,170 total.
No new function may exceed 90 lines; no existing function may grow by more
than 25 lines. Prefer a new focused Bzlmod owner over adding the category to
the 17,245-line `source_preparation.rs` or 5,616-line `host_package.rs`.
`module_extension_repository_file_effect.rs` may receive only orchestration
helpers and the bounded retry loop. No benchmark is required; retained size
ceilings, map cap and warm-DICE nonreplay proof are mandatory.

## Validation and terminal stops

Run serially:

- focused label-path Bzlmod and repository-context/effect tests;
- `cargo test -p slug_bzlmod_v2 --lib -q`;
- `cargo test -p slug_loading_v2 --lib -q` and every loading integration test;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- stale `slugd` cleanup before and after the authentic rules_rust replay;
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  verification.

Return `REPLAN` before or during Rust if:

- package lookup cannot provide the actual root without BUILD evaluation;
- the target is read, listed, resolved, watched or otherwise observed;
- `HostRepositoryPathKey`/`ResolvedPathKey` is used for the target;
- a root is inferred from a `.bzl` path, output-base convention, generated
  repository name or display string;
- built-in catalog, string/generated-root path or filesystem method support is
  needed to make the bounded value honest;
- an evaluator, heap, lock, state or `RefCell` borrow crosses a DICE compute;
- arbitrary evaluation errors can be mistaken for path needs, partial attempt
  events/effects/environment escape, or the retry cap is absent;
- a demand/prepared map enters retained effect identity, an injected frontier,
  process global, cache or registry;
- Label mapping provenance is copied into path identity or canonical address
  is reconstructed by parsing text;
- the retained size, file allowlist, growth caps or large-file boundaries are
  exceeded; or
- symlink/template/effect, ruleset/toolchain/repository/platform special cases
  or broader path semantics become necessary.

Architecture result: `ACCEPT`. Independent review confirms the natural owner,
no-target-observation boundary, mapping-provenance exclusion, 256-address
compatibility cap, speculative-attempt event discard and lock-safe retry
lifetime. The packet creates one lexical package-root path owner over existing
DICE dependencies and one bounded invocation-local retry bridge; it neither
weakens observation identity nor pretends the resolved source-read path is
Bazel's path constructor. Rust may begin only under this contract.
