# Current Slug V2 Packet

Packet: WP-4-5-7A-package-context-label-string-category-design-r4

Milestone: M7A bootstrap-critical generic Starlark/loading and repository
closure. Converge every currently admitted package-context dependency-label
string consumer on one Bazel 9.2 grammar and typed canonical projection.

Status: terminal review of the R3 implementation returned `REPLAN` after its
focused architecture acceptance. R3 correctly selected the ordinary extension
evaluation package for dependency labels, but accidentally applied that base
and the dependency parser's special-main-package rule to repository
`Output`/`OutputList` strings. R4 changes only that deferred route: the five
dependency-label shapes use the evaluation base, while repository outputs keep
the repository-rule definition base and pre-packet parser. R1/R2/R3 already
settled innate ownership, `@//`/`@@//`, collision projection, complete consumer
inventory, BUILD/tag output non-widening and the BUILD proof allowlist. The
predecessor repository-rule file-admissibility category remains terminally
accepted in `95b4f0da6`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.
Do not edit or stage it.

## Trigger and source basis

The rebuilt authentic rules_rust 0.73 replay clears verbatim Bazel-tools
`build_file = attr.label(allow_single_file = True)` and next stops in
`crate_universe/extensions.bzl` at the ordinary descriptor default
`attr.label(default = "@rust_host_tools")`: Slug reports that an apparent
repository Label has no package separator. The authenticated archive integrity
is `sha256-LQyLlnthnVcXvoIQ9SokxapiTjIpo43EBxcS2x3VIvI=`. The source later
passes the typed value to `module_ctx.path`; this packet owns only label
construction, not that later path capability.

Pinned Bazel 9.2 is the semantic authority:

- `LabelParser.Parts.parse` owns one complete lexical table, including bare and
  colon-prefixed same-package targets, colon-free absolute target inference,
  apparent and canonical full forms, and repository-only `@repo`/`@@repo`
  shorthand normalized as `@repo//:repo`/`@@repo//:repo`;
- `Label.parseWithPackageContext` applies the current package and repository,
  maps apparent names, bypasses mapping for canonical names, and keeps
  unqualified absolute `//conditions` and `//visibility` in the main
  repository;
- `LabelConverter.forBzlEvaluatingThread`, `BuildType.LABEL.convert`,
  `StarlarkAttrModule` and the `Label()` implementation use the innermost
  executing `.bzl` package context; typed Label values pass through unchanged;
- `StarlarkBazelModule`/`TypeCheckedTag` use the calling module's root package
  and selected mapping for explicit module-extension tag values; and
- `RepoRule.instantiate` receives a caller-specific converter while descriptor
  defaults were already converted in the defining `.bzl` context: ordinary
  module extensions use their evaluation base/full generated-repository
  namespace, whereas `InnateRunnableExtension` uses the repository-rule `.bzl`
  package plus the calling module's mapping; and
- `BuildType.LabelKeyedDictType` rejects distinct raw dictionary keys that
  convert to the same Label instead of silently overwriting or retaining both.

The existing Bazel `LabelParserTest`, `LabelTest`, strict-visibility Label tests
and `@@repo` Args regression discriminate shorthand, target inference, apparent
mapping, canonical bypass, special main packages and context ownership. Reuse
those pinned-source regressions; add no Java helper or production artifact.

## Slug audit and natural owner

Slug has one correct typed result, `CanonicalLabel`, but overlapping partial
parsers:

- `slug_identity_v2::ResolvedOptionLabel` already accepts the complete lexical
  shorthand/full/relative table for its distinct command-option contexts;
- `starlark_label::resolve_label` rejects every canonical spelling and both
  repository-only shorthands, and rejects a bare `Label("target")`;
- scalar `attr.label` defaults use that `.bzl` resolver, while aggregate
  label-bearing defaults fall through `RawLabelContext::Root` and discard the
  defining module's repository mapping;
- ordinary BUILD values, module-extension tag values and repository-rule
  supplied values each reconstruct a similar grammar around
  `ApparentLabel::parse`, which itself intentionally accepts only unambiguous
  absolute apparent labels; and
- direct toolchain conversion contains a canonical-label fast path only
  because the shared resolver rejects it.

The natural owner is a single pure, borrowed package-label spelling parser in
`slug_identity_v2::label`, shared by `ResolvedOptionLabel` and a new
`CanonicalLabel` package-context conversion entry point. It validates and
normalizes syntax, but receives the base `PackageIdentifier` and a caller-owned
apparent-name resolver. It performs no repository selection, DICE lookup,
filesystem/package/target observation, caching or interning.

Loading retains every context decision:

