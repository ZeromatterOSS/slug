# Current Slug V2 Packet

Packet: `WP-4-5-7A-typed-build-setting-value-resolution`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `84bda1971`.

Result: delete the temporary copied-default string bridge. Authenticate every
admitted configured input and transition output against its loading-owned
build-setting declaration, resolve all five typed values and declaration scope,
retain only nondefault overrides, and expose the effective typed value through
`ctx.build_setting_value`. This packet completes category-2 value resolution;
it does not parse real command occurrences or match configured conditions.

## Accepted predecessor and boundaries

Commits `b949ce8da`, `57b1e8a1f`, and `84bda1971` accept the full-category
architecture, all five loading declarations, and the sole typed scoped-option
map. The map is immutable, canonical-label sorted, kind/scope preserving, and
already owns target-to-exec projection. Its migration deliberately preserved
one default-equal string row pending this packet.

Buck2-derived Rust remains the sole syntax/evaluator owner. BCR Starlark owns
every rule and control path including `cc_internal`; `cc_common` is a demanding
client of the generic evaluator/provider/host ABI, never a Rust C++ parser or
rule engine. Pinned Bazel 9.2 at
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` supplies peer ownership and compact-
publication guidance only.

## Live preflight

- Analysis still discovers one legacy required string label, copies its loaded
  default into configuration, and assumes `Default` scope. The explicit bridge
  arrives as one already typed string entry but is not authenticated against
  the declaration's kind, default or scope.
- Transition execution accepts exactly one declared output but requires a
  string and inserts it with `Default` scope. It cannot produce integer,
  Boolean, string-list or string-set values and cannot elide a default.
- `AnalysisContext` retains `Option<CompactString>` and exposes only string
  `ctx.build_setting_value`. The loading target already owns a complete
  `BuildSettingDeclaration`; no new declaration store is needed.
- The accepted option map can represent the full category. The missing owner is
  one declaration-authenticated resolver shared by explicit and transition
  paths plus one evaluator allocation boundary for context access.

## Implementation contract

### One declaration-authenticated value resolver

Add one analysis-owned module that converts between loading
`BuildSettingDeclaration`, evaluator-independent `StarlarkOptionValue`, and
short-lived Starlark values. It is the sole configured build-setting converter
for all five kinds. Do not add kind-specific stores, per-call text encodings or
another effective-value enum.

For a canonical setting label, load its package through the existing Root or
Canonical configured-package DICE path, require the exact target, require a
valid build-setting declaration, and retain no duplicate declaration. Convert
loading default and scope into the existing configuration value/scope types.
Resolve a candidate only when its shape matches the declaration:

- integer accepts arbitrary-precision Starlark integer and retains `BigInt`;
- Boolean accepts only Boolean;
- ordinary string accepts only string;
- `allow_multiple` string accepts only a list of strings and uses a singleton
  list containing the scalar declaration default when no override exists;
- string-list accepts only a list of strings, preserving order and duplicates;
- string-set accepts a set or list of strings and normalizes once to sorted
  unique membership.

Wrong target, malformed declaration, kind mismatch or wrong element type fails
closed before a configured child key is published. Do not stringify evaluator
values or parse command text in this packet.

### Effective nondefault map invariant

Compare the normalized typed candidate with the declaration's effective
default. A default-equal candidate removes/omits that canonical row; a
nondefault candidate inserts exactly one row with scope derived from the
declaration, ignoring any placeholder scope carried by the temporary explicit
bridge. Unrelated rows survive. Equal update and absent removal preserve the
existing map/configuration `Arc` identity.

Absent input leaves no row. A build-setting rule obtains its effective value by
binary lookup of its own canonical label, verifies that an override kind agrees
with its declaration, or falls back directly to the loading-owned declaration
default. No declaration default, source tag, effective-value cache or evaluator
value enters configuration identity.

Delete `root_string_build_setting_default`, the required-label copied-default
path and any assertion that a build-setting configured node must carry its own
row. Preserve the root command's current admitted single-string observation by
routing its one typed candidate through the general resolver. The singular
command adapter remains a bounded category-3 input bridge, not semantic state.

### Transition and context cutover

The existing one-output transition subset evaluates as today, then uses the
same declaration lookup and resolver before constructing each child
configuration. Generalize its output value shape to all five kinds above;
preserve the current one-dictionary-entry/output-label restriction. A
default-equal transition removes a pre-existing row, and a nondefault
transition replaces only its declared label. Equivalent string-set list/set
outputs converge on one configuration identity.

`ctx.build_setting_value` becomes a generic ephemeral Starlark value allocated
from the already resolved typed effective value. Return Starlark integer,
Boolean, string, list, or set with the exact effective shape; in particular an
absent `allow_multiple` string returns `[default]`, ordered lists preserve
duplicates, and string sets expose set semantics. The heap value is evaluator
scratch and never crosses result/DICE boundaries.

## Compatibility classification

- **Exact:** declaration authentication; all five typed transition/context
  shapes; arbitrary-precision configured integer; allow-multiple singleton
  fallback; list order/multiplicity; set normalization; declaration-owned
  scope; default-row elision; unrelated-row survival; and the admitted root
  string lifecycle.
- **Slug-native:** Rust module/layout, compact allocation, structural
  configuration bytes/tokens, and diagnostic wording.
- **Unsupported/deferred:** command text conversion and occurrence precedence,
  transition inputs/multiple outputs/splits/aliases/native options, condition
  matching, selector resolution, `PROJECT.scl`, provider payloads,
  platform/toolchain choice, Bazel checksum/output bytes, and wider rule flow.

## Proof obligations

1. Declaration conversion covers integer beyond i64, Boolean, empty/nonempty
   string, allow-multiple string, duplicate-preserving string list and
   list/set-origin string set, with exact wrong-kind/element rejection.
2. Each declaration scope replaces placeholder input scope and participates in
   nondefault configuration identity; project remains retained in target
   configuration and fails only at the accepted exec boundary.
3. Absent input reads the loaded default with no row; explicit default-equal
   input removes/omits the row; nondefault input retains one row; unrelated
   rows survive; A/B/A restores map and configuration identity.
4. Default-equal transitions remove an inherited row, nondefault transitions
   replace one row, list order stays significant, and equivalent set/list set
   outputs converge.
5. `ctx.build_setting_value` exposes all five effective Starlark shapes,
   including singleton fallback for allow-multiple string, list duplicates and
   set behavior, without retaining evaluator memory.
6. Root string command, transition, analysis, cquery/build/run and retained-
   daemon lifecycle remain accepted after projection tokens change from the
   copied-default configuration to the empty override map.
7. A canonical-external build-setting label resolves the declaration, default
   and scope from its own selected canonical repository package without root-
   label repair, and the configured-package dependency is observable in the
   analysis preparation/observation proof.
8. Declaration Need, semantic failure and cancellation publish neither an
   override row nor a transitioned child key. A later successful request
   recovers through the same declaration dependency and publishes exactly one
   resolved child; stale partial state is not reused.
9. Old root-only default/required helpers and the named temporary bridge
   disappear; no second declaration/value/scope owner is introduced.

Use the pinned Bazel `StarlarkRuleContext`, `StarlarkTransition`,
`FunctionTransitionUtil`, `CoreOptionConverters`, `ConfigStringSetTest` and
existing loading declaration evidence. Add a new oracle only if one of the
named conversion shapes is not discriminated by pinned sources/tests.

## Ownership and memory

Loading packages remain the only retained declaration owner. Configuration
remains the only retained effective-override owner. The resolver uses bounded
scratch and returns an existing immutable configuration or one changed
`Arc<[StarlarkOption]>`; evaluator allocations die with the rule/transition
module. Add no retained map, declaration cache, interner, frozen heap, text
identity, global store or lock.

Zabel's per-canonical-label declaration producer, typed effective-value stage
and unchanged-parent reuse are useful ownership patterns. Slug implements them
through its existing configured-package DICE producer and immutable option map;
it does not copy Zabel representation, diagnostics or checksum policy.

## Allowlist and caps

Production:

1. `Cargo.lock` (generated `slug_analysis_v2` dependency edge only, if direct
   `num-bigint` use requires it; no version/checksum change);
2. `app/slug_analysis_v2/Cargo.toml`;
3. `app/slug_analysis_v2/src/lib.rs`;
4. `app/slug_analysis_v2/src/build_setting.rs`;
5. `app/slug_analysis_v2/src/dice.rs`;
6. `app/slug_analysis_v2/src/starlark_rule.rs`.

Proof:

7. `app/slug_analysis_v2/tests/configured_target.rs`;
8. `app/slug_analysis_v2/tests/root_analysis.rs`;
9. `app/slug_analysis_v2/tests/starlark_rule.rs`;
10. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`;
11. `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs`;
12. `app/slug_cli_v2/tests/cli.rs`;
13. `app/slug_server_v2/src/tests.rs`.

