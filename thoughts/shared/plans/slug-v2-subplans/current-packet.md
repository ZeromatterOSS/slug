# Current Slug V2 Work Packet

Packet: WP-7-152-m7a-default-apple-single-arch-platform-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Advance the rebuilt authentic Rust-toolchain `cquery //:probe --output=label`
past WP851's first stop at pinned
`@@rules_cc++cc_configure_extension+local_config_cc//:cc_toolchain_config.bzl:38`:
`ctx.fragments.apple.single_arch_platform.platform_type`. Admit the
default Apple fragment selector's actual Bazel `apple_platform` MACOS value,
including its four fields, type, repr, immutability, same-type equality and
hashability. The platform type string must feed the already admitted typed
`XcodeVersionInfo.minimum_os_for_platform_type` method as `"macos"`, giving
typed default minimum OS `10.11`. This is one semantic slice in the full
Rust-toolchain cquery, not configured C++ or M7A closure.

For `apple_platform_type="macos"`, empty `apple_split_cpu` and absent/empty
`macos_cpus` under the already admitted default Apple option path, the
selector and MACOS value's type, four fields and repr are `exact` against
Bazel 9.2. Rust-native allocation/identity, valid-Unicode diagnostics and
Slug's structural configuration identity are `Slug-native`; do not claim
Bazel enum pointer identity or numeric hash bytes. Nondefault platform
types, nonempty split CPU or macOS CPU lists, all other single-arch Apple
selection, unsupported Apple fields, Xcode variants, generated repository
bytes and configured C++ closure remain `unsupported/deferred`. Invalid or
unmodeled selector values fail closed on this attribute, never silently
projecting MACOS.

## Pinned source and semantic owner

Bazel 9.2 `AppleConfiguration.java:109-156,212-258` selects the Apple platform
from `apple_platform_type`, the nonempty split CPU or first platform CPU, and
`ApplePlatform.forTarget`; its actual getter does not read CoreOptions.cpu.
`AppleCommandLineOptions.java:188-208,274-280` declares the three defaults.
On Linux x86_64 the default CPU is `x86_64`, and both supported default host
architectures map to `ApplePlatform.MACOS` through
`ApplePlatform.java:40-72,110-183`. `ApplePlatformApi.java:23-82` declares
Starlark type `apple_platform` and the four fields. `ApplePlatform.java:87-108,
194-203` supplies immutable/hashable enum behavior and lowercase repr.
`XcodeConfigTest.java:615-670` exercises the default Apple platform-type to
minimum-OS expression; `ObjcStarlarkTest.java:386-466` checks another
platform's exposed type/repr. Pinned rules_cc 0.2.17
`cc/private/toolchain/unix_cc_toolchain_config.bzl:37-40,1748` has this one
Apple access, followed by the WP851 Xcode provider call. Its source digest is
recorded in `target/wp757/source-attribution.json`; no new oracle fixture is
needed for the already source-discriminated default case.

`SlugConfiguration` already retains the complete native option vector and
the configured rule key owns all option identity, target/exec transitions and
invalidation. `AppleFragmentProjection` is a cheap evaluator-local view of
that structural owner. Add a lazy typed selector gate there for the three
option values; do not gate `AppleFragmentProjection::new`, which must retain
WP849's unrelated admitted fields for nondefault selectors. Loading owns a
zero-state evaluator-local `apple_platform` MACOS value with four literal
fields: `name="macos"`, `platform_type="macos"`,
`name_in_plist="MacOSX"`, `is_device=True`, repr `macos`. Its type,
equality/hash and reflection must not be faked by the distinct
`apple_common.platform.macos` Starlark struct. The existing root-rule
fragment declaration/call-lifetime gate applies unchanged, including
undeclared and expired access. Analysis already constructs the fragment from
the configured structural value; no production Analysis hook or DICE owner
changes. No new retained option store, DICE key, Host read, cache, interner,
lock, closure, or global registry. Use existing Starlark simple-value hash
and `Allocative` patterns; no Buck2/V1 import or graph representation change.

## Evidence and validation

