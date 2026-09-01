# Current Slug V2 Packet

Packet: `WP-6-7A-cpp-configuration-field-catalog-completion-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 configured fragment,
late-bound dependency, and command-configuration breadth.

Status: independent design review returned `ACCEPT`; implement only the frozen
boundary below.
Base commit `cb477b7ab` terminally accepts the complete coverage configuration-
field category. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Complete Bazel 9.2's finite `cpp` configuration-field catalog. Slug already
admits ten of the eleven annotated fields; `custom_malloc` is the sole missing
member. This packet adds that one member through the existing typed catalog and
thereby closes the category rather than patching only the authentic
rules_rust declaration.

Exact admitted behavior:

- `.bzl` `configuration_field(fragment = "cpp", name = "custom_malloc")`
  produces the same typed late-bound identity family as the ten accepted C++
  fields. The complete accepted C++ set is `zipper`, `custom_malloc`,
  `fdo_optimize`, `fdo_prefetch_hints`, `fdo_profile`, `cs_fdo_profile`,
  `propeller_optimize`, `xbinary_fdo`, `memprof_profile`, `libc_top`, and
  `proto_profile_path`; every other name remains rejected at declaration;
- the field is valid only as a private `attr.label` default under the already-
  accepted ordinary/subrule restrictions. Default `None` adds no edge; an
  explicit `--custom_malloc=<label>` supplies the configured dependency through
  the existing target/Exec, provider, file, executable, edge, and error-order
  machinery;
- `--custom_malloc=<label>` becomes a typed native command option over the
  already-retained CppOptions descriptor, with repository mapping, last-wins,
  structural identity, and same-DICE A/B/A restoration. Joined value is
  required. The generic command parser rejects `--nocustom_malloc` and every
  other admitted non-Boolean native no-form with Bazel's illegal-prefix
  diagnostic instead of silently treating it as an unrelated compatible flag;
- ordinary and subrule `ctx.fragments.cpp.custom_malloc` return the configured
  Label or `None` when `cpp` was declared. This field is public and performs no
  caller-manifest or `BuiltinRestriction` check. Existing private C++ fragment
  methods remain restricted, so one non-allowlisted module can read
  `custom_malloc` while still being denied `compilation_mode()`; and
- field default and fragment facade project the same structural `custom_malloc`
  option. No second label parser, option store, resolver, fragment object,
  command overlay, DICE key, cache, or lock is added.

Slug-native behavior remains limited to the accepted structural configuration
identity and Rust-native diagnostic decoration. Exact Bazel configuration
checksum/output-path bytes remain M9.

Unsupported/deferred behavior:

- custom-malloc rule semantics, `CcInfo` production, link selection, malloc
  precedence, action generation, and execution remain later BCR Starlark and
  configured-action categories;
- other fragment catalogs remain unsupported rather than accepted through a
  stringly generic fallback; and
- configured aspect application retains its existing typed deferred boundary.

This is finite generic configuration-field and fragment projection work. It is
not parser grammar, `set`, a Rust rule implementation, or a `cc_common` /
`cc_internal` special case. Bazel 9 BCR Starlark remains the owner of all rule
bodies, including C++ rules; rules_rust is only the authentic consumer that
selected the next catalog member.

## Bazel 9.2 authority and evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic
authority. Pinned source SHA-256 values are:

- `CppConfiguration.java`:
  `d0c0fae644272fa8992e44461e8ec2f6655681e370b41594954f4f8f1adf9a71`;
- `CppConfigurationApi.java`:
  `57c88f6f17764f56974b083494cf3716aa9cbe2fd1aa9f9ee4ba24b6365cf841`;
- `CppOptions.java`:
  `ac9f2f4c4e1bcacc4066791b5d8f264b9ee5434477dfa90c1492ab8c8317c7a1`;
- `StarlarkIntegrationTest.java`:
  `ced8fc27cbe35bf30174678800d29b73012f800bff00bcdff6a5cf8c78fef836`;
  and
- `OptionsParserImpl.java`:
  `4bc80c745cad2b427b6f42b28c1fd75a0d121b94681ff20d11393d24413e8652`.

`CppConfiguration` contains exactly eleven `@StarlarkConfigurationField`
annotations. Comparing that closed source inventory to Slug's typed enum proves
`custom_malloc` is the only missing member. Its method directly returns the
nullable CppOptions label. `CppOptions` declares `--custom_malloc` as a nullable
`LabelConverter` option that changes inputs and outputs.

`CppConfigurationApi.customMalloc` is a public nullable struct field with no
Starlark thread parameter or allowlist check. This differs from the accepted
private C++ methods and requires a mixed-visibility facade, not a relaxation of
the entire object. A fresh Bazel 9.2 oracle from an ordinary non-builtin `.bzl`
module reports `None` by default and `@@//:malloc` under
`--custom_malloc=//:malloc`, with no allowlist error. The pinned integration
test separately proves the late-bound private attribute resolves to `None` or
the configured dependency and enforces its `CcInfo` provider schema.

`OptionsParserImpl` proves a `no` prefix on a known non-Boolean option is an
error. A fresh Bazel 9.2 oracle reports exactly
`Illegal use of 'no' prefix on non-boolean option: --nocustom_malloc`.

