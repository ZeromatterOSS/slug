# Current Slug V2 Work Packet

Packet: WP-4-7A-build-imported-bzl-label-caller-provenance-audit-r1

Status: docs-only audit/design checkpoint. Commit `4d59d7451` terminally
accepts the Java configuration-field implementation. The bounded successor
below is frozen for independent review; Rust implementation is not authorized
before `ACCEPT`.

## Predecessor and selected boundary

Commit `4d59d7451` accepts
`WP-4-6-7A-java-configuration-field-declaration-fail-closed-implementation-r3`
at 75 production/205 proof/280 aggregate gross Rust additions. Its focused
native/loading/analysis proof, serial terminal validation, formatting, diff,
scope and archive-baseline gates pass. Authenticated bounded-PATH rules_rust
replay clears both selected Java `configuration_field` declarations and then
stops while loading selected rules_java 9.1.0 `//toolchains`:

```text
@@rules_java+//toolchains:default_java_toolchain.bzl:212
Label() may only be called in a .bzl module
```

`toolchains/BUILD` loads `java_runtime_files` at lines 9-15 and first calls it
at 102. The imported helper is defined at
`toolchains/default_java_toolchain.bzl:201-217`; `native.filegroup` at 204-208
mutates only the attempt recorder, its first Label string at 212 fails, and the
second at 213 is unreached. Package failure is atomic, so no target publishes.
The initializer-bearing `default_java_toolchain` call at BUILD line 365 is
later and remains deferred.

## Durable source authority

The selected BCR coordinate is `rules_java` 9.1.0. Descriptor
`https://bcr.bazel.build/modules/rules_java/9.1.0/source.json` hashes to
`da589573c1dee2c9ac4a568b301269a2e8191110ff0345c1a959fa7ea6c4dfd6` and
selects the 114,566-byte/114-entry release archive with SHA-256
`4e1a28a25c2efa53500c928d22ceffbc505dd95b335a2d025836a293b592212f`
and integrity `sha256-Thooolwu+lNQDJKNIs7/vFBd2VszWi0CWDaik7WSIS8=`.
Both relevant files are regular `0444` with trailing LF:

| Source-relative path | SHA-256 | Bytes/lines | Exact anchor |
|---|---|---:|---|
| `toolchains/BUILD` | `b23a9b08e5928120d2d3f3a559b9c54f8472cabf1a4b99baf7cc6f29886a9b73` | 15,306/430 | load 9-15; first helper call 102; later toolchain call 365 |
| `toolchains/default_java_toolchain.bzl` | `6f963992c933e6cbc48f0c64f3349484422ee06f01830473ea802731b874deea` | 9,335/219 | helper 201-217; filegroup 204-208; Labels 212/213 |

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is the compatibility authority:

- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleFunctionsApi.java:1031-1057`,
  SHA-256 `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`,
  specifies that `Label()` uses the package of the calling `.bzl` source;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleClassFunctions.java:2122-2143`,
  SHA-256 `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`,
  deliberately stack-inspects the innermost `.bzl` rather than embedding a
  context in each builtin copy;
- `src/main/java/com/google/devtools/build/lib/cmdline/BazelModuleContext.java:140-190`,
  SHA-256 `007811248fee3973fe6947ec37898f616dc6a282c4ae9b7b8a1ebeea6bb26bd4`,
  obtains the module of the innermost enclosing Starlark function and rejects
  when none exists; and
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleClassFunctionsTest.java:1799-1838`,
  SHA-256 `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`,
  covers Label construction, idempotence and package-relative identity; and
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkIntegrationTest.java:4338-4356`,
  SHA-256 `ced8fc27cbe35bf30174678800d29b73012f800bff00bcdff6a5cf8c78fef836`,
  directly proves that BUILD loading a `.bzl`-re-exported Label builtin still
  rejects its call. The selected authenticated replay supplies the positive
  BUILD-to-imported-def discriminator; add no oracle fixture.

Slug's starlark-rust evaluator already exposes the required inlining-safe
lexical evidence at
`starlark-rust/starlark/src/eval/runtime/evaluator.rs:545-578`, SHA-256
`a6e65891755ad76e142c62c25159bf03dc008c8d650b621b53b983c8e4bc16cd`:
`native_call_source_filename()` preserves the source containing the native
call after compiler inlining, while `native_caller_function_filename()` is the
fallback for an executing def. Existing Bzl evaluation already applies this
order in `BzlEvaluationContext::source_identity_for_call`.

## Decision and compatibility

Audit result: `ACCEPT`, pending independent review, for one generic lexical
category: a BUILD evaluation may call an imported frozen `.bzl` def whose body
invokes `Label(string)`, and Label resolution uses the source containing that
native call expression. The implementation must use
`native_call_source_filename().or_else(native_caller_function_filename)` in
that order—concretely,
`eval.native_call_source_filename().or_else(|| eval.native_caller_function_filename())`.
Reversing the order is forbidden because compiler inlining can erase the
physical def frame.

**Exact:** accept ordinary and Bzlmod BUILD calls into imported/re-exported
`.bzl` defs and resolve relative, apparent-repository and canonical-repository
labels against the innermost lexical call source's package and repository
mapping. Direct `.bzl` top-level calls and Label idempotence remain unchanged.
Direct BUILD `Label(string)` and a Label builtin merely aliased/re-exported
from `.bzl` remain rejected: their native call expression is in BUILD, not in
an executing `.bzl` def. Nested cross-module wrappers use the innermost source
that actually contains `Label()`, not the BUILD caller, outer wrapper or export
site. Freeze the direct re-exported-alias diagnostic exactly as:

