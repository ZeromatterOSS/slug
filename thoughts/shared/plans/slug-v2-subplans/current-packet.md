# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-second-aspect-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned frozen aspect attributes and required-aspect identity
Base: `d66059ac`

Result: load and freeze the second declaration in accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:152-192`. Extend the existing frozen aspect owner with
the two fixed private label attributes and the single
`requires = [rustfmt_srcs_aspect]` producer edge. Preserve each Label default's
defining-module repository and the required aspect's first-export identity.
Do not apply or propagate either aspect, resolve configured dependencies, expose
fragments/toolchains, or advance into the later rustfmt test aspect.

## Accepted starting point and first absent fact

Commit `d4d4d6dc` freezes the first rustfmt aspect with its exact two singleton
provider alternatives and fixed `cpp` fragment. Commit `d66059ac` authenticates
the next source-order call. Function `_rustfmt_aspect_impl` at lines 129-150 is
lazy, so evaluation reaches `rustfmt_aspect = aspect(...)` at lines 152-192.
Slug already accepts its implementation, documentation, provider predicate,
fragment and canonical toolchain requirement. The `attrs` dictionary at lines
170-182 is the first unsupported argument; the same call's
`requires = [rustfmt_srcs_aspect]` at line 187 is the next missing declaration
fact.

The fixed dictionary has exactly two private label descriptors in source order.
`_config` has the typed default `//rust/settings:rustfmt.toml`,
`allow_single_file = True`, target configuration and no executable policy.
`_process_wrapper` has typed no-colon default `//util/process_wrapper`, which is
canonically `//util/process_wrapper:process_wrapper`, plus `cfg = "exec"` and
`executable = True`. Both defaults resolve in the rustfmt defining module. The
required edge names the already first-exported `rustfmt_srcs_aspect` object from
that same module. Neither implementation may execute while loading or freezing.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive, not sibling HEADs.

The authenticated Bazel chain is:

- `StarlarkRuleFunctionsApi.aspect` declares named `attrs` and `requires`
  inputs with empty defaults;
- `StarlarkRuleClassFunctions.aspect` builds every descriptor, rejects
  explicitly configurable, materializing and dormant attributes, requires a
  default on implicit attributes, rejects computed defaults, and limits public
  parameters to bool/int/string; the two fixed private labels pass without
  public-parameter validation;
- the same constructor casts `requires` to `StarlarkAspect` and retains an
  immutable set of the required aspect objects without running implementations;
- `StarlarkDefinedAspect` retains attributes and required aspect objects in
  equality/hash, assigns the producer module/name on first export, and only in
  later `buildDefinition` converts a required object to its aspect class;
- `StarlarkRuleClassFunctionsTest.testAspectExtraDeps`,
  `testAspectNoDefaultValueAttribute` and the required-aspect stack/order tests
  authenticate declaration retention, implicit-default failure and producer
  identity; `AspectCollection` detects inconsistent duplicate/cycle paths only
  while assembling applied aspect paths, which remains deferred here.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_rule_declaration.zig` retains named attributes and the complete
required-aspect value inside one producer-owned `AspectDefinition`, while
`AspectExportIdentity` separately records producer module plus first exported
name; module freeze explicitly retains `requires` as a child. Slug follows that
owner shape with its existing frozen schema and aspect value. No Zig code,
representation, evaluator behavior, cache, analysis algorithm or compatibility
claim may be copied; Bazel 9.2 remains sole compatibility authority.

## Compatibility classification

- **Exact:** validation, defining-module canonicalization and freeze of the two
  fixed private label descriptors; their source order, kinds, defaults,
  single-file and exec/executable policy; retention of exactly one exported
  required aspect object with its producer module/name; the second aspect's
  existing implementation, provider predicate, `cpp` fragment, toolchain,
  documentation and first-export identity; lazy implementation behavior.
- **Slug-native:** public `_name` storage rather than Bazel's internal `$name`
  spelling, the existing `RuleAttributeSchemaGen`, `CoercedAttributeValue`,
  `CanonicalLabel`, `CompactString`, Arc-backed slices and frozen Starlark value
  edge, Rust equality/ordering, complete-module fingerprint over-invalidation
  and nonrequired diagnostics.
- **Unsupported/deferred:** public aspect parameters, other/private attribute
  dictionaries, configurable/materializing/dormant/computed defaults, more than
  one required aspect, duplicate/cycle observability, aspect class derivation,
  application/propagation, provider matching, configured dependencies,
  `ctx.fragments`, toolchain selection, actions, later rustfmt declarations,
  M8/M7B and exact Bazel configuration/output identity.

## Natural owner, lifetime and utility reuse

Reuse the existing caller-aware attribute descriptor conversion so string and
typed Label defaults are canonical before the aspect receives them. Reuse
`RuleAttributeSchemaGen` as the single declaration schema; no aspect-only label
record is permitted. `AspectDefinitionGen` and `FrozenAspectDefinition` remain
the sole declaration/freeze owners. Store fixed schemas in one Arc slice and
the required producer as one traced value that freezes with the recursive Bzl
module. The frozen edge points to the complete required aspect object, whose
existing `defining_label` and `exported_name` remain authoritative; do not copy
or reconstruct those fields in the consumer.

No evaluator heap or request scratch survives freeze. No DICE key, source
observer, repository mapping, I/O, cache, interner, hash domain, lock, async
task or command result changes. Existing recursive module identity/fingerprint
invalidates the complete declaration. No fallback is introduced. The Buck2
utility audit selects existing Arc slices, compact strings, canonical labels,
frozen values and `Allocative`; no new retained utility, collection or
representation family is admitted.

## Implementation boundary

1. Extract the existing descriptor-to-`RuleAttributeSchemaGen` projection into
   one loading-private helper shared by `rule(attrs)` and this aspect path;
   preserve rule behavior.
2. Add a fixed aspect-attribute converter that accepts exactly `_config` then
   `_process_wrapper`, requires transient `attr.label` descriptors with the
   authenticated defaults and policies, and retains their complete schemas.
   Omitted `attrs` remains empty for earlier admitted aspects. Reject partial,
   reordered, wider, renamed or differently shaped dictionaries.
3. Add a fixed required-aspect converter that accepts either omission or one
   already-exported aspect value. Retain that value directly and freeze it with
   the consumer; reject empty, wider, non-aspect and unexported inputs. Do not
   derive a class key or rebuild producer identity.
4. Freeze both fields inside the existing aspect definition and expose only
   crate-private test projections. Preserve all accepted constructor behavior,
   first-export identity and BUILD absence.
5. Do not execute either implementation, retain raw attribute dictionaries,
   add an aspect registry, or wire attributes/requirements into configured
   analysis.

## Discriminating proof

- Extend the accepted recursive `providers.bzl -> common.bzl -> rustfmt.bzl ->
  root.bzl` proof with the exact second aspect. Assert both frozen schemas in
  source order, the canonical defaults
  `@@dep+//rust/settings:rustfmt.toml` and
  `@@dep+//util/process_wrapper:process_wrapper`, single-file and exec policy,
  and the second aspect's producer identity.
