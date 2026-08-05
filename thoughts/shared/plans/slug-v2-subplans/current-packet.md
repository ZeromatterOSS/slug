# Current Slug V2 Packet

Packet: `WP-6-m2-native-configuration-metadata-and-cache-grammar`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: introduce the source-complete native configuration descriptor boundary
without executing context-dependent option semantics.
Predecessor: the accepted `WP-6-m2-bazel-9-target-configuration-input-ledger`,
which records all 17 classes/341 options, every graph-derived input and owner,
and the required serial implementation route.

Create a lowest-level `slug_configuration_v2` crate containing:

- one immutable static descriptor slice for all 341 pinned Bazel 9.2 options,
  preserving fully qualified class order and option-name order;
- every ledgered canonical name, old name, raw default literal/source
  expression, field type, converter identifier, repeat flag, expansion,
  implicit requirements, and class normalizer identifier; and
- the exact native cache-field grammar for `NULL`, `EMPTY`, and quoted scalar
  values with backslash/quote escaping, without computing a mixed checksum.

The crate may initially use only retained utility dependencies. Apply
`.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing: prefer a
static slice and borrowed static strings, with no runtime map, interner, cache,
global, or invented hash. Preserve descriptor identity structurally.

Allowlist:

- `Cargo.toml`
- `app/slug_configuration_v2/Cargo.toml`
- `app/slug_configuration_v2/src/lib.rs`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/registry.rs`
- `app/slug_configuration_v2/src/native/cache_grammar.rs`
- `app/slug_configuration_v2/src/native/tests.rs`

Caps: 2,400 production, 1,400 test, 3,800 total formatted net lines across
seven files. The intentionally ignored workspace `Cargo.lock`, existing crates,
generated source/data, fixtures, oracle records, and external dependency
versions must not change.

Acceptance requires source-backed tests for exactly 17 ordered classes and 341
unique ordered options. Tests must carry an independent compact pinned expected
row for every descriptor—not expectations derived from the production slice—
and compare every metadata field. They must also call out the three formerly
missed rows `test_filter`, `xcode_version`, and `start_end_lib`; old-name,
repeat, expansion, implicit-requirement, constant-default, and all P/C/T
normalizer metadata families; and exact `NULL`/`EMPTY`/backslash/quote
cache-field bytes. Run crate tests, formatting, workspace/archive/scope/cap/
diff checks, and independent retained-representation review.

Stop on any omitted/duplicated/misordered descriptor or metadata field; source
ambiguity; converter execution; typed/default value normalization; argv, RC,
`--config`, Host OS/CPU, repository mapping, label resolution, platform,
Starlark setting/scope, `PROJECT.scl`, transition, command/wire, DICE, checksum,
analysis-key, configured-path/platform/ActionKey/aquery/execution work; a new
map/interner/cache/global/hash; or cap breach. Configured-target dependency
cycles remain deferred with user approval.
