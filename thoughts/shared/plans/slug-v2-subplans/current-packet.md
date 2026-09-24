# Current Slug V2 Work Packet

Packet: WP-7-110-m7a-module-extension-label-caller-context-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit `Label()` in a loaded `.bzl` function invoked by a selected Bzlmod
module extension, including a transitive imported helper. Authenticated
rules_swift 3.1.2 `swift/extensions.bzl:17-21` calls its imported
`swift/repositories.bzl:112-121` helper, which supplies
`Label('//third_party:com_github_apple_swift_protobuf/BUILD.overlay')` as
`http_archive(build_file = ...)`. This must resolve in the rules_swift
repository from the helper's defining `.bzl` module, not the extension
entrypoint, root module or generated repository. The rebuilt normalized-PATH
authentic `slug build //:probe` must pass the current top-level-only
`Label()` error at toolchain registration row 56; a later independently
typed stop is acceptable. HTTP archive execution, Swift toolchain semantics,
target success and M7A close are not claimed.

Exact Bazel 9.2 slice: during a loaded `.bzl` function call from a selected
module-extension implementation, `Label(string)` uses the innermost
executing `.bzl` module's package and repository mapping; an existing Label
input is returned unchanged even without source-context lookup. Relative
`:`, same-repository `//`, apparent `@` and canonical `@@` forms retain their
already admitted Label grammar.
Slug-native: the existing `BzlLoadManifest`/`BzlEvaluationContext` Rust
source carrier, lexical Starlark source lookup, DICE observation equality and
error text. Unsupported/deferred: direct MODULE or non-`.bzl` invocation
contexts, unmanifested source frames, broader module-extension effects,
repository materialization and configured Swift behavior.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkRuleClassFunctions.java:2121-2140` passes an
existing Label through before context lookup, then selects the innermost
calling `.bzl` module for a string. `BazelModuleContext.java:163-185,206-208`
supplies that module context or fails. `Label.java:210-245` resolves source
repository/package and apparent
mapping, with canonical spelling bypassing mapping;
`ModuleExtensionResolutionTest.java:740-792` checks Label construction in
module-extension invocation with its repository mapping. The authenticated
rules_swift source is the selected case, but its entrypoint and imported
helper share package `swift` and its `//third_party` label cannot alone
discriminate wrong source selection. An offline Bazel 9.2 scratch oracle at
`/tmp/slug-wp810-imported-label-Lc6aZI` imports a helper from a different
repository/package with its own `@alias` mapping. Two fresh output bases
query the generated BUILD: the helper produces
`@@producer+//owner:local`, `@@producer+//third_party:overlay` and
`@@dep+//mapped:target`, while direct entrypoint `Label(':caller')` produces
`@@//ext:caller`. Direct `Label()` in BUILD and MODULE is unbound in
separate negative queries. Scratch stays outside this checkout and proves
no downstream Slug repository effects or configured Swift semantics.

`app/slug_loading_v2/src/starlark_label.rs::Label` and its resolver already
own grammar, idempotence and source-specific mapping. The selected
`module_extension.rs` invocation currently installs
`module_extension_repository_rule::RepositoryRuleInvocationState` as the
evaluator extra, so the existing `BzlEvaluationContext::from_evaluator`
cannot see the already reacquired `FrozenBzlModule.manifest`, and fallback
`PackageRecorder` reports the top-level-only error. Nest a manifest-derived
`BzlEvaluationContext` in that invocation-only state, then add a dedicated
extension-state branch only in `starlark_label.rs::Label` to obtain its
source identity. Do not expose this context through generic
`BzlEvaluationContext::from_evaluator`: doing so would admit unrelated
Bzl-only declarations and constructors in extension execution. Reuse the
existing `source_identity_for_call` and `resolve_label` methods unchanged.
Require an actual `.bzl` caller frame before using the extension-runtime
context for a string; the existing context resolver otherwise defaults to
its root source when no filename exists. Direct `Label()` inside the
extension implementation's
loaded `.bzl` function is valid, while no-frame native callbacks remain
outside the admitted string slice. Wire both selected-owner retries and the
pure preflight invocation route, carrying the reacquired manifest through
`PurePreflight` as invocation scratch. Do not
derive package/mapping from the extension root label, call-site location,
root module or filesystem, and do not add a new resolver, DICE key, retained
call projection, mutable global context or lock. The existing recursive
manifest source digests/mappings and selected-owner observed source/route
dependencies own source changes, Need/error, retry and A/B/A restoration;
the context is synchronous evaluator scratch and drops before DICE awaits.
The pure-preflight manifest clone is immutable transfer-owned async scratch
from the already retained Arc-backed carrier: it may span preflight awaits,
but never evaluator execution across an await or publication in a new DICE
value. The accepted repository-rule Label runtime-context pattern in Stage 4
is the precedent, not a donor fallback.
Manifest cloning shares its reachable/direct-child Arc slices; the existing
context constructor copies one flat source-filename table in
O(reachable sources) per invocation attempt and drops it with the evaluator,
rather than retaining a parallel index or scanning the filesystem.

