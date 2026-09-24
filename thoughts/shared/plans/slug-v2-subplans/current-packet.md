# Current Slug V2 Work Packet

Packet: WP-7-112-m7a-apple-configuration-field-declaration-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the single Bazel 9.2 `configuration_field(fragment = "apple",
name = "xcode_config_label")` declaration during ordinary `.bzl` initialization,
including the private `attr.label` default in authenticated rules_swift 3.1.2
`swift/toolchains/xcode_swift_toolchain.bzl:969-974`. The rebuilt authentic
`slug build //:probe` must clear the registration-row-56 invalid-fragment
stop. A later independently typed stop is acceptable. This packet does not
claim the generated Xcode toolchain target, configured Swift, or M7A close.

Exact Bazel 9.2 slice: the two-string `configuration_field` call recognizes
`apple` and exactly `xcode_config_label` in `.bzl` initialization; the result
is a late-bound label default acceptable in a private rule `attr.label`, not
a plain Label or an implicit `fragments = ["apple"]` requirement. Unknown
fragment, unknown Apple field and public-attribute forms reject. Existing
BUILD-global absence and the already admitted C++, Coverage, Java and Proto
fields remain protected. Slug-native: finite typed declaration identity,
diagnostic text and tools-repository carrier. Unsupported/deferred: Bazel's
load-phase default-label package/query edge, generated repository targets,
configured Apple/Xcode option and label resolution, Apple fragment access,
Swift analysis/actions, and any extension-invocation widening. The annotation
default `//tools/objc:host_xcodes` and the command-option default
`@bazel_tools//tools/cpp:host_xcodes` are different domains; do not conflate
them or synthesize a configured value from either.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`BazelBuildApiGlobals.java:96-109` validates registered fragment then
annotated field; `ConfiguredRuleClassProvider.java:825-835`,
`ObjcRules.java:37-38` and `AppleConfigurationApi.java:25-30` register the
`apple` fragment; `AppleConfiguration.java:320-326` declares the sole named
field and its tools-repository default; `StarlarkLateBoundDefault.java:58-94`
separates loading default and configured resolution.
`StarlarkRuleImplementationFunctionsTest.java:3022-3144` covers unknown
fragment/field, private-attribute restriction and no implicit Apple-fragment
access. Authenticated Swift has one top-level Apple call at the cited source.

The existing one-byte `ConfigurationField` enum in
`app/slug_configuration_v2/src/native/configuration_field.rs` is the sole
retained field owner. Add one Apple variant and its finite parse/name branch;
preserve one-byte layout, `Allocative`, Eq/Hash/Ord and the existing
`ConfigurationFieldIdentity(field, tools_repository)` structural identity.
The `subrule.rs` `.bzl` global already obtains the caller's source mapping;
`package.rs` already freezes the typed private-label default. Do not add a
string fallback, second registry, label parser, DICE key, cache, lock, source
read, fragment declaration or new graph representation. Existing source and
repository-mapping observations own A/B/A invalidation. Add an explicit
`UnsupportedAppleConfigurationField` error before the configured projection
falls through; every configured consumer must fail closed before dependency,
toolchain or implementation work. This is a finite enum extension, not a new
Buck2/V1 import; retain the Stage 9 compact typed-field pattern. No new
per-command or service memory is retained beyond the existing one-byte field
and existing identity, and no asynchronous owner changes.

## Evidence and validation

Fresh offline Bazel 9.2 scratch at `/tmp/slug-wp812-apple-config-field`,
with ignored RCs and disabled repository downloads, accepts a private
`attr.label` default (`query --output=xml //:good`, exit 0) and shows
`@bazel_tools//tools/objc:host_xcodes` as a rule input. Four isolated
negative queries reject unknown fragment, unknown Apple field, public
attribute and `attr.string` default (exit 7 each). The XML edge is an
oracle discriminator, not an admitted Slug package/query edge. Pinned
source/tests plus this oracle establish the bounded declaration claim; no
new Stage 1 fixture is needed unless a specific gap is demonstrated.

