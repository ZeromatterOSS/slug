# Current Slug V2 Work Packet

Packet: WP-4-6-7A-java-configuration-field-declaration-fail-closed-audit-r3

Status: R2 was accepted at commit `6542169e8`, and its parked five-file Rust
candidate clears both selected Java declarations. Authenticated replay then
disproved only the predicted next stop: BUILD invokes an earlier imported
`.bzl` helper before the Java toolchain initializer call. This docs-only R3
corrects source order and replay claims without changing the implementation
scope or Rust candidate, and awaits independent review.

## Predecessor and replay boundary

Commit `c8b4c9e86` terminally accepts the rules_java private `java_common`
loading facade at 71 production/154 proof/225 total gross Rust additions. Its
focused tests and formatting/diff/scope/archive gates pass. Authenticated
bounded-PATH rules_rust replay clears that facade and stops during `.bzl`
loading at selected rules_java 9.1.0
`java/common/rules/java_toolchain.bzl:602`:

```text
configuration_field(fragment = "java", name =
    "java_toolchain_bytecode_optimizer")
invalid configuration fragment name 'java'
```

No target is invoked and no Java rule implementation, configured dependency,
toolchain operation, provider constructor or action executes first.

## Durable selected-source evidence and complete census

The selected BCR coordinate is `rules_java` 9.1.0. Its durable descriptor is
`https://bcr.bazel.build/modules/rules_java/9.1.0/source.json`, SHA-256
`da589573c1dee2c9ac4a568b301269a2e8191110ff0345c1a959fa7ea6c4dfd6`.
It selects
`https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`,
a 114,566-byte/114-entry archive with SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`
and integrity `sha256-Thooolwu+lNQDJKNIs7/vFBd2VszWi0CWDaik7WSIS8=`.
The relevant files are regular `0444` with trailing LF:

| Source-relative path | SHA-256 | Bytes/lines | Role |
|---|---|---:|---|
| `java/common/rules/java_toolchain.bzl` | `5ad6511cdef925246961c7e7a9039475c192371fedbf909c63cf92334779e875` | 24,304/612 | initializer returns at 262 and is bound at 266; Java fields at 602/606; body uses at 112/133 |
| `java/bazel/rules/bazel_java_test.bzl` | `33b1b5e205c6658c661be6b0cd1b30fe0339d78f0b4fbc061a350024a412f412` | 5,416/148 | already-admitted coverage field at lines 90-93 |
| `java/rules_java_deps.bzl` | `40ce0f5b44b124f9fdc3986d542caa6b3a3213c2abbd4927cdea65ad42f31a23` | 8,257/224 | generated proxy load order |
| `toolchains/default_java_toolchain.bzl` | `6f963992c933e6cbc48f0c64f3349484422ee06f01830473ea802731b874deea` | 9,335/219 | defines `java_runtime_files` at 201-217; transient `native.filegroup` at 204-208; first/second `Label` at 212/213 |
| `toolchains/BUILD` | `b23a9b08e5928120d2d3f3a559b9c54f8472cabf1a4b99baf7cc6f29886a9b73` | 15,306/430 | loads `java_runtime_files` at 9-15, first calls it at 102; later first `default_java_toolchain` call at 365 |
| `MODULE.bazel` | `ee63f27e36a3fada80342869361182f120a9819c74320e8e65b1e04ba0cd7a9d` | 4,218/136 | registers `//toolchains:all` |

The complete archive census is exactly three `configuration_field` calls in
two files: Java `java_toolchain_bytecode_optimizer` on private executable
`_bytecode_optimizer` with `cfg="exec"`; Java
`local_java_optimization_configuration` on private
`_local_java_optimization_configuration` with `cfg="exec"` and
`allow_files=True`; and the already-admitted coverage `output_generator`.
There is no fourth selected call.

Load order is `default_java_toolchain.bzl` -> public
`java/toolchains:java_toolchain.bzl` -> generated compatibility proxy. The
proxy loads `bazel_java_test.bzl` before `java_toolchain.bzl`, so coverage
already succeeds before the first Java call stops. The authentic
`_java_toolchain_initializer` returns `kwargs` at
`java_toolchain.bzl:254-262`, is bound by `rule(initializer=...)` at 264-266,
and the surrounding durable anchor continues through the example at 282.
That later boundary is not reached. `toolchains/BUILD` first loads
`java_runtime_files` from `default_java_toolchain.bzl` at lines 9-15 and calls
it at line 102. The helper at lines 201-217 invokes `native.filegroup` at
204-208, mutating the package recorder transiently, before its first Label
string at line 212 fails; the second Label at line 213 is unreached. Package
failure is atomic, so none of that transient recorder state is published.

