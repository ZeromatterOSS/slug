# Current Slug V2 Work Packet

Packet: WP-7-130-m7a-java-plugins-flag-alias-loading-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Clear the authentic `@@bazel_tools//tools/jdk:BUILD:48` Loading stop by
admitting Bazel 9.2's native `java_plugins_flag_alias` rule declaration, not
its configured Java behavior. Exact selected Loading: BUILD and `.bzl`
`native` callability, distinct recorded rule-class identity, the inherited
attribute-name/order contract, private implicit `:java_plugins` label-list
with empty Loading default, and no ordinary dependency edge under default
flags. Existing inherited-attribute diagnostics and query rendering remain
Slug-native where already classified. Slug-native fail-closed direct Build and
configured-analysis errors must replace any false `LoadedOnly` success.
Unsupported/deferred: `--plugin` option projection, late-bound configured
dependencies and exec transition, JavaPluginInfo provider validation/merging,
configured target or cquery parity, Java actions, and full JDK package Loading.
A later authentic stop cannot widen this packet.

## Source, owner and decision

Pinned Bazel 9.2 source commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`tools/jdk/BUILD.tools:47-50` calls the native rule with only `name`;
`JavaPluginsFlagAliasRule.java:47-69` adds `:java_plugins` to `NativeBuildRule`,
using an execution transition, either JavaPluginInfo identity and
`JavaSemantics.JAVA_PLUGINS`. `BaseRuleClasses.java:226-339` supplies its 18
inherited slots after the `name` slot inserted by `RuleClass.java:753-817`.
`JavaSemantics.java:49-53` reads target-configured
plugins; `Attribute.java:1788-1805` gives a label-list late-bound default an
empty Loading value. `JavaOptions.java:351-362` owns repeatable `--plugin`.
The rule's allowlisted-label check and provider merge occur only in its
configured factory; Bazel's `JavaPluginsFlagAliasTest` covers default, one/two
plugins, wrong provider and wrong location. No Java helper enters Slug.

An ignored-RC/no-fetch Bazel 9.2 oracle in the authentic workspace resolves
the JDK target as class `java_plugins_flag_alias`. XML default-value query
reports 20 ordered attributes: the 19 inherited native slots followed by
empty private `:java_plugins`. Default `deps(target, 1)` returns only the
target. Default cquery succeeds, while an explicit `--plugin` pointing at
`:toolchain_type` fails the mandatory JavaPluginInfo check. The scratch output
base required a copy of already-materialized external repositories after
`--nofetch`; no new network fetch or complete configured-valid-plugin proof
is claimed. Bazel's inherited `licenses` default is a LICENSE value containing
`NONE`; Slug's existing native schema stores an empty string list, an inherited
Slug-native query-rendering boundary rather than exact license parity.
Existing source/catalog certificates own these immutable BUILD
bytes; the source file and package-load DICE dependencies own invalidation.

Append one `NativeRuleClass` schema in `attrs.rs`, preserving existing enum
discriminants. Add a unit `PackageTargetKind` and fixed `RuleCapability` in
`package.rs`, with recording via existing `PackageRecorder`, generator and
override handling, BUILD global and `.bzl` `native` method. The rule class
itself encodes the late-bound nature; retain no fabricated plugin labels or
option value. Keep `:java_plugins` implicit and non-callable with its empty
Loading value. Reuse the existing `Arc`-backed native-attribute slice,
`CompactString`, `Allocative`, target equality, source manifest and DICE
ownership; no new map/interner/cache/lock/key or evaluator borrow. Query's
root and external projections report the distinct class and default empty
edges. Core must route a directly requested instance to the existing
configured-analysis failure instead of treating it as `LoadedOnly` success;
do not synthesize empty Java providers.

## Evidence and validation