1. `Label()` and descriptor defaults use `source_identity_for_call`, preserving
   the innermost executing/defining `.bzl` package and mapping;
2. ordinary BUILD dependency values use the loaded package and its complete
   repository mapping;
3. explicit module-extension tag values use the calling module's root package
   and mapping, never the tag-class definition mapping;
4. explicit ordinary extension repository-rule dependency values use the
   selected extension evaluation `.bzl` package from the selected request plus
   the full generated-repository namespace, even when the repository rule was
   imported from another `.bzl` package;
5. explicit innate `use_repo_rule` values use the repository-rule `.bzl`
   package and the calling module's mapping, not the generated namespace;
6. every pretyped Label remains unchanged rather than being parsed or mapped
   again; and
7. apparent-name ambiguity/missing visibility remains diagnosed by the
   existing caller that owns that mapping.

Repository `Output`/`OutputList` strings are the deliberate non-dependency
exception: retain the repository-rule definition package, the existing
namespace visibility map, rejection of canonical strings and repository
shorthand, and the pre-packet treatment of unqualified `//conditions` and
`//visibility` in the definition repository. Typed output policy is unchanged.

Do not widen `ApparentLabel::parse` or `CanonicalLabel::parse`: those APIs own
already-unambiguous absolute identity and are deliberately used by load,
pattern and internal identity boundaries with different admissibility rules.

## Complete category

Admit the complete Bazel 9.2 package-context dependency-label string category:

1. same-package `:target` and bare `target`/`path/to/target`;
2. current-repository `//pkg:target` and colon-free `//pkg/sub`;
3. apparent `@repo//pkg:target`, colon-free `@repo//pkg/sub`, and shorthand
   `@repo` mapped through the active context;
4. canonical `@@repo//pkg:target`, colon-free `@@repo//pkg/sub`, and shorthand
   `@@repo`, bypassing mapping;
5. empty apparent `@//pkg:target`, resolved through apparent mapping entry `""`,
   versus empty canonical `@@//pkg:target`, which bypasses mapping to main;
6. explicit root-package targets for unqualified, apparent and canonical
   repository spellings;
7. main-repository `//conditions` and `//visibility` special cases; and
8. exact typed canonical `(repository, package, target)` identity after target
   inference and terminal `/.` normalization.

Route every currently admitted package-context dependency-label string
position through that owner:

- universal `Label()` and its Label passthrough;
- rule/aspect/subrule/macro/repository-rule/tag-class descriptors for
  `attr.label`, `attr.label_list`, `attr.string_keyed_label_dict`,
  `attr.label_keyed_string_dict` and `attr.label_list_dict`, including every
  scalar/list/dictionary key/value default position;
- rule/aspect toolchain requirements and `config_common.toolchain_type`;
- ordinary and symbolic-macro BUILD dependency values in the same five
  constructors, including selector keys;
- every existing direct `PackageRecorder` dependency-label consumer:
  visibility/default visibility, package metadata, package-group includes,
  `filegroup.srcs`, `test_suite.tests`, `alias.actual`, config-setting label
  fields, and admitted constraint/platform/toolchain declarations;
- aspect/subrule toolchains and admitted aspect execution-compatibility labels;
- explicit module-extension tag values in the same five constructors; and
- explicit ordinary and innate repository-rule values in the same five
  constructors.

For `attr.label_keyed_string_dict`, reject two distinct raw string/Label keys
that normalize to the same canonical Label in descriptor defaults, BUILD or
symbolic-macro values, explicit tags and explicit repository calls. Preserve
the same collision invariant in already admitted label-keyed native fields such
as `config_setting.flag_values`. Exact diagnostic wording remains deferred.

This is a generic host conversion category. rules_rust is only the authentic
consumer that exposed it. BCR Starlark remains the owner of rules_rust,
rules_cc, `cc_common` and `cc_internal`; add no ruleset, toolchain, C++ or
consumer branch.

## Compatibility classification

Admit as **exact** for the named Bazel 9.2 successful surface:

- all eight grammar/normalization rows above;
- apparent mapping versus canonical bypass;
- innermost `.bzl`, loaded BUILD package, calling module, ordinary extension
  evaluation and innate repository-call conversion contexts at the named
  consumers;
- typed Label passthrough without a second mapping lookup; and
- canonicalized label-key collision rejection and typed repo/package/target
  results through frozen descriptors, calls and existing loading/DICE results.

Keep **Slug-native**:

- the Rust borrowed parse scratch, existing owned `CanonicalLabel`, its retained
  mapping provenance and structural equality/allocation accounting, and
  caller-specific diagnostic wrapping; mapped and canonical spellings are not
  claimed to have exact Bazel equality outside the named canonical projection;
  and
