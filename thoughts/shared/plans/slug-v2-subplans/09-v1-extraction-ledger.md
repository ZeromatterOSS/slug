# Stage 9: V1 Extraction Ledger

## Goal

Track every deliberate extraction from Slug V1 or Buck2-derived infrastructure
into V2 so useful work is preserved without importing V1 architectural debt or
Buck user semantics blindly.

## Extraction Rule

An extraction is acceptable only when it has:

- a named V2 owner stage;
- a source class, source path or test path, and exact immutable source commit;
- the Bazel source/test oracle or oracle fixture that justifies it;
- a clear import mode: copy, port, rewrite from behavior, reference-only, or
  reject;
- exact reachable V2 commit ids for landed or partially landed work;
- validation evidence after landing.

The current mixed-root `codex/slugv2` branch is treated as an extraction source
too. Importing code or fixtures from that branch into a clean-root V2 line must
record whether the import is a direct cherry-pick, a port, a rewrite from
behavior, or reference-only. Do not treat mixed-root commits as already accepted
V2 trunk work merely because they compile in the old workspace.

Buck2-derived infrastructure follows the same rule. Reuse its DICE,
starlark-rust, REAPI, interning, hashing, compact-collection, allocation, and
strong-hash primitives behind V2-owned Bazel abstractions; do not expose Buck
cells, labels, target patterns, executor configuration, or output semantics.

The clean-root remediation branch keeps the already-landed V2-only stage
artifacts from `codex/slugv2` only where the owner subplan records the oracle
fixture or Bazel source citation. V1-only source/test trees and the unwrapped
`remote_execution` source candidate are reference material through archive refs,
not active-root content.

## Workflow

1. Open the V2 owner plan and identify the exact behavior needed.
2. Inspect V1 or Buck2 source and tests, plus the Bazel oracle source/test.
3. Choose an import mode:
   - `copy`: code is infrastructure with no V1 semantic leak;
   - `port`: code is useful but names/types/path assumptions must change;
   - `rewrite from behavior`: keep tests/lessons, not implementation shape;
   - `reference-only`: external implementation or backend contract only;
   - `reject`: document why the V1 surface should not enter V2.
4. Add or update an oracle fixture before landing the extraction.
5. Record the exact source commit and the exact V2 commit or commit list. A
   branch name or prose such as "the checkpoint containing this entry" is not
   recoverable evidence.
6. Update this ledger and the owner plan with validation evidence.

## Review Checklist

Before moving an entry out of `Proposed`, answer:

- Does it contain Buck cell identity, `buck-out`, `BUCK`, `.buckconfig`, or
  direct-local assumptions?
- Does it rely on process-global state or fallback scanners?
- Which DICE key owns the semantic value in V2?
- Is a retained Buck2 utility or V1 shared-DAG primitive preferable to a new
  owned `String`, collection, recursive copy, or text-derived digest?
- Which Bazel source/test proves the behavior?
- Which command proves the extraction after landing?
- Can every source and V2 commit be recovered with `git show`, without relying
  on a movable branch name?

## Ledger

Source-path convention: `slug-v1-archive:path` explicitly names archived V1
content at `e218054d4c796655939b968d90208b185decb352`; both
`slug-v1-archive^{commit}` and `v1-archive` were verified at that commit for
this ledger baseline. Every archived V1 source, test, fixture tree, and plan
path in this ledger must carry the `slug-v1-archive:` qualifier; an unqualified
path denotes retained active-root or V2 content, never an implicit V1 fallback.
A later source revision must record its own exact commit rather than inheriting
this baseline implicitly.

