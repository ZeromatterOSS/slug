# Current Slug V2 Work Packet

Packet: WP-7-104-m7a-proto-configuration-field-declarations-r1
Status: ACCEPTED 2026-09-24; M7A remains partial

## Outcome and compatibility

Admit three source-authentic protobuf v33.4 `configuration_field`
declarations for `proto_toolchain_for_java_lite`, `proto_toolchain_for_java`
and `proto_toolchain_for_cc` on fragment `proto` as typed, declaration-only
private `attr.label` defaults in Bzl Loading. The authentic normalized-PATH
build must pass its current row-55 invalid-fragment stop. A later
independently typed stop is acceptable; protobuf configured toolchain
resolution, target success and M7A close are not claimed.

Exact Bazel 9.2 slice: recognize the three selected annotated fields at
`.bzl` evaluation, preserve their late-bound identities through private
label defaults, and retain the existing distinct invalid-fragment/invalid-
field diagnostics. Slug-native: finite field admission and the existing
compact field-plus-tools-repository identity. Unsupported/deferred:
configured resolution of these fields, the other proto fields, proto
fragment access/options semantics, and broader protobuf/toolchain behavior.
Do not return fabricated default labels or silently treat the configured
fields as `None`.

## Source, owner and decision

Pinned Bazel 9.2 `BazelBuildApiGlobals.java:96-109` resolves
`configuration_field` against registered fragments and annotated fields;
`BazelRuleClassProvider.java:253-259` registers `proto`, and
`ProtoConfiguration.java:188-230` annotates the selected three names. The
native options use related but sometimes different spellings (notably
`proto_toolchain_for_javalite`) and already have Slug descriptors; their
deferred option status is not configured-field support. Authenticated
protobuf v33.4 `java_lite_proto_library.bzl:76,172`,
`bazel_java_proto_library_rule.bzl:77,161` and
`bazel_cc_proto_library.bzl:126,193` eagerly invoke the three selected
fields while constructing dict arguments to `toolchains.if_legacy_toolchain`.
That helper discards the dict under Bazel 9.2's default enabled toolchain
resolution; argument evaluation still requires the declarations. The direct
current `rules_java` load closure demonstrates Java Lite and Java; C++ is a
deliberate, separately oracle-checked sibling in the authenticated protobuf
source, not a claim about that immediate load closure. The `proto_compiler`
call in `proto_lang_toolchain_rule.bzl:159` is in an untaken conditional
branch, but `protobuf.bzl:81` also calls it eagerly outside the demonstrated
current closure. `proto_toolchain_for_j2objc` is annotated upstream but not
called in this authenticated archive. Both remain valid Bazel fields, not
admitted Slug declarations. Preserve the current caller/source
and `@bazel_tools` repository-mapping identity path.

The sole declaration owner is
`app/slug_configuration_v2/src/native/configuration_field.rs`'s flattened
`#[repr(u8)] ConfigurationField`, consumed by
`app/slug_loading_v2/src/subrule.rs::configuration_field_global` and the
existing private `attr.label` late-bound carrier. Add three discriminants
and only their source-spelled parser/projection. Do not add a public sibling
enum, map, interner, per-field string, new retained carrier, option
projection, DICE key or cache. `ConfigurationFieldIdentity` still owns only
the one-byte field and `CanonicalRepoName`; clone/equality/hash/Allocative,
source-fingerprint invalidation and immutable rule slices remain structural.
Ordinary no-field rules retain no extra allocation. Keep the one-byte field
and current identity layout. `configuration.rs` must reject configured
resolution of these selected proto fields with a typed unsupported
diagnostic, following the accepted Java declaration-only precedent. The
DICE ownership and no-lock-across-compute guidance in
`docs/developers/dice.md` is unchanged.

## Evidence and validation

