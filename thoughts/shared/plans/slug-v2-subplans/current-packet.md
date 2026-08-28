# Current Slug V2 Packet

Packet: `WP-4-5-7A-target-platform-and-exec-configuration-prerequisite`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `ce38f0373`.

Result: implement the independently accepted category-4 prerequisite: exact
configuration-owned target-platform selection for the admitted surface,
platform-specific exec configuration identity, one reusable configured
platform fact owner, and constraint-category matching through the sole
configured-condition key. Do not implement toolchain selection in this packet.

## Accepted design and authority

Commit `ce38f0373` freezes the full configured toolchain selection architecture
after independent correction review. This packet is step 1 of 2. The future
`WP-4-5-7A-configured-toolchain-selection` must consume these owners and may
not absorb or replace them.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority:
`PlatformOptions#getNormalized`/`computeTargetPlatform`, `PlatformKeys`,
`RegisteredExecutionPlatformsFunction`, `ConstraintCollection`,
`ExecutionTransitionFactory`, `ConfigSetting` and their focused tests. Clean
`../zabel` `0795445f…` is peer ownership/representation guidance only.
Buck2-derived Rust remains the generic Starlark evaluator and compact utility
substrate. BCR Starlark owns all rule definitions and control flow including
`cc_internal`; `cc_common` is only a later generic host/provider-ABI client.

## Compatibility

- **Exact:** first configured `platforms` label or `host_platform` fallback;
  pinned default `@bazel_tools//tools:host_platform` aliasing BCR
  `@platforms//host`; nonconvergent static alias chains; direct no-default
  constraint setting/value identity; duplicate-setting rejection; configured
  `config_setting.constraint_values`; selected-platform installation in the
  derived exec configuration.
- **Slug-native:** Rust/DICE layout, structural configuration bytes,
  observation carriers and unproved diagnostic wording.
- **Unsupported/deferred:** command `--platforms`/`--host_platform`, platform
  mappings/flags, configurable alias `actual`, constraint-setting default
  semantics, converged registered execution-platform aliases, platform
  required-settings/flags/allowed types, host/forced execution candidates,
  toolchain eligibility/selection, providers and implementation analysis.

The two upstream builtin files are ported byte-for-byte and with upstream
`100644` mode. Do not invent or simplify `@bazel_tools` content.

## Required implementation

### Configuration and embedded source

1. Add a direct typed conversion between a visible `ResolvedOptionLabel` and
   `CanonicalLabel`; non-visible repositories remain an error with their
   requested/owner state intact. Do not stringify or expose private option
   rows.
2. Add `SlugConfiguration::target_platform_label()`: return the first typed
   `platforms` entry, otherwise the typed `host_platform` label. Reject wrong
   shapes and non-visible labels. It works for any structural configuration.
3. Replace the generic call-site use of `to_exec()` with
   `to_exec_for_platform(&CanonicalLabel)`. It requires a Target configuration,
   writes exactly one selected actual platform into the existing `platforms`
   native row, applies the existing Starlark exec projection, and publishes one
   `Exec` configuration. No parallel field/store is allowed; keep `to_exec()`
   only if an existing direct API proof still needs it, and do not use it for
   action/platform identity.
4. Add upstream `tools/BUILD.tools` as builtin `tools/BUILD` and upstream
   `tools/build_defs.bzl`, extend the immutable catalog and exact snapshot
   digest tests, and prove the default alias reaches the selected BCR
   `@platforms//host` source. Do not touch the obsolete sync script.

### Sole alias and platform owners

5. Add `actual_configured_target` to `ConfiguredNodeResult`: direct configured
   nodes publish self; the existing alias branch publishes its child's terminal
   actual key while preserving the alias node, provider projection and
   `AliasActual` edge. Null nodes have no actual. Do not add another alias key,
   walk or cache.
6. Admit direct native toolchain declaration nodes as provider-empty configured
   nodes so alias recursion has a terminal. This does not admit their provider
   or implementation analysis. Add `ToolchainDeclaration` only to the typed
   node-kind enum.
7. Generalize configured native platform/constraint analysis to Target and Exec
   structural configurations. When a platform names a constraint-value alias,
   or a value names a setting alias, validate the terminal actual node through
   the existing child analysis and retain the original ordered graph edge.
   A constraint setting with `default_constraint_value` fails closed at this
   admitted slice after loading retains its canonical presence.
8. Add `ConfiguredPlatformKey(workspace, requested ConfiguredTargetKey)`. It
   consumes the requested configured-node result, its actual terminal result,
   and each existing platform/value/setting edge; publishes requested and
   actual keys, platform fact, and
   `Arc<[ConfiguredActionPlatformConstraint]>` normalized to actual value and
   setting keys; and rejects duplicate actual settings. It does not reread a
   package, copy a native declaration or own an event batch.
