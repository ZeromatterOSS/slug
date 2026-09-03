# Current Slug V2 Work Packet

Packet: WP-4-7A-java-common-private-loading-facade-audit-r1

Status: docs-only audit/design checkpoint complete. The bounded implementation
successor below is frozen but awaits independent review; no Rust work is
authorized before `ACCEPT`.

## Predecessor and selected replay stop

Commit `60528af77` terminally accepts the provider declaration-identity closure
at 16 production/139 proof/155 total gross Rust additions. It admits exported
live initialized-provider identities and TemplateVariableInfo only in loading
declarations, while configured Template use still fails closed. Its accepted
focused and broad loading/query/CLI, formatting, diff, archive and daemon gates
pass.

The authenticated bounded-PATH rules_rust replay clears the selected
rules_java declarations and stops while loading
`@@rules_java+//java/private:native.bzl:19`:

```text
return java_common.internal_DO_NOT_USE()
Variable `java_common` not found
```

No Java provider initializer, configured rule implementation, toolchain
operation or action executes before this loading-global failure.

## Durable selected rules_java evidence and complete closure

The selected BCR coordinate is `rules_java` 9.1.0. Its durable descriptor is
`https://bcr.bazel.build/modules/rules_java/9.1.0/source.json`, SHA-256
`da589573c1dee2c9ac4a568b301269a2e8191110ff0345c1a959fa7ea6c4dfd6`.
It selects
`https://github.com/bazelbuild/rules_java/releases/download/9.1.0/rules_java-9.1.0.tar.gz`,
a 114,566-byte/114-entry archive with SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`
and integrity `sha256-Thooolwu+lNQDJKNIs7/vFBd2VszWi0CWDaik7WSIS8=`.
All six closure files below are regular `0444` with trailing LF:

| Source-relative path | SHA-256 | Bytes/lines | Selected role |
|---|---|---:|---|
| `java/private/native.bzl` | `81fd742661f632db4c6b36efa5acb76075e5d65d176ffa3372ed47b340bc9ae1` | 844/19 | lines 18-19 define the sole bridge and call `java_common.internal_DO_NOT_USE()` |
| `java/private/java_common.bzl` | `f40732378d4e0ae55646958c9ff17313e5f98cd8af5751deaadaa2d8437ddfe5` | 11,126/323 | lines 296-323 construct/export the public struct; line 313 is the sole eager internal-member call |
| `java/private/java_info.bzl` | `02438c92066a825629a47f6dd01d9ea2200dc90a666b68fb4ee1ebf09e6a3026` | 47,438/1,038 | four lazy member calls inside provider helpers/initializers |
| `java/private/java_common_internal.bzl` | `a1c0222c084b2110fcec953b74e782f3287d8edba816b1efe3a7eb5275b0e652` | 19,260/467 | seven lazy provider/action/classpath calls |
| `java/common/rules/java_toolchain.bzl` | `5ad6511cdef925246961c7e7a9039475c192371fedbf909c63cf92334779e875` | 24,304/612 | three lazy `expand_java_opts` calls |
| `java/common/rules/java_package_configuration.bzl` | `d0b7fd1e91158b9605a15bec544fa2b574f088336f77f2b857ed9692c38bfacd` | 3,475/124 | one lazy `expand_java_opts` call |

This is the complete matching closure: exactly 26 literal
`get_internal_java_common` occurrences across six files. Five are load imports;
the other 21 are definition/call-form occurrences comprising one bridge
definition plus 20 returned-member call sites using ten distinct selected
names. `java_common.bzl:293` is a legacy-only `check_provider_instances` call
unreachable when the default legacy flag is false. The remaining 19 reachable
member calls are one eager call at `java_common.bzl:313` and 18 lazy calls.
Two lazy calls, `java_info.bzl:177,677`, are exact admitted
`google_legacy_api_enabled() == False` queries; only the other 16 lazy calls
invoke deferred members. Enclosing JavaInfo/provider behavior remains deferred.
`google_legacy_api_enabled` therefore has exactly three syntactic calls.

The generated compatibility proxy is durably specified by
`java/rules_java_deps.bzl:29-58`, SHA-256
`40ce0f5b44b124f9fdc3986d542caa6b3a3213c2abbd4927cdea65ad42f31a23`,
8,257 bytes/224 lines. It loads the private Java common producer after the
runtime/toolchain modules. Public `java/common/java_common.bzl`, SHA-256
`2848c3ac4f305d2b2d4c96dc2daa2828b1031ee0c0764515ecdebc8f125bdb71`,
727 bytes/18 lines, re-exports that proxy value. `toolchains/BUILD` and
`toolchains/java_toolchain_alias.bzl` are the selected entry route; they hash
to `b23a9b08e5928120d2d3f3a559b9c54f8472cabf1a4b99baf7cc6f29886a9b73`
and `56f84699c33ebd2e871615b30bcab4ae5a824cfab78e0dd80afc7e1fbf92e510`.

## Bazel 9.2 authority and learned facts

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a` is
the sole compatibility authority:

