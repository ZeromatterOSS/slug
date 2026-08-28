# Current Slug V2 Packet

Packet: `WP-4-5-7A-configured-toolchain-selection-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `e1d0723ea`.

Result: freeze category 4's provider-independent configured platform and
toolchain selection boundary and its bounded implementation sequence. This
packet changes documentation only.

## Boundary and authority

Commit `e1d0723ea` completes category 3. Configured analysis consumes typed
command registrations before unchanged MODULE registrations through the sole
loading pattern/package walker. Category 4 consumes those canonical results;
it adds no command parser, registration store, provider graph or ruleset path.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` contributes only
peer ownership and representation guidance. Buck2-derived Rust remains the
generic Starlark syntax/evaluator and compact-utility substrate. Authenticated
BCR Starlark owns every rule definition and control path, including
`cc_internal`; `cc_common` is a later demanding client of the same generic
host/provider ABI, never a Rust C++ parser or rule engine.

The existing marker selector is a bridge, not architecture: it accepts one
mandatory root type, rejects aliases and target compatibility, gives every
candidate the same exec configuration, and analyzes the implementation under
the requesting target configuration.

## Source findings and compatibility

Bazel `PlatformOptions#getNormalized` retains only the first `platforms`
entry, while `computeTargetPlatform` uses that entry or falls back to
`host_platform`. `PlatformKeys` configures target, host and registered
platforms and replaces aliases with their actual labels. Earlier,
`RegisteredExecutionPlatformsFunction` inserts those actual keys with
`ImmutableMap.Builder#buildOrThrow`; distinct registrations converging on one
actual platform are not a first-occurrence deduplication surface. Bazel's
default host label is
`@bazel_tools//tools:host_platform`; in Bazel 9's embedded `tools/BUILD.tools`
that is an alias to BCR `@platforms//host`.

Slug already structurally stores typed `platforms` and `host_platform` label
rows but exposes no label projection. Its builtin `@bazel_tools` tree does not
contain upstream `tools/BUILD.tools`/`tools/build_defs.bzl`, so the default
target cannot honestly load. Its `to_exec()` also does not write the selected
platform into the `platforms` row. These are real prerequisites; an execution
candidate must not stand in for the target platform.

Bazel `RegisteredToolchainsFunction` and
`RegisteredExecutionPlatformsFunction` configure labels and obtain terminal
providers through aliases. All registered toolchain labels are validated;
`target_settings` are evaluated before selection and source order survives.
`ToolchainTypeLookupUtil` retains requested and actual type identity.
`SingleToolchainResolutionFunction` scans declarations in registration order
and, for every still-unfilled candidate, keeps the first declaration matching
that type, target platform and execution platform.

`ToolchainResolutionFunction` resolves all types, reports all missing
mandatory types, permits absent optional types, filters platforms missing any
mandatory type, then chooses the platform resolving the greatest number of
distinct types. Java's sequential `Stream.max` keeps the first candidate on
equal counts. The relevant source regressions are the registered target-
setting and platform-alias tests, toolchain-type alias test, optional first/
second/max/multiple/missing cases, missing mandatory case and no-type case.

The selected exact surface is configured static alias chains whose registered
execution-platform terminals remain distinct; direct
constraint settings with no default; target-setting filtering; explicit target
and execution constraints; `use_target_platform_constraints`; registered and
candidate ordering; requested/actual type identity; mandatory/optional
multi-type resolution; and stable maximal-optional platform choice. Rust
layout, DICE decomposition, diagnostics and structural configuration/exec
identity are Slug-native.

Unsupported/deferred are command `--platforms`/`--host_platform`, platform
mappings and platform flags; constraint-setting defaults beyond retaining and
rejecting their presence; platform `required_settings`, flags and allowed-type
policy; forced/host execution candidates and exec groups; configurable alias
`actual`; converged registered execution-platform aliases (fail closed rather
than invent first-wins behavior); provider payloads, selected implementation analysis under exec
configuration, `ctx.toolchains`, and exact Bazel configuration/output bytes.

## Frozen producers and identities

