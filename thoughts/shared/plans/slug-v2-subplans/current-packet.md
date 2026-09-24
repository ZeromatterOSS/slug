# Current Slug V2 Work Packet

Packet: WP-7-91-m7a-selected-extension-tag-list-concat-r1
Status: accepted; independent design and final reviews ACCEPT

## Outcome and compatibility

Advance the authentic root `cquery` through the selected rules_rust 0.73.0
`crate` extension's `mod.tags.from_cargo + mod.tags.from_specs` expression
at `crate_universe/extensions.bzl:1129` (repeated at 1149). Both operands
are Slug's immutable invocation tag-list views. Return a fresh, mutable
ordinary Starlark list with left tags then right tags, without mutating the
source views. Report the next honest root terminal, not M7A closure.

Exact: the selected two-tag-list `+` result's order, length, truth, fresh
mutability and unchanged immutable source views; rejection of `+=` on a
source view and list-plus-tuple refusal. Slug-native:
tag-object allocation/identity and the vendored index-error wording remain
the accepted Rust invocation view,
not Bazel's JVM object identity. Unsupported/deferred: other `+=` and
operator families, and modules exceeding a conservative 2^20 total-tag
preflight cap (Bazel's list-concat cap is 2^30 elements). Do not widen
this into blanket list parity.

## Evidence and semantic owner

WP790 at `fc3319670` passed the real selected path and checksum-pinned
download. The fresh root stops at rules_rust `extensions.bzl:1129`:
`for cfg in mod.tags.from_cargo + mod.tags.from_specs:`. Root
`MODULE.bazel` supplies one `from_cargo` and zero `from_specs` tags; the
same expression appears at line 1149. The next likely call is
`module_ctx.watch(cfg.cargo_lockfile)` at line 1153, but only a fresh
smoke establishes the actual next terminal.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkBazelModule.java:115-160` constructs immutable tag-class lists;
`EvalUtils.java:78-111` dispatches `list + list` to `StarlarkList.concat`;
`StarlarkList.java:208-230` allocates a fresh mutable list and copies
left then right with a 2^30 cap; `EvaluationTest.java:345-382,501-506`
tests order, new-result mutation and list-plus-tuple refusal. These
pinned sources/tests plus the authentic rules_rust call are the evidence;
add a Bazel oracle only for a demonstrated gap.

Slug's `InvocationTags::get_attr` currently returns a custom
`InvocationTagList` backed by `Arc<[InvocationTag]>` for each class. It
advertises Starlark `list` and supports indexing/length/iteration, but
vendored native-list concat only accepts concrete `ListRef`. Do not add
only `StarlarkValue::add` to this custom type: vendored `add_assign`
would then make `+=` on Bazel-immutable tag views succeed. Instead keep
the retained `Arc` source, materialize each class access as vendored
native `AllocImmutableList` on the invocation heap, and let vendored
list `+` produce a fresh mutable list while native immutable-list `+=`
rejects. Preflight each module's total tag count at 2^20 in both
`owner_invocation_modules` and at the start of `invoke_pure_preflight`
before its legacy batch evaluates any module;
a pair of class views then lies safely below Bazel's 2^30 concat cap
and cannot reach vendored unchecked length addition.
Counts above that conservative Slug boundary fail closed before events,
calls or repository publication. The heap list is evaluation-local;
do not add a retained cache, interner or duplicated semantic store.

## Scope, stops and validation

Base clean `main` at `fc3319670`. Root owns serial Cargo, CLI build,
smoke, review and commit. Production/test allowlist:
`app/slug_loading_v2/src/module_extension.rs` and colocated tests.
Scheduling allowlist: this manifest and the canonical Current paragraph.
No vendored Starlark, DICE, Bzlmod, repository, archive, download,
action, REAPI, MODULE or lockfile changes. The Arc-backed prepared/tag
ownership remains; only the evaluation-local list value changes. The
Buck2 utility skill requires checking clone/allocation cost and keeping
the retained Arc rather than introducing a new graph collection. No
performance or memory-reduction claim is made.

Obtain independent design ACCEPT before Rust edits and independent final
review of the actual diff. Focused tests must exercise the real selected
external `.bzl` route with one left tag and an empty right list, a
two-sided nonempty order case, result freshness/mutability without source
mutation, source immutability including `+=` refusal, list-plus-tuple
refusal, and synthetic 2^20 bound checks covering both invocation
entry points, including an oversized later legacy module that fails
before earlier modules can publish events or calls. Preflight exact
nonignored selectors, then run them and direct dependents. Build
`slug_cli_v2` before fresh authentic root smoke. Clean stale `slugd`
before and after. The smoke must pass the list-concat stop and report
the next one. Run `cargo fmt --all -- --check`, `git diff --check`,
archive status and plan status. Preserve ordinary Bazel RC discovery;
never inspect, copy or print `~/.bazelrc` authentication contents.

Stop and `REPLAN` if the selected operands cannot be native immutable
list values over the existing Arc-owned tags, or the result cannot be
fresh and mutable without changing retained semantic ownership. Diagnose normal
compiler/test/smoke failures within the accepted contract. Keep
`module_ctx.watch` and later generator execution for a successor.

## Immediate predecessor

WP790 at `fc3319670` admitted selected logical path and verified
generator artifact Need/replay, with independent design and final
ACCEPT. The real root now reaches tag-list concatenation.

## Validation receipt

Pinned Bazel 9.2 and authentic rules_rust source reviewed. Independent
design review ACCEPTED the revised native immutable-list and two-entry
preflight contract before Rust edits. Pinned-nightly offline/locked
Loading lib tests compiled; exact nonignored selector preflight found
all new and direct-dependent tests. The native-list unit test, synthetic
2^20 boundary test, legacy later-module entry test, selected external
`.bzl` route/entry test, prior negative-index test and selected
download Need/replay test passed 6/6. All 24
`module_extension::tests` passed. The prior forbidden-ABI test had stale
assertions that WP790 `path`/`download` and earlier `Label.workspace_root`
were absent; its assertions now check their admitted scoped behavior.
The full Loading suite is not green: current test binary has 644 pass,
39 fail, one ignored, versus the pre-change WP790 binary's 639 pass,
40 fail, one ignored, with no new failing test names. The failures are
outside this module-extension test group; no full-suite success claim.

Fresh `slug_cli_v2` build passed. Authentic root `cquery` (exit 2)
passed both occurrences of the tag-list addition and stopped at
rules_rust `extensions.bzl:1153`: `module_ctx` lacks `watch` for
`cfg.cargo_lockfile`. This is a separate deferred file-watch effect,
not configured-root success. No stale `slugd` was present before or
after. `cargo fmt --all -- --check`, `git diff --check`, archive
status and plan status passed. Independent final review ACCEPTED the
actual corrected diff and test evidence. The Buck2 utility review retained the existing Arc
ownership and used vendored ephemeral immutable lists; no donor import,
new cache/interner, or performance/memory claim.