| Status | V2 Stage | Source Surface | Import Mode | Oracle / Validation |
|--------|----------|------------|-------------|---------------------|
| Proposed | Stage 2 | Retained Buck2-derived DICE runtime: `dice/dice/src/transaction.rs`, `dice/dice/src/api/computations.rs`, `dice/dice/src/api/key.rs` | Adopt behind the V2 runtime wrapper; expose no Buck cells or labels | Generate and check in the `simple-rule-action` and `load-invalidation` Bazel expectations first; then prove a real transaction and same-daemon recomputation |
| Proposed | Stage 2 | Retained Buck2-derived Starlark runtime: `starlark-rust/starlark/src/eval.rs`, `starlark-rust/starlark/src/eval/runtime/evaluator.rs`, `starlark-rust/starlark/src/eval/runtime/file_loader.rs` | Adopt the evaluator/compiler runtime only; Stages 4 and 5 own Bazel loading and globals | Generate the `simple-rule-action` Bazel expectation first; then prove the V2 runtime invokes `Evaluator::eval_module` |
| Proposed | Stage 4 | Retained evaluator `starlark-rust/starlark/src/eval.rs`; V1 loading lessons in `slug-v1-archive:app/slug_interpreter_for_build/src/interpreter/calculation.rs`, `slug-v1-archive:app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs`, `slug-v1-archive:app/slug_interpreter_for_build/src/interpreter/interpreter_for_dir.rs` | Rewrite the `BUILD.bazel`/`.bzl` load boundary around Stage 3 labels and Stage 4 DICE keys; reject Buck file/cell semantics | Generate and check in `build-file-loading` and `load-invalidation` with Bazel first; then compare the Slug package/load results |
| Proposed | Stage 5 | Retained evaluator `starlark-rust/starlark/src/eval.rs`; V1 module-evaluation lessons in `slug-v1-archive:app/slug_bzlmod/src/parser.rs`, `slug-v1-archive:app/slug_bzlmod/src/globals.rs` | Rewrite `MODULE.bazel` evaluation with V2-owned globals and Stage 5 DICE keys; retain directive recording only as scaffold | Generate and check in `module-file-directives` and `simple-rule-action` with Bazel first; then compare Slug module evaluation and invalidation |
| Proposed | Stages 3 / 6 | V1 utility wrappers `slug-v1-archive:app/slug_core/src/target/label/interner.rs`, `slug-v1-archive:app/slug_util/src/arc_str.rs`, `slug-v1-archive:app/slug_util/src/hash.rs`; retained Buck2-derived utilities `starlark-rust/starlark_map/src/small_map.rs`, `starlark-rust/starlark_map/src/small_set.rs`, `shed/static_interner/src/lib.rs`, `gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, `gazebo/strong_hash/src/lib.rs` | Selective port or retained dependency by measured hot-path need; keep wrappers V2-owned | Generate `labels-and-output-paths`, `custom-rule-analysis-basic`, or `depset-orders-and-rejections` first as appropriate; then add focused allocation and determinism tests |
| Proposed | Stage 6 | V1 shared-DAG sources `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`, `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`; archived design record `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/54-depset-transitive-set-shared-core.md` | Port shared node/traversal concepts; keep the Bazel depset facade V2-owned and reject implicit `transitive_set` coercion | Generate `depset-orders-and-rejections` first; then prove shared child identity and no implicit flattening |
| Proposed | Stage 7 | V1 input-tree source `slug-v1-archive:app/slug_execute/src/execute/inputs_directory.rs`; V1 protocol source `slug-v1-archive:remote_execution/oss/re_grpc_proto/proto/build/bazel/remote/execution/v2/remote_execution.proto` | Port the protobuf/Merkle contract behind V2 action types; reject Buck paths and executor configuration | Generate serialized `Command`/`Directory`/`Action` expectations first; then run `shell-action-reapi` and `reapi-paramfile-input-tree` through NativeLink |
| Proposed | Stage 5 | `slug-v1-archive:app/slug_bzlmod/src/parser.rs` | Rewrite through starlark-rust; retain directive recording only as scaffold | `MODULE.bazel` evaluation fixtures against Bazel |
| Proposed | Stage 5 | `slug-v1-archive:app/slug_bzlmod/src/extension_execution_dice.rs` | Rewrite from behavior plus selective port | module extension replay fixtures |
| Proposed | Stage 5 | `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py` | Port tests after fixture cleanup | bzlmod same-daemon replay fixtures |
| Proposed | Stage 7 | `slug-v1-archive:tests/plan34/test_reapi_local_executor_smoke.py`; paramfile fixture tree `slug-v1-archive:tests/plan34/fixtures/paramfile` | Port the harness and fixture inputs after Stage 6 can declare the matching action graphs | `shell-action-reapi`, `reapi-paramfile-input-tree`, and `reapi-generated-output-reupload` |
| Proposed | Stage 7 | `slug-v1-archive:tests/plan34/validate_reapi_evidence.py`, `slug-v1-archive:tests/plan34/test_ci_gate.py`, `slug-v1-archive:.github/actions/setup_plan34_nativelink/action.yml`, `slug-v1-archive:.github/actions/run_plan34_reapi/action.yml`, `slug-v1-archive:.github/workflows/plan34-reapi.yml` | Rewrite behavior into V2 oracle harness; copy only small validator schema ideas | `shell-action-reapi` plus evidence validator |
| Proposed | Stage 7 | `slug-v1-archive:app/slug_execute/src/execute/action_digest.rs`, `slug-v1-archive:app/slug_execute/src/execute/action_digest_and_blobs.rs` | Selective port of protobuf/blob assembly; rewrite only the V2 action boundary | REAPI action identity fixtures |
| Proposed | Stage 7 | `slug-v1-archive:app/slug_execute_impl/src/executors/re.rs`, `slug-v1-archive:app/slug_execute/src/re/client.rs`, `slug-v1-archive:app/slug_execute/src/re/remote_action_result.rs`, `slug-v1-archive:app/slug_execute_impl/src/re/download.rs` | Selective rewrite; reject Buck executor/config assumptions | upload/execute/download REAPI fixtures |
| Proposed | Stage 7 | `slug-v1-archive:app/slug_execute_impl/src/sqlite/action_cache_db.rs`, `slug-v1-archive:app/slug_execute_impl/src/sqlite/tables/action_cache_table.rs`, `slug-v1-archive:app/slug_execute_impl/src/executors/action_cache.rs`, `slug-v1-archive:app/slug_execute_impl/src/executors/caching.rs`, `slug-v1-archive:app/slug_server/src/daemon/state.rs`, `slug-v1-archive:tests/plan31/test_persistent_re_action_cache.py`, `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/31-bazel-perf-parity.md` | Port schema/value semantics, normal materializer-path reuse, and stale-entry behavior; Stage 3 owns output/cache layout | durable RE `ActionDigest -> ActionResult` hit/stale-entry fixtures |
| Proposed | Stage 7 / Stage 8 | archived fixture trees `slug-v1-archive:tests/plan34/fixtures/cc_actions` and `slug-v1-archive:tests/plan34/fixtures/rules_cc`; evidence owner `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/34-sandboxed-execution-strategy.md` | Stage 7 owns execution evidence; Stage 8 owns ruleset conformance breadth | `rules-cc-reapi-basic` plus Stage 8 rules_cc fixtures |
| Reference only | Stage 7 | NativeLink source checkout | Backend contract reference, not Slug import | same REAPI evidence with `remote_service=nativelink` |
| Reference only | Stage 7 | actiond source checkout | Optional REAPI backend validation only | same REAPI evidence with `remote_service=local_actiond` |
| Partially landed | Stage 6 | `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`; `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs` | Rewrite behavior only; shared-DAG extraction remains proposed | V2 commits `9e519f97`, `ed636308`, `aa9b820f`; fixtures `depset-orders-and-rejections`, `custom-rule-analysis-basic`, `ctx-attrs-files-executable`, `default-info-runfiles-executable`, `provider-output-group-basic`; validation in Stage 6 plan |
| Proposed | Stage 3 | `slug-v1-archive:app/slug_core/src/target/label/interner.rs`, `slug-v1-archive:app/slug_bzlmod/src/repo_mapping.rs`, and `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/26-string-interning.md` | Rewrite label/repository semantics; selectively reuse typed interning utilities | Bazel label/output path oracle fixtures plus allocation/determinism tests |
| Proposed | Stage 4 | `slug-v1-archive:app/slug_interpreter_for_build/src/interpreter/globals.rs`, `slug-v1-archive:app/slug_interpreter_for_build_tests/src/interpreter.rs`, `slug-v1-archive:app/slug_interpreter_for_build_tests/src/functions/load_symbols.rs` | Port focused tests and rewrite the loading boundary | Bazel `PackageFunction`/`BzlLoadFunction` fixtures |
| Proposed | Stage 6 | `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/cc_common/feature_config.rs`, `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/cc_common/providers.rs`, `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/cc_common.rs` | Port selectively | rules_cc public fixture plus Bazel analysis oracle |
| Proposed | Reject by default | Direct-local executor success, copied-output bridge hits, Buck output-root spelling, and old compiled NativeLink/actiond adapter artifacts | Reject | Does not prove V2 Bazel REAPI parity |
| Proposed | Reject by default | V1 Buck cell graph and fallback cell machinery | Reject unless Stage 5 proves a Bazel equivalent | Stage 3/5 identity and bzlmod tests |
| Proposed | Reject by default | V1 BXL user surface | Defer as Slug extension | Not part of Bazel compliance |
| Reference only | Stage 0 | Mixed-root `codex/slugv2` V1 source/test paths, old docs/plans, root Bazel/Buck metadata, shims, wrappers, and `remote_execution/` | Reject from active root; inspect through archive refs or prototype branch only | `scripts/v2_archive_status.sh`; clean-root tracked-file grep in Stage 0 |
| Landed | Stage 1 | `codex/slugv2` Stage 1 oracle harness and fixtures introduced by `5181fabb` | Retain as V2-only scaffold | Owner plan `01-compliance-oracle-harness.md`; `python3 -B -m tools.v2_oracle list`; `python3 -m pytest -q tests/v2_oracle/test_v2_oracle.py` |
| Landed | Stage 2 | `codex/slugv2` Stage 2 CLI/core V2 crates introduced by `75d8147a` | Retain as V2-only scaffold | Owner plan `02-rust-skeleton-and-runtime-substrate.md`; `cargo check -p slug_cli_v2 -p slug_core_v2`; `cargo test -p slug_cli_v2` |
| Partially landed | Stages 3-8 | `codex/slugv2` V2 crate and fixture checkpoints under `app/slug_*_v2` and `tests/v2_oracle/`; recoverable mixed-root source snapshot `70c5e924`, beginning with Stage 3 commit `fa4af489` | Retain only as scaffolding or where the owner records fixture/citation evidence; exact accepted commits are recorded in the stage owners and detailed entries below | Owner plans `03` through `08`; focused cargo tests and oracle fixture commands recorded per stage |

## Evidence Template

Use this when updating a ledger row after landing. `Landed` and
`Partially landed` entries must name a full commit id or a repository-unambiguous
abbreviation verified with `git cat-file -e <id>^{commit}`. An inclusive range
is allowed only when every commit in that range belongs to the entry. A branch
name is useful orientation but does not replace either commit field. For a
multi-commit entry, map each commit to its fixture or implementation slice and
point to the exact companion evidence entry instead of copying its full detail.

```text
Status:
Source ref/commit(s):
V2 commit(s):
Source class:
Source inspected:
Reusable primitive or lesson:
V2 wrapper/boundary:
Bazel oracle:
V2 fixture:
Expected evidence artifact:
Implementation summary:
Validation:
Residual risk:
```

## First-Real-Build Reuse Order

Use this order for every packet in the canonical integration gate before
expanding standalone substrates:

1. Stage 1 adds or strengthens the narrow fixture and its comparison fields.
2. Run the fixture with Bazel 9, generate and check in the expected oracle,
   verify `expected/oracle.json` records `generated: true`, then rerun Bazel
   without `--update-expected`. Do not start implementation while the expected
   result is still a placeholder.
3. Stage 2 connects the packet to actual DICE and starlark-rust runtime paths.
4. Stage 4 evaluates the packet's `BUILD.bazel` and loaded `.bzl` files through
   Bazel-shaped loading keys and globals.
5. Stage 5 evaluates its `MODULE.bazel` through starlark-rust and actual
   bzlmod DICE keys; a directive recorder is not this integration.
6. Stage 6 carries the evaluated rule through providers, shared-DAG depsets,
   and the declared action IR.
7. Stage 7 serializes the real REAPI `Command`, `Directory`, and `Action`,
   constructs the input Merkle tree, and executes through NativeLink.
8. Stage 1 runs the same fixture with Slug, compares it with the checked-in
   Bazel result, validates REAPI and same-daemon evidence where applicable,
   and only then records landed evidence here and in each owner plan.

Do not call a `*DiceKey` structure, a parser record, a text digest, or an
evidence validator an implementation of its corresponding runtime boundary
until this chain exercises it. A later packet must repeat the Bazel-first step;
it cannot inherit an unrelated fixture's generated oracle as implementation
permission.

### Stage 5 MODULE.bazel parser directive records

Status: Partially landed
V2 commit(s): `9a5faa1d`, `2f6049e4`, `b6add7d7`, `3252e8b1`, `42b10f93`, `c484d9bf`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entries "Stage 5 repo directive parser checkpoint", "Stage 5 module extension tag parser checkpoint", "Stage 5 registration dev-dependency parser checkpoint", "Stage 5 use_repo_rule dev-dependency checkpoint", "Stage 5 multiline MODULE directive parser checkpoint", and "Stage 5 single-quoted MODULE string parser checkpoint", respectively
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/parser.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 oracle fixtures for MODULE directive shape
V2 fixture: `module-repo-directives`, `module-extension-tags`, `module-registration-dev-dependency`, `module-use-repo-rule-dev-dependency`, `module-multiline-directives`, `module-single-quoted-directives`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected` for each fixture
Implementation summary: Ported behavior selectively by rewriting directive capture in the independent V2 parser: repository-rule proxies, extension repo overrides/injections, extension tags, registration `dev_dependency` flags, `use_repo_rule` invocation-vs-factory `dev_dependency` validation, multiline logical statement collection, and Starlark single-quoted string handling; no V1 Starlark evaluator, Buck cell, or process-global machinery was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle runs for each listed fixture; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: parser remains a lightweight directive recorder; full Starlark MODULE evaluation, extension execution, repo mapping replay, and lockfile lifecycle are still later Stage 5 DICE slices

