# Plan 54: depset and transitive_set shared core

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Related: [Plan 51](./51-slugd-memory-profiling.md)

## Status: COMPLETE

## Progress Notes

### 2026-05-09 follow-up: BazelOutput path blocker cleared

- Plan 54 remains complete. The Plan 44 declared-output path slice did not
  weaken depset validation or the deferred TransitiveSet streaming behavior.
- Focused verification passed:
  `pytest -q tests/core/analysis/test_ctx_actions.py::test_actions_declare_file_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_file_external_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_directory_bazel_path_shape tests/core/analysis/test_ctx_actions.py::test_actions_declare_directory_external_bazel_path_shape`,
  `pytest -q tests/core/analysis/test_cmd_args.py::test_args_add_all_map_each_sequence_returns tests/core/analysis/test_cmd_args.py::test_args_add_joined_map_each_sequence_returns tests/core/analysis/test_cmd_args.py::test_args_depset_add_all_transforms tests/core/analysis/test_cmd_args.py::test_args_depset_add_joined_transforms`,
  `cargo test -p slug_build_api_tests --lib interpreter::rule_defs::cmd_args::tests::map_each_sequence_returns_expand_as_items`,
  `cargo build -p slug`, and `git diff --check`.
- Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  bash -o pipefail -c 'timeout 220s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep '\''slugd\[zeromatter\].*plan44-bazel-output-path-1'\'' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan44-bazel-output-path-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan44-bazel-output-path-1.log'
  ```

  Result: Slug exited status `3` after 158s with
  `memory_smoke_summary elapsed_s=158 peak_rss_kib=802136 final_rss_kib=644308`.
  The previous glibc `select_file` path blocker did not recur. The run advanced
  to a new Plan 15 blocker in rules_cc:
  `rules_cc+0.2.17/cc/private/link/cpp_link_action.bzl:127` rejects
  `object_files = object_files + additional_object_files` because the operands
  are `tuple` and `list`.
- Current blocker is not Plan 54. Continue in Plan 15 with systemic
  Bazel-compatible Starlark sequence addition behavior.

### 2026-05-09 follow-up: cmd_args map_each tuple blocker cleared

- Plan 54 remains complete. The focused Plan 15 work preserved depset
  validation and the deferred depset/TransitiveSet streaming behavior; transformed
  `cmd_args.add_all`/`add_joined` still materialize values because Bazel-style
  `map_each` callbacks must run at analysis time.
- Focused verification for the current Plan 15 slice passed:
  `cargo fmt -- app/slug_build_api/src/interpreter/rule_defs/cmd_args/typ.rs app/slug_build_api_tests/src/interpreter/rule_defs/cmd_args/tests.rs`,
  `cargo test -p slug_build_api_tests map_each_sequence_returns_expand_as_items -- --nocapture`,
  `pytest -q tests/core/analysis/test_cmd_args.py -k map_each`,
  `cargo build -p slug`, and `git diff --check`.
- Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  bash -o pipefail -c 'timeout 220s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep '\''slugd\[zeromatter\].*plan15-map-each-seq-1'\'' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan15-map-each-seq-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan15-map-each-seq-1.log'
  ```

  Result: Slug exited status `3` after 190s with
  `memory_smoke_summary elapsed_s=190 peak_rss_kib=891896 final_rss_kib=668632`.
  The previous rules_cc `tuple (repr: ())` map_each blocker did not recur.
  The run advanced to a new Plan 15 blocker in
  `bazel_skylib+1.9.0/rules/select_file.bzl:36`: `llvm+0.7.0//runtimes/glibc:libc.s`
  cannot find the requested file among `generate_glibc_stubs` generated outputs.
- Current blocker is not Plan 54. Continue in Plan 15 with systemic generated
  artifact/path selection parity for `select_file`, avoiding target-name
  special cases.

### 2026-05-09 follow-up: provider-callable key blocker cleared

- Plan 54 remains complete. The depset/transitive_set validation and streaming
  behavior were preserved; the latest focused work did not weaken depset
  validation.
- Focused verification for the current Plan 15 slice passed:
  `cargo fmt -- app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`,
  `cargo test -p slug_build_api_tests provider_collection_contains_native_provider_keys -- --nocapture`,
  `cargo test -p slug_build_api_tests provider_collection_contains_methods_and_in_operator -- --nocapture`,
  `cargo test -p slug_build_api_tests test_schema_provider_missing_fields_are_absent -- --nocapture`,
  `pytest -q tests/core/configurations/test_configuration_dep_uquery_correctness.py`,
  `pytest -q tests/core/configurations/transition/test_attr.py`,
  `cargo build -p slug`, and `git diff --check`.
- Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  bash -o pipefail -c 'timeout 220s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep '\''slugd\[zeromatter\].*plan15-provider-callable-1'\'' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan15-provider-callable-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan15-provider-callable-1.log'
  ```

  Result: Slug exited status `3` after 184s with
  `memory_smoke_summary elapsed_s=184 peak_rss_kib=902404 final_rss_kib=710988`.
  The previous `AnalysisTestResultInfo ... got function` provider-key blocker
  at `with_cfg/private/transitioning_alias.bzl:55` did not recur. The run
  advanced to a new Plan 15 blocker in rules_cc command-line handling:
  `rules_cc+0.2.17/cc/private/rules_impl/cc_static_library.bzl:174`
  rejects `tuple (repr: ())` from `actions.args().add_all(..., map_each = ...)`.
- Current blocker is not Plan 54. Continue in Plan 15 with
  Bazel-compatible `cmd_args.add_all(map_each=...)` sequence return handling.

### 2026-05-09 follow-up: Plan 15 toolchain label blocker cleared

- Plan 54 remains complete. The depset/transitive_set provider blockers and the
  configured-node wait did not recur in the latest smoke.
- Focused verification for the current Plan 15 slice passed:
  `cargo fmt -- app/slug_build_api/src/interpreter/rule_defs/context.rs`,
  `cargo test -p slug_build_api toolchain_type_lookup --lib`,
  `cargo test -p slug_analysis test_normalize_constraint_label --lib`,
  `cargo build -p slug`, and `git diff --check`.
- Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  bash -o pipefail -c 'timeout 220s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep '\''slugd\[zeromatter\].*plan15-toolchain-label-canon-1'\'' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan15-toolchain-label-canon-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan15-toolchain-label-canon-1.log'
  ```

  Result: Slug exited status `3` after 164s with
  `memory_smoke_summary elapsed_s=164 peak_rss_kib=785084 final_rss_kib=605488`.
  The previous
  `Toolchain type '@@rules_rust+0.69.0//rust:toolchain_type' was not resolved`
  blocker did not recur. The run advanced into C++ toolchain analysis and failed
  through `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain`.
- Current blocker is not Plan 54. Continue in Plan 15 with the
  `with_cfg.bzl+0.12.0/with_cfg/private/transitioning_alias.bzl:51`
  `ctx.attr.exports[0]` provider collection indexing error:
  `provider collection operation [] parameter type must be a provider type ...
  got int`.

### 2026-05-09 follow-up: provider blocker cleared, configured wait narrowed

- Inspected `/tmp/plan54-provider-frozen-fields-1.log`: the previous
  `rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl:118`
  `LibraryToLinkInfo` depset mutability blocker did not recur, and the run
  reached `zeromatter//sdk:sdk_contents` analysis.
- Added gated diagnostics for the post-provider frontier:
  `app/slug_analysis/src/analysis/calculation.rs` now logs analysis dep
  request phases, `app/slug_analysis/src/analysis/env.rs` serializes/logs
  eager registered-toolchain loading, and `app/slug_configured/src/nodes.rs`
  logs configured-node/gather-deps phases under `SLUG_MEMORY_CHECKPOINTS=1`.
- Fresh bounded smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  bash -o pipefail -c 'timeout 180s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep '\''slugd\[zeromatter\].*plan54-configured-gather-probe-1'\'' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan54-configured-gather-probe-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan54-configured-gather-probe-1.log'
  ```

  Result: Slug exited status `3` after 158s with
  `memory_smoke_summary elapsed_s=158 peak_rss_kib=653752 final_rss_kib=578572`.
  The configured-node wait is cleared enough to expose analysis deps:
  `zeromatter//sdk:sdk_contents` logged `analysis_dep_request_complete` for
  `zeromatter//sdk:sdk_with_configs` and failed later through
  `rules_rust//ffi/rs:empty_allocator_libraries`.
- Current blocker is no longer Plan 54 depset/provider mutability. The failing
  message is:

  ```text
  Toolchain type '@@rules_rust+0.69.0//rust:toolchain_type' was not resolved.
  Ensure the toolchain is registered via register_toolchains() and the rule declares it in toolchains=[...]
  ```

  This should continue in Plan 15's Bazel 9 toolchain resolution work,
  specifically Rust toolchain type label canonicalization/registration parity
  for `ctx.toolchains[Label("//rust:toolchain_type")]`.

### 2026-05-09 follow-up: hashable dict freeze verified, next allocator LTL blocker

- Verified the current `_cc_internal.freeze` finish slice where frozen dicts
  remain dict-shaped while becoming immutable/hashable. This preserves
  `.keys()`, `.get()`, truthiness, iteration, membership, `dict.update`, and
  depset membership for the focused cc_common e2e.
