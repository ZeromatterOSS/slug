# Current Slug V2 Work Packet

Packet: WP-7-151-m7a-default-xcode-version-info-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Advance the rebuilt authentic Rust-toolchain `cquery //:probe --output=label`
past the WP850 stop at pinned `@@apple_support+//xcode:xcode_config.bzl:98`:
`apple_common.XcodeVersionConfig(...)` is currently only a declaration key.
Admit the pinned Linux `local_config_xcode` empty-version constructor tuple,
its genuine `XcodeVersionInfo` provider publication, configured-target
membership/indexing, and its five function-valued fields. The constructor
must eagerly validate every named argument, and the retained value must
represent all source-observed semantic facts without keeping evaluator
closures. A downstream rule must be able to call `xcode_version()`,
`availability()`, `execution_info()`, `sdk_version_for_platform(platform)`
and `minimum_os_for_platform_type(platform_type)` on the returned provider.
The last two return typed DottedVersion values and preserve the pinned
default SDK/minimum versions. Admit `apple_common.platform_type`'s six
upstream string members for typed calls. The minimum-OS method compares
their literal strings; the SDK method reads a Starlark platform struct's
`.name`, so a structurally equivalent struct with a recognized name has
the same result. This does not admit general Apple fragment behavior.

For the exact 14-argument default tuple, provider identity, required-keyword
shape, eager validation, callable field results, platform mapping and
configured-provider lookup are `exact` against Bazel 9.2 plus pinned
apple_support 1.24.2. Structural retained representation, Rust valid-Unicode
diagnostics and Slug's configured identity remain `Slug-native`. Other
constructor tuples, non-default Xcode selection, provider variants, general
DottedVersion grammar, Java ApplePlatform objects, Objc provider
construction/indexing, generated-repository bytes and M7A closure remain
`unsupported/deferred`. The public API is finite; unknown or unmodeled
inputs fail closed rather than collapsing to this default provider.

## Pinned source, owner and boundary

WP850's rebuilt authentic cquery first stopped at `xcode_config.bzl:98`.
Pinned `apple_support` 1.24.2 `xcode_config.bzl:55-126` (SHA-256
`7db1f74ab1dfac13e3988d8ab29578e6fd4ccddf82beb0e2f98d232b57e482ca`)
passes ten paired SDK/minimum strings: iOS `8.4`, visionOS `1.0`, watchOS
`2.0`, tvOS `9.0`, macOS `10.11`; then `xcode_version=None`,
`availability="UNKNOWN"`, `xcode_version_flag=None`, and
`include_xcode_execution_info=False`. Bazel 9.2 builtins
`common/objc/apple_common.bzl:19-46` aliases `XcodeVersionConfig` to
`common/xcode/providers.bzl`'s `XcodeVersionInfo`; its initializer at
`providers.bzl:19-134` eagerly reparses ten versions, creates two execution
requirements, and returns five callable fields. Its provider declaration
at :136-173 and `common/objc/apple_platform.bzl:19-72` establish identity,
schema and platform values. `XcodeConfigTest.java:459-612` covers the
constructor, methods and eager bad-input error; the empty default is
anchored at :53-62. Non-default vectors in that test are deferred here.

Loading owns the `apple_common` key and evaluator-local provider facade.
The constructor validates exactly the 14 named values before returning a
typed default instance. Do not expose a raw string, ordinary empty provider,
or evaluator closure as a substitute for Bazel's callable fields. Build API
owns a dedicated compact, immutable `ProviderValue::XcodeVersionInfo`
variant, not an overloaded generic `ProviderOccurrence` with fake fields.
Analysis returns that typed value from the rule result, retains it under
builtin identity `XcodeVersionInfo`, and rematerializes the same finite
callable facade for a dependent target. Relax configured-target lookup only
for this key/typed variant; keep Objc's declaration-only rejection.
Reject a generic `ProviderOccurrence` claiming builtin `XcodeVersionInfo`
so an untyped occurrence cannot bypass the finite constructor gate.
The existing configured rule key, structural configuration options and
loaded-source certificates own dependencies/invalidation; the finite
constructor accepts no extra semantic input. A source/option change that
leaves the exact tuple may equality-cut off; any changed tuple fails closed.
The new retained value is DICE-owned through the existing configured-target
provider collection, not borrowed from evaluator scratch. There is no new
DICE key, cache, interner, lock, Host read or global registry. Use the
existing `SmallMap`, Arc/Dupe and Allocative patterns rather than a second
provider store; Stage 9's retained-utility policy applies.