The accepted WP829 authentic replay is the prechange discriminator: the first
new failure is the missing `java_plugins_flag_alias` BUILD global at line 48.
Add a source-shaped Loading regression for a bare BUILD call and a `.bzl`
`native` call. Verify rule class, public/package/declared visibility,
source-relative generator metadata, exact schema order and implicit empty
plugin slot, duplicate/unknown/private-attribute negatives, and source A/B/A
restoration. Add root/external Query class and empty-edge controls, preserving
the inherited Slug-native rendering boundary. Add a direct requested-Build
negative proving configured failure before any action, publication or false
success; a source/configuration change must not turn that into success.
Reuse the pinned source and no-fetch oracle; no new Stage 1 fixture.

Resolve the pinned nightly via `rustup which`, compile selected test binaries
with Cargo JSON `--no-run`, and use `scripts/v2_test_preflight.py` for exact
nonignored selectors before execution. Run focused Loading, Query and Core
tests, direct checks for Loading/Query/Analysis/Core/CLI, and rebuild
`slug_cli_v2` before one normalized-PATH authentic `//:probe` replay. The
replay is required to show this Loading stop cleared; it does not prove a
configured JDK. Clean/check `slugd` before and after. Run pinned rustfmt,
`git diff --check`, plan and archive checks. Obtain independent DESIGN and
FINAL reviews for the new native rule-class/retained target boundary.

## Scope and stops

Base clean main `8c22cc128`. Allowlist: this manifest and canonical Current;
compact Stage 4 and matching Stage 9 notes after acceptance; production
`app/slug_loading_v2/src/{attrs,package,bzl_module}.rs`,
`app/slug_query_v2/src/graph.rs`, and
`app/slug_core_v2/src/runtime/build_requested_target.rs`; focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs`,
`app/slug_loading_v2/tests/build_file_loading.rs`,
`app/slug_query_v2/tests/{loading_query,observed_loading_query}.rs`, and
`app/slug_core_v2/src/runtime/tests/build_command_tests.rs`. The large
production/test modules remain cohesive Loading, Query and Build owners;
extracting a new owner would add routing without reducing the rule-family
coupling. Expected growth about 100 production/180 proof lines is a review
trigger, not a cap. `REPLAN` if exact Loading requires configured `--plugin`
resolution, a second retained attribute owner, a new DICE key, or invented
`@bazel_tools` content. No provider or Java execution implementation is in
scope. M7A remains partial.

## Immediate predecessor

WP829 accepted at `8c22cc128`: the native JDK Bzl's private `java.launcher`
configuration-field declaration now loads through the existing compact
identity. Its configured resolution remains fail-closed; authentic replay
advances to this distinct native `java_plugins_flag_alias` BUILD call.

## Acceptance receipt

Independent DESIGN and FINAL reviews accepted the append-only native rule
class, unit package target and existing Arc-backed attribute representation.
Pinned Bazel 9.2 source and an ignored-RC/no-fetch oracle establish the exact
selected declaration. Direct Cargo checks for Loading, Query, Analysis, Core
and CLI pass; 11 exact preflighted focused/protected tests pass (six Loading,
three Query, two Core), including direct-Build source/configuration A/B/A
failure. Pinned rustfmt, `git diff --check`, plan and archive checks pass.

The rebuilt normalized-PATH `slug build //:probe` first reports missing
`config_common.FeatureFlagInfo` at rules_python `flags.bzl:158`, but parallel
package-load ordering makes that alone insufficient to prove the JDK gate.
A direct authentic-package `slug query
'@bazel_tools//tools/jdk:java_plugins_flag_alias'` reaches the later
`proguard_allowlister` executable-rule validation at JDK BUILD line 140,
positively proving evaluation passed `java_plugins_flag_alias` at lines 48-50.
No `slugd` remained before or after the replays. The JDK package still does
not publish; configured Java, `--plugin`, provider merge and Java actions
remain deferred. M7A remains partial. Predecessor commit to acceptance was
about 27 minutes; compile, test and review phases were not timed separately.
