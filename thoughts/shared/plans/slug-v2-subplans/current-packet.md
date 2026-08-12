# Current Slug V2 Packet

Packet: `WP-5-host-module-version-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement one exact shared Bazel 9.2 module-version semantic domain
without activating the Host discovery-to-MVS graph.

## REPLAN predecessor

`WP-5-host-selected-module-graph-owner-design-r3` ends `REPLAN`. Commit
`dbeb1fb9` closes effective root/command/default override classification, but
the live representations still do not expose one reusable exact version
domain to discovery and selection:

- root `module()`, `bazel_dep()`, `single_version_override()`, and
  `multiple_version_override()` validate version spellings but retain the raw
  `+build` suffix, although Bazel discards it before constructing module keys;
- the nonroot evaluator strips `+build`, while public `NonrootModuleKey::new`
  accepts an unchecked string and `HostDiscoveredModuleKey` assumes its caller
  supplied an already-normalized effective key;
- `lockfile_v28.rs` contains the already accepted exact Bazel 9.2 parser and
  ordering, but only as private lockfile-specific `LockfileModuleVersion`
  machinery; and
- `registry.rs::compare_versions` belongs to the forbidden supplied-file
  `ResolvedGraph`, does not implement Bazel `Version` ordering, and cannot be
  promoted into the Host graph.

A selected graph cannot truthfully rewrite discovery edges, choose maxima, or
snap multiple-version overrides while these inputs can disagree. Copying the
lockfile comparator into MVS would create a second semantic owner.

## Source authority

Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is authoritative, especially
`Version`, `ModuleFileGlobals`, `ModuleThreadContext`, `Discovery`,
`Selection`, `ModuleKey`, and the lockfile `ModuleKey` adapter.

The accepted source audit establishes that a version is
`RELEASE[-PRERELEASE][+BUILD]`; release identifiers exclude hyphens,
prerelease/build identifiers admit them, numeric identifiers are unsigned
64-bit values, and empty is a distinguished nonregistry version greater than
all nonempty versions. Build metadata is validated and then discarded.
Equality/hash use the normalized spelling. Ordering is lexicographic over
identifier lists, compares numeric identifiers numerically then by original
spelling, orders numeric identifiers before nonnumeric identifiers, orders a
prerelease below the same release, and orders empty last.

`Discovery.applyOverrides` rewrites root-name dependencies to the root key,
nonregistry overrides to empty, and nonempty single-version overrides to their
normalized version before requesting module files. `Selection` then uses that
same `Version` ordering for maxima, multiple-version ceilings, deterministic
resolution strategies, and graph rewriting. No lockfile spelling or legacy
canonical repository name participates in this identity.

## Design to freeze

Audit and freeze one crate-private shared semantic owner, provisionally
`BazelModuleVersion`, with:

- a fallible parser over valid Unicode Rust strings that implements the pinned
  ASCII grammar, unsigned-64-bit bound, normalized spelling, and empty
  sentinel;
- exact `Eq`, `Hash`, and `Ord` semantics shared by root evaluation,
  nonroot/effective discovery keys, lockfile-v28 module keys, and the future
  selected graph;
- a compact retained form using `CompactString` and immutable Arc-backed
  identifier storage only if the audit proves cached parsed identifiers are
  measurably justified; otherwise retain the normalized compact spelling and
  one shared comparison routine without per-consumer copies; and
- typed adapter errors so root evaluation, nonroot evaluation, lockfile JSON,
  and future selection preserve their existing owner-specific diagnostics
  without duplicating validation.

The design must inventory every semantic version field and constructor in
`module_eval.rs`, `interim_module.rs`, `source_preparation.rs`,
`lockfile_v28.rs`, and the future graph seam. It must decide the smallest
bounded migration that guarantees all Host-discovered keys are validated and
normalized before any DICE lookup and that root header/dependency/registry
override values no longer retain build metadata. Existing public scaffolding
may remain string-shaped only where it cannot enter the Host selected graph
without crossing the sole checked adapter.

The accepted lockfile behavior must remain byte-for-byte stable. The old
registry `compare_versions`, `ModuleKey`, and `ResolvedGraph` stay legacy-only;
do not silently redirect them into production or claim their tests as Host
MVS evidence.

## Completed design audit

The sole owner is a new crate-private `module_version.rs` value:

`BazelModuleVersion { normalized, release, prerelease }`.

The public shape is intentionally absent. `normalized` is a `CompactString`;
release and prerelease are immutable Arc slices of compact identifiers, each
retaining numeric/non-numeric kind, unsigned numeric value, and original
identifier spelling. Empty uses an enum sentinel and allocates no identifier
slices. Clones are Arc-cheap for future graph rewriting. Equality and hash use
only normalized spelling; ordering uses the retained parsed identifiers. This
keeps repeated MVS comparisons linear in identifier count without reparsing or
introducing an interner/global cache.

The exact truth table is:

| Surface | Accepted/ordering result |
| --- | --- |
| empty | valid and greater than every nonempty version |
| `1`, `1.alpha.2`, `1-a-b.2` | valid |
| `1+build-1.2`, `1-a+build-1.2` | valid; suffix discarded |
| `18446744073709551615` | valid unsigned numeric identifier |
| `_`, non-ASCII, `1..2`, `1-`, `1+`, `1+a+b` | invalid |
| `18446744073709551616` | invalid numeric overflow |
| `1+a` versus `1+b` | equal/hash-equal normalized `1` |
| `1-01` versus `1-1` | distinct; `1-01 < 1-1` by spelling tie-break |
| `1` versus `1.0` | `1 < 1.0` by lexicographic list length |
| `1-a` versus `1` | prerelease first |
| `2` versus `10` | numeric `2 < 10` |
| `1` versus `alpha` | numeric identifier first |

The migration adapters are singular:

1. `module_eval.rs` maps the shared parser error to the existing directive
   diagnostics and stores only normalized strings in root header,
   dependencies, single-version overrides, ordered multiple-version override
   entries, and nonroot values. Normalized duplicates in an ordered multiple
   override remain duplicate elements, matching Bazel's parsed `ImmutableList`.
2. `lockfile_v28.rs` replaces its private parser/order/type with the shared
   value and maps its typed parse error to the existing direct-adapter
   `LockfileParseError`; rendering and normalized duplicate-key behavior stay
   unchanged.
3. `HostDiscoveredModuleKey::try_new` parses and normalizes before constructing
   the DICE key. Its retained `NonrootModuleKey` remains string-shaped for the
   accepted public evaluator scaffold, but no unchecked spelling can enter the
   Host discovery graph. The future selected graph retains
   `BazelModuleVersion` directly and crosses this same checked constructor.

`ModuleSourcePreparationKey` and lower source/closure keys keep compact strings
because their sole production Host caller receives the normalized discovered
key. `interim_module.rs` therefore needs no migration or new public type.
There is no source observation, DICE compute, lock, network access, or mapping
work in the version owner.

## Active implementation contract

Implement only
`WP-5-host-module-version-owner-implementation` in these six files:

- new `app/slug_bzlmod_v2/src/module_version.rs`;
- `app/slug_bzlmod_v2/src/lib.rs` for one private module declaration only;
- `app/slug_bzlmod_v2/src/module_eval.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/lockfile_v28.rs`; and
- `app/slug_bzlmod_v2/src/lockfile_v28_tests.rs`.

No seventh file, public export/API, `interim_module.rs`, `registry.rs`, legacy
resolver, DICE input/schema, fixture/oracle, Cargo/BUILD metadata, dependency,
cache/interner/global, filesystem/network observation, selected graph/MVS,
mapping, loading, or consumer is authorized. Cap formatted net growth at 240
production lines, 360 test lines, and 600 total.

Required implementation proof:

- exhaustive focused grammar/normalization/equality/hash/order tests for the
  table above and property checks that `cmp == Equal` exactly when equality;
- root/nonroot header, dependency, single/multiple override, and discovered-key
  build-suffix normalization, with invalid/overflow diagnostic preservation;
- checked Host key rejection before any source/closure/builtin child key
  activation;
- lockfile-v28 focused and full byte-for-byte parse/render/order/duplicate/error
  regression, proving no adapter-surface change;
- real-DICE root and Host `+build` A/B/A plus spelling-equivalent cold/warm
  reuse, and semantic version A/B/A invalidation/restoration;
- full `slug_bzlmod_v2`, direct core/runtime dependents, formatting, diff,
  archive, exact allowlist/cap, and structural scans proving one parser/order
  owner and no production call to legacy `registry.rs::compare_versions`; and
- fresh independent representation/implementation review.

Return `REPLAN` on any required public type, second parser/order,
lockfile behavior change, unchecked Host key, seventh file, graph/MVS breadth,
cap excess, or independent-review blocker.

## Compatibility

Exact: Bazel 9.2 version grammar, normalized spelling, empty sentinel,
unsigned numeric bounds, identifier/prerelease ordering, build-metadata
discard, and structural invalidation for normalized versions used by the
actual Host graph.

Slug-native: Rust error/type names, compact retained representation, and DICE
key/display framing.

Unsupported/deferred: selected discovery/MVS itself, canonical/full repository
mappings, selected RepoSpecs and yanked policy, extension identities/execution,
lockfile production, package/Bzl loading, configured analysis/toolchains/
actions, Test, execution/results/BEP/coverage, native Windows command-path
semantics, JVM/Java, and exact Bazel identity bytes.

## Accepted design evidence

The pinned-source/live-owner audit, compact representation review, exact truth
table, adapter inventory, scope/cap check, routing record, and independent
reserved architecture review returned `ACCEPT`. The former four-document
design scope is historical and grants no files or actions in this packet.

## Accepted predecessor evidence

Commit `dbeb1fb9` and its independent final review accept the effective
override owner at +189 production/+416 test/+605 total. That historical
implementation scope grants no files or actions in this packet.
