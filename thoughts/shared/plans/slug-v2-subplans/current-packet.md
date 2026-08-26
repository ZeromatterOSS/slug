# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-empty-list-freeze-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: existing private C++ bridge and starlark-rust frozen empty list
Base: `152caa6f`

Result: admit the exact empty-list row of `cc_internal.freeze`, allowing
rules_cc to construct and freeze `EMPTY_COMPILATION_OUTPUTS`. Stop before
claiming non-empty iterable or dictionary copies, or any configured C++
semantics.

## Accepted starting point and source-order stop

Commit `152caa6f` accepts documented string-to-string schemas on the existing
loading-only initialized-provider owner. This completes the source-shaped
`CcInfo` and `CcLauncherInfo` declarations, followed by the direct provider
declarations in the shared-library hint and LTO children. Focused proof, all
205 loading units, configured analysis, locked checks, rebuilt CLI and hygiene
pass. Independent terminal review returned `ACCEPT`.

The first absent expression is rules_cc 0.2.17
`cc/private/compile/cc_compilation_outputs.bzl:86`:

```starlark
objects = _cc_internal.freeze(objects),
```

This runs inside the top-level
`EMPTY_COMPILATION_OUTPUTS = create_compilation_outputs_internal()` call. All
ten `freeze` arguments on that path are the function's default empty lists.
The other fields are already admitted `None`, the accepted empty LTO provider,
and `wrap_with_check_private_api(depset([]))`, whose wrapper body stays lazy.

Accepting the exact empty-list row therefore completes the top-level empty
compilation-output provider. Stop immediately after proving that source-shaped
row and audit the next recursively loaded child of
`cc/private/cc_common.bzl`, `cc/private/compile/compile.bzl`, separately. Do
not infer that its lazy compilation methods or later C++ children are
implemented.

## Fixed sources and compatibility authority

Reuse the accepted rules_rust/rules_cc materialization. Relevant fixed inputs:

- `cc/private/compile/cc_compilation_outputs.bzl` SHA-256
  `294e3da16da4444122e7dee058ec1e06b30cec93d64a32f217cf9e1e3e4bfb44`;
- `cc/private/compile/lto_compilation_context.bzl` SHA-256
  `a17435cd56fa165c71081e99f9af73407f7b4cc1dc086e53771dcf74df81b3f4`;
- `cc/common/cc_helper_internal.bzl` SHA-256
  `793ab429f8e397df9c486f4c3c7b5c57fae81c8432ba6d08189d65d75676dae1`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is sole behavior authority.
`CcStarlarkInternal.freeze` returns `Dict.immutableCopyOf` for dictionaries,
`StarlarkList.immutableCopyOf` for other Java iterables, and the original value
otherwise. The selected row is narrower but exact: an empty Starlark list
produces an immutable empty Starlark list. Mutation of the result fails, its
type remains `list`, and the caller's empty source has no elements to alias.
Pinned source is sufficient evidence for this source-exercised row; no new JVM
artifact enters Slug.

## Zabel and Buck2 architectural guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is architectural and test guidance only. Its `freezeCall` delegates to one
evaluator-owned `immutableCopyOfContainerOrIterable` boundary, and its tests
separate source mutation, result immutability, list/tuple conversion and
dictionary preservation. Slug follows the same ownership principle for the
selected empty row: the result comes from the evaluator's frozen heap rather
than borrowing the mutable input. No Zig code, representation, allocator,
diagnostic or behavior is copied. Bazel remains compatibility authority.

The Buck2/starlark-rust reuse audit selects the retained `AllocList::EMPTY`
and `Evaluator::frozen_heap()` path already used by empty HeaderInfo. This is a
statically shared immutable empty list with existing tracing, freezing,
equality, iteration, memory accounting and list methods. Add no `Vec`, map,
interner, registry, side store, custom immutable wrapper or dependency. No
Stage 9 ledger row is required because the representation is unchanged.

## Compatibility classification

- **Exact:** rules_cc-owned `cc_internal.freeze([])` accepts one positional or
  named empty built-in list and returns an immutable empty value whose Starlark
  type is `list`; all ten empty-list calls in
  `create_compilation_outputs_internal()` succeed; the resulting documented
  `CcCompilationOutputsInfo` freezes with the expected empty list fields,
  empty LTO context, lazy temps callback and `None` info files.
