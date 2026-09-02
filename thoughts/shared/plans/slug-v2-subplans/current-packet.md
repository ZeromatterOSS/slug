# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-rule-file-admissibility-category-implementation-r1

Milestone: M7A bootstrap-critical generic Starlark/repository loading. Preserve
the complete Bazel 9.2 repository-rule file-admissibility declaration category
through the existing frozen definition and invocation owners.

Status: architecture accepted; implementation active. Independent review
returned `ACCEPT` on 2026-09-01 for the retained owner, no-resolution phase,
compact identity, proof matrix, bounds and deferred adjacent policies.

The immediate predecessor
`WP-4-5-6-7A-root-repository-load-route-publication-replan-r1` is terminally
accepted in `75fad534c`. Its authentic rules_rust 0.73 replay clears generated
root publication and next stops while verbatim
`@@bazel_tools//tools/build_defs/repo:git.bzl` declares
`build_file = attr.label(allow_single_file = True)`: Slug reports
`unsupported repository_rule attribute schema 'build_file'`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.
Do not edit or stage it.

## Learned facts and source basis

Pinned Bazel 9.2 is the semantic authority:

- `RepositoryModuleApi.repositoryRule` accepts an ordinary attribute-descriptor
  dictionary and documents private repository attributes as a separate name
  category;
- `StarlarkRepositoryModule.repositoryRule` converts every descriptor with
  `Descriptor.build`, retaining the ordinary `Attribute` schema;
- `StarlarkAttrModule.createAttributeFactory` owns the complete file policy:
  `allow_files` on `label`, `label_list`, `string_keyed_label_dict`,
  `label_keyed_string_dict` and `label_list_dict`; `allow_single_file` only on
  `label`; exact mutual exclusion, Boolean/ordered-suffix forms and the
  independent `SINGLE_ARTIFACT` bit;
- `RepoRule.instantiate` and `AttributeUtils.typeCheckAttrValues` type-check,
  default and visibility-check invocation values but do not resolve targets or
  apply file-type predicates; retaining the schema is therefore the complete
  declaration/instantiation responsibility of this packet; and
- verbatim Bazel 9.2 `tools/build_defs/repo/git.bzl` and `http.bzl`, SHA-256
  `c4f89658...` and `9e908b9d...`, both declare `build_file` with
  `allow_single_file = True`. Slug's embedded copies have exactly those hashes.

The BCR rules_rust 0.73 archive is a demanding consumer only. Its source is
pinned by BCR integrity `sha256-LQyLlnthnVcXvoIQ9SokxapiTjIpo43EBxcS2x3VIvI=`;
the authentic replay reaches the verbatim Bazel-tools declaration before any
repository implementation or Rust/C++ rule body.

Slug already has the exact V2-owned `FileAdmissibility` representation:
NoFiles, AnyFile or an immutable ordered suffix slice plus an independent
single-artifact bit. The five general attribute constructors already produce
it with Bazel-compatible argument validation. The repository-rule boundary
currently rejects every non-NoFiles value and then drops the field from
`RepositoryRuleAttribute`; that is the sole demonstrated gap.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
`module_extension_declaration_host.zig` retains ordinary loaded attribute
schemas separately from explicitly supplied repository invocation values, and
its real-shaped Bazel-tools proof includes `build_file` with
`allow_single_file`. Slug adopts only that ownership/test lesson. Copy no Zig
type, allocator, evaluator host, normalization, cache, scheduler, error or
compatibility claim.

## Decision and complete category

Add the existing `FileAdmissibility` value to `RepositoryRuleAttribute` and
populate it from the existing `AttributeDefinition`. Remove only the
repository-rule filter that rejects a non-NoFiles policy or the independent
single-artifact bit. The field then participates automatically in frozen
definition, projection, call-record and DICE structural equality.

Admit the complete default-enabled Bazel 9.2 file-policy category:

1. `attr.label`: absent/None, Boolean and ordered list/tuple `allow_files`;
2. `attr.label`: absent/None, Boolean and ordered list/tuple
   `allow_single_file`, retaining the independent single-artifact bit even for
   False or an empty suffix sequence;
3. `attr.label_list`: all `allow_files` forms;
4. `attr.string_keyed_label_dict`: all `allow_files` forms;
5. `attr.label_keyed_string_dict`: all `allow_files` forms; and
6. `attr.label_list_dict`: all `allow_files` forms.

The existing attr constructor remains the sole binder/validation owner for
conflicting keywords, value types, suffix order/duplicates, empty suffixes and
unsupported `allow_single_file` constructors. Repository-rule construction
must not parse or normalize those arguments a second time.

This packet does not resolve a label to a target or physical file, apply suffix
matching during repository instantiation, or add an eager package/source read.
Bazel's admitted repository instantiation path does not do that either. Later
repository-context path/read/symlink capability packets must consume the
retained label and existing observed source owners when their first semantic
use requires it.

## Compatibility classification

Admit as **exact** for the named Bazel 9.2 surface:

- successful repository-rule declarations for the six constructor/policy rows
  above, including verbatim Bazel-tools `build_file` declarations;
- NoFiles/AnyFile/ordered-suffix and single-artifact structural distinctions,
  including exact order, duplicates, explicit False and empty suffix lists;
- freeze/export/reacquisition and invocation-call projections retaining the
  same policy; and
- source-revision and DICE A/B/A behavior when only file policy changes.

Keep **Slug-native**:

- the Rust enum/Arc representation, compact structural identity, allocation
  accounting and complete-only DICE equality; and
- existing typed diagnostics where the exact Bazel wording is not named.

