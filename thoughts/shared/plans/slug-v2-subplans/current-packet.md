# Current Slug V2 Packet

Packet: `WP-6-7A-testing-bootstrap-loading-implementation-r4`

Milestone: M7A category 6 registered-toolchain closure prerequisite.

Base: accepted selected-BCR archive realization `1599d730c`, accepted ordered
transform identity `01f2802f0`, accepted exec-configured loading `831e574e6`,
accepted canonical repository Host capabilities `26a68d61c`, and parked
proof-only registration base `20ad71ffa`. The passing four-row proof draft and
retained selected-context R2 candidate remain dirty and read-only.

## Why this corrected implementation packet is active

The selected-BCR archive owner is terminally accepted. Its 32 focused rows
pass, the full core run adds no packet-related failure, and two fresh-root real
REAPI replays consume the authenticated rules_shell global PAX comment,
strip-prefix, ordered patch and registry MODULE before loading three nested
rules_shell modules. Both then stop at the same next boundary:

```text
@@rules_shell+//shell/private:sh_executable.bzl:89:31
Variable `coverage_common` not found
```

This is not an archive, parser, `set`, `cc_common`, C++ rule or REAPI defect.
`coverage_common` is one member of Bazel 9.2's fixed testing-support bootstrap.
The user requires category architecture rather than a one-symbol patch, so this
packet audits and freezes the complete `TestingBootstrap` loading/Host-ABI
category before any Rust edit.

Independent R2 review accepts the category, identity, context and callability
architecture, but finds one mechanical implementation-allowlist omission. The
generic advertised-provider carrier change also reaches three existing clean
assertions in `host_package_load_tests.rs` that render user-provider identities.
R3 admits only those assertions and requires explicit `.user_id()` projection;
it does not add `Display` to `ProviderIdentity` or widen runtime behavior.
Independent correction review returns `ACCEPT`; the architecture and bounded
implementation successor below are now frozen.

The first R3 compile exposes one additional assertion in the same clean test
file and carrier-fallout category: a two-element `.map(ToString::to_string)` at
the advertised-provider deduplication proof. R3 explicitly authorized exactly
three assertions, so the candidate stops unstaged. R4 adds only that fourth
explicit user-provider projection; no production contract or cap changes.
Independent R4 correction review returns `ACCEPT`.

## Authority and learned facts

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority:

- `TestingSupportRules.init()` installs one `TestingBootstrap` containing
  `testing`, `coverage_common`, `InstrumentedFilesInfo`,
  `AnalysisFailureInfo`, and `AnalysisTestResultInfo`;
- `TestingModuleApi` exposes `ExecutionInfo`, deprecated `TestEnvironment`,
  and `analysis_test` on `testing`;
- `CoverageCommonApi` exposes `instrumented_files_info` with the complete Bazel
  9 signature, including source/dependency attributes, coverage support and
  environment, extensions, metadata, reported-to-actual sources, and baseline
  coverage files;
- `CoverageCommon` performs typed validation, derives `InstrumentedFilesInfo`
  from the rule context and dependency/provider graph, and restricts its
  internal-only arguments; and
- `BazelStarlarkEnvironment` installs bootstrap objects as stable predeclared
  bindings for ordinary `.bzl` loading. A function body may retain these
  identities during loading without invoking analysis semantics.

The authenticated rules_shell 0.6.1 source calls
`coverage_common.instrumented_files_info(ctx, source_attributes = ["srcs"],
dependency_attributes = ["deps", "_runfiles_dep", "data"])` inside the lazy
`_sh_executable_impl`; module evaluation needs the predeclared namespace, while
configured analysis later needs the real Host/provider operation.

Slug already centralizes the exact Rust Starlark language universe in
`slug_starlark_v2`; its Buck2-derived `SetType` owns `set` semantics and is not
part of this packet. Loading already has distinct context overlays and retained
Host/provider callables for `platform_common`, `DefaultInfo`,
`RunEnvironmentInfo`, `OutputGroupInfo`, `cc_common`, and `depset`, but it has
no testing-bootstrap category owner.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is peer design/optimization guidance only. Its
`generic_rule_loading_namespaces.zig` separates reusable stable namespace
identities from invocation-local analysis capabilities, and its loading runtime
publishes testing/coverage bindings through a shared predeclared table. Slug
may learn from that ownership split, but must not copy Zig code, accept its
compatibility claims, or omit Bazel 9.2 members/arguments merely because Zabel
does.

## Compatibility classification

- **Exact:** the complete Bazel 9.2 `TestingBootstrap` top-level name set;
  complete `testing` and `coverage_common` member inventories; process-stable
  top-level namespace/provider identity across admitted ordinary `.bzl`
  loading contexts; builtin provider-key identity through `provides` and
  analysis rematerialization; exact provider type/repr/callability; and
  fail-closed invocation of every lawfully callable operation whose analysis
  semantics are not yet admitted. This successor is exact for loading and
  declaration identity, not for configured coverage or analysis-test effects.
