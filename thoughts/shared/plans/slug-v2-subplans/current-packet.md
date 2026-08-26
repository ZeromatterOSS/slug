# Current Slug V2 Packet

Packet: `WP-4-7A-rust-analyzer-toolchain-rule-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 attribute descriptors and frozen Starlark rule schema
Base: `3c714c34`

Result: load and freeze the complete fixed
`rust_analyzer_toolchain = rule(...)` declaration at accepted rules_rust
`rust/private/rust_analyzer.bzl:359-402`. Retain executable and exec-transition
policy in the existing declaration owner and fail closed before target
recording. Stop before configured dependency or analysis behavior.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkAttrModuleApi.stringAttribute` and `labelAttribute` make all fixed
  descriptor parameters named-only; `doc` is `string | None`.
- `StarlarkAttrModule.createAttribute` trims/retains docs, sets mandatory and
  executable flags, requires non-`None` `cfg` when executable is true, and
  retains `allow_single_file` as both file policy and `SINGLE_ARTIFACT`.
- `StarlarkAttrModule.convertCfg` maps `"exec"` to
  `ExecutionTransitionFactory`; it is distinct from target/no-transition and
  from a Starlark transition.
- `StarlarkRuleClassFunctions.rule/createRule` retains the ordered attribute
  schema and implementation until first producer export constructs the rule
  class; declaration loading does not execute the implementation.
- `StarlarkRuleClassFunctionsTest.testAttrDoc`,
  `testAttrDocValueBadType`, `testAttrSingleFileWithList`,
  `testMandatoryConfigParameterForExecutableLabels`, `testRuleDoc`, and the
  focused exec-label integration rows discriminate the selected behavior.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
The exact fixed declaration has optional `proc_macro_srv` and `rust_analyzer`
exec/single-file labels, mandatory `rustc` with the same policy, mandatory
target-configured `rustc_srcs`, and string defaults `"library"` and `""`.

Slug already retains mandatory, configurable, single-file, typed defaults,
custom transitions, implementation lifetime and exported rule class in one
`RuleAttributeSchemaGen`/`RuleDefinitionGen` freeze owner. The first live
failure is label `doc`; next, `cfg = "exec"` is rejected as not a custom
transition, and `executable` is not in the call ABI. Accept string/`None` docs
and discard them consistently with accepted provider/rule doc loading.
Add separate executable and exec-transition booleans beside the existing
custom transition; do not encode exec as a custom transition or ordinary
target identity.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `ordinary_dependency_facts.AttributeSchema` retains executable,
single-file and dependency-transition policy together while target-local
values use a thin schema index; declaration-owner and executable-module
identity remain separate relations. Follow only that single-owner/projection
lesson. Do not copy its Zig code, layout, evaluator, DICE keys or behavior.
Bazel 9.2 remains authoritative.

The Buck2 utility-reuse audit selects the existing compact rule schema,
`CompactString`, Arc-owned schema slice and frozen Starlark values. Two booleans
add no collection, allocation, interner, cache, registry, hash domain or Stage
9 ledger entry. Do not widen `AttributeSchema` in `attrs.rs`: this packet must
reject invocation before that target-local projection exists.

## Decision and exact boundary

Admit only the fixed declaration shapes:

- named label `doc` as omitted, string or `None`, validated then discarded;
- named label `executable` as a boolean defaulting false; omitted and explicit
  false are the same retained policy;
- label `cfg = "exec"` as a distinct retained marker, while preserving the
  accepted custom-transition path;
- existing label mandatory and single-file policy; and
- named string `doc` plus the existing typed defaults needed by the two fixed
  attributes.

Require executable true to have an admitted non-`None` cfg: either the exec
marker or the existing custom-transition value. Exec cfg is independently
valid when executable is omitted or false. A rule definition with executable-
true or exec-marked attributes freezes and exports normally, but calling it
must fail before `PackageRecorder` records a target. An omitted/false
executable with an existing custom transition retains the accepted invocation
path. Do not project either new field into ordinary dependency analysis in this
packet.

Explicit `cfg = None`, `cfg = "target"`, other native transitions,
documentation retention/extraction and every other new descriptor parameter
remain unsupported/deferred. Do not widen rule parameters or Label behavior.

