# Current Slug V2 Packet

Packet: `WP-4-7A-clippy-aspect-attribute-loading`

Milestone: M7A command/ruleset bootstrap closure.

Result: retain the exact ordered 11 private label attributes declared by
rules_rust 0.73.0's `rust_clippy_aspect`, then stop at its still-unadmitted
mixed aspect toolchain list.

## Starting point and fixed sources

Base is `9dea8ee7`. Its docs-only audit authenticates the attribute map as the
next implementation dependency after the corrected clippy route. Both the Slug
worktree and fixed source checkouts must remain clean.

Selected rules_rust 0.73.0:

- `rust/private/clippy.bzl`, SHA-256
  `a778d2ddc77587ffbffc72efcdaa458a1ffae0763e500da1c876b9b567b2a686`;
- relevant source stop: `rust_clippy_aspect` attributes at lines 317-364;
- later boundary: mixed toolchains at lines 370-373.

Behavior authority is clean Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`:

- `StarlarkRuleFunctionsApi.java:835-856` documents aspect `attrs`;
- `StarlarkRuleClassFunctions.java:1284-1297,1345-1446` validates names,
  converts private names, preserves dictionary order, rejects explicit
  configurability, requires implicit defaults and retains built attributes;
- `StarlarkAttrModule.java:357-444,718-766` owns defining-module label
  conversion, executable/cfg and file-allowance construction;
- `StarlarkRuleClassFunctionsTest.testAspectExtraDeps`,
  `testAspectNoDefaultValueAttribute`, `testAspectParameterBadType`,
  `testAspectCannotSetConfigurableOnAttr`,
  `testAttrAllowedSingleFileTypesWrongType` and
  `testAttrSingleFileWithList` are the focused regressions.

## Exact source contract

Retain these Starlark names and canonical defining-repository defaults in
dictionary order:

| Name | Default target | Additional retained state |
|------|----------------|---------------------------|
| `_capture_output` | `//rust/settings:capture_clippy_output` | none |
| `_clippy_error_format` | `//rust/settings:clippy_error_format` | none |
| `_clippy_flag` | `//rust/settings:clippy_flag` | none |
| `_clippy_flags` | `//rust/settings:clippy_flags` | none |
| `_clippy_output_diagnostics` | `//rust/settings:clippy_output_diagnostics` | none |
| `_config` | `//rust/settings:clippy.toml` | `allow_single_file=True` |
| `_error_format` | `//rust/settings:error_format` | none |
| `_extra_rustc_flag` | `//rust/settings:extra_rustc_flag` | none |
| `_incompatible_change_clippy_error_format` | `//rust/settings:incompatible_change_clippy_error_format` | none |
| `_per_crate_rustc_flag` | `//rust/settings:per_crate_rustc_flag` | none |
| `_process_wrapper` | `//util/process_wrapper` | `executable=True`, `cfg="exec"` |

Every row is `attr.label`, nonmandatory, has omitted configurability, disallows
ordinary files, has no provider predicate, attached aspect, allowed values or
custom transition, and owns a concrete label default. All rows except
`_config` have no single-file allowance. All rows except `_process_wrapper`
are nonexecutable and do not use the exec configuration.

## Decision and ownership

Extend `aspect_attributes` with one exact source gate beside the existing
rustfmt pair. Validate the complete ordered shape and every retained field,
then call the existing `declared_attribute_schema` for each row. Do not add a
parallel aspect schema, change ordinary rule lowering, or create a broad
private-attribute compatibility claim.

`AttributeDefinition` remains the evaluator-local producer;
`declared_attribute_schema` remains the detachment boundary; and
`AspectDefinitionGen.attributes` plus its frozen form remain the retained
owner. The existing immutable `Arc<[RuleAttributeSchema]>` representation,
equality, freezing and memory lifetime are unchanged. No DICE key, request
overlay, analysis consumer, cache, fallback or asynchronous lifetime changes.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` guides only this shared
`NamedAttribute`/`AttrDefinition` ownership and evaluator-detached retention.
Copy no Zig code, layout, diagnostics or algorithm. Bazel 9.2 remains the sole
behavior authority. Because retained representation, hashing, collections,
clone cost and accounting do not change, no Buck2 utility or Stage 9 ledger
work is selected.

Compatibility classification:

- **Exact:** the listed Bazel private-label default, file, executable,
  exec-configuration, configurability and order semantics.
- **Slug-native:** retain Starlark `_name` spelling and existing Rust canonical
  labels/immutable schema rather than Bazel's internal `$name` spelling.
- **Unsupported/deferred:** every other aspect attribute map; public
  parameters; label lists and other private kinds; absent/`None`, computed,
  late-bound, materializing or dormant defaults; explicit configurability;
  provider/aspect predicates; custom transitions; other file allowance shapes;
  configured aspect execution; and the mixed toolchain list/complete clippy
  aspect.

## Allowlist, proof and caps

Only these files may change:

- `app/slug_loading_v2/src/package.rs`, base 6,142 lines, SHA-256
  `974990551b1d717106c24e37237ef2e1910cf5a64207e659cbec910ac478ee8f`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`, base 6,575 lines,
  SHA-256
  `9bc0a07c319b34e8f6b9089415978700d1831e86b3a996948e015e96f05c8ce0`.

Caps are 110 production, 160 proof and 270 total additions; physical ceilings
are 6,255 and 6,735 lines. `package.rs` exceeds the complexity trigger, but the
change stays in its existing cohesive declaration-validation owner and may not
add a second representation or cross-owner helper.

Required proof:

1. Freeze a source-shaped exact clippy map with the later toolchain list
   omitted or reduced to the already-admitted singleton string. Assert all 11
   names, canonical defaults and flags in order, and prove the implementation
   stays lazy.
2. Preserve the existing rustfmt aspect proof unchanged.
3. Replace fixed-pair negative assertions that Bazel admits with mutation
   cases discriminating missing/reordered/extra rows, wrong defaults, missing
   defaults, public label parameters, explicit configurability, wrong kinds,
   file/provider/aspect/transition additions and executable/exec mismatches.
4. Prove the unchanged source-shaped mixed String/typed-requirement list still
   terminates at the aspect toolchain boundary. Do not claim the complete
   clippy aspect or run its implementation.

The test source is a focused extract of the pinned rules_rust file, not a new
oracle fixture. Existing Bazel tests cover the constructor contract; no copied
workspace or fixture growth is authorized.

## Validation and STOP

Run serially with
`CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1`:

- focused new and rustfmt aspect tests;
- `cargo test -p slug_loading_v2 --lib --locked`;
- `cargo test -p slug_loading_v2 --test bzl_invalidation --locked`;
- `cargo test -p slug_loading_v2 --test build_file_loading --locked`;
- `cargo check -p slug_loading_v2 --locked`;
- `cargo fmt --all -- --check` and `git diff --check`.

Independent terminal review is required because `package.rs` is above the
complexity trigger and the source gate changes retained aspect declarations.

STOP and `REPLAN` for dirty authority; a changed retained representation or
DICE/analysis owner; a need to widen `attr.label`; ordinary-rule regression;
an unbounded aspect API; copied Zabel behavior; Java/JVM work; toolchain
parsing; complete-clippy claims; an additional file; or a cap violation.

## Immediate predecessor

`WP-4-7A-clippy-aspect-attribute-audit` found the existing schema complete for
this source subset and received independent bounded-implementation approval.
The earlier toolchain candidate remains fully reverted.