- `src/main/starlark/builtins_bzl/common/java/java_common.bzl:17-42`,
  SHA-256
  `2083873bdc74038b46f7773277d66eb3dd80b5e73af75d78791e006bc3922cfb`,
  constructs an eleven-member private facade, checks its rules_java allowlist
  and exports `internal_DO_NOT_USE`;
- `src/main/java/com/google/devtools/build/lib/bazel/rules/JavaRules.java:63-65`,
  SHA-256
  `2819dd07d95cc6afb57a997683f71e9e6cd7019b90f527c42f84e4a7397f928c`,
  injects the Bzl top-level and native Java implementation;
- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/java/JavaCommonApi.java:645-690`,
  SHA-256
  `b701da2231e7f82080ee1991bd8d1f7c9a92bef0e3af272ad1a484a2e36eb824`,
  declares the private member APIs;
- `src/main/java/com/google/devtools/build/lib/rules/java/JavaStarlarkCommon.java:282-284,345-392`,
  SHA-256
  `a8bc7d5e1875978d60550088ab517a38430068730f680267054e8f0b847c2e92`,
  repeats the caller check at member invocation and reads the semantics flag;
- `src/main/java/com/google/devtools/build/lib/packages/semantics/BuildLanguageOptions.java:313-322,916`,
  SHA-256
  `b01e106ef0ff7af458766248bce7799b49c0f54fc14d023a8297aeb7dbfb44e5`,
  establishes `experimental_google_legacy_api=false` by default; and
- `src/main/java/com/google/devtools/build/lib/packages/BuiltinRestriction.java:34-91,124-149,169-216`,
  SHA-256
  `383c157100c35564c11ddcc270f7a2757c9f151f343ac0ce6253bffb5dfb5081`,
  establishes innermost-Bzl caller provenance and rules_java allowlisting.

`JavaStarlarkApiTest.java:1847-1886`, SHA-256
`56518ceb51aeb7297fafe7bcc95cb4b72c11b1da894be1b4485b6c51da0039b8`,
proves the internal Java members are private. Bazel's facade contains eleven
members; selected rules_java references ten, and
`incompatible_disable_non_executable_java_binary` is the sole unreferenced
member.

## Decision and compatibility boundary

Audit result: `ACCEPT`, pending independent review, for one bounded
**java_common private loading facade**.

Classify as **exact** for the admitted default configuration: expose
`java_common` only to ordinary/Bzlmod `.bzl` evaluation; admit zero-argument
`internal_DO_NOT_USE()` and zero-argument `google_legacy_api_enabled()`; apply
the same allocation-free package-local source-identity predicate at both
method calls for a selected canonical/apparent rules_java caller in package
`java` or a descendant; return `False`; survive freeze/import/re-export; and
omit the global from direct BUILD evaluation.

Classify as **Slug-native**: two zero-sized Rust Starlark values replace the
eleven-field Bazel struct, expose only the selected eager member, and use
Slug's canonical repository/source-manifest identity and standard missing-field
diagnostic for the sparse deferred surface. The narrow predicate deliberately
fails closed where Bazel is broader: `internal_DO_NOT_USE` calls from
rules_java packages outside `java/**`, Bazel's two main-repository allowlist
prefixes (`javatests/com/google/devtools/grok/kythe/analyzers/build/testdata/pkg`
and `third_party/bazel_rules/rules_java`), and any stricter direct top-level/
callerless case are unsupported Slug-native rejections, not exact parity claims.

Keep **unsupported/deferred**: `experimental_google_legacy_api=true`; the ten
other Bazel facade members and their 16 selected lazy calls; enclosing JavaInfo
and provider behavior even around the two admitted false queries; Java provider
payloads or initialization, target membership/indexing, fragments, configured
rule implementations, toolchain resolution, option expansion, classpath
processing and Java compilation/actions. Do not add inert placeholders.

## Ownership, lifetime and invalidation

`app/slug_loading_v2/src/package.rs` owns loading-global registration, both
stateless facade values and one allocation-free caller predicate. At both the
bridge and returned-member call, use
`BzlEvaluationContext::source_identity_for_call` and admit only a real Bzl
caller whose canonical/apparent repository identity is `rules_java` and whose
package is exactly `java` or its descendant. Reject direct top-level/callerless
calls. Do not reuse the broader `builtin_restriction` default allowlist. This
is exact for selected positive rules_java `java/**` callers and intentionally
Slug-native fail-closed for the broader cases above; no direct filesystem
lookup occurs.

Both values are zero-field `starlark_simple_value!` carriers. They may live in
the existing Globals/frozen-module heaps but retain no evaluator reference,
source identity, collection, callable, semantic marker or dynamic allocation.
Add no package/configured value, map, compact string, Arc slice, interner,
registry, cache, DICE key/input/observation, lock, await, retry or fixture.
Existing module source/recursive-manifest fingerprints provide invalidation;
request overlays, overlapping requests, equality cutoffs, cancellation and
shutdown behavior are unchanged. Buck2/V1 provides no implementation and no
Stage 9 row is needed. The package file exceeds the size trigger, but remains
the cohesive owner because this adds only two Bzl-global ZSTs beside existing
`platform_common`; splitting would add registration/module plumbing without a
new semantic owner.

## Frozen implementation successor

After independent `ACCEPT`, activate
`WP-4-7A-java-common-private-loading-facade-implementation-r1` and modify only:

- `app/slug_loading_v2/src/package.rs`, production owner plus adjacent
  zero-size proof; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, proof only.

Caps: 72 production Rust, 160 proof Rust, 232 aggregate gross additions.

Proof must cover the exact 844-byte/19-line `native.bzl` hash/source; the
selected `_make_java_common` eager call under ordinary and Bzlmod globals;
default `False` and absent legacy additions; zero arguments and rejection of
positional/named extras; caller rejection for direct top-level/callerless calls,
root, unrelated repositories and rules_java outside the admitted `java`
prefix; provenance recheck after freeze/import/re-export; BUILD absence;
zero-sized transient/frozen facade state; standard failure/absence for all ten
deferred members; both lazy false queries without executing enclosing provider
behavior; unchanged ordinary globals and no Java callback, implementation or
action execution. Treat the selected positive caller cases as exact and every
required broader/callerless negative as Slug-native fail-closed proof.

Run serial focused facade/recursive-load tests, then the full loading library
and integration targets, query library, direct pinned-nightly CLI rebuild,
exact authenticated bounded-PATH replay, stale-slugd, formatting, diff,
archive, allowlist and cap gates. Reuse the accepted archive/source evidence;
add no oracle fixture. Replay must clear line 313 and stop only at the next
independent typed boundary.

Return `REPLAN` if another internal member executes during loading; either
method cannot enforce caller provenance after import/re-export; configurable
legacy semantics, Java provider/configured/action ownership, another production
file, retained state, a new key/cache/fixture or more than the caps is required;
or any deferred member becomes reflectively present or silently succeeds.