- **Slug-native:** Rust valid-Unicode diagnostics and reuse of starlark-rust's
  static frozen empty list are native implementation choices.
- **Unsupported/deferred:** non-empty lists; tuples, sets, ranges and other
  iterables; dictionaries; scalar/pass-through values; nested mutable values;
  full `cc_internal.freeze`; configured C++ providers/actions; invoking the
  lazy temps callback; `compile.bzl` and later rules_cc/rules_rust source; M8,
  M7B and exact output bytes.

## Ownership, lifetime and implementation boundary

Add one method to the existing opaque `CcInternalModule`. It must first prove
the argument is a built-in mutable or frozen list and that its length is zero,
then return `eval.frozen_heap().alloc(AllocList::EMPTY).to_value()`. The
evaluator/module frozen heap owns the result for the complete evaluation and
frozen-module lifetime. Do not retain the input or create a second owner.

Reject every unselected shape before allocation. Do not use `Freezer` during
evaluation, forward or freeze the caller's mutable value, construct a custom
list/dict, or alter starlark-rust. `BzlModuleEvalKey` and recursive source
observations remain the sole invalidation owner. There is no DICE, request,
command, async, cache, publication, cancellation or shutdown change.

## Discriminating proof

- Evaluate a rules_cc-shaped `create_compilation_outputs_internal()` with all
  ten empty-list defaults and freeze its documented provider instance.
- Prove every list field is empty and type `list`, the LTO field survives, the
  temps field is a lazy function, and the two info-file fields are `None`.
- Prove mutation of `cc_internal.freeze([])` fails during evaluation.
- Accept positional and named empty-list calls, including an already-frozen
  empty list observation when practical.
- Reject non-empty list, tuple, dictionary, scalar, missing and extra argument
  shapes without widening configured-provider admission.
- Keep documented initialized-provider, empty HeaderInfo and configured
  provider regressions green.

## Allowlist and caps

Only these files may change from base `152caa6f`:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/cc_common.rs` | `8d3bd46908dc3c536cf545644ffcabc8e7cf84a9b4b1002b0e1b76d01212202e` | 165 | 190 | exact empty-list freeze method |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `ea4c17169fb4d95da5373c89035216c5fdbb2e7f72053415031862c28749125e` | 5,757 | 5,855 | source-shaped and boundary proof |

Production additions are capped at 20, proof additions at 90 and total
additions at 110. Deletions do not buy addition budget. No new or touched
function may exceed 120 lines. The test file exceeds the 2,000-line trigger,
but this proof belongs beside the existing private C++ bridge, HeaderInfo and
documented-provider regressions sharing `eval_bzl_with_identity`; splitting it
would widen `lib.rs` and the allowlist.

Plan-only selection edits are limited to the canonical plan, Stage 4 subplan
and this manifest and are excluded from implementation caps.

## Serial validation

Use `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target` and
`CARGO_BUILD_JOBS=1`:

- focused empty-list freeze/empty compilation outputs test;
- existing documented initialized-provider and empty HeaderInfo tests;
- one configured provider analysis regression;
- `cargo test -p slug_loading_v2 --lib`;
- `cargo check --locked -p slug_analysis_v2 -p slug_core_v2`;
- `cargo build -p slug_cli_v2` after Rust changes;
- `cargo fmt --all -- --check`;
- `git diff --check`;
- `scripts/v2_archive_status.sh`.

The broad daemon-sensitive loading integration remains 30/31 only for its
known stale `@external` diagnostic-order row and need not rerun unless focused
evidence exposes integration risk. Recheck base hashes, caps, allowlist,
function sizes, configured-analysis non-widening and the clean Zabel pin.

The retained frozen-container boundary requires independent selection and
terminal reviews. Both must verify Bazel authority, Zabel's guidance-only role,
reuse of the existing starlark-rust representation, fail-closed non-empty and
dictionary behavior, compatibility classes and every cap.

## STOP / REPLAN

STOP and REPLAN for a file outside the implementation allowlist; a
starlark-rust, analysis/build-api or DICE edit; non-empty iterable, dictionary
or scalar admission; a custom container or second owner; configured C++
lowering; invoking the temps callback; another C++ method; source/mapping/
materializer/network/fixture change; Java/JVM work; copied Zabel code or
behavior; cap violation; or a claim beyond the top-level empty compilation
outputs row. Once that provider freezes, audit `compile.bzl` separately.
