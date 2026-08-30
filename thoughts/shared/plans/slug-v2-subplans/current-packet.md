# Current Slug V2 Packet

Packet: `WP-4-5-7A-exec-configured-label-attribute-loading-design`

Milestone: M7A category 6 registered-toolchain prerequisite correction.

Base: accepted Host-capability implementation `26a68d61c` and activated
proof-only registration packet `20ad71ffa`. The passing row-3 proof draft and
retained selected-context R2 candidate remain dirty and must not be edited by
this docs-only packet.

## Why this design is active

The proof-only registration packet authenticates all four exact
`@bazel_tools` toolchain declarations and generically realizes the non-Windows
`local_config_winsdk` row as an empty generated package. Its focused loading
test passes, including warm reuse and a Host-platform transition.

The first real command/REAPI dependent then fails at registration row 1 while
loading verbatim `@@bazel_tools//tools/launcher:BUILD`. The package invokes
`single_binary_toolchain(exec_binary = ...)`; exact
`//tools:build_defs.bzl` selects a rule whose `binary` attribute is
`attr.label(cfg = "exec", allow_single_file = True, mandatory = True)`.
Slug currently rejects every target invocation whose declaration is executable
or exec-configured before recording the target.

This is not a winsdk, ruleset, parser, builtin, selected-context, action, or
REAPI defect. Fixing it in the proof-only packet would violate that packet's
zero-production stop. Design one generic loading/analysis boundary first.

## Authority and compatibility

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole semantic authority. `StarlarkAttrModule.label` requires a `cfg` when
`executable=True`; `cfg="exec"` installs Bazel's execution transition on the
attribute; package loading retains that declaration and its raw label value;
configuration and executable-provider projection happen later during analysis
through the configured prerequisite and `FilesToRunProvider` owners.

The verbatim BCR files
`@bazel_tools//tools:build_defs.bzl` and
`@bazel_tools//tools/launcher:BUILD` are the discriminating real consumer.
Their first non-Windows exec-configured invocation is source line 72. No BCR
content may be altered or special-cased.

Live Slug already retains `executable` and `exec_configuration` in frozen
`RuleAttributeSchemaGen`, but the public immutable `AttributeSchema` projection
drops both and `FrozenRuleDefinition::reject_deferred_attribute_invocation`
rejects them at package invocation. Configured analysis therefore has no typed
way to fail closed after loading. That ownership split, not Starlark parsing,
is the missing seam.

- **Exact:** valid Bazel 9 label-attribute declaration validation; package
  target creation; retained target/exec/custom-transition distinction;
  executable-bit retention; unconfigured dependency/query topology; and
  launcher package inventory needed by registration expansion.
- **Slug-native:** compact Rust retained representation, structural DICE
  equality/invalidation, and the diagnostic used when configured analysis
  reaches an admitted loading-only exec/executable attribute.
- **Unsupported/deferred:** configured exec-edge traversal, executable
  prerequisite/`ctx.executable`/FilesToRun projection, custom transitions on
  executable attributes, and analysis of a target that actually owns one of
  these attributes. Those targets must fail closed during analysis, not loading.

