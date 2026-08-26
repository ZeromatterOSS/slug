# Current Slug V2 Packet

Packet: `WP-4-7A-lint-test-label-default-loading-r2`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned attribute definition, defining external-Bzl identity and frozen rule schema
Base: `2936c570`

Result: complete declaration-time loading and freezing of the fixed
`LINT_TEST_COMMON_ATTRS` dictionary at accepted rules_rust 0.73.0
`rust/private/lint_test.bzl:45-62`. Resolve its raw `@bazel_tools` scalar label
default through the innermost defining module's immutable repository mapping,
preserve its already-constructed relative `Label` default as canonical typed
identity, and prove both values by composing accepted selected-route mapping
evidence with caller-aware external-module freeze. Do not invoke a target,
apply the transition, or broaden the later rustfmt aspect.

## Accepted starting point and first absent fact

Commit `129ff448` completes and freezes
`rust/private/rust_analyzer.bzl` through line 484. Commit `e71db43e` selects the
post-file docs audit. Slug's recursive external-Bzl driver computes resolved
children serially in source order and stops on the first child failure. The
authenticated return path is:

1. `rust/toolchain.bzl:11-14` next loads
   `//rust/private:rustfmt.bzl`.
2. `rust/private/rustfmt.bzl:3-11` first loads `common.bzl`, then
   `lint_test.bzl`.
3. `common.bzl` is already complete from the accepted rust-analyzer closure;
   the first new child is therefore `rust/private/lint_test.bzl`.
4. The function bodies at lines 15-35 remain lazy. The fixed transition at
   lines 37-41, `platform` descriptor at lines 46-48 and boolean default at
   lines 49-52 already construct.
5. The first unsupported expression is the raw string default at lines 53-55:
   `@bazel_tools//tools/allowlists/function_transition_allowlist`.

`attribute_definition` currently converts every non-`None` default through
`raw_attribute_value`, then calls `coerce_raw_value` with only the defining
package path. The label branch consequently rejects every `@...` string and
cannot consult the already-retained defining-module mapping. The immediately
adjacent `_runner` at lines 56-60 supplies
`Label("//rust/private/lint_test_runner")`; the same raw adapter rejects this
typed value. A raw-string-only fix would stop one expression later and would
not complete the newly selected module, so this packet admits exactly both
scalar label-default forms through one existing owner.

The exact frozen results for the admitted defining identity are:

- `_allowlist_function_transition` defaults to
  `@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`;
- `_runner` defaults to
  `@@dep+//rust/private/lint_test_runner:lint_test_runner` even though the
  root apparent dependency is `dep_alias` and the defining module's self-name
  is `rules_rust`; and
- `_runner` retains executable true plus the accepted exec-configuration
  marker. No implementation function executes.

## Learned proof-harness boundary

The first implementation proof found that the selected loading fixture is not
a valid direct discriminator for the built-in spelling. Its synthetic root is
declared as `module(name='bazel_tools')`, so the selected dependency's implicit
`bazel_tools` dependency resolves to that root rather than canonical
`@@bazel_tools`. Renaming the synthetic root to an ordinary name activates the
complete pinned built-in MODULE dependency closure and fails first on absent
`rules_license` registry evidence. An explicit local override separately hits
Slug's existing `ExplicitBuiltinOverride` unsupported boundary. Growing either
path would add unrelated fixture/source breadth and is forbidden here.

This is not a production mapping absence. `NonrootModuleBuilder::build`
already injects the singleton built-in dependency after user declarations;
selected graph transformation retains it; `HostSelectedRepositoryMapping`
owns it in the existing ordered `SmallMap`; and the accepted
`selected_definition_source_is_request_owned_and_route_structural` Bzlmod test
proves a selected non-root route maps `bazel_tools` to the built-in snapshot.
The route projection carries that same mapping Arc into `BzlModuleIdentity`,
where it already participates in equality/hash and manifest fingerprinting.

The corrected proof is compositional and adds no semantic owner:

- reuse the accepted Bzlmod selected-route/built-in-child regression;
- keep the synthetic selected loading route only for the distinct
  `rules_rust -> dep+` producer identity and typed Label preservation; and
- evaluate/freeze the exact fixed lint-test dictionary in a focused
  caller-aware `BzlLoadManifest` whose defining identity contains the admitted
  `bazel_tools -> bazel_tools` mapping.

