# Current Slug V2 Work Packet

Packet: WP-7-105-m7a-exported-provider-constructor-dict-keys-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit exported user-provider constructors as Starlark dictionary keys for
the authenticated rules_kotlin 1.9.6 Loading closure. Both ordinary
`provider()` and `provider(init = ...)` constructor forms, live and frozen,
must hash and compare by their existing defining-source/export-name identity.
The imported private `derive.info` key in `opts.javac.bzl:64,97` must load
and remain retrievable by the same imported key. The authentic normalized-
PATH build must pass its present `provider_callable is not hashable` stop;
a later independently typed stop is acceptable. Kotlin analysis, actions,
toolchain target success and M7A close are not claimed.

Exact Bazel 9.2 slice: an exported ordinary or initialized user-provider
constructor is a usable dict key; first export, including a private `_` name,
sets its key; same-constructor aliases/import re-exports keep that key; other
exported names/sources remain distinct. Inline or otherwise unexported
constructors and the separate initialized-provider raw constructor are
unhashable. Exactness is limited to ordinary `.bzl` Loading in the selected
external-source closure, not prelude/builtins/module-extension identities.
Slug-native: `ProviderId`'s Rust structural source-label/export-name token,
`StarlarkHasher` bytes, and existing source-fingerprint/DICE equality.
Unsupported/deferred: Bazel `BzlLoadValue.Key` context classes beyond this
ordinary closure, Java hashCode bytes, arbitrary callable hash parity,
provider-instance/configured behavior and broader Kotlin semantics. Existing
provider-instance hash barriers remain unchanged.

## Source, owner and decision

The authenticated rules_kotlin 1.9.6 selected BCR source has
`derive.bzl:1-22` (`_value_to_flag_info = provider(...)`, then
`derive = struct(info = _value_to_flag_info, ...)`), `opts.javac.bzl:64,97`
(two `derive.info` dict keys), and `convert.bzl:18-19` (`get` and indexing by
that imported key). Verified source file SHA-256 values are respectively
`62262236201716c4629a69b6b8692f3c90a1c170afdceffc50e4664f02b5e3f0`,
`e4a745d7629a04ead86819d4f33fda2ffe40d756c67e9d167f80dbbf07c92fcb`
and `8161f8699a500aedfa8cb21864aaffbbcf5ca77c97a127048f2245657b583698`.
Pinned Bazel 9.2 `BzlLoadFunction.java:1416-1430` exports newly assigned
top-level values, including `_` names, without rekeying an already-exported
import. `StarlarkProvider.java:505-524,606-642` uses its exported key for
equality/hash and is immutable only after export; `StarlarkValue.java:96-121`
and `Dict.java:599-603` gate key insertion on hashability. The
`provider(init = ...)` API returns its same provider constructor plus a
separate raw constructor (`StarlarkRuleClassFunctions.java:379-403`), whose
Bazel value is unhashable. Bazel's key includes the load-key class/context as
well as label and export name; only the ordinary external `.bzl` projection
is admitted here. Bazel dict literals evaluate key and value before the
hashability/insertion check (`Eval.java:634-650`); this packet does not change
or claim side-effect ordering.

`app/slug_loading_v2/src/provider.rs` is the sole semantic owner. Its four
live/frozen ordinary/initialized `provider_callable` values already retain
the same `ProviderId` allocated on first `export_as`; that ID is an
`Arc`/`Dupe` structural source-label/export-name pair with `Allocative`.
Implement symmetric equality and hash for all four using only a bound ID;
an unbound constructor must return a hash error and compare unequal to a
different unbound value. Keep the initialized raw constructor separately
unhashable rather than inheriting starlark-rust's generic `function` hash.
Do not key by schema, display text, pointer, importer, evaluator heap or
source bytes, and do not change `ProviderId`, schema or provider-instance
representations. Add no second key, map, interner, cache, DICE key, lock or
allocation. Existing Bzl source/recursive-manifest fingerprints own
invalidation and restoration; imported child heaps remain lifetime owners,
not semantic side storage. Ordinary and initialized callables remain in the
same compact `Arc`/`CompactString`/Allocative owner. The existing ~2,050-line
`provider.rs` remains cohesive because the change is confined to methods
beside its four callable types and raw constructor; no structural split is
needed for this bounded behavior.

## Evidence and validation

