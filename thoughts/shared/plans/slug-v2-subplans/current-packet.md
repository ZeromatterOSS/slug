# Current Slug V2 Packet

Packet: `WP-6-7A-coverage-configuration-field-category-architecture-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 configured fragment,
late-bound dependency, and command-configuration breadth.

Status: R2 architecture independently accepted; commit the design, then
implement only its frozen boundary. R1 independent review returned `REPLAN`
for one public-access distinction. R1 incorrectly
described the coverage facade as shared "just as" the C++ facade without
forbidding C++'s private-caller manifest. R2 requires an unrestricted coverage
value and non-allowlisted ordinary/subrule proofs; every other boundary is
unchanged. Focused R2 rereview returns `ACCEPT`. Base commit `507ae2994`
terminally accepts the complete label file-
admissibility category. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`;
do not edit or stage it.

## Observable result and compatibility classification

Close Bazel 9.2's complete one-field `coverage` configuration-fragment
category rather than admitting only the authentic rules_rust declaration.

Exact admitted behavior:

- `.bzl` `configuration_field(fragment = "coverage", name =
  "output_generator")` produces the same typed late-bound identity family as
  the accepted `cpp` fields. Unknown coverage fields and unknown fragments fail
  at declaration; BUILD files still do not receive the global;
- the field is valid only as a private `attr.label` default under the existing
  rule/subrule restrictions. Symbolic macros, repository rules, tag classes,
  non-label attributes, public attributes, direct invocation overrides, and
  every already-rejected consumer continue failing before publication;
- when `collect_code_coverage` is false the configured field resolves to
  `None`, so no dependency edge is added. When true it resolves to the
  structurally retained `coverage_output_generator` label, whose Bazel default
  is `@bazel_tools//tools/test:lcov_merger` and whose command override observes
  the caller's repository mapping;
- `--coverage_output_generator=<label>` becomes a typed native command option
  beside the already-admitted Boolean `--[no]collect_code_coverage`. Both
  values remain independent structural configuration inputs even while the
  late-bound result is suppressed;
- ordinary and lifted/subrule late-bound dependencies continue through the one
  accepted configured-dependency resolver, including target/exec configuration,
  provider, executable, file-admissibility, FilesToRun, edge, and error order;
- ordinary and subrule `ctx.fragments.coverage.output_generator` expose the
  same optional configured label only when `coverage` was declared in
  `fragments`; undeclared access, inactive evaluator access, and unknown
  members fail under the existing fragment lifecycle. Unlike the restricted
  C++ fragment methods, this public coverage field performs no caller-manifest
  or `BuiltinRestriction` check and works from an ordinary non-allowlisted
  `.bzl` module; and
- configuration-field and fragment projections share one typed coverage-field
  producer. No second label parser, late-bound resolver, option store, DICE
  key, or fragment-specific dependency path is added.

Slug-native behavior is limited to the already-approved structural
configuration identity and Rust-native diagnostic decoration. Exact Bazel
configuration checksum/output-path bytes remain M9.

Unsupported/deferred behavior:

- configured aspect application remains the existing typed deferred boundary;
  this packet neither claims nor approximates Bazel aspect execution. Fixed
  admitted aspect declarations must continue rejecting schemas outside their
  frozen shapes;
- `coverage_report_generator` is a native CoverageOptions label but is not a
  Bazel `@StarlarkConfigurationField`; it remains retained registry input and
  is not exposed as a configuration field;
- coverage instrumentation, filters, report actions, test execution, and
  `coverage_common` behavior are separate categories; and
- other fragment classes/fields remain unsupported rather than being accepted
  through a stringly generic escape hatch.

This is a generic fragment/late-bound/command category. It is not parser
grammar, `set`, a Rust rule implementation, or a `cc_common`/`cc_internal`
branch. Bazel 9 BCR Starlark remains the owner of every rule body, including
`cc_internal`; `cc_common` is only a later consumer.

