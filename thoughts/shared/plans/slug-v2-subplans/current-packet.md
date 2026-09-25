# Current Slug V2 Work Packet

Packet: WP-7-149-m7a-default-apple-rule-fragment-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the rebuilt authentic Rust-toolchain
`cquery //:probe --output=label` past the WP848 stop at
`@@apple_support+//xcode:xcode_config.bzl:34` (`ctx.fragments.apple`),
then report the first new stop. Admit only the Bazel 9.2 configured
rule-context Apple fragment fields consumed on the pinned Linux
`local_config_xcode` empty-version path. Expose the fragment only to a
root rule declaring `fragments = ["apple", ...]`, with the same active-call
guard as the existing C++/coverage facades. Retain the structurally owned
configured target options as the sole source; do not synthesize an Apple
object from label text, Host OS, or evaluator defaults.

Declared-fragment access and the default/null values of the source-backed
fields are `exact` for the pinned Bazel 9.2 plus apple_support 1.24.2
slice. Rust valid-Unicode option text, full configuration and output
identity, the evaluator-local facade and structural DICE identity remain
`Slug-native`. Non-null dotted-version objects, unadmitted explicit Apple
command options, other Apple members, subrule/aspect Apple fragments,
Xcode selection, `apple_common.dotted_version` and
`XcodeVersionConfig`/provider behavior, generated-repository bytes,
non-Linux Apple paths and M7A closure remain `unsupported/deferred`.
No generic Apple-fragment parity claim follows from `dir(ctx.fragments)`.

## Pinned source, owner and boundary

The WP848 authentic replay exited 2 at the source-backed Apple access,
not at the previous built-in-route panic. Pinned
`apple_support` 1.24.2 `xcode/xcode_config.bzl:29-35,55-113,128-178`
(SHA-256 of `xcode_config.bzl`:
`7db1f74ab1dfac13e3988d8ab29578e6fd4ccddf82beb0e2f98d232b57e482ca`)
declares Apple and C++ fragments, reads Apple first, and selects the
explicit/no-version branch for the Linux built-in Xcode stub with empty
version attrs. In source order that branch reads `xcode_version_flag`,
four nullable SDK-version flags, four nullable minimum-OS flags and
`include_xcode_exec_requirements`; `prefer_mutual_xcode` is read only
on the local/remote Xcode branch. The existing C++ facade owns the
later `minimum_os_version()` call. Bazel 9.2
`AppleConfigurationApi.java:57-133`,
`AppleConfiguration.java:92-113,261-315` and
`AppleCommandLineOptions.java:49-168,304-318` define the option-backed
field types/defaults. `XcodeConfigTest.testEmptyConfig_noVersionFlag`
anchors the no-version default; the explicit-SDK test and
`AppleFragmentTest` cover later/non-stub branches, not this packet.

The configured target's `SlugConfiguration` already retains all relevant
native Apple options in its Arc-backed structural vector, included in
configured-key equality/invalidation. Add an evaluator-local Apple
projection analogous to `CppFragmentProjection`; it holds only a cheap
clone of that existing configuration and validates the named options
before publication. Nullable `xcode_version`/DottedVersion options may
project `None` only when the retained value is actually absent. A
non-null dotted option must fail closed, because Bazel returns a
`DottedVersionApi`, not a Rust string. A non-null Xcode-version text
option also remains unsupported within this default-only packet.
Project the two source-backed booleans from their retained native
values: `experimental_prefer_mutual_xcode` and
`experimental_include_xcode_execution_requirements`; the former is
available as a typed field but not claimed as exercised by the empty
stub. Do not bake SDK fallback defaults (8.4 etc.) into the fragment:
the pinned Starlark provider source applies those later.

The rule-context construction in `starlark_rule.rs` already computes
fragments from `key.configuration().slug_configuration()` and the
rule's required-fragment declarations. Create the Apple value only
for a declaring root rule; a missing structural configuration or
invalid/unsupported retained option fails before rule invocation.
`RuleFragmentCollection` in Loading owns active-call/declaration checks
and exposes only the named fields. No new DICE key, Host read, cache,
lock, fallback scan, Java runtime or source bytes enter Slug. The
configured rule key and existing loaded-rule source certificate own
invalidation, cancellation and final publication; the facade is
evaluator scratch, not a second retained configuration owner.

The affected configuration, Loading and Analysis files cross the
large-file trigger. Keep projection/type and the finite API in the
existing `native`/`analysis_fragments` owners; keep the one assembly
branch with the current fragment construction in `starlark_rule.rs`.
No new monolithic fragment file or duplicate option store is warranted.
Any new projection is Arc/Dupe/Allocative-compatible and reuses the
single structural option vector, following the Stage 9 retained-utility
policy; there is no new interner, compact collection or hash scheme.