A pinned Bazel 9.2 offline scratch oracle at
`/tmp/slug-wp805-provider-key`, run with `--ignore_all_rc_files`, admits
named ordinary and initialized provider constructors as dict keys, including
private names and aliases/re-exports, and distinguishes other constructors.
Inline/unexported ordinary and initialized constructors fail at Loading with
`unhashable type: 'Provider'`; the raw initialized constructor fails with
`unhashable type: 'RawConstructor'`. This directly discriminates the present
gap and the adjacent negative boundary. The scratch stays outside this
checkout. Pinned source and this oracle supply the parity evidence; no new
Stage 1 fixture is selected unless a focused regression exposes a missing
discriminator. No `~/.bazelrc` contents, BuildBuddy credentials, remote
transport or NativeLink enter this Loading packet.

Add focused Loading tests for a source-shaped `derive` export/import through
two dictionary key insertions and a later lookup; ordinary and initialized
bound keys across live/frozen evaluation; alias/re-export and private export;
same `ProviderId` A/B/A equality/hash, distinct name/source, unexported
constructor and raw-constructor rejection. Reuse existing provider-instance
hash barrier and provider export/loaded-source tests without broadening
their claims. A public/cross-crate behavior change requires a named direct
dependent compile (`slug_cli_v2` rebuild). Resolve the pinned cargo/rustc
from `rust-toolchain`; compile Loading library tests with Cargo JSON
`--no-run`, batch exact selectors through preflight, then run selected tests
serially. Rebuild CLI before the authentic normalized-PATH smoke; check/clean
`slugd` before and after. Run formatting, `scripts/v2_archive_status.sh`,
diff and plan-status checks and obtain independent DESIGN and FINAL reviews.
No broad suite or remote-service gate is selected by this Loading packet.

## Scope and stops

Base clean main `213e20ffb`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 and Stage 9 owner contracts only after acceptance;
production `app/slug_loading_v2/src/provider.rs`; focused tests
`app/slug_loading_v2/src/host_package_load_tests.rs` and, only if a smaller
direct unit is warranted, the existing `provider.rs` test module. Production
growth under roughly 100 lines and proof under roughly 180 lines are review
triggers, not permission ceilings. REPLAN if the source-authentic key is not
an exported user provider, Bazel distinguishes ordinary vs initialized key
identity in the admitted closure, the existing `ProviderId` lacks required
semantic inputs, unbound/raw rejection requires a new retained owner, or the
authentic stop cannot be cleared without configured Kotlin behavior.

## Immediate predecessor

WP804 at `213e20ffb` accepted three selected protobuf `proto`
configuration-field declarations. Its seven focused tests, CLI rebuild and
authentic replay passed the old proto fragment stop and reached this separate
rules_kotlin provider-callable hashability error at toolchain registration
row 55. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The pinned Bazel 9.2
scratch oracle at `/tmp/slug-wp805-provider-key` distinguished named ordinary
and initialized provider keys, private export, aliases/import re-export and
the authenticated Kotlin-shaped `derive.info` dictionary from inline,
function-local, unbound initialized and raw-constructor negatives. Three
positive queries also passed in an independent fresh output base. No Stage 1
fixture was added because this source-backed, discriminating scratch oracle
and the focused Rust regression cover the selected gap without copied fixture
growth; the scratch remains outside the checkout.

The pinned-nightly Loading JSON library-test compile passed. Exact preflight
verified five nonignored selectors and all five passed: two new source-shaped
and live/frozen provider-key tests, plus protected provider-instance hash,
initialized-provider and rules_rust provider-freeze tests. The new test also
proves equal keys across a schema change at the same source/export identity.
The direct `slug_cli_v2` rebuild passed. Authentic one-shot
`slug build //:probe` from the normalized-PATH scratch workspace cleared the
old `provider_callable` hashability stop and failed independently at toolchain
registration row 55 in generated
`@@rules_kotlin+//src/main/starlark/core/repositories:compiler.bzl:67` on
`unsupported repository_rule attribute schema '_capabilities_templates'`.
No `slugd` remained before or after. Format, archive, diff and plan-status
checks passed. Compile/test/review phases were not separately timed.
No remote cache, NativeLink or BuildBuddy gate was selected. The exported
constructor-key Loading slice is accepted; generated-repository schema,
configured Kotlin semantics, target success and M7A close remain deferred.
