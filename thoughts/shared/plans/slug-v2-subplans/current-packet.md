# Current Slug V2 Packet

Packet: `WP-4-5-7A-direct-config-setting-matching`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `aaf23abcc`.

Result: add the sole configured-condition DICE owner and exact direct matching
for native `values`, `define_values`, and typed Starlark `flag_values` across
the complete admitted build-setting category. Keep `constraint_values`
fail-closed until the configured target-platform fact exists. This packet owns
condition truth, not selector resolution or command occurrence parsing.

## Accepted predecessor and boundaries

Commits `b949ce8da`, `57b1e8a1f`, `84bda1971`, and `aaf23abcc` accept the
category architecture, four-field loading predicate, sole typed scoped-option
map, declaration-authenticated effective-value resolver, and generic typed
`ctx.build_setting_value`. Configuration contains only normalized nondefault
overrides; declarations remain loading-owned and defaults are read in place.

Buck2-derived Rust remains the sole syntax/evaluator owner. BCR Starlark owns
every rule and control path including `cc_internal`; `cc_common` is a demanding
client of the generic evaluator/provider/host ABI, never a Rust C++ parser or
rule engine. Pinned Bazel 9.2 at
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` supplies peer ownership and compact
matching guidance only.

## Live preflight

- Loading already publishes one `ConfigSettingTarget` with canonical normalized
  `values`, `define_values`, `flag_values`, ordered `constraint_values`,
  provenance, and semantic label dependencies. All-empty declarations load so
  the configured owner can issue the Bazel-shaped semantic failure.
- `SlugConfiguration` owns the complete typed native option vector and the sole
  typed Starlark override map, but exposes no matching API. Native converters,
  defaults, descriptor metadata, repeatability, list and map occurrences are
  private to `slug_configuration_v2`; analysis must not reconstruct them.
- `build_setting.rs` already converts loading declarations to effective typed
  values. It lacks only the `flag_values` string converter/matcher. No new
  retained build-setting representation is needed.
- Analysis has no configured-condition key or match result. Existing configured
  package DICE routing already distinguishes root and canonical repositories
  and is the required declaration dependency owner.
- No configured target-platform fact exists for constraint membership. A
  nonempty `constraint_values` predicate therefore cannot be truthfully
  evaluated in this packet.

## Implementation contract

### One native matching boundary

Add one borrowed, allocation-bounded native config-setting matcher owned by
`slug_configuration_v2`. Its public input is the loading predicate's raw
`values` and `define_values`; do not expose `OptionRecord`, `OptionValue`,
converter families, ordinals, Java-shaped objects, or a second configuration
view. Resolve old names to the existing canonical descriptor when admitted,
reject unknown, internal/non-configurable, unsupported, and malformed values,
convert expected text through the same native converter/default metadata, and
compare against the configuration's existing typed occurrence.

Match Bazel 9.2 option behavior over the admitted descriptor universe:

- scalar/absent values use typed equality after conversion;
- repeatable list expectations use collection containment, including exact
  empty/empty behavior;
- repeatable map expectations contain one entry and use the last actual entry
  with the same key;
- each `define_values = {key: value}` entry is the native `define` occurrence
  `key=value`, so repeated actual keys use the same last-wins rule;
- all predicates are conjunctive and a valid mismatch returns `false`, while an
  invalid name/value or unsupported converter is a semantic error.

Parsing is scratch. Retain no expected-value cache, copied option vector,
lookup map, boxed dynamic converter, text digest, or diagnostic diff in the
configuration. The accepted structural configuration bytes and identities do
not change.

### Complete typed `flag_values` matching

Extend the existing analysis build-setting module with one pure
declaration-owned expected-text matcher. Load each canonical flag target from
its own configured package, require a valid build-setting declaration, obtain
override-or-default through the accepted effective-value resolver, and convert
the predicate text by declaration kind:

- integer uses Bazel/Starlark base-aware arbitrary-precision parsing;
- Boolean uses the pinned Bazel Boolean converter spellings;
- ordinary string is exact text;
- `allow_multiple` string converts one scalar and matches membership in the
  effective list;
- string-list converts comma-separated text, requires exactly one converted
  member, and matches membership in the effective ordered list;
- string-set converts comma-separated text to unique membership, requires
  exactly one member, and matches membership in the effective set.

Invalid text, an empty or multiple-member list/set expectation, a wrong target,
or a configured kind/scope disagreement is an error rather than no-match.
Actual list order and duplicate count do not affect membership matching;
configuration identity remains order-sensitive for string-list and normalized
for string-set as already accepted. Do not add aliases, feature flags,
label-valued build settings, or a second expected/effective value enum.

### Sole configured-condition DICE owner

Add one public analysis DICE key identified by workspace and a
`ConfiguredTargetKey` whose label is the canonical `config_setting` target and
whose configuration is structural. It loads that exact target through the
existing Root/Canonical configured-package path, requires
`PackageTargetKind::ConfigSetting`, and owns match/no-match/error. It evaluates
native, define, and flag predicates conjunctively and loads each referenced
flag declaration at most once per key computation.

An all-empty predicate is a semantic error before matching. Any nonempty
`constraint_values` is an explicit unsupported/deferred error naming the
missing configured target-platform fact; never silently ignore it or return
false. Duplicate canonical flag keys remain a loading failure. Need, loading
failure, semantic failure, and cancellation publish no condition result. A
later corrected request through the same dependency graph recovers normally.

Return one compact match/no-match value suitable for the next selector packet.
Do not analyze the config-setting as a Starlark rule, fabricate a
`ConfigMatchingProvider`, retain referenced declarations, or make condition
labels configured dependencies of their eventual selector branches.

## Compatibility classification

- **Exact:** direct native scalar/list/map and `define_values` matching for the
  admitted descriptor/converter universe; all five typed build-setting text
  conversions; collection membership semantics; conjunctive matching;
  all-empty failure; canonical external flag declaration lookup; and
  match/no-match/error separation.
- **Slug-native:** Rust layout, compact scratch, configured-condition result
  representation, structural configuration identity, and unproved diagnostic
  wording.
- **Unsupported/deferred:** `constraint_values` until the configured
  target-platform fact, native flag aliases/disabled-select warnings, feature
  flags, label/label-list build settings, selector resolution, condition
  specialization/ambiguity, command text occurrences and precedence,
  transitions beyond the accepted subset, providers, platform/toolchain
  selection, Bazel checksum/output bytes, and wider rule flow.

## Proof obligations

1. Pure native matching discriminates scalar equality, absent/null, list
   containment and empty behavior, last-wins map entries, `define_values`
   key/value assembly, unknown/non-configurable names, invalid text and
   unsupported converters without changing configuration identity.
2. Direct configured conditions match and mismatch representative native
   values and `define_values`; multiple native/define entries are conjunctive.
3. `flag_values` covers integer beyond i64 and base-aware text, every admitted
   Boolean spelling, exact string, allow-multiple membership, ordered-list
   membership, normalized-set membership, defaults and nondefault overrides.
4. List/set expected text converting to zero or multiple members errors;
   malformed integer/Boolean text, wrong target kind, configured kind/scope
   mismatch and missing package fail closed.
5. One condition can combine native, define and multiple typed flag predicates;
   one mismatch makes the result false and no source-order shortcut changes
   Need/error precedence.
6. A canonical-external config-setting and canonical-external flag declaration
   resolve from their own selected repositories. Their package activations are
   observable, deduplicated, and invalidated independently from root packages.
7. All-empty declaration and nonempty constraints error before a condition
   result publishes; the constraint error explicitly preserves the category-4
   target-platform boundary.
8. Predicate/declaration/configuration A/B/A restores condition truth and DICE
   identity. Need, semantic failure and deterministic cancellation publish no
   condition result; cold recovery through the same graph publishes one result.
9. No second native option store, build-setting converter, effective-value map,
   condition matcher, provider payload, selector resolver or evaluator value is
   retained.

Use pinned Bazel `ConfigSetting`, `ConfigSettingTest`,
`CoreOptionConverters.BUILD_SETTING_CONVERTERS`, `BuildSetting`, and
`ConfigStringSetTest` evidence. Add an oracle only for a named converter or
matching shape not discriminated by those pinned sources/tests.

## Ownership and memory

`SlugConfiguration` remains the only retained native/effective-override owner.
Loading packages remain the only predicate and build-setting declaration
owners. The configured-condition DICE value retains only truth plus key
identity; native expected occurrences, text conversion buffers, declaration
joins and mismatch details are bounded scratch and drop on completion or
cancellation. Use borrowed option rows and existing immutable `Arc` data; add
no global table, interner, heap, lock or workspace-wide condition cache.

Zabel's separation of canonical options, typed condition inputs and configured
matching is useful ownership guidance. Slug implements that separation through
its existing configuration/loading/analysis producers and copies no Zig
representation, algorithm, diagnostics, checksum policy or behavior.

## Allowlist and caps

Production:

1. `app/slug_configuration_v2/src/native/mod.rs`;
2. `app/slug_configuration_v2/src/native/configuration.rs`;
3. `app/slug_configuration_v2/src/native/matching.rs`;
4. `app/slug_analysis_v2/src/lib.rs`;
5. `app/slug_analysis_v2/src/build_setting.rs`;
6. `app/slug_analysis_v2/src/dice.rs`.

Proof:

7. `app/slug_configuration_v2/src/native/tests.rs`;
8. `app/slug_analysis_v2/tests/starlark_rule.rs`;
9. `app/slug_analysis_v2/tests/configured_target.rs`.

Completion docs remain the canonical plan, this manifest and Stage 6 owner
plan. Caps: 900 production Rust lines, 1,250 proof Rust lines, 2,150 total Rust
lines and 220 completion-ledger lines. `matching.rs` is the sole native
converter/comparator boundary; `build_setting.rs` is the sole typed flag-text
boundary; `dice.rs` owns dependency orchestration and publication.

## Validation

Run serially: pure native scalar/list/map/define matching tests; all typed
flag-text converter/membership tests; focused root and canonical-external
configured-condition lifecycle tests; all-empty/constraint/Need/error/
cancellation/recovery proofs; `cargo test -p slug_configuration_v2`; `cargo
test -p slug_analysis_v2`; locked checks for analysis and every direct consumer;
`cargo fmt --all -- --check`; `git diff --check`; exact allowlist/caps; named
archive baseline; and independent native-converter/DICE/retained-memory review.

## Stops

STOP and `REPLAN` for a required file outside the allowlist; a second native
option/value/scope/predicate/condition store or converter; configuration
reconstruction in analysis; exposed internal option records; evaluator value
retention; text-only actual values; copied defaults; source-order-dependent
truth; constraint matching without the configured target-platform owner;
selector/provider/toolchain/command-overlay work; Rust BCR rule flow,
`cc_internal` or `cc_common` parsing; Zabel authority; a lock across DICE; cap
overflow; or a material contract correction.