## Bazel 9.2 authority and producer gap

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is
the compatibility authority:

- `src/main/java/com/google/devtools/build/lib/analysis/starlark/BazelBuildApiGlobals.java:97-109`,
  SHA-256 `a54b4657f61846171d0dcaf42e3565e98ee1624316d06f4a47e8c66800fcf897`,
  validates the fragment and returns a Starlark late-bound label default;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkLateBoundDefault.java:50-114,170-244`,
  SHA-256 `68f54ff5291c2a2f38739fb5fec40671350f52eb0f493533d48e1de4e37c5abe`,
  owns fragment/field/tools-repository identity and configured reflection;
- `src/main/java/com/google/devtools/build/lib/rules/java/JavaConfiguration.java:236-339`,
  SHA-256 `23f67a5d8f447043fb9c834fb3a861c37a4d52bdcd77301c735f01a8d6a6de74`,
  declares five Java fields, including the two selected fields at 318/332;
- `src/main/java/com/google/devtools/build/lib/rules/java/JavaOptions.java`, SHA-256
  `a7ff72302b77cd071dabe4907289edce5fe56b16b7757c538c085fd777abf6a8`,
  owns the optimizer map, local-enable Boolean and optional local label;
- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/java/JavaConfigurationApi.java`, SHA-256
  `3b4f85cc1c526af138fc5f56c1a32cbfd0c3774177c5235774f71bd45f7afcaf`,
  names fragment `java`, while
  `src/main/java/com/google/devtools/build/lib/bazel/rules/JavaRules.java:39`, SHA-256
  `2819dd07d95cc6afb57a997683f71e9e6cd7019b90f527c42f84e4a7397f928c`,
  registers it; and
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleImplementationFunctionsTest.java:3007-3229`, SHA-256
  `89e6caf0c6d234be610ccb597a015610568c27f8071d572e55a7378a106597d8`,
  covers invalid fragment/field, private attribute and label-only behavior.

`JavaConfiguration` also exposes `launcher`, `proguard_top` and
`bytecode_optimizer`; none occurs in the selected archive. Bazel's
`src/test/java/com/google/devtools/build/lib/rules/java/JavaConfigurationTest.java`
and `src/test/java/com/google/devtools/build/lib/analysis/AutoExecGroupsTest.java:1352-1420` hash to
`3df5aba5e5411c5139655d2c77503c7b6b7d150536bc510f9b1afe3474465bbf`
and `293bf5070e33ac5e165e3c1fba7e76c0ef9e21f7e9cf028e3c693b9fd62062f8`;
they exercise producer validation and nondefault optimizer/action behavior,
which this packet deliberately does not claim.

Slug's native registry records defaults for `bytecode_optimizers`,
`experimental_local_java_optimization_configuration` and
`experimental_local_java_optimizations`, but `NativeCommandOption` exposes
none. There is therefore no admitted command mutation path for any of the
three producer facts. Resolving the selected fields to default `None` would be
a partial configured-parity claim and could advance into deferred Java rule
bodies; this packet must reject instead.

## Decision, compatibility and ownership

Audit result: `ACCEPT`, pending independent R3 review, for declaration retention
plus a configured fail-closed boundary.

**Exact:** recognize fragment `java` only for the two selected field names in
the existing Bzl-only `configuration_field` ABI; retain typed field plus tools
repository through rule declaration and freeze/import/re-export; preserve
source order, BUILD absence and existing C++/coverage behavior. Generic
no-initializer targets using the same declarations remain separately exact and
unchanged.

**Slug-native:** both Java identities fail configured projection before
dependency discovery, exec projection, toolchain lookup or rule invocation.
Freeze the Slug-native diagnostic bytes as:

```text
configuration_field(fragment = "java", name = "<selected-name>") configured resolution is unsupported
```

The closed Rust enum and structural identity replace Bazel reflection. The
three Bazel-valid but unselected fields remain rejected rather than silently
gaining parity.

**Unsupported/deferred:** BUILD-to-imported-`.bzl` Label definition/caller
provenance; initializer execution and authentic Java toolchain final-target
recording; every Java command-option producer, default and nondefault field
resolution, Java fragment facade, the other three fields, Java
toolchain/provider/rule-body behavior, optimization and compilation actions.
Add no inert label or default-`None` placeholder.

`configuration_field.rs` remains the natural producer. Add a private or
`pub(super)` `#[repr(u8)] JavaConfigurationField` with exactly two variants and
flatten it into the existing one-byte `ConfigurationField`; do not publicly
re-export the sibling enum. Existing `ConfigurationFieldIdentity` retains the
field and `CanonicalRepoName`, and existing immutable late-bound slices retain
rule state. `configuration.rs` owns the configured error. Loading/analysis
owners otherwise stay unchanged.