### Stage 5 local module graph substrate

Status: Partially landed
V2 commit(s): `9499cfaa`, `e8a815d4`, `3e9669b5`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entries "Stage 5 local module graph substrate checkpoint", "Stage 5 local override declared-version checkpoint", and "Stage 5 local override request-order checkpoint", respectively
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/resolution.rs`
Bazel oracle: Bazel 9.1.1 `module-resolution-basic` fixture with build plus cquery evidence
V2 fixture: `module-resolution-basic`, `module-local-override-version-selection`, `module-local-override-request-order`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the local-override graph behavior into a small V2 substrate: typed module keys, root/local sources, Bazel-shaped `<module>+` canonical repo names, deterministic apparent-to-canonical repo mappings, local override declared-version selection, and order-independent repeated local override requests; registry MVS, yanked policy, lockfiles, DICE keys, and repository materialization remain later Stage 5 work
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle runs for `module-resolution-basic`, `module-local-override-version-selection`, and `module-local-override-request-order`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: local graph substrate still does not implement multiple_version_override, registry-backed modules, yanked policy, or lockfile replay; those remain owned by Stage 5.2, 5.3, and 5.6

### Stage 5 registry MVS substrate

Status: Partially landed
V2 commit(s): `242568cb`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 registry MVS substrate checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:app/slug_bzlmod/src/resolution.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-mvs-basic` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `module-registry-mvs-basic`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the first registry-backed MVS behavior into V2: typed registry module records, max requested-version selection, registry-backed module sources, and repo mapping through the existing `ResolvedGraph`; no V1 registry client, cache, lockfile, or fetch/materialization implementation was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-mvs-basic`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry file hashes, source.json repo spec materialization, multiple-version overrides, lockfile replay, and DICE keys remain later Stage 5.2/5.6 work

### Stage 5 yanked-version policy substrate

Status: Partially landed
V2 commit(s): `bbefd325`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 yanked-version policy checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `yanked-version-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the yanked selected-version policy into V2: selected yanked versions are rejected by default and accepted through an explicit allowlist or allow-all policy over the resolved graph; no V1 registry client, lockfile writer, environment policy, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry file hashes, lockfile selected-yanked-version recording, `BZLMOD_ALLOW_YANKED_VERSIONS`, multiple-version overrides, source.json repo specs, and DICE keys remain later Stage 5.2/5.6 work

