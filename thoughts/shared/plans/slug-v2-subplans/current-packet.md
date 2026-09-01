# Current Slug V2 Packet

Packet: `WP-6-7A-recursive-build-glob-category-implementation-r3`

Milestone: M7A generic Starlark/ruleset closure; BUILD glob loading semantics.

Status: proof-allowlist correction review active after R2 ended `REPLAN` at a
stale protected assertion. The accepted architecture, compatibility classes,
production allowlist, caps and proof obligations are unchanged.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Objective and compatibility boundary

Implement the complete Bazel 9.2 recursive BUILD `glob()` category across the
retained flat `PackageListing` and observed Host traversal paths. This is not a
literal `"**"` workaround. `@platforms//host:BUILD`, rules_rust, `cc_common`
and `cc_internal` are downstream discriminators only. Change no parser, BCR
rule body, rule family, configured analysis, provider or action semantics.
Bazel 9 BCR Starlark continues to own all rules.

Admit as **exact** for valid Rust-Unicode patterns/results on the Unix Host:

- literal and ordinary `*` segments, standalone leading/middle/trailing,
  consecutive and multiple `**`, zero-directory matches, hidden-name and
  wildcard-parenthesis behavior, and literal regex punctuation;
- include union, Bazel's literal/shortcut/validated exclude split, per-include
  empty checks before exclude filtering, final all-excluded checking, explicit
  `allow_empty` and the Bazel 9.2 default false;
- arbitrary-size integer `exclude_directories`, where zero includes directories
  and every positive or negative nonzero value excludes them;
- file/directory selection, package-root omission, leading-`@` projection,
  Java UTF-16 result order and duplicate removal;
- package and ignored-directory stops, deleted-package continuation, dangling
  symlink omission and source-spelled resolvable symlinks; and
- semantic failure for matched literal/wildcard/recursive symlink cycles and
  recursive unbounded symlink expansion, while unmatched cycles remain inert.

Keep as **Slug-native** Rust Host observations and deterministic DICE
Need/error scheduling, typed Rust filesystem/cycle diagnostics,
starlark-rust argument diagnostic decoration, non-UTF-8 matched-output
fail-closed behavior, and the injected flat path's eager immutable snapshot and
symlink/special-entry failure boundary.

Keep **unsupported/deferred** non-Unix traversal, NUL and Java unpaired-UTF-16
edges, historical filesystem snapshots,
`--incompatible_disallow_empty_glob=false`, `subpackages()`, exact Skyframe
node/prefetch topology, and ignore/deleted-package policy on the legacy flat
scaffold. No successful path may fall back to an unobserved scan.

## Bazel 9.2 authority

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Pinned sources are:

- `StarlarkNativeModuleApi.java`
  `0451254c4e4f587a90d919c99a63bb469a49d80898deb1187dcf5ebd46866273`;
- `StarlarkNativeModule.java`
  `600541da8362b71249e093552b84ee009da5e112d1c942a95eeb9c783fd16204`;
- `UnixGlob.java`
  `f86ca1900a2d4668233771a85814bc8aaf5139808b7e27ef9d47714e125ea460`;
- `GlobCache.java`
  `cf79d5a4a64924990936dfa1ae186aec94ea4ea9b0b7d7192c4ac30329558236`;
- `GlobValue.java`
  `b4ace32f5b31b2057a50d81bf0c47eec36c53aeb648acfb9cf068c9c14879c27`;
- `GlobsValue.java`
  `4dc7dc58d3c53fc81598f27cf9b2d5527c561489816371964e3a7dd52dd9af43`;
- `PackageFunctionWithMultipleGlobDeps.java`
  `6c1c6bfdf88fa008dce55ceaf31751ab9993266a537d04c3e327e2c644f84ddc`;
- `GlobberUtils.java`
  `0942ec734eca33c22df6d5576773458e48e6ae6408c952c5ffceedf482ec2adf`;
  and
- `BuildLanguageOptions.java`
  `b01e106ef0ff7af458766248bce7799b49c0f54fc14d023a8297aeb7dbfb44e5`.

Pinned regression owners are `GlobTestBase.java`
`4d6769e18428e2540fcf2b022fd1dc52748fcbabd5abd3ae3c9c02bb95cc57e2`,
`GlobCacheTest.java`
`874d8e4c7c09d257270a4da238de06c4afeed22ff18d19380bfc3aab67486d0a`,
`GlobTest.java`
`d37947c41d912ece50bb4b3980b974befc0d8997a79145de6de88248935135b2`
and `PackageFunctionTest.java`
`62c104416003600ec390b795c07f7b3e62d46aaf61b707f746eb94623c014d06`.
Reuse them; add no Java helper, checked-in oracle or copied fixture.

## Frozen architecture

One V2-owned immutable `GlobPattern` serves `GlobSpec`, flat matching,
`HostGlobLoadingRequest`, Host traversal and segment candidate keys. Retain one
`Arc<str>` raw spelling and one compact immutable fragment slice. Ordinary
fragments retain checked byte ranges plus a literal/wildcard discriminator;
recursive fragments are a unit enum. `GlobSegmentPattern` is a cheap
pattern-Arc/index view whose manual equality/hash uses only the segment bytes
and discriminator, preserving cross-pattern segment-key sharing.

Derive or preserve `Allocative` and `Dupe`. Add no owned `String`/`Vec` graph
field, interner, regex cache, second parser, semantic side store, DICE key,
cache, task or lock. Use phase-local linear-space dynamic matching for multiple
`**`; never add exponential backtracking or an eager repository walk.