- Hygiene/verification before the zeromatter smoke:
  `git diff --check` passed. Earlier focused verification for this local patch
  had already passed: `cargo fmt`, `cargo build -p slug`,
  `pytest -q tests/core/cc_common/test_link.py -k frozen_dict_depset_element`,
  and
  `cargo test -p slug_build_api_tests depset_validation_matches_bazel_9_1_0_probe -- --nocapture`.
- Bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  timeout 220s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/scripts/memory_smoke.sh \
      --interval 5 \
      --include-pgrep 'slugd\[zeromatter\].*plan54-hashable-dict-freeze-1' \
      -- \
      /var/mnt/dev/slug/target/debug/slug \
        --isolation-dir plan54-hashable-dict-freeze-1 \
        build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan54-hashable-dict-freeze-1.log
  ```

  The command exited with Slug status `3` after 179s
  (`memory_smoke_summary elapsed_s=179 peak_rss_kib=771808 final_rss_kib=611132`).
  The previous
  `create_library_to_link.bzl:106 Object of type tuple has no attribute keys`
  blocker did not recur, confirming the dict-shaped freeze behavior advanced
  the smoke beyond that point.
- The new concrete blocker is still a depset/provider hashability shape, now
  in `rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl:118`.
  `make_libstd_and_allocator_ccinfo` creates depsets of `_ltl(...)` values;
  `_ltl` calls `cc_common.create_library_to_link(static_library = library,
  pic_static_library = library)`. Slug rejects those direct `LibraryToLink`
  provider elements with `depset elements must not be mutable values` while
  analyzing
  `rules_rust+rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain`.
  Next slice should identify which `LibraryToLinkInfo` field remains
  non-hashable/mutable instead of weakening depset validation.

### 2026-05-09 follow-up: LibraryToLink dict field immutability fixed

- Inspected the `LibraryToLink` path exposed by `zstd//:zstd`: rules_cc
  freezes list/dict fields in `create_library_to_link.bzl` before
  `create_linking_context_from_compilation_outputs.bzl` wraps
  `cc_linking_outputs.library_to_link` in `depset([..])`.
- Slug's `_cc_internal.freeze` already normalized lists/tuples to tuples, but
  still returned dicts unchanged. That left provider/library fields with
  mutable dict values and preserved the depset mutable-value rejection.
- `app/slug_build_api/src/interpreter/rule_defs/cc_common/actions.rs` now
  recursively normalizes dicts as immutable, hashable dict-shaped values at
  the existing cc freeze boundary. The depset mutable-value rejection was not
  weakened, and TransitiveSet streaming behavior was untouched.
- Focused verification passed:
  `cargo fmt`;
  `cargo check -p slug_build_api`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo build -p slug`;
  `pytest -q tests/core/cc_common/test_link.py`;
  `git diff --check`.
- Bounded fresh zeromatter smoke:
  `/tmp/plan54-library-dict-freeze-1.log`, isolation
  `plan54-library-dict-freeze-1`, ran from `/var/mnt/dev/zeromatter` with
  `timeout 180s env SLUG_MEMORY_CHECKPOINTS=1 ... build //sdk:sdk_contents`.
  The old `cc_linking_outputs.library_to_link` depset mutability failure did
  not recur. The run reached `zeromatter//sdk:sdk_contents` analysis and timed
  out at the already-tracked
  `rules_rust//ffi/rs:empty_allocator_libraries` analysis wait. It also logged
  a non-terminal `llvm+llvm_source+llvm-raw` `http_bsdtar_archive`
  `rctx.execute([rctx.path(host_bsdtar)] + args)` `No such file or directory`
  repository-rule failure before creating a stub.

### 2026-05-09 follow-up: LibraryToLink mutability exposed by zstd

- A fresh zeromatter `//sdk:sdk_contents` smoke after the lockfile spoke
  pre-seed guard advanced past the stale `@@zstd//:` generated BUILD error
  and reached `zstd//:zstd`.
- The new blocker is another Plan 54 immutability shape:
  `rules_cc+0.2.17/cc/private/link/create_linking_context_from_compilation_outputs.bzl`
  constructs `depset([cc_linking_outputs.library_to_link])`, and Slug still
  reports `depset elements must not be mutable values` for that
  `LibraryToLink` value.
- This differs from the completed `LinkerInput` slice: the failing value is
  `cc_linking_outputs.library_to_link` returned by
  `cc_common.create_linking_context_from_compilation_outputs` while analyzing
  `zstd//:zstd`. Next subagent should inspect the Rust/Starlark
  representation of `LibraryToLink` and freeze/normalize any list/dict fields
  at construction boundaries without weakening depset mutable-value rejection
  or changing TransitiveSet streaming behavior.
- Evidence: `/tmp/plan15-lockfile-preseed-zstd-1.log`, isolation
  `plan15-lockfile-preseed-zstd-1`, peak RSS about 599 MiB, final RSS about
  516 MiB.

### 2026-05-09 cc provider immutability slice

- Investigated the zeromatter `//sdk:sdk_contents` failure in
  `rules_cc+0.2.17//:link_extra_lib` where
  `cc_common.create_linking_context(linker_inputs = depset([linker_input]))`
  failed with `depset elements must not be mutable values`.
- Confirmed the Bazel/rules_cc source shape: rules_cc's
  `create_linker_input.bzl` freezes provider fields via
  `_cc_internal.freeze(...)` before constructing `_LinkerInputInfo`; Slug's
  no-op `cc_internal.freeze` left list-valued fields mutable, so hashable
  provider-like values still failed depset membership.
- Implemented recursive tuple freezing for `_cc_internal.freeze`, normalized
  native `cc_common.create_linker_input`,
  `cc_common.create_library_to_link`, `create_compilation_outputs`, and the
  NativeShim linking paths to store immutable sequence fields, and adjusted
  `cc_common.link` to consume tuple/list/depset user link flags instead of
  requiring a depset-only shape.
- Added focused coverage for a `LinkerInput` with nested `user_link_flags`
  being accepted as a direct depset element and then passed through
  `cc_common.create_linking_context`.
- Verification passed:
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `pytest -q tests/core/cc_common/test_link.py -k 'linker_input or linking_contexts or link_deps_statically'`;
  `git diff --check`.
- Bounded zeromatter reruns now pass the old mutable-`LinkerInput` frontier.
  `/tmp/plan54-cc-provider-immutability-3-memory.log` fails later in the
  real LLVM/glibc C++ toolchain dependency chain on rules_cc's
  `implementation_deps` gate:
  `fail: requires --experimental_cc_implementation_deps`.

### 2026-05-09 cc implementation_deps flag/parity follow-up

- Investigated the `implementation_deps` gate exposed after the Plan 54
  immutability fix. Bazel source of truth:
  `src/main/java/com/google/devtools/build/lib/rules/cpp/CppOptions.java`
  still defines `--experimental_cc_implementation_deps`, but Bazel 9 defaults
  it to `true`; `CppConfiguration.java` exposes the value to Starlark as
  `ctx.fragments.cpp.experimental_cc_implementation_deps()`.
- Implemented Slug plumbing instead of special-casing rules_cc:
  `.bazelrc`/CLI normalization now preserves this Starlark-visible
  experimental flag, clap accepts both
  `--experimental_cc_implementation_deps` and
  `--noexperimental_cc_implementation_deps` with last-one-wins behavior, the
  client context carries the resolved boolean, server build config stores it,
  and `CppFragment` returns it. Default is now Bazel 9 parity: enabled.
- Focused verification passed:
  `cargo fmt`;
  `cargo test -p slug_client_ctx experimental_cc_implementation_deps --lib`;
  `cargo test -p slug_client_ctx bazelrc --lib`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.
- Bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

  ```sh
  timeout 180s env SLUG_MEMORY_CHECKPOINTS=1 \
    /var/mnt/dev/slug/target/debug/slug \
      --isolation-dir plan54-cc-implementation-deps-1 \
      build //sdk:sdk_contents \
    2>&1 | tee /tmp/plan54-cc-implementation-deps-1.log
  ```

  The old `requires --experimental_cc_implementation_deps` failure is gone.
  The run reached `zeromatter//sdk:sdk_contents` analysis, logged a rules_rs
  module extension failure while executing
  `rules_rs+override/rs/private/toml2json.bzl:6`
  (`ctx.execute([Label(toml2json), toml_file])`, `No such file or directory`),
  then stayed in the daemon wait loop until the 180s timeout. The command
  pipeline masked the timeout through `tee`; the remaining daemon for
  `plan54-cc-implementation-deps-1` was killed with `slug --nobazelrc ... kill`.

### 2026-05-09 Phase 7 final cleanup

- Treated Phase 6 as closed with documented limits: simple safe depset args
  are lazy; frozen File depsets reached from those args have narrow
  `ArtifactGroup::Depset` support; transformed `add_all`/`add_joined` depset
  forms intentionally flatten per Bazel 9.1.0 probes; live depsets
  intentionally flatten because they lack a durable `AnalysisValueStorage`
  owner/key; and `ArtifactGroup::TransitiveSetProjection` remains tset-only.
- Confirmed `DepsetWithListGen` remains deleted and active app/test scans have
  no `collect_depset_elements` or `request_value::<.*Depset>` bridge callers.
- Confirmed there is no active fallback that treats arbitrary Starlark values
  as depset-like by scraping `.direct`/`.transitive`.
- Removed the remaining broad action-input fallback that treated any value with
  a `to_list` attribute as depset-like. Action inputs/tools and touched
  cc_common helpers now branch on the depset facade before flattening depsets.
- Refreshed negative parity test wording for `.order`, `len(depset)`, and
  `depset | depset`; these tests remain negative Bazel 9 parity coverage, not
  Slug-only positive behavior.
