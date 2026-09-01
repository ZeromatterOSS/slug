# Current Slug V2 Packet

Packet: `WP-6-7A-rule-predeclared-outputs-complete-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 4 package output facts
and Stage 6 rule-context output projection.

Status: R1 architecture pre-review returned `REVISE`; focused correction
rereview accepts R2 for implementation. Base commit `6cb5ab55b` terminally
accepts generic
rule-level regular-transition attachment. Authentic rebuilt rules_rust 0.73.0
replay clears that category and stops at the generic named-only
`rule(outputs = {"rust_doc_zip": "%{name}.zip"})` declaration in
`rust/private/rustdoc.bzl:319-436`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Implement the complete default-enabled Bazel 9.2 Starlark rule predeclared-
output declaration category, not the one rules_rust dictionary. A generic
`rule()` accepts omitted/`None`, an ordered string-to-string dictionary, or a
Starlark callback in `outputs`; retains the adjacent `output_to_genfiles`
declaration bit; resolves every target's implicit output keys and labels during
package loading; publishes generated-file targets; and exposes the same files
through `ctx.outputs` and synthesized `DefaultInfo.files`.

Exact admitted behavior:

- `outputs` and `output_to_genfiles` are named-only. `outputs` defaults to
  `None`; `output_to_genfiles` defaults to `False`. Omitted and explicit
  `outputs = None` are equivalent;
- a dictionary retains insertion order and requires string keys and string
  templates. A callback must be a Starlark function, is frozen/importable with
  the rule definition, and is invoked once for each target during package
  loading;
- callback positional arguments follow the function's declared parameter-name
  order and are populated from target attributes. Defaults do not excuse an
  absent matching attribute. A named attribute containing `select()` fails
  before the callback body, while unrelated configurable attributes are simply
  omitted and do not reject the call. The return value must be an ordered
  string-to-string dictionary;
- each dictionary result produces exactly one key/name pair. Templates retain
  literal percent signs and incomplete `%{` text, expand repeated complete
  `%{field}` placeholders independently, and require every placeholder to
  yield exactly one distinct projected value;
- `%{name}` uses the complete target name. `%{dirname}` is empty or the target
  name's directory plus `/`; `%{basename}` is its final component. String
  values are verbatim, label values omit the final filename extension, and
  output values retain it. The corresponding list kinds use the same
  conversion and deduplicate equal projected strings: a nonempty list with one
  distinct projection succeeds, while two distinct projections fail. Boolean,
  integer, dictionary, absent, unknown, configurable, empty-list, and
  distinct-multiple substitutions fail with Bazel-shaped diagnostics;
- output keys need not be valid identifiers and remain available through
  `getattr(ctx.outputs, key)`. Key and resolved label order are semantic. Each
  resolved name must form a valid target in the generating rule's package;
- generated files are package targets owned by the generating rule and inherit
  its visibility. Duplicate generated names and direct collisions with another
  ordinary package target fail through the existing target recorder;
- a nonempty explicit `attr.output`/`attr.output_list` value whose declaration
  name equals an implicit output key fails during loading. Empty/absent explicit
  output attributes reach analysis and then fail when `ctx.outputs` would add
  the duplicate key;
- `ctx.outputs` exposes explicit output attributes in declaration order and
  then implicit output keys in dictionary/callback order. Implicit outputs are
  real derived Files for the current configured owner and may be registered as
  action outputs; and
- an omitted `DefaultInfo.files` contains implicit predeclared outputs before
  explicit output-attribute files, followed by the existing executable
  handling. Generated target query/owner behavior uses the same loading facts.

Slug-native behavior:

- one `Arc<[PredeclaredOutput]>` of compact key plus canonical same-package
  label is the final package and analysis identity. It does not claim Java
  object identity, Java serialization, Bazel configuration checksum, or exact
  `bazel-out` path bytes;
- `output_to_genfiles` participates structurally in rule/package equality. At
  Bazel 9.2's default `--incompatible_merge_genfiles_directory=true`, both
  values use the admitted merged output root and therefore yield the same
  relative action path; and
- the callback is transient definition/package-loading behavior. Final target
  semantics retain only resolved key/label pairs plus the placement bit, while
  the loaded frozen module continues to own callback lifetime.

Unsupported/deferred behavior:

- `--incompatible_no_rule_outputs_param=true` remains outside the admitted
  command-wide build-language-option surface;
- `output_to_genfiles=True` with
  `--incompatible_merge_genfiles_directory=false` fails closed before rule
  implementation evaluation. Exact separate bin/genfiles output-root bytes
  remain M9 work;
- parent/rule-extension output inheritance, native implicit-output functions,
  computed-default callbacks, executable-output lazy creation, aspects,
  output groups beyond already admitted `OutputGroupInfo`, and action families
  not already supported remain separate categories; and
- Bazel's special output-equals-generating-rule warning, output-prefix
  conflicts, and input/output conflicts remain unsupported/deferred. This
  packet performs no package-wide output-name scan and makes no parity claim
  for those surrounding collision categories; and
- no parser grammar, `set`, rules_rust rule body, ruleset special case,
  `cc_common`, `cc_internal`, C++ rule, BCR, command, or repository branch is
  added. Bazel 9 BCR Starlark continues to own all rule bodies.

## Bazel 9.2 authority and accepted evidence

Bazel tag `9.2.0` commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic authority.
The commit object remains available even though the clean `../bazel` checkout
has advanced to master. Pinned source SHA-256 values are:

- `StarlarkRuleFunctionsApi.java`: `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`;
- `StarlarkRuleClassFunctions.java`: `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`;
- `StarlarkCallbackHelper.java`: `2d73165555218adb39af0dff93fbbe8b02cb7fabd81dac2498266c76e3174e3f`;
- `ImplicitOutputsFunction.java`: `31b17c66166808cd7d9f42fb68a8b6eb0e4d1a2c8aa9c130bd8892c75b59fdb0`;
- `Rule.java`: `64b41387a6f309b61f9090d3566299c5cbefc0c137beae972d8230d2fafb6b87`;
- `StarlarkRuleContext.java`: `5200266852f65ca66a958a3adaf82a29f9b5cbbd1a604a4e91d7815476985072`;
- `outputs_test.sh`: `5a19aa62ade7ef56a5a6556ff5e08879848de89f7460451f41d2c8037d2e3dfe`;
- `ImplicitOutputsFunctionTest.java`: `ec68d9877b95a0eac8429e4d7a21193be7ccd0fdce693d3bcc0598aff61d6517`;
- `CoreOptions.java`: `89835ed74107b21f7c51b4723e16be8b96b3c1bf43855fc63220b1dd21f5c67a`; and
- `BuildLanguageOptions.java`: `b01e106ef0ff7af458766248bce7799b49c0f54fc14d023a8297aeb7dbfb44e5`.

The API supplies the exact union and defaults. `StarlarkRuleClassFunctions`
distinguishes callbacks from ordered maps. `StarlarkCallbackHelper` owns
parameter-name order, positional construction, the transient evaluator and
print handling. `ImplicitOutputsFunction` owns attribute projection, callback
result conversion, placeholder parsing, type/cardinality conversion and static
configurable rejection. `Rule` resolves
implicit outputs before explicit ones, publishes generated files, validates
labels and loading collisions. `StarlarkRuleContext` constructs `ctx.outputs`
in explicit-then-implicit order and detects the remaining duplicate-key case.
The default merge-genfiles option is exact source evidence for the placement
classification.

`outputs_test.sh` proves static and callback success, call-binding rejection,
configurable static failure and generated-target reachability. The pure
placeholder tests prove parsing, percent escaping, cross product and duplicate
value behavior. Rule/RuleContext source is stronger evidence for retained
ordering, key ownership and collision phase. No new permanent Bazel fixture is
justified; Slug focused regressions adapt these pinned cases, and the authentic
rules_rust replay supplies the imported BCR consumer.

## Learned Slug facts and architecture decision

Slug already creates `PackageTargetKind::GeneratedFile` for explicit output
attributes, retains final package equality under the package load fingerprint,
materializes explicit output Files through `ctx.outputs`, and synthesizes
`DefaultInfo.files` from predeclared output attributes. What is missing is a
rule-definition output producer and a separate final key/label namespace;
pretending implicit outputs are attributes would leak them through `ctx.attr`,
query schema and dependency logic.

Add a V2-owned `rule_outputs` module with:

- `RuleOutputsDefinitionGen<Value/FrozenValue>` for either ordered compact
  template pairs or one frozen callback;
- a pure template parser/substitution function over retained attribute schema
  and values; and
- public `PredeclaredOutput { key: CompactString, label: CanonicalLabel }` for
  final package/analysis handoff.

`rule()` validates the union and retains the definition. Freezing retains only
the callback value or shares immutable template pairs. Target invocation first
coerces all attributes using the existing owner, then executes a callback in a
fresh synchronous evaluator with the definition's Bzl context and package
print handler when applicable, resolves templates, checks loading collisions,
and records the rule plus generated targets atomically through the existing
recorder. `StarlarkRuleImplementation` retains the resolved output slice and
placement bit in structural equality. Analysis borrows that slice for
`ctx.outputs`, default-file synthesis and action-output ownership; it does not
rerun templates or callbacks.

Do not create synthetic `AttributeSchema` rows, a second generated-target
registry, path inference, callback source text, repository-mapping copy,
ordinal side table, global interner, cache, DICE key, output-name scan or
analysis fallback.

## Lifetime, memory, incremental ownership, and peer guidance

Static declaration maps use ordered `Arc<[(CompactString, CompactString)]>`;
resolved outputs use `Arc<[PredeclaredOutput]>`. This preserves observable
order, gives constant-time slice clones, derives `Allocative`, and avoids a
retained hash map for the normally tiny collection. Construction and template
substitution use request-local `Vec`/`SmallSet` scratch only. No new global
interner is justified; canonical labels retain their existing owner.

The frozen module owns callback code. Its fresh evaluator, argument values,
callback result dictionary and substitution buffers are synchronous package-
loading scratch and are dropped before publication. Package publication owns
resolved outputs and generated targets; structural equality supplies DICE
cutoff and A/B/A restoration. No evaluator borrow, command scratch, async task,
cache, eviction, cancellation, shutdown or overlapping-request behavior
changes.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
architecture guidance only. `build_invocation_capture.zig`
(`4e3bff2cc636a52c26e64346ff4271490d1a7a0cf59917bc46d8578bc7f404d1`)
supports separate resolved key/name package facts;
`configured_rule_analysis.zig`
(`02838bcd7f7aba0743338b61179e8bfeca79378eaf17b6b78d85366280f44126`)
supports implicit-before-explicit `DefaultInfo` materialization and a separate
`ctx.outputs` namespace. Slug does not copy its allocator, dense indexes,
capture stages, or name-only `%{name}` template limitation, and Zabel is not a
behavior oracle.

Buck2 supplies no Bazel `rule(outputs=...)` semantic donor. The matching Stage
9 utility decision is reuse of Slug's already adopted Buck2-derived
`CompactString`, immutable `Arc` slices and `Allocative`; no Buck2/V1 code,
interner, map, hasher or new utility enters this packet.

## Implementation boundary, caps, and proofs

Production allowlist:

- `app/slug_loading_v2/src/rule_outputs.rs` (new pure owner);
- `app/slug_loading_v2/src/lib.rs`;
- `app/slug_loading_v2/src/package.rs` only for binding/freeze/invocation and
  final target handoff;
- `app/slug_configuration_v2/src/native/configuration.rs` only for a typed
  merge-genfiles Boolean projection; and
- `app/slug_analysis_v2/src/starlark_rule.rs` only for `ctx.outputs`, default
  files and the nonmerged-genfiles stop.

Proof allowlist:

- colocated `rule_outputs.rs` unit tests;
- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_loading_v2/tests/bzl_invalidation.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_configuration_v2/src/native/tests.rs`; and
- `app/slug_analysis_v2/tests/starlark_rule.rs`; and
- `app/slug_query_v2/tests/loading_query.rs` only for implicit-generated-target
  lookup, kind and generating-rule ownership over the existing query graph.

Plan/status allowlist is this manifest, the canonical plan, Stage 6 and the
Stage 9 ledger. Caps are 360 net / 520 gross production Rust lines, 420 net /
650 gross proof Rust lines, and 1,170 total gross Rust lines. No new function
may exceed 140 lines.

`package.rs` exceeds the complexity trigger, so template semantics and retained
types belong in the new cohesive module; `package.rs` receives only seam code.
`starlark_rule.rs` is below the 2,000-line trigger and already owns all
`ctx.outputs`/DefaultInfo projections. No demonstrated hot-path performance
claim is made; exact ordering and structural equality precede any future
measurement.

Focused proofs must cover:

1. omitted/`None`/empty, static map, callback and both named-only bindings;
   wrong union, dictionary member, native-callable and callback-return types;
2. literal percent/incomplete placeholders, name/dirname/basename, string,
   label/output extension treatment, singleton lists, duplicate-equal versus
   distinct-multiple list projections, repeated placeholders,
   unknown/absent/unsupported/configurable/empty values, and exact error phase;
3. direct and transitive imported callbacks, callback parameter ordering,
   every currently admitted attribute value carrier, defaulted parameters,
   named configurable rejection, unrelated configurable success, print capture,
   once-per-target execution and ordered returned dictionaries;
4. final key/canonical-label order and equality, template/key/placement
   discrimination, equivalent-result convergence, and same-DICE A/B/A;
5. generated target ownership/visibility/order, invalid or duplicate names,
   ordinary target collisions, and nonempty versus empty explicit-output-key
   collision phases;
6. `ctx.outputs` explicit-then-implicit access including `getattr` for a
   nonidentifier key, action registration with an implicit File, and synthesized
   `DefaultInfo.files` implicit-before-explicit order;
7. loading-query implicit generated-target lookup, generated-file kind and
   generating-rule ownership over the same package facts;
8. false/true `output_to_genfiles` structural identity under merged genfiles and
   pre-implementation failure for the nonmerged configuration; and
9. rebuilt authentic rules_rust replay clears the complete generic output
   declaration category and stops at the next independent generic frontier,
   never at parser, `set`, `cc_common`, `cc_internal`, or a ruleset branch.

Validation is serial: focused and complete loading/configuration/analysis/query
tests, `cargo fmt --check`, Cargo metadata, `git diff --check`, archive status,
pinned source-object hashes, clean Buck2/Zabel, parked-file hash,
`cargo build -p slug_cli_v2`, stale-`slugd` cleanup, and authentic replay.
Independent architecture pre-review and terminal implementation review are
required for new retained cross-crate identity and callback lifetime.

There is no fallback. `REPLAN` before implementation if callbacks require a
retained evaluator borrow, if generated targets cannot be recorded atomically,
if exact callback parameter projection requires a starlark-rust fork, if
implicit outputs must masquerade as attributes, or if a new DICE key/global
registry is required. `REPLAN` during implementation if package equality cannot
own every key/label/placement input, nonmerged genfiles cannot fail before rule
execution, the production cap is exceeded, or replay contradicts pinned Bazel
9.2 evidence.

Residual risk is explicit: the deprecated API is needed by current BCR rule
sets, but disabling it and exact split bin/genfiles path bytes remain deferred
command/configuration categories. Bazel's prefix/input/output and own-rule
collision cases are also deferred package-validation categories.

## Immediate predecessor

Commit `6cb5ab55b` terminally accepts
`WP-6-7A-rule-level-transition-attachment-r1`: generic named-only regular
transition attachment survives live/frozen/imported/final rule ownership,
shares attribute transition identity and allowlist generation, and fails
before configured work. Complete loading passed 560 tests with one ignored,
analysis passed 122, independent terminal correction rereview returned
`ACCEPT`, and authentic replay advanced to the generic output category selected
here.
