# Current Slug V2 Packet

Packet: `WP-4-5-7A-effective-native-command-fdo-closure-prerequisite-r1`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: accepted subrule loading successor `965cfde5e`. Unrelated dirty
analysis/toolchain and loading work remains parked; implementation must stage
and validate only packet hunks.

## Why this prerequisite exists

Independent review returned `REPLAN` on the configured-subrule R2 packet. Its
ten exact `cpp` `configuration_field` projections require non-default native
configuration states, but Slug currently captures only Starlark build settings,
`extra_toolchains`, and `extra_execution_platforms`. A public/test-only raw
mutator would bypass command capture, repository mapping, descriptor conversion,
implicit requirements, and DICE ownership.

Admit the bounded native command closure below through the real command and
configuration owners. Then resume
`WP-4-5-7A-subrule-configured-hidden-dependencies-and-query-r3` on the accepted
producer. This packet does not edit loading, subrule, query, or configured-rule
dependency code.

## Observable result

Joined build and cquery flags for the admitted FDO closure are captured in raw
order before one-shot/daemon selection, resolved with the root repository
mapping through the existing command-configuration DICE key, converted by the
pinned Bazel 9.2 descriptors, and published once through the sole structural
`SlugConfiguration` owner. Repetition, last-wins, boolean no-form, label
mapping, and the `fdo_instrument`/`cs_fdo_instrument` implicit `copt` rows
invalidate and restabilize A/B/A in one retained daemon graph.

The packet exposes no generic native-option mutation API and invokes no
subrule or C++ rule implementation.

## Authority and compatibility

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the
sole semantic authority:

- `CppOptions.java:350-430,632-758` defines the FDO values, label converters,
  and both `--copt=-Wno-error` implicit requirements;
- `CoreOptions.java:567-580` defines `collect_code_coverage`;
- `OptionsParserImpl.java:382-430,640-712` records a valued option before
  parsing its implicit child-priority requirements;
- `OptionValueDescription.java:446-493` proves that an explicitly supplied
  default still expands implicit requirements; and
- the pinned 341-row Slug descriptor registry is the source regression for
  names, types, converters, repeatability, old names, and implicit rows.

**Exact:** the admitted joined `--name=value` spellings; bare and joined
boolean plus `--nocollect_code_coverage`; raw direct-occurrence order;
descriptor conversion; main-repository apparent-label mapping; last-wins for
single options; ordered accumulation for `copt`; direct-then-implicit order;
explicit-default implicit expansion; and the resulting structural
configuration facts.

**Slug-native:** structural configuration bytes/identity, Rust diagnostics not
fixed by a discriminator, and compact typed command carriers.

**Unsupported/deferred:** space-separated native values; rc/`--config`,
invocation-policy and mixed-priority expansion; every other native flag;
generic descriptor admission; configuration transitions that write native
options; exact Bazel checksum/output paths; C++ fragment projection; configured
hidden dependencies; and every invocation/action effect.

## Exact admitted closure

Retain one `NativeCommandOption` enum with these thirteen values and no stringly
open set:

1. field inputs: `fdo_optimize`, `xbinary_fdo`, `fdo_profile`,
   `cs_fdo_profile`, `fdo_prefetch_hints`, `propeller_optimize`,
   `memprof_profile`, `proto_profile_path`, and `grte_top` (the producer for the
   Starlark `libc_top` field);
2. suppressors: `fdo_instrument`, `cs_fdo_instrument`, and
   `collect_code_coverage`; and
3. the implicit/repetition closure: `copt`.

`zipper` has no independent command row: the later configuration-field packet
derives it from the admitted FDO state and its typed tools-repository identity.
No rules_cc-specific name or dependency path is introduced.

## Architecture

### Raw command carrier and capture

Extend `CommandConfigurationOccurrence` with one typed native row containing
`NativeCommandOption`, the unconverted joined value when present, and boolean
negation. The existing immutable `Arc<[...]>` overlay remains the only retained
command sequence and participates in its existing equality/hash/serde identity.
Every new retained type derives `Allocative` and serde traits.

`slug_commands_v2::common` recognizes only the thirteen typed names. Nonboolean
options require joined values. `collect_code_coverage` accepts bare, joined
boolean, and no-form spellings and rejects a valued no-form. Build and cquery
already call this common capture before rejecting unsupported flags; do not add
a parallel parser or change their request owners.

### Descriptor-driven preparation

Generalize `PreparedCommandNativeOptions` from native-only values to opaque
complete option-row replacements. It remains phase scratch and publishes no
partial configuration.

For each raw occurrence in order:

1. select the exact pinned descriptor through the typed enum's class/name;
2. validate the occurrence shape against the typed option;
3. convert text/boolean/list rows through the existing native converter and
   label rows through the existing label converter with the supplied root
   `OptionLabelContext`;
4. apply single-option last-wins or append an allow-multiple occurrence to the
   current scratch value; and
5. after the direct row, parse and apply that descriptor's pinned implicit
   requirements at child order through the same typed descriptor path.

Both instrumentation descriptors must obtain `--copt=-Wno-error` from their
registry metadata, not a second semantic table. Explicit null/default values
still expand. Preserve duplicates and exact order in `copt`; do not reuse the
dedupe behavior of registration-pattern flags.