| Fact | Sole producer/key | Retained value |
|---|---|---|
| target-platform option | existing `SlugConfiguration`; new typed `target_platform_label()` projection | first visible canonical `platforms` label, otherwise visible canonical `host_platform`; errors preserve a non-visible repository |
| derived exec configuration | existing configuration owner; new `to_exec_for_platform(&CanonicalLabel)` | same typed native vector with `platforms=[selected actual platform]`, existing Starlark exec projection, and `Exec` kind; no parallel platform field |
| configured alias terminal | existing `ConfiguredNodeAnalysisKey` recursion | new general `actual_configured_target` projection: self for a direct node, child's terminal for an alias; alias edges and providers remain unchanged |
| configured platform | `ConfiguredPlatformKey(workspace, requested configured key)` | requested and actual configured keys, platform fact, and an immutable ordered actual constraint value/setting slice |
| target platform | `ConfiguredTargetPlatformKey(workspace, structural configuration)` | the exact `ConfiguredPlatformKey` result selected by that configuration's projection; category 4 calls it with the requesting target configuration and later exec analysis can reuse it |
| registrations | accepted `PreparedRegistrations` producer | command-before-MODULE requested labels and original positions, unchanged |
| toolchain resolution | `ConfiguredToolchainResolutionKey(workspace, structural target configuration, Arc<[ToolchainTypeRequirement]>)` | target platform, selected actual execution platform under its derived exec configuration, and ordered requested requirements with actual type and optional selected declaration |

`ConfiguredNodeResult` gains no copied platform/toolchain declaration store.
Direct native toolchain declarations become provider-empty configured nodes so
the general alias recursion can terminate on them; selection rereads the
loading-owned native declaration through the already-observed package carrier.
The result projection is canonical configured identity, not a provider.

`ConfiguredPlatformKey` resolves the platform alias and each constraint-value
and setting alias through that same configured-node owner, requires the
terminal native kinds, and rejects duplicate actual settings. It retains
`Arc<[ConfiguredPlatformConstraint]>`, canonical configured keys, `Dupe` and
`Allocative`. The exact child result Arcs remain DICE dependencies; packages
and native declarations remain loading-owned.

`ConfiguredToolchainResolution` retains
`Arc<[ResolvedToolchainRequirement]>`. Each row is
`{requested_type, actual_type, mandatory, selection}`; `selection` is `None`
only for an unresolved optional request, otherwise it contains the requested
registration label, actual declaration label and implementation label. The
target platform and selected execution platform are retained once. No
evaluator value, provider occurrence, checksum, display label or text digest
enters semantic identity.

All keys include the full structural configuration. Mapping provenance is
already present in canonical labels and configuration rows. Alias targets,
package carriers, condition results, constraint facts, declaration fields,
requirement order/mandatory bits and registration order are direct DICE
dependencies. Complete successes and outer observation errors use equality
cutoff; semantic errors and Need are invalid; cancellation publishes nothing
and same-graph repair must recover.

## Selection algebra

The implementation is one provider-independent computation:

```text
target := configured_target_platform(configuration)
candidates := resolve each registered execution platform alias in order;
              reject distinct requests with one actual platform key
declarations := resolve every registered toolchain alias in order;
                validate terminal kind and all declaration fields
types := resolve every requested type alias in request order
groups := group types by actual type, preserving first actual occurrence;
          group.mandatory = OR(request.mandatory)

resolve every configurable target_settings expression using the existing
selector resolver; batch every selected condition through the sole configured
condition key; a declaration is eligible only when every condition matches

validate target, candidate and declaration constraint values through
ConfiguredPlatformKey's actual setting identity; reject defaults in this slice
for each actual type group, in group order:
  for declaration in registration order where declaration.actual_type == group:
    if explicit target constraints do not match target: continue
    for each candidate not yet filled for this group, in candidate order:
      required_exec := target.constraints when use_target_platform_constraints,
                       otherwise declaration.exec constraints
      if required_exec matches candidate: fill with this declaration

accumulate all outer states, Need unions and semantic failures before terminal
precedence; then fail with all actual groups missing a mandatory request
suitable := candidates having a fill for every mandatory actual group
if suitable is empty: fail NoMatchingExecutionPlatform, distinct from a
                      mandatory group that resolved on no candidate at all
choose the first suitable candidate with the greatest number of distinct
filled actual groups
publish one row per requested requirement; converged aliases share the same
group selection, while each requested label and mandatory bit remains visible
```