Large-file boundary: `provider.rs` and `starlark_rule.rs` already exceed the
plan guide's review trigger. Put the finite constructor/facade in a narrow
Loading sibling module, with only key invoke/extractor and return-lowering
hooks in those owners. Do not mix this provider with general callable
serialization or Apple configuration flags.

## Evidence and validation

Use the pinned Bazel source and `XcodeConfigTest` as the accepted source
regression. Add focused Loading tests for the 14-argument shape, all five
methods, six platform types/ten platform instances, eager invalid-input
refusal, literal platform-type input, a cloned platform struct and
wrong-name/shape refusal, wrong/missing/extra/positional args,
freeze/global lifetime, and
preserved Objc negatives. Add Build API identity/equality/publication tests
for the typed retained variant. Add a direct configured-rule test returning
the provider and a dependent indexing it by
`apple_common.XcodeVersionConfig` and calling each method; protect generic
unsupported function-field rejection. Split the protected coincidentally
named `XcodeVersionInfo` occurrence test: `ProviderCollection::from_values`
must reject that untyped occurrence before publication, while typed Xcode
membership/indexing succeeds and Objc operations stay denied. Exercise
source A/B/A in one DICE by changing the producer from `availability="UNKNOWN"`
to unsupported `"LOCAL"`, requiring a fail-closed dependent, then restoring
the source and its structural result; this proves no stale success, not a
second admitted tuple or pointer reuse. Assert the dependent's exact method
results: `None`, lowercase `unknown`, exactly the two empty-valued
execution requirements, and every SDK/minimum mapping including catalyst's
iOS minimum versus macOS SDK. Reuse synthetic rule
fixtures; add no new oracle fixture unless a discriminating source gap is
found.

Use JSON `--no-run` and exact-name preflight for focused Loading, Build API
and Analysis tests; serialize Cargo with one target directory. Compile the
direct CLI dependent and rebuild `slug_cli_v2` before the authentic replay.
Check/clean stale `slugd`, then run one bounded authentic one-shot cquery
and record its first new labelled stop. The old line-98 provider-call and
any function-field-lowering error must be absent; this does not prove full
Xcode repository or M7A parity. Format, `git diff --check`, plan-status and
archive checks close validation. Independent DESIGN and FINAL reviews gate
implementation and acceptance; update compact Stage 6/9 owner notes only
after acceptance.

Allowlist: this manifest and canonical Current; Loading
`app/slug_loading_v2/src/{apple_common.rs,provider.rs,lib.rs}` and one narrow
new Xcode provider module; Build API `app/slug_build_api_v2/src/{providers/mod.rs,lib.rs}`
and a narrow provider test module; Analysis
`app/slug_analysis_v2/src/{analysis_value.rs,starlark_rule.rs}` and focused
`tests/starlark_rule.rs`; compact Stage 6/9 owner notes after acceptance.
No Bzlmod, Core, repository generator, remote-cache/action or fixture edit.
`REPLAN` if the authentic tuple differs, the five callable fields cannot be
represented and rematerialized without generic closure retention, actual
consumers require a new identity/configuration owner, or source-backed
provider behavior beyond the finite default is needed before this gate can
be observed. Ordinary compiler/test failures remain inside this packet.

## Immediate predecessor

WP850 at `44b9c1708` accepted the five default DottedVersion values with
independent DESIGN/FINAL review. Its rebuilt authentic cquery cleared the
line-400 constructor stop and first stopped at the separate line-98
`XcodeVersionConfig(...)` provider call. WP849's non-null Apple option gate,
broader provider/repository behavior and M7A closure remain open.

## Acceptance receipt

Independent DESIGN and FINAL reviews: ACCEPT. The focused Build API
`xcode_version_info` tests pass 2/2; Loading `apple_common::tests` pass 5/5
and `xcode_version_info::tests` pass 2/2. The Analysis typed-key test,
producer/consumer five-method and source A/B/A test, and protected generic
function-field rejection each pass. Exact-name preflight verified the selected
tests before execution. `cargo fmt --all -- --check`, `git diff --check`,
plan-status and archive checks complete the packet hygiene.

The rebuilt `slug_cli_v2` authentic one-shot
`cquery //:probe --output=label` in the existing Rust-toolchain fixture
cleared `@@apple_support+//xcode:xcode_config.bzl:98` and did not hit generic
function-field lowering. Its first new stop is
`@@rules_cc++cc_configure_extension+local_config_cc//:cc_toolchain_config.bzl:38`:
`ctx.fragments.apple.single_arch_platform.platform_type` is unavailable.
The exit was 2, not a successful query. No stale `slugd` was present before
or after the replay. This is a new deferred Apple-fragment boundary, not
Xcode repository, configured C++ or M7A acceptance.