## Ownership, revision and lifetime

The attribute descriptor is evaluation-scratch and freezes into the defining
rule's existing ordered schema. The exported rule's producer `.bzl`,
implementation, class name and schema remain one owner; imported aliases do
not acquire new identity. Existing recursive Bzl DICE keys observe the source
and retain the frozen module closure. No new key, source observation,
filesystem read, request overlay or service cache is added.

Docs are nonsemantic metadata outside the admitted extractor surface and are
discarded before freeze. Executable and exec-transition bits affect future
dependency semantics, so both survive descriptor freeze and rule-schema
freeze. They must be copied into no target-local value until that consumer is
implemented; invocation fails first. Scratch descriptors die with evaluation,
and frozen fields die with their existing module/package owners. There is no
async, cancellation, eviction or shutdown duty.

## Files and caps

Only these files may change, against the listed base SHA-256:

| File | Base SHA-256 | Final line cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `68ed3d71d6224e206dba88fc32961a0f75f74b523083128549985b94b02da4ba` | 5,510 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `f2691185e43417424dd82d8574cb6eca9b2d5cb2dd8c9a3cce887142c17bde67` | 4,440 |
| `app/slug_loading_v2/tests/build_file_loading.rs` | `5896bf427a600fde0b5ef4b09e416f9bb2efe263b47716f18b89e469221abbcb` | 3,165 |

Cap production additions at 105, proof additions at 150 and total additions at
255. No touched function may exceed 150 lines, with one explicit legacy
exception: `FrozenRuleDefinition::invoke` is already about 250 lines and may
receive only an early call to a new fail-closed helper, capped at four added
lines and no other body change. The large files remain cohesive:
`package.rs` owns the descriptor/frozen rule schema and invocation gate;
host tests own recursive external-Bzl freeze proof; integration tests own local
target-recording proof. `REPLAN` before a fourth Rust file or any cap breach.

## Proof and validation

Prove:

- the exact six-attribute declaration loads recursively and freezes with three
  executable exec/single-file labels, the mandatory plain label and both
  string defaults, without executing its implementation;
- executable and exec markers survive descriptor and rule freeze independently
  of mandatory/single-file policy, while custom transitions remain unchanged;
- string/`None` docs are accepted and bad doc types reject; executable true
  without cfg rejects during descriptor creation;
- cfg exec with executable omitted and explicit false both retain the exec
  marker and reach the same invocation gate; executable true with exec and
  with an existing custom transition retains both independent policies;
- omitted and explicit-false executable values preserve the accepted custom-
  transition descriptor/invocation path without reaching the new gate;
- invoking an exported rule carrying either new semantic marker rejects before
  the package contains that target; and
- existing typed descriptor, single-file, custom-transition, recursive export
  and BUILD named-only tests remain green.

Run serially: `cargo fmt --all -- --check`, focused descriptor/recursive/fail-
closed tests, full `cargo test -p slug_loading_v2`,
`cargo check -p slug_core_v2 --locked`, `cargo build -p slug_cli_v2 --locked`,
`git diff --check` and `scripts/v2_archive_status.sh`. Clean stale `slugd`
before/after any smoke. Pinned source and focused local proof suffice; do not
run Bazel or add an oracle fixture.

## Compatibility and STOP

- **Exact:** the fixed named descriptor calls, accepted values/type rejection,
  retained mandatory/single-file/default/executable/exec schema, recursive
  freeze and producer export identity.
- **Slug-native:** compact Rust representation, discarded docs within this
  loading-only surface, fail-closed invocation and nonrequired diagnostics.
- **Unsupported/deferred:** doc extraction, wider cfg/executable combinations,
  target invocation, configured exec dependencies, executable prerequisite
  validation, analysis/actions, later rust-analyzer declarations, aspect
  application, M8/M7B and exact output bytes.

STOP on dirty overlap, target recording for either newly gated policy,
`attrs.rs`/analysis changes, exec
configuration synthesis, Zabel code/behavior adoption, Java/JVM work,
fixture/network/dependency drift, public rules_rust success claims or any cap
breach. `REPLAN` before widening.