`use_target_platform_constraints=True` with either explicit target or exec
constraints is a declaration error. In the admitted no-default surface,
matching means every required actual setting maps to the identical actual
value; extra platform settings are irrelevant. Repeated declaration aliases
do not change the first-compatible result. Converged execution-platform aliases
fail closed at registered-platform preparation and never reach selection.

Loading already rejects an identical requested type twice. If distinct
requested aliases converge, grouping prevents duplicate resolution and
optional scoring; mandatory is the strictest (`OR`) requirement, and all
requested aliases remain lookup-ready for category 6. This is the effective
Bazel 9 behavior while avoiding its internal duplicate-key accidents.

With zero requirements, an invoked resolution chooses the first candidate and
retains an empty requirement slice. Existing owners that bypass registration
because they have neither requirements nor local declarations remain
`UnresolvedDefault`; a registered local declaration without requirements still
gets the existing selected-platform-only context.

## Marker bridge and result cutover

`ToolchainTopology` retains the general resolution plus the ordered actual
candidate sequence, each projected under its own platform-specific exec
configuration; the selected key is one member of that sequence. The old
singular `ToolchainSelection` and
`ConfiguredActionToolchainContext(marker)` are produced only by
`prepare_marker_toolchain_bridge` after selection when exactly one mandatory
request resolved and no optional request is present. The bridge analyzes that
implementation under the requesting target configuration solely to preserve
the already accepted marker fixture. It cannot filter candidates, change the
chosen declaration, enter `ConfiguredToolchainResolution`, or widen its string
payload.

Category 6 deletes the bridge and singular marker context after it can analyze
every selected implementation under `execution_configuration`, require the
real `ToolchainInfo` occurrence from category 5's value graph, and expose it by
both requested and actual type through generic `ctx.toolchains`.

## Bounded rollout

1. `WP-4-5-7A-target-platform-and-exec-configuration-prerequisite`: add the
   two configuration projections, port the required Bazel 9.2
   `tools/BUILD.tools` as builtin `tools/BUILD` plus referenced embedded files
   verbatim, add the general configured-node actual projection,
   `ConfiguredPlatformKey` and reusable `ConfiguredTargetPlatformKey`, and activate
   constraint-category matching in the sole condition key. Retain and reject
   `constraint_setting(default_constraint_value=...)`; do not implement its
   matching semantics.
2. `WP-4-5-7A-configured-toolchain-selection`: add the one resolution key and
   value, replace the inline single selector, carry all rule requirements,
   derive the selected exec configuration, generalize topology, and isolate
   the marker bridge. Do not analyze an implementation as part of resolution.

The prerequisite is real: condition constraint matching must consume target
platform state without cycling through toolchain target-setting selection, and
the default platform label currently names absent builtin content. Do not
merge the packets merely to reduce scheduling rows.

Packet 1's source baseline is clean `cf91fe8de`; the docs-only architecture
commit must not change these blobs. Its exact existing-file allowlist, baseline
blob/lines and maximum added lines is:

| Path | Baseline blob / lines | Ceiling |
|---|---:|---:|
| `app/slug_identity_v2/src/label.rs` | `081bbb5b49238d361a83c437dbebd29b543334f4` / 537 | +30 |
| `app/slug_configuration_v2/src/native/configuration.rs` | `12b7e78d753633a42f0a5fc1ebdb4be0fdfe2536` / 1,540 | +90 |
| `app/slug_configuration_v2/src/native/tests.rs` | `4f9b01a779a6ebd5518c46728954348512987c8c` / 3,529 | +90 |
| `app/slug_bzlmod_v2/src/builtin_repository.rs` | `28819e3b37b6be21f1d855bbf68d9de6a37f4d44` / 889 | +20 |
| `app/slug_bzlmod_v2/src/host_module.rs` | `28c78c310ab6804da7824829efcc2c06f9d5bca8` / 5,349 | +4 |
| `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs` | `3002f00320df7540b4c4905610f11e42534b4f7b` / 149 | +35 |
| `app/slug_loading_v2/src/package.rs` | `bfc62b265d336a57a612e2f50def2ce3da587a2e` / 6,852 | +50 |
| `app/slug_loading_v2/tests/build_file_loading.rs` | `fa35fbbedc839f49b701ffc98810554349d28629` / 3,559 | +55 |
| `app/slug_analysis_v2/src/dice.rs` | `08711874e49e37b297b8a7eb989ba7a1c60d70e1` / 3,748 | +300 |
| `app/slug_analysis_v2/src/result.rs` | `2d5fb57083c522ea5229610e1c033371065ad790` / 668 | +100 |
| `app/slug_analysis_v2/src/lib.rs` | `777f01622c2051a3b54c2a697173e136072ac792` / 77 | +10 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `5fba7dd923011f724073ac8b6674b1ce4d283db9` / 6,304 | +320 |

