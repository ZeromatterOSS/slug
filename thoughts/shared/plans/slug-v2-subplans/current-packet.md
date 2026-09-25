# Current Slug V2 Work Packet

Packet: WP-7-147-m7a-apple-xcode-configured-field-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain alias `cquery //:probe --output=label`
past the post-WP846 `_xcode_config` configured-field rejection, then
report the first new stop. Admit exactly the configured projection of
`configuration_field(fragment = "apple", name = "xcode_config_label")`
from the owning `SlugConfiguration`'s retained native
`xcode_version_config` Label option. Under the admitted default, the
canonical result is `@@bazel_tools//tools/cpp:host_xcodes`, never the
distinct Loading annotation fallback `//tools/objc:host_xcodes`.

The Bazel 9.2 field-to-option projection, default configured label and
ordinary same-configuration implicit dependency edge are `exact` for
this slice. Slug's structural configuration identity, DICE equality,
canonical label representation and display token remain `Slug-native`.
Explicit `--xcode_version_config` command overrides, Loading's query
fallback edge, `@local_config_xcode` materialization and providers,
`XcodeVersionConfig` semantics, full C++/Swift toolchain analysis,
exact Bazel configuration/output identity and M7A closure remain
`unsupported/deferred`. Preserve all accepted WP837-846 slices.

## Pinned owner, evidence and implementation boundary

The authentic one-shot Slug cquery at `/tmp/slug-wp797-authentic-2YQmSt`
now fails resolving `_xcode_config` because the Apple configured field
is unsupported. The diagnostic has no owning target label. Pinned
rules_cc 0.2.17
`cc/private/toolchain/unix_cc_toolchain_config.bzl:2032-2038`
declares that late-bound attribute in the generated
`@local_config_cc` toolchain config; `BUILD.tpl:108-135` and
`unix_cc_configure.bzl:350-365` establish its generation path.
Source attribution does not prove which configured instance emitted
the authentic error.

Pinned Bazel 9.2 `AppleConfiguration.java:92-100,321-327` returns
`AppleCommandLineOptions.xcodeVersionConfig`.
`AppleCommandLineOptions.java:293-326` defaults that option to
`@bazel_tools//tools/cpp:host_xcodes` but supplies
`//tools/objc:host_xcodes` as the annotation's Loading fallback.
`StarlarkLateBoundDefault.java:38-61,75-109` resolves the configured
default in the owning rule's already-selected configuration; the attr
has no `cfg` transition of its own. Pinned
`XcodeConfigTest.testConfigurationFieldForRule` discriminates two
option-selected labels/providers. Fresh ignored-RC Bazel 9.2 in a
temporary Linux workspace independently returned the objc fallback
from `query 'deps(//:probe,1)'` and the cpp default from
`cquery 'deps(//:probe,1)'`. An explicit Bazel option changed the
configured edge; Slug's closed CLI option set does not admit it.

Slug Loading already retains the typed field-plus-tools-repository
identity, and the native descriptor registry already retains
`xcode_version_config` as a Label with the cpp default. Change only
`configuration_field_label` in
`app/slug_configuration_v2/src/native/configuration.rs` to read that
option via the existing native-label projection. Do not synthesize a
constant from the field identity or use its tools-repository spelling:
that spelling belongs to Loading. Keep nonvisible/wrong-typed native
label errors fail-closed. The existing Analysis configured-dependency
row owner consumes the result, so no Analysis production route changes.
Existing native-option vector, configured key and DICE-owned option
equality provide structural identity and invalidation; no new DICE
key, lock, retained representation, hash, interner, Host read, cache
or memory-accounting form is allowed.

The production method is a small projection in its existing owner;
splitting it or expanding the command-option allowlist would widen
this packet. No donor runtime, Bazel semantic delegation, repository
fallback or Apple provider emulation is permitted. The default alias
leads to `@local_config_xcode//:host_xcodes`; pinned
`tools/osx/xcode_configure.bzl:284-303` generates a non-Darwin stub,
but rules_cc eagerly reads its Apple provider. Clearing this field
cannot claim that later closure.

## Validation and stop rules