- **Slug-native:** Rust retained-value representation, collision-safe provider
  identity, static frozen allocation for member callables, structural
  configuration/action identity, memory accounting, and unsupported-invocation
  diagnostics. Stable member-method pointer identity is not an exact claim.
- **Unsupported/deferred:** invocation of callable `testing.ExecutionInfo`,
  `AnalysisTestResultInfo`, `testing.TestEnvironment`,
  `testing.analysis_test`, and
  `coverage_common.instrumented_files_info`; all resulting provider values,
  target-graph and action effects; Java/HotSpot state; `@_builtins` `.bzl`
  loading; and every non-testing language module. `InstrumentedFilesInfo` and
  `AnalysisFailureInfo` are exact noncallable provider keys, not unsupported
  callable placeholders.

## Frozen architecture

### Complete category and context boundary

One new loading-owned `testing_bootstrap` module publishes the complete fixed
Bazel 9.2 category in one operation. Its exact top-level names are
`testing`, `coverage_common`, `InstrumentedFilesInfo`, `AnalysisFailureInfo`,
and `AnalysisTestResultInfo`. `dir(testing)` contains exactly
`ExecutionInfo`, `TestEnvironment`, and `analysis_test`; `dir(coverage_common)`
contains exactly `instrumented_files_info`.

Install the category only in `complete_loading_globals(true)`, the shared
ordinary `.bzl` overlay used for admitted BUILD- and MODULE-loaded modules. Do
not add it to BUILD files, MODULE.bazel, REPO.bazel, core evaluation or
`slug_starlark_v2::populate_universe`. Bazel's `createBuiltinsBzlEnv` also
excludes `registeredBzlToplevels`; Slug does not yet own a distinct
`@_builtins` loading context, so that pseudo-repository remains unsupported
rather than inheriting this exact ordinary-`.bzl` claim.

### Stable namespace and provider identity

Use `GlobalsStatic` plus static frozen values so repeated construction of Slug
loading globals reuses the same top-level namespace and provider identities.
Represent the four provider keys—three top-level constructors plus
`testing.ExecutionInfo`—with loading-owned typed builtin-provider tokens, not
`struct`, `None`, display-text lookup or per-module allocation. Preserve Bazel
observable `Provider` type, `<function Name>` repr and exact `dir()`
inventories. `ExecutionInfo` and `AnalysisTestResultInfo` are callable
providers; `InstrumentedFilesInfo` and `AnalysisFailureInfo` expose no invoke
path.

Extend `declaration_provider_id` and every advertised-provider carrier from
user-only `ProviderId` to the already shared `ProviderIdentity`; user providers
remain `ProviderIdentity::User` and builtin tokens become
`ProviderIdentity::Builtin`. Extend the existing
`starlark_provider_identity` and `alloc_starlark_provider_callable` owners for
the four fixed builtin keys. Identity then flows through rule/aspect
`provides`, immutable package declarations and actual analysis
rematerialization without a parallel registry or digest stand-in. Required-
provider constraints remain outside this successor because the exposed path is
`provides`; broadening them requires its own discriminating consumer evidence.
Existing `SmallMap`/`CompactString` provider storage remains sole retained
representation.

Every namespace operation is a callable token. Calling one in this successor,
or either lawfully callable provider constructor, returns an explicit
unsupported-analysis error before producing a value or effect. Attempting to
call either noncallable provider follows ordinary Starlark noncallable
behavior. Merely compiling/freezing a lazy function that names a token is exact
and side-effect free. The later invocation successor must replace barriers
with an evaluator-local composite analysis capability; it may not use a global
callback or overload the current one-purpose `ToolchainInfo` marker.

This deliberate split is required by the live path: rules_shell loading only
retains `coverage_common.instrumented_files_info`, while Bazel's real call
collects context attributes, transitive `InstrumentedFilesInfo` providers and
coverage artifacts. Implementing that call without its configured provider
graph would be a semantic stub. The entire declared bootstrap category lands
together now; configured invocation remains one separately bounded category.

### Retained-cost boundary

All top-level fixed values live once in static frozen heaps. Member callables
may use the same static allocation as a Slug-native optimization, but equality
does not rely on pointer stability. Per-`Globals` population copies only frozen
handles; per-module evaluation retains ordinary references already owned by
starlark-rust. No DICE key, lock, hash owner, background task, dynamic string
registry, clone-heavy side table or memory-accounting exemption is added.

## Active implementation boundary

Implement only `WP-6-7A-testing-bootstrap-loading-implementation-r4` against:

- `app/slug_loading_v2/src/lib.rs` blob
  `0cd03c1d18a8bff96a9e7b8f8ff8bce1d65ad777`;
- `app/slug_loading_v2/src/package.rs` clean blob
  `a35c3274fc2e010a8fc54e223fc97f250e8c910e`, allowing only the advertised-
  provider `ProviderIdentity` carrier/parser hunks and one category-population
  hook beside `complete_loading_globals(true)`; its existing selected-context
  diff, currently SHA-256
  `0295ce524e14da9f5a2ee6e623111177f32620462ffa3590eb1e2dca448e3128`,
  remains read-only and unstaged;
- `app/slug_loading_v2/src/provider.rs` blob
  `410e007296f5d3e1894b18442f2a53e598f3f816`;