- Updated the parent plan bridge section to point here and to document the
  current explicit, Slug-only, lossy bridge semantics. Refreshed the older
  rule-primitives plan note that still described depset as a transitive_set
  alias.
- User docs did not need depset edits; the user-facing docs only compare
  Slug/Buck transitive sets to Bazel depset at a high level.
- Verified:
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_set -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/analysis/test_cmd_args.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `git diff --check`.

### 2026-05-08 Phase 0 first slice

- Probed installed Bazel 9.1.0 for depset public surface, `None`
  constructor arguments, default/preorder/postorder/topological flattening,
  direct duplicate suppression, same-type errors, mutability/hashability
  errors, and order mismatch errors.
- Added a small shared `NestedSetOrder` scaffold under
  `app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`; kept
  `TransitiveSetOrdering` separate and only added a mapping for the common
  preorder/postorder/topological variants.
- Tightened depset behavior without making it a `TransitiveSet` alias:
  removed Starlark-visible `.direct`, `.transitive`, and `.order`; made
  `len(depset)` and `depset | depset` fail; accepted `direct = None`;
  stopped default-order inference from transitive children; changed default
  flattening to the Bazel 9.1.0-observed postorder-like behavior; added
  topological traversal for the documented diamond shape; added direct
  element hashability/mutability and top-level same-type validation.
- Added focused Rust parity tests in
  `app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs` for surface,
  validation, ordering, and frozen/live crossing. These are currently blocked
  by unrelated `slug_build_api_tests` compile errors in stale
  `ActionsRegistry::register(...)` call sites before the depset tests run.
- Rewrote `tests/core/analysis/test_depset_order.py` expectations for Bazel 9
  parity and added a topological diamond fixture. Verified
  `pytest -q tests/core/analysis/test_depset_order.py` passes.
- Verified existing transitive set e2e coverage still passes with
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`.
- Ran the Plan 51 memory checkpoint baseline command against
  `../zeromatter` with `SLUG_MEMORY_CHECKPOINTS=1` and
  `--isolation-dir plan54-depset-baseline`. The run was manually stopped after
  RSS reached about 10,097,636 KiB during package-file-tree loading. No
  `depset_to_list_frozen` or `depset_to_list_live` checkpoint lines appeared in
  `/tmp/plan54_depset_baseline_memory.log` before that point, so these
  checkpoints are not currently implicated in the early high-RSS phase seen in
  that run. This does not rule out later analysis-time depset flattening after
  package loading advances.

### 2026-05-08 Phase 1 traversal scaffold slice

- Added a representation-agnostic `collect_nested_set` traversal helper in
  `app/slug_build_api/src/interpreter/rule_defs/nested_set.rs` for
  `default`/`postorder`, `preorder`, and `topological` nested-DAG walks.
  Callers provide node identity, direct-item extraction, and child extraction,
  so this does not collapse depset into `TransitiveSet` or change either public
  facade.
- Reworked depset flattening to use the shared nested-set traversal helper for
  both frozen `Depset` values and live `LiveDepsetGen` values. This removes the
  depset-specific duplicated recursive/postorder and topological traversal code
  while preserving value-level dedupe in `to_list`.
- Repaired the unrelated `slug_build_api_tests` stale
  `ActionsRegistry::register(...)` call sites by passing the default exec-group
  metadata (`None` exec group and empty per-action exec properties). This
  unblocked the new Rust depset parity tests.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `cargo check -p slug`.

### 2026-05-08 Phase 1 depset facade slice

- Added an internal `DepsetView` facade over frozen `Depset`, live
  `LiveDepsetGen<Value>`, and frozen-live `LiveDepsetGen<FrozenValue>` values.
  Shared depset operations now use this view for direct items, transitive
  children, and stored order, instead of repeating type-specific extraction
  branches or falling back through removed Starlark-visible `.direct` /
  `.transitive` attributes.
- Reworked the generic `collect_depset_elements` path to use the same ordered
  `collect_nested_set` walk as `to_list` while still leaving value-level
  dedupe to `to_list`. This keeps extraction order consistent for frozen and
  live depsets without making depset a public `TransitiveSet` alias.
- Removed the separate `DepsetWithListGen` wrapper used by native
  `DefaultInfo.files`; unfrozen native default outputs now allocate the
  normal live depset facade with direct outputs and no transitive children.
- Kept the existing streaming `TransitiveSet` preorder/postorder/topological
  iterators separate for now. They are still used by projection and action
  input paths, so this slice only shares order vocabulary and traversal
  algorithms rather than forcing those paths through a materializing helper.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 1 completion slice

- Added explicit shared nested-set dedupe strategy entry points:
  `NestedSetDedup::NodeIdentity`, `NestedSetDedup::ValueHashEq`,
  `collect_nested_set_with_dedup`, `collect_nested_set_node_dedup`, and
  `collect_nested_set_value_dedup_by`.
- Added focused unit coverage for the shared preorder/postorder/topological
  diamond behavior and value-hash/equality output dedupe in
  `app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`.
- Kept `TransitiveSet` streaming iterators as the execution path for
  projection/reduction/action-input behavior. The shared core now owns the
  common Bazel-order vocabulary and materializing traversal/dedupe algorithms;
  tset-only `bfs`/`dfs` behavior remains outside the shared Bazel-order core.
- Phase 1 is complete. The next work is Phase 2 representation replacement,
  where any deeper tset sharing should preserve streaming behavior rather than
  forcing action/projection paths through materializing collection.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`.

### 2026-05-08 Phase 2 depset storage slice

- Replaced the live depset payload from Starlark list values with a shared
  `DepsetGen<V>` facade that stores direct values, transitive child depsets,
  parsed `NestedSetOrder`, top-level element type metadata, O(1) emptiness, and
  approximate nested-DAG depth at construction.
- Kept a thin native `Depset` wrapper around `DepsetGen<FrozenValue>` so
  existing Rust-created empty/frozen depsets remain allocable on live heaps;
  this wrapper no longer owns a separate depset graph shape.
- Reworked depset construction through the unified live/frozen builders. Direct
  duplicates are suppressed during construction, transitive order/type checks
  use stored child metadata instead of list-valued reinterpretation, and
  `DefaultInfo.files`/runfiles helper paths now allocate the normal depset
  builder rather than constructing `LiveDepsetGen` from list values.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 2 native construction decision slice

- Inspected the remaining native `heap.alloc(Depset::empty())` call sites in
  C++ provider NativeShim helpers, rule context helpers, Java helpers, and
  runfiles/default info construction. Migrating every empty depset to a live
  helper would expand this storage slice across unrelated provider surfaces, so
  keep the thin native `Depset` wrapper as the stable Rust construction API for
  empty/frozen native depsets.
- Keep `Depset::empty()` for existing native empty depset construction and
  narrow `Depset::from_frozen_values` to crate-local frozen native construction
  paths such as `DefaultInfo.files`. Removed the unused public
  `Depset::new(...)` constructor rather than preserving another native frozen
  construction entry point. A future cleanup can rename the remaining helpers
  to explicit builder helpers after consumer APIs are narrowed in Phase 3.
- No Bazel 9 error wording changes were made in this slice; those should stay
  paired with focused Bazel probes/tests for each validation branch.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 2 depset error wording slice

- Ran focused black-box probes against installed Bazel 9.1.0 for depset
  validation failures: invalid order, non-depset transitive element, mixed
  direct element types, direct/transitive type mismatch, mutable list/dict
  elements, order mismatch, `len(depset)`, and `depset | depset`.
