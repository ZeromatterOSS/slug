# Current Slug V2 Work Packet

Packet: WP-7-102-m7a-package-relative-label-select-key-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit the source-authentic generated `pythons_hub` macro call to
`native.package_relative_label(settings[i - 1])` and its immediate use as a
typed `select()` condition key. Under the existing normalized-PATH authentic
rules_python 1.7.0 smoke, Slug must pass both the current missing-native-method
stop and the following Label-key `select` intake; a later independently typed
stop is acceptable. This is one coupled Loading edge, not complete Python
toolchain or configurable-alias analysis.

Exact Bazel 9.2 slice: during ordinary BUILD-package/macro evaluation, a
string input is parsed against the calling BUILD package identifier and its
repository mapping, not the defining `.bzl` package; an existing Label passes
through unchanged after package-context validation. Supported strings are
bare/relative, same-repository absolute, known apparent-mapped and explicit
canonical labels. Malformed strings and wrong input types fail before package
publication; a call without BUILD package context fails closed. `select()`
accepts ordered string and Label keys, keeping strings raw under Bazel 9.2's
default non-eager semantics and retaining typed canonical keys. Both raw and
typed root `//conditions:default` are default branches at attribute coercion;
other key types fail. Slug-native: structural selector/package/DICE identity
and Rust error text rather than Skyframe or Java exception bytes.
Unsupported/deferred: Bazel's nonvisible Label value and delayed failure for
unknown apparent repositories (Slug fails closed at mapping lookup), the
rule-initializer thread-local converter, `ctx.package_relative_label`, the
eager-select-key flag, `select(no_match_error = ...)`, mixed string/Label
duplicate-key precedence, rule-implementation calls and broad selector/
toolchain analysis. Preserve all accepted raw-string selector, Label(), native
alias and package-source behavior.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkNativeModuleApi.java:274-308` and
`StarlarkNativeModule.java:650-666` require String-or-Label input, obtain a
package `LabelConverter` before passthrough, and resolve strings in the current
BUILD context. `LabelConverter.java:61-93` uses package plus repository
mapping. `bazel_module_test.py:573-623` distinguishes calling `@bar+//quux`
from defining root `.bzl`, known apparent alias, explicit canonical spelling
and Label passthrough. Bazel's initializer special case is separately owned
by `StarlarkRuleClassFunctions.java:1780-1790` and remains deferred.
`SelectorList.java:90-115,279-307` accepts Label or string dict keys;
`BuildLanguageOptions.java:853-863` defaults eager string resolution off;
`SelectTest.java:158-205`, `ConfigurableAttributesTest.java:2225-2273`
and `BuildType.java:700-738` establish raw strings, typed keys and canonical
default selection.

Authenticated rules_python 1.7.0
`python/private/py_toolchain_suite.bzl:58-89` supplies bare config-setting
names. Bazel_skylib `lib/selects.bzl:177-185` immediately uses the resulting
Label as a `select` dictionary key alongside a string default. Thus a
method-only patch cannot pass the authentic expression. Fresh Bazel 9.2
scratch `/tmp/slug-wp802-label-context` proves a generated BUILD macro
resolves `:marker.txt` in the generated hub while `Label(':marker.txt')`
resolves in its producer `.bzl`; it also proves typed Label condition plus
string default selects `:value` in dbg and `:fallback` in opt. A wrong-type
call fails. Mixed typed/string equivalent keys did **not** fail in the scratch
probe, so no duplicate rejection claim may be inferred from it. Convert the
positive, negative and branch controls into one compact Stage 1 fixture and
independently replay it.

`PackageRecorder` in `package.rs` is the calling-BUILD semantic owner. The
generated package driver already injects its canonical package identifier,
repository mapping and observed source route. Add the native method in the
existing `NATIVE_METHODS` table. Obtain `PackageRecorder` **before** inspecting
input, return an existing `StarlarkLabel` Value unchanged, and resolve strings
through `PackageRecorder::dependency_label` / the existing
`package_context_label_with_repository` parser; never use defining-source
`Label()` resolution or ambient filesystem state. `select_globals` in the same
file currently unpacks a string-keyed map. Accept a Starlark dictionary,
project string keys to the existing `SelectorCondition::Raw` and typed
`StarlarkLabel` keys to the existing `SelectorCondition::Canonical`, preserving
order and values. The existing attribute coercer resolves raw keys and parses
canonical keys; extend only its default recognition for canonical
`@@//conditions:default`. Reject non-string/non-Label keys without coercing
arbitrary objects to text. No new label representation, DICE key, mapping,
cache, interner, side registry, lock or command-side repair.

The selected value is transient/frozen evaluator data using existing
`SelectorValueGen` Trace/Freeze/Allocative and ordered Vec storage. At package
coercion, canonical selector conditions enter the existing immutable
`CoercedAttributeValue::Selector` and `LoadedPackage` structural equality;
the observed package/source/route DICE keys own invalidation and A/B/A
restoration for mapping or generated BUILD edits. The Buck2 utility-reuse
check found no new representation or hot-path container decision; Stage 9's
retained evaluator/SmallMap utility patterns remain unchanged and no donor
code is imported. `package.rs` is already large, but both edits belong to its
existing native/select ABI and package coercer; splitting that shared method
table in this packet would add indirection without a distinct owner.