9. Add `ConfiguredTargetPlatformKey(workspace, structural configuration)`. It
   consumes only `target_platform_label()` and `ConfiguredPlatformKey`, so
   category 2 conditions and category 4 selection cannot form a DICE cycle.
   Its retained result is the exact platform result `Arc`.

### Condition cutover

10. Replace the current `constraint_values before target platform` rejection
    in the sole `ConfiguredConditionKey`. Compute the target-platform key and
    every requested constraint value through configured-node actual identity;
    match when every requested actual setting maps to the identical actual
    value on the platform. Extra platform settings do not matter. Preserve
    native/define/flag matching, all-empty rejection, batching, result shape,
    outer-before-Need-before-semantic precedence and invalid semantic errors.

## DICE, lifetime and complexity

`ConfiguredPlatform` is retained semantic state: immutable configured keys,
the existing compact platform fact and an `Arc` constraint slice with `Dupe`
and `Allocative`. `ConfiguredTargetPlatformKey` retains the exact child result
`Arc`. Alias/edge child results remain dependencies, not copied state. Vectors,
`SmallSet` duplicate detection and Need/error accumulation are compute scratch.

Complete successes and observed outer errors use equality cutoff. Need and
semantic errors are invalid. Cancellation publishes nothing; cold cancellation
at alias, constraint and target-platform boundaries must recover on the same
graph. A changed option, alias target, platform constraint, value setting,
setting default-presence bit or child fact invalidates the result.

Complexity is one existing alias traversal per configured node and one linear
normalization over a platform's constraint edges. There is no declaration or
candidate cross product. Measure retained size, unchanged-result `Arc` reuse
and A/B/A invalidation; no benchmark is required unless the default-host
bootstrap smoke exceeds the existing analysis envelope.

## Exact allowlist and caps

The accepted source baseline is `cf91fe8de`; `ce38f0373` changed docs only.
Before editing, every blob and line count below must match.

| Path | Baseline blob / lines | Maximum added lines |
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

The only new non-plan files are:

- `app/slug_bzlmod_v2/builtin/bazel_tools/tools/BUILD`: 50 lines, upstream
  `tools/BUILD.tools`, mode `100644`, SHA-256
  `b0fbb2f8eb70acce9a307cca3d487a360f32a89d412e22a39c38346b979fc1a6`.
- `app/slug_bzlmod_v2/builtin/bazel_tools/tools/build_defs.bzl`: 106 lines,
  mode `100644`, SHA-256
  `d5f935c4e72a365438711f08a2640094cbf0a03392eebb06d8cecdc58b8ab19c`.

Writable plans are only the canonical plan, Stage 6, Stage 9 and this manifest.
No Cargo, lockfile, sync-script, fixture or generated evidence is allowed.
Caps: 604 production, 500 proof and 1,200 total added Rust lines; upstream
assets must be exact. Plan caps remain 260 manifest, 140 Stage 6, 40 canonical
and 40 Stage 9 net lines.

## Proof and validation

Required focused proofs:

- visible/non-visible target-platform projection, first platform and host
  fallback; Target/Exec shape checks; selected platform changes exec structural
  identity while equal input reuses the configuration `Arc`;
- builtin catalog bytes, modes, snapshot identity, directory listing and
  default host alias through authenticated BCR `@platforms//host`;
- direct and multi-hop aliases for platform, constraint value and setting;
  wrong terminal kinds, cycles, default setting and duplicate actual setting;
- target-platform Target and Exec results, exact child `Arc` reuse,
  independent alias/constraint/option invalidation, A/B/A and cold
  cancellation/same-graph repair;
- constraint-category match/no-match/extra-setting plus competing error/Need;
  previous native/define/flag condition and selector suites unchanged; and
- locked scans proving one alias recursion, one condition key, no package read
  in platform-result consumers, no retained standard collection/cache/interner,
  no toolchain selector/provider/ruleset specialization and no lock across DICE.

Run targeted identity/configuration, builtin Bzlmod, loading and analysis tests,
then the full affected crate suites serially where Cargo shares `target/`.
Run rustfmt on touched Rust, `git diff --check`, exact asset SHA/mode checks,
blob/line/cap accounting, packet/canonical ID matching and
`scripts/v2_archive_status.sh`. Build `slug_cli_v2` before any binary smoke and
clean stale `slugd` before/after daemon-sensitive tests. Independent Sol review
must return `ACCEPT`; one material correction is allowed, a second is `REPLAN`.

## Stops

STOP and `REPLAN` for a missing baseline/allowlist file; non-verbatim builtin
content; execution candidate substituted for target platform; string/display/
checksum label conversion; copied option vector or parallel platform field; a
second alias/platform/condition owner; a package/source read in a result
consumer; constraint-default matching instead of typed rejection; lost alias
edge; shared exec configuration across different platforms; toolchain
selection, provider or implementation analysis; retained standard map/set,
cache/interner/evaluator value or lock across DICE; superlinear normalization;
Rust BCR/`cc_internal` control flow; `cc_common` specialization; Zabel
authority; cap breach; or a second material correction.