- Tightened only the probed depset validation strings: invalid order now uses
  `Invalid order: ...`; transitive type errors include the failing index and
  Starlark type; order incompatibility uses Bazel's `Order '...' is
  incompatible with order '...'` shape; type mismatch uses single-quoted type
  names.
- Added exact Rust assertions for the newly probed invalid-order branch and for
  the tightened transitive/type/order mismatch wording in
  `app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`.
- Considered renaming `make_depset_from_lists`, but kept the helper name stable
  because its current public Rust call sites span cc/coverage/default-info
  provider surfaces and would make this wording slice broader than intended.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 2 public-surface error assertion slice

- Re-ran focused Bazel 9.1.0 black-box probes for remaining covered depset
  public-surface/mutability failures: mutable list direct element, mutable dict
  direct element, `len(depset)`, and `depset | depset`.
- Confirmed the current Slug mutable-element wording already matches Bazel's
  `depset elements must not be mutable values` message for both list and dict
  direct elements, so no depset implementation change was needed for that
  branch.
- Tightened the Rust public-surface assertions for `len(depset)` and
  `depset | depset` to the full Bazel 9.1.0-observed wording:
  `in call to len(), parameter 'x' got value of type 'depset', want 'iterable or string'`
  and `unsupported binary operation: depset | depset`.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 2 constructor depth validation slice

- Ran focused Bazel 9.1.0 black-box probes for depset nested-DAG depth during
  analysis. A direct-containing chain with final depth 3500 passes, while the
  next direct-containing parent fails at construction with
  `depset depth 3501 exceeds limit (3500)`.
- Confirmed with the same probes that `depset(transitive = [child])` does not
  increase the effective depth in Bazel 9.1.0, so the Slug depth metadata now
  preserves child depth for transitive-only parents and only increments when
  the new depset has direct elements.
- Added the Bazel 3500 depth limit and exact depth-limit error wording to the
  shared live/frozen depset builder path. This covers live Starlark depsets and
  frozen/native depsets without adding another graph representation.
- Added focused Rust parity coverage for the passing 3500-depth construction,
  transitive-only non-deepening behavior, and failing 3501-depth construction.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Phase 2 is complete for Plan 54 depset storage and constructor/error parity:
  depset now uses the shared live/frozen storage facade, stores parsed order,
  element type, emptiness, and depth metadata at construction, validates the
  currently probed Bazel 9.1.0 constructor/error branches, and keeps tset
  streaming paths separate.

Remaining Plan 51 follow-up, outside the completed Phase 2 implementation:

- Revisit the Plan 51 zeromatter memory repro after package-file-tree loading can
  advance far enough to hit analysis-time depset flattening checkpoints.

### 2026-05-08 Phase 3 consumer-helper first slice

- Added typed depset consumer helpers in
  `app/slug_build_api/src/interpreter/rule_defs/depset.rs`:
  `depset_to_list(value, heap) -> starlark::Result<Vec<Value>>`,
  `depset_to_artifact_inputs(value, heap) -> starlark::Result<Vec<Value>>`,
  and `is_depset_value(value)`. `depset_to_list` now owns the Bazel-style
  flattening plus value-equality dedupe path used by Starlark `to_list`.
- Kept `collect_depset_elements` as a temporary bridge for unconverted
  cc/coverage call sites, preserving its previous non-deduping behavior while
  the remaining consumers are migrated. Kept `depset_direct_and_transitive`
  public only as a documented native Rust graph-shape bridge used by
  `native.transitive_set_from_depset`, not as a Starlark-visible behavior
  dependency.
- Converted `cmd_args.add_all` and `cmd_args.add_joined` to detect depsets via
  `is_depset_value` and flatten through `depset_to_list` instead of the ad hoc
  `collect_depset_elements` path.
- Converted runfiles tree synthesis to use `depset_to_artifact_inputs` for
  `runfiles.files`, so depset flattening and artifact validation are centralized
  before the tree action maps files to runfile paths.
- Converted the touched DefaultInfo/Runfiles paths away from local depset type
  string checks and old live depset construction: DefaultInfo `files` extraction
  uses `depset_to_artifact_inputs` for depsets, `ctx.runfiles` uses
  `is_depset_value`, and runfiles unions/builders go through
  `make_depset_from_lists`.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Phase 3 remains incomplete. Remaining consumers include the
  `collect_depset_elements` call sites in cc_common actions/providers and
  coverage_common, plus java_common and any remaining native construction or
  bridge call sites that should move to typed helper APIs.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `cargo check -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 3 coverage consumer slice

- Migrated `coverage_common` transitive dependency collection away from the
  temporary `collect_depset_elements` bridge. `InstrumentedFilesInfo`
  dependency fields now flatten through `depset_to_list`, sharing the same
  Bazel-style depset traversal and value-dedupe behavior as other typed
  consumers.
- Changed `collect_dep_coverage` to return `starlark::Result` and propagate
  typed depset helper errors when an `InstrumentedFilesInfo` field exists but
  is not a depset. No new Bazel wording probe was run, so this slice did not
  add or tighten any error-message assertions.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Phase 3 remains incomplete. Remaining known consumers are the
  `collect_depset_elements` call sites in `cc_common/actions.rs`, plus
  java_common and any remaining native construction or bridge call sites that
  should move to typed helper APIs.
- Verified:
  `cargo fmt`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `pytest -q tests/core/analysis/test_native_rules.py -k instrumented_files_info`;
  `cargo check -p slug`;
  `git diff --check`.

### 2026-05-08 Phase 3 cc_common/actions completion slice

- Migrated the remaining `cc_common/actions.rs` depset consumers away from
  `collect_depset_elements`. Provider depset fields now flatten through
  `depset_to_list`, action-input/header paths use
  `depset_to_artifact_inputs`, and mixed list/depset header/linking paths use a
  local helper that preserves the previous iterable fallback for non-depset
  values.
- Replaced the last `request_value::<Depset>()` check in the touched cc_common
  group with `is_depset_value`, so live, frozen-live, and frozen depsets all go
  through the same facade check.
- Fixed `java_common.compile`'s empty JavaInfo transitive jar fields to expose
  actual empty depsets instead of empty lists, completing the remaining Phase 3
  checklist item for java_common.
- Focused Bazel 9.1.0 probes on 2026-05-08 showed that direct depset elements
  may be `struct`, providers, and depsets when their fields are immutable, while
  a provider containing a mutable list still errors with
  `depset elements must not be mutable values`. Based on that probe, made
  `LibraryToLink`, `LinkerInput`, ctx-cheat label stubs, and depsets hashable in
  the Starlark sense without weakening mutable-field rejection.
- Preserved the separate Bazel depset facade and left `TransitiveSet`
  streaming projection/reduction/action-input paths unchanged.
- Phase 3 is complete: `rg "collect_depset_elements|request_value::<.*Depset|DepsetWithListGen" app -g '*.rs'`
  now finds only the temporary helper definition in `depset.rs`, and the only
  remaining `depset_direct_and_transitive` users are the documented
  native/tolist bridge paths reserved for Phase 5.