```text
Label() can only be used during .bzl initialization (top-level evaluation)
```

Preserve atomic package failure and existing diagnostics for the other named
admitted cases.

**Slug-native:** use a filename-indexed immutable slice and fail closed on a
missing or ambiguous recursive-manifest filename. Its internal diagnostics and
sparse Rust representation are not Bazel byte-parity claims.

**Unsupported/deferred:** arbitrary Label availability at BUILD module scope;
caller inference from repository/package strings; symbolic-macro, initializer,
computed-default, rule-implementation or other configured callback expansion;
Java toolchain/provider/action semantics; and any source absent from the
already loaded recursive manifest. Do not special-case rules_java,
`java_runtime_files` or the selected strings.

## Ownership, lifetime and invalidation

`bzl_module.rs` composes the already loaded modules' first-seen recursive
manifest identities into a filename/`BzlModuleIdentity` slice.
`PackageRecorder` owns one attempt-only
`Option<Arc<[(CompactString, BzlModuleIdentity)]>>`: `None` means no imported
Bzl sources and allocates nothing; `Some` must be nonempty. The slice is
installed when each legacy or Host package attempt is constructed, borrowed
only during evaluation, and dropped on success, failure, glob retry,
cancellation or driver release. It never enters `PackageState`,
`PackageEvaluation`, `LoadedPackage`, a frozen module or a DICE value.

`starlark_label.rs` remains the sole Label resolution owner. Keep its existing
Bzl-context path unchanged; only if evaluator extra is a `PackageRecorder` may
it look up the inlining-safe lexical filename in that attempt slice and pass
the matched identity to existing `resolve_label`. Do not add `PackageRecorder`
to `BzlEvaluationContext::from_evaluator`, edit `provider.rs`, or make a global
Bzl context available during BUILD evaluation.

No new DICE key/input, graph edge, lock, await, cache, interner, map, raw
pointer or filesystem read is added. Existing source observations,
`BzlLoadManifest` equality/fingerprint and package-load dependencies own
revision and A/B/A invalidation. Overlapping requests share only immutable
loaded modules/Arc data; attempt recorders and failure state remain isolated.
Constant-time Arc clone and the existing `Allocative` coverage on manifest
identities remain; `PackageRecorder` is attempt scratch, not a new retained
Allocative owner. This is a cold loading boundary, so no benchmark is required.
Buck2/V1 supplies no code; Stage 9 records reuse of Slug's existing
Arc-slice/compact-identity pattern.

The large-file trigger is reviewed. `package.rs` (11,654 lines) is the sole
package-attempt/recorder owner and `bzl_module.rs` (11,424) is the sole loaded
manifest/evaluation driver; moving this two-owner bridge would duplicate their
private boundaries. Touched functions must remain below 150 lines.
`host_package_load_tests.rs` and `tests/build_file_loading.rs` are proof-only.
No split or unrelated cleanup is authorized.

## Frozen implementation successor

After independent `ACCEPT`, activate
`WP-4-7A-build-imported-bzl-label-caller-provenance-implementation-r1` with
exactly this allowlist:

- `app/slug_loading_v2/src/starlark_label.rs`;
- `app/slug_loading_v2/src/package.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`; and
- `app/slug_loading_v2/tests/build_file_loading.rs`.

Caps are 120 production, 260 proof and 380 aggregate gross Rust additions.

Proof must cover the rules_java-shaped helper order: the pre-Label filegroup
mutation, both Label strings resolving, final successful publication, and
source hash/line anchors. Add generic ordinary and external/Bzlmod cases for
relative, apparent and canonical labels; direct import, re-export and nested
cross-module wrappers; lexical-source rather than BUILD/outer/export identity;
an inlining-shaped small def; Label idempotence; direct BUILD and re-exported
builtin rejection; missing/ambiguous filename fail-closed behavior; and
unchanged direct `.bzl` evaluation. Prove `None` for no loads,
`Some(nonempty)` for loaded closures, source-order/first-seen deduplication,
attempt failure non-publication, retry isolation and same-DICE loaded-source
A/B/A restoration. Repository-mapping correctness is covered by the external
lexical apparent/canonical mapping discriminator; do not add a separate mapping
A/B/A test. Reuse inline scratch sources and accepted replay evidence; add no
fixture.

Run focused Starlark Label, legacy package and Host package tests plus the
named build-file-loading integration target; then serial full loading/query
suites, direct pinned-nightly CLI build, authenticated bounded-PATH replay,
stale-slugd, formatting, diff, archive, allowlist and cap checks. Replay must
clear both selected Labels and report only the next authentic boundary; it
must not claim Java initializer/configured semantics.

Return `REPLAN` if exact resolution cannot use the lexical call source through
compiler inlining; direct BUILD or an aliased builtin becomes callable; a
source outside the recursive manifest is inferred; filename ambiguity is
accepted; package failure publishes partial state; no-load attempts allocate;
the slice survives package evaluation or enters semantic equality; a new
DICE/provider/global-context owner, map/interner/cache/fixture, sixth file or
cap excess is required; or a touched function exceeds 150 lines.