## Evidence and validation

Add focused configuration tests that read the structural default
target and exec Apple options, verify the exact `None`/boolean values,
reject synthetic non-null dotted/text states or malformed types without
turning them into strings, and prove an A/B/A structural projection
identity using an admitted option mutation. Label such an A/B/A test
as whole-configuration retention, not Apple-option CLI replay: current
`NativeCommandOption` does not admit Apple SDK/Xcode flags. Add one
direct rule-analysis test in `slug_analysis_v2/tests/subrule.rs` beside
the existing C++/coverage facade tests: declared Apple access and all
default-path fields, undeclared access denial, active-call expiry,
target/exec defaults and unsupported-member denial. Reuse existing
synthetic fixture machinery; no copied `apple_support` file or new
oracle fixture unless a discriminating gap is demonstrated.

Use JSON `--no-run`, batch exact-name preflight and focused configuration,
Loading (if touched tests) and Analysis tests with serialized Cargo.
Compile the direct downstream CLI; rebuild `slug_cli_v2` after Rust
changes, check/clean stale `slugd`, then run one bounded authentic
one-shot cquery and record the next labelled stop. Reuse WP848's
accepted Bazel 9.2/Source evidence. Format, `git diff --check`,
plan-status and archive checks close validation. Independent DESIGN
and FINAL reviews gate edit and acceptance; Stage 9/appropriate owner
notes update only after acceptance.

Allowlist: this manifest and canonical Current; new
`app/slug_configuration_v2/src/native/apple_fragment.rs` plus
`native/{mod.rs,configuration.rs}` and `lib.rs` exports/tests;
`app/slug_loading_v2/src/analysis_fragments.rs` and its owned tests;
`app/slug_analysis_v2/src/starlark_rule.rs` and
`app/slug_analysis_v2/tests/subrule.rs`; compact Stage 9/owner notes
after acceptance. No Bzlmod, Core, provider, repository generator,
remote-cache/action or fixture edit. `REPLAN` if the authentic rule
requires a non-null dotted value before this finite ingress can be
observed; if configuration options lack structural ownership; if
another source needs a generic Apple fragment to distinguish this
stop; or if provider construction must execute to validate fragment
projection. Ordinary compilation or test failures stay in packet.

## Immediate predecessor

Independent DESIGN review ACCEPT verified the structural configuration
owner, exact default/null claim, non-null dotted/text fail-closed gate,
root-rule-only declaration boundary and bounded first-stop evidence.
It permits a direct Loading token/collection unit for active-call expiry
if the root-rule Analysis fixture cannot expose an expired context.

WP848 at `4a57c89fe` accepted the finite built-in Xcode extension
definition source with independent DESIGN/FINAL review and five
focused/protected tests. The rebuilt authentic cquery passed the former
built-in-route panic and first stopped at the pinned `apple_support`
rule's `ctx.fragments.apple` access. Xcode extension effects,
generated-repository bytes and Apple provider semantics remain open.

## Acceptance receipt

Independent FINAL review ACCEPT verified the structural configuration
owner, default/null typed fields, root-rule declaration/lifetime gate,
non-null text/dotted refusal and deferred subrule/provider boundaries.
JSON no-run builds for `slug_configuration_v2` (lib),
`slug_loading_v2` (lib) and `slug_analysis_v2 --test subrule` passed.
Exact-name preflight selected three new configuration tests, two new
Loading tests, one new Analysis test and the protected existing fragment
facade test; all seven passed. The configuration tests verify target/exec
default values, synthetic non-null/malformed rejection and whole-config
A/B/A identity; the Analysis test covers both routes, target/exec and
negative access. `cargo fmt --all -- --check`, `git diff --check`,
plan-status and archive checks passed. Rebuilt `slug_cli_v2` authentic
one-shot `cquery //:probe --output=label` exited 2 after roughly 65
seconds: pinned `xcode_config.bzl:76` called `_dotted_version_or_default`,
which first stopped at `apple_common.dotted_version` on line 400. The
prior `ctx.fragments.apple` stop is absent; no Xcode provider or generated
repository parity is inferred. No stale `slugd` was present before or
after. Focused compile/test preparation took roughly 110 seconds;
the CLI rebuild took roughly 37 seconds. The evaluator-local projection
reuses a cheap Arc clone of the existing structural configuration;
no new DICE key, collection, interner, Host read or memory-accounting
form was added.