- fail-closed missing/ambiguous apparent mapping rather than retaining Bazel's
  non-visible Label object where that exact invalid-value lifecycle is not yet
  admitted.

Keep **unsupported/deferred**:

- output/output-list same-package policy, computed/late/materialized defaults,
  dormant labels and target/provider/file resolution;
- `load()`'s `.bzl`-only/repository route, repo-context transition setting
  syntax, command-line label/target-pattern contexts and exact invalid-label
  diagnostic wording/precedence;
- package existence, visibility, file admissibility application and
  `repository_ctx`/`module_ctx.path` effects; and
- exact Bazel configuration/output identity or any ruleset-specific behavior.

## Identity, revision and memory

No retained type changes. `CanonicalLabel` remains the only successful label
identity in these consumers. The parser returns only construction scratch;
mapping selection happens synchronously through an existing immutable slice or
map and the result retains no mapping copy, raw spelling or evaluator value.

Refactor the existing option-label grammar rather than adding a third parser.
Prefer borrowed apparent-repository spelling so a visible lookup does not
allocate a temporary repository `String`; allocate only the existing final
typed components or already-required non-visible option state. Add no cache,
interner, global registry, collection, DICE key, lock or filesystem input.

The defining `.bzl` source identity/mapping, BUILD package mapping, module tag
input mapping, ordinary extension namespace and innate calling-module mapping
already participate in their respective complete keys/results. Selection of
the innate map is borrowed construction state, not another retained mapping.
A source or mapping A/B/A change must alter the typed result and restoration
must recover structural equality. Need remains carrierless; cancellation
publishes no complete result and no lock is held across DICE compute.

## Buck2/starlark-rust and Zabel guidance

The vendored/adopted starlark-rust parser supplies Starlark language syntax and
the real `set`; Bazel Label semantics remain a host responsibility. Its
`starlark_bin/bin/bazel/label.rs` example independently demonstrates one
lexical label parser with repository-only shorthand, but is not a production
library owner and is not copied as a retained type.

No V1/Buck2 extraction is required. Refactor the existing V2 option-label
spelling parser and reuse `PackagePath`, `TargetName`, `CanonicalRepoName` and
`CanonicalLabel`. Record this no-extraction decision in Stage 9.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Its `core.labels.parseSyntax` and generic Label host separate borrowed pure
syntax from active module mapping resolution, preserve special main packages,
and avoid DICE/I/O in construction. Adopt that ownership/optimization lesson;
copy no Zig type, allocator, evaluator host, non-visible representation,
cache, scheduler, diagnostic or behavior claim.

## Evidence and proof

Add focused proof for:

- the complete lexical table, implicit targets, terminal `/.`, special main
  packages, distinct `@//` mapping/`@@//` bypass, and invalid
  relative-package/triple-dot/single-slash boundaries;
- apparent shorthand/full mapping and canonical shorthand/full bypass, with
  root/nonroot base packages and typed Label passthrough;
- an imported function proving `Label()` uses its innermost defining `.bzl`
  context rather than its caller;
- string and pretyped Label defaults across all five dependency constructors,
  including dictionary keys/nested values, frozen export and import/re-export;
- the defining `.bzl` mapping for defaults, calling module mapping for explicit
  tags, ordinary extension namespace for ordinary repository calls, and
  repository-rule `.bzl` package plus calling-module mapping for innate calls;
- an ordinary extension importing a repository rule from another `.bzl`
  package, proving explicit dependency strings use the extension evaluation
  package while descriptor defaults, `RepoRuleId`, relative output strings and
  unqualified special-package output strings retain the repository-rule
  definition package;
- ordinary/symbolic-macro BUILD package conversion, selector keys, and
  representative direct consumers covering visibility/package metadata,
  alias/filegroup/test-suite/config-setting and platform/toolchain families;
- converted-label dictionary-key collisions in defaults, BUILD/macro values,
  tags and both repository-call kinds, including string-versus-pretyped and two
  apparent names mapping to one canonical key;
- the `//conditions`/`//visibility` exception without changing adjacent output
  or `load()` policy;
- existing missing/ambiguous mapping and adjacent output/load/transition
  rejection boundaries remaining fail closed;
- source/mapping/default DICE A/B/A and warm reuse through an existing loaded
  descriptor or module-extension harness; and
- authentic rules_rust replay clearing `@rust_host_tools` and stopping at the
  next honest generic boundary.