### Stage 5 registry source.json policy substrate

Status: Partially landed
V2 commit(s): `7a459f21`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 registry source.json policy checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `registry-source-json-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `registry-source-json-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only archive `source.json` metadata validation into V2: structured JSON parsing for `url`/`urls`, `integrity`, `type`, `strip_prefix`, `patches`, and `patch_strip`, with Bazel-shaped diagnostics for missing source URL, missing integrity, and malformed JSON; no V1 registry client, downloader, repository materializer, lockfile writer, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `registry-source-json-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: archive download/extraction, patch application, registry hash enforcement, repository materialization, lockfile replay, and DICE-owned registry metadata keys remain later Stage 5.2/5.5/5.6 work

### Stage 5 registry metadata parser substrate

Status: Partially landed
V2 commit(s): `665403bf`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 registry metadata parser checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `yanked-version-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only registry `metadata.json` parsing into V2: required `versions`, optional `homepage`, optional `repository`, and `yanked_versions`, with conversion from version-string yanked entries into the existing `ModuleKey -> reason` validation input; no V1 registry client, fetch cache, lockfile writer, environment policy, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry clients, registry file hashes, lockfile selected-yanked-version recording, `BZLMOD_ALLOW_YANKED_VERSIONS`, multiple-version overrides, and DICE-owned metadata keys remain later Stage 5.2/5.6 work