## Bazel 9.2 authority and evidence

Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole semantic
authority. Pinned source SHA-256 values are:

- `CoverageConfiguration.java`:
  `9f1759880b3e5367d0ffe3fabbb196dc21d837abf0fe00f5e4537a154e1a27c8`;
- `CoverageConfigurationApi.java`:
  `17c68de9d055e4d3afe0b9c255a895fdc7d91621a9c6e6045ab6180e84e69516`;
- `BazelBuildApiGlobals.java`:
  `a54b4657f61846171d0dcaf42e3565e98ee1624316d06f4a47e8c66800fcf897`;
- `StarlarkSubruleTest.java`:
  `b4cad33b5eec81f34d53b17d8f7543d51dedbb41a9a8a5359908afd70e8060e9`;
- `StarlarkRuleImplementationFunctionsTest.java`:
  `89e6caf0c6d234be610ccb597a015610568c27f8071d572e55a7378a106597d8`;
- `AspectTest.java`:
  `d1bf5d2b0e2230a6d3d7b66ea442f09e7f710664706405ea2cd6c4df0d0f6465`;
  and
- builtins `common/cc/semantics.bzl`:
  `08e948c02184e5d3fcd2313ecb46dfa5631a21a2a836c4fe2d5faf148096db0f`.

`CoverageConfiguration` proves there is exactly one annotated field. Its
constructor suppresses the fragment's option view unless CoreOptions
`collectCodeCoverage` is true; `outputGenerator()` then returns the typed
`coverageOutputGenerator` label. The API's public struct field has no
`StarlarkThread` parameter or caller restriction, unlike the private C++
fragment methods. The API and Bazel tests prove optional return, private label-
default use, rule/subrule resolution, and the fragment/member spelling.
`BazelBuildApiGlobals` proves fragment and field validation occurs at
declaration through the registered fragment class and tools repository.

The live Slug registry already owns both CoverageOptions descriptors and the
CoreOptions Boolean in structural configuration identity. The live command
surface already owns `collect_code_coverage`; it deliberately does not yet own
`coverage_output_generator`. The accepted `ConfigurationFieldIdentity`,
ordinary/lifted dependency resolver, and C++ fragment lifecycle are the owners
to generalize, not duplicate.

## Representation and ownership decision

Replace the `#[repr(transparent)] ConfigurationField(CppConfigurationField)`
with a one-byte closed enum over typed fragment fields:

```text
ConfigurationField
  Cpp(CppConfigurationField)
  Coverage(CoverageConfigurationField::OutputGenerator)
```

Keep `ConfigurationFieldIdentity { field, tools_repository }` unchanged. Add
typed `cpp_field() -> Option<_>` and `coverage_field() -> Option<_>` accessors;
no caller may branch on raw fragment/name strings after declaration. Preserve
the one-byte field layout and current compact identity clone/equality/hash.

`SlugConfiguration::configuration_field_label` switches on the typed fragment
field. Coverage reads the existing structural CoreOptions Boolean and
CoverageOptions label record. Refactor any C++-named Boolean error helper into
a generic typed native-Boolean projection rather than routing coverage errors
through `InvalidCppConfiguration`. The label is resolved only through the
existing `OptionLabelContext`; tools-repository identity remains part of the
late-bound producer but is not substituted over an already canonical explicit
option label.

One evaluator-owned `CoverageFragmentValue` exposes `output_generator` and is
shared by root and subrule fragment collections. It contains only the optional
canonical output-generator label: it must not retain the C++ facade's caller
manifest, call `check_default_allowlist`, or otherwise consult
`BuiltinRestriction`. Allocate it only when a root or reachable subrule
declares `coverage`; carry the same unrestricted frozen value through the
existing invocation payload. Do not add a global callback, retained evaluator
value, dynamic fragment map, cache, lock, or DICE key.

## Buck2 and Zabel guidance