## Evidence and validation

Make one small Stage 1 Bazel 9.2 fixture with `fixture.toml` provenance and a
local-path-overridden producer. A generated nonroot BUILD must call an
imported macro and expose the caller-package versus defining-package label
distinction, known apparent and explicit canonical spelling, Label passthrough,
a Label-valued `select` key with raw-string default, and wrong-type or
wrong-phase failure. Use exact query/cquery outputs or stable message shape;
fresh generation and independent fresh-root replay must agree. Keep at most
ten small files and below the orchestration fixture-growth checkpoint. Reuse
WP801's accepted canonical-owner-load fixture as a protected regression, not
as a mutable prerequisite. Upstream initializer tests are skipped because
their thread-local converter is a distinct deferred owner; mixed equivalent
selector-key precedence is not inferred from the scratch probe. No upstream
source, archive, auth or credential material enters the fixture.

Focused Rust tests must prove macro caller-package versus defining-source
context in a generated external BUILD, known apparent/explicit canonical and
Label passthrough, raw/typed selector key order and canonical default, wrong
type/syntax, no package context (including Label input), and unchanged
`Label()`/raw-string behavior. Show that a mapping A/B/A or generated BUILD
source revision changes/restores the canonical condition and observed package
result; no interrupted Need pass publishes a partial target. Include a
configured or equivalent structural selector proof that dbg/opt choose the
expected branch only where the existing native alias family is already
admitted. Compile Loading plus named direct Analysis, Query and Core
dependents with pinned-toolchain JSON `--no-run`; batch exact-selector
preflight and run focused tests serially. Rebuild `slug_cli_v2` before the
authentic normalized-PATH smoke; check/clean stale `slugd` before and after.
Run Bazel fixture generation/replay, protected fixture replay, formatting,
diff, archive and plan-status checks; require independent DESIGN and FINAL
reviews. No broad suite, remote-transport gate, M7A close or BuildBuddy
credential access is selected.

## Scope and stops

Base clean main `923c80db7`. Allowlist: this manifest, canonical Current
paragraph, Stage 4 owner contract only for accepted invariants;
`app/slug_loading_v2/src/package.rs` for native/select/coercion;
`app/slug_loading_v2/src/host_package_load_tests.rs` for focused tests; and
`tests/v2_oracle/fixtures/native-package-relative-label-select/` for the
new fixture. Production growth estimate is under 80 lines and proof growth
under 300 lines; these are review triggers, not permission ceilings. Preserve
the accepted `StarlarkLabel` and `SelectorCondition` representations. REPLAN
if a typed Label key cannot use the existing canonical selector variant, the
calling package/mapping is absent from the existing recorder in the authentic
macro, structural equality loses condition identity, or the authentic next
stop requires a separate repository/analysis owner to pass this precise
expression. A later selector/alias/toolchain failure is a new packet.

## Immediate predecessor

WP801 at `923c80db7` accepted nonroot canonical `.bzl` loads from generated
BUILDs; its Bazel 9.2 fixture generation/replay, eleven focused Rust tests
and independent reviews passed. The authentic smoke moved from the
`@@rules_python+` LoadLabel stop to this `native.package_relative_label`
call. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT. The eight-file Bazel
9.2 fixture passed five generated-BUILD, mapping, dbg/opt selector and
wrong-type rows in generation (6.39 s) and independent fresh-root replay
(5.28 s); WP801's canonical-owner-load fixture replay also passed. The
package-owned method resolves strings in the calling BUILD/macro package and
returns an existing Label unchanged; `select()` retains ordered raw string
and typed canonical keys, with canonical root default recognition. No DICE
key, mapping owner or retained selector representation changed. The final
Loading JSON library test compile passed (12.27 s); exact preflight selected
seven nonignored tests, and three new plus four protected Rust tests passed.
Analysis, Query and Core JSON library test targets compiled, and the CLI
rebuild passed (10.79 s). A broader direct-dependent integration-test compile
is not a packet gate: unchanged orphan Analysis test files such as
`tests/directory_args.rs` fail at top-level `use super::*`; the named library
dependents compile. Formatting, diff, archive and plan-status checks passed.

The rebuilt authentic normalized-PATH smoke passed the former
`native.package_relative_label` and typed-`select` expression, then stopped at
toolchain registration row 55 in `@@rules_kotlin+//kotlin/internal` on a
separate selected-BCR `REPO.bazel` materialization entry mode. No stale
`slugd` remained. The Slug test's nonroot BUILD uses an existing materialized
canonical route rather than extension generation; actual generated BUILD
behavior is covered by the Bazel fixture and authentic smoke. A/B/A restores
structural `LoadedPackage` equality, while observation carriers remain
unequal across Host metadata epochs. Rule-initializer context,
unknown-apparent nonvisible Label values and broader selector/analysis
behavior remain deferred. No remote transport, BuildBuddy authentication or
NativeLink gate was used. Packet wall-clock and independent review durations
were not separately timed; the measured compile/oracle durations above are
the available timing evidence. M7A and M8 remain open.