`GlobSpec::new` parses includes in source order. Excludes retain one of exact
literal removal, Bazel's raw prefix/suffix `**/*` shortcut, a validated shared
pattern, or a deferred parse error. Complete all includes and per-include empty
checks before surfacing an exclude error or filtering. Excludes never activate
Host traversal.

Host segment matching remains byte-wise. Callable-facing matching uses valid
Rust Unicode; only validated complex excludes admit `?`, consuming one Unicode
scalar. Parentheses are literal for literal segments and ignored only in a
wildcard segment. `PackageRecorder::glob` owns final raw-path exclusion,
non-UTF-8 rejection, leading-`@` projection, UTF-16 sorting and deduplication.

Both global and `native.glob` bindings use one private typed unpacker in
`package.rs` for `exclude_directories`. It admits only runtime type `int`, uses
`Value::to_bool()` transiently, and retains only the boolean in `GlobSpec`; add
no bigint dependency or graph state.

Preserve existing ownership: `PackageListingKey` owns flat candidate/boundary
facts; Host traversal, segment and package-boundary observation keys own Host
discovery; `PackageLoadKey` retains the successful package and glob identity.
No evaluator borrow enters DICE and no lock crosses a compute.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Its explicit recursive states, route pruning and proof themes may guide shape;
copy no Zig representation, behavior, allocator, key, scheduler, cache,
ordering, diagnostic or limit. Bazel 9.2 alone fixes semantics. Buck2/
starlark-rust supplies only already-retained `Arc`, `Dupe`, compact collection
and `Allocative` utilities.

## Closed ownership and caps

Production allowlist:

- `app/slug_loading_v2/src/glob.rs`;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/host_glob/mod.rs`;
- `app/slug_loading_v2/src/host_glob/adapter.rs`; and
- `app/slug_loading_v2/src/host_glob/traversal.rs`.

Proof allowlist:

- `app/slug_loading_v2/tests/glob_boundaries.rs`;
- `app/slug_loading_v2/tests/glob_invalidation.rs`;
- `app/slug_loading_v2/src/host_glob/tests.rs`;
- `app/slug_loading_v2/src/host_glob/adapter_tests.rs`;
- `app/slug_loading_v2/src/host_glob/traversal_tests.rs`; and
- `app/slug_loading_v2/src/host_package_attempt_tests.rs`; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, limited to replacing
  the one stale exclude-traversal dependency expectation in
  `observed_root_package_preserves_semantics_arcs_order_events_and_families`.

Do not touch the parked proof, any other `host_package_load_tests.rs` line,
fixtures, generated files or another crate. Gross caps are 1,000 production, 1,000 proof
and 1,720 total Rust lines; deletions and moves count. In `package.rs`, change
only the existing glob/binding corridor; `glob.rs` remains the shared semantic
owner.

R1 ended `REPLAN` at its measured 987 production/624 proof/1,611 total gross
candidate. The 720 production estimate counted the replacement architecture
but undercounted more than 300 gross deleted lines from removing two
superseded Host pattern implementations and their adapter plumbing. R2 changes only the
production cap to 1,000; it adds no file, semantic row, proof exception, DICE
owner, cache, lock or eager walk. The unchanged 1,720 total cap still bounds
the complete packet. Independent review recomputed exactly 987 production, 624
proof and 1,611 total gross lines and returned `ACCEPT`.

R2 ended `REPLAN` when the required full loading suite proved that the named
protected test still required an observed Host traversal dependency for the
exclude `sub/no.txt`. The accepted architecture intentionally filters excludes
after the include union and forbids that dependency. R3 adds only the exact
proof assertion above; changing its three expected traversal keys to the two
include keys adds two gross proof lines, for 987 production/626 proof/1,613
total. It does not authorize another change in the 37,117-line file.

Stop with `REPLAN` if another production file is required, segment-key equality
cannot remain byte/discriminator based, or the implementation needs a new DICE
key/cache/lock/eager walk.

## Required proof and validation

Cover the complete validation/matcher/order table: multiple and zero-directory
`**`, literal/special/wildcard/hidden/parenthesis cases, include-`?` rejection,
complex exclude-`?`, literal/shortcut exclude quirks, deferred complex-exclude
error precedence, package-root omission, directories, leading `@`, UTF-16 order
and flat/Host agreement.

Both bindings must prove default, zero, small and arbitrarily large positive
and negative `exclude_directories`, plus boolean/string/`None` rejection before
glob evaluation. Host proofs must show no exclude traversal, dangling and
resolvable symlinks, literal and wildcard matched cycles, recursive cycle/
unbounded expansion, an unmatched inert cycle, boundary behavior, non-UTF-8
failure, Need/error precedence, cancellation and complete-only equality.

Same-DICE proof covers recursive create/delete/recreate, nested membership,
BUILD/BUILD.bazel markers, ignore and deleted-package A/B/A, warm reuse and
equal restoration. Run formatting, `git diff --check`, focused glob/Host tests,
the full `slug_loading_v2` suite, named direct loading/query dependents, source
hashes and `scripts/v2_archive_status.sh`. Rebuild `slug_cli_v2`, clean `slugd`
before and after, and rerun the authentic BCR replay. It must clear
`@platforms//host:BUILD` without a special case; the next genuine generic
failure selects the next packet.

## Immediate predecessor

Commit `6b1a27c29` terminally accepts repository declaration documentation
binding. The accepted glob design was independently reviewed, corrected once
for arbitrary integer and cycle coverage, and accepted before this Rust packet.