No new fixture file or oracle artifact is authorized. Reuse pinned Bazel
source/tests, existing in-memory loading fixtures and the authenticated BCR
archive.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_identity_v2/src/label.rs`;
- `app/slug_loading_v2/src/starlark_label.rs`;
- `app/slug_loading_v2/src/package.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`.

Proof may change only those files' existing test modules plus:

- `app/slug_identity_v2/tests/label_roundtrip.rs`;
- `app/slug_loading_v2/src/host_package_inventory_tests.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- `app/slug_loading_v2/tests/build_file_loading.rs`, solely for ordinary BUILD
  package-context success/collision coverage and stale expectations directly
  contradicted by the admitted canonical-label surface; and
- only if required for the existing repository-declaration DICE harness,
  `app/slug_loading_v2/src/module_extension.rs`.

Scheduling records may change only the canonical plan, owner plans 04/05,
Stage 9 and this manifest. Caps are 320 gross added production Rust lines,
1,050 proof lines and 1,370 total. No new function may exceed 150 lines.

`package.rs`, `host_package_load_tests.rs`,
`module_extension_repository_instantiation.rs` and `module_extension.rs`
exceed the 2,000-line trigger. `package.rs` may only converge existing label
conversion call sites; repository instantiation may only replace its private
parser and extend focused proof; `module_extension.rs` is test-only. Add no new
semantic key, retained type or unrelated helper to these files.

## Validation and stops

Run serially:

- `cargo test -p slug_identity_v2 --test label_roundtrip -q`;
- focused loading parser/default/BUILD/tag/repository-call/DICE tests;
- `cargo test -p slug_loading_v2 --lib -q`;
- `cargo test -p slug_loading_v2 --test build_file_loading -q` if the ordinary
  BUILD proof owner changes;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after;
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  SHA-256 verification.

Return `REPLAN` before or during Rust if:

- a successful consumer needs a second label representation/parser, mapping
  reconstruction, target/package/file lookup, new DICE key or I/O;
- explicit module tags use the defining `.bzl` mapping, descriptor defaults use
  the caller mapping, ordinary/innate repository-call mappings are conflated,
  or pretyped Labels are remapped;
- `@//` and `@@//` collapse to one repository choice or converted label-key
  collisions are retained/overwritten;
- dependency conversion rebases repository outputs or gives them special-main
  package handling; output/load/transition/command semantics otherwise widen;
  non-visible retained labels or
  exact invalid-diagnostic parity become necessary;
- any rule/ruleset/C++/`cc_common`/`cc_internal` specialization appears;
- a cache/interner/global registry or retained raw spelling is introduced;
- production/proof caps or file allowlists are exceeded; or
- the authentic replay does not clear the exact `@rust_host_tools` boundary.

Focused independent R2 architecture rereview returns `ACCEPT`. Ordinary and
innate contexts are representable through existing borrowed inputs, the empty-
repository and collision semantics are complete, the expanded consumer proof
matrix is bounded, and the retained identity/no-extraction architecture stands.
During implementation collision checks must compare canonical
repository/package/target identity rather than optional Slug mapping provenance,
and innate conversion must borrow `definition_parts().3`, never the generated
namespace returned by `namespace_parts()`.

Focused terminal R2 implementation review returns `REPLAN`. R3 requires
ordinary instantiation to pass `receipt.request.parts().0` as conversion base
while retaining the full namespace mapping; the repository rule's defining
label remains the `RepoRuleId` and default-conversion owner. Restore the
pre-packet parser/policy for BUILD, module-tag and repository-call
`Output`/`OutputList` positions; only the five dependency-label constructors
use the shared package-context owner. Add the existing BUILD integration test
file to the proof allowlist because the exact ordinary BUILD surface cannot be
proved while preserving its now-stale canonical-external rejection. No other
architecture, compatibility class, production allowlist, cap, retained owner,
DICE boundary or authentic replay requirement changes. Rust may resume only
after focused R3 rereview returns `ACCEPT`.

Focused independent R3 architecture rereview returns `ACCEPT`. The ordinary
evaluation-base owner, deferred output non-widening boundary, narrow BUILD proof
allowlist, unchanged caps and unchanged retained-state architecture are accepted.

Focused terminal R3 implementation review returns `REPLAN`. The implementation
correctly separates ordinary dependency strings from definition-owned defaults
and rule identity, but still feeds repository outputs through the ordinary
evaluation base and dependency parser. R4 selects the conversion base by
attribute kind: only the five dependency-label shapes use the extension
evaluation base; `Output`/`OutputList` use the repository-rule definition base
and preserve pre-packet `//conditions`/`//visibility` behavior. Add one imported-
rule output discriminator. No compatibility class, owner, mapping, retained
state, allowlist, cap, validation gate or replay boundary changes. Rust resumes
only after focused independent R4 architecture rereview returns `ACCEPT`.