Keep **unsupported/deferred**:

- private repository attribute names/default dependencies, dormant dependency
  kinds, `remotable`, materializers, computed/late defaults, configuration or
  aspect transitions, executable/provider/rule-class/allowed-value and other
  separately owned attribute-policy categories at this boundary;
- actual repository label target/file/package validation and later
  repository_ctx path/read/symlink effects;
- generated artifacts as repository-rule file inputs;
- repository materialization breadth, exact output/configuration identity; and
- rules_rust, rules_cc, C++, `cc_common`, `cc_internal`, parser or consumer
  specialization.

## Natural owner, revision and memory

`AttributeDefinition.file_admissibility` remains the declaration producer.
The frozen `RepositoryRuleDefinition` and its
`RepositoryRuleDefinitionProjection` retain the immutable
`Arc<[RepositoryRuleAttribute]>`; invocation records and existing loading DICE
keys already carry that projection. No new key, dependency, registry, graph,
mapping, filesystem observation, environment input, lock, cache or interner is
added.

The defining `.bzl` source identity and transitive load closure already drive
module evaluation and invalidation. A source revision that changes only file
policy changes the retained projection; restoring it restores equality and the
original complete result. Overlapping requests share the existing DICE compute,
Need remains carrierless, cancellation publishes no complete value and no lock
is held across a DICE compute.

The added field is DICE-retained semantic memory. It clones the existing small
enum and, for suffix predicates, one immutable Arc slice; it adds no `String`,
`Vec`, map/set, evaluator value or copied suffix storage to each invocation.
`CompactString`, `Arc<[T]>` and `Allocative` remain sufficient. Buck2/V1
extraction is `none`; record that no-extraction decision in Stage 9.

## Evidence and proof

Reuse the accepted general file-admissibility evidence rather than adding an
oracle fixture. Add focused loading proof for:

- all five `allow_files` constructors and the scalar-only
  `allow_single_file` row;
- absent/None, true/false, ordered/duplicate/empty list and tuple policies;
- mutual exclusion and typed invalid-policy rejection remaining owned by the
  general attr constructor;
- frozen export/projection/call identity, including same declaration values
  sharing the existing Arc and different policies comparing unequal;
- default and explicit repository invocation values remaining unchanged;
- source-only file-policy DICE A/B/A and warm complete-result reuse; and
- the verbatim Bazel-tools `git.bzl`/`http.bzl` declarations or an exact
  source-shaped regression proving `build_file` clears without invoking a
  repository implementation.

The existing authentic rules_rust 0.73 replay is the downstream discriminator.
It must clear the current `build_file` declaration terminal and stop at the
next honest generic boundary. No new fixture files are authorized.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/module_extension_repository_rule.rs`;
- `app/slug_loading_v2/src/package.rs`; and
- only if required by the existing retained constructor surface,
  `app/slug_loading_v2/src/attrs.rs`.

Proof/mechanical constructor updates may change only:

- those production files' existing test modules;
- `app/slug_loading_v2/src/module_extension.rs` for the existing DICE
  repository-declaration A/B/A owner;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`; and
- `app/slug_loading_v2/src/repository_rule_context.rs`.

Scheduling records may change only the canonical plan, owner plans 04/05,
Stage 9 and this manifest. Caps are 180 gross added production Rust lines, 420
proof lines and 600 total. No new function may exceed 150 lines.

`package.rs` and `module_extension.rs` exceed the 2,000-line trigger. The former
is still the natural global/binder owner and may change only its existing
repository-rule filter/projection; the latter is test-only for one existing
DICE harness. Do not add a semantic key, retained type or helper to either.
`module_extension_repository_instantiation.rs` also exceeds the trigger and may
receive only mechanical test-constructor/default-value checks; invocation
semantics remain unchanged.

## Validation and stops

Run serially:

- focused repository-definition/projection tests;
- the focused repository-declaration DICE A/B/A test;
- `cargo test -p slug_loading_v2 --lib -q`;
- one direct compile dependent, preferably `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before the authentic replay;
- the authentic rules_rust configured-query replay with daemon cleanup before
  and after;
- `cargo fmt --check`, `git diff --check`, the archive checker and parked-proof
  SHA-256 verification.

Return `REPLAN` before or during Rust if:

- declaration acceptance requires target/file resolution, a package/source
  read, a new DICE key or a second file-policy representation;
- any repository invocation, RepoSpec, default, label mapping or context value
  changes beyond carrying the existing policy in definition identity;
- private names, dormant kinds, `remotable` or another attribute-policy
  category becomes necessary;
- suffix storage is copied per call or a new collection/interner/cache appears;
- the authentic replay does not clear the exact declaration gap;
- the allowlist/caps are exceeded or another material correction is needed; or
- implementation requires ruleset/C++, parser/builtin specialization, Java/JVM
  semantics, exact identity bytes or a lock across DICE compute.

Independent review must confirm that preserving the existing compact policy in
the frozen repository definition is the natural complete category, that Bazel
does not require file resolution in this phase, and that every adjacent policy
is honestly deferred before Rust begins.

Independent review returns `ACCEPT`. The packet preserves Bazel 9.2's complete
file-policy schema at its natural frozen repository-definition owner across all
five label-bearing constructors; `RepoRule.instantiate` needs no file
resolution. Structural identity, DICE A/B/A invalidation, Arc-backed lifetime,
proof, caps, stops and adjacent deferrals are complete. Residual implementation
risk is mechanical propagation through every test constructor and proving Arc
reuse only within one frozen definition/call cohort, never across separately
evaluated declarations.
