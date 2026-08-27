# Current Slug V2 Packet

Packet: `WP-4-5-7A-shared-module-registration-expander`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `aa79d7736`.

Result: add the two independently keyed MODULE registration families and one
shared loading driver that expands the accepted selected-pattern, contextual
syntax, root/canonical subtree and package-inventory owners into ordered
canonical labels plus retained ambiguity facts. Add no configured consumer or
semantic activation.

## Immediate predecessor

Commit `aa79d7736` independently accepts both prerequisite owners from the
frozen shared architecture. `CanonicalTargetPattern` now shares the command
parser's one absolute grammar while resolving `//`, `@apparent`, `@//` and
`@@canonical` in a declaring repository context. The selected pattern view
provides borrowed mapping point lookup. `RepositoryPackageInventoryKey` and
its observed sibling now solely own Root/Canonical external BUILD evaluation,
loaded targets, events and observation epochs beneath the unchanged public
consumer-policy adapter.

## Learned facts and research basis

Pinned Bazel 9.2 remains compatibility authority:

- `RegisteredToolchainsFunction` and
  `RegisteredExecutionPlatformsFunction` iterate selected modules and their
  declarations in order, parse under each module's canonical repository and
  final repository mapping, expand target patterns, then suppress duplicates
  by first occurrence.
- `TargetsInPackage.getWildcardConflict` checks the same-named explicit target
  only after loading. A conflict retains that exact target and emits a warning;
  without a conflict, wildcard candidates are filtered by registration family.
- Exact targets must exist but bypass wildcard family filtering. Toolchain
  wildcards retain native `toolchain` candidates. Execution-platform
  wildcards retain native `platform` and `alias` candidates; configured alias
  and `PlatformInfo` validation remains downstream.
- `RecursivePkgFunction` supplies child transitives before the direct package.
  The accepted exact order is component-wise lexical siblings, descendants
  before their prefix, and lexical target names within each package.
- MODULE inputs are positive only. Signed folding and command-line extra
  registration precedence are not part of this key.

Reuse the accepted
`tests/v2_oracle/fixtures/registration-target-pattern-syntax` evidence for
wildcard spelling, explicit-name conflict polarity/warning and recursive
child-before-parent order. Pinned registered-family source tests discriminate
exact-versus-wildcard filtering, alias retention, selected/declaration order,
deduplication and reload behavior. Add no oracle fixture unless implementation
reveals an uncovered distinction.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
`selected_registration_patterns.zig` supports compact declaration-owner
references; `registered_labels_projection.zig` supports separating canonical
text, package facts and category builders; and
`session_recursive_package_discovery.zig` supports deriving a stable-postorder
consumer projection without making that projection the subtree owner's
identity. Copy no Zig code, packed layout, store IDs, diagnostics or
compatibility claim.

Buck2/V1 utility review selects existing V2 utilities. Retain final immutable
labels and warning facts in `Arc` slices, use `SmallSet` plus an output `Vec`
for scratch first-seen deduplication, and use `SmallMap` only for bounded
per-compute route/package memoization. Reuse `CompactString`, `Dupe` and
`Allocative`; add no dependency, `HashMap`, `HashSet`, interner, process cache,
side table or Stage 9 donor import.

## Decision and non-decisions

### Public family keys and retained value

Add one public `ModuleRegistrationExpansionKey` whose structural identity is
`NormalizedAbsolutePath workspace + ModuleRegistrationFamily`. Provide
toolchain and execution-platform constructors, and add one observed sibling.
The two family instances compute independently: requesting one must not demand,
load, warn for or publish events from the other.

The successful retained value owns:

1. an ordered `Arc<[CanonicalLabel]>`; and
2. an ordered `Arc<[ModuleRegistrationAmbiguity]>`.

