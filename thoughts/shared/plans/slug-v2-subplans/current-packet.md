# Current Slug V2 Packet

Packet: `WP-4-7A-rustfmt-test-target-attribute-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned frozen rule dependency declaration schema
Base: `cb8df441`

Result: load and freeze accepted rules_rust 0.73.0
`rust/private/rustfmt.bzl:218-243`. Extend the existing rule attribute owner
with the fixed `targets` label-list provider predicate, attached aspect and
custom transition while validating its documentation. Reject target invocation
before those facts can be dropped. Do not apply the aspect, evaluate the
transition, match dependency providers or run the implementation.

## Accepted starting point and first absent fact

Commit `50205fb3` freezes the third rustfmt aspect and its complete recursive
producer identities. Commit `cb8df441` selects this audit. The implementation
body remains lazy, rule test/doc arguments are accepted and the four
`LINT_TEST_COMMON_ATTRS` descriptors already freeze.

The exact `dict(LINT_TEST_COMMON_ATTRS, **{"targets": descriptor})` call is
already supported by the Starlark evaluator. Keyword overlay updates a
collision without moving the key and appends a missing key; the base contains
no `targets`, so the fixed descriptor is fifth. Slug's first absent argument is
the label-list `doc`; the same descriptor then supplies two required-provider
alternatives, one aspect and the previously accepted `platform_transition`.
These are one dependency-declaration schema, not configured edge results.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Use only those pinned objects and archive.

The authenticated Bazel chain is:

- `StarlarkAttrModuleApi.labelListAttribute` declares named `doc`,
  `providers`, `cfg` and `aspects` arguments;
- `StarlarkAttrModule.labelListAttribute` passes them into
  `createAttributeFactory`, where doc is type-checked/trimmed, provider
  alternatives become immutable producer-ID sets, `convertCfg` wraps the
  complete `StarlarkDefinedConfigTransition`, and `AspectsList.Builder`
  requires exported aspects and retains their recursive declaration objects;
- flat providers are one AND set, nested lists are ordered OR alternatives,
  duplicates within an alternative normalize, an empty inner alternative
  means accept-any, and unexported/non-provider shapes reject;
- an attached unexported or duplicate aspect rejects during descriptor
  construction; the fixed singleton keeps its defining module and first export;
- `Attribute.ImmutableAttributeFactory` includes doc, transition, required
  providers and aspects in immutable equality/hash and builds the named rule
  attribute without executing them; and
- `StarlarkRuleClassFunctions.rule` notes aspect propagation and a Starlark
  transition, then freezes the rule while implementation and configured
  behavior remain deferred.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `src/starlark_host/engine/build_rule_declaration.zig` keeps optional
providers/aspects/cfg in one producer-owned `AttrDefinition`, snapshots aspect
membership and follows declaration values during module freeze. Its
`build_invocation_capture.zig` later detaches provider identities, aspect
export identities and transition provenance for configured consumers. Slug
follows only that phase/owner split. No Zig code, layout, evaluator behavior,
cache, analysis algorithm or compatibility conclusion may be copied; Bazel
remains sole behavior authority.

## Compatibility classification

- **Exact:** valid string/`None` doc input; the fixed ordered provider
  alternatives
  `[[@@dep+//rust/private:providers.bzl%CrateInfo],
  [@@dep+//rust/private:providers.bzl%TestCrateInfo]]`; one complete exported
  `_rustfmt_test_aspect` value; the complete existing `platform_transition`
  value; fifth-position dictionary merge; recursive rule freeze and
  `rustfmt_test` first-export identity; lazy implementation behavior.
- **Slug-native:** discarding documentation after validation; the existing
  `ProviderId`, Arc-backed alternatives, optional frozen aspect, frozen
  transition representation, compact strings, Rust equality/ordering,
  diagnostics, complete-module fingerprint over-invalidation and memory
  accounting; fail-closed target invocation.
- **Unsupported/deferred:** documentation extraction; flat, empty, duplicate,
  wider or non-singleton provider predicates beyond rejection; zero, duplicate
  or wider aspect lists beyond rejection; native providers/aspects;
  aspect/required-aspect application or propagation; transition evaluation;
  configured dependency provider matching; target invocation, `ctx.attr`,
  analysis/actions; following root source order; M8/M7B and exact Bazel
  configuration/output identity.

## Natural owner, lifetime and utility reuse

`RuleAttributeSchemaGen` and `FrozenRuleAttributeSchema` remain the sole
declaration/freeze owners. Reuse the exported-provider projection shared with
aspect declarations and store the two alternatives as Arc-backed singleton
slices. Retain the one aspect as its complete traced value and freeze it with
the recursive rule module; do not reconstruct its label/name. Reuse the
existing `TransitionDefinitionGen` value already used by label attributes;
its implementation closure and output remain owned by `lint_test.bzl` and are
not invoked here.

No evaluator heap or request scratch may survive freeze. No DICE key, source
observer, repository mapping, I/O, cache, interner, hash domain, lock, async
task or command result changes. Existing recursive module identity/fingerprint
invalidates the complete schema. The Buck2 utility audit selects existing Arc
slices, `ProviderId`, frozen values, `CompactString` and `Allocative`; no new
retained utility, collection or representation family is admitted.

## Implementation boundary

1. Add named `doc`, `providers`, `aspects` and `cfg` inputs to
   `attr.label_list`. Validate/discard doc using the existing helper.
2. Map omitted providers to empty and accept only two distinct singleton
   alternatives of already-exported user-provider constructors when explicit.
   Reuse their `ProviderId`s; reject flat, empty, duplicate, wider,
   non-provider and unexported shapes.
3. Map omitted aspects to no attached aspect and accept only one already
   exported aspect when explicit. Retain the complete value and freeze it;
   reject explicit empty, duplicate/wider, non-aspect and unexported shapes.
4. Pass `cfg` through the existing custom-transition conversion and retain its
   complete transition value unchanged. Do not broaden transition call shapes.
5. Carry both new facts from `AttributeDefinitionGen` through
   `RuleAttributeSchemaGen` and rule freeze. All existing descriptors receive
   empty state. Reject invocation of any frozen rule with a provider-constrained
   or aspect-bearing schema before the ordinary loading projection can discard
   those facts; preserve accepted custom-transition-only invocation behavior.
6. Do not add fields to the configured `AttributeSchema`, apply either policy,
   retain raw provider/aspect lists, add a registry, or execute any function.

## Discriminating proof

- Extend the accepted recursive rustfmt fixture through `rustfmt_test` and an
  importer alias. Assert the frozen rule's four common attributes followed by
  `targets`; its LabelList kind, defaults/policy, exact two provider IDs, exact
  attached aspect defining label/first export/advertised provider/required
  aspect, and transition output. Give the rule implementation a failure that
  must not run.
- Assert earlier label-list and other rule schemas retain empty
  provider/aspect state and unchanged transitions.
- Reject invalid docs; flat/empty/duplicate/wider/non-singleton,
  non-provider and unexported provider predicates; empty/duplicate/wider,
  non-aspect and unexported aspect lists; wrong cfg values; and a target
  invocation of the newly frozen rule. Preserve BUILD absence where relevant.
- Keep the third/second aspect, lint-test attributes, custom-transition and
  fixed rule-schema tests green. Add no fixture, oracle, network or Bazel run.

## Allowlist and growth caps

Only these files may change from base `cb8df441`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `9871768a44901f4a25ed965c17e0578c524cd92de3cc66ec2544dc16edc3053a` | 5,707 | 5,827 | retained provider/aspect schema and fail-closed invocation |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `2ccbbfaa67e7075b915c5e99daade7abd42a9130d8ae6ed1556364312c97444b` | 4,886 | 5,066 | recursive rule identity/freeze and rejection proofs |

Additions are capped at 120 production lines, 180 proof lines and 300 total
lines. Deletions do not buy addition budget. No touched function may exceed
150 lines. `package.rs` already exceeds the 2,000-line review trigger, but the
existing attribute/rule definitions are the cohesive declaration owner; a new
module or registry would split one semantic lifetime. STOP if either converter
cannot remain a small private helper.

## Serial validation and review

Run Cargo commands serially with one shared target directory:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_test_rule
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 rustfmt_test_aspect
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 lint_test_common_attributes
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 transition
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The archive checker may report only its known three retained thoughts paths
plus active packet files. Recheck hashes, additions, physical lines and
touched-function lengths before review. Independent terminal review is
mandatory before commit and must verify source order, pinned Bazel behavior,
pinned Zabel guidance-only use, dict ordering, producer identities, exact
fixed shapes, lazy behavior, fail-closed invocation, frozen lifetime, caps,
serial validation and absence of a new semantic side owner.

## STOP / `REPLAN`

STOP and `REPLAN` if completion requires a file outside the allowlist; provider
or aspect breadth beyond the fixed shapes; raw evaluator-value or mutable-list
retention; identity reconstruction; configured `AttributeSchema` changes;
aspect application/propagation, transition execution, provider matching,
target invocation, analysis/actions; a new DICE key, mapping, cache, I/O path,
interner, hash or lifetime owner; Java/JVM work; Zabel code or behavior
adoption; an unpinned source; a new fixture/oracle/network request; a cap
violation; or a public rules_rust success claim. After the frozen rule loads,
stop and replay selected root source order separately.
