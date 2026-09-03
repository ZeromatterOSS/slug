# Current Slug V2 Work Packet

Packet: WP-4-5-8-7A-native-genrule-imported-declaration-audit-r1

Status: docs/oracle audit and frozen design, awaiting independent review.
Commit `21a5279cf` terminally accepts imported-Bzl `Label()` caller
provenance. Rust implementation is not authorized before `ACCEPT`.

## Predecessor and authentic boundary

Commit `21a5279cf` accepts
`WP-4-7A-build-imported-bzl-label-caller-provenance-implementation-r1` at
119 production/241 proof/360 aggregate gross Rust additions. Focused and full
loading/query tests, the pinned-nightly CLI build, formatting, diff, scope,
archive-baseline, daemon and terminal-review gates pass.

The rebuilt bounded-PATH rules_rust replay now stops at selected rules_java
9.1.0 `toolchains/default_java_toolchain.bzl:210`:

```text
Object of type `native` has no attribute `genrule`
```

This member-lookup failure occurs before Starlark evaluates the call's keyword
expressions. It therefore does **not** prove that either `Label()` expression
at lines 212-213 ran. The accepted predecessor proves those expressions in
isolated BUILD-to-imported-def coverage; the positive genrule evidence below,
not the failing replay, authorizes this packet.

## Selected source and oracle evidence