### Stage 5 multiple-version override resolver substrate

Status: Partially landed
V2 commit(s): `f92de49f`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 multiple-version override resolver checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:app/slug_bzlmod/src/resolution.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-multiple-version-override` fixture using a workspace-local registry, `bazel mod graph`, and `bazel mod dump_repo_mapping`
V2 fixture: `module-registry-multiple-version-override`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only selected registry graph behavior for root `multiple_version_override`: allowed requested versions remain selected side by side, ordinary modules still use MVS highest-version selection, and canonical repos for multiple selected versions use Bazel-shaped `<name>+<version>` names; no V1 registry client, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-multiple-version-override`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: multiple-version override lockfile data, exact full repo-mapping fixture coverage, registry client fallback, DICE-owned graph keys, and materialization remain later Stage 5.2/5.3/5.6 work

### Stage 5 single-version override resolver substrate

Status: Partially landed
V2 commit(s): `8f0c5d94`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 single-version override resolver checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:app/slug_bzlmod/src/resolution.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-single-version-override` fixture using a workspace-local registry, `bazel mod graph`, and `bazel mod dump_repo_mapping`
V2 fixture: `module-registry-single-version-override`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only selected registry graph behavior for root `single_version_override`: override-provided versions replace requested versions before registry module lookup, normal canonical repo names are preserved, and ordinary MVS plus multiple-version override behavior remains intact; no V1 registry client, patch application, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-single-version-override`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: single-version override patches, alternate registry selection, lockfile data, registry client fallback, DICE-owned graph keys, and materialization remain later Stage 5.2/5.3/5.6 work

### Stage 5 ordered registry fallback substrate

Status: Partially landed
V2 commit(s): `1ec4bb16`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 ordered registry fallback checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/registry.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `registry-fallback-order` fixture using two workspace-local registries and `bazel mod graph`
V2 fixture: `registry-fallback-order`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only ordered registry module selection into V2: earlier registries win per module key and later registries fill misses before the existing registry resolver consumes the selected module set; no V1 HTTP client, cache, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `registry-fallback-order`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: HTTP/file registry clients, registry file hashes, refresh/error lockfile modes, same-daemon registry mutation replay, DICE-owned registry metadata keys, and materialization remain later Stage 5.2/5.6 work

### Stage 5 selected-yanked visible lockfile substrate

Status: Partially landed
V2 commit(s): `159df871`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 selected-yanked lockfile checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `lockfile-selected-yanked-version` fixture using a workspace-local yanked registry, `--allow_yanked_versions=yyy@1.0.0`, and generated lockfile-line printing
V2 fixture: `lockfile-selected-yanked-version`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only visible lockfile subset parsing into V2: `lockFileVersion`, `registryFileHashes`, and `selectedYankedVersions` are parsed from JSON and selected yanked keys become `ModuleKey` values; no V1 writer, hidden lockfile, replay/error-mode state machine, extension cache, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-selected-yanked-version`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: lockfile write/update/refresh/error modes, hidden lockfiles, extension replay entries, facts/factsVersions, environment-sourced allowlists, registry hash enforcement, DICE ownership, and same-daemon stale-data rejection remain later Stage 5.6 work

### Stage 5 registry-hash lockfile error substrate

Status: Partially landed
V2 commit(s): `2688c70f`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 registry-hash lockfile error checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-registry-hash` fixture using a stale BCR `registryFileHashes` entry and `bazel mod graph --lockfile_mode=error`
V2 fixture: `lockfile-error-mode-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only registry-hash comparison over parsed visible lockfile data into V2: expected hashes from `MODULE.bazel.lock` are compared with an explicit observed digest map and mismatches produce Bazel-shaped checksum diagnostics; no V1 registry client, hasher, writer, hidden lockfile, replay/error-mode state machine, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-error-mode-registry-hash`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: computing registry file hashes, HTTP/file registry clients, lockfile write/update/refresh modes, hidden lockfiles, same-daemon stale-registry replay, DICE-owned registry inputs, and materialization remain later Stage 5.2/5.6 work

### Stage 5 yanked-version environment allowlist substrate