- Verified:
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/cc_common`;
  `pytest -q tests/core/analysis/test_native_rules.py -k 'cc_common or cc_library or java_common'`.

### 2026-05-08 Phase 3 final audit/cleanup

- Audited the remaining Phase 3 scan hit and confirmed
  `collect_depset_elements` had no Rust callers under `app`; it was only the
  temporary bridge definition left behind after the consumer migrations.
- Removed the `collect_depset_elements` API so wrong-type depset consumers can
  no longer silently flatten to an empty collection through its
  `unwrap_or_default()` path.
- Kept `depset_direct_and_transitive` as the documented native graph-shape
  bridge used by `native.transitive_set_from_depset` and memory-checkpoint
  to-list metadata; this preserves TransitiveSet streaming behavior and leaves
  Phase 4 untouched.
- Phase 3 is complete after this audit: the typed helper replacements cover the
  listed consumers, the temporary bridge API is removed, and the only remaining
  depset graph-shape bridge is the explicitly documented native path.
- Verified:
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `rg -n "collect_depset_elements|request_value::<.*Depset|DepsetWithListGen" app -g '*.rs'`
  (no matches);
  `git diff --check`.

### 2026-05-08 Phase 4 transitive-set shared traversal slice

- Added shared streaming nested-set node iterators in
  `app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`. The existing
  materializing `collect_nested_set_node_dedup` path now collects by consuming
  the same streaming node iterators, so depset flattening and tset shared-order
  traversal use the same preorder/postorder/topological traversal machinery
  without changing depset's public facade.
- Reworked `TransitiveSetGen`'s `preorder`, `postorder`, and `topological`
  iterators to use the shared nested-set streaming node iterator adapter. The
  tset iterator still yields `&TransitiveSetGen` nodes lazily, so
  projection/reduction/action-input streaming behavior is preserved.
- Preserved tset-only `bfs` and `dfs` iterators locally. They are not Bazel
  depset orders and do not belong in the shared Bazel-order nested-set core.
- Kept tset's existing preorder enqueue-dedupe behavior via an explicit
  `NestedSetPreorderDedupe::OnChildEnqueue` mode. The broader
  `transitive_set` Rust filter caught that a plain Bazel-style
  visit-dedupe preorder changes the documented tset ordering for shared
  sibling nodes, so depset and tset now share the streaming helper while each
  preserves its existing preorder semantics.
- Kept `TransitiveSetGen` storage unchanged. A full physical storage unification
  would be broader than Phase 4 and is not needed after the depset facade and
  shared traversal paths are correct.
- Phase 4 is complete: shared traversal now covers all tset orderings that
  overlap with Bazel nested-set order vocabulary, while tset-only traversal and
  public semantics remain unchanged. Phase 5 bridge semantics and Phase 6 lazy
  depset action expansion remain future work.
- Verified:
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_set -- --nocapture`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `cargo test -p slug_build_api nested_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `git diff --check`.

### 2026-05-08 Phase 5 bridge semantics slice

- Reworked `native.transitive_set_from_depset` so the internal
  `BazelDepsetTset` bridge now creates one transitive-set node per depset node,
  not one node per direct depset element. Because `TransitiveSet` nodes can
  carry only zero or one value, each bridged depset node stores its direct
  values as one immutable tuple-valued tset payload.
- Added a per-call depset identity cache while converting depset nodes to tset
  nodes. The cache lifetime is limited to one bridge invocation, so it preserves
  shared depset child identity within that conversion without introducing
  cross-analysis or frozen-heap lifetime questions.
- Kept the bridge explicitly lossy and documented it at the native helper
  boundary: `depset -> transitive_set` uses the internal tuple payload shape,
  and `transitive_set -> depset` still materializes values and loses
  projections, reductions, definition identity, and tset node-identity
  semantics.
- Taught `native.depset_from_transitive_set` to recognize the internal
  `BazelDepsetTset` definition and expand tuple payloads back into direct
  depset values. For that internal bridge only, `order = "default"` now uses the
  Bazel depset default/postorder-like traversal instead of the legacy generic
  tset `bfs` default, so default roundtrips preserve Bazel 9.1.0-observed
  depset order.
- Added focused e2e coverage in `tests/core/analysis/test_depset_order.py` and
  `test_depset_order_data` for the internal bridge shape and default/preorder
  roundtrip ordering.
- Preserved the separate Bazel depset facade and did not make depset a public
  alias for `TransitiveSet`. `TransitiveSet` projection, reduction, action-input
  streaming paths remain unchanged.
- Phase 5 is complete. Phase 6 lazy depset action expansion remains future
  work.
- Verified:
  `cargo fmt`;
  `cargo build -p slug`;
  `cargo check -p slug`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_set -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `git diff --check`.

### 2026-05-08 Phase 6 deferred command-line prep slice

- Inspected the current `cmd_args` rendering and action-input path before
  adding a lazy representation. `cmd_args` stores `CommandLineArgLike` values,
  while execution input sharing is currently expressed through
  `ArtifactGroup::TransitiveSetProjection`; there is no depset-shaped
  artifact group today.
- Added an internal `DepsetCommandLineArg` wrapper for the safe Phase 6 subset:
  plain `ctx.actions.args().add_all(depset)` and plain
  `add_joined(depset)` over depsets whose stored top-level type is known to be
  command-line safe (`string`, `File`, or `OutputArtifact`, plus empty depsets).
  These calls now store the depset value in `cmd_args` and expand it when the
  command line or artifact visitor is evaluated, instead of flattening during
  the Starlark `add_all`/`add_joined` call.
- Kept existing eager behavior for transformed forms (`before_each`,
  `format_each`, `map_each`, `uniquify`, `terminate_with`, and
  `format_joined`) because those paths either need analysis-time Starlark
  callbacks or have stringification details that should not be changed without
  Bazel parity probes.
- Added depset metadata/flattening helpers for command-line consumers without
  exposing depset as a public `TransitiveSet` alias.
- Deferred a true `ArtifactGroup::Depset`/shared action-input representation.
  The existing action registration and execution code assumes artifact groups
  resolve through single artifacts, promises, or transitive-set projections.
  Routing depsets through `ArtifactGroup::TransitiveSetProjection` would not
  preserve Bazel depset value-dedup/order semantics, and adding a new artifact
  group variant needs a separate design through `ResolvedArtifactGroup`,
  `ensure_artifact_group_staged`, action input dedupe, content-based path
  eligibility, and detailed metrics traversal.
- Preserved `ArtifactGroup::TransitiveSetProjection` for tset projections only
  and did not make Bazel depset a public alias for `TransitiveSet`.
- Phase 6 is partially complete: safe command-line flattening has moved out of
  `add_all`/`add_joined` construction for the common simple depset cases, but
  shared deferred artifact input expansion remains future work.
- Verified:
  `cargo fmt`;
  `cargo check -p slug_build_api`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `cargo test -p slug_build_api_tests depset_add_all_and_joined_render_after_freeze -- --nocapture`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_depset_order.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `pytest -q tests/core/analysis/test_cmd_args.py`;
  `git diff --check`.

### 2026-05-08 Phase 6 deferred artifact-input slice

- Added a narrow `ArtifactGroup::Depset` representation for the safe action
  input subset: frozen depsets whose element type is `File`, reached from the
  lazy `DepsetCommandLineArg` artifact visitor. Empty depsets and string
  depsets visit no inputs; live depsets, `OutputArtifact` depsets, and
  transformed `add_all`/`add_joined` forms keep the existing flattening paths.
- Preserved Bazel depset value-dedup/order semantics by expanding the depset
  with depset traversal helpers when the artifact group is materialized, not by
  routing through `ArtifactGroup::TransitiveSetProjection`. Tset projections
  remain tset-only.
- Threaded the new group through `ResolvedArtifactGroup`,
  `ensure_artifact_group_staged`, `ArtifactGroupValues` construction,
  content-based path metadata, target-platform-aware input dedupe eligibility,
  detailed action input metrics traversal, aquery/BXL/query input expansion,
  and the copy action's unsupported grouped-input match.
- The representation intentionally stores only a frozen depset value plus
  identity/metadata. Arbitrary live depsets still do not have a durable
  analysis-storage key/owner comparable to tset projection keys, so action
  registration-time visits for live depsets still flatten rather than creating
  an unsafe long-lived group.
- `ensure_artifact_group_staged` grew from the previous checked future size of
  1088 bits to 1600 bits after adding the depset staging branch. This is a
  deliberate size regression for the new branch, not an accidental unbounded
  future from recursive async calls.
- Phase 6 is further along but still incomplete: frozen `File` depsets have a
  shared deferred action-input representation, while transformed
  `add_all`/`add_joined` forms and live depset input registration remain future
  work pending separate Bazel probes and/or a durable depset storage key.
- Verified:
  `cargo fmt`;
  `cargo check -p slug_build_api`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_cmd_args.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `git diff --check`.

### 2026-05-09 Phase 6 live-storage and transformed-args closure slice

- Investigated whether arbitrary live depsets can be assigned a durable
  `AnalysisValueStorage`-style owner/key comparable to transitive-set
  projections. They cannot be represented safely with the current storage
  shape. `AnalysisValueStorage` records actions and transitive sets; a
  `TransitiveSetKey` is minted by `AnalysisRegistry::create_transitive_set`
  from the current `DeferredHolderKey` plus a stable per-analysis index, and
  the frozen analysis storage can later look that key up. Bazel depsets are
  constructed by the global `depset()` function as ordinary Starlark heap
  values, are not registered in `AnalysisValueStorage`, and currently have
  only Starlark value identity until they freeze. Reusing that live identity as
  an action-input key would not survive the analysis freeze/lookup boundary and
  would not provide the owner checks used by tset projection lookup.
- Did not add live-depset `ArtifactGroup::Depset` support. Making it safe would
  require a new registered depset storage architecture analogous to tsets:
  per-analysis depset indexes/keys, constructor access to the owning analysis
  registry, frozen lookup plumbing, and action/input metadata threading. That is
  broader than this Phase 6 slice and should only be reopened with Plan 51
  evidence that live depset flattening is a dominant memory problem.
- Ran a fresh Bazel 9.1.0 probe in `/tmp/slug-plan54-args-probe` for
  transformed `ctx.actions.args().add_all(depset)` and `add_joined(depset)`
  forms. Observed behavior for `depset(["a", "b", "a"], transitive =
  [depset(["c", "b"])])` is that Bazel first uses the depset's deduped default
  traversal (`c`, `b`, `a`), then applies the command-line transform:
  `before_each` interleaves before each item, `format_each` formats each item,
  `map_each` may drop `None` or splice returned lists, `terminate_with` appends
  after non-empty `add_all`, `format_joined` wraps the final joined string, and
  `uniquify` is a no-op after depset dedupe for this probe. Example probe
  outputs included `--x c --x b --x a`, `c b1 b2 a`,
  `[c]:[b]:[a]`, `c:b1:b2:a`, and `<c:b:a>`.
- Kept transformed `add_all`/`add_joined` depset forms on the existing eager
  flattening path. The implementation already needs analysis-time Starlark
  callbacks for `map_each`, and the Bazel probe confirms the current eager
  model is the conservative parity-preserving behavior. Only the existing
  simple untransformed safe forms remain lazy.
- Added focused e2e coverage in
  `tests/core/analysis/test_cmd_args_data/{defs.bzl,BUILD.bazel}` and
  `tests/core/analysis/test_cmd_args.py` for transformed depset
  `add_all`/`add_joined` semantics against the Bazel 9.1.0 probe results.
- Phase 6 is closed with documented limits: simple safe depset command-line
  forms are lazy, frozen `File` depsets reached from those forms have narrow
  deferred action-input support, transformed forms intentionally flatten for
  parity, live depsets intentionally flatten until a durable registered depset
  storage design exists, and `ArtifactGroup::TransitiveSetProjection` remains
  tset-only.
- Verified:
  `USE_BAZEL_VERSION=9.1.0 bazelisk build //...` in
  `/tmp/slug-plan54-args-probe`;
  `cargo fmt`;
  `cargo check -p slug`;
  `cargo build -p slug`;
  `cargo test -p slug_build_api_tests depset -- --nocapture`;
  `cargo test -p slug_build_api_tests transitive_sets_iteration -- --nocapture`;
  `pytest -q tests/core/analysis/test_cmd_args.py`;
  `pytest -q tests/core/analysis/test_runfiles.py`;
  `pytest -q tests/core/transitive_sets/test_transitive_sets.py`;
  `git diff --check`.

## Problem

Slug inherited Buck2's `transitive_set` and has since added a separate
Bazel-compatible `depset` implementation. These occupy the same broad design
space: a cheap-to-merge transitive DAG that can be flattened with deterministic
deduplication. They are not, however, the same public abstraction.

The current split has several costs:

- `depset` has an independent graph representation in
  `app/slug_build_api/src/interpreter/rule_defs/depset.rs`.
- `TransitiveSet` has a separate representation and traversal machinery in
  `app/slug_build_api/src/interpreter/rule_defs/transitive_set/`.
- The explicit bridge in
  `app/slug_interpreter_for_build/src/interpreter/natives.rs` is lossy and
  expensive: `depset -> transitive_set` creates one tset node per direct depset
  item, and `transitive_set -> depset` materializes a flat list.
- Slug's current depset surface includes non-Bazel behavior (`.order`,
  `.direct`, `.transitive`, `len(depset)`, and `depset | depset`) that should be
  removed or explicitly quarantined for Bazel 9 parity.
- Slug's current depset ordering is not exact Bazel behavior. In particular,
  `topological` is treated like `postorder`, and omitted/default order is
  inferred from transitive depsets in a way Bazel 9 does not do.