An independent pinned Bazel 9.2 offline scratch oracle at
`/tmp/slug-wp804-proto-field` used `--ignore_all_rc_files` and a fresh output
base. `query //:good --output=xml` passed for all three private late-bound
defaults and emitted rule inputs `@bazel_tools//tools/proto:{javalite,java,cc}_toolchain`.
Separate unknown fragment and
unknown field cases exited 7 with distinct loading diagnostics; making the
late-bound attribute public exited 7 with the private-attribute diagnostic.
Pinned source annotations and the authenticated archive census corroborate
all three fields. An offline `cquery` with local labels set through the three
native proto options reached all three selected defaults, but this packet
does not implement or claim those configured projections in Slug. No BuildBuddy credentials,
remote transport or NativeLink execution entered this oracle. The scratch
fixture stays outside this checkout; add no Stage 1 fixture unless the focused
regression exposes a demonstrated evidence gap.

Add focused Core tests for exact selected spellings, a truly unknown proto
name, valid-but-deferred proto fields, field/identity/hash/layout and
explicit configured rejection.
Add focused Loading tests for Bzl-only API, private label-default retention
through declaration/freeze, source/repository-mapping identity, and invalid-
name boundaries; reuse protected Java, C++, coverage and existing unknown-
fragment tests. Do not invoke a configured proto consumer as a success claim.
Resolve pinned cargo/rustc from `rust-toolchain`; compile affected library
tests with Cargo JSON `--no-run`, batch exact selectors through preflight,
then run selected tests serially. Rebuild `slug_cli_v2` before the authentic
normalized-PATH build; check/clean `slugd` before and after. Run formatting,
diff, archive and plan-status checks and obtain independent DESIGN and FINAL
reviews. No broad suite or remote-service gate is selected by this
declaration-only packet.

## Scope and stops

Base clean main `b4777a817`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 and Stage 9 owner contracts only after acceptance;
production `app/slug_configuration_v2/src/native/configuration_field.rs`,
`app/slug_configuration_v2/src/native/configuration.rs`; focused tests in
`app/slug_configuration_v2/src/native/tests.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs`. Production growth under
roughly 55 lines and proof under roughly 160 lines are review triggers, not
permission ceilings. REPLAN if Bazel's selected fields resolve at loading
to different identities, the authentic calls need configured semantics or a
new retained owner to pass the present stop, the one-byte layout changes, or
a later stop cannot be separated from this declaration boundary.

## Immediate predecessor

WP803 at `b4777a817` accepted the source-authentic rules_kotlin 1.9.6
selected-BCR regular `0555` mode. The CLI rebuild and authentic smoke cleared
that archive stop and reached the present protobuf `proto` configuration-field
loading error at row 55. M7A remains partial.

## Acceptance receipt

Independent DESIGN and FINAL reviews returned ACCEPT after the test separated
the Bazel-invalid `missing` field from valid-but-deferred `proto_compiler` and
`proto_toolchain_for_j2objc`. The pinned Bazel 9.2 offline scratch oracle
accepted all three selected private defaults and projected the source default
toolchain labels; its local-option-override `cquery` also confirmed all three
Bazel field/option mappings without authorizing Slug configured projection.
The source-authentic protobuf v33.4 calls and direct rules_java closure were
independently audited. No Stage 1 fixture was added because the pinned source
and scratch oracle discriminate this finite Loading slice; no upstream or
credential material entered the checkout.

Pinned-nightly Core and Loading JSON library test compiles passed. Exact
preflight verified three Core and four Loading nonignored selectors. The three
Core tests and four Loading tests passed; after the final deferred-field
assertion edit, the Loading compile and preflight were repeated and the
changed exact test passed again. The two new Loading tests cover frozen
aspect/rule and actual package recording; the Core test pins one-byte field,
unchanged identity layout/hash and typed configured rejection. The CLI rebuild
passed. Authentic one-shot `slug build //:probe` from the normalized-PATH
scratch workspace cleared the old proto fragment stop and failed separately
at row 55 in
`@@rules_kotlin+//src/main/starlark/core/options:opts.javac.bzl:64:13`:
`Value of type provider_callable is not hashable`. No `slugd` remained before
or after. Format, diff and plan-status checks passed. No remote cache,
NativeLink or BuildBuddy gate was selected. The exact declaration slice is
accepted; configured proto semantics, other valid fields, Kotlin hashability,
target success and M7A close remain deferred.