Status: Partially landed
V2 commit(s): `5a5a69a9`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 yanked-version environment allowlist checkpoint"
Source inspected: `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py` and `slug-v1-archive:app/slug_bzlmod/src/registry.rs`; no env-specific allowlist implementation or fixture was imported
Bazel oracle: Bazel 9.1.1 `yanked-version-env-allowlist` fixture using `BZLMOD_ALLOW_YANKED_VERSIONS=yyy@1.0.0`
V2 fixture: `yanked-version-env-allowlist`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only env value parsing into V2: absent/empty rejects, `all` allows all, and comma-separated `module@version` entries become a `YankedVersionPolicy::AllowList`; no V1 process environment plumbing, DICE key, lockfile writer, command-line precedence logic, or same-daemon replay logic was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-env-allowlist`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: process env wiring into DICE-owned bzlmod keys, env-change invalidation, command-line/env precedence, selected-yanked lockfile writing, and same-daemon replay remain later Stage 5.2/5.6 work

### Stage 5 bzlmod DICE environment key substrate

Status: Partially landed
V2 commit(s): `6f5099b9`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 bzlmod DICE environment key checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/dice_graph.rs`, `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-env-change` fixture proving command env changes alter yanked-version policy behavior
V2 fixture: `yanked-version-env-change`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Added only key-shaped bzlmod semantic input structs into V2: lockfile mode and parsed environment policy participate in equality, hash, and stable serialization for resolved graph keys; no V1 DICE compute implementation, async locking, registry client, lockfile replay, extension execution, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-env-change`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: actual DICE `Key` wiring, digest producers for files/env/registries, registry clients, lockfile replay modes, module extension keys, and same-daemon materialization replay remain later Stage 5.2/5.4/5.6 work

### Stage 5 bzlmod command/environment policy key substrate

Status: Partially landed
V2 commit(s): `1621bf26`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 yanked command/environment policy checkpoint"
Source inspected: No V1 compute code imported for this slice; it extends the prior Stage 5 yanked-policy/DICE substrate from Bazel-observed behavior
Bazel oracle: Bazel 9.1.1 `yanked-version-command-env-union` fixture proving `--allow_yanked_versions` and `BZLMOD_ALLOW_YANKED_VERSIONS` combine as a union
V2 fixture: `yanked-version-command-env-union`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Added a command-policy key alongside the environment-policy key, made resolved bzlmod DICE inputs include both policies in equality/hash/stable serialization, and modeled the effective yanked-version policy as their union; no V1 DICE compute, CLI execution path, lockfile writer, registry client, or same-daemon replay logic was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-command-env-union`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: actual DICE `Key` wiring, command flag plumbing into evaluation, selected-yanked lockfile writes, and same-daemon command/env replay remain later Stage 5.2/5.6 work

### Stage 5 module-extension recorded-input lockfile substrate

Status: Partially landed
V2 commit(s): `2f4664fa`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 module-extension recorded file lockfile error checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs`
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-error-recorded-file` fixture proving `--lockfile_mode=error` rejects stale module extension FILE recorded inputs
V2 fixture: `module-extension-lockfile-error-recorded-file`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the visible recorded-input behavior into V2: `FILE:<label> <digest>` lockfile entries parse into typed recorded-file inputs, unknown recorded input values are preserved as raw JSON, and explicit observed digest maps produce the Bazel-shaped stale recorded-file diagnostic; no V1 lockfile writer, hidden lockfile replay, file hashing, module extension execution, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-extension-lockfile-error-recorded-file`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: real recorded-input digest production, hidden lockfile replay, lockfile update/refresh/error lifecycle, module extension execution, and same-daemon invalidation remain later Stage 5.6 work

### Stage 5 module-extension recorded-environment lockfile substrate

Status: Partially landed
V2 commit(s): `5357e84e`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 module-extension recorded environment lockfile error checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs` for recorded-input orientation only
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-error-recorded-env` fixture proving `--lockfile_mode=error` rejects stale module extension ENV recorded inputs
V2 fixture: `module-extension-lockfile-error-recorded-env`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the visible recorded-environment behavior into V2: `ENV:<name> <value>` lockfile entries parse into typed recorded-env inputs, unknown recorded input values remain raw JSON, and explicit observed environment maps produce the Bazel-shaped stale-env diagnostic; no V1 process environment plumbing, lockfile writer, hidden lockfile replay, module extension execution, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-extension-lockfile-error-recorded-env`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: real process-environment capture into DICE keys, hidden lockfile replay, lockfile update/refresh/error lifecycle, module extension execution, and same-daemon invalidation remain later Stage 5.6 work

### Stage 5 visible lockfile version substrate

Status: Partially landed
V2 commit(s): `64b43202`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 lockfile version error checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs` for lockfile-surface orientation only
Bazel oracle: Bazel 9.1.1 `lockfile-version-error` fixture proving `--lockfile_mode=error` rejects unsupported visible lockfile versions with query exit code 48
V2 fixture: `lockfile-version-error`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only visible lockfile version validation into V2: `lockFileVersion` 26 is exposed as the Bazel 9 supported version and mismatches produce the Bazel-shaped unsupported-version diagnostic; no V1 lockfile writer, hidden lockfile replay, mode state machine, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-version-error`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: actual lockfile read/update/refresh/error lifecycle, hidden lockfiles, atomic writes, version migration policy, and same-daemon invalidation remain later Stage 5.6 work

### Stage 5 registry checksum lockfile error substrate