This plan is also the likely structural follow-up to the remaining Plan 51
high-RSS analysis issue. After alias diagnostics, package-listing corruption,
dynamic-cell reference stability, and unbounded diagnostic payloads were ruled
out or fixed, the remaining zeromatter failure shape is genuine memory pressure
during analysis of large bzlmod/toolchain graphs. The leading suspect is large
toolchain/provider depsets being flattened or retained repeatedly instead of
remaining shared DAGs.

The goal is to merge the duplicated graph mechanics without collapsing two
public APIs that intentionally differ.

## Source of truth

### Bazel depset

Use Bazel 9 source and docs as the compatibility source of truth:

- `Depset.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/Depset.java>
- `NestedSet.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/NestedSet.java>
- `NestedSetBuilder.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/NestedSetBuilder.java>
- `Order.java`:
  <https://github.com/bazelbuild/bazel/blob/release-9.0.0/src/main/java/com/google/devtools/build/lib/collect/nestedset/Order.java>
- Builtin API docs:
  <https://bazel.build/rules/lib/builtins/depset>
- Concept docs:
  <https://bazel.build/extending/depsets>

Important Bazel design points:

- `depset` is a Starlark wrapper over `NestedSet`.
- A nested set is an immutable ordered DAG. Direct elements are leaf
  successors, transitive depsets are non-leaf successors.
- Construction is cheap; flattening (`to_list`) is intentionally expensive and
  should be avoided in rule hot paths.
- Element type is tracked without flattening. Empty depsets have no element
  type and can merge with any element type.
- Elements are constrained by Bazel's current hashability/mutability checks and
  by same top-level Starlark type.
- `is_empty` and truthiness are O(1).
- `to_list` suppresses duplicate element values using hash/equality.
- Direct duplicates are eliminated by the nested-set builder.
- Order is selected at construction. Bazel's Starlark orders are:
  `default`, `postorder`, `preorder`, and `topological`.
- `default` is stable/unspecified deterministic order. It is compatible with
  other orders but should remain the constructed order unless Bazel source or
  tests prove otherwise.
- `topological` is not postorder. For a diamond, Bazel's docs show:
  `d = depset(["d"], transitive = [b, c], order = "topological")` flattens to
  `["d", "b", "c", "a"]`.

Black-box check against installed Bazel 9.1.0 on 2026-05-08:

- `hasattr(depset(["x"]), "order") == False`
- `hasattr(depset(["x"]), "direct") == False`
- `hasattr(depset(["x"]), "transitive") == False`
- `len(depset(["x"]))` errors with `want 'iterable or string'`
- `depset(["c"], transitive = [depset(["a"], order = "preorder")])`
  flattened to `["a", "c"]`, meaning omitted order did not inherit preorder
  traversal from the child in that observed Bazel 9.x build.

Before implementation, re-run focused probes against the exact Bazel 9 version
Slug is targeting if there is any contradiction between docs and source. Prefer
release source over old docs when they disagree.

### Buck2 transitive_set

Use Buck2 docs and Slug's inherited implementation for intent:

- Buck2 docs:
  <https://buck2.build/docs/rule_authors/transitive_sets/>
- Slug docs:
  `docs/rule_authors/transitive_sets.md`
- Slug implementation:
  `app/slug_build_api/src/interpreter/rule_defs/transitive_set/`

Important transitive_set design points:

- `transitive_set` is nominal. Users first define a set type with
  `transitive_set(...)`, then create instances with `ctx.actions.tset`.
- Each logical tset node has zero or one value and any number of child tsets.
- Values are projected eagerly at node creation into args/json projection
  values. Reductions are also computed eagerly.
- Projection objects are cheap to create and are lazily expanded later.
- Action input discovery can keep a transitive set projection as a shared graph
  edge via `ArtifactGroup::TransitiveSetProjection`, avoiding one action input
  edge per flattened artifact.
- Tset traversal skips already visited nodes by node identity. It does not
  promise Bazel's value-level duplicate suppression.
- Tset order is selected at use site (`traverse`, `project_as_args`,
  `project_as_json`), not at set construction.
- Tsets support traversal orders beyond Bazel depset: `bfs` and `dfs`.

### Current Slug implementation

Relevant current files:

- `app/slug_build_api/src/interpreter/rule_defs/depset.rs`
- `app/slug_build_api/src/interpreter/rule_defs/transitive_set/transitive_set.rs`
- `app/slug_build_api/src/interpreter/rule_defs/transitive_set/transitive_set_iterator.rs`
- `app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`
- `app/slug_interpreter_for_build/src/interpreter/natives.rs`
- `app/slug_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`
- `app/slug_build_api/src/interpreter/rule_defs/cmd_args/typ.rs`
- `app/slug_action_impl/src/context/runfiles_tree.rs`
- `app/slug_util/src/memory_checkpoint.rs`
- `scripts/memory_smoke.sh`

Current local behavior after Phase 1:

- `app/slug_build_api/src/interpreter/rule_defs/nested_set.rs` now provides
  the shared `NestedSetOrder` vocabulary and materializing
  preorder/postorder/topological collection helpers with explicit
  node-identity and value-hash/equality dedupe entry points.
- `Depset` stores `direct: Vec<FrozenValue>`, `children: Vec<FrozenValue>`,
  and `order: String`; Phase 2 should replace this frozen-only shape with a
  generic live/frozen facade.
- `LiveDepsetGen<V>` stores `direct` and `transitive` as list values plus an
  order string; Phase 2 should stop repeatedly reinterpreting these list values
  during traversal.
- `DepsetView` provides an internal facade over frozen `Depset`, live
  `LiveDepsetGen<Value>`, and frozen-live `LiveDepsetGen<FrozenValue>`.
- `DefaultInfo.files` now uses the normal live depset facade for native
  default outputs; the old `DepsetWithListGen` wrapper is gone.
- `collect_depset_elements` silently ignores non-depsets in some call paths.
- `to_list` uses the shared nested-set traversal helper and then dedupes by
  value equality while preserving order.
- `len(depset)`, `depset | depset`, and Starlark-visible `.direct`,
  `.transitive`, and `.order` now fail or remain absent for Bazel 9 parity.
- Default-order and topological depset traversal now match the focused Bazel
  9.1.0 probes covered by tests.
- `TransitiveSet` keeps its existing streaming iterators for
  projection/reduction/action-input behavior. It shares the common
  preorder/postorder/topological order vocabulary but not a materializing
  execution path.

### NativeShim boundary follow-up

The current `//sdk:sdk_contents` frontier is again a depset mutable-element
failure:

```text
rules_rust+0.69.0/rust/private/rustc.bzl:1374
deps = depset(deps)
error: depset elements must not be mutable values
```

Before adding another local conversion, identify whether the mutable value
crossed a NativeShim or intrinsic provider boundary. If so, fix the shared
freezing/hashability contract at that boundary and add focused Bazel 9 parity
coverage for the provider value that enters the depset. The systemic plan for
these boundaries is tracked in
[56-native-intrinsic-provider-shims.md](56-native-intrinsic-provider-shims.md);
this depset plan should keep enforcing Bazel's mutable-value rejection rather
than adding allowlists.

Plan 51 added `SLUG_MEMORY_CHECKPOINTS`-gated depset flattening checkpoints
around current `depset.to_list()` paths:

- `depset_to_list_frozen`
- `depset_to_list_live`

These record root direct/transitive counts, collected element count before
dedupe, deduped element count, duplicate count, RSS, and max RSS. Use them to
confirm which depsets are flattening during the zeromatter repro before making
large representation changes.

## Decision

Do not make Bazel `depset` a public alias for Buck/Slug `TransitiveSet`.

Instead, create a shared nested-DAG engine and keep separate public facades:

- `depset`: Bazel-compatible Starlark value over the shared core.
- `TransitiveSet`: Buck-compatible action/projection/reduction value over the
  shared core or over shared traversal/building primitives.

This is the best tradeoff because:

- Bazel `depset` and Buck `transitive_set` have different semantic boundaries.
- A raw alias would either expose Buck-only projection APIs on Bazel depsets or
  remove the execution-facing benefits that make tsets useful.
- Shared graph mechanics solve the real duplication while letting each facade
  enforce its own invariants.
- The shared core can fix depset topological behavior by reusing or generalizing
  the existing tset topological iterator.
- The shared core creates a path to deferred depset command-line/action-input
  expansion instead of repeated analysis-time flattening.

## Non-goals

- Do not preserve Slug's previous prototype depset surface if it conflicts with
  Bazel 9 parity.
- Do not support Bazel 8.x or legacy depset names beyond what Bazel 9 accepts.
- Do not make tset projections/reductions part of Bazel depset.
- Do not silently coerce arbitrary `TransitiveSet` values to depsets or depsets
  to arbitrary tset definitions.
- Do not flatten as the primary implementation strategy except at explicit API
  boundaries such as `depset.to_list()`.

## Target architecture

### Layer 1: nested DAG core

Introduce an internal module, likely under
`app/slug_build_api/src/interpreter/rule_defs/nested_set/`, with shared order,
builder, and traversal mechanics.

The exact Rust representation should be chosen after prototyping with
Starlark's `Trace`, `Freeze`, `Coerce`, and lifetime requirements. The
preferred shape is:

```rust
enum NestedSetOrder {
    Default,
    Postorder,
    Preorder,
    Topological,
}

struct NestedSetGen<V> {
    order: NestedSetOrder,
    direct: Box<[V]>,
    children: Box<[V]>, // values pointing to same facade/core node type
    approx_depth: u32,
    is_empty: bool,
    // Optional flatten cache for frozen/transitive-heavy nodes.
}
```

