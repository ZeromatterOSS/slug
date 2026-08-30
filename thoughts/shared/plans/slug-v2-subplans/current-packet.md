# Current Slug V2 Packet

Packet: `WP-4-5-7A-module-extension-native-existing-rules-family`

Milestone: M7A bootstrap-critical repository/ruleset closure.

Base: accepted repository-rule attribute family `0c3a172ed`, accepted generated
repository routing `f747507f6`, and accepted verbatim Bazel repository package
`3023718a0`. Selected-context, configured-analysis, registration, REAPI, and
the pre-existing `package.rs` definition-source changes remain dirty, parked,
and read-only.

## Immediate predecessor and observable result

Commit `0c3a172ed` accepts all thirteen Bazel 9.2 public repository-rule
attribute kinds, ordinary/innate convergence, recursive ordered-map
publication identity, full owner suites, two fresh rules_rust replays, and
independent terminal review. Both valid fresh-root replays advance beyond the
former `attr.string_list_dict` rejection and stop identically at:

```text
Object of type `native` has no attribute `existing_rule`
@@bazel_tools//tools/build_defs/repo:utils.bzl:318
```

Implement the complete module-extension-context family together:

- `native.existing_rule(name)` returns `None`; and
- `native.existing_rules()` returns an empty ordinary Starlark dictionary.

Two fresh copied-workspace/fresh-output-root rules_rust cqueries must advance
beyond `utils.bzl:318` and stop identically at the next authentic unsupported
boundary or succeed. The successor is scheduling evidence, not authority to
widen this packet.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is sole
semantic authority:

- `StarlarkNativeModule.ExistingRulesShouldBeNoOp` marks module-extension
  evaluation specifically so both methods avoid package-rule introspection.
- `StarlarkNativeModule.existingRule` returns `Starlark.NONE` when that marker
  is present; `existingRules` returns `Dict.empty()`.
- `ModuleExtensionResolutionTest.nativeExistingRuleIsEmpty` proves a module
  extension can branch on empty `native.existing_rules()` and create its
  repository.
- `NativeExistingRulesTest` covers BUILD/finalizer rule snapshots, attribute
  conversion, select round trips, immutability, dict-like behavior, and JSON.
  Those tests are deliberately skipped because this packet admits only the
  distinct module-extension no-op context.

The real rules_rust workspace is stronger downstream evidence for the exact
`native.existing_rule` use in verbatim `@bazel_tools` content. Add no fixture.
Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` has no
corresponding builtin implementation; it remains concept-only guidance and
supplies no code or semantic claim.

## Decision, compatibility, and non-decisions

Use the existing loading `NativeModule` and the existing evaluator-local
`RepositoryRuleInvocationState` marker. Add one private context predicate and
register both methods on the shared native object. The methods return no
retained value and inspect no package, repository, filesystem, command, or
DICE state.

- **Exact:** method presence and arity in loaded `.bzl` code; during ordinary
  and selected module-extension invocation, `existing_rule(any string)` is
  `None`, `existing_rules()` is an empty dict, and conditionals observe those
  values exactly.
- **Slug-native:** the private Rust marker/predicate and error wording for use
  outside the admitted context.
- **Unsupported/deferred:** BUILD/legacy-macro/finalizer rule snapshots;
  symbolic-macro restrictions; dict-like immutable rule views; attribute,
  label, selector, computed-default, JSON and kwargs projection; repository
  context methods; parser or `set` work; `cc_common`, `cc_internal`, rules_cc,
  and C++ rule/action semantics; the next replay boundary.

Do not return empty values in BUILD evaluation: unavailable BUILD/finalizer
semantics fail closed. Do not add a package recorder scan, snapshot, side
registry, evaluator mode enum, command repair, or ruleset special case.

## Ownership, request/revision, and memory

The invocation owner already installs `RepositoryRuleInvocationState` as the
evaluator extra for both ordinary prepared invocations and the selected-owner
path. The native methods borrow that evaluator-local marker synchronously and
return evaluator-heap values. No marker or result crosses evaluation, enters a
call receipt, or changes DICE equality/invalidation.

No immutable request projection, filesystem observation, final validation,
overlapping-session policy, cache, async task, cancellation, join, shutdown,
or eviction boundary changes. All memory is evaluator scratch. Existing
repository-rule call receipts remain the sole retained invocation output.

## Proof matrix

Colocated tests must prove:

1. a module-extension implementation observes `existing_rule("missing") ==
   None`, `len(existing_rules()) == 0`, false membership, and an immutable
   empty dict result;
2. branching on both values invokes a repository rule and retains the expected
   call, proving the methods execute in the authentic invocation context;
3. invalid arity/type follows the Starlark method boundary and creates no
   partial repository call;
4. top-level `.bzl` and BUILD evaluation do not receive a fabricated empty
   snapshot and fail closed; and
5. both ordinary prepared and selected-owner module-extension paths remain
   covered by their existing invocation suites.

Reuse the real rules_rust workspace for two fresh replays. No checked-in oracle,
copied registry subtree, mutation, manifest, or expected-output file is added.

## Allowlist, caps, complexity, and stops

Exact implementation allowlist:

- `app/slug_loading_v2/src/package.rs` — only the two native method entries;
- `app/slug_loading_v2/src/module_extension_repository_rule.rs` — private
  marker predicate and colocated invocation/context proofs; and
- scheduling documents only for packet activation/closure.

The live `package.rs` contains the parked definition-source diff; preserve it
byte-for-byte and stage only this packet's native-method hunk. All other dirty
files are excluded. Cap net Rust production growth at 35 lines, tests at 100,
and total at 135. No new file, crate, dependency, unsafe code, public type,
DICE key, lock, cache, fallback, or fixture.

`package.rs` remains the cohesive native-method registry despite its size; two
context-gated methods belong beside the existing `native` surface and moving
them would split method registration from its value owner. Add no new touched
function above 150 lines; keep the predicate and tests bounded. This is not a
hot-path or retained-memory change.

Validate serially:

1. focused native/repository-invocation tests and direct selected-owner tests;
2. `cargo test -p slug_loading_v2`;
3. `cargo build -p slug_cli_v2`;
4. clean stale `slugd`, run two fresh real rules_rust cqueries with Slug's
   supported Starlark-label projection, and clean `slugd` afterward;
5. `cargo fmt --all -- --check`, `git diff --check`, exact allowlist/caps, and
   parked `package.rs` isolation; and
6. `scripts/v2_archive_status.sh` plus root terminal review.

`REPLAN` before widening if module-extension context cannot be authenticated
from the existing evaluator marker, the two invocation paths use different
owners, a BUILD snapshot is required to pass the real use, either method must
publish retained state, the dirty `package.rs` hunk overlaps, caps fail, or the
next replay boundary is needed to make this pair observable.
