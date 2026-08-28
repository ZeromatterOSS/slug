# Current Slug V2 Packet

Packet: `WP-4-5-7A-batched-selector-resolution`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `21ad43d24`.

Result: resolve every retained configurable attribute through one generic
typed resolver, batch its direct conditions through the accepted configured-
condition DICE key, expose resolved values through the ordinary analysis
attribute ABI, and apply the same condition path to native toolchain
`target_settings`. This closes typed build-setting category 2. Command
occurrences, configured target-platform constraint matching, feature flags and
providers remain separate named categories.

## Accepted predecessor and architectural boundary

Commits `b949ce8da`, `57b1e8a1f`, `84bda1971`, `aaf23abcc`, and `21ad43d24`
accept the category architecture, loading-owned four-field predicates, sole
typed scoped-option map, declaration-authenticated effective values, all
direct native/define/typed-flag matching and the sole configured-condition
DICE owner. A condition value retains only match/no-match; loading packages
remain the only owners of predicate declarations.

Buck2-derived Rust remains the sole generic Starlark syntax/evaluator owner.
BCR Starlark owns every rule and control path including `cc_internal`;
`cc_common` is a demanding client of the generic evaluator/provider/host ABI,
never a Rust C++ parser or rule engine. Pinned Bazel 9.2 at
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` supplies peer ownership and compact
resolution guidance only.

## Live preflight

- Loading retains every admitted attribute as `CoercedAttributeValue`, with
  selectors and concatenations structural. Selector keys are separately
  deduplicated into `$config_dependencies`; branch labels remain ordinary
  potential query dependencies and are not selector-key dependencies.
- The loading type owner already implements every admitted concatenation for
  unconfigured `attr()` candidate expansion, but analysis has no shared
  resolved-value concatenation primitive. Add that primitive to the existing
  type owner; do not copy its type matrix into analysis.
- Configured analysis currently recognizes only direct scalar-label and
  label-list dependency values. `ctx.attr` exposes prepared dependencies and
  one marker special case, not the complete admitted ordinary attribute set.
- `ConfiguredConditionKey` already owns Root/Canonical condition lookup,
  native and typed flag matching, Need/error/cancellation behavior and compact
  truth. It deliberately retains no declaration/provider payload.
- Native toolchain `target_settings` is retained as a configurable label-list
  expression, but configured selection rejects every nonempty value.
- Canonical configured-target analysis remains bounded by
  `require_supported_canonical_configured_target`: native toolchain shapes and
  dependency-free marker leaves only. This packet does not widen that gate.
- Configured target-platform facts still do not exist. Therefore every
  condition containing `constraint_values` continues to fail closed through
  the accepted condition owner.

## Implementation contract

### One typed configured-value resolver

Add one analysis-owned pure recursive resolver over borrowed
`CoercedAttributeValue`. It consumes a request-local map from canonical
condition labels to accepted condition truth plus borrowed loading declarations
for specialization. Atomic values remain unchanged. For each selector:

- examine every nondefault condition and surface all condition errors before
  selecting a value;
- retain all matching conditions not strictly refined by another match;
- define refinement exactly as Bazel's proper-superset relation over the
  currently matchable combined native `values` plus synthesized `define`
  entries and `flag_values`;
- accept multiple maximal matches only when their recursively resolved branch
  values are equal; deterministic source order may choose the representative
  only after equality is proved;
- otherwise report ambiguity without allowing map order to choose a winner;
- use the default branch only when no explicit condition matches;
- fail closed when neither an explicit nor default branch matches;
- resolve selected `None` through the declaration's existing default/mandatory
  contract rather than inventing a value.

Resolve concatenations only through one shared operation on the existing
loading-owned type. Cover all currently admitted scalar, ordered list and
dictionary shapes; preserve label identity, list order/duplicates and the
existing dictionary collision behavior. The result is request scratch. Retain
no configured copy, selector decision cache, evaluator value, flattened branch
table, provider or diagnostic diff.

The current `select()` builtin retains no custom `no_match_error`; keep that
argument unsupported rather than silently claiming the Bazel diagnostic
surface. Constraint-setting refinement remains with category 4 because no
constraint-bearing condition can yet produce a successful match.

### Batched condition preparation

Inside the sole configured-node analysis computation, obtain the analyzed
target's attribute selector keys from its loading-owned `$config_dependencies`
value. Deduplicate by full canonical label and compute each
`ConfiguredConditionKey` at most once for the target configuration. Load the
corresponding config-setting declaration through the existing Root/Canonical
package path only for scratch refinement; do not alter or duplicate the
condition matcher. Candidate-toolchain selector keys and selected setting
labels enter the same request-local batch through the separate two-phase path
below; they are not assumed to appear in the analyzed target's metadata.

Aggregate all condition Need/frontier/semantic outcomes without source-order
shortcuts. Outer frontier failure wins Need, Need wins semantic failure, and no
resolved attribute/dependency/provider/action publishes on failure or
cancellation. A corrected same-graph request recovers normally. Selector keys
must never become configured dependency edges of the selected branch.

After resolution, derive declared dependency keys only from the selected
typed values. Unselected branch labels produce no configured analysis, edge,
provider lookup or action. Apply transitions to selected dependency values
through the existing declaration-authenticated transition path.

### Generic analysis attribute ABI

Replace the marker-only scalar shortcut with one ephemeral resolved-attribute
view supplied to `evaluate_loaded_rule`. `ctx.attr` must allocate every
currently admitted resolved shape using the Buck2 Starlark value heap:
`None`, integer, Boolean, string, label, ordered label/string lists and the
admitted string/label/list dictionaries. Dependency-bearing labels allocate
the already prepared analysis dependency objects so provider indexing
continues to work.

Output and output-list attributes are nonconfigurable in the admitted loading
surface. Expose them through generic `ctx.outputs`, not `ctx.attr`, as
predeclared file values backed by their existing loaded canonical output
identity. They may be consumed by the existing action API but never become
input dependencies. Do not add another output declaration store.

Keep the resolved attribute view scratch-only for one synchronous rule
evaluation. The configured result continues to retain providers, actions,
declared outputs and configured edges, not a duplicate attribute tree or
evaluator heap.

### Native toolchain target settings

Resolve each candidate toolchain's configurable `target_settings` label-list
with the same resolver and target configuration through two explicit phases:

1. walk that candidate's retained expression to collect and deduplicate its
   selector-key labels, compute those conditions with the same outer > Need >
   semantic aggregation, and resolve the expression;
2. treat the resulting label-list members as config-setting requirements,
   compute and deduplicate those condition labels against the request-local
   batch, and retain the candidate only when every selected setting matches.

Selector-key labels and selected setting labels are distinct roles and may be
different targets or packages. An empty selected list remains eligible. A
selected no-match is candidate ineligibility; selector or selected-setting
Need/frontier/semantic failure remains a request outcome and is never silently
treated as ineligibility. No unselected label-list member is computed.

Do not broaden execution-platform or constraint compatibility, registration
order, toolchain types, implementation providers or multi-toolchain selection.

## Compatibility classification

- **Exact:** selector default/no-default behavior; direct config-setting
  proper-superset specialization; equal-value maximal matches; ambiguity;
  typed concatenation; selected-only dependency/configured-edge behavior;
  canonical external conditions and selected branches within the existing
  canonical native-toolchain/dependency-free-marker admission; complete
  admitted `ctx.attr` allocation; and native toolchain `target_settings` truth
  filtering.
- **Slug-native:** Rust scratch layout, structural configuration identity,
  configured-condition/preparation result representation and unproved
  diagnostic wording.
- **Unsupported/deferred:** custom `select(no_match_error=...)`, constraint-
  setting specialization until category 4, condition
  aliases/groups, feature flags and label-valued build settings,
  `constraint_values` truth until the configured target-platform owner,
  command text/occurrences and precedence, wider transitions, general provider
  payloads, broader platform/toolchain choice, Bazel checksum/output bytes and
  Rust implementation of BCR rule flow.

## Proof obligations

1. Pure resolver covers each atomic admitted value, nested selectors, chained
   concatenations and every allowed list/dictionary concatenation without a
   second type matrix.
2. One/no explicit match, default fallback and no-default error are distinct;
   false conditions do not evaluate or publish their branch dependencies.
3. Proper-superset specialization covers native, synthesized define and typed
   flag predicates. Incomparable equal values converge; incomparable unequal
   values error independent of source order.
4. Root and canonical-external selector keys batch once by canonical identity.
   Within the existing canonical configured-target admission, condition,
   declaration and selected branch packages invalidate independently and A/B/A
   restores truth, selected edges and DICE identity; no general external
   Starlark graph is admitted.
5. Every admitted resolved `ctx.attr` shape reaches a generic Starlark rule;
   dependency-bearing labels preserve providers, order and configured edge
   kinds. Every output shape reaches `ctx.outputs` as a predeclared file and
   never becomes an input edge.
6. Selected dependency transitions still authenticate their output setting and
   preserve unrelated option rows; unselected transitioned branches do not
   load declarations or publish children.
7. Configurable native toolchain `target_settings` resolves before eligibility.
   A selector-key condition distinct from the selected config-setting label
   proves both phases, including Need/error precedence and zero activation for
   the unselected label-list member. Empty/all-match remain eligible and one
   false selected setting rejects the candidate.
8. Need, multiple semantic errors and deterministic cancellation before
   attribute resolution publish no parent, branch child, provider or action;
   cold same-graph recovery publishes exactly one parent and selected closure.
9. Retained-size/source scans prove no second condition key/matcher, predicate
   store, configured attribute tree, selector cache, provider payload or
   evaluator value.

Use pinned Bazel `ConfiguredAttributeMapper`, `ConfigMatchingProvider`,
`ConfiguredAttributeMapperTest`, `ConfiguredAttributeMapperCommonTest` and
`ConfigurableAttributesTest` evidence. Add an oracle only for a named retained
shape not discriminated by those sources/tests.

## Ownership and memory

Loading remains the sole owner of schemas, coerced values, selectors and
config-setting declarations. Configuration remains the sole native and typed
override owner. `ConfiguredConditionKey` remains the sole matcher. Analysis
retains only ordinary configured results; condition joins, refinement sets,
resolved values and evaluator allocations are bounded request scratch. Use
borrowed slices, existing `Arc` data, `SmallMap`/`SmallSet`, `Dupe` and
`Allocative`; add no global table, interner, heap, lock or workspace cache.

Zabel's separation of typed condition inputs from configured attribute
resolution is useful peer guidance. Slug maps that idea onto its existing
loading/configuration/DICE/evaluator owners and copies no Zig representation,
algorithm, diagnostics or identity policy.

## Allowlist and caps

Production:

1. `app/slug_loading_v2/src/attrs.rs`;
2. `app/slug_loading_v2/src/package.rs`;
3. `app/slug_analysis_v2/src/configured_attribute.rs`;
4. `app/slug_analysis_v2/src/dice.rs`;
5. `app/slug_analysis_v2/src/starlark_rule.rs`;
6. `app/slug_analysis_v2/src/lib.rs`.

Proof:

7. `app/slug_analysis_v2/tests/starlark_rule.rs`;
8. `app/slug_analysis_v2/tests/configured_target.rs`;
9. loading-local tests inside `app/slug_loading_v2/src/attrs.rs`.

The canonical-external lifecycle proof exposed one bounded manifest
omission: Host root-package rule attributes still selected the pre-Bzlmod
label coercion path for canonical-external labels. This replan admits only
`package.rs` so the Host carrier reuses the existing repository-aware
canonicalizer. The legacy listing loader and unmapped apparent labels remain
fail closed, and analysis's existing canonical configured-target gate
continues to reject every external shape beyond the admitted native toolchain
and dependency-free marker leaf.

Completion docs remain the canonical plan, this manifest and Stage 6 owner
plan. Caps: 1,450 production Rust lines, 1,650 proof Rust lines, 3,100 total
Rust lines and 240 completion-ledger lines. The new module is the sole
configured-value recursion/refinement owner and stays at or below 650 lines;
no function exceeds 120 lines without an explicit split review.

## Validation

Run serially: pure resolver/refinement/concatenation tests; focused root and
canonical selector lifecycles; complete generic `ctx.attr` shapes; selected-
only transition/dependency/action proofs; toolchain `target_settings`; Need/
error/cancellation/recovery; `cargo test -p slug_loading_v2`; `cargo test -p
slug_analysis_v2`; locked checks for both crates and every direct consumer;
`cargo fmt --all -- --check`; `git diff --check`; exact allowlist/caps; named
archive baseline; and independent selector/DICE/evaluator-lifetime review.

## Stops

STOP and `REPLAN` for a required production file outside the allowlist; a
second selector or condition key/matcher/store; configured attribute retention;
copied declaration/default/type matrices; evaluator values escaping synchronous
evaluation; selector keys becoming branch dependency edges; source-order truth;
unselected branch computation; constraint matching without a configured
target-platform fact; command/provider/platform breadth; Rust BCR rule flow,
general external configured Starlark graph widening, `cc_internal` or
`cc_common` parsing; Zabel authority; a lock across DICE; cap overflow; or a
material contract correction.
