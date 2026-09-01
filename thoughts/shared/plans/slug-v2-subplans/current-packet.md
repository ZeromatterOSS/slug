# Current Slug V2 Packet

Packet: `WP-6-7A-bzlmod-declaration-selection-identity-parity-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 Bzlmod declaration
selection and repository-rule producer identity.

Status: implementation terminally `ACCEPTED`; commit pending. The design is
frozen and independently `ACCEPTED`. The first retained-
representation/public-ABI review returned `REVISE` because public visibility
alone does not exclude a public-named raw `load` binding in starlark-rust. The
focused correction requires assigned origin and public visibility together for
`use_repo_rule`; rereview returned `ACCEPT`. Commit `817d017b6` terminally
accepts the complete Bzlmod declaration-call signature category and is this
packet's Rust base. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and complete category boundary

Match Bazel 9.2's complete admitted Bzlmod declaration-selection category:

1. a `module_extension` selected by `use_extension` is found among globals
   actually assigned by the selected `.bzl` module, independent of underscore
   visibility. Direct private definitions, private/public aliases, and
   assignment-based reexports work; raw `load` bindings that were never
   assigned by that module remain absent;
2. a `repository_rule` called from a module-extension implementation may have
   a public or private first assignment. Later file-effect execution reacquires
   that exact defining-module binding at any visibility and still compares the
   complete retained producer projection before invoking its implementation;
3. `use_repo_rule` continues to select only a public assigned global, so a
   directly requested underscore/private name and a raw loaded binding fail.
   Public aliases and assignment-based reexports succeed while retaining the
   underlying repository rule's first-export defining label/name as the
   `RepoRuleId`; and
4. `tag_class` has no independent module selector. Its accepted call binding
   and embedding in the selected module-extension projection remain unchanged.

Every module-extension load, drift-reacquisition, and selected-owner invocation
path must use the same assigned-global capability. Missing/wrong-kind values,
load-only bindings, changed manifests/projections, unsupported extension
factors, and repository projection drift continue to fail closed. After
focused proof, rebuild the V2 CLI and run two daemon-clean
`cquery //app/slug_cli_v2:slug` replays. Both must pass the authentic rules_cc
0.2.17 private `_compatibility_proxy_repo_rule` boundary and stop identically
at the next unsupported boundary, or succeed. Do not consume that next
boundary in this packet.

This is generic Starlark module-binding and Bzlmod Host-selection architecture.
It adds no parser grammar, evaluator language value, set behavior, manual
builtin binder, rule body, repository operation, DICE key, configured-analysis
owner, action, execution, `cc_common`, `cc_internal`, rules_cc, or C++ branch.
Bazel 9 BCR Starlark remains the rule-body owner.

## Learned facts and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources and SHA-256 values are:

- `StarlarkRepositoryModule.java`
  `c6adf0f521e56419ec22e7980def6b27778bab4d5c5294b3556c2286f5b6bcea`;
- `BzlLoadFunction.java`
  `bc3efe6f47de9c0a4f8a5a865972575ace4594564c4851d64edea37184fdf2cc`;
- `RegularRunnableExtension.java`
  `1c91439270aef8dcd1d4615dd40369e51431cebaee36f8dc085a51a9d0aead20`;
- `InnateRunnableExtension.java`
  `1d2d87a071281f20b9be4253cd92ed007c6c3915ca117214e7ce3d720861698e`;
- net.starlark `Module.java`
  `d8893ba4f6beea6c12a997122873591a5cb917b31bd3b18a33f2eddb6a9d3e49`;
- `RunfilesRepoMappingManifestTest.java`
  `8df1c7f6cc4558fe35405f43e7130ffc4f0588f41e75f18709adf520146545df`;
  and
- `ModuleExtensionResolutionTest.java`
  `d8602fd385d34ab5387cb0ef3891ef9acc0ca62cd8f67324e09fd33ea7a3e769`.

`BzlLoadFunction.execAndExport` attaches first-assignment identity to every
`StarlarkExportable`, including underscore bindings. `StarlarkRepoRule` stores
that first rule name. `RegularRunnableExtension` selects a
`Module.getGlobal(extensionName)` with no underscore/public-visibility gate.
`InnateRunnableExtension` instead rejects requested names beginning `_`, then
selects `Module.getGlobal(ruleName)` and uses the selected rule's own
`RepoRule`; it does not require the requested alias to equal the rule's first
export label/name. net.starlark `Module.getGlobals` contains globals assigned
by that module and excludes unassigned load bindings.

The pinned runfiles test executes an underscore-bound `_deps_repo` inside a
module extension. The pinned module-extension resolution test separately proves
that `use_repo_rule(..., "_data_repo")` is rejected. These are complementary,
not contradictory, visibility surfaces.