Conversion must process every occurrence in raw order before the one existing
`with_prepared_command_configuration` publication. Errors publish no
configuration. The empty overlay retains the base allocation.

### DICE-owned root mapping

Move native preparation in `slug_analysis_v2::command_configuration` after its
existing optional root-mapping acquisition. Any admitted label-valued native
row requests that same mapping input; Starlark and native rows share one
mapping result, observation epoch, Need union, and error ordering. Root-local
and apparent-external labels use `OptionLabelContext::MainRepository`.

Do not create a native-option DICE key, mapping cache, label side table, or
second publication path. The complete structural configuration remains the
existing command-preparation result and therefore its DICE equality and
invalidation identity.

## Proof contract

Only these exact proof paths are authorized:

- `app/slug_configuration_v2/src/native/tests.rs`, base blob
  `27b61e8a76dafc4b0e3dd78332988dc59ee019ac`;
- `app/slug_commands_v2/tests/commands.rs`, base blob
  `56124af3e9b41f4f0d66649ad5530cddbc9d3e11`; and
- `app/slug_server_v2/src/tests.rs`, base blob
  `af68f3a8801c63ed40813672f366905d53f2a9da`.

Persist compact regressions for:

- build and cquery capture of all thirteen typed rows in exact order, all
  admitted boolean forms, and rejection of missing/malformed joined values;
- all nine direct field inputs plus all three suppressors, root/apparent-
  external label mapping, empty-to-null label behavior, and `grte_top`
  conversion;
- single-value last-wins, ordered repeated `copt`, both instrumentation
  expansions, repeated expansions, explicit-default expansion, and direct/
  implicit precedence in both orders;
- no partial publication on a late bad occurrence, empty-overlay allocation
  reuse, and structural A/B/A bytes;
- one retained daemon A/B/A discriminator using existing `config_setting`/
  selector analysis for representative text, label, boolean, and implicit
  `copt` facts, with no source invalidation; and
- serde/request round-trip retaining the typed overlay and raw order before
  daemon analysis.

The tests must cite the pinned Bazel source behavior above. Do not use a public
raw mutator, a test-only production constructor, or configured-subrule code to
manufacture the states.

## Frozen implementation envelope

Allowed production paths at base `965cfde5e`:

- `app/slug_configuration_v2/src/command.rs`, blob
  `86c5bfd822430ac4ada8a2b52d6918ad1c03a954`;
- `app/slug_configuration_v2/src/lib.rs`, blob
  `2f85f37d7ecd3518eb2b53c8504f49a9258e44a4`;
- `app/slug_configuration_v2/src/native/configuration.rs`, blob
  `f62eabe1132452035599a2ea12f9bcce65da0d77`;
- `app/slug_commands_v2/src/common.rs`, blob
  `dbb0594a3a20e2f2090d74357acf6243b5b9dec3`; and
- `app/slug_analysis_v2/src/command_configuration.rs`, blob
  `99e57d733691fb485aabf2c4a82b06af27739618`.

The registry, native/label converters, build/cquery request owners, loading,
query, configured-rule driver, and every dirty path are read-only inputs.

Caps: 800 production additions, 750 proof additions, 1,550 aggregate additions,
and no new production function above 130 lines. `NativeCommandOption` must fit
in one byte; the occurrence and overlay retained-size assertions must remain
bounded. No benchmark is required without a measured regression.

## Validation and review

Run serially:

1. focused configuration command tests;
2. focused commands capture tests;
3. focused retained-daemon native A/B/A test;
4. full tests for `slug_configuration_v2`, `slug_commands_v2`,
   `slug_analysis_v2`, and `slug_server_v2`;
5. named build/cquery/command-configuration dependents;
6. `cargo check` for the four affected crates;
7. formatting, `git diff --check`, forbidden-surface grep, retained-size checks,
   caps, archive checker, and scheduling consistency; and
8. an index-only archive repeat containing only this packet and excluding all
   parked hunks.

Independent terminal review must check the exact thirteen-row closure,
descriptor-owned implicit expansion, root-mapping/DICE ownership, error and
publication ordering, CLI/daemon A/B/A proof, dirty-hunk isolation, caps, and
the absence of any generic raw mutator.

Stop and `REPLAN` for an untyped/open native name, public or test-only raw
mutator, hard-coded duplicate implicit semantic table, missing copt ordering,
mapping outside the existing DICE owner, partial configuration publication,
new DICE key/cache/side registry, configured-subrule or query work, an edit
outside the envelope, unisolatable parked changes, cap overflow, Java helper/
runtime code, or a rules_cc-specific shortcut.

## Zabel peer guidance

Zabel's separation of raw invocation capture from effective configuration is
useful peer guidance. Slug keeps that separation while using its existing typed
descriptor registry and sole structural option vector. Copy no Zig code,
allowlist, layout, diagnostic, or parity claim. Bazel 9.2 remains authoritative.

## Immediate successor

After terminal acceptance and commit, re-freeze
`WP-4-5-7A-subrule-configured-hidden-dependencies-and-query-r3`. It must consume
only this real structural producer, retain the corrected pre-call boundary and
deferred XML classification from R2, and freeze exact proof paths/base blobs.