- new `app/slug_loading_v2/src/testing_bootstrap.rs`; and
- new `app/slug_loading_v2/src/testing_bootstrap_tests.rs`; plus the isolated
  builtin-rematerialization proof in
  `app/slug_analysis_v2/src/analysis_value.rs` blob
  `e0f314611e2165f40915c7eaf7d5baaf4d3e325f`; and
- exactly four user-provider rendering assertions in clean
  `app/slug_loading_v2/src/host_package_load_tests.rs` blob
  `16a046d63baa4eaf94a6abea8b63a5a0cb002b16`: the two-element mapped rendering
  at the advertised-provider deduplication proof plus the three scalar
  assertions already named by R3. Each must use explicit `.user_id()`
  projection after the carrier conversion; adding a generic `Display`
  implementation for `ProviderIdentity` is forbidden.

Cap additions at 500 production, 700 proof and 1,200 aggregate Rust lines.
`package.rs` already exceeds the authoring split trigger, so it receives only
the typed-carrier correction and composition hook; all category behavior and
most proof stay in the two new cohesive modules. `provider.rs` remains below
2,000 lines, and `analysis_value.rs` receives proof only. No analysis
production, core, BCR, registration, command, REAPI, Cargo, fixture or
starlark-rust file may change.

Proof must cover exact top-level/member inventories and context exclusion;
process-stable top-level pointer identity across repeated globals, module
freezing and imports; exact `ProviderIdentity` retention in rule/aspect
`provides` and rematerialization for all four keys; exact provider type/repr/
callability; unsupported invocation for every lawful callable; absence of side
effects; context exclusion including BUILD and an explicit unsupported
`@_builtins` statement; and a rules_shell-shaped lazy function that freezes
without executing its coverage call.

Run focused new tests, the full serial `slug_loading_v2` and
`slug_analysis_v2` suites, the parked registration-row test, and the real REAPI
dependent twice from fresh roots. Rebuild `slug_cli_v2` before tests using
`SLUG_V2_BIN`; clean stale `slugd` before and after. Then run fmt, diff/scope/
blob/cap/dirty-isolation audits and `scripts/v2_archive_status.sh`. The real
dependent may expose a later missing category or admitted invocation boundary;
it must at least clear every testing-bootstrap loading lookup consistently.

## Terminal implementation evidence

The bounded R4 implementation is terminally `ACCEPT` after independent review.
Its cached diff is confined to the seven authorized Rust files and adds 287
production, 381 proof and 668 aggregate lines. The existing selected-context
`package.rs` candidate reconstructs to its recorded SHA-256
`0295ce524e14da9f5a2ee6e623111177f32620462ffa3590eb1e2dca448e3128`
and remains unstaged byte-for-byte.

Focused TestingBootstrap loading and shared-provider rematerialization proofs
pass, as do the complete serial `slug_loading_v2` and `slug_analysis_v2`
suites and the parked four-registration-row proof. `slug_cli_v2` rebuilds
successfully. Two daemon-clean fresh-workspace/fresh-output-root
`rules-rust-073-toolchain-owner` cqueries both clear every testing-bootstrap
lookup and stop identically at the next independent boundary while reading
`@@bazel_tools//tools/build_defs/cc:action_names.bzl`: its built-in source lacks
the required immutable source owner. This is not a bootstrap inventory,
identity, context or invocation failure.

Formatting, diff, blob, allowlist, cap, dirty-isolation and archive checks pass
apart from the archive checker's three already tracked retained-document
exceptions. Independent terminal review confirms the exact inventories,
static top-level identity, builtin-provider carrier/rematerialization,
callable/noncallable barriers, user-only required-provider constraints, lack
of `ProviderIdentity: Display`, and absence of new retained hot-path state.

## Implementation evidence and exclusions

The R3 candidate remained unstaged during correction review. Production and
proof edits are now authorized only in the frozen implementation allowlist
above. Scheduling status may additionally change only in:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`; and
- its bounded monthly history file when log rotation requires it.

The terminal design must name exact implementation blobs/baselines, production
and proof caps, focused/full/direct-dependent validation, dirty-state
isolation, and explicit `REPLAN` stops. Independent architecture review is
mandatory before implementation.

## Stops and successor

`REPLAN` for a `coverage_common = struct()` or rules_shell-only token; partial
TestingBootstrap inventory; provider identity derived from display text;
invocation without configured context ownership; loading-time execution of a
lazy rule implementation; analysis behavior delegated to Bazel/Java; global
mutable callbacks; universal-environment widening; `set`, parser, `cc_common`
or `cc_internal` work; or implementation outside the accepted R4 allowlist.

BCR Starlark remains the complete rule/control-flow owner, including
`cc_internal`; `cc_common` is only another consumer of the generic Host/provider
ABI. After the accepted implementation successor clears the complete
testing-bootstrap loading lookup in both fresh-root real dependents, classify
the next boundary: resume the unchanged proof-only four-registration-row
closure if it passes, or freeze one new generic category packet if it does not.
The retained selected-context R2 review remains after that closure.