If a single concrete storage type is too invasive because depset and tset
children point at different Starlark wrapper types, use a shared builder and
generic traversal framework first:

- a shared `NestedSetOrder`;
- a shared direct/transitive construction algorithm;
- a shared traversal trait for "node with direct values and child nodes";
- a shared value-deduping flatten implementation for depset;
- a shared node-deduping traversal implementation for tset.

That fallback still eliminates semantic divergence while keeping the option for
physical storage unification later.

### Layer 2: Bazel depset facade

Replace `Depset`, `LiveDepsetGen`, and `DepsetWithListGen` with a single
live/frozen `DepsetGen<V>` over the nested core.

Required facade behavior:

- Starlark type name is exactly `depset`.
- Constructor signature matches Bazel 9:
  `depset(direct = None, order = "default", *, transitive = None)`.
- Positional direct argument remains accepted if Bazel 9 accepts it.
- Public Starlark methods include `to_list`.
- Do not expose `.direct`, `.transitive`, `.order`, `len`, or `|` unless a
  deliberate non-Bazel diagnostic hook is hidden from normal Bazel Starlark.
- Truthiness uses O(1) emptiness.
- `to_list` performs a traversal based on the depset's construction order and
  suppresses duplicate element values.
- Flattening returns a copy.
- Debug/introspection can expose direct/transitive internally for Rust callers,
  but not as Starlark attributes.
- `DefaultInfo.files`, runfiles, coverage, cc providers, and action APIs should
  all consume this single depset type.

Validation requirements:

- `transitive` must be a sequence of depsets.
- Direct and transitive elements must have compatible top-level Starlark type.
- Empty depsets do not constrain element type.
- Direct elements must obey Bazel 9 hashability/mutability checks. Match Bazel
  9 source and black-box tests exactly; do not implement a future Bazel TODO
  stricter than the target version.
- Direct list/dict elements must be rejected if Bazel 9 rejects them.
- Order compatibility follows Bazel 9 `Order.isCompatible`.
- Do not infer parent order from children unless source/tests prove Bazel 9
  does so. Current evidence says the parent order remains the requested order,
  including `default`.
- Enforce Bazel's nested set depth limit if Slug has an equivalent semantics
  knob. If Slug does not expose the flag yet, add a TODO with a focused parity
  test and do not leave unbounded pathological recursion.

### Layer 3: Buck/Slug TransitiveSet facade

Keep `TransitiveSet` as a distinct public type.

Do not change these semantics without a separate user decision:

- users define a nominal type with `transitive_set(...)`;
- instances are created by `ctx.actions.tset`;
- each logical node has zero or one value;
- projections and reductions are tied to the nominal definition;
- projections/reductions are evaluated at node creation;
- projection values are cached per node;
- traversal order is chosen at projection/traversal use site;
- node identity, not value equality, controls deduplication;
- `bfs` and `dfs` remain tset-only orders;
- action input discovery continues to use `ArtifactGroup::TransitiveSetProjection`.

What can be made more depset-like:

- Use the same iterative traversal algorithms for preorder, postorder, and
  topological where semantics match.
- Use a shared graph-node interface so traversal logic is tested once.
- Consider shared depth accounting and cycle/pathological-depth checks.
- Consider shared flatten caching for `list(tset.traverse(...))` only if it
  does not interfere with lazy projection expansion or memory behavior.

What should not be made depset-like:

- Do not add same-Starlark-type element restrictions to tsets. Tset definitions
  and projection functions are the type boundary.
- Do not dedupe tset values by equality. Distinct nodes with equal values can
  have distinct projections/reduction context and should remain distinct unless
  they are the same visited node.
- Do not move tset order to construction time. Use-site order is a useful Buck
  divergence.
- Do not support multiple direct values per public tset node unless projections
  and reductions are redesigned. A tset node value maps to exactly one set of
  projections and one reduction input.

## Detailed migration phases

### Phase 0: parity characterization

Add focused tests before refactoring. These tests should fail against current
Slug where behavior is wrong.

Before changing representation, run at least one Plan 51 zeromatter repro with:

```sh
SLUG_MEMORY_CHECKPOINTS=1 scripts/memory_smoke.sh \
  --include-pgrep '<zeromatter slugd pgrep pattern>' \
  -- target/debug/slug --isolation-dir <name> build //sdk:sdk_contents
```

Capture all `depset_to_list_frozen` and `depset_to_list_live` lines. This gives
the baseline for whether the shared-core work needs to prioritize lazy
command-line/action expansion, `to_list()` caching, or provider construction
dedupe. If these checkpoints do not fire near the high-RSS phase, the memory
root cause is probably adjacent provider/toolchain retention rather than
flattening itself.

Depset constructor and surface:

- `type(depset()) == "depset"`.
- `depset().to_list() == []`.
- `bool(depset()) == False`; `bool(depset(["x"])) == True`.
- `hasattr(d, "order")`, `hasattr(d, "direct")`, and
  `hasattr(d, "transitive")` are false.
- `len(depset(["x"]))` errors like Bazel.
- `depset(["x"]) | depset(["y"])` errors like Bazel.
- `depset(transitive = None)` behavior matches Bazel 9.
- `depset(direct = None)` behavior matches Bazel 9.

Depset validation:

- Transitive elements must be depsets.
- Mixed direct element types fail.
- Direct type and non-empty transitive depset type mismatch fails.
- Empty transitive depsets do not constrain type.
- List/dict direct elements fail if Bazel 9 fails.
- Unhashable but frozen values follow exact Bazel 9 behavior.

Depset order:

- Preorder simple tree.
- Postorder simple tree.
- Topological diamond:
  `d -> b -> a` and `d -> c -> a` should match Bazel's `["d", "b", "c", "a"]`
  for the documented construction.
- Default simple tree matches a Bazel 9 probe, not Slug's current preorder
  assumption.
- Explicit non-default parent plus incompatible child order fails.
- Default parent plus non-default child stays default if Bazel source/probe says
  so.

Depset freezing/providers:

- Depsets created in one rule and read in another preserve behavior.
- Depsets exported from loaded `.bzl` files freeze and thaw correctly.
- `DefaultInfo.files.to_list()` works for frozen and live outputs.
- `runfiles.files` works through `ctx.runfiles(transitive_files = depset(...))`.

TransitiveSet regression:

- Existing `transitive_set` tests continue to pass.
- Projection input discovery still emits `ArtifactGroup::TransitiveSetProjection`.
- `project_as_args` and `project_as_json` remain lazy from action perspective.
- Tset topological, bfs, dfs examples in docs still match.

### Phase 1: introduce shared order and traversal tests (complete)

Added `NestedSetOrder` with Bazel names:

- `default`
- `postorder`
- `preorder`
- `topological`

Kept `TransitiveSetOrdering` as a separate public enum and mapped its common
variants to `NestedSetOrder`. Tset-only variants remain `bfs` and `dfs`, and
the existing tset streaming iterators remain the execution path.

Moved materializing traversal algorithms into a shared module with two dedupe
strategies:

- `NestedSetDedup::NodeIdentity`.
- `NestedSetDedup::ValueHashEq`.

This phase is behavior-preserving for tsets and allows depset to call the same
preorder/postorder/topological traversal code without making depset a public
alias for `TransitiveSet`.

### Phase 2: replace depset internals (complete)

Create `DepsetGen<V>` and remove the separate frozen-only graph shape once
live/frozen handling is proven. A thin native `Depset` wrapper may remain for
Rust-created empty/frozen depsets as long as it delegates to `DepsetGen`.

Phase 2 guardrails:

- Split representation replacement into small, reviewable slices. Prefer
  parsed-order storage first, then a unified builder, then live/frozen storage
  replacement. Do not combine representation churn, consumer rewrites, and
  Bazel error wording changes in one broad patch.
- Treat `NestedSetDedup::ValueHashEq` as a helper for hashable caller-supplied
  identities, not as proof that arbitrary Starlark `Value` dedupe can use a
  stable hash key. Real depset `to_list` must preserve Bazel value-equality
  behavior.
- Do not claim Plan 51 memory improvement from Phase 2 until a zeromatter repro
  reaches `depset_to_list_frozen` or `depset_to_list_live` checkpoints and the
  before/after counts support that claim.
- Keep saying precisely that `TransitiveSet` shares order vocabulary and tests
  with the nested-set core while its streaming projection/reduction/action-input
  paths remain separate.
- Add approximate depth or depth-limit checks only with focused Bazel 9 source
  confirmation or black-box probes. Avoid inventing Slug-only depth behavior.

Implementation notes:

- Derive or implement `Trace`, `Freeze`, `Coerce`, `Allocative`,
  `ProvidesStaticType`, and `NoSerialize` as needed.
- Store direct elements as `Box<[V]>`, not a list `Value`, to avoid repeatedly
  reinterpreting list values.
- Store transitive children as `Box<[V]>` pointing to depset values.
- Store `NestedSetOrder` rather than `String`.
- Store element type metadata in the depset facade, not in the generic core if
  the core is also used by tsets.
- Store `is_empty` and `approx_depth` at construction.
- Optionally cache flattened frozen results behind a weak/cache mechanism if
  current memory model supports it. Do not cache live `Value<'v>` flattening
  across mutability boundaries.

Update constructors:

- Replace `make_depset_from_lists` with a builder that validates type, order,
  hashability, and depth.