Each ambiguity fact retains the family, selected/declaration row position, raw
pattern spelling in `CompactString`, wildcard spelling and resolved conflict
label. It retains no repository mapping, source route, `LoadedPackage` or
evaluator heap. Warning facts participate in equality even when the label
slice does not change. Errors retain family/row context and a typed parse,
route, subtree, root-package or external-inventory cause; unproved wording is
Slug-native.

The family key stores one local `EventBatch` only for a complete semantic
terminal when event capture is requested. Its warning events are a pure
projection of the retained ambiguity facts in order. Child selected/route,
subtree and package keys remain the sole owners of their events. Need and
cancellation store no family batch.

### One shared expansion driver

Both family keys call one private driver parameterized by legacy versus
observed mode and family. It must preserve this order:

1. demand `HostSelectedRegistrationPatterns{Observation}Key`;
2. select only the requested family iterator;
3. for each row in selected/declaration order, parse through
   `CanonicalTargetPattern::parse` using the view's canonical owner and borrowed
   `mapping_target`;
4. resolve the referenced canonical repository;
5. load an exact/package inventory or subtree in pattern order;
6. append candidates in exact expansion order; and
7. suppress duplicates by first occurrence.

Root-repository packages use `RootPackageLoadKey` and its observed sibling;
nonroot packages use the crate-private general
`RepositoryPackageInventoryKey` and observed sibling, never the public policy
adapter. Canonical source input comes only from
`HostCanonicalRepositoryLoadRouteKey` or its observed sibling. Root recursion
uses `RootSubtreePackageSetKey`; canonical recursion uses
`ExternalSubtreePackageSetKey`. Do not read a directory, BUILD file or mapping
directly.

An exact target loads its package, requires the named target, and appends it
without family filtering. A package wildcard first checks its retained
conflict candidate. If present in the loaded inventory it appends only that
target and records one ambiguity fact. Otherwise sort targets lexically and
apply the family filter. Recursive rows demand the accepted subtree set,
convert complete package strings to scratch `PackagePath`s and sort with a
component-wise comparator: lexical at the first differing component and the
longer descendant before an equal prefix. Expand each resulting package as a
wildcard, with no explicit-name ambiguity candidate.

Use bounded compute scratch caches for repeated canonical routes, subtree
results and package result `Arc`s. Cache identity includes root-versus-canonical
route and package/prefix; observed cache entries carry the exact merged child
epoch. Cache hits must not reconstruct epochs or publish events. All caches,
parsed patterns, sort buffers and dedupe state are dropped on completion or
cancellation.

### Observation and DICE contract

Legacy and observed forms share the same semantic driver. Observed mode merges
epochs in actual dependency order and preserves every child result `Arc`.
Selected owner outer/Need precedes parse; route/effect precedes canonical
subtree/package source; subtree precedes its package inventories; the first
row terminal stops later rows. Legacy mode invariant-asserts observed-only
frontier errors unreachable.

Keys use complete-only equality/validity. Semantic equality includes family,
ordered labels, ordered warnings and complete typed error state. Prove warm
reuse, A/B/A restoration, overlapping family isolation and cancellation
nonpublication. In the warning-specific A/B/A case, `:all` conflict and `:*`
wildcard epochs must produce the same labels but distinct warning facts, and
restoration must restore the original facts.

No lock may cross a DICE compute. The final label/warning slices and error are
DICE-retained semantic memory. Route/package maps, parsed values, comparator
buffers and ordered-set construction are compute scratch. Event batches are
request-selected operational data. No retained result may borrow the selected
view or evaluator scratch.

### Read-only downstream boundary

Export the family key/value/error/fact API from `slug_loading_v2` and require
`slug_analysis_v2` to compile and test unchanged. Do not add an analysis DICE
dependency, replace existing direct registration adapters, retry ordinary
rules_rust, or claim configured provider/settings/alias semantics. A later
configured packet consumes only the expanded canonical labels.

## Compatibility classification