The authenticated rules_rust 0.73.0 `rust/private/rust.bzl`, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`,
declares private `_custom_malloc` at lines 960-966 with this field and a
`[[CcInfo]]` provider constraint. Rebuilt Slug cquery currently stops at that
exact declaration before any rule implementation, `cc_common`, or
`cc_internal` behavior runs.

## Representation and ownership decision

Add `CustomMalloc` to `CppConfigurationField` and its flattened
`ConfigurationField::CppCustomMalloc` discriminant. Preserve the one-byte
`#[repr(u8)]` field, typed fragment accessors, and unchanged
`ConfigurationFieldIdentity { field, tools_repository }`. The closed enum,
not a string pair or dynamic registry, remains the authority after declaration.

Map the new field in `SlugConfiguration::configuration_field_label` to the
existing CppOptions `custom_malloc` label using the already-general
`native_label` projection. Add `NativeCommandOption::CustomMalloc` through the
existing mapping-aware command descriptor path. Extend the command parser's
known `no`-prefix branch once, generically, so all admitted non-Boolean native
options reject before configuration publication.

Keep `CppFragmentProjection` as a cheap phase-scratch view of the sole
`SlugConfiguration`; do not duplicate the label into a second retained field.
Expose a typed optional-label projection and allocate the evaluator-local
Starlark Label only at facade access. Add `custom_malloc` to
`CppFragmentValue` without calling its private-method `check`; all existing
methods continue calling that check. Root and subrule collections already share
the same frozen C++ value and token lifecycle, so neither transport nor analysis
orchestration changes.

## Buck2 and Zabel guidance

starlark-rust remains the parser/evaluator/generated-binder substrate and owns
`set`; no language change is required. Existing compact enums, frozen values,
structural native option vectors, and label conversion are the retained
Buck2-derived utility baseline. No new collection, interner, hash, or memory-
accounting owner is justified.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance
only. Its captured typed configuration-field identity and mixed C++ fragment
projection support keeping declaration identity, configured label ownership,
and public `custom_malloc` access separate. Zabel supplies no semantic or
source-order authority; copy none of its Zig code, arena lifetimes, diagnostics,
or compatibility claims.

## Proposed implementation boundary, caps, and proofs

Production allowlist:

- `app/slug_configuration_v2/src/command.rs`;
- `app/slug_configuration_v2/src/native/configuration_field.rs`;
- `app/slug_configuration_v2/src/native/configuration.rs`;
- `app/slug_configuration_v2/src/native/cpp_fragment.rs`;
- `app/slug_commands_v2/src/common.rs` for generic known-native no-prefix
  rejection; and
- `app/slug_loading_v2/src/analysis_fragments.rs` for the public facade field.

Proof allowlist:

- inline tests in those production files;
- `app/slug_configuration_v2/src/native/tests.rs`;
- `app/slug_loading_v2/tests/subrule_loading.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`;
- `app/slug_analysis_v2/tests/subrule.rs`; and
- `app/slug_commands_v2/src/common.rs` tests.

Proposed cap is 180 net / 300 gross production Rust lines, 450 net / 650 gross
proof Rust lines, and 950 total gross. No new file is expected. No new or
expanded semantic helper may exceed 120 lines.

Focused proofs must cover:

1. exact eleven-member C++ field inventory, unknown-field rejection, one-byte
   layout, typed equality/hash, and tools-repository A/B/A;
2. default-None / explicit-label / default-None restoration through the sole
   field resolver, repository mapping, non-visible labels, provider validation,
   omitted/default edge behavior, and configured target dependency identity;
3. exact joined `--custom_malloc`, last-wins structural identity and mapping,
   plus generic rejection of `--nocustom_malloc`,
   `--nocoverage_output_generator`, and one previously admitted non-Boolean
   option while Boolean no-forms remain valid;
4. ordinary and subrule `ctx.fragments.cpp.custom_malloc` Label/None access from
   a non-allowlisted `.bzl`, including declaration/invocation lifecycle and
   same-DICE A/B/A;
5. the same non-allowlisted caller remains denied an existing private C++
   method, proving field-specific public access rather than facade widening;
6. macro/repository/tag/non-label/public/fixed-aspect controls remain closed and
   the existing ten configuration fields regress; and
7. rebuilt authentic cquery clears `custom_malloc` and records the next generic
   frontier before any rule-body or C++ builtin special case.

Then run focused configuration/loading/analysis/command proofs and complete
`slug_configuration_v2`, `slug_loading_v2`, `slug_analysis_v2`, and affected
`slug_commands_v2` suites serially; rebuild `slug_cli_v2`; clean `slugd` before
and after authentic replay; run fmt, metadata, archive, diff, cap, pinned-source,
clean-Zabel, and parked-SHA gates; obtain independent terminal implementation
review before commit.

`REPLAN` before adding a raw field-name fallback, second option/label owner,
resolver, fragment value, caller manifest, DICE key/cache/lock, accepting
another fragment, implementing custom-malloc/C++ rule semantics, touching a
ruleset or starlark-rust, or exceeding a cap.

Independent design review returned `ACCEPT`. It verified the exact eleven-to-
ten inventory comparison, mixed public/restricted facade, generic native no-
prefix correction, one-byte and single-owner reuse, compatibility boundaries,
allowlists, caps, proofs, and stops.

## Immediate predecessor

Commit `cb477b7ab` terminally accepts
`WP-6-7A-coverage-configuration-field-category-implementation-r3`. Its rebuilt
authentic replay clears coverage and stops at the sole missing C++
configuration-field catalog member, `cpp.custom_malloc`.