## Evidence and validation

Add a focused invocation test with a root entrypoint `.bzl` and an imported
helper in another repository/package with a deliberately different `@alias`
mapping; have the helper call `Label()` for `:`, `//`, apparent and canonical
labels and pass typed results to a repository-rule call record. Assert the
helper's defining package/mapping, Label-input
idempotence, ordered records and no successful owner publication on invalid
mapping or an unmanifested caller; a diagnostic `current_calls` prefix may
remain after a later error. Include a no-frame string fail-closed control.
In the selected owner DICE route, mutate the loaded helper source A/B/A and
assert call identity/restoration;
cover the pure preflight route if the selected test does not execute it.
Reuse the accepted repository-rule Label context evidence and existing
module-extension source fixtures; add a new Stage 1 fixture only for a
demonstrated gap. Compile Loading library tests with pinned Cargo JSON
`--no-run`, preflight exact nonignored selectors, run selected tests serially,
rebuild `slug_cli_v2` before the authentic smoke and check/clean `slugd`
before and after. Run format, archive, diff and plan-status checks; obtain
independent DESIGN and FINAL reviews. No broad suite or remote-service gate
is selected, and no `~/.bazelrc` contents, BuildBuddy credentials,
NativeLink or remote transport enter this Loading packet.

## Scope and stops

Base clean main `1171116a5`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 module-extension Label owner contract after acceptance;
production `app/slug_loading_v2/src/module_extension_repository_rule.rs`,
`app/slug_loading_v2/src/module_extension.rs` and
`app/slug_loading_v2/src/starlark_label.rs`; focused tests in those modules,
including new `app/slug_loading_v2/src/module_extension_label_tests.rs`.
Production growth under
roughly 45 lines and proof under roughly 180 lines are review triggers, not
permission ceilings. REPLAN if Bazel binds Label to extension entrypoint
rather than the innermost imported function, the manifest lacks that
function's source/mapping, selected replay cannot authenticate its source,
new retained semantic identity is required, or the authentic stop cannot
clear without unrelated repository effects.

## Acceptance receipt

Independent DESIGN and FINAL reviews each returned ACCEPT. Pinned Bazel 9.2
source and two fresh offline output bases at
`/tmp/slug-wp810-imported-label-Lc6aZI` establish imported-helper package,
repository and apparent mapping. The final pinned-nightly Cargo JSON Loading
library `--no-run` succeeds with zero compiler errors; selector preflight
verifies six exact nonignored tests and all six pass serially (four WP810,
two protected). The selected-owner test changes the imported helper source
A/B/A in one DICE graph and proves receipt equality/restoration; the pure
route has a successful typed Label receipt and a failed mapping terminal
without a successful invocation publication. Formatting, diff, archive,
plan-status and pre/post-smoke `slugd` checks pass.
The proof exceeds the packet's approximate 180-line review trigger because
it separately exercises direct imported-source context, pure preflight and
selected-owner invalidation; all three independent FINAL reviews accepted
that scope.

The CLI was rebuilt with `cargo build -p slug_cli_v2` before the normalized-
PATH authentic one-shot smoke. That smoke clears the prior rules_swift
registration-row-56 `Label()` error and stops later at
`@@rules_swift+//swift/internal:target_triples.bzl:21` because
`apple_common.platform` is absent. This is a separate Loading surface, not
proof of configured Swift or repository materialization. The focused test
run is under one second; the final JSON compile took about seven seconds
and the CLI rebuild about sixteen seconds. The packet took roughly eighteen
minutes elapsed including design review, implementation and final review.
No remote transport or credentials were used.

## Immediate predecessor

WP809 accepted at `1171116a5`: frozen top-level `.bzl` two-part
string-list selectors now reuse the ordered BUILD-attribute coercer under a
frozen-only kind/nested-value gate. Five focused tests and the rebuilt
authentic smoke passed its Kotlin stop and reached this independent
rules_swift `Label()` module-extension context error at registration row 56.
M7A remains partial.