A disposable Bazel 9.2 oracle additionally proved all discriminating rows:

- private and public repository rules invoked inside one extension both
  generated queryable repositories;
- direct private `use_repo_rule` failed with the expected does-not-export
  message shape;
- public `use_repo_rule` aliases to both first-private and first-public rules
  succeeded;
- direct private, private/public alias, and assignment-reexported
  `module_extension` selections succeeded;
- raw public/private load bindings failed for both Bzlmod selectors; and
- private/public extension reexports and a public repository-rule reexport
  from another `.bzl` succeeded.

Authentic rules_cc 0.2.17 `cc/extensions.bzl`
`a190a467ac48329a76e1a9ccab1fea53519af4bb2202e22346b23fc24dcf9872`
declares and invokes `_compatibility_proxy_repo_rule` privately. Current Slug
passes its positional `module_extension` declaration after `817d017b6`, then
fails only because `authenticate_rule` calls public-only `FrozenModule::get`.

## Compatibility classification

**Exact:** assigned-global versus raw-load selection for `module_extension`;
private/public/alias/reexport shapes above; internal repository-rule
reacquisition; public-only `use_repo_rule` selection; first-export repository
producer identity; missing/wrong-kind/load-only/projection-drift rejection; and
unchanged declaration-call signatures.

**Slug-native:** Rust/starlark-rust diagnostic wording where no exact text is
claimed, Rust Unicode, the compact frozen-name representation, DICE key bytes,
and existing structural publication identity.

**Unsupported/deferred:** `repository_rule.remotable`, unadmitted
repository/module-context operations, exact Java exception text, module
extension environment/OS/architecture/facts execution, later BCR loading and
configured-analysis failures, and action/execution breadth. No classification
is widened by this packet.

## Frozen architecture and natural owners

The adopted starlark-rust parser and evaluator remain the sole language
implementation. Add one general, hidden `FrozenModule::get_assigned(name)`
capability; do not parse source, scan AST text, add a Slug side table, infer
underscores, or use `get_any_visibility` for Bzlmod selectors.

`MutableNames` owns an evaluation-scratch packed assignment bit per module slot.
Ordinary top-level assignment stores, including modify/reassignment, mark the
slot; `load` stores and `import_public_symbols` do not. Freeze folds that bit
into each existing compact `FrozenNames` entry. The retained tuple must not
grow in `size_of`; add a layout assertion. `Module::set` is assigned, while the
existing private import helper stays unassigned. `get_assigned` returns the
assigned value together with its unchanged `Visibility`, and treats load-only
bindings as absent. It is a hidden origin-aware lookup, not a change to public
module exports.

All four module-extension selection/reacquisition call sites in `bzl_module.rs`
and `module_extension.rs` consume `get_assigned` and accept either returned
visibility. Repository file-effect authentication alone uses existing
`get_any_visibility` because its retained projection already names the exact
defining module and first-export binding; complete projection equality prevents
imported/alias substitution. `use_repo_rule` also selects through
`get_assigned`, then requires the returned visibility to be `Public`; this
public-and-assigned conjunction rejects both underscore/private definitions and
public-named raw load bindings. Its selected value is the authentication
boundary: remove only the false requirement that the selected alias's request
label/name equal the underlying first-export producer identity. The two-load
stable-projection comparison and all type checks remain.

No new DICE key, cache, interner, registry, global state, hash, collection
graph, task, lock, or fallback is admitted. Existing source/manifests own
invalidation; existing frozen-module heaps own values; existing selected
definition/owner keys own request-local selection and structural cutoffs. The
mutable packed bit vector is evaluator scratch released at freeze. The folded
bit is DICE-retained semantic module metadata and borrows no evaluator heap.
Cancellation, publication, eviction, and shutdown remain the existing module
load/key lifecycles.