Do not mutate the synthetic root, add registry responses, admit explicit
built-in overrides, or change selected mapping construction in this packet.

## Second implementation stop and canonical target correction

The corrected proof reached the typed runner assertion and exposed a second
material contract error before any Rust was retained. The accepted source
spelling is `Label("//rust/private/lint_test_runner")`, with no colon. At the
pinned Bazel 9.2 commit, `LabelValidator.parseAbsoluteLabel` assigns the whole
post-`//` spelling to the package and derives the implicit target from its last
path segment. `LabelParserTest.parserTable` directly authenticates
`//foo/bar` as package `foo/bar`, target `bar`. The exact runner identity is
therefore:

```text
@@dep+//rust/private/lint_test_runner:lint_test_runner
```

It is not `@@dep+//rust/private:lint_test_runner`; that spelling would require
an explicit colon before `lint_test_runner` in the source. Pinned Zabel's
`generic_label.zig` likewise keeps package path and target name as distinct
retained identity parts and derives the no-colon raw name from the final path
segment. That is architectural guidance for the producer-owned identity split
only; pinned Bazel remains the sole behavior authority.

Because this is the second material implementation/contract correction, the
prior packet stops with `REPLAN`. This `-r2` packet preserves the already
reviewed owner, proof composition, implementation allowlist and caps while
correcting every expected runner identity. No Rust from either stopped attempt
is retained.

## Authorities and compatibility classification

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned Git objects and the accepted archive, not current sibling
HEADs.

The minimum authenticated Bazel chain is:

- `StarlarkAttrModule.createAttribute` passes the default and
  `LabelConverter.forBzlEvaluatingThread(thread)` to the attribute builder;
- `Attribute.Builder.defaultValue` delegates typed conversion to the
  attribute type;
- `BuildType.LabelType.convert` returns an input `Label` unchanged, converts a
  string with the supplied `LabelConverter`, and rejects other value types;
- `LabelConverter.forBzlEvaluatingThread` selects the innermost defining Bzl
  module's package context/repository mapping and delegates to
  `Label.parseWithPackageContext`; and
- focused `StarlarkRuleClassFunctionsTest`, `StarlarkIntegrationTest` and
  `BzlLoadFunctionTest` cases authenticate declaration-time string conversion,
  retention of Label inputs, remote label defaults and defining-module Bzlmod
  mappings. Later repository visibility and target lookup are separate.

Compatibility is classified as follows:

- **Exact:** scalar `attr.label(default = <string>)` resolution through the
  innermost defining `.bzl` identity; scalar `attr.label(default = <Label>)`
  retention without re-resolution; the two fixed canonical defaults, their
  schema fields, recursive freeze/export and lazy implementations in
  `lint_test.bzl:37-62`.
- **Slug-native:** existing `CanonicalLabel`, `CoercedAttributeValue`, Arc and
  frozen Rust representation; complete repository-mapping
  over-invalidation/fingerprint framing; and nonrequired diagnostic text.