- **Exact:** admitted Bazel 9.2 MODULE `//`, `@apparent`, root/nonroot `@//`
  and `@@canonical` exact/package/recursive expansion; selected/declaration
  order; explicit wildcard conflict polarity and warning presence;
  descendant-before-prefix package priority; lexical sibling and target order;
  represented family wildcard filters; exact-target bypass/existence;
  first-seen deduplication; and selected/route/subtree/package terminal order.
- **Slug-native:** Rust public names, typed error wording outside accepted
  fixtures, key/carrier layout, structural hashes, scratch caches/comparator,
  warning event text, epoch transport and memory accounting.
- **Unsupported/deferred:** configured `ToolchainInfo`/`PlatformInfo`, target
  settings, alias resolution, custom advertised platform providers, actual
  toolchain/platform selection, command-line extra registrations and signed
  folding, semantic activation, other builtin categories, rules, actions and
  exact configuration/output bytes.

## Exact allowlist, caps and validation

Production files:

1. `app/slug_loading_v2/src/registration_expansion.rs` (new)
2. `app/slug_loading_v2/src/lib.rs`
3. `app/slug_loading_v2/src/bzl_module.rs` (crate-private inventory/root
   observation wiring only if the new cohesive module cannot consume the
   accepted surface directly)

Proof files:

4. `app/slug_loading_v2/src/registration_expansion_tests.rs` (new)

No Cargo manifest/lockfile, identity/Bzlmod/subtree/package/query/analysis/core
production, BUILD, fixture, oracle or other plan file is admitted after this
scheduling commit. `bzl_module.rs` may only remove next-packet dead-code
annotations or expose existing crate-private constructors/accessors; it may not
change evaluation or policy behavior.

Caps: at most 1,350 net production lines, 1,900 net proof lines and 3,250 net
total lines; at most 1,280 net lines in `registration_expansion.rs`, 1,850 in
its test file, 35 in `lib.rs` and 60 in `bzl_module.rs`; at most 120 lines for
any new/touched function. The new module triggers complexity review at 1,000
physical lines. STOP rather than widen the allowlist or duplicate one of the
accepted owners.

Run serially:

1. focused direct/package/recursive tests for both families;
2. selected/declaration/mapping order, root and canonical route, lexical
   postorder, duplicate and conflict-warning matrices;
3. observed dependency/epoch/event ownership, Need/error precedence,
   overlapping family isolation, cancellation and A/B/A tests;
4. complete `slug_loading_v2` tests;
5. direct dependent `slug_query_v2` and read-only `slug_analysis_v2` tests;
6. complete `slug_bzlmod_v2` and `slug_identity_v2` regression gates if the
   focused selected/contextual tests do not already cover their unchanged API;
7. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and
   `scripts/v2_archive_status.sh` against its recorded three-file baseline.

No new Bazel oracle is planned: the accepted target-pattern oracle plus pinned
registered-family source regressions discriminate this nonactivated loading
projection. Rebuild `slug_cli_v2` only if a later test invokes `SLUG_V2_BIN`;
this packet must not run the ordinary rules_rust command. Require independent
terminal review of family isolation, order/filter/conflict behavior, DICE
ownership/equality, epoch/event identity, cache scratch lifetime, utility reuse
and caps before acceptance.

Residual risk: exact Bazel warning prose is not claimed, root and canonical
package errors have distinct existing types, and the temporary public
repository package-policy adapter remains for old consumers. None authorizes
activation, a unified root/external package representation or duplicated
evaluation.

## Stops

STOP and `REPLAN` for a second parser, selected-pattern owner, mapping copy,
subtree walk, BUILD evaluator or source read; use of the public external policy
adapter by the expander; one family activating the other; root/canonical route
identity collapse; retained scratch cache/mapping/evaluator values; warning
facts omitted from equality; epoch reconstruction; duplicate family events; a
lock across DICE; process/global cache or interner; query/analysis/core edits;
configured activation; scope/cap overflow; Rust ownership of BCR rules or
`cc_internal`; a C++ parser/rule engine; or treating Zabel as compatibility
authority.
