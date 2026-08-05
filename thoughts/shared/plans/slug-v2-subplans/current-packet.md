# Current Slug V2 Packet

Packet: `WP-6-m2-pure-native-value-default-and-rendering-kernel`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: implement only the pure native value/default/rendering kernel; every
contextual or Java-regex path must refuse explicitly.
Predecessor: accepted 17-class/341-option metadata/cache grammar `b043d54d`
and `WP-6-m2-native-value-cohort-and-rendering-design`.

Within `slug_configuration_v2`, add:

- a closed structurally equal `NativeValue` algebra for primitive/text/enum/
  dotted-version/list/entry/ordered-map/environment/fission values and the
  dedicated numeric `RunsPerTestSeed` default;
- source-default materialization with exact annotation `"null"` semantics,
  repeatable empty behavior, all literal categories, and a private pinned table
  for the six symbolic label expressions, without resolving labels;
- descriptor-directed one-occurrence conversion for exactly the 287 pure
  descriptor paths and explicit typed refusal for eight Java-regex, five Host,
  and 41 repository/package/loading descriptors; and
- exact source-backed Java `value.toString()` projection into the existing
  outer cache-field grammar, including list/entry/map/env/duration/dotted-
  version text, valid-Unicode Java UTF-16 ordering, and
  `[(?:(?>.*)) Options: [1]]` for the numeric `runs_per_test` default seed.

Apply `.codex/skills/slug-buck2-utility-reuse/SKILL.md`. Use `CompactString`
for retained dynamic scalar text, immutable `Arc<[T]>`/`Arc<[(T,T)]>` slices,
and `Allocative`. Use `Dupe` only if an introduced aggregate is demonstrably
pointer-cheap; never mark owned leaves cheap. Preserve structural equality and
null-versus-empty identity. No runtime descriptor map, interner, cache, global,
hash, generic unordered map, or derived Rust `Debug`/`Display` cache text.

Allowlist:

- `app/slug_configuration_v2/Cargo.toml`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/cache_grammar.rs`
- `app/slug_configuration_v2/src/native/tests.rs`
- `app/slug_configuration_v2/src/native/value.rs`
- `app/slug_configuration_v2/src/native/defaults.rs`
- `app/slug_configuration_v2/src/native/convert.rs`

Caps: 1,550 production, 1,250 test, and 2,800 total formatted net lines across
seven files. Add only demonstrated retained workspace dependencies; no external
dependency/version, registry, root workspace, ignored lockfile, generated
source/data, fixture, oracle, scheduling, or downstream crate change.

Acceptance requires source-pinned tests for all 341 routing results and exact
287/8/5/41 counts; all default families and six symbolic values; special-null
versus explicit `null`; repeatable empty versus scalar/list empty; every pure
converter identifier and enum spelling; the numeric-only `runs_per_test="1"`
seed and exact cache text; list/entry/ordered-map/env/duration/dotted-version
rendering; `NULL`/`EMPTY`/escaping; structural equality; Java UTF-16 ordering
with valid non-BMP inputs; and typed refusal of every contextual/unsupported
family. Reuse the independent 341-row metadata table rather than duplicate it.

Stop on Java pattern generation/rendering or a general `PerLabelOptions`;
lone-surrogate/lossy UTF-8 behavior; Host access; label/repository/package/
loading/Starlark resolution; argv/RC/repeat/expansion/implicit/alias behavior;
whole or partial P/C/T normalization; generic map/record rendering; class or
mixed checksum; command/wire/DICE/analysis/path/platform/ActionKey/aquery/
execution work; retained-state ambiguity; cap breach; or a second material
source/rendering correction. Configured-target dependency cycles remain
deferred with user approval.