The only new non-plan files are
`app/slug_bzlmod_v2/builtin/bazel_tools/tools/BUILD` (exact 50 lines and
nonexecutable `100644` mode from
pinned `tools/BUILD.tools`, SHA-256
`b0fbb2f8eb70acce9a307cca3d487a360f32a89d412e22a39c38346b979fc1a6`)
and `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs.bzl` (exact 106
lines and nonexecutable `100644` mode, SHA-256
`d5f935c4e72a365438711f08a2640094cbf0a03392eebb06d8cecdc58b8ab19c`).
Writable plans are only the canonical plan, Stage 6, Stage 9 and this manifest.
No Cargo, lockfile, sync-script, fixture or generated-evidence file is allowed.

Packet 1 caps 604 production, 500 proof and 1,200 total added Rust lines;
assets must be byte-identical rather than fit a growth cap. Its complexity is
one alias walk per configured node plus one linear constraint normalization
per platform; DICE owns reuse, while duplicate detection uses scratch
`SmallSet`. No nested declaration scan belongs to this prerequisite. Packet 2
remains reserved, with its exact allowlist and baselines materialized only
after Packet 1 lands; its provisional category caps are 760 production, 1,000
proof and 1,600 total added Rust lines. A baseline mismatch, allowlist need,
cap breach, superlinear platform normalization, competing owner or provider-
shaped result is `REPLAN`.

Reuse accepted registration, first-platform, canonical-package, condition and
lifecycle evidence. Add focused discriminators only for: default host alias to
BCR platform; explicit internal configuration projection; nonconvergent
platform alias chains and converged-platform fail-closed behavior; type and
declaration alias chains/convergence; constraint aliases/duplicates;
target versus exec constraints; target-to-exec policy; target-setting false and
error precedence; mandatory/optional multi-type maximal selection, ties and
the distinct no-common-platform failure;
exec-configuration distinction; zero-type platform choice; marker nonauthority;
cold cancellation and same-graph repair. Record upstream defaults, platform
policy, forced/host candidates and provider-context tests as skipped for the
declared boundary.

Run targeted and full affected crate tests serially where Cargo shares a target
directory, `cargo build -p slug_cli_v2` before binary gates, daemon cleanup,
locked owner/identity scans, rustfmt, `git diff --check`, cap accounting and
`scripts/v2_archive_status.sh`. Measure retained size and unchanged-result Arc
reuse for both keys; benchmark candidate-by-declaration selection only if the
accepted bootstrap fixture exceeds the existing analysis envelope.

## Stops

STOP and `REPLAN` for an execution platform substituted as target platform;
invented rather than verbatim `@bazel_tools`; display/checksum identity; copied
native option storage; a second alias, condition, constraint, registration or
selection owner; package/source discovery by a result consumer; a retained
standard map/set, cache, interner, evaluator value or lock across DICE; provider
analysis inside resolution; one-required-type architecture; silent optional-
to-mandatory conversion; first-wins deduplication of converged execution-
platform aliases; marker-influenced selection; Rust ruleset or
`cc_internal` control flow; `cc_common` specialization; Zabel as authority; or
a second material architecture correction after review.

## Architecture proof

The source-to-owner table, algorithm, identity/lifetime contract, real
prerequisite, bridge deletion condition, allowlists, caps and tests above are
independently reviewed and return `ACCEPT` after the one permitted correction.
The correction limits exact execution-platform aliases to distinct actual
terminals, names the separate no-common-platform failure, and freezes Packet
1's exact file/blob/line and upstream-asset inventory. A further material
architecture correction is `REPLAN`.