- Downcast the frozen required edge to `FrozenAspectDefinition` and assert it
  preserves `@@dep+//rust/private:rustfmt.bzl%rustfmt_srcs_aspect`; assert the
  provider identities remain owned by `providers.bzl`. Give both lazy bodies a
  failing implementation to prove neither executes.
- Prove omission remains empty and reject partial/reordered/wider/renamed or
  wrong-kind/policy/default attrs, zero/multiple/non-aspect/unexported
  `requires`, and the same aspect call from BUILD globals.
- Keep the accepted aspect, Label, lint-default and first-rustfmt tests green.
  Add no fixture, registry response, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `d66059ac`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `f9eb5227134d00e8e0534d394ce0be775eb8fc210bd6f1f81b228dace9f696b8` | 5,581 | 5,706 | shared schema projection and retained aspect fields/conversion |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `38128309fa50de13ab98bae00dd72c25b5f13ae1372ef69fcfc380ec77ac1717` | 4,740 | 4,920 | recursive freeze/identity and fail-closed proofs |

Additions are capped at 125 production lines, 180 proof lines and 305 total
lines. Deletions do not buy addition budget. No touched function may exceed
150 lines. `package.rs` already exceeds the 2,000-line review trigger, but the
aspect and shared schema definitions remain one cohesive loading owner; a new
module or registry would split the semantic lifetime. STOP if either converter
cannot remain a small private helper.

## Serial validation and review

Run Cargo commands serially with one shared target directory:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_second_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_first_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 bazel_aspect_definition_validates_admitted_fixed_abi_and_build_absence
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 label_attribute_defaults_keep_defining_module_identity
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The archive checker may report only its known three retained thoughts paths
plus active packet files. Recheck hashes and count additions, physical lines
and touched-function lengths before review. Independent terminal review is
mandatory before commit and must verify source order, pinned Bazel behavior,
pinned Zabel guidance-only use, fixed schemas/default owners, complete required
producer identity, lazy implementations, frozen lifetime, BUILD absence, caps,
serial validation and absence of a new semantic side owner.

## STOP / `REPLAN`

STOP and `REPLAN` if completion requires a file outside the allowlist; any
aspect dictionary beyond the exact two fixed private labels; a computed,
late-bound, materializing or configurable attribute; more than one required
aspect; unexported/consumer-reconstructed aspect identity; raw dictionary or
evaluator-scratch retention; aspect application, class derivation,
propagation, configured fragments/dependencies, analysis/actions; a new DICE
key, mapping, cache, I/O path, interner, hash or lifetime owner; Java/JVM work;
Zabel code or behavior adoption; an unpinned source; a new fixture/oracle/network
request; a cap violation; or a public rules_rust success claim. After the
second aspect freezes, stop at the first unsupported expression in the later
rustfmt declarations and select its audit separately.