BCR Starlark owns the rule and macro control flow, including `cc_internal`;
`cc_common` remains only a generic Host/provider ABI consumer. Clean
`../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is architecture
and optimization guidance only, never behavior authority.

## Frozen architecture

`app/slug_loading_v2/src/attrs.rs` owns the immutable loading ABI. Replace the
ambiguous optional-transition-only projection with one compact sealed
dependency-configuration value:

- `Target` for ordinary attributes;
- `Exec` for `cfg="exec"`; and
- `Starlark(TransitionDefinition)` for the already-admitted custom transition.

Retain the executable bit separately. Constructors default to `Target`; one
crate-owned constructor/projector admits exactly one of Exec or Starlark.
Expose read-only accessors. Both fields participate automatically in immutable
attribute/package/DICE equality. Add no side table, interner, cache, string
token, or analysis-owned reconstruction.

`app/slug_loading_v2/src/package.rs` remains the declaration/projector owner.
Project the already-validated frozen flags into `AttributeSchema` and stop
rejecting a target merely because its declaration is Exec or executable.
Preserve all definition-time validation, allow-file/single-file coercion,
ordinary dependency extraction, transition syntax, provider/aspect and
fragment stops. Package loading and registration inventory do not configure or
analyze the dependency.

`app/slug_analysis_v2/src/dice.rs` owns the configured boundary. Before
selector/dependency computation or Starlark rule invocation, inspect the
retained schema. If an attribute is Exec or executable, return one typed
semantic unsupported diagnostic naming the attribute and requested target.
This prevents silent target-configuration analysis while allowing unrelated
package inventory and custom toolchain selection to proceed. The later exact
analysis packet can consume the same enum without changing loading identity.

No lock may cross DICE compute/await. The new enum/bit live in the existing
Arc-backed package value; no new retained collection or material copy is
authorized. This is not a demonstrated hot path and needs no benchmark.

## Exact future implementation allowlist, blobs, and caps

After independent design `ACCEPT`, implementation may change only these live
baseline blobs:

- `app/slug_loading_v2/src/attrs.rs`
  `ecb7ea40cd781a5f924599a8fee0994d69e208f0`;
- `app/slug_loading_v2/src/package.rs`
  `d2b495fc31bb9d95231fa2bc21a740c7dd78686e`;
- `app/slug_analysis_v2/src/dice.rs`
  `fcc89b578d5a6cf7887a5ea31adca528b0279782`;
- `app/slug_loading_v2/tests/build_file_loading.rs`
  `7b1e2a98a54b8fa49ce4bda3c32c6d819f0771c4`; and
- `app/slug_analysis_v2/tests/starlark_rule.rs`
  `d4ee2e6f47aacfff39d969e140b75b38a79b9c24`.

The latter four hashes deliberately include the retained selected-context R2
candidate where applicable. Preserve those bytes and stage only prerequisite
hunks. Maximum additions: 220 production Rust, 350 proof Rust, 570 aggregate.
No registration proof, core, REAPI, catalog, fixture, Cargo, action, provider,
selected-context, command, or other file may change in implementation.

The parked proof draft is currently isolated in
`registration_expansion_tests.rs`, `build_command_tests.rs`, `reapi.rs`, and
this packet's prior manifest correction. It is not implementation authority;
after the prerequisite passes, reactivate the proof packet against those exact
live bytes.

## Discriminating proof and validation

Loading proof must distinguish Target, Exec, and Starlark transition identity;
retain executable separately; prove A/B/A invalidation; preserve declaration
defaults/coercion, canonical label, ordinary dependency and query shape; accept
the exact launcher `binary` schema and target invocation; and retain existing
invalid `executable=True` without cfg plus malformed cfg diagnostics.

Analysis proof must show a loaded Exec attribute and a valid executable
attribute each fail only when that target is configured/analyzed, before child
configuration, provider lookup, Starlark invocation, action, or publication.
An unrelated target in the same package must still analyze and warm-reuse.

Run focused loading and analysis tests, full serial `slug_loading_v2`, direct
`slug_analysis_v2`, the parked registration-row test, and the previously red
REAPI test. Then run `cargo fmt --all`, `git diff --check`, exact blob/scope/
cap/dirty-isolation audits and `scripts/v2_archive_status.sh`.

## Stops and successor

`REPLAN` for a launcher/winsdk/ruleset special case; package-time configuration
or analysis; flags dropped from retained identity; analysis silently treating
Exec as Target; executable provider fabrication; custom-transition widening;
new cache/interner/dependency; production change outside the three owners;
proof change outside the two test owners; cap breach; or dirty-candidate
overlap that cannot be isolated.

After independent design `ACCEPT`, implement only the frozen five-file packet.
After its terminal `ACCEPT`, resume the parked proof-only four-registration-row
closure; only then may selected-context R2 return to terminal review.
