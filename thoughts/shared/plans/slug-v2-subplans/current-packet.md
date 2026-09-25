# Current Slug V2 Work Packet

Packet: WP-7-148-m7a-builtin-xcode-extension-definition-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain alias `cquery //:probe --output=label`
past the post-WP847 selected-extension panic, then report the first
new stop. Admit the pinned built-in `bazel_tools` module's one demanded
definition, `@@bazel_tools//tools/osx:xcode_configure.bzl` export
`xcode_configure_extension`, as a request-owned built-in source for its
`local_config_xcode` repository demand. The selected graph's built-in
route identity and definition-host mapping must survive into the
Loading request; never construct a `HostCanonicalSelectedModuleDefinition`
for the built-in route.

The Bazel 9.2 built-in definition label/export, ownership by the
`bazel_tools` module and selected generated-repository name/mapping
are `exact` within this source ingress. Slug's immutable route/source
identity, DICE keys, observations and repository display identity
remain `Slug-native`. Extension invocation effects, generated repository
bytes, `@apple_support` Loading/providers, general built-in extensions,
Apple/Xcode provider semantics, full C++ toolchain analysis, exact
Bazel configuration/output identity and M7A closure remain
`unsupported/deferred`. Preserve WP837-847 accepted slices.

## Demand, pinned source and semantic owner

The rebuilt authentic cquery twice passed the old Apple-field guard
then exited 134 at `selected_repo_spec.rs:2901`, with backtrace
`selected_extension_definition_source:4477` ->
`selected_extension_demand::owner_inputs:641`. The panic is a built-in
route reaching a selected-module view; the backtrace does not identify
the requesting target or extension. The source-authored
`@bazel_tools//tools/cpp:host_xcodes` alias targets
`@local_config_xcode//:host_xcodes`, and built-in MODULE declares
`xcode_configure_extension`/`use_repo(local_config_xcode)`. This is
the finite authentic demand, not a claim that the backtrace proves
the owner ID.

Pinned Bazel 9.2 `src/MODULE.tools:16-17,40` and
`tools/osx/xcode_configure.bzl:325-329` define the extension and
its source file; the latter has no top-level `load()` imports.
`tools/cpp/BUILD.tools:124-128` supplies the alias.
`BazelDepGraphFunction.java:105-134,187-215` and
`BazelDepGraphValue.java:130-159` resolve the definition through
the built-in module's mapping and assign
`bazel_tools+xcode_configure_extension+local_config_xcode`
under the pinned nonconflicting extension set.
`SingleExtensionValue.java:69-74` agrees on the generated name.
Upstream `BazelDepGraphFunctionTest:321-326` and
`ModuleExtensionResolutionTest:3081-3177` discriminate namespace
and mapping behavior; the direct Apple shell test is Darwin-only,
so it is not a Linux acceptance oracle. Reuse WP847's fresh
query/cquery default-label oracle; do not add a duplicate fixture
without a demonstrated gap.

Slug `HostSelectedExtensionDemandKey` already selects the generated
name's typed extension owner, and
`HostSelectedExtensionOwnerInputsKey` already depends on selected
routes/mappings and retains the definition-host mapping plus matching
module inputs. A built-in provenance carries a
`BuiltinBazelToolsRouteIdentity` whose snapshot and manifest digest
come from the DICE-owned built-in module producer. Project this
identity into a distinct
`HostSelectedExtensionDefinitionSource::BuiltinBazelTools` variant
only for the exact Xcode label/export and only when the host route is
the canonical built-in `bazel_tools` route. Match that provenance
before any selected-definition view. Other built-in definitions
return the existing typed unsupported outcome, not a panic.
Keep the public canonical selected-definition key's
`BuiltinDeferred` result unchanged.

`RootRepositoryRoute::for_selected_extension_definition` may turn
the new source into its existing `RootRepositorySource::BuiltinBazelTools`
route, copying the graph's identity rather than regenerating
`CURRENT`. Loading's existing external-Bzl route should consume
the verbatim built-in source; no new loader or special Java/Bazel
runtime is admitted. The pinned definition file has no top-level
imports, so the existing built-in Root route's empty Bzl-load mapping
is sufficient only for this source file. Keep the full selected
definition-host mapping separately in the request for invocation
and generated-repository ownership; do not treat the empty route
mapping as general built-in mapping parity. Source bytes remain the
existing built-in catalog/capability observation, and transient
`Need`, complete errors, cancellation and final publication continue
through their existing DICE owners. No new key, lock, Host read,
fallback scan, global cache or direct filesystem path is allowed.