The selected BCR coordinate remains rules_java 9.1.0. Its 114,566-byte,
114-entry release archive has SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`.
Both selected files are regular `0444` with trailing LF:

| Source-relative path | SHA-256 | Exact anchor |
|---|---|---|
| `toolchains/BUILD` | `b23a9b08e5928120d2d3f3a559b9c54f8472cabf1a4b99baf7cc6f29886a9b73` | load 9-15; six helper calls 102-130 |
| `toolchains/default_java_toolchain.bzl` | `6f963992c933e6cbc48f0c64f3349484422ee06f01830473ea802731b874deea` | helper 201-217; `native.genrule` 210-217 |

Each helper call first records a filegroup, then one genrule using only the
selected names `name`, `srcs`, `toolchains`, `cmd`, `outs`, and `tags`.
The first expands to:

```text
name = "gen_include/jni.h"
srcs = [@@rules_java+//toolchains:current_java_runtime]
toolchains = [@@rules_java+//toolchains:current_java_runtime]
cmd = "cp $(JAVABASE)/include/jni.h $@"
outs = [@@rules_java+//toolchains:include/jni.h]
tags = ["manual"]
```

A pinned Bazel 9.2.0 query over the authenticated rules_rust workspace returns
six `genrule rule` declarations with those canonical values and generator
frames at BUILD lines 102, 107, 112, 117, 122 and 127 plus helper line 210.

An isolated two-package Bazel 9.2.0 oracle supplies the caller-context
discriminator. `defs/helper.bzl` (SHA-256
`84d42ec1739e21ae7404d57dbfadf09d3f72380ac44156792777cb749c4e9397`)
calls imported `native.genrule`; `consumer/BUILD.bazel` (SHA-256
`f7d480c06151c22b1436277312fa463d7dd5ba8eff0c0eb8676790efc87d8416`)
invokes it. Bazel reports:

```text
genrule rule //consumer:generated
srcs = [//consumer:raw.txt, //defs:typed.txt]
toolchains = [//defs:typed.txt]
outs = [//consumer:nested/out.txt]
generator_function = "make_genrule"
generator_location = "consumer/BUILD.bazel:3:13"
generator_name = "generated"
generated file //consumer:nested/out.txt
```

Thus a raw string is converted in the BUILD package/repository context, an
already constructed Label preserves its lexical `.bzl` identity, and an
output belongs to the BUILD package. The scratch oracle was removed after its
hashes and output were captured; no fixture enters the repository.

The same archive contains a bare BUILD `genrule` in
`java/bazel/BUILD.bazel:23` (file SHA-256
`f718e9b6d50f4125680b93b3127ea4acf2f98b5db66c9a835b45f5e8e725c1f0`).
It is not on the selected package boundary and is deliberately deferred.

## Bazel 9.2 authority

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the
compatibility authority:

- `RuleFactory.java:58-107,279-328`, SHA-256
  `6a7e75749c60ce77d4e896ad480adb1df3e4cab9367f6222071d29ace402d60a`,
  validates names, exposes a named-only built-in rule callable, uses the
  thread's target-definition context and creates/adds the rule;
- `TargetDefinitionContext.java:406-407`, SHA-256
  `9f3d14396aa277853ec4ed017e6ff0f0121417ccb2c7c49d935f3f7674b254a2`,
  owns BUILD-package/repository-mapping conversion;
- `BuildType.java:412-425,562-568`, SHA-256
  `3064c09abcb9f38829c03c16ed1fb2799a40ebbca2ea3904a68808e158d325f8`,
  preserves typed Labels, converts strings with that context, and confines
  outputs to the BUILD package;
- `GenRuleBaseRule.java:44-336`, SHA-256
  `0ed81609afab11446634a460df66164a1cc00f1ede1c5775a1cec099b59fe874`,
  defines `srcs`, `toolchains`, mandatory `outs`, commands and the
  default-false executable projection;
- `BaseRuleClasses.java:247-262`, SHA-256
  `ed804a0bdcad1b7f244d828ee443bf53901562a9a9d1a653d77f646024171f33`,
  defines order-independent tags and generator name/function/location;
- `BazelGenRuleRule.java:30-57`, SHA-256
  `e149ea4765673b84f162be296ad55155b2d9ed8e3a627987886c4abc9bc75e7a`,
  fixes class identity and configured-only setup/stamp behavior;
- `GenRuleBase.java:68-77` (17,055 bytes/402 lines), SHA-256
  `bd8c80e041310e8bf66ff8a30a7f8ccb91bb8d25262b4f42c37f38373596c555`,
  reports empty outputs only after configured genrule creation begins;
- `RuleClass.java:2057-2074`, SHA-256
  `33be32dc5c884d7fba2338f13f3bc4bcd0c175e3479c70fcd810474a5749b5e6`,
  converts attributes and populates output files; and
- `TargetRecorder.java:401-432,542-602`, SHA-256
  `fe314cb714d04ff3991a1dafdd797c73de21cc2fa7611a8c0afdbb3ee7d21060`,
  owns generated-output linkage and collision checks.

Pinned `OutputFileTest.java:58-168` (SHA-256
`b73173bb3c0bb17c1ffe935133577754057d7481e485ca363e89d3dac7df5fa8`)
and `PackageFactoryTest.java:211-258` (SHA-256
`5a0aa8aa9db3da0bc197560bb640a93c1afc7b1ef59eb6e1324a380a6445fba6`)
cover nested output ownership and conflicts. Existing accepted generator-frame
evidence remains authoritative.

## Decision and compatibility

Audit result: `ACCEPT`, pending independent review, for one generic selected
loading category: a BUILD evaluation may call an imported `.bzl` def whose
body calls `native.genrule` with required `name`, `cmd`, and nonempty `outs`,
optional `srcs`, `toolchains`, and `tags`, and no other attributes. Requiring
nonempty `outs` during loading is the selected Slug-native restriction.

**Exact:** retain ordered `srcs`, `toolchains`, `outs`, exact `cmd`, normalized
order-independent `tags`, class name `genrule`, omitted `executable=false`,
macro generator metadata, BUILD-context raw-string coercion, existing typed
Label identity, BUILD-package output identity, generated-file ownership,
omitted/package-default rule visibility, generating-rule output visibility and
atomic package publication. Support ordinary and Bzlmod Host package attempts
without a rules_java branch.

**Slug-native:** use compact Rust values and fail closed with explicit stable
Slug diagnostics at the unconfigured-query and configured-analysis
boundaries. Reject empty `outs` during loading to preserve the nonempty
retained invariant even though Bazel reports that error during configured
genrule analysis. Diagnostics for malformed or excluded calls are not
Bazel-byte parity claims.

**Unsupported/deferred:** bare BUILD `genrule`; positional calls; selectors;
`tools`, `cmd_bash`, `cmd_bat`, `cmd_ps`, `local`, `message`,
`output_to_bindir`, `output_licenses`, `stamp`, `heuristic_label_expansion`,
`executable=true`, visibility overrides and every other attribute; complete
native RuleClass/query projection; configured dependency transitions,
toolchain/provider/TemplateVariableInfo resolution, Make/location expansion,
shell/environment selection, providers, Spawn/actions, ActionKeys, execution,
run behavior, output materialization and later Java semantics.

Excluded or malformed shapes must fail before any package result publishes.
Do not claim Bazel diagnostic bytes for them.

## Ownership, state and complexity

`package.rs` remains the sole declaration/attempt owner. Add one `Allocative`,
structurally comparable compact declaration carrying optional Arc slices for
explicit `srcs`, `toolchains`, and normalized tags, a nonempty Arc output
slice, one compact command, and an optional compact generator
name/function/location triple computed from the evaluator before recording.
Add `PackageTargetKind::Genrule`, its static non-executable capability, one
shared validate/coerce/precheck/record helper, and only the `native_methods`
wrapper. Do not add a bare BUILD global or a partial `NativeRuleAttributes`
sidecar.

Use existing package label/output coercion, `record_target`,
`generated_file`, visibility and attempt-level atomicity. Before inserting the
rule or any output into attempt scratch, precheck the rule and full output set
for duplicate outputs, existing target/output collisions, prefix collisions in
both directions, and input=output. No partial `PackageEvaluation` may publish.

`bzl_module.rs` may admit this typed declaration and only generated files
whose producer is an admitted genrule through the external loaded-package
gate. `slug_query_v2/src/graph.rs` must return the frozen unsupported terminal
instead of inventing edges or attributes. Existing analysis catch-all must be
proved to reject the genrule before computing children, toolchains, an
implementation or actions; no analysis production edit is authorized.

The declaration enters existing `PackageEvaluation` equality and Allocative
coverage, so existing BUILD/Bzl source observations, recursive manifests,
package-load keys and Host repository routes own invalidation and A/B/A. Add
no DICE key/input/edge, lock, await, cache, map, interner, registry, filesystem
read, process state or duplicate retained owner. Arc clone remains constant
time; this cold loading seam needs no benchmark. Overlapping requests share
only immutable DICE-retained source and `PackageEvaluation` facts; every
`PackageRecorder` remains attempt-local, and failure or cancellation publishes
no completed package. Stage 9 records no V1/Buck2 extraction.

The large-file trigger is reviewed for `package.rs` (11,706 lines),
`bzl_module.rs` (11,442) and `graph.rs`'s existing large match. Each is the
sole existing owner of the changed boundary. The graph edit is an explicit
terminal of at most eight lines, while the external gate needs only a bounded
producer lookup; moving either whole match would obscure ownership. New or
materially changed functions must remain below 150 lines.

## Frozen implementation successor

After independent `ACCEPT`, activate
`WP-4-5-8-7A-native-genrule-imported-declaration-fail-closed-implementation-r1`
with exactly this allowlist:

- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_query_v2/src/graph.rs`;
- `app/slug_loading_v2/tests/build_file_loading.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- `app/slug_analysis_v2/tests/starlark_rule.rs` (proof only).

Caps are 170 production, 260 proof and 430 aggregate gross Rust additions.
Production file ceilings are 145 in `package.rs`, 15 in `bzl_module.rs`, and 10
in `graph.rs`.

Proof must cover the two-package oracle discriminator, ordered typed/raw label
identity, selected rules_java-shaped pre-filegroup plus genrule declaration,
all six selected calls without special-casing, nested generated-file ownership,
generator metadata, non-executable class capability, ordinary and Bzlmod Host
packages, repository mapping, omitted versus explicit-empty slices, and
same-DICE source A/B/A across command/label/output/tag changes. Prove the bare
BUILD global remains absent; positional, missing/wrong `name` or `outs`, empty
outs, wrong types and excluded attributes including `executable=true` fail
without publication. Cover duplicate outputs within one rule, existing
target/generated-output collision, prefix collisions in both directions and
input=output. Freeze the query
terminal exactly as:

```text
native genrule query projection is unsupported
```

Regression-proof ordinary native query behavior and, in the analysis proof
owner, direct genrule rejection before any child/toolchain/implementation/action
work. A configured generated output may follow only its existing producer edge;
prove the producer then rejects before its own dependency/toolchain/
implementation/action work. Reuse inline scratch sources and the accepted
oracle/replay evidence; add no fixture.

Run focused native/package/Host/query tests, then serial full loading, query
and analysis library suites, direct pinned-nightly CLI build, authenticated
bounded-PATH replay, stale-slugd, formatting, diff, archive, allowlist and cap
checks. Replay must clear all six selected genrule declarations and report
only the next authentic boundary; it must not claim genrule configured/action
or Java initializer semantics.

Return `REPLAN` if bare BUILD `genrule`, any excluded attribute, selector,
configured child/toolchain/provider/action work or query dependency projection
is required; typed inputs need duplicate native-attribute storage; generated
outputs cannot be admitted without widening other loaded target kinds; a new
DICE/global/cache/map/interner/filesystem/process owner, seventh file or cap
excess is required; a touched function exceeds 150 lines; or replay selects or
analyzes a genrule/generated output rather than merely loading it.