## Buck2 and Zabel guidance

Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` remains the
starlark-rust donor. Its compact `SmallMap` name/slot table and generated
assignment stores are retained. Its current `FrozenModule` exposes public
lookup while private/imported bindings share one visibility state, so it has no
assigned-only selector to reuse verbatim. The bounded V2 addition is a leaf
module-environment capability; do not fork parser grammar, bytecode evaluation,
hashing, slot identity, or module storage. No V1 extraction applies.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance,
not truth. `module_extension_declaration_host.zig`
`7474d1ddb37d2ffaa0006b4ce3b19df3917bb6dce055c4db87363fcf50067600`
binds one producer `.bzl` label/name to a repository-rule declaration and
preserves it through aliases; `module_extension_execution_capture.zig`
`8f03505b2302f79443d3ab95f12cbca2b65eec8a417ff94e739fb9fafcd06fc0`
retains that pair in invocation rows. Adopt only the producer-identity and
module-owner concepts. Copy no Zig code, allocator, row layout, evaluator,
selector behavior, scheduler, cache, errors, or compatibility claim.

## Allowlist, caps, validation and stops

Production allowlist:

- `starlark-rust/starlark/src/environment/names.rs`;
- `starlark-rust/starlark/src/environment/modules.rs`;
- `starlark-rust/starlark/src/eval/runtime/evaluator.rs`;
- `starlark-rust/starlark/src/eval/compiler/module.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/module_extension.rs`;
- `app/slug_loading_v2/src/module_extension_innate_repository.rs`; and
- `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`.

Proof stays inline in those files. Scheduling/status edits may touch this
manifest, canonical Live Status, Stage 6, and Stage 9. No fixture is committed;
the disposable Bazel oracle is already recorded. No routing-log row is needed
unless review changes the route.

Caps are 90 net / 120 gross starlark-rust production lines, 30 net / 45 gross
Slug production lines, 260 net / 330 gross proof lines, and 420 gross total
Rust lines. The large Slug files remain cohesive because only their existing
selector/reacquisition call sites and owning tests change; do not add another
central module or refactor unrelated history.

Validate serially with the focused starlark assigned-global test; focused
module-extension selection, innate alias, and private file-effect tests; full
`starlark` and `slug_loading_v2`; direct `slug_query_v2` and
`slug_analysis_v2` dependents; rebuilt `slug_cli_v2`; two daemon-clean real
bootstrap replays; `cargo fmt --all -- --check`; Cargo metadata;
`scripts/v2_archive_status.sh`; `git diff --check`; cap/layout accounting; and
parked-file SHA-256 verification. Clean stale `slugd` before and after replays.

`REPLAN` before changing parser grammar, Starlark visibility/load behavior,
bytecode value semantics beyond the assignment-origin bit, public
`FrozenModule::get`, a DICE key, module-load ownership, retained producer
identity fields, repository effects, an unsupported declaration parameter, an
action/rule/provider owner, any ruleset/C++ branch, or a cap. Independent
retained-representation/public-ABI review is required before Rust and
independent terminal review before acceptance and commit.

## Implementation candidate evidence

The candidate keeps the frozen architecture. `MutableNames` owns one packed
assignment-origin bit per existing slot; ordinary assignment stores and
`Module::set` mark it, while the dedicated load store and
`import_public_symbols` do not. Freeze folds the bit into the existing name
entry, and a compile-time assertion proves the retained tuple remains the same
size. One hidden origin-aware lookup serves all four module-extension sites and
the public-and-assigned `use_repo_rule` gate. Internal repository-rule
reacquisition alone uses any-visibility lookup with full projection equality.
No parser, set, DICE, ruleset, action, `cc_common`, `cc_internal`, C++, Cargo,
fixture, or public-export lookup changed.

Focused assigned/load/import/modify/visibility, module-extension
private/alias/reexport/load-only, innate first-producer alias, and private
repository-effect tests pass. The complete `slug_loading_v2` owner passes
462/462 with one pre-existing ignored fixture test; complete `slug_query_v2`
and `slug_analysis_v2` plus integration and doc tests pass. The complete
starlark library command is 808/838 with exactly the same 30 pre-existing
profile/function-name, bytecode source-span/name, and struct-JSON-order
failures as detached clean design base `2cbd2042b` at 807/837; the sole added
test passes.
The V2 CLI rebuild passes. Two daemon-clean authentic
`cquery //app/slug_cli_v2:slug` replays both pass the prior private rules_cc
repository-rule boundary and stop at the same next unsupported category:
rules_rust `attr.label_keyed_string_dict(doc=...)` declaration breadth.

Accounting is 78 net / 118 gross starlark-rust production, -1 net / 27 gross
Slug production, 202 net / 226 gross proof, and 371 gross Rust total. The
parked registration proof remains unchanged at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.
Independent terminal review returned `ACCEPT`: assignment origin remains
scratch-only until the unchanged-size frozen fold, every selector uses the
correct origin/visibility conjunction, and repository producer identity stays
projection-authenticated. The only residual risk is the separate rules_rust
attribute-constructor breadth frontier exposed by both replays.

## Immediate predecessor

Commit `817d017b6` terminally accepts exact first-parameter binding across
`repository_rule`, `module_extension`, and `tag_class`. It changes only the two
mismatched generated-binder annotations plus category proof, passes full owner
and downstream validation, and advances the real BCR replay from the old
named-only error to the private repository-rule selection boundary.