Add a configuration projection test for default target/exec selection and
direct negative option-value checks for nondefault/malformed platform type,
split CPU and macOS CPU list using the validator called by the lazy getter.
Inspect that wiring and preserve the prior-field regression: the new gate
must not run in `AppleFragmentProjection::new`. Internal Apple selectors are
not in the command-option overlay, so do not invent a CLI A/B/A path. Reuse
WP849's whole-configuration A/B/A as structural equality evidence and test
that a changed unrelated option does not alias the projection. Add Loading
tests under an active declared Apple fragment for exact type/fields/repr,
equality and matching hashability across two independently allocated MACOS
values, distinction from an ordinary struct, and
unknown-field refusal; preserve undeclared/expired fragment negatives.
Extend the existing configured target/exec Apple-fragment test in both
Analysis routes to call the platform type and the WP851 Xcode minimum-OS
method, asserting `10.11` where the same provider prerequisite is available;
otherwise use a focused direct rule whose provider comes from a real
configured dependency, not an authored stand-in. Test the public
production-wrapper path, not only a manually allocated facade.

Resolve the pinned Rust toolchain first. Serialize Cargo; compile focused
Configuration, Loading and Analysis test executables with JSON `--no-run`,
batch exact-name preflight, then execute only selected tests and protected
fragment/Xcode controls. Rebuild `slug_cli_v2` before the authentic replay;
check/clean stale `slugd` before and after. One bounded one-shot cquery must
clear `single_arch_platform.platform_type` and its immediately following
Xcode minimum-OS call; record the first new labelled stop or success. The
old Apple field/type/method errors must be absent, but a later stop cannot
prove full toolchain analysis. Run rustfmt, `git diff --check`, plan-status
and archive checks. Independent DESIGN review gates implementation; FINAL
review gates acceptance. Update compact Stage 6/9 owner notes only after
acceptance.

Allowlist: this manifest and canonical Current; Configuration
`app/slug_configuration_v2/src/native/apple_fragment.rs`; Loading
`app/slug_loading_v2/src/analysis_fragments.rs` and
`app/slug_loading_v2/src/analysis_fragments/apple_fragment_tests.rs`;
Analysis `app/slug_analysis_v2/tests/subrule.rs` or one focused existing
configured-rule test file if its real-provider dependency requires it;
compact Stage 6/9 notes after acceptance. No Bzlmod, Core, repository
generator, Build API, remote-cache/action, runtime fixture or general Apple
option parser edit. `REPLAN` if the default selector does not yield MACOS,
the typed value requires new retained/Host identity, the authentic consumer
demands nondefault selection before this gate can be observed, or a real
configured dependency cannot be tested inside this ownership boundary.
Compiler/test corrections inside the accepted contract are not REPLAN.

## Immediate predecessor

WP851 at `6cb0858ec` accepted the exact default `XcodeVersionInfo` tuple,
typed publication and five callable fields. Its rebuilt authentic cquery
cleared the apple_support provider-call stop and first reached the separate
rules_cc `single_arch_platform.platform_type` field. Nondefault Apple/Xcode
state and M7A remain open.

## Acceptance receipt

Independent DESIGN and FINAL reviews: ACCEPT. Pinned Bazel 9.2 source and
tests establish the default selector and typed MACOS fields; the Slug
Configuration owner tests pass 4/4, Loading Apple fragment tests pass 3/3,
configured Analysis target/exec Legacy/Observed test passes 1/1, and the
real Xcode producer/consumer source A/B/A test with fragment platform type
feeding a typed `10.11` minimum passes 1/1. Protected generic function-field
rejection passes 1/1. JSON `--no-run` compiles and batched exact-name
preflights selected each focused test executable. The helper-level negative
selector tests and source-inspected lazy getter gate do not claim a public
nondefault configuration route.

`cargo build -p slug_cli_v2` passes. The rebuilt broad one-shot
`cquery //:probe --output=label` exits 2 at a different label,
`@@platforms//os:none is not a config_setting`; it is not a successful
Rust-toolchain query and alone would not prove the Apple call executed.
A temporary root `//:local_probe` alias to the authentic generated
`@@rules_cc++cc_configure_extension+local_config_cc//:local` rule in the
same scratch fixture isolates that rule. Its rebuilt one-shot cquery exits
0 with the configured alias label; the unchanged rule necessarily evaluates
`_target_os_version(ctx)` at rules_cc line 1748, which reads this Apple
platform field and invokes the WP851 Xcode minimum-OS method. The temporary
alias was removed and the scratch BUILD restored. No stale `slugd` was
present before/after. Formatting, diff, plan-status and archive checks pass.
The next `@@platforms//os:none` selector boundary and M7A remain open.