- **Unsupported/deferred:** label-list/dict defaults, computed or late-bound
  defaults, `configuration_field`, raw canonical-string breadth, target
  lookup/invocation, executable prerequisite validation, transition
  allowlisting/application, configured dependencies, providers, toolchains,
  rustfmt aspect `required_providers`/`fragments`, analysis/actions, M8/M7B and
  exact Bazel configuration/output identity.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_invocation_capture.zig` declared-default and captured-Label paths
reinforce one producer-owned typed default: rebase/resolve string spelling in
the defining `.bzl` module, preserve an existing canonical Label value, and do
not make the consuming BUILD package repair identity. No Zig code,
representation, label or mapping behavior, evaluator rule, cache or DICE
relation may be copied. Pinned Bazel 9.2 remains sole behavior authority.

## Implementation boundary

Change only the loading-owned attribute-default path:

1. Reuse `BzlEvaluationContext::source_identity_for_call(eval)` to retain the
   complete innermost `BzlModuleIdentity`, rather than reducing it to a source
   label/package before coercion.
2. For `AttributeKind::Label` only, preserve the accepted `None` behavior,
   resolve a scalar string with the shared pure `resolve_label(raw, source)`,
   or clone the canonical identity from an actual `StarlarkLabel` value.
3. Store either result in the existing owned
   `CoercedAttributeValue::Label`. Every other attribute kind and raw default
   follows its existing `raw_attribute_value`/`coerce_raw_value` path.
4. Do not teach `raw_attribute_value` about Starlark values generally, do not
   add external-label behavior to package-local coercion, and do not
   reconstruct or re-resolve a `StarlarkLabel` from its display string.

The defining module identity and its immutable repository mapping are already
inputs to recursive Bzl identity, equality/hash and manifest fingerprinting.
The declaration stores an owned canonical label and freeze transfers it into
the existing frozen module/rule-schema lifetime. No new DICE key, dependency,
revision source, lookup, I/O, cache, map, interner, collection, hash domain,
lock or lifetime owner is permitted. The Buck2-derived utility audit therefore
selects the existing identity, mapping, enum and Arc owners; no retained
representation changes.

## Discriminating proof

Use two loading proofs without growing the existing 142-line selected route
test:

- retain root apparent name `dep_alias`, defining-module self-name
  `rules_rust`, and canonical repository `dep+` as three distinct identities,
  then freeze a typed `Label("//rust/private/lint_test_runner")` default through
  that recursive producer route;
- in a focused external-module evaluator, construct an exact defining
  `BzlModuleIdentity` plus `BzlLoadManifest` with canonical repo `dep+` and
  the admitted `bazel_tools -> bazel_tools` mapping, evaluate the fixed
  transition/four-entry `LINT_TEST_COMMON_ATTRS`, project the dictionary into
  one test-only frozen rule, and inspect both exact canonical defaults,
  `_runner` executable/exec markers, the boolean default and lazy bodies;
- add a focused absent/conflicting apparent-mapping rejection through the raw
  scalar attribute-default converter before module freeze.

The proof must discriminate preservation from stringification: the runner
Label is created in the defining module and reaches the schema as
`@@dep+//rust/private/lint_test_runner:lint_test_runner`; do not pass
`str(Label(...))` to the attribute default. Reuse the accepted Bzlmod test
`selected_definition_source_is_request_owned_and_route_structural` as the
route-to-built-in mapping proof. Add no fixture, registry response, network
request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `2936c570`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `cc669d2158c036d20bd92bcff61ee929d65b317e3a85883fbcbe14238b53d9b0` | 5,492 | 5,532 | bounded scalar label-default coercion in the existing owner |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `b83d263c05f1933d8a82e3d3b38b22f5d58d343acea4fb0aedb77522983faf13` | 4,546 | 4,646 | selected-route frozen-schema and fail-closed proofs |

Additions are capped at 40 production lines, 100 proof lines and 140 total
lines. Deletions do not buy addition budget. No touched function may exceed
150 lines. Keep the existing selected route test at or below its current 142
lines by placing new setup/assertions in the already-extracted fixture/helper
or a new focused helper/test.

## Serial validation and review

Run Cargo commands serially with one shared target directory:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 root_package_loads_selected_registry_bzl_through_admitted_route
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 label_attribute_defaults_keep_defining_module_identity
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 label_attribute_default_rejects_unadmitted_apparent_mapping
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2 selected_definition_source_is_request_owned_and_route_structural
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The archive checker may report only its known three retained thoughts paths
plus active packet files. Recheck the two file hashes against base before
implementation and count additions/physical lines/function lengths before
review.

Independent terminal review is mandatory before commit. The reviewer must
inspect the full base-to-worktree diff and explicitly verify source-order
selection, the Bazel string-versus-Label distinction, defining-module mapping,
the synthetic-root proof boundary, accepted Bzlmod built-in-route composition,
exact canonical values, no Label re-resolution, missing/conflicting failure,
lazy implementation boundary, file/function/addition caps, serial validation
and absence of a new semantic owner. A plain `ACCEPT` or actionable `REJECT` is
required; correct every rejection and re-review.

## STOP / `REPLAN`

STOP and `REPLAN` if completion requires any file outside the allowlist; a
label-list/dict, computed/late-bound or canonical raw-string default; changing
global raw-value semantics; target lookup/invocation; transition application
or allowlist enforcement; rustfmt aspect/provider/fragment breadth; a new
mapping, DICE dependency, cache, I/O path, interner, hash or lifetime owner;
Java/JVM work; Zabel behavior/code adoption; an unpinned source; a new
fixture/oracle/network request; a cap violation; or a public rules_rust success
claim. After `lint_test.bzl` freezes, stop before the next rustfmt expression
and select the subsequent source-order audit in a separate packet.
