# Current Slug V2 Packet

Packet: `WP-6-7A-recursive-build-glob-category-design-r1`

Milestone: M7A generic Starlark/ruleset closure; BUILD glob loading semantics.

Status: docs-only source/owner audit active after terminal acceptance of
`WP-6-7A-repository-declaration-documentation-category-implementation-r2`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Objective and compatibility boundary

Design the complete Bazel 9.2 recursive BUILD-glob category, not a literal
`"**"` workaround. The authentic `@platforms//host:BUILD` call is one consumer
discriminator. No platforms, rules_rust, ruleset, `cc_common`, `cc_internal`,
parser or rule-family special case is allowed.

The design audit must classify exact, Slug-native, and unsupported/deferred
behavior for:

- recursive wildcard segments at the start, middle and end, including multiple
  `**` segments and the zero-directory match;
- ordinary `*`, mixed recursive/ordinary segments, include union, exclude
  subtraction, duplicate elimination and deterministic result order;
- source files, directories under `exclude_directories`, symlinks, dangling or
  cyclic paths, hidden names, non-UTF-8 Host names and Host-path flavor;
- subpackage boundaries, ignored directories, deleted-package policy, package
  marker changes, repository roots and external repository mappings;
- empty includes, per-pattern empty failures, all-excluded behavior,
  `allow_empty`, invalid absolute/dot/up-level/empty/embedded-`**` patterns and
  exact phase/error precedence;
- the retained flat listing path used by injected/package tests and the observed
  Host traversal path used by production requests; and
- same-DICE create/edit/delete, directory-boundary invalidation, A/B/A
  restoration, Need/error ordering, cancellation and warm reuse.

Do not assume all of those rows are already admitted. Any behavior without a
bounded Rust-native owner must be classified unsupported/deferred and fail
closed rather than silently widened.

## Bazel 9.2 authority

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Pinned starting sources are:

- `StarlarkNativeModuleApi.java`:
  `0451254c4e4f587a90d919c99a63bb469a49d80898deb1187dcf5ebd46866273`;
- `StarlarkNativeModule.java`:
  `600541da8362b71249e093552b84ee009da5e112d1c942a95eeb9c783fd16204`;
- `UnixGlob.java`:
  `f86ca1900a2d4668233771a85814bc8aaf5139808b7e27ef9d47714e125ea460`;
- `GlobCache.java`:
  `cf79d5a4a64924990936dfa1ae186aec94ea4ea9b0b7d7192c4ac30329558236`;
- `GlobDescriptor.java`:
  `8b06f007ca5ded81d72f342cb509bdad3c2ff0be70e73d876f152da45c48e310`;
- `GlobFunction.java`:
  `77a19c81fa09e9fc84bf0bd86aadfd906194faa379e12aed394976aa90ed63a6`;
- `GlobValue.java`:
  `b4ace32f5b31b2057a50d81bf0c47eec36c53aeb648acfb9cf068c9c14879c27`;
  and
- `FragmentProducer.java`:
  `410e8b8917247c774ddc4506859cc3efae5231c2d2f55507d8a5940d1b4f2dba`.

The audit must locate and pin the smallest relevant Bazel regressions for
recursive matching, subpackage pruning, excludes, files/directories and
incremental invalidation. Add no Java helper or artifact to Slug.

## Learned Slug facts to verify

Slug currently has two related owners:

- `glob.rs` owns public `GlobSpec` validation and expansion over an already
  retained flat `WorkspaceDirectorySnapshot`; it currently rejects recursive
  patterns before expansion; and
- `host_glob/traversal.rs` already parses a standalone `**` segment and performs
  observed recursive traversal with package-boundary checks. `package.rs`
  nevertheless constructs `GlobSpec` before dispatch, so the public validator
  blocks this owner from receiving the authentic pattern.

The audit must determine whether one shared immutable parsed pattern can serve
both flat-listing and Host traversal without replacing the existing observed
keys, and whether the flat path can implement identical matching from its
retained snapshot. Preserve DICE observation ownership: do not move Host I/O
into the Starlark evaluator or hold a lock across a compute.

Read `docs/developers/dice.md` before proposing key/ownership changes. Inspect
the full current glob/host-glob caller and proof graph, not only the replay
failure.

## Reuse and peer guidance

Reuse existing Buck2-derived `Arc`, `CompactString`, compact maps/sets,
`Allocative`, DICE keys and path-observation carriers. The design must account
for parsed-pattern retained size, clone cost and memory accounting before adding
another representation.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Inspect `src/load/build_glob_pattern.zig`,
`session_package_glob_computation.zig`, and the recursive package-source tests
for route pruning, recursive-segment representation and proof ideas. Copy no
Zig code, allocator/layout, IDs, limits, scheduler, cache, error, ordering or
behavior. Bazel 9.2 alone fixes semantics.

## Design deliverable and stops

This packet is documentation-only. It may edit only this manifest, the Stage 6
owner plan, Stage 9 utility ledger if a retained representation decision is
made, and canonical status. No Rust, fixture or generated file may change.

Produce an execution-ready successor with:

- exact/Slug-native/deferred rows for the complete matrix above;
- one representation and owner flow across `GlobSpec`, flat expansion and Host
  traversal, or a justified `REPLAN` if unification is unsound;
- closed production/proof/fixture allowlists, per-file complexity decisions and
  gross-line caps;
- pinned Bazel source/test hashes and one permanent oracle only if existing
  evidence cannot discriminate the gap;
- same-DICE, cancellation, error-order, package-boundary and replay gates; and
- independent architecture review before Rust.

Return `REPLAN` rather than designing an unbounded filesystem walk, duplicate
graph, eager full-repository snapshot, second cache/interner, platform-specific
special case, or semantics borrowed from Zabel.

## Immediate predecessor

The terminally accepted documentation-binding packet uses typed
`Option<NoneOr<&str>>` for the complete three-sibling gap, retains no metadata,
and passes complete loading validation. Its rebuilt replay clears
`repository_rule(doc = ...)` and exposes this recursive glob frontier.