Replace the existing configuration-unit negative with a positive
default/identity test: the configured label must be the cpp label for
two distinct field tools-repository identities while their typed
identities remain distinct. Also test target and execution
configuration projection through the same field. Preserve invalid
field-name, Java and Proto negatives. The existing Analysis
configured-dependency route is unchanged; its integration harness
cannot expose a pre-child row without following the default
`@local_config_xcode` alias and crossing this packet's deferred
boundary. Do not add a downstream-failure test as if it proved the row.
Pinned source and the discriminating temporary Bazel 9.2 oracle are
the reference; do not add a flaky registry-dependent fixture or
misclassify query's Loading fallback as configured evidence.

Use JSON `--no-run` compilation, exact test-name preflight, focused
tests, and `cargo build -p slug_cli_v2` after the production Rust edit.
Then run the bounded authentic one-shot cquery, recording the first
new diagnostic without attributing an unlabelled error to a target.
Serialize Cargo and check/clean stale `slugd` before and after the
smoke. Format, diff, plan-status and archive checks close validation.
Independent DESIGN and FINAL reviews gate implementation and acceptance;
add compact Stage 6/9 owner notes only after acceptance.

Allowlist: this manifest and canonical Current;
`app/slug_configuration_v2/src/native/{configuration.rs,tests.rs}`;
compact Stage 6/9 owner notes after acceptance. No command
option, Analysis production, Loading, repository, rules_cc, remote
cache/action or fixture edits.

`REPLAN` if the native option is absent or noncanonical in a
configuration where the field resolves, if the field requires the
annotation fallback in configured analysis, if an Apple provider or
repository owner must run before the projection can be observed, if
structural option identity fails to distinguish admitted values, or if
fresh-binary replay plus inspection of the projected option/type shows
an unmodeled owner or prerequisite that the accepted projection
cannot explain. A stale binary, missed branch or ordinary implementation
bug must be corrected within this packet. A later
`@local_config_xcode`/provider stop is a separate packet.

## Acceptance receipt

The only production change projects the retained Apple native Label
through `native_label` and removes the obsolete Apple-unsupported
error arm. No configuration storage, DICE owner, Loading or Analysis
route changed. The focused configuration test checks the cpp label
for target and exec configurations under three typed field identities,
including two distinct tools-repository spellings, and rejects the
objc Loading fallback. JSON `--no-run` compilation and exact-name
preflight passed. The focused test passed after correcting only its
local test Host OS input; the full configuration library passed 93/93.
`cargo build -p slug_cli_v2` passed after the production edit.

Two bounded one-shot authentic cqueries using the rebuilt CLI no
longer emitted the Apple configured-field rejection. Both instead
exited 134 at `selected_repo_spec.rs:2901`:
`builtin selected routes fail before publication`. The second run's
backtrace identifies `selected_extension_definition_source:4477`
through `selected_extension_demand::owner_inputs:641` as the caller.
This is a Bzlmod selected-route invariant panic, not evidence of
Xcode provider or full toolchain success. Its path is newly observed
after the Apple projection; the unlabelled diagnostic and concurrent
graph evaluation do not prove which target demanded the route.
Pinned built-in `bazel_tools/MODULE.bazel:16-17` uses
`xcode_configure_extension`, and `tools/cpp/BUILD:124-127` aliases
`host_xcodes` to its generated repository. That source path makes
Xcode demand a plausible cause, but the backtrace has no requested
canonical-repository or extension-owner ID, so it remains an inference.
No stale `slugd` was present before or after. Format, diff, canonical
plan-status and archive-status checks passed. No credential file was
read; neither BuildBuddy nor NativeLink was needed for this field
projection.

Independent DESIGN review ACCEPT verified the existing structural
option owner, configured-versus-Loading label distinction, target/exec
unit discriminator and bounded authentic replay. The review corrected
one pinned rules_cc line citation and required the stop rule to
distinguish an implementation failure from an architecture contradiction.
Independent FINAL review ACCEPT checked the production delta, pinned
query-versus-cquery distinction, target/exec test, 93/93 configuration
suite, fresh CLI and cautious unlabelled-panic attribution. The
selected-extension route is the next separately reviewed owner.

## Immediate predecessor

WP846 at `bcc16845f` accepted package-inherited notice licenses on
configured native filegroups with fresh Bazel/Slug evidence, focused
root/external and restoration tests, and independent FINAL review.
Authentic cquery cleared that guard and reached the Apple field
rejection above. WP845 at `df7c4992b` ported the pinned Bazel 9.2
built-in C++ whitelist package verbatim. M7A remains partial.