Status: Partially landed
V2 commit(s): `5ba0180d`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 missing registry checksum lockfile error checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs` for lockfile-surface orientation only
Bazel oracle: Bazel 9.1.1 `lockfile-error-missing-registry-hash` fixture proving `--lockfile_mode=error` rejects missing registry checksum entries with query exit code 48
V2 fixture: `lockfile-error-missing-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the visible lockfile missing-registry-checksum behavior into V2: callers pass required registry file URLs and the parsed `registryFileHashes` map must contain them, otherwise V2 emits the Bazel-shaped missing checksum diagnostic; no V1 registry client, lockfile writer, refresh/update state machine, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-error-missing-registry-hash`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: real required-URL discovery, registry fetching/hash production, lockfile update/refresh/error lifecycle, hidden lockfiles, and same-daemon registry invalidation remain later Stage 5.6 work

### Stage 5 lockfile mode policy substrate

Status: Partially landed
V2 commit(s): `f065e5df`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 lockfile mode off policy checkpoint"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs` for mode-policy orientation only
Bazel oracle: Bazel 9.1.1 `lockfile-mode-off` fixture proving `--lockfile_mode=off` leaves `MODULE.bazel.lock` absent by an empty manifest root
V2 fixture: `lockfile-mode-off`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only visible lockfile read/write mode policy into V2 `LockfileMode` helpers; no V1 lockfile writer, lockfile reader, hidden lockfile replay, mode state machine, process-global flag handling, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-mode-off`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: actual lockfile reads/writes, refresh/update lifecycle, hidden lockfiles, atomic writes, and same-daemon mode transitions remain later Stage 5.6 work

## Rejection Template

Use this when a V1 surface should not be imported:

```text
Surface:
Reason rejected:
Replacement V2 owner:
Bazel oracle:
Cleanup needed in V1 archive docs:
```

## Validation

The ledger is documentation, but any entry moved out of `Proposed` must cite the
landed validation command in the owning stage plan. A first-real-build segment
also needs its generated Bazel oracle and an integration result, not only a
unit test of a standalone data model.

Doc-only validation:

```bash
git diff --check -- thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md
```

## Landed Evidence

### Stage 2 DICE and starlark-rust root-evaluation packet

Status: Pending reviewer decision; not landed
Source ref/commit(s): retained active-tree `dice/dice/src/{api/dice.rs,api/key.rs,api/computations.rs}` and `starlark-rust/starlark/src/{eval.rs,eval/runtime/evaluator.rs}`; inspected `slug-v1-archive^{commit}:app/slug_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs` at `e218054d4c796655939b968d90208b185decb352`
V2 commit(s): none; current worktree only, so this entry is deliberately not an accepted extraction record
Source class: retained Buck2-derived runtime plus V1 reference-only interpreter delegate
Reusable primitive or lesson: use a real `Dice` transaction and `Key`, then `AstModule` plus `Evaluator::eval_module`; keep the runtime wrapper V2-owned
V2 wrapper/boundary: `slug_core_v2::runtime::evaluate_workspace` and `WorkspaceEvaluationKey`; CLI dispatch reaches this boundary before analysis
Bazel oracle: Bazel 9.1.1 generated `simple-rule-action` expected result, including declared output digest `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49`
V2 fixture: `simple-rule-action`; it is oracle-ready but cannot yet run under Slug because Stage 4/6/7 semantics are not connected
Validation: `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_core_v2 -p slug_cli_v2`; `scripts/v2_archive_status.sh`; `git diff --check`
Decision pending: adopt retained DICE/starlark-rust primitives; V1 delegate is reference-only/rejected for Buck cells, package labels, file-ops, and global interpreter state; await Sol acceptance before moving this entry to Partially landed
Residual risk: no DICE-tracked root-file dependencies or same-daemon invalidation, no Bazel-shaped globals/load graph, no configured target/action path, and no REAPI execution

### Stage 6 depset/provider/rule context tests

Status: Partially landed
V2 commit(s): `9e519f97`, `ed636308`, `aa9b820f`
Source inspected: `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`, `slug-v1-archive:app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`
Bazel oracle: Bazel 9.1 depset/provider probe expectations captured in the V1 tests plus V2 oracle fixture scaffolds
V2 fixture: `depset-orders-and-rejections`, `custom-rule-analysis-basic`, `ctx-attrs-files-executable`, `default-info-runfiles-executable`, `provider-output-group-basic`
Expected evidence artifact: Stage 1 oracle expected output remains placeholder until V2 configured-target analysis can execute fixtures
Implementation summary: Rewrote behavior into V2 depset/provider/context substrates without importing V1 Buck labels, `transitive_set` coercions, or direct-local assumptions. On 2026-07-14, replaced recursive by-value depset storage with immutable shared `Arc` nodes and child slices; composition preserves child identity and flattening is explicit. Retained Buck2 `FxHashSet` is used only for flattening deduplication. The V1 nested-set sources remain behavior/reference inputs, not imported Buck-facing code.
Validation: `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_build_api_v2 depset --no-fail-fast`; the focused structural regression proves shared child identity; `cargo test -p slug_analysis_v2`; `py -3 -B tools/v2_oracle list`; Stage 6 shortcut grep recorded in `06-analysis-toolchains-and-actions.md`
Residual risk: Starlark evaluator integration and Slug-side Bazel oracle execution remain pending

### Stage 6 first-rule analysis handoff

Status: Pending reviewer decision; current worktree only
Source inspected: `slug-v1-archive:app/slug_interpreter_for_build/src/rule.rs`
for the frozen rule-callable lifecycle; retained
`starlark-rust/starlark/src/eval.rs` for `Evaluator::eval_function`; retained
Buck2 `starlark_map` and DICE primitives remain the only runtime imports
V2 wrapper/boundary: `slug_loading_v2::StarlarkRuleImplementation` retains a
frozen implementation with the loaded package; `slug_analysis_v2::analyze_loaded_rule`
owns the prepared-context evaluation and produces the existing V2
`AnalysisResult`/`DefaultInfo`/`ActionSpec` values
Validation: focused analysis regression; clean rebuilt CLI smoke from
`tests/v2_oracle/fixtures/simple-rule-action/workspace` emitted
`dice_starlark_rule_analysis` with one analyzed target and one declared action
Decision: retain V1's freeze-before-call lesson only. Do not import V1 rule
IDs, Buck labels, global interpreter context, or action registry.
Residual risk: Starlark `DefaultInfo`/`depset` return values are placeholders
while the first vertical derives `DefaultInfo.files` from declared outputs;
full provider return decoding, attrs, DICE-owned configured-target keys, and
Stage 7 execution remain open.

### Stage 7 protobuf/Merkle identity correction

Status: Pending reviewer decision; current worktree only
Source inspected: Bazel `third_party/remoteapis/build/bazel/remote/execution/v2/remote_execution.proto` at local `3579084382`, specifically the `Action`, `Command`, `Platform`, and canonical `Directory` contracts; `slug-v1-archive:app/slug_execute/src/execute/action_digest_and_blobs.rs`
V2 wrapper/boundary: `slug_reapi_v2::{command,input_tree,proto,executor}` owns a narrow wire-compatible REAPI v2 subset. It serializes `Command`/`Action`, builds child-first `Directory` blobs from normal path segments, drives CAS discovery/upload and Execution, verifies/downloads returned outputs, and exposes the V2-owned materializer to `slug_cli_v2` for `bazel-bin` output paths.
Reusable primitive or lesson: retain V1's protobuf-blob assembly boundary and Bazel's canonical ordering rules; do not retain V1 Buck executor settings, path vocabulary, or the provisional V2 debug/text digest.
Validation: a real local NativeLink CAS/AC/Execution/worker process passed the focused ignored write-action smoke through V2 `FindMissingBlobs`, `BatchUpdateBlobs`, `Execute`, output download, digest verification, and materialization; `cargo test -p slug_reapi_v2 --no-fail-fast` passed 13 non-backend tests; `cargo check -p slug_cli_v2`; `cargo fmt --check`; `git diff --check`.
Residual risk: Stage 1 has not yet driven this path through its checked-in oracle fixture. Headers/TLS/retries, directory outputs, durable remote/local AC replay, generated-output reupload, and same-daemon invalidation remain open.

### Stage 8 public ruleset fixtures

Status: Partially landed
V2 commit(s): `86e1c5d5` (`rules-cc-basic`, `bazel-skylib-basic`, `rules-python-basic`), `70c5e924` (`rules-cc-run-env`, `rules-cc-test-env-inherit`), `2645e432` (`rules-python-runfiles`), `26b05ac1` (`protobuf-basic`), `39f5b4be` (`rules-rust-basic`), `43617d18` (`rules-oci-basic-no-daemon`)
Companion evidence: fixture-introduction commits above, verified with `git log --diff-filter=A -- tests/v2_oracle/fixtures/<fixture>`; validation and residual scope are recorded under [Stage 8: Public Ruleset Fixture Start](./08-ruleset-and-command-conformance.md#public-ruleset-fixture-start)
Source inspected: `slug-v1-archive:tests/plan34/fixtures/rules_cc/MODULE.bazel`, `slug-v1-archive:tests/plan34/fixtures/rules_cc/BUILD.bazel`, `slug-v1-archive:tests/plan34/fixtures/rules_cc/hello.c`
Bazel oracle: Bazel 9.1.1 with BCR metadata for current public ruleset module versions
V2 fixture: `rules-cc-basic`, `rules-cc-run-env`, `rules-cc-test-env-inherit`, `bazel-skylib-basic`, `rules-python-basic`, `rules-python-runfiles`, `protobuf-basic`, `rules-rust-basic`, `rules-oci-basic-no-daemon`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools/v2_oracle --update-expected` for each fixture
Implementation summary: Rewrote behavior from `slug-v1-archive:tests/plan34/fixtures/rules_cc` into Bazel 9 bzlmod fixtures and added fresh skylib/python/protobuf/rules_rust/rules_oci public ruleset fixtures, including C++ run/test environment and Python runfiles coverage, without importing V1 execution or output-root assumptions
Validation: `python3 -B -m tools.v2_oracle run --fixture rules-cc-basic --tool bazel --bazel <Bazel-9.1.1-binary>`; same command for `rules-cc-run-env`, `rules-cc-test-env-inherit`, `bazel-skylib-basic`, `rules-python-basic`, `rules-python-runfiles`, `protobuf-basic`, `rules-rust-basic`, and `rules-oci-basic-no-daemon`
Residual risk: rules_oci full no-daemon image/package build still needs a Linux-backed oracle or upstream Windows wrapper fix; output/runfiles comparisons need platform-aware oracle manifests before upgrading beyond message-shape checks
