# Current Slug V2 Work Packet

Packet: WP-7-121-m7a-proto-compiler-configuration-field-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the Bazel 9.2 `configuration_field("proto", "proto_compiler")`
declaration in ordinary/Bzlmod `.bzl` Loading and private `attr.label`
defaults. The rebuilt authentic `slug build //:probe` in
`/tmp/slug-wp797-authentic-2YQmSt` must clear the protobuf `protobuf.bzl:81`
invalid-field stop; any later stop is separate.

Exact Bazel 9.2 slice: finite field-name/fragment recognition and typed
private-label default declaration, retaining the source-selected tools
repository identity. Slug-native: structural field/source identity and
diagnostics. Unsupported/deferred: configured `proto_compiler` option/default
label projection, `--proto_compiler` overrides, legacy proto toolchain use,
the valid `proto_toolchain_for_j2objc` sibling, arbitrary proto fields and
exact Bazel configuration/output identity. Configured use must fail explicitly
before dependency resolution, never substitute the annotation's default
label. Unknown fragment/field and public-attribute default remain errors.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`ProtoConfiguration.java:187-195` annotates `proto_compiler` backed by
`options.protoCompiler`; `ProtoConstants.java:43-45` supplies a default
label, while `BazelBuildApiGlobals.java:97-111` validates the field at
Loading and `StarlarkLateBoundDefault.java:54-86,207-240` resolves it later.
Ignored-RC/offline Bazel oracle `/tmp/slug-wp821-proto-compiler-FWKNVb`
accepts a private label default in query and exposes the default rule input;
an explicit local `--proto_compiler` override builds and resolves to that
local label. Its default build cannot complete offline because an unrelated
`rules_pkg+` repository fetch is disabled; no success is inferred from that
environment failure. Wrong fragment and unknown proto field fail at query.
Authenticated protobuf v33.4 `protobuf.bzl:76-83` eagerly constructs the
private executable exec-configured label default inside the argument to
`toolchains.if_legacy_toolchain`. Its default-modern helper then returns
`{}`; Slug's selected modern flag is true. The field value is therefore
constructed but discarded in the observed path. Legacy=false and configured
use are not admitted by this observation.

Slug's closed one-byte `ConfigurationField` in
`app/slug_configuration_v2/src/native/configuration_field.rs:112-240`
currently admits three proto names and rejects `proto_compiler`. Append one
`ProtoCompiler` discriminant, preserving prior numeric discriminants and
one-byte layout, and extend only name/fragment/typed-accessor matches. The
existing `ConfigurationFieldIdentity { field, tools_repository }`, frozen
late-bound rule slices, module source/manifest and package keys already own
structural equality, invalidation, export/replay, Need/error and A/B/A
restoration. Update the existing explicit proto-unsupported branch in
`native/configuration.rs:1094-1118` to include the new field; otherwise
configured use would leak a generic unknown-option error. No native option
projection, DICE key, cache, lock, second carrier, string payload or new
interner. The field is DICE-retained through existing immutable frozen rule
facts; evaluator values are scratch. Buck2-utility review retains the
existing compact enum and `Arc` carrier, importing no donor code.

## Evidence and validation

First move `proto_compiler` out of existing protected negative sets and
extend the focused Core/Loading tests so the source-shaped eager discarded
`attr.label(cfg="exec", executable=True, allow_files=True,
default=configuration_field("proto","proto_compiler"))` fails on the
unmodified branch. Assert one-byte field and unchanged identity size,
field/tools-repository equality/hash distinctions, frozen private default,
source/mapping A/B/A and package-recorded late-bound attribute. Preserve
the `proto_toolchain_for_j2objc`, unknown-field, wrong-fragment, BUILD and
public-default negatives. Prove configured `proto_compiler` fails with the
specific unsupported-proto diagnostic before dependency/implementation;
reuse the existing configured-proto protected path where suitable. Reuse the
offline oracle; no Stage 1 fixture growth.

Resolve pinned-nightly Cargo, compile selected Core/Loading (and directly
affected Analysis) test binaries with JSON `--no-run`, exact nonignored
selector preflight and serial focused/protected runs. Direct
Configuration/Loading/Analysis/Core/CLI compile and
`cargo build -p slug_cli_v2` precede one normalized-`PATH` authentic smoke.
The roughly 35-second smoke is needed to prove the selected transitive
Loading closure advances; focused tests cannot establish that. Check/clean
`slugd` before/after. Run Rust format, diff, plan and archive checks.
Independent DESIGN and FINAL reviews cover the retained enum/identity and
fail-closed boundary. No broad suite or remote transport; never inspect
BuildBuddy authentication.

## Scope and stops

Base clean main `e1b48a978`. Allowlist: this manifest, canonical Current
paragraph, compact Stage 4 and Stage 9 owner notes after acceptance;
production `app/slug_configuration_v2/src/native/configuration_field.rs` and
`configuration.rs`; tests in `app/slug_configuration_v2/src/native/tests.rs`
and `app/slug_loading_v2/src/host_package_load_tests.rs`, plus a focused
Analysis test only if existing configured guard proof is insufficient.
Rough growth 14 production/65 proof lines is a review trigger, not authority
for configured proto projection. Existing closed enum and resolver are the
natural owners; a new type or map would fragment this cold declaration
surface. REPLAN if Bazel rejects the field, if the selected modern branch
consumes the configured label, if representation grows, or if configured use
cannot stay fail-closed. A later error cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the declaration-only field
extension. The selected source-shaped Loading test first failed on unmodified
production at `configuration_field("proto", "proto_compiler")` with the
authentic invalid-field diagnostic (exit 101). The final enum remains one
byte and `ConfigurationFieldIdentity` retains its 32-byte layout on 64-bit;
the appended field has distinct structural identity, matching source and
tools-repository A/B/A behavior. The eager-discard private attribute and
frozen package late-bound slice retain the typed field; unknown, J2ObjC,
BUILD and public-default controls stay negative. The specific configured
proto error fires before dependency, toolchain or implementation activation
on both Analysis routes. Pinned-nightly JSON `--no-run` compiled Core,
Loading and Analysis test binaries; exact preflights selected 1, 2 and 2
nonignored tests respectively, and all five passed serially. Direct
Configuration/Loading/Analysis/Core/CLI `cargo check` and CLI rebuild passed.
The normalized-`PATH` authentic smoke clears the prior protobuf field stop
and next fails separately at `@@protobuf+//:BUILD.bazel:654` because `genrule`
is unavailable (exit 2, about 35 seconds). Pre/post checks found no `slugd`;
Rust format, diff, plan and archive checks pass. No remote transport or
BuildBuddy credentials were used. The 131 proof-line delta exceeds the rough
65-line trigger because it directly covers eager Loading, immutable identity
and both configured failure routes; final review accepted that proportionality.
Packet wall time from the accepted predecessor through final review was about
16 minutes; five final tests ran in under 0.1 second combined, while Cargo
preparation and the authentic smoke dominated validation time.

## Immediate predecessor

WP820 accepted at `e1b48a978`: positional `config.exec(value)` uses the
already-retained default/named transition and clears its rules_python stop.
The authentic build next reaches this separate protobuf field declaration.