`configuration.rs` is currently 2,243 lines and triggers the >2,000-line review.
It remains the sole cohesive configuration projection/error owner: splitting
this two-field terminal would duplicate or expose its private projection
boundary. The touched projection and diagnostic functions remain below 150
lines; no split is authorized in this packet.

This adds one-byte discriminants only: no map, interner, registry, cache,
extra Arc, evaluator borrow, package/configuration field, command overlay,
DICE key/input/observation, lock, await, retry or fixture. Existing source and
recursive-manifest fingerprints own invalidation; configuration equality is
unchanged because Java projection always errors. Ordinary no-field rules add
no allocation. Request overlap, cutoff, cancellation and shutdown behavior do
not change. Buck2/V1 supplies no code; the Stage 9 row records reuse of the
existing compact enum/identity only.

## Frozen implementation successor

After independent R3 `ACCEPT`, activate
`WP-4-6-7A-java-configuration-field-declaration-fail-closed-implementation-r3`
with exactly this allowlist:

- `app/slug_configuration_v2/src/native/configuration_field.rs`;
- `app/slug_configuration_v2/src/native/configuration.rs`;
- `app/slug_configuration_v2/src/native/tests.rs`;
- `app/slug_loading_v2/tests/subrule_loading.rs`; and
- `app/slug_analysis_v2/tests/subrule.rs`.

Caps are 80 production, 210 proof and 290 aggregate gross Rust additions.

Proof must cover the exact three-call selected census and hashes; both Java
names plus coverage success; all three other Bazel-valid Java fields and an
unknown field rejecting; positional/named ABI and BUILD absence; private
`attr.label`-only use; freeze/import/re-export without initializer execution;
the unknown-field loading diagnostic
`invalid configuration field name 'missing' on fragment 'java'`; a separately
identified generic no-initializer target-recording control; selected
initializer declaration without authentic invocation; source order;
same-field/tools-repository equality/hash and pair/repository discrimination; one-byte
`ConfigurationField`, unchanged identity/late-bound carrier sizes and no
ordinary-rule carrier; same-DICE source A/B/A restoration; and byte-exact
frozen Slug-native configured errors before dependency, toolchain and
implementation sentinels. Preserve
C++/coverage configured projection tests.

Run focused native-configuration, loading and analysis tests, then serial full
configuration/loading/analysis/query suites, direct pinned-nightly CLI build,
authenticated bounded-PATH replay, stale-slugd, formatting, diff, archive,
allowlist and cap gates. Reuse source evidence; add no fixture. Replay must
clear both Java `.bzl` declarations and reproduce the current next stop at
`@@rules_java+//toolchains:default_java_toolchain.bzl:212`:

```text
Label() may only be called in a .bzl module
```

The failed package must publish no targets, but proof must not claim
pre-recorder rejection: the imported helper's `native.filegroup` has already
mutated only the doomed recorder transiently. Line 213 and the later
initializer-bearing `default_java_toolchain` call at `toolchains/BUILD:365`
remain unreached.

After this implementation, audit
`WP-4-7A-build-imported-bzl-label-caller-provenance-audit-r1` as the bounded
generic BUILD-calls-imported-`.bzl` Label definition/caller-provenance category;
do not special-case rules_java or this helper.

Return `REPLAN` if rejection cannot precede dependency/toolchain/body work; a
fourth selected Java field appears; implementation needs a command option,
Java fragment, package/schema/DICE owner, retained store, public sibling-enum
export, fixture or another production file; enum/identity size grows; a Java
field returns a label/`None`, an authentic Java initializer/target/body
executes, the failed BUILD package publishes transient recorder state, or any
cap is exceeded.