- Replace direct calls to `Depset::empty()` with the new empty depset builder.
- Replace `Depset::from_frozen_values` with the same builder over frozen values.
- Remove `DepsetWithListGen`; a live depset should handle non-frozen default
  outputs directly.

Update methods:

- Keep only `to_list` in Starlark methods.
- Remove `length`.
- Remove `has_attr/get_attr` for `direct`, `transitive`, `order`.
- Remove `bit_or`.
- Keep Rust-only accessors for direct/transitive only if bridge/internal code
  still needs them.

### Phase 3: fix depset consumers (complete)

Replace ad hoc collection APIs with typed helpers:

- `depset_to_list(value, heap) -> Result<Vec<Value>>`
- `depset_to_artifact_inputs(value, heap) -> Result<Vec<Value>>`
- `depset_direct_and_transitive` only as an internal debug/bridge helper, not
  as a Starlark-visible behavior dependency.

Update call sites:

- `cmd_args.add_all` and `add_joined` in
  `app/slug_build_api/src/interpreter/rule_defs/cmd_args/typ.rs`.
- runfiles tree synthesis in
  `app/slug_action_impl/src/context/runfiles_tree.rs`.
- cc_common actions/providers.
- coverage_common.
- java_common.
- DefaultInfo and Runfiles in
  `app/slug_build_api/src/interpreter/rule_defs/provider/builtin/default_info.rs`.
- any `request_value::<Depset>()` code that assumes the old concrete type.

Prefer returning errors for wrong-type values rather than silently returning an
empty collection.

### Phase 4: preserve and simplify TransitiveSet

Refactor `TransitiveSetGen<V>` to use the shared traversal/building
implementation where possible.

Possible approaches:

1. Minimal: keep current storage, replace iterator internals with shared
   traversal traits.
2. Medium: replace `children: Box<[V]>` and `node: Option<NodeGen<V>>` traversal
   access with a shared `NestedGraphNode` adapter.
3. Full: store graph structure in a reusable core and keep `TransitiveSetGen`
   metadata beside it.

Start with minimal or medium. Full physical storage unification should happen
only if it reduces complexity after the depset facade is correct.

Do not refactor projections/reductions during this phase except to adjust them
to the shared traversal adapter.

### Phase 5: bridge semantics

Revisit `native.transitive_set_from_depset` and
`native.depset_from_transitive_set`.

Desired end state:

- These helpers are unnecessary for normal Bazel compatibility.
- If kept for Slug-specific internals, they should be explicit and documented as
  lossy where semantics cannot be preserved.
- `depset -> transitive_set` should preserve graph shape when using a built-in
  `BazelDepsetTset` definition, not materialize one node per direct element
  unless no better representation is possible.
- `transitive_set -> depset` necessarily loses projections, reductions,
  definition identity, and tset node-identity semantics. It may materialize a
  list and build a depset from values.
- Add caching only after semantic correctness, and only at stable/frozen
  boundaries where cache lifetime is clear.

If public Bazel mode should not expose these helpers, move them behind a Slug
internal namespace or keep them only in native internals.

### Phase 6: deferred depset action expansion

After depset storage is correct, reduce analysis-time flattening:

- Teach `ctx.actions.args().add_all(depset)` and `add_joined(depset)` to carry a
  lazy depset command-line item where possible.
- Add artifact visitation for depsets of files without flattening everything
  during analysis if a shared artifact group can represent the depset.
- Consider adding an `ArtifactGroup::Depset` or similar if a depset of artifacts
  can be represented safely through execution.
- Keep `ArtifactGroup::TransitiveSetProjection` for tset projections. Do not
  force depsets through tset projections unless that preserves Bazel semantics.

This phase is performance work and should not block the semantic cleanup unless
Plan 51 checkpoints show that analysis-time depset flattening is the dominant
zeromatter RSS driver. In that case, pull this phase forward immediately after the
depset facade is semantically correct.

### Phase 7: remove obsolete code and docs

Complete as of 2026-05-09:

- Confirmed `DepsetWithListGen` remains deleted.
- Confirmed there is no remaining old fallback code that treats arbitrary
  values as depset-like by scraping `.direct` and `.transitive`.
- Rewrote Slug-only depset tests for `.order`, `len`, and `|` to keep them
  as negative parity tests.
- Updated the bridge section in the parent plan to point to this shared-core
  plan and document the explicit lossy bridge.
- Audited user docs; no depset behavior edits were needed outside parity docs.

## Technical tradeoffs

### Why not make depset a raw TransitiveSet alias?

Raw aliasing is simpler but wrong:

- `TransitiveSet` has nominal definitions; `depset` is anonymous/generic.
- `TransitiveSet` requires `ctx.actions.tset` and a deferred key; `depset()` is
  a plain Starlark constructor usable wherever Bazel allows it.
- `TransitiveSet` has projections/reductions; `depset` does not.
- `TransitiveSet` traversal dedupes nodes; `depset.to_list` dedupes values.
- `TransitiveSet` order is selected at use site; `depset` order is fixed at
  construction.
- `TransitiveSet` permits `bfs`/`dfs`; Bazel depset does not.

### Why not make TransitiveSet more like depset?

Some internals can be shared, but the public differences are valuable:

- Tset nominal definitions make projections and reductions type-directed.
- Eager projection/reduction validation catches errors at tset construction.
- Action input projection keys allow execution to preserve shared graph edges.
- Use-site ordering lets the same tset support different projection consumers.
- Node-deduping is the correct unit for projections; value-deduping would change
  behavior when two nodes contain equal values.

Changing these would reduce the technical value of tsets and would not improve
Bazel compatibility, because Bazel rules should use `depset` at their API
boundary.

### Why a shared nested-DAG core is better

Shared core gives most of the benefit:

- one order parser/enum for Bazel orders;
- one tested topological traversal implementation;
- one depth/emptiness accounting model;
- one place to optimize flattening and direct duplicate removal;
- one path to deferred action expansion for depsets;
- separate facades for separate invariants.

## Risks

- Starlark lifetime and GC constraints may make a single physical storage type
  awkward. Mitigation: start by sharing traversal/building traits and only move
  to physical storage unification if it reduces complexity.
- Bazel docs and source occasionally disagree around default order wording.
  Mitigation: use Bazel 9 release source plus focused black-box probes.
- Tightening depset parity will break existing Slug prototype tests or local
  rules relying on `.order`, `len`, or `|`. This is acceptable under Bazel 9
  parity policy.
- Deferring depset expansion into action execution may require new artifact
  group plumbing. Mitigation: treat that as Phase 6 after semantic parity.
- Same-type/hashability validation can be subtle with Starlark Rust values.
  Mitigation: create a dedicated element-type helper with tests for strings,
  ints, artifacts/files, providers, structs, lists, dicts, tuples, and frozen
  values.

## Verification matrix

Unit tests:

- depset constructor validation;
- depset order traversal;
- depset topological diamonds;
- depset value deduplication;
- depset freeze/live behavior;
- tset traversal regression;
- tset projection/reduction regression.

Integration tests:

- `tests/core/analysis/test_depset_order.py`, rewritten for Bazel parity;
- `tests/core/analysis/test_runfiles.py`;
- `tests/core/analysis/test_providers.py`;
- `tests/core/transitive_sets/test_transitive_sets.py`;
- rules_cc fixtures that pass compilation/include depsets through providers;
- rules_python fixtures that pass runfiles depsets through `DefaultInfo`.

Black-box Bazel checks:

- Generate tiny temporary Bazel workspaces for every behavior where source is
  ambiguous.
- Record exact Bazel version used in the test comment or plan progress note.
- Do not accept Slug behavior based only on old Slug tests.

Memory checks:

- With `SLUG_MEMORY_CHECKPOINTS=1`, compare `depset_to_list_*` counts before
  and after the migration.
- The number and size of large `depset.to_list()` expansions during the zeromatter
  repro should either drop materially or be explained by user-visible Starlark
  calls that Bazel would also flatten.
- If deferred depset expansion is implemented, actions should not need to
  flatten depsets of files during analysis solely to discover inputs.

Commands for implementation PRs:

- `cargo test -p slug_build_api_tests transitive_set`
- `cargo test -p slug_build_api_tests depset` if a focused test module exists
  or is added.
- Relevant e2e tests under `tests/core/analysis`.
- Representative rules_cc/rules_python builds that exercise provider depsets
  and runfiles.

## Proposed implementation order

1. Add parity tests and mark current failures.
2. Add shared `NestedSetOrder` and traversal module.
3. Reimplement depset on the shared machinery.
4. Update depset consumers and remove `DepsetWithListGen`.
5. Refactor tset traversal to use shared machinery while preserving public tset
   behavior.
6. Replace or retire lossy bridge helpers.
7. Add deferred depset action expansion if benchmarks show analysis-time
   flattening remains material.
8. Update parent plan and docs.

## Definition of done

- Bazel-facing `depset` matches Bazel 9 behavior for constructor, public
  surface, validation, truthiness, and `to_list` order.
- Slug no longer has three independent depset wrapper shapes.
- Tset projection/reduction behavior is unchanged.
- Shared traversal code is used for common preorder/postorder/topological
  behavior or there is a documented reason why a specific path remains separate.
- `depset -> transitive_set` and `transitive_set -> depset` are either removed
  from public surface or documented as explicit, lossy conversions with tests.
- No action-input or runfiles path relies on silent "unknown depset-like value"
  fallbacks.