The new enum variant is DICE-retained semantic state. Retain only
the cheap copyable built-in route identity and existing
Arc/CompactString/Allocative request machinery; no new collection,
interner, hash shortcut, clone-heavy snapshot copy or memory-accounting
form. Full request Eq/Allocative must still distinguish changed graph
mapping and built-in manifest identity. The touched
`selected_repo_spec.rs` and `host_module.rs` exceed the plan's
large-file trigger, but the new branches remain in their existing
route projection/constructor owners; splitting selected graph or
root-route responsibilities for this finite variant would add an
unnecessary second semantic owner.

## Evidence and validation

Add a focused built-in owner-input test using the existing synthetic
selected mapping: exact Xcode owner -> new source/graph identity ->
built-in Root route; other built-in extension -> typed unsupported;
missing/duplicate/inconsistent host routes retain their errors.
Add a DICE test of actual pinned built-in MODULE demand through
`HostSelectedExtensionDemandKey` and
`HostSelectedExtensionOwnerInputsKey` that verifies the exact
`local_config_xcode` canonical name, complete selected mapping,
and stable warm result. Prove changed/restored host-mapping structural
request identity with A/B/A input projections using the same route
identity. Preserve the existing
`BuiltinDeferred` canonical-definition negative and built-in catalog
byte/usage tests. Add one direct Loading test that loads the pinned
Xcode `.bzl` through the new source route and checks its exported
definition before invoking or instantiating the extension. No static
source-string assertion substitutes for that test.

Use JSON `--no-run`, exact-name preflight, focused Bzlmod tests and a
direct `slug_loading_v2` compile check; serialize Cargo. Rebuild
`slug_cli_v2` after production changes, check/clean stale `slugd`,
then run one bounded authentic one-shot cquery and record the first
new stop without assigning an unlabelled error to Xcode. Format,
diff, plan-status and archive checks close validation. Independent
DESIGN and FINAL reviews gate edit and acceptance. Update compact
Stage 5/9 owner notes only after acceptance.

Allowlist: this manifest and canonical Current;
`app/slug_bzlmod_v2/src/{selected_repo_spec.rs,host_module.rs}`;
`app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs`;
`app/slug_loading_v2/src/module_extension.rs` tests only if needed
for isolated source-loading proof; compact Stage 5/9 notes after
acceptance. No Loading production, Core, registry, repository
generator, Apple provider, remote cache/action or fixture edit.

`REPLAN` if source loading requires a generic built-in mapping change
or canonical-route owner before this finite definition can be
observed; if the selected graph lacks the built-in identity or
complete host mapping; if invocation must execute to discriminate
source selection; or if exact Bazel source contradicts the finite
tuple. Ordinary compile/test failures or a stale CLI stay in this
packet. A later generated-repository/`@apple_support` stop is
separate.

## Immediate predecessor

Independent DESIGN review ACCEPT verified the exact tuple gate,
graph-selected built-in identity, retained definition-host mapping,
unchanged canonical-definition negative and no-load source boundary.
It required the direct pre-invocation Loading proof and structural
host-mapping A/B/A gate now stated above.

WP847 at `54f03db41` accepted the configured Apple Xcode label
projection from the structural native option. Configuration tests
passed 93/93; the rebuilt authentic cquery newly exposed the
built-in selected-extension route panic above. Independent DESIGN
and FINAL reviews accepted that bounded projection; M7A remains
partial.

## Acceptance receipt

Independent FINAL review ACCEPT verified the exact tuple gate, graph-selected
route identity, retained complete host mapping and unchanged unsupported
boundaries. JSON no-run builds for `slug_bzlmod_v2` and `slug_loading_v2`
passed; exact-name preflight selected two new Bzlmod tests, one new Loading
test and two existing `BuiltinDeferred` negatives. All five passed. The
new tests cover missing/duplicate/inconsistent routes, other built-in
definitions, A/B/A mapping equality, actual pinned built-in MODULE demand,
warm DICE owner inputs and direct Xcode `.bzl` export loading before
invocation. `cargo fmt --all -- --check`, `git diff --check`, plan-status and
archive checks passed. The rebuilt `slug_cli_v2` authentic one-shot
`cquery //:probe --output=label` exited 2 at
`@@apple_support+//xcode:xcode_config.bzl:34:22`: `ctx.fragments.apple` is
unavailable. The former selected-route panic is absent; no Xcode repository
or Apple-provider parity is inferred. No stale `slugd` was present before
or after. Compilation and focused tests took roughly 75 seconds; the
authentic replay took roughly 65 seconds. The packet used existing
Arc/CompactString/Allocative structures and one cheap copyable route
identity, without a new key, collection, interner or memory-accounting
form.