Completion docs remain the canonical plan, this manifest and Stage 6 owner
plan. Caps: 1,100 production Rust lines, 1,500 proof Rust lines, 2,600 total
Rust lines and 220 completion-ledger lines. The new module is the single pure
category converter; `dice.rs` owns lookup/orchestration and
`starlark_rule.rs` owns ephemeral context allocation.

## Validation

Run serially: pure resolver conversion/elision tests; focused root preparation,
transition and typed-context lifecycle tests; canonical-external declaration
dependency plus Need/error/cancellation/recovery tests; `cargo test -p
slug_configuration_v2`; `cargo test -p slug_analysis_v2`; direct core, CLI and
server projection/lifecycle proofs; locked checks for every direct consumer;
`cargo fmt --all -- --check`; `git diff --check`; exact allowlist/caps; named
archive baseline; and independent conversion/DICE/evaluator-lifetime review.

## Stops

STOP and `REPLAN` for a required file outside the allowlist; a second
declaration/value/scope store or converter; copied defaults after resolution;
evaluator or frozen-heap retention; text-only typed conversion; integer
narrowing; list/set conflation; missing declaration authentication; transition
child publication before its declaration dependency; project-scope exec
acceptance; command text/occurrence parsing; condition/selector/provider/
platform/toolchain work; Rust BCR rule flow, `cc_internal` or `cc_common`
parsing; Zabel authority; cap overflow; or a material contract correction.
