# Current Slug V2 Packet

Packet: `WP-4-7A-post-utils-eager-values-parent-import-frontier-audit`

Milestone: M7A command/ruleset bootstrap closure.

Result: audit the exact rules_rust `utils.bzl` child-to-`rust.bzl` boundary
after the accepted eager-value proof, authenticate the fifteen parent-imported
function exports and their compiler/freeze closure, and select one bounded next
proof or record `REPLAN`.

## Accepted base and exact frontier

Base is `adde01290` (`Prove exact rules rust utils eager values`). It embeds
five separately hashed, unabridged source slices totaling 124 lines from the
authenticated 1,032-line rules_rust 0.73.0 `rust/private/utils.bzl`, SHA-256
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`.
Under exact producer `@@rules_rust+//rust/private:utils.bzl`, the proof freezes:

- the exact ordered six-string `UNSUPPORTED_FEATURES` list;
- `_FORCE_DISABLE_CC_TOOLCHAIN = False`;
- the exact ordered 31 encodings and derived 63 substitutions;
- pointer-identical `substitutions_for_testing`; and
- pointer-identical `encode_raw_string_for_testing` as a frozen function.

The exact lines 692-740 `_replace_all` body is present solely because the lazy
encode function resolves that global during compilation/freeze. No utility was
invoked, and the proof does not establish the complete utils module.

The authenticated parent `rust/private/rust.bzl` is 1,821 lines, SHA-256
`a645bd5db6344bd3c0997dcf73600475c0af53fb4dd025890be24b8e1e2dbfd8`.
Its exact lines 40-57, SHA-256
`1ad3406b7c58cc7d74e1e86991fdb6aeadbd836d32926fc54eee9583295ab500`,
import these fifteen names in this order:

`can_build_metadata`, `can_use_metadata_for_pipelining`, `compute_crate_name`,
`crate_root_src`, `dedent`, `deduplicate`, `determine_lib_name`,
`determine_output_hash`, `expand_dict_value_locations`, `find_toolchain`,
`generate_output_diagnostics`, `get_edition`, `transform_deps`,
`transform_link_deps`, `transform_sources`.

None of those fifteen parent bindings is accepted merely by the eager proof.
Do not return to parent line 59 until a bounded child export proof is accepted.

## Authorities and compatibility discipline

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, its pinned
`ResolverTest` global/load-closure regressions and `function.star` frozen-state
regression, plus the authenticated rules_rust source are sole exact behavior
authority. Audit compilation/freeze separately from function invocation.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is architectural guidance only.
Use its defining-module reachability traversal to check that selected exported
functions retain every referenced load/global needed after evaluator closure.
Copy no Zig code, representation, owner pointer, ordering/capture algorithm,
diagnostic, identity or behavior.

- **Exact:** accepted five-slice bytes, eager values/aliases, authenticated
  utils/parent hashes, exact parent load slice, names and order, and any bounded
  source-derived function export selected by this audit.
- **Slug-native:** Rust frozen-value representation and audit documentation.
- **Unsupported/deferred:** complete utils freeze; invocation/results/diagnostics
  of every utility; configured toolchain/action/allocator behavior; parent line
  59 onward; and later rules_rust source.

The Buck2 utility review selects no action because this packet is docs-only and
changes no retained data structure, hash, compact collection/string, interner,
clone path, graph storage or memory accounting.

## Allowlist, audit and caps

Only these files may change:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md` only for a
  genuinely reusable/unusual routing decision or `REPLAN`.

Caps are 0 production and 0 proof additions. Documentation growth must remain
bounded to the authenticated audit result.

Required audit:

1. Resume from the accepted eager slices inside utils; do not silently treat
   the whole 1,032-line child as frozen or skip directly to parent line 59.
2. Map the fifteen parent-imported functions to their exact definitions in
   utils and inventory every load, global, helper and eager composite referenced
   by their bodies. Distinguish direct from transitive compiler/freeze closure.
3. Account for accepted child bindings and eager aliases without duplicating
   them. Identify the smallest source-complete subset that can compile/freeze
   one coherent parent-needed export family without invoking a helper.
4. Check the pinned Bazel resolver/frozen-closure evidence and applicable
   rules_rust tests. Reuse accepted evidence; require a new oracle only for a
   demonstrated observable gap.
5. Classify exact, Slug-native and deferred surfaces, including proof-only
   projections, function identity and all invocation/configured behavior.
6. Select one bounded implementation/proof packet with explicit allowlist,
   caps, validation and stops, or record `REPLAN` if no bounded Rust-native
   slice exists.

Request/revision, DICE, retained-memory, async ownership, fixture growth and
hot-path measurement are inapplicable: this docs-only audit changes no runtime
key, request, allocation, fixture or measured path. Any selected successor must
re-evaluate the applicable checklist items for its actual surface.

## Validation and STOP

Run `git diff --check`, verify only allowlisted documentation changed, and run
`scripts/v2_archive_status.sh` with only its three known archive-only misses.
Independent terminal review must verify the accepted/eager boundary, exact
fifteen-name parent load, transitive compiler/freeze audit requirement,
compatibility classes, bounded successor, Zabel guidance-only role and scope.

STOP and `REPLAN` for Rust changes, utility invocation, an implicit whole-file
claim, skipped compiler dependency, parent-body work, configured/toolchain/
action semantics, Java/JVM work, copied Zabel content or dirty authority.

## Immediate predecessor

`adde01290` accepted the five exact eager-value/dependency slices with 227 unit,
24 invalidation and 31 BUILD-loading tests green. Independent review verified
hashes, ordered values, alias identity and non-invocation under the packet caps.