starlark-rust remains the parser/evaluator/generated-binder substrate and owns
`set`; no language change is required. Existing compact enums, `Arc`, frozen
evaluator values, and structural configuration vectors are the retained
utility baseline required by the Buck2-reuse policy.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance
only. Its typed `ConfigurationFieldDefinition`/`LateBoundLabel` split supports
keeping declaration identity distinct from configured materialization. Zabel
does not implement Bazel's coverage option producer or establish semantics;
copy none of its Zig code, arena lifetimes, diagnostics, or compatibility
claims.

## Proposed implementation boundary, caps, and proofs

Production allowlist:

- `app/slug_configuration_v2/src/command.rs`;
- `app/slug_configuration_v2/src/native/configuration_field.rs`;
- `app/slug_configuration_v2/src/native/configuration.rs`;
- `app/slug_configuration_v2/src/native/mod.rs` and
  `app/slug_configuration_v2/src/lib.rs` for typed reexports only;
- `app/slug_loading_v2/src/subrule.rs`;
- `app/slug_loading_v2/src/analysis_fragments.rs`;
- `app/slug_loading_v2/src/subrule_invocation.rs`; and
- `app/slug_analysis_v2/src/starlark_rule.rs`.

Proof allowlist:

- inline tests in those production files;
- `app/slug_configuration_v2/src/native/tests.rs`;
- `app/slug_loading_v2/tests/subrule_loading.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`;
- `app/slug_analysis_v2/tests/subrule.rs`; and
- `app/slug_commands_v2/src/common.rs` tests for exact command binding only.

Proposed cap is 520 net / 850 gross production Rust lines, 650 net / 900 gross
proof Rust lines, and 1,750 total gross. No new file is expected. No new or
expanded semantic helper may exceed 150 lines; existing generated method tables
and analysis orchestrators retain their accepted cohesion decisions.

Focused proofs must cover:

1. exact coverage fragment/field acceptance, unknown names/fragments, BUILD
   exclusion, equality/hash/layout, tools-repository A/B/A, and absence of a
   raw-string fallback;
2. false/true/false `collect_code_coverage` resolution to None/default/None,
   explicit output-generator label mapping, invalid/non-visible labels, and
   same-DICE result/error restoration;
3. exact command forms for `--coverage_output_generator`, last-wins structural
   identity, mapping sensitivity, and Boolean interaction;
4. ordinary target and lifted/subrule late-bound dependency resolution,
   including target/exec configuration, provider/file/executable validation,
   omitted edge under false, and one shared resolver;
5. root/subrule `ctx.fragments.coverage.output_generator` from an ordinary
   non-allowlisted `.bzl` module, true-label and false-None projections,
   declaration and invocation lifecycle, inactive-token rejection, absence of
   a caller manifest/restriction, and C++ facade regression;
6. macro/repository/tag/non-label/public/fixed-aspect controls remain closed;
   and
7. authentic rebuilt cquery clears the coverage field and records the next
   generic frontier before any C++-specific branch is considered.

Then run focused configuration/loading/analysis/command proofs, complete
`slug_configuration_v2`, `slug_loading_v2`, `slug_analysis_v2`, and affected
`slug_commands_v2` suites serially; rebuild `slug_cli_v2`; clean `slugd` before
and after authentic replay; run fmt, metadata, archive, diff, cap, and parked-
SHA gates; obtain independent terminal implementation review before commit.

`REPLAN` before adding a second option store, label parser, fragment map,
late-bound resolver, DICE key/cache/lock, accepting another fragment/member,
implementing aspects/coverage actions/instrumentation, touching a ruleset or
starlark-rust, adding a C++ branch, or exceeding a cap. Independent design
review is required before Rust.

## Immediate predecessor

Commit `507ae2994` terminally accepts
`WP-6-7A-label-file-admissibility-category-parity-r1`. Its rebuilt authentic
replay clears `allow_files` and stops at rules_rust
`configuration_field(fragment = "coverage", name = "output_generator")`.