Focused Core tests must check field/name round trip, one-byte layout,
structural identity including different tools repositories, unknown names
and explicit configured refusal. Loading tests must construct a source-shaped
private rule, freeze and retain the typed field, test same-DICE source A/B/A
and direct-evaluator tools-mapping A/B/A, and cover invalid/BUILD/public-attr
controls; protect the existing Proto/C++ paths. Existing repository-mapping
observation/invalidation ownership is unchanged; this packet does not claim a
new same-DICE repository-mapping transition.
Use pinned-nightly Cargo JSON `--no-run`, exact nonignored selector preflight
and serial execution, plus named direct dependent compilation. Rebuild
`slug_cli_v2` before a normalized-PATH authentic one-shot smoke; check/clean
stale `slugd` before and after. Run Rust formatting, `git diff --check`,
archive and plan-status checks. Obtain independent DESIGN review before the
shared enum/error change and FINAL review of the actual diff and evidence.
No broad suite or remote-service gate is selected; leave `~/.bazelrc` and
BuildBuddy auth untouched.

## Scope and stops

Base clean main `580dee127`. Allowlist: this manifest, canonical Current
paragraph, Stage 4 and Stage 9 owner updates after acceptance; production
`app/slug_configuration_v2/src/native/{configuration_field,configuration}.rs`;
focused tests in `app/slug_configuration_v2/src/native/tests.rs`,
`app/slug_loading_v2/tests/{subrule_loading,bzl_invalidation}.rs` and the
existing focused mapping test in
`app/slug_loading_v2/src/host_package_load_tests.rs`. Approximate
production/proof growth 35/120 lines is a review trigger, not a permission
ceiling. The configuration owner is large but the new refusal branch stays
within its sole projection owner; the very large Loading test module already
owns the only direct-evaluator tools-mapping helper, so amend its adjacent
Proto regression rather than add another framework or split it for one field.
REPLAN if Bazel's observed value is not a
late-bound label default, the field needs a new retained owner, the authentic
stop requires configured Apple semantics, or current private-default
retention cannot preserve the typed identity. Do not paper over the next
independent stop.

## Acceptance receipt

Three independent DESIGN and FINAL reviews returned ACCEPT for the finite
declaration boundary. The offline Bazel 9.2 positive query and four isolated
negative queries distinguish the exact private `attr.label` constructor from
unknown names, public attributes and wrong attribute kind. Pinned-nightly
Cargo JSON `--no-run` compiled the Core library and Loading library plus both
focused integration binaries without compiler errors. Exact nonignored
preflight and serial execution passed: Core new field/identity/refusal 1/1;
Swift-shaped Loading plus two protected tests 3/3; direct-evaluator Apple/Proto
tools-mapping and frozen Proto recording 2/2; same-DICE Apple source
Apple→C++→Apple owned-identity restoration 1/1. The new DICE test clones only
the owned identity while the loaded package remains alive.

Two additional older `bzl_invalidation` selectors were run diagnostically and
did not pass. `same_dice_subrule_descriptor_attachment_and_typed_producer_restore_semantics`
segfaulted or panicked during panic formatting even after the attempted Apple
case was removed. `same_dice_attribute_and_rule_output_edits_are_semantic_and_recreate_cleanly`
failed its existing whole-rule equality assertion at line 2126. Source review
identified a plausible frozen-value lifetime hazard in the former test's
cloned-rule-outside-package pattern; a clean-base execution has not proved
baseline status. These failures are unresolved and are neither acceptance
evidence nor claimed passing. WP812 changes no C++/generic branch and its
focused safe-lifetime same-DICE test passes; track the old tests separately.

The rebuilt `slug_cli_v2` succeeds. A normalized-PATH authentic one-shot
`slug build //:probe` clears the prior row-56 Apple configuration-field stop
and reaches independent Loading failure in rules_rust → bazel_skylib
`selects.bzl:178` on `native.alias(visibility = None)` (exit 2). No configured
Apple/Xcode or remote transport was run. Pre/post-smoke `slugd` checks show no
processes. Rust format, diff, archive and plan-status checks pass. The packet
took roughly 19 minutes from selection through final review; the CLI build
and authentic smoke took roughly 30 and 46 seconds respectively.

## Immediate predecessor

WP811 accepted at `580dee127`: `apple_common.platform` ordinary structs
clear the prior rules_swift table load. Three focused/protected tests,
offline Bazel oracle and independent reviews pass. Structural behavior is
exact within the packet; generic struct text remains Slug-native. The
rebuilt authentic smoke reaches this Apple configuration-field declaration.
