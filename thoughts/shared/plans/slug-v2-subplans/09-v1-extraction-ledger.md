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

## Immutable Source Baselines

Record a new row before consuming a later revision. Sibling checkout `HEAD`s are
convenience locations, not semantic versions.

| Source | Required baseline | Use |
|--------|-------------------|-----|
| Bazel | tag `9.2.0`, commit `8220c6198837d5c13d53fea211cf3282aa12408a` in `../bazel` | Sole parity oracle for new/acceptance fixtures. |
| Buck2 | commit `088c75c7e36805df99c3de29062baa95db700b8b` in `../buck2` | Rust architecture and selective infrastructure/query/analysis utility reuse. |
| Slug V1 | commit `e218054d4c796655939b968d90208b185decb352` via `slug-v1-archive` | Test themes, useful implementation lessons, and explicitly approved ports. |
| actiond | commit `ca39423bbd78916457f3225dcab826283c18f412` in `../actiond` | Local REAPI backend reference/testbed; no direct Slug-core integration. |
| llvm-project | no valid `HEAD` during the 2026-07-22 review | Optional future stress corpus only after the checkout is populated. |

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
| Adopted | Stage 6 | Retained `gazebo/strong_hash/src/lib.rs` trait plus workspace `blake3`; reviewed against V1 `slug-v1-archive:app/slug_util/src/strong_hasher.rs` and live Buck2 `/run/media/system/Colossus/dev/buck2/app/buck2_util/src/strong_hasher.rs` | Keep `StrongHash` only as an adapter that writes Slug's frozen canonical byte stream; use domain-separated BLAKE3 directly for the full 32-byte projection. Do not import V1/Buck2 native-endian primitive hashing or treat the projection as structural/DICE/REAPI identity | `slug_configuration_v2` golden projection, all-341 construction, enum/domain discrimination, and semantic-regex tests; `slug_core_v2` C0-C1-C0 plus in-memory/durable collision tests. Residual: this intentionally does not match Bazel's configuration checksum or `bazel-out` spelling; later M9 may analyze and reproduce those bytes in Rust only |
| Adopted | Stage 6 | Retained `compact_str::CompactString`, immutable `Arc` slices, and `allocative::Allocative` | Retain canonical key-ordered Platform `exec_properties` directly on the analyzed node; scan the existing action closure for borrowed resolution, with no second index or graph | Focused loading retention, analysis reorder/value/restoration, and core resolved-view/absent-platform tests |
| Adopted | Stage 6 | Existing V2 configuration canonical-byte framing and immutable `Arc<[u8]>` ownership; Buck2 `strong_hash` reviewed but not used as identity | Encode the admitted resolved FileWrite structure into versioned tagged bytes; exact bytes are identity, with no digest, projection, retained graph pointer, or new cache | Focused framing, legacy rejection, field discrimination, normalized property-order, and constraint A/B/A restoration tests |
| Adopted | Stage 6 | Existing workspace BLAKE3 derive-key and lowercase-hex projection pattern; Buck2 `strong_hash` remains unnecessary over already frozen canonical bytes | Derive the full 32-byte `slugact-display-v1:` formatter projection from complete FileWrite semantic identity with context `slug.v2.filewrite.aquery-display.v1`; allocate only request-local display/final strings and retain no token, cache, equality, or graph state | Exact formatter golden plus C0/C1/C0, P0/P1/P0, content/output/property/constraint token relations and fail-closed formatter tests. Residual: the projection deliberately does not match or substitute for Bazel ActionKey, DICE/cache identity, configured paths, or REAPI/CAS digests |
| Proposed | Stages 6 / 8 | Zabel `c7298478e2e56262a2f438e9c065325744c9f0fc` fingerprint/internal-string/FileWrite/common-tail leaves; retained workspace SHA-256 implementation | Reimplement a small V2-owned `BazelFingerprint` and per-family projection from Bazel 9.2 source; compute from the immutable action row in request/phase scratch; do not reuse Buck2 fast hashes, `StrongHash`, Slug canonical bytes, Zabel's monolithic dispatcher, or the REAPI Action digest | Fresh Bazel 9.2 source anchors plus exact FileWrite regular/compressed vectors, conditional-field mutations, platform/property ordering discriminators, and an output-name cross-domain test proving Bazel ActionKey and REAPI ActionDigest are not aliases |
| Proposed | Stage 6 | Buck2 `../buck2/app/buck2_analysis/src/analysis/calculation.rs`, `analysis/env.rs`, `../buck2/app/buck2_build_api/src/analysis/registry.rs`, and attribute coercion/interning under `../buck2/app/buck2_interpreter_for_build/`; V1 counterparts `slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs`, `slug-v1-archive:app/slug_analysis/src/analysis/env.rs`, `slug-v1-archive:app/slug_build_api/src/actions/registry.rs`, and `slug-v1-archive:app/slug_interpreter_for_build/src/attrs/coerce/` | Port the DICE analysis-key, recursive dependency, prepared environment, registry, and compact attribute patterns behind V2 Bazel types; reject Buck cells/labels/configurations/output paths | Bazel 9.2.0 `RuleConfiguredTargetTest`, Starlark rule-context/implementation tests, plus multi-target provider/action and same-daemon invalidation fixtures |
| Adopted | Stage 6 | Existing V2 scalar `AttributeSchema` storage and retained `allocative::Allocative`; reviewed against Buck2 attribute coercion/interning, archived V1 attribute coercion, and Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` captured-declaration policy | Add one V2-owned `AttributeFlags(u32)` value covering the pinned 25-name bit capacity. Retain normalized bits only; import no map, set, string slice, interner, cache, V1/Buck2 schema, or Zabel representation | Pinned Bazel 9.2 constructor/`EnumSet` sources; five-constructor binding matrix; duplicate/reorder retained-schema equality; exact-source A/B/A restoration without a cross-source DICE-cutoff claim; `Allocative`/size and final package-lowering proofs; real rules_rust replay. Residual: only `DIRECT_COMPILE_TIME_INPUT` is admitted initially and its unsupported `compile_one_dependency` consumer remains deferred |
| Adopted | Stage 6 | Existing V2 `CompactString`, immutable `Arc<[T]>`, `Allocative`, structural `SlugConfiguration::configured_action_path_flavor()`, configured-dependency validation, and `ActionOutputKind::Directory`; reviewed against Buck2 parse fixtures and Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` ordered `allow_single_file` capture | Replaced the split Boolean/optional schema and old public carrier with one V2-owned typed file-admissibility value: no/any/exact ordered suffixes plus independent single-artifact state. Shared immutable suffix storage and only selected structural Host flavor drive suffix matching; Boolean any/no-file paths observe no Host state. No ambient OS read, raw evaluator value, second collection, interner, cache, DICE key, or imported schema | Pinned Bazel 9.2 API/conversion/Attribute/FileType/RuleContext sources; complete five-constructor and scalar-only binding matrix; retained order/duplicate/empty/single-bit and same-DICE configured result/error A/B/A; Unix/Windows/missing-Host, source/generated/directory/singleton, dictionary-orientation and conversion-control proofs; 552 loading plus 117 analysis tests; real rules_rust replay to the later coverage configuration field; independent terminal `ACCEPT` |
| Proposed | Stage 8 | Buck2 generic query parser/evaluator/graph machinery in `../buck2/app/buck2_query_parser`, `../buck2/app/buck2_query`, and `../buck2/app/buck2_query_impls`; matching V1 crates `slug-v1-archive:app/slug_query_parser`, `slug-v1-archive:app/slug_query`, `slug-v1-archive:app/slug_query_impls`, and `slug-v1-archive:app/slug_cmd_query_server` | Port parser spans, generic evaluator, traversal, compact deterministic sets, and uquery/cquery/aquery environment separation; replace every Buck literal, cell, pattern, function registry, configured node, action, diagnostic, and printer with Bazel 9 behavior | Bazel 9.2.0 `QueryParserTest`, `AbstractQueryTest`, `ConfiguredTargetQuerySemanticsTest`, `ProtoOutputFormatterCallbackTest`, `ActionGraphQueryTest`, and exact `ActionGraphContainer` fixtures |
| Rejected | Stage 8 | `java_regex` 0.1.0; published SHA-256 `1f3b3ff81a66205722b636dae12fc5cb2e77147569e8968f38a1d73b2b05fbe6`; packaged/upstream commit `ed518dc23dacbe1a88d7cb3f26f0cfe31cc91393` | Reject the published API/source as a production substrate; reference only, with no dependency or port. Any future V2-owned UTF-16 engine requires a new reviewed row | `java-pattern-utf16` proves Java `\uD800` finds an unpaired surrogate but not NUL; the crate lowers the escape to NUL and finds NUL, while `&str` cannot represent the Java subject. Supply-chain pin, `MIT OR Apache-2.0`, Rust 1.78, four normal Unicode dependencies, 7/14 measured find allocations, and ordinary-false 5,000,000-step/500-depth limits are retained rejection evidence |
| Proposed | Stage 8 | V1 `slug-v1-archive:tests/core/query/test_bazel_compat_query.py` plus focused `tests/core/query/{uquery,cquery,aquery}` themes | Migrate useful graph/function/set/format scenarios, rewriting fixture metadata and expected results against Bazel 9.2.0; V1 golden output is not the oracle | Stage 1 `query-parser-and-sets`, `query-functions-and-patterns`, `cquery-provider-starlark`, and expanded `aquery-action-shape` fixtures |
| Reference only | Stage 10 | V1 root `MODULE.bazel`, `BUILD.bazel`, and Buck-generated build metadata | Reject as the bootstrap graph because it encodes V1/Buck-shaped ownership; inspect only for source inventory | Fresh Bazel 9.2.0 bzlmod/rules_rust graph; Bazel/BuildBuddy build and stage0→stage1→stage2 fixed-point proof |
| Proposed | Stage 6 | V1 shared-DAG sources `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/nested_set.rs`, `slug-v1-archive:app/slug_build_api/src/interpreter/rule_defs/transitive_set/traversal.rs`; archived design record `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/54-depset-transitive-set-shared-core.md` | Port shared node/traversal concepts; keep the Bazel depset facade V2-owned and reject implicit `transitive_set` coercion | Generate `depset-orders-and-rejections` first; then prove shared child identity and no implicit flattening |
| Reference only | Stage 6 | Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a`: `src/analysis/{nested_set,depset,generic_depset_traversal,file_depset_traversal,generic_depset_fingerprint,file_depset_fingerprint,file_depset_action_importer}.zig`, `src/starlark_host/engine/generic_depset.zig`, and depset oracle scenarios | Use its Bazel-specialized packed rows/bit layout, dense indexes, construction/traversal algorithms, generic/File specialization, external producer references, late materialization/action import, and invocation-local caching as the starting design for an independently implemented V2 Rust core; copy no Zabel production code. Test scenarios may be rewritten with provenance, but every expected result is regenerated from Bazel 9.2. Buck2 ideas are optional later measured refinements | Stage 6 Zabel-informed retained depset core gate; all orders plus alias/equal-leaf/diamond/depth/error-precedence fixtures; measured retained bytes, allocations, cold/warm flattening, and direct action consumption |
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
| Reference only | Stage 7 | actiond source checkout at `ca39423bbd78916457f3225dcab826283c18f412` | Preferred local REAPI conformance backend; no source import or Slug-core shortcut | same mandatory backend-neutral REAPI evidence with `remote_service=local_actiond` plus focused actiond health/e2e validation |
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

## Current Analysis/Query Integration Reuse Order

Use this order for every packet in the current M1-M5 integration path before
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
7. Stage 8 exposes the loaded/configured/action graph through `query`,
   `cquery`, or `aquery` as appropriate, using the generic query extraction
   decisions recorded above.
8. Stage 1 runs the same fixture with Slug, compares it with the checked-in
   Bazel graph result, and records the accepted analysis/query evidence.
9. Only after exact `aquery`, Stage 7 serializes the same real action to REAPI,
   constructs the input Merkle tree, and executes through BuildBuddy or
   actiond, with NativeLink retained for regression coverage.
10. Stage 1 compares execution/materialization/cache results with the checked-in
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

Status: Accepted for the DICE read boundary
V2 commit(s): `64b43202`, `6d354e10`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 lockfile version error checkpoint"; [Stage 5 evidence shard 2](./05-bzlmod-checkpoint-evidence-2.md), entry "Stage 5 visible-lockfile v28 DICE read"
Source inspected: `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs` for early surface orientation only; implementation is grounded directly in Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a` `BazelLockFileFunction.java` and `BazelLockFileValue.java`
Bazel oracle: Bazel 9.2.0 `lockfile-version-error` fixture proving `--lockfile_mode=error` rejects unsupported visible lockfile versions with query exit code 48; the read packet does not claim the external exit transport
V2 fixture: `lockfile-version-error`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: The earlier V2 validator is now corrected to visible lockfile version 28 and consumed by a real workspace-scoped DICE key. The key mirrors Bazel's first Java-ASCII marker and signed-32-bit scan before JSON parsing, preserves semantic EMPTY/equality, and recovers across retained create/edit/delete/recreate transitions; no V1 reader/writer, hidden replay, registry client, or materializer was imported.
Validation: full `slug_bzlmod_v2`, `slug_loading_v2`, and `slug_core_v2` suites; focused scan/equality/lifecycle activation tests; formatting, diff, archive checks; independent final review `ACCEPT`
Residual risk: Registry/yanked hash enforcement and production, command-owned writes, exact external exit 48, hidden lockfiles, and later replay/materialization remain serial Stage 5 work.

### Stage 5 registry/yanked owner design

Status: Accepted as an oracle-first replan
V2 commit(s): N/A (documentation-only design checkpoint)
Companion evidence: [Stage 5 evidence shard 2](./05-bzlmod-checkpoint-evidence-2.md), entry "Stage 5 registry/yanked resolution owner design"
Source inspected: no V1 registry client was selected; ownership is grounded directly in pinned Bazel 9.2 registry, module-discovery, resolution, yanked, RepoSpec, and lockfile-update source
Bazel oracle: accepted pinned update/refresh and command/environment-union fixtures are insufficient; older version-26/local-registry fixtures remain non-acceptance corroboration
Expected evidence artifact: one controlled loopback HTTP registry fixture pinning version-28 update replay, refresh refetch, recorded absence, checksum enforcement, error precedence, and no write after failure
Design summary: Final selected-yanked and registry-hash products are post-MVS in Bazel, so the superseded pre-MVS unified packet is rejected. The serial replacement is controlled oracle, DICE-owned policy/IO, request transport, per-module discovery, MVS, selected-yanked/RepoSpec aggregation, then command-owned writing.
Validation: pinned-source/live-substrate/fixture audits and independent corrected-design review `ACCEPT`
Residual risk: The controlled remote oracle, DICE-owned registry policy/IO,
and primitive command/daemon transport are accepted through `2777b6f8`.
Per-module discovery, MVS, selected-yanked/RepoSpec aggregation, semantic
writing, extensions, and materialization remain pending; no V1 cache, pure
MVS helper, raw writer, or process-global semantic state is authorized.

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

Status: Partially landed; read side accepted
V2 commit(s): `f065e5df`, `6d354e10`
Companion evidence: [Stage 5 evidence shard 1](./05-bzlmod-checkpoint-evidence.md), entry "Stage 5 lockfile mode off policy checkpoint"; [Stage 5 evidence shard 2](./05-bzlmod-checkpoint-evidence-2.md), entry "Stage 5 visible-lockfile v28 DICE read"
Source inspected: V1 only for early mode-policy orientation; the live read owner is grounded directly in pinned Bazel 9.2.0 lockfile source
Bazel oracle: Bazel 9.2.0 `lockfile-mode-update-refresh` and `lockfile-version-error`
V2 fixture: owner-local retained DICE mode/equality/recovery tests; accepted Bazel fixtures remain evidence for later registry/write packets
Expected evidence artifact: `off` has no visible workspace-file dependency; update/refresh/error use the observed file with pinned read precedence and retained mode restoration
Implementation summary: Existing pure `LockfileMode` helpers now drive the bzlmod-owned visible read key. `off` returns before file observation; update/refresh/error read through the neutral workspace key, and update→off→update is request-local and recoverable in retained loading/runtime graphs. No writer, refresh registry policy, hidden replay, or materializer was imported.
Validation: full bzlmod/loading/core suites; focused activation dependency and A→B→A regressions; formatting, diff, archive checks; independent final review `ACCEPT`
Residual risk: Registry cache/refetch semantics, produced hashes and selected-yanked replay, exact update/refresh/error enforcement, command-owned semantic writes, and hidden lockfiles remain later serial packets.

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

Status: Partially landed
Source ref/commit(s): retained active-tree
`dice/dice/src/{api/dice.rs,api/key.rs,api/computations.rs}` and
`starlark-rust/starlark/src/{eval.rs,eval/runtime/evaluator.rs}`; Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_common/src/file_ops/{dice.rs,metadata.rs}`; inspected V1
`e218054d4c796655939b968d90208b185decb352` interpreter delegate, globspec,
and watcher only as rejection/reference material
V2 commit(s): `3659b0f9`, `35612655`
Source class: retained Buck2-derived DICE/Starlark runtime and compact utility
shapes behind V2-owned inputs; V1 reference-only behavior
Reusable primitive or lesson: retain one DICE owner and transaction, injected
immutable snapshots, per-value equality, compact names, and sorted shared
directory entries; reject V1 Buck cells, labels, file-ops, global interpreter
state, globspec, and watcher policy
V2 wrapper/boundary: `slug_core_v2::runtime::WorkspaceRuntime` owns one
workspace DICE instance; file and directory snapshots enter one updater/commit;
root and package loading share its transaction; `WorkspaceDirectoryKey` is
demand-driven and reads no filesystem state
Bazel oracle: Bazel 9.2.0 generated
`glob-directory-invalidation` create/rename/delete expectations at Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`; earlier `simple-rule-action`
remains the first root/action reference
V2 fixture: `glob-directory-invalidation` is generated and independently
verified; this first half establishes inputs only and does not claim Slug query
parity
Validation:
`CARGO_TARGET_DIR=/tmp/slug-m1-directory-target CARGO_BUILD_JOBS=1 cargo test
-p slug_core_v2 -p slug_loading_v2 -p slug_server_v2 -p slug_analysis_v2
-p slug_cli_v2`; `cargo fmt --all -- --check`; `git diff --check`; Sol-low
post-review `ACCEPT`
Decision: adopt the retained DICE/Starlark primitives and Buck2's compact,
sorted directory-value lesson behind V2 identities. The first eager
all-directory evidence API was rejected in review; production now computes no
directory key until a semantic consumer requests it.
Residual risk: the migration observer still scans the full workspace. The
Stage 4 package-listing consumer and activation evidence landed separately in
`de835cdc`; fine-grained watcher input remains open. Bazel-shaped configured
target/action coverage and REAPI execution remain incomplete beyond their
owning partial packets.

### Stage 4 DICE-prepared package listing and Starlark glob packet

Status: Partially landed
Source ref/commit(s): retained active-tree DICE and starlark-rust; Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_common/src/package_listing/{dice.rs,interpreter.rs,listing.rs}` and
`app/buck2_interpreter_for_build/src/interpreter/{module_internals.rs,globspec.rs,functions/path.rs}`;
V1 `e218054d4c796655939b968d90208b185decb352` globspec, calculation delegate,
and watcher inspected only as rejection/reference material
V2 commit(s): oracle `19451b23`; implementation `de835cdc`
Source class: selectively ported Buck2 prepared-listing architecture and
compact immutable utility shapes behind V2-owned Bazel package identities;
V1 behavior reference only
Reusable primitive or lesson: gather a package listing asynchronously through
DICE, then synchronously filter it during Starlark evaluation; retain
`CompactString`, immutable `Arc` slices, `Dupe`, `Allocative`, sorted values,
and key-specific `ActivationTracker` evidence
V2 wrapper/boundary: `PackageListingKey` recursively consumes only
`WorkspaceDirectoryKey`, prunes nested BUILD package boundaries, and feeds one
prepared listing to `PackageLoadKey`; global and native glob calls never read
the filesystem or suspend the evaluator
Bazel oracle: Bazel 9.2.0 generated `glob-callable-contract` and
`glob-directory-invalidation` fixtures at Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`; semantic source anchors are
recorded in the Stage 4 owner plan
V2 fixture: exact callable defaults, list/tuple inputs, explicit excludes,
directory inclusion, macro context, empty-match/type errors, and retained-DICE
create/rename/delete plus package-boundary transitions
Implementation summary: replaced the unused data-only glob key and direct
filesystem glob traversal with a compact sorted package listing. Implemented
global `glob()` and `native.glob()`, recorded used specs in loaded packages,
rejected unreviewed syntax and participating symlinks explicitly, and proved
that package loads consume the listing through one async DICE boundary.
Validation:
`CARGO_TARGET_DIR=/tmp/slug-m1-glob-target CARGO_BUILD_JOBS=1 cargo test
-p slug_loading_v2 -p slug_core_v2 -p slug_server_v2 -p slug_analysis_v2
-p slug_cli_v2`; `cargo fmt --all -- --check`; ownership and forbidden-surface
greps; `git diff --check`; Sol-low post-review `ACCEPT`
Decision: adopt Buck2's prepared-listing architecture and retained compact
utilities, but keep Bazel callable semantics and V2 key identity authoritative.
Activation evidence distinguishes no activation for an untouched cached key
from `Reused` after dependency validation.
Residual risk: the observer is still a full-workspace migration scanner;
symlink resolution, full Bazel glob syntax, ignored-path policy,
repository-aware identity, query exposure, and configured analysis remain
owned by later packets.

### Stage 4 private Host glob segment-candidate owner

Status: Landed, private and dormant
Source ref/commit(s): Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`
`PatternWithoutWildcardProducer`, `PatternWithWildcardProducer`,
`DirectoryListingFunction`, `GlobsFunction`, `GlobValue`, `GlobsValue`, and
`UnixGlob`; Buck2 baseline
`088c75c7e36805df99c3de29062baa95db700b8b` only for the already-approved
shared immutable, `Dupe`, `Allocative`, and compact value patterns; V1
`e218054d4c796655939b968d90208b185decb352` glob/loading code rejected and
retained as reference only
V2 commit(s): matcher oracle `9f42c3e5`; implementation/evidence `bd12c015`
Source class: V2-owned Bazel-parity rewrite using retained DICE/path
observation APIs; no Buck or V1 glob semantics, traversal, key ownership, or
consumer code imported
Reusable primitive or lesson: one shared raw-name-sorted candidate slice,
temporary work vectors, complete-only equality, and a batched matched-symlink
frontier; no retained standard map/set/string/vector or matcher dependency
V2 wrapper/boundary: private Unix-dormant
`HostGlobSegmentCandidatesKey` consumes one Host typed directory listing plus
only reached base/literal/matched path resolutions. It owns one validated raw
literal/simple-`*` segment and semantic candidate/error projection, but no
package boundaries, recursive traversal, include/exclude composition, parser,
evaluator retry, events, or callers.
Bazel oracle: pinned Bazel 9.2 callable, POSIX dirent, and Linux raw-name glob
evidence proves exact question-mark rejection, leading-dot behavior,
non-adjacent stars, raw-byte identity/order, symlink/special classification,
and warm restoration
Validation: focused 19, full loading 73, workspace 36, and bzlmod 387 tests;
all doctests; 20 GNU-Windows no-run executables; exact +1,605/-0 scope/caps;
format, diff, archive, credential, forbidden-surface, event, caller, and
process guards; three terminal corrected latest-diff reviews
Decision: retain only the approved compact value/worklist utility patterns
from the Buck baseline. Reject V1 glob behavior and ownership. The private
owner remains dormant until a separately reviewed Host package-boundary
projection and later recursive composition exist.
Residual risk: selected-root/no-package/deleted/ignored/subpackage states are
not yet represented. Pinned Bazel 9.2 treats a nested `MODULE.bazel` without a
BUILD marker as ordinary traversal; its retained incorrect-repository branch
is unreachable, so no nested-repository state is required. Multi-segment and
`**` traversal, native-Windows byte ordering, regular-or-special
BUILD/`.bzl` acquisition, parser/evaluator retry, and consumer publication
remain later packets.

### Stage 4 public Host root-package boundary projection

Status: Landed, public and dormant
Source ref/commit(s): Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`
`DirectoryDirentProducer`, `PackageLookupFunction`, `PackageLookupValue`,
`IgnoredSubdirectoriesFunction`, and `LocalRepositoryLookupFunction`; Buck2
baseline `088c75c7e36805df99c3de29062baa95db700b8b` only for the already
approved shared immutable, `Dupe`, `Allocative`, and complete-only DICE
patterns; V1 `e218054d4c796655939b968d90208b185decb352` loading/glob code
rejected and retained as reference only
V2 commit(s): boundary oracle `85ba4975`; implementation/evidence `ad6751ef`
Source class: V2-owned Bazel-parity projection over retained private Host
ignore and package-lookup owners; no Buck or V1 lookup, traversal, identity,
consumer, or representation code imported
Reusable primitive or lesson: one semantic workspace/package key,
`PathOutcome<Arc<Result<...>>>`, opaque complete values, cheap shared clones,
and key-specific activation/dependency evidence
V2 wrapper/boundary: public dormant `HostRootPackageBoundaryKey` computes
repository ignore first and exposes only ordinary/deleted continue,
ignored/package stop, selected package-path root, and typed error/Need
propagation. Marker basename, ignore match, invalid-name details, physical
path, and private state remain opaque.
Bazel oracle: exact pinned Bazel 9.2 six-state `glob-package-boundaries`
fixture proves ordinary, deleted-BUILD, and nested-MODULE continuation plus
actual-subpackage, ignored, and ignored-plus-deleted stops
Validation: focused 7 twice; full bzlmod 210 units plus integrations, loading
73, workspace 36, and all doctests; 20 GNU-Windows no-run executables; exact
three-file +1,133/-0 scope/caps; formatting, archive, public-surface,
dependency/caller, event, credential, process, and forbidden-surface guards;
independent correction rereview `ACCEPT`
Decision: retain only the approved compact value and DICE utility patterns
from the Buck baseline. Reject V1 behavior/ownership. Keep the projection
dormant until a separately reviewed private Host traversal composes it.
Residual risk: multi-segment and standalone-`**` traversal, operation
filtering, final deduplication, native-Windows byte ordering,
regular-or-special BUILD/`.bzl` acquisition, parser/evaluator retry, and
consumer publication remain later packets. Nested `MODULE.bazel` without
BUILD continues and no incorrect-repository detector is authorized.

### Stage 4 private Host glob traversal owner

Status: Landed, private and dormant
Source ref/commit(s): Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`
`GlobComputationProducer`, `FragmentProducer`, `DirectoryDirentProducer`, and
`GlobTestBase`; Buck2 baseline
`088c75c7e36805df99c3de29062baa95db700b8b` only for approved immutable,
`Dupe`, `Allocative`, and compact-worklist patterns; V1 glob/loading retained
as rejected reference only
V2 commit(s): traversal oracle `5abff72e`; implementation/evidence in this
commit
Source class: V2-owned Bazel-parity traversal over retained Host observations;
no Buck/V1 glob semantics, traversal, ownership, consumer, or JVM machinery
imported
Reusable primitive or lesson: shared `Arc` slices, temporary `VecDeque`, and
the existing `SmallSet` only for multi-`**` visitation; no dependency, cache,
interner, retained standard collection, or global registry
V2 wrapper/boundary: private Unix-dormant `HostGlobTraversalKey` owns one
checked full pattern and operation, composes segment candidates with
root-package boundaries, and returns sorted/deduplicated raw matches or exact
complete-error-before-Need state. It has zero production callers.
Bazel oracle: exact nine-label Bazel 9.2 traversal row plus the protected
six-state boundary row in `glob-package-boundaries`
Validation: focused traversal 13, direct boundary 7, full loading 86,
workspace 36, bzlmod 394, doctests, GNU-Windows no-run linkage, formatting,
diff/scope/caller/dependency/IO/lock/event guards, and independent final
`ACCEPT`
Decision: retain only the approved compact utility patterns. Reject V1
behavior and all JVM/Bazel production delegation. Keep the traversal dormant
until a separate private loading adapter consumes one full pattern/operation.
Residual risk: consumer/callable transactions, include/exclude/`allow_empty`,
BUILD/`.bzl` acquisition, external repositories, SUBPACKAGES, native-Windows
byte ordering, and lone-surrogate parity remain later packets.

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

### Stage 6 Zabel-informed retained depset core design

Status: Architecture accepted in
`WP-6-7A-dense-retained-depset-action-import-r1`; implementation and proof
terminally accepted in
`WP-6-7A-dense-retained-depset-action-import-implementation-r1`
Primary design reference: Zabel
`0795445f3ab60f4e49070bdd0b94425c5610f73a` generic/File depset, retained
nested-set, traversal, fingerprint, action-import, and architecture sources;
Buck2 `088c75c7e36805df99c3de29062baa95db700b8b` transitive-set nodes and
traversals remain optional sources for later measured refinements
Bazel oracle: pinned Bazel 9.2 `DepsetTest`, `NestedSetTest`,
`NestedSetTopologyTest`, `NestedSetFingerprintCacheTest`, and freshly generated
fixture results; Zabel fixtures are scenario inputs, never semantic authority
V2 fixture: extend `depset-orders-and-rejections` with topological aliases,
distinct equal leaves, diamonds, empty incompatible children, multi-child
depth, validation precedence, and supported-limit stack safety; add direct
Args/action-input consumption and cross-owner provider fixtures
Implementation summary: independently implement a Rust dense retained store
starting from Zabel's packed rows and Bazel-specific construction/traversal
algorithms, specialized File storage, external producer references, late
materialization, direct action import, and invocation-local caches. Copy no
Zabel production code. Treat the current `Arc` DAG as migration scaffolding.
Retain the declared Bazel order and exact construction-time validation, while
permitting internal traversal selection and work at consumption time when every
observable result and error boundary remains exact. Consider Buck2 ideas only
after this baseline works and measurements identify a concrete improvement; do
not expose Buck2 tset orders, projections, reductions, BFS/DFS, or coercion.
Validation: exact Bazel 9.2 outputs and diagnostics plus retained-byte,
allocation, composition, cold/warm flatten, direct-consumption, and realistic
rules_cc/rules_rust fan-in measurements; `Allocative`, release, DICE-identity,
action-projection, and stack-safety tests
Decision: select the Zabel-informed packed/dense Bazel-specialized design as
the starting point; Buck2-inspired changes require later focused measurements
Residual risk: Starlark value identity/equality, cross-owner topology lifetime,
cache bounds, exact topological alias behavior, action/Aquery topology, and
Bazel ActionKey projections remain part of the gate

### Stage 6 generic Args/spawn/artifact-symlink category

Status: Architecture accepted in
`WP-6-7A-generic-args-spawn-symlink-category-architecture-r2`; configured-
action-environment prerequisite implementation terminally accepted in
`WP-6-7A-configured-action-environment-owner-implementation-r1`
Primary design reference: Bazel 9.2 `Args`, `StarlarkCustomCommandLine`,
`StarlarkActionFactory`, public Args/action APIs and symlink implementations;
Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architecture and
optimization guidance only
Authenticated consumer: rules_cc 0.2.17
`cc/private/rules_impl/fdo/fdo_context.bzl` SHA-256
`91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`
and `cc/private/cc_common.bzl` SHA-256
`5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`
Implementation summary: the bounded prerequisite now derives the canonical
fixed-map/inherited-name environment from the sole structural option vector
and one process-latched Host fact, preserves it through structural
configuration and Exec conversion, and composes it with action-fixed values.
It retains sorted compact `Arc` slices with `CompactString`, `Dupe`, and
`Allocative`; it imports no V1/Buck2/Zabel semantic owner, retained mutable map,
interner, cache, DICE key, parser, action, or ruleset branch. Planned successors
remain one evaluator-local Args recipe finalized into typed immutable segments,
one dense-topology-aware input/tool owner, one common run/run_shell SpawnSpec,
canonical execution-requirement maps, Host-flavored normalized executable
paths, and tagged artifact/unresolved/authenticated-absolute symlink targets.
`cc_common` and `cc_internal` remain ordinary Starlark consumers/bridges, not
Rust rule or parser owners.
Decision: review the whole non-callback category architecture before the
bounded environment and scalar/FDO implementation successors; generated
artifact path bytes are Slug-native until M9, map equality ignores insertion
order, and callbacks, directory expansion, runtime paramfile decisions,
inherited client-value resolution and new execution/REAPI behavior remain
deferred
Residual risk: exact snapshot/freeze timing, repository-mapped label rendering,
FilesToRun/runfiles topology, action-owner execution context, client-
environment execution invalidation, unresolved symlink configuration and
migration from raw action vectors require the frozen successor proofs

Validation: exact pinned Bazel option/default/Exec/composition regressions;
all modeled Host OS classes; Windows path and 8.3 rejection; configuration and
environment A/B/A; concurrent one-read Host latching; public dependent tests;
and 597 production / 489 proof / 1,086 total Rust-line caps. Independent
terminal review returned `ACCEPT`.

The active bounded basic Args/run/symlink successor's first pre-review returned
`REPLAN` while accepting the environment dependency and borrowed sink. Its
focused correction binds mutable Args to the starlark-rust evaluator heap,
rejects Windows short-path candidates without a filesystem observation, reuses
the existing depset publication comparator for manual Spawn/ActionSpec equality,
and admits the generic custom tuple allowlist plus Starlark-function depth check
that authentic rules_cc invokes before its absolute symlink. No parser, set,
FDO, C++ action, second graph walk, or Zabel semantic owner is introduced. The
focused correction rereview returned `ACCEPT` and authorized the now-frozen R1
implementation only within that contract.

R1 implementation terminal review subsequently returned `REPLAN`: the action
owners themselves pass, but unchanged authentic rules_cc demonstrates that the
older string-backed `DefaultInfo.files` target projection bypasses typed
artifact identity, configured evaluation lacks its loaded recursive source
manifest, and equality-only A/B/A evidence does not prove DICE cutoff.

The active R2 correction reuses the already accepted dense `AnalysisDepset`,
`AnalysisArtifact`, iterative materializer, shared publication-equality state,
compact immutable source manifest and parent-key DICE test patterns. It imports
no V1/Buck2/Zabel provider, graph, interner, parser, action, scheduler, cache or
identity. Bazel 9.2 `FileConfiguredTarget`, `DefaultInfo`, `FileProvider` and
the existing Args/action sources remain authority; Zabel remains peer
architecture/optimization guidance only. Independent architecture acceptance
returned `ACCEPT`; correct the frozen R1 Rust candidate only within that
contract.

R2 terminal status: `ACCEPT`. The implementation publishes typed
source/generated/declared `DefaultInfo.files`, hands the recursive source
manifest into configured evaluation, proves parent-key DICE cutoff, and owns
scalar Args/run/artifact/absolute-symlink state in one generic retained action
model. The authenticated direct-source rules_cc FDO configured-analysis route
passes without a C++/FDO/parser branch; typed-action aquery/execution/ActionKey/
REAPI projection remains explicitly deferred. Validation passed at 1,849
production, 1,050 proof, and 2,899 total added Rust lines; four core failures
and three archive-path findings were independently reproduced or identified as
unchanged frozen-base state. The active
`WP-6-7A-noncallback-vector-args-paramfiles-implementation-r1` draft fills the
accepted retained recipe with sequence/depset vector calls, non-callback
transforms, param-file policy, and typed Args-backed write. Its terminal review
accepts the compact recipe and shared publication design but returns `REPLAN`:
Bazel typed binding validates a supplied sequence/depset source before callback
handling, and the one-line no-op sink test adapter was outside the proof
allowlist. Active R2 corrects only that evaluator validation seam and formally
admits the frozen adapter; it imports no donor utility or semantic owner and
leaves all retained representation, hashing, clone-cost, and memory-accounting
decisions unchanged. Focused correction and terminal rereview returned
`ACCEPT`; all owner/direct-dependent suites and hygiene gates pass within the
packet caps. Commit `a01a23fe7` freezes the implementation. The first
default-context Spawn-envelope design returned `REPLAN` for treating
`resource_set` as a direct dictionary and for failing to detect
executable-attribute-backed FilesToRun association. R2 corrected those owners
but returned `REPLAN` because top-level and sequence-nested tools depsets take
different Bazel association branches. Active R3 extends only the one retained
action owner, accepts omitted/`None` resource callbacks, forwards
scope-separated executable-Artifact provenance, validates top-level depset
leaves without flattening, and leaves nested depsets uninferred. Associated
Files fail closed instead of losing runfiles. It reuses the same compact Rust
utilities and imports no V1, Buck2, or Zabel action representation. Callback,
provider/runfiles expansion, named-exec-group, directory, and execution breadth
remains deferred.
Independent focused R3 correction review returned `ACCEPT`; no V1 extraction
decision or peer semantic ownership changes.

Commit `bfe6f2690` terminally accepts the complete default-context
non-callback Spawn envelope. The corrected evaluator binding pass, one typed
run/run-shell owner, scoped executable provenance, compact retained values and
publication proofs require no V1 extraction and import no Buck2/Zabel semantic
owner. Typed FilesToRun/runfiles expansion is now the next standard-provider
category; callbacks, named exec groups and execution remain separate.

The zero-Rust category architecture is independently `ACCEPT`. It reuses the
existing dense depset/artifact utilities and one provider/action owner; no V1,
Buck2, or Zabel representation is extracted. Four bounded successors cover
typed provider core, runfiles values/DefaultInfo, support actions, and Spawn
expansion without future schema replacement.

The typed provider-core successor is terminally `ACCEPT`. It uses existing
Rust-native Artifact, dense depset, compact map, Arc, and publication-equality
owners; no V1, Buck2, or Zabel representation is extracted. The terminal
schema correction reserves the final typed RetainedRunfiles and
RunfilesSupport fields now, so the runfiles-value successor adds behavior
rather than replacing the provider or support layout.

### Stage 6 recursive configured custom-rule analysis

Status: Partially landed
Source ref/commit(s): Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_analysis/src/analysis/{calculation.rs,env.rs}` and
`app/buck2_build_api/src/analysis/registry.rs`; V1
`slug-v1-archive:app/slug_analysis/src/analysis/calculation.rs` at
`e218054d4c796655939b968d90208b185decb352` inspected as reference only
V2 commit(s): oracle `9e6a4450`; implementation `4f4599e0`
Source class: selectively translated Buck2 recursive DICE calculation,
prepared dependency environment, target-local action-registry ownership, and
compact utility shapes behind V2-owned Bazel identities; V1 freeze/evaluation
lessons only
Reusable primitive or lesson: one semantic configured-target key recursively
computes unique dependencies in parallel, restores declared order before
evaluation, and returns owned provider/action facts; structural provider
identity must survive Starlark export and freeze
V2 wrapper/boundary: `ConfiguredTargetAnalysisKey` consumes `PackageLoadKey`
and root-repository `attr.label_list` dependencies in the retained workspace
transaction. `ProviderId` is `.bzl` source label plus exported name;
`AnalysisResult` owns ordered direct dependency keys, returned providers,
`DefaultInfo.files`, target-local actions, and diagnostics.
Bazel oracle: Bazel 9.2.0 at immutable commit
`8220c6198837d5c13d53fea211cf3282aa12408a`
V2 fixture: `recursive-custom-rule-providers-actions`
Expected evidence artifact: generated cquery/aquery oracle at `9e6a4450`;
focused Slug structural and exact activation regressions at `4f4599e0`
Implementation summary: removed the parallel digest-only analysis identity and
direct production helper; retained rule schemas and invocation dependency
values; added frozen string-field provider constructors and owned decoding;
made returned providers/`DefaultInfo.files` authoritative; kept actions
target-local; and wired recursive analysis through the daemon's retained DICE
transaction. Graph values reuse `CompactString`, `SmallMap`, `SmallSet`,
immutable `Arc` slices, `Dupe`, and `Allocative`.
Validation: seven-crate serial Cargo suite; exact per-key ActivationTracker
multisets for initial/identical/unrelated/edit/delete/recreate revisions;
focused external-label and structural-provider lookup tests; format/diff and
ownership greps; Sol-low post-review `ACCEPT`
Decision: adopt Buck2's computation/environment/registry ownership and compact
utility patterns, but retain V2 Bazel labels, configuration keys, provider
semantics, output paths, and oracle as authoritative. Do not import Buck cells,
V1 global registries, digest scaffolds, command-owned graphs, or action
aggregation.
Residual risk: root repository and one exact label-list/string-provider subset
only; repository mapping, transitions, general attrs/providers, query
consumers, execution, and materialization remain open. The migration observer
still scans before injecting immutable inputs.

2026-08-09 single-owner follow-up: the parallel legacy analysis key was removed
and the existing Need-aware recursive key renamed `ConfiguredNodeAnalysisKey`.
No representation or utility import changed: the retained owner still uses
structural `ConfiguredTargetKey`, immutable `Arc` results/slices,
`CompactString`, `SmallMap`/`SmallSet`, `Dupe`, and `Allocative`. Full analysis,
server, focused downstream lifecycle, archive, formatting, and independent
review accepted the migration; the two known unrelated full-suite failures are
unchanged. Root-setting request mode remains the named M2b residual before
structural/null configured-node identity.

2026-08-09 M2b follow-up: root-setting requests were removed from the DICE key.
One analysis-owned preparation path now resolves command roots and recursive
children to structural configurations, using the existing loading keys and
compact configured-target/result storage. No V1/Buck2 code or representation
was newly imported; structural/null node and classified-edge/result substrate
remain the next Stage 6 packet.

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

### Stage 7 NativeLink oracle harness integration

Status: Pending reviewer decision; current worktree only
Source inspected: `slug-v1-archive:tests/plan34/test_reapi_local_executor_smoke.py` (NativeLink config shape, binary discovery, startup readiness, teardown); `slug-v1-archive:.github/actions/setup_plan34_nativelink/action.yml` (binary build path)
V2 wrapper/boundary: `tools/v2_oracle_lib/nativelink.py` owns NativeLink lifecycle for the V2 oracle harness; `tools/v2_oracle_lib/runner.py` starts/stops it and injects `--remote_executor`; `tools/v2_oracle_lib/fixture.py` parses the `[reapi]` section; `tools/v2_oracle_lib/compare.py` validates REAPI evidence
Reusable primitive or lesson: retain V1's local CAS/AC/scheduler/worker config shape and readiness signal ("Worker registered with scheduler"); reject V1 Buck executor settings, `buck-out` paths, `--isolation-dir`, and Plan 34 evidence schema
Bazel oracle: Bazel 9.1.1 checked-in `simple-rule-action` expected manifest with declared output digest `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49`
V2 fixture: `simple-rule-action` with `[reapi] remote_executor = true`
Expected evidence artifact: Stage 1 oracle `expected/oracle.json` (generated, unchanged); slug run evidence captured in run JSON `reapi_evidence` field
Implementation summary: Rewrote the NativeLink lifecycle for the V2 oracle boundary. Binary discovery checks `SLUG_V2_NATIVELINK_BIN` then sibling `../nativelink/target/{release,smol,debug}/nativelink`. The runner starts NativeLink with a local filesystem CAS/AC store, simple scheduler, and one local worker; polls for port readiness plus the worker-registered log line; injects `--remote_executor=<endpoint>` and declared `default_exec_properties` into the slug argv; extracts the REAPI evidence JSON from stderr; and tears down NativeLink in a `finally` block. The build command emits valid JSON with `action_digests`, `uploaded_digests`, and `materialized_outputs` lists. Materialized outputs use mode `0o555` to match Bazel. No V1 Buck executor settings, `buck-out` paths, or Plan 34 evidence schema were imported.
Validation: `python3 -B -m tools.v2_oracle run --fixture simple-rule-action --tool slug --slug <slug-v2-bin> --timeout 60` reported `status: ok` with `reapi_actions=1`, `direct_local_actions=0`, materialized output digest matching the Bazel oracle; `CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2 -p slug_reapi_v2 --no-fail-fast` passed 20 tests (1 ignored); `python3.12 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py` passed 17 tests; `cargo fmt --check`; forbidden-surface grep unchanged
Decision: retain V1's NativeLink config shape and readiness signal only. Do not import V1 Plan 34 evidence schema, Buck executor settings, or `buck-out` paths.
Residual risk: `shell-action-reapi` Bazel oracle is still a placeholder; headers/TLS/retries, output-directory/tree handling, durable AC replay, generated-output reupload, and same-daemon invalidation remain open

### Stage 7 run_shell shell-argv pad parity

Status: Pending reviewer decision; current worktree only
Source inspected: Bazel `src/main/java/com/google/devtools/build/lib/analysis/actions/ShellCommand.java:43-48` (`pad` branch: `ImmutableList.of(shExecutable, "-c", command, "")`); `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkActionFactory.java:627-631` (`boolean pad = !arguments.isEmpty()`); starlark-rust 0.13 `starlark-rust/starlark/src/values/layout/value.rs:968-979` (`get_attr` method-lookup shadows `get_attr` trait method)
V2 wrapper/boundary: `slug_build_api_v2::CtxActions::run_shell` (argv construction with pad); `slug_analysis_v2::starlark_rule` (`DeclaredFile` `get_attr`/`has_attr` for the `path` property; `run_shell` Starlark binding)
Reusable primitive or lesson: Bazel's `ShellCommand` pads an empty `$0` before user arguments so the first argument is `$1` (`sh -c 'script' '' arg1`); V2 must match this exactly or `$1`-style scripts fail. starlark-rust 0.13 resolves `obj.attr` via method table first, so a `path` property must be `get_attr`, not a `#[starlark_module]` method named `path`.
Bazel oracle: Bazel 9.2.0 `shell-action-reapi` expected manifest with declared output digest `ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28` (content "reapi", 5 bytes), generated with `--remote_executor` against NativeLink 1.4.0
V2 fixture: `shell-action-reapi` with `[reapi] remote_executor = true`, exercising `ctx.actions.run_shell(outputs=[out], command="printf reapi > $1", arguments=[out.path])`
Expected evidence artifact: Stage 1 oracle `expected/oracle.json` (generated); slug run `reapi_evidence` with `reapi_actions=1`, `direct_local_actions=0`
Implementation summary: Added `ctx.actions.run_shell` Starlark binding and `DeclaredFile.path` property (`get_attr`/`has_attr`, not a method). Fixed the argv pad bug in `CtxActions::run_shell`: when `args` is non-empty, an empty `$0` is inserted (`argv = [sh, -c, command, "", args...]`), matching Bazel's `ShellCommand` pad branch. No V1 code imported; the fix is a direct port of the cited Bazel source behavior.
Validation: `python3 -B -m tools.v2_oracle run --fixture shell-action-reapi --tool slug --slug <slug-v2-bin> --timeout 60` reported `status: ok` with `reapi_actions=1`, `direct_local_actions=0`, materialized output digest matching the Bazel oracle; `CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2 -p slug_reapi_v2 -p slug_analysis_v2 -p slug_build_api_v2 --no-fail-fast` passed 49 tests (1 ignored); `python3.12 -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py` passed 17 tests; `cargo fmt --check`; 2 new focused tests (`run_shell_pads_empty_dollar_zero_when_arguments_are_present`, `run_shell_omits_pad_when_no_arguments`)
Decision: adopt Bazel's pad behavior verbatim. No V1 candidate existed for this; the V1 `run_shell` (if any) was not inspected because the Bazel source citation is the parity oracle.
Residual risk: `output_files` paths with nested directories rely on the NativeLink worker creating parent dirs (confirmed for 1.4.0); output-directory/tree handling, durable AC replay, and same-daemon invalidation remain open

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

### Stage 8 loading-query thin vertical — approved extraction plan

Status: Thin vertical landed; full loading query remains open
Source ref/commit(s): Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_query_parser`,
`app/buck2_query/src/query/{environment.rs,graph.rs,traversal.rs,syntax/simple}`,
and `app/buck2_query_impls/src/uquery`; V1
`e218054d4c796655939b968d90208b185decb352`
`app/{slug_query_parser,slug_query,slug_query_impls,slug_cmd_query_server}` and
`tests/core/query/test_bazel_compat_query.py`
V2 commit(s): oracle `7e8993b2`; implementation `61ca25db`
Source class: port Buck2's Bazel-neutral parser spans, generic evaluator,
traversal, and compact deterministic-set machinery; adapt environment/DICE
separation; use V1 only as same-lineage reference and scenario inventory
Reusable primitive or lesson: parse generic calls before registry validation;
keep one generic evaluator/traversal substrate with command-specific
environments; compute literal/package inputs through the retained DICE
transaction; keep structural compact nodes and sets
V2 wrapper/boundary: V2 owns Bazel labels/patterns, the complete Bazel 9.2
loading-query registry, diagnostics, demand-driven
`UnconfiguredPackageGraphKey`, recursive-only `RootPackageSetKey`, structural
rule/source nodes, normalized alias/filegroup/custom-rule edges, ordering, and
text rendering. The existing daemon wire boundary may gain only tagged
Build/Query requests and a common response so query uses the retained
`WorkspaceRuntime`; it must not become a general command bus.
Bazel oracle: generated with Bazel 9.2.0 at
`8220c6198837d5c13d53fea211cf3282aa12408a`
V2 fixture: `query-parser-and-sets` and `query-loading-thin-vertical`; stale
Bazel 9.1.1 Windows `query-basic` is superseded for this text-query slice and
is not acceptance evidence
Expected evidence artifact: `7e8993b2` generated CLI results/errors for the
implemented expression, graph, target-pattern, and ordering matrix;
source-cited Rust unit tests remain required for AST/span facts Bazel CLI
cannot expose
Decision: port parser/spans/generic evaluator/traversal/compact sets; adapt
uquery environment separation; reject Buck cells, labels, patterns,
registries, attributes, diagnostics, printers, configured/action graphs, and
V1 process/server context. Keep known-but-deferred functions and formats as
explicit errors, not parity claims.
Implementation summary: Ported Buck2's borrowed-span `nom` parser,
non-recursive binary sequence, generic callable registry with typed optional
arguments, compact target sets, and depth-limited traversal. Adapted the
environment to V2 canonical labels and demand-driven DICE package/directory
keys. V1 remained same-lineage reference material; no V1 server/process
context, cells, labels, configured nodes, action nodes, diagnostics, or
printers were imported. Added only tagged Build/Query daemon requests so raw
loading queries execute in the retained `WorkspaceRuntime`.
Validation: both fixtures generated and independently reran no-update with
`/usr/bin/bazel` 9.2.0; discovery/provenance/generated/assertion/whitespace and
candidate credential checks passed. Both fixtures then passed through the
rebuilt Slug V2 CLI. The serial six-crate suite passed 67 tests; exact DICE
events cover identical/unrelated/affected revisions and recursive package
create/delete/recreate; build protocol and same-daemon query regressions pass.
Root reran the suite and Slug oracles after final fixes; Sol-low final review
`ACCEPT`.
Residual risk: this is the first integrated M3 vertical, not full query.
External repositories, the remaining function registry, Sky Query,
configuration/action environments, and non-text formatters remain open.

### Stage 8 reverse-deps/subtree-pattern packet — approved extraction plan

Status: Packet landed; full loading query remains open

Source ref/commit(s): Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_query/src/query/graph/graph.rs`,
`query/environment.rs`, and
`query/syntax/simple/functions/deps.rs`; V1
`e218054d4c796655939b968d90208b185decb352`
`tests/core/query/test_bazel_compat_query.py`

Source class: directly port Buck2's stable graph construction, reversal,
depth-bound traversal, and generic `rdeps` invocation; use V1 only as scenario
inventory

Reusable primitive or lesson: derive reverse traversal request-locally from
the forward universe closure; keep semantic graph ownership in demand-driven
DICE package keys; specialize package-local direct reverse lookup without
enumerating the workspace

V2 wrapper/boundary: V2 owns Bazel root-repository subtree patterns,
`SubtreePackageSetKey { workspace, prefix }`, universe-closure semantics,
same-package edge qualification, Bazel diagnostics, depth/order rendering, and
the retained mutable-DICE serial lookup adaptation. Root `//...` is the
empty-prefix specialization. No persistent reverse graph, Buck labels/cells,
external repositories, configured/action nodes, printers, or `siblings`
surface is imported.

Bazel oracle: commit `5b7806d7`, generated and independently verified with
`/usr/bin/bazel` 9.2.0 at Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`

V2 fixture: `query-rdeps-and-subtree-patterns`

V2 commit(s): oracle `5b7806d7`; implementation `cdc5af41`

Expected evidence artifact: `5b7806d7` exact root-subtree expansion/failure,
`rdeps` universe/depth/cycle/seed/order behavior, package-local/criss-cross
direct reverse-dependency behavior, and arity/expression-operand diagnostics

Decision: port the named Buck2 graph/reverse traversal machinery; adapt its
lookup to the existing retained DICE transaction and compact V2 labels; reject
V1 implementation/output and all Buck/Bazel semantics outside the reviewed
Stage 8 packet

Implementation summary: Ported Buck2's request-local integer graph, stable DFS
remap, reversal, bounded retention, postorder, universe filtering, and generic
invoke around V2 structural labels and serial mutable-DICE lookup. Added
prefix-local `SubtreePackageSetKey` and operand-package-local direct reverse
lookup. No V1 graph/server/output implementation, persistent reverse cache,
whole-workspace subtree filter, Buck cell/label semantics, or `siblings`
surface was imported.

Validation: generation plus worker and root independent Bazel no-update reruns
passed; fixture provenance/generated/assertion/whitespace and credential checks
passed. The serial six-crate implementation suite passed 71 tests; the rebuilt
V2 CLI passed the complete 26-command fixture and both preceding query
fixtures. Exact DICE events cover prefix-local outside/inside
create/delete/recreate, universe edge closure loss/regain, and operand-local
same-package reverse lookup. Ownership/reuse, formatting/diff, daemon, and
scope checks passed. Sol-low oracle, early reuse, and final implementation
reviews returned `ACCEPT`.

Residual risk: external repositories, Sky Query, the other 13 loading
functions, non-text formatters, remaining ordering modes, `cquery`, and
`aquery` remain open.

### Stage 8 path-topology packet — approved extraction plan

Status: Packet landed; full loading query remains open

Source ref/commit(s): Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`app/buck2_query/src/query/graph/async_bfs.rs`,
`query/environment.rs`, and
`query/syntax/simple/functions/deps.rs`; V1
`e218054d4c796655939b968d90208b185decb352`
`tests/core/query/test_bazel_compat_query.py`

Source class: reuse the landed Buck2-derived unbounded reverse traversal
directly for `allpaths`; directly port Buck2's compact BFS parent-map path
reconstruction into the landed `ResolvedGraph`; use V1 only as scenario
inventory

Reusable primitive or lesson: one request-local forward closure supports both
all-path reverse projection and one shortest-path reconstruction without a
second semantic graph or dependency walk

V2 wrapper/boundary: V2 owns Bazel argument evaluation, root-repository
structural labels, arbitrary multi-root/endpoint choice, exact
default/auto/full rendering where stable, bounded diamond alternatives,
diagnostics, serial mutable-DICE lookup, and only the
`QueryCommand`/`QueryExpression` top-level-`somepath` AUTO-sorting exception in
`evaluate_loading_query`. Nested set/binary/`let` expressions keep ordinary
AUTO sorting. No generated/output nodes,
persistent cache, new DICE key, Buck label/cell/general environment,
configured/action state, filter, or printer is imported.

Bazel oracle: must be generated and independently verified with
`/usr/bin/bazel` 9.2.0 at commit
`8220c6198837d5c13d53fea211cf3282aa12408a`; output-policy anchors are
`runtime/commands/QueryCommand.java:112-118` and
`query2/engine/QueryExpression.java:110-114`

V2 fixture: `query-path-topology`

V2 commit(s): oracle `2b73c08d`; implementation `7d851ce9`

Expected evidence artifact: exact all-path sets; unique shortest paths; bounded
complete diamond/multi-pair alternatives; zero/no-path, cycle, source
direction, multiple/duplicate/empty operands; stable order modes; arity and
integer-literal diagnostics; direct top-level forward ordering and nested
set-operation lexical AUTO ordering

Decision: call the landed unbounded reverse-dependency helper for `allpaths`;
port only Buck2's integer BFS/parent reconstruction for `somepath`; adapt both
to the existing compact V2 graph and retained DICE transaction. Add the
top-level AST ordering exception only in `evaluate_loading_query`; reject
function-local, graph-local, CLI/protocol, or broader sorting policy. Reject
V1 implementation/output and all semantics outside the reviewed Stage 8
packet.

Validation: oracle generation plus two independent sequential no-update Bazel
runs passed all 43 commands and anchored patterns. Worker and root independent
implementation validation each passed the serial 76-test six-crate suite,
rebuilt the V2 CLI, and passed all four query fixtures. Exact activation
multisets, retained-daemon transitions, ownership/reuse, formatting/diff,
scope, and daemon-cleanup checks passed. Sol-low returned `ACCEPT` at the
architecture, oracle, early implementation, and final evidence gates.

Residual risk: Bazel's arbitrary diamond/multi-pair choice must be expressible
as bounded complete alternatives. Generated/output-file reverse edges, the
other 11 loading functions after this packet, repositories, patterns, order
modes, non-text formatters, `cquery`, and `aquery` remain open.

### Stage 8 arbitrary-selection packet — approved extraction plan

Status: Landed and validated

Source ref/commit(s): Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`
`SomeFunction.java`, ordinary/Sky `EvaluateExpression` implementations,
`QueryEnvironmentFactory.java`, `QueryParser.java`, and `Lexer.java`; Buck2
`088c75c7e36805df99c3de29062baa95db700b8b`
`query/syntax/simple/eval/set.rs`; V1
`e218054d4c796655939b968d90208b185decb352`

Source class: implement Bazel ordinary-query `some` semantics over V2's landed
compact ordered target set; reuse only Buck2's ordered compact-set lesson;
reject V1 and Sky Query semantics

Reusable primitive or lesson: arbitrary bounded selection needs no semantic
graph beyond the already evaluated unique `SmallSet`; ordinary Blaze query
does not cancel remaining operand evaluation

V2 wrapper/boundary: V2 owns signed Java-`int` argument conversion,
expression-position integer preservation, arbitrary-result alternatives,
empty-selection diagnostics, AUTO/FULL rendering, root-repository DICE
demand, and retained-daemon transitions. No streaming/cancellation surface,
graph, target representation, DICE key, cache, runtime, protocol, filesystem,
or order exception is added.

Bazel oracle: must be generated and independently verified with
`/usr/bin/bazel` 9.2.0 at commit
`8220c6198837d5c13d53fea211cf3282aa12408a`; cancellation and signed-integer
source anchors are the files named above

V2 fixture: `query-some-selection`

V2 commit(s): oracle `e8e1d9ef`; implementation `b25c8aff`

Expected evidence artifact: singleton and arbitrary bounded selections;
omitted/zero/negative/equal/excess counts; duplicate/nested/empty/cycle/
recursive operands; normal-query later-error precedence; signed `i32`
boundaries; retained expression-position integers; `deps`/`rdeps` shared
signed-depth behavior; arity/type diagnostics; AUTO/FULL ordering; and
candidate create/rename/delete/recreate transitions

Decision: activate only `some`; select from the existing unique insertion
order; add the minimum typed signed-integer seam shared with `deps`/`rdeps`;
keep generic expression integers and all graph/DICE/protocol ownership
unchanged. Selection order is not FULL output order: the shared selected-graph
deterministic topological renderer follows Bazel
`AbstractUnorderedFormatter`/`Digraph` and, as of `d19a9b29`, is backed by
request-local recorded evaluation edges. The failed `equal_count_full` gate
exposed the topological rather than insertion-order boundary. The UTF-8-safe
three-token bare-negative diagnostic is part of that landed parser boundary.
Defer `filter` until an exact Java `Pattern` substrate exists.

Validation: final Bazel generation plus worker/root independent sequential
no-update reruns passed all 42 commands and anchored patterns. Normal-query
later-error probes emitted empty stdout; provenance, generated metadata,
diff/whitespace, and fixture-only credential checks passed; Sol-low returned
`ACCEPT`. Implementation `b25c8aff` then passed worker and root independent
serial six-crate suites (82/82) and all five Slug fixtures (133/133 rows):
worker run ends `030821/030825/030829/030833/030837`; root parser/loading/rdeps/
path/some runs are `031045-559795`, `-559816`, `-559841`, `-559894`, and
`-559794`. Exact activation/retained transitions, signed `i32` depth behavior,
scope/reuse, formatting/diff, and daemon cleanup passed. No key, cache,
protocol, filesystem, or lock was added; Buck2 `SmallMap`/`SmallSet` and `u32`
indices remain the hot-path representation.

Residual risk: the accepted ordinary-query stop probes show no masked later
failure or partial stdout, and signed-depth behavior is now covered. Sky
Query, Java regex, BUILD pseudo-nodes, generated nodes,
metadata, attrs, loads, visibility, tests, executables, external repositories,
the other ten loading functions, non-text formatters, `cquery`, and `aquery`
remain open.

### Stage 8 siblings BUILD-file-node packet — landed extraction

Status: Landed

Source ref/commit(s): Bazel `8220c6198837d5c13d53fea211cf3282aa12408a`,
`src/main/java/com/google/devtools/build/lib/query2/engine/SiblingsFunction.java`,
`src/main/java/com/google/devtools/build/lib/query2/engine/QueryEnvironment.java`,
`src/main/java/com/google/devtools/build/lib/pkgcache/PackageProvider.java`
lines 147-153, `src/main/java/com/google/devtools/build/lib/packages/Package.java`
lines 858-862, 1036, and 1462-1474,
`src/test/java/com/google/devtools/build/lib/packages/PackageFactoryTest.java`
line 943, and the named `AbstractQueryTest` sibling and exported-BUILD themes;
Buck2
`088c75c7e36805df99c3de29062baa95db700b8b` compact ordered collections; V1
`e218054d4c796655939b968d90208b185decb352` rejected for no siblings support.

V2 fixture: `query-siblings-build-file-node`

V2 commit(s): fixture base `8c28877b`; attribute correction `20f88c05`;
FULL-provenance oracle `1a3dec16`; implementation `d19a9b29`

Decision implemented: port no Buck2/V1 function semantics. The V2-owned
`BuildFile` package-graph node uses its actual loaded basename and zero edges;
generic `siblings` projects packages with compact package
deduplication. Do not normalize `BUILD` to `BUILD.bazel`; an absent basename
is the normal target-missing path. Coalesce a matching `ExportedFile` for the
active BUILD basename into the one `BuildFile` node; any rule/alias/custom
collision remains an invariant error. Keep it non-rule and zero-edge so it
does not alter `:all`, recursive, or traversal behavior. Defer transitive
loads/fake `.bzl` nodes, regex/kind, attributes/labels, visibility, tests,
executables, generated/external/configured/action state.

Expected evidence: exact actual/wrong BUILD labels including root and matching
`exports_files` coalescing; complete same/multiple-package sibling sets;
rule/source/alias/custom/BUILD operands; implemented compositions and FULL
ordering; empty/error behavior; and exact retained-DICE lifecycle,
BUILD-content, basename-priority/rename, and package delete/recreate
transitions.

Validation: the attribute-corrected update/no-update/root Bazel runs
`034446-589899`, `034516-592708`, and `034623-595736` passed. The
43-row FULL-provenance discovery/anchored-update/no-update/root runs
`035638-609525`, `035734-612675`, `035759-615627`, and `035853-619234` passed
and prove direct and graphless-wrapped `siblings` retain the same FULL order,
while `siblings(deps(...))` retains evaluation provenance. Rebuilt Slug passed
91/91 and the six-fixture 176/176 gate at worker runs `040407-626548`,
`040411-626572`, `040414-626601`, `040418-626692`, `040423-626782`,
`040427-626870` and root runs `040534-628098`, `040540-628123`,
`040546-628189`, `040549-628247`, `040554-628339`, `040558-628428`. The
implementation uses the exact BuildFile basename/coalescing/zero-edge/nonrule
representation, evaluates siblings once with package deduplication, and keeps
the `u32`/`Vec`/`SmallMap` evaluation graph request-local; FULL has no
render-time DICE read. No key/cache/protocol/filesystem/lock/global boundary
entered the packet. External RC could be consumed only by Bazel invocation,
and no credentials were accessed.

Residual risk: `buildfiles`/`loadfiles` require a separate transitive loading
representation; do not treat their source tests or `kind` interactions as
accepted Slug semantics in this packet.

### Stage 4/8 load-provenance and fake-target packet — reviewed extraction plan

Status: Gate A and Gate B are accepted. Gate A, B1 query-core activation, and
B1.5 landed in `791e26b2`, `ba457999`, and `d25bc8c0`; diagnostics and cycle
recovery landed in `4428df22` and `237e7cac`. B2 landed in `cb514747`,
accepting its seven graph rows and the complete 64-row fixture.

Parent: `WP-4-8-m3-build-load-files`. Gate A is a V2-owned
`load-provenance-fake-target-substrate`; Gate B activates only `buildfiles`
and `loadfiles` after A acceptance. One combined Bazel 9.2 oracle fixture is
required before either gate. B now leaves seven deferred ordinary functions;
exact Java regex and missing target metadata keep the
others deferred.

| Candidate | Source / mode | Decision |
| --- | --- | --- |
| Bazel transitive loading | Bazel `8220c6198837d5c13d53fea211cf3282aa12408a:src/main/java/com/google/devtools/build/lib/query2/{engine/BuildFilesFunction.java,engine/LoadFilesFunction.java,common/AbstractBlazeQueryEnvironment.java,compat/FakeLoadTarget.java,query/BlazeQueryEnvironment.java,query/BlazeTargetAccessor.java}`; specifically `transitiveLoadFiles`, `getTransitiveLoadFilesHelper`, `getPackage`, and `TargetKeyExtractor` | Semantic authority. Port no Java implementation; fixture must prove full transitive load behavior, fake printed-label versus consuming-package behavior, label-keyed uniqueness across compositions, broken companion basename, and failure cycles. |
| V1 traversal | `slug-v1-archive:app/slug_query_impls/src/uquery/environment.rs` (`allbuildfiles`, `get_transitive_loads`) | Reference-only extraction candidate. Its DICE traversal/lifetime lesson may inform a V2 adapter, but reject Buck paths, cells, identities, query filesets, and direct import. |
| Buck2 query utilities | `../buck2/app/buck2_query/src/query/{environment.rs,graph.rs,traversal.rs}` and `syntax/simple/` | Selectively reuse compact request-local graph/collection and environment-separation patterns only. Reject Buck function names, labels, cells, and file semantics. |
| Existing V2 loading | `app/slug_loading_v2/src/{keys.rs,bzl_module.rs,load_label.rs,package.rs}` | Adopt. Build on `BzlParseKey`, `BzlModuleEvalKey`, load-label resolution, `PackageLoadKey`, package listings, and injected workspace observations. |

Required V2 boundary: immutable compact provenance manifests contain canonical
root label/path, direct children, and transitive fingerprint in `Arc` slices;
`LoadedPackage` exposes its BUILD roots/reachable closure and retains
`FrozenModule` lifetimes separately. `LoadedPackage` semantic equality includes
the direct roots and transitive manifest identity/fingerprint, not frozen
pointer identity. Request-local fake-node state preserves `(printed label,
consuming package, real/fake)` provenance until the oracle establishes the
winner through function/set compositions; do not assume request-global
first-owner behavior. This is not a global identity change: fake `.bzl` and
companion BUILD targets never belong in package graphs, `:all`, recursive
patterns, or dependency edges, while `deps(fake)` returns only the
function-produced target. Companion BUILD basename discovery is DICE-tracked
but parse-independent and does not require the companion package to load. FULL
preserves only real operand-evaluation edges and adds no synthetic fake-load
edges.

Stop/replan instead of importing or widening on external repository mapping,
silent `.scl` loss, direct filesystem scans, whole-workspace discovery,
unreviewed DICE keys, global query-label rewrites, dropped frozen-module
lifetimes, or a `.bzl` cycle represented as success. Any new DICE key needs
Sol approval before implementation.

Validation must prove manifest equality invalidates the owning package/query
for direct-root, transitive-edge, and leaf-content changes while excluding
retained `FrozenModule` pointer identity from equality.

#### Oracle evidence landed (2026-07-23)

`8f6f02b3` (`test: add build and load files oracles`) established 58 Bazel 9.2
records; `e8014b25` (`test: isolate fake target set algebra`) corrects
`query-build-load-files-provenance` to 64 with a singleton fake-target
topology. Update `051423-694832`, Terra clean `051521-700085`, and root clean
`051644-705470` passed; Sol-low final review was `ACCEPT`. The source anchors
are `BuildFilesFunction`, `LoadFilesFunction`,
`AbstractBlazeQueryEnvironment#transitiveLoadFiles`, `FakeLoadTarget`,
`BlazeQueryEnvironment#getTransitiveLoadFilesHelper`,
`BlazeTargetAccessor#getPackage`, `TargetKeyExtractor`,
`BinaryOperatorExpression#evalPlus/#evalMinus/#evalIntersect`, `QueryUtil`'s
label-key set, and `SiblingsFunction`.

No V1/Buck semantics were imported. The observed V2 boundary is stricter:
request-local state retains `(printed label, consuming package, real/fake)`
through set composition. `seenBzlLabels` deduplicates per invocation, but
separate invocations can project the same printed label to different consumers;
do not replace that with global `QueryLabel` identity or a request-global
winner. Intersection retains the left representative, equal-label `except`
removes in both directions, and union sends distinct callback batches to
`siblings`; the older fake-left survivor is unmatched transitive `two.bzl`, not
an asymmetric real/fake operation. Factored FULL
(`--output=graph --graph:factored`) confirms zero fake edges and forbids
synthetic projection edges. Gate A and the B1 query core now implement this
boundary. Exactly `buildfiles` and `loadfiles` are active, seven ordinary
functions remain deferred, and Gate B acceptance now awaits only B2 graph
output.

#### Stage 4 Gate A half landed (2026-07-23)

Commit `b0670e33` is a V2-owned implementation, not a V1/Buck import. It
adds compact public `BzlLoadManifest`/`BzlModuleIdentity` and aligned
`FrozenBzlLifetimeEntry`: canonical label/path, source-order label-first direct
IDs, first-seen transitive closure, `[u8; 32]` SHA-256, semantic package
equality, and separately retained opaque frozen modules. It also reuses the
existing `WorkspaceDirectoryKey` for parse-independent primary/fallback
companion discovery, including symlinks and explicit missing/read-error paths.

Stage 4 plus Stage 8 are accepted as Gate A: `791e26b2` supplies the
fake-target algebra. There is still no function activation; Gate B and nine
ordinary functions remain deferred. Root validation
passed 27 loading, 11 analysis, and 22 query integrations; Sol-low `ACCEPT`
followed corrections for shared validation, alignment truncation, direct/
transitive edge lifecycle plus BUILD non-over-invalidation, and memory
accounting.

#### Stage 8 Gate A fake-target provenance algebra landed (2026-07-23)

Commit `791e26b2` is V2-owned, crate-private query substrate rather than a V1
or Buck semantic import: it adds `app/slug_query_v2/src/provenance.rs` and one
module declaration. A checked-`u32` `Vec`/`SmallMap` arena avoids an `Arc` per
candidate while retaining full symmetric real/fake identity. A callback delivery
is one nonempty `Arc`-ID batch with label-first representation; union preserves
batches, `eval_all`/intersection/`except` materialize labels, intersection
retains the LHS representative, and equal-label `except` is symmetric.
`siblings` scans every batch for consuming-package ownership and delayed output
deduplicates labels. Fake `evaluation_graph_label` is `None`, while fake labels
remain printable and zero-edge for future activation.

At the Gate A checkpoint no V1/Buck evaluator, graph, registry, DICE, or
function surface was imported or activated: this module was deliberately
disconnected. Worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2` with 32 tests (10 provenance,
16 loading-query, 6 parser/registry); Sol-low final review was `ACCEPT` with no
rework.

#### Stage 8 Gate B B1 query-core activation landed (2026-07-23)

Commit `ba457999` is a V2-owned integration of the Gate A algebra, not an
import of V1/Buck query semantics. A crate-private associated `E::Set` makes
the generic evaluator preserve request-local candidate-ID batches end to end;
the loading environment owns the arena and binds only `buildfiles` and
`loadfiles`. The implementation removed unused public evaluator reexports and
added no DICE key, global label identity, filesystem/protocol boundary, other
crate, or additional ordinary function.

The Bazel-derived split is explicit: `seenPackages` keys on the printed
candidate package, while `PackageLoad` and transitive load visitation use the
candidate owner. `.bzl` uniqueness and output uniqueness remain separate;
companion discovery uses the absolute package path through the existing
DICE-only helper. Fake candidates have zero dependencies, `siblings` scans all
preserved deliveries, and FULL rendering takes the first printed-label
representative before retaining only recorded real edges.

Worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2` with 34 tests (10 unit,
18 loading, 6 registry/parser). Root also passed the serial command/server/CLI
suite: 11, 12, and 14 tests respectively, with zero doc tests. Sol-low final
review returned `ACCEPT`; the dedicated `eval_set_arg` seam and the
printed-package/owner/separate-set/absolute-companion corrections were made
live before final review, with no post-final-review rework. Root also removed
one transient candidate-package `String` allocation before the final tests.

#### B1.5 diagnostic and cycle extraction landed (2026-07-23)

Commit `4428df22` ports Bazel-observed diagnostic shape, not Java loading
machinery: missing loads name the canonical load label and malformed `.bzl`
files retain the parse error plus Bazel's module-compilation summary.

Commit `237e7cac` selectively adapts Buck2's
`buck2_util::cycle_detector::LazyCycleDetector` pattern. The result is a
request-scoped DICE user detector, installed per loading transaction, that
tracks only `BzlModuleEvalKey` start/finish/edge events in compact
`SmallMap`/`SmallSet` state. Its typed `BzlLoadCycle` separates the acyclic
path from the cycle so the Bazel BUILD-origin, multi-node, and self-edge
diagrams can be rendered exactly. A deliberately invalid poison dependency
keeps the detected cycle from surviving an input repair.

This is a utility-pattern extraction, not Buck identity, cell, label, or file
semantics. Focused tests prove bounded release of recursive DICE waits,
two-node/self-edge diagrams, preservation of the path into a cycle, a
non-cycle diamond, and recovery after repair in the same DICE instance.
Sol-low caught the blocking loss of the path-to-cycle segment; Terra-high
corrected the typed result and final review returned `ACCEPT`.

#### B1.5 downstream evidence landed (2026-07-23)

Commit `d25bc8c0` accepts the exact 57-row non-graph CLI set, including raw
success output and failure exit/stdout/stderr behavior. The full CLI suite
passed 14 integration plus 1 unit test. Retained-daemon regressions cover leaf
edits, direct/transitive edge switch-delete-recreate, companion BUILD priority,
and `buildfiles` versus unaffected `loadfiles` invalidation with exact counts;
the server suite passed 14 tests. Sol-low returned final `ACCEPT`.

#### Stage 8 Gate B B2 graph presentation landed (2026-07-23)

Commit `cb514747` completes Gate B without importing V1 query evaluation,
graph identity, or serialization code. The V2 evaluator retains a compact
request-local structural selected graph in `QueryOutput`; the CLI and daemon
format it without reevaluation or DICE access. The implementation selectively
uses Buck2-derived checked-`u32`, compact-map/set, deterministic-graph, and
presentation-separation lessons while keeping Bazel 9's semantics
authoritative.

Both factored and unfactored modes are implemented. Factoring requires equal
predecessor and successor sets and deduplicates quotient edges. Ordering
matches Bazel's lexicographical member-label sequence comparator and reverse
DFS postorder; a regression proves that a factored `//a:a\\n//z:z` class is
not ordered by its joined DOT spelling against `//a:a0`. Serialization remains
Bazel's narrow always-quoted spelling, not a general DOT escaper.

All seven graph rows now pass exactly, so the shared 64-row fixture is
accepted. Root also passed four focused formatter tests, explicit unfactored
coverage, and the serialized four-crate suite: 12 command, 14 query unit, 18
loading-query, 6 parser/registry, 15 server, 14 existing CLI integration, 2
graph integration, and 1 CLI unit tests. Sol-low returned final `ACCEPT`.
Seven ordinary query functions remain deferred; B2 adds no new function,
DICE key, filesystem boundary, or global state.

### WP-4-8-m3-labels-metadata-foundation — reviewed extraction decision

Authoritative next packet; pending. Sol-low rejected Terra's initial `filter`
recommendation: Java `Pattern.compile`/`Matcher.find` has no exact reusable
path; finite oracle, fancy-regex, and Rust regex are not parity.

Port no V1 labels code: `slug-v1-archive:app/slug_query/src/query/syntax/simple/
functions.rs#labels` is unimplemented. Buck2
`buck2_node/src/attrs/{attr,attr_type,coerced_attr,traversal,spec}.rs` and
`app/buck2_query/src/query/syntax/simple/eval/set.rs` provide compact
map/set/shared-slice/string and traversal shapes only. Reject Buck
cells/labels, attribute kinds, provider labels, select/configured-dependency,
visibility/plugin, and regex/query semantics. Stage 4 owns ordered immutable
schema, structured coerced values/provenance/selectors, and canonical labels;
Stage 4 also owns the exact output/output-list generated-target representation
required by upstream `labels(outs, ...)`. Stage 8 owns only the separate
reachable-label projection and `labels` function. `LoadedPackage`,
`QueryNode`, and `UnconfiguredPackageGraph` equality include their semantic
structures, never frozen lifetimes.
Authority:
`LabelsFunction`, `BlazeTargetAccessor#getPrerequisites`,
`AggregatingAttributeMapper#getReachableLabels`, and
`AbstractQueryTest#testLabelsOperator` at `8220c619…`. Add formal ledger entry
after implementation with source/mode, oracle, validation, and residual risk.

`8dfae99c` is accepted immutable 31-row Bazel evidence: seven public default
label-bearing constructors, dormant exclusion, selector-key false, valid dedup,
distinct output generators, generated kind, and fail-fast errors. All 31 rows
now have Slug CLI evidence: the final two `label_kind` rows are activated by
`WP-8-m3-query-label-kind-output`. No extraction decision changes.

Gate A `1b7c179c` is accepted and V2-owned: ordered immutable `Allocative`
seven-label-kind-plus-String schemas/values, defaults/configurability,
provenance/select structure, canonical generated owner, outputs outside deps,
equality, same-DICE tracker, and preactivation guard. No V1/Buck semantics
entered. Root passed fmt/diff, loading 35/query 39/analysis 11; Sol corrected
six blockers then `ACCEPT`. Its later Stage 8 29-row function gate and two
generated-kind formatter rows are now accepted.

`f3e8ad48` is accepted V2-owned prerequisite evidence: sorted compact native
`config_setting` values, zero-edge rule kind, semantic reorder/change tests,
load-only behavior, and fail-closed unsupported attrs. No configuration
evaluation or V1/Buck semantics entered; Sol `ACCEPT`. Define/flag/constraint/
common attrs and matching remain deferred; Stage 8 remains 29+2.

`8fec2696` is accepted V2-owned labels-only activation: immutable Allocative
query attrs separate from deps, selectors exclude keys, generated files only
output→own-generator edges, and exact 29 function rows. The two formerly
formatter-deferred label-kind rows are now accepted separately through
`WP-8-m3-query-label-kind-output`. Sol `ACCEPT` after graph/order corrections;
no V1/Buck import.

### WP-4-8-m3-executables-rule-capability — landed Gate A/B extraction decision

Oracle `c8e469f5` and Stage 4 implementation `c86fc656` are landed and
Sol-accepted. Stage 8 activation `69565a29` is also landed and Sol-accepted.
Its 32 semantic rows and eight representation-only `label_kind` rows establish
the current-loadable rule-capability boundary without importing V1 behavior.
Stage 4 owns a V2 `RuleCapability {
rule_class: CompactString, executable: bool }`, exported-name capture through
starlark-rust `StarlarkValue::export_as`, and equality/invalidation. Stage 8
owns only `executables(EXPR)` projection/filtering. Use the bounded Buck2 rule
export/capture shape and existing V2 provider `OnceCell`/freeze pattern as
reference mechanisms; do not port Buck rule classification, cells, query
semantics, or `fancy-regex`. V1 query target capability hooks are
reference-only, not a Bazel source of truth. Bazel 9.2
`ExecutablesFunction`, `BlazeTargetAccessor`, and `TargetUtils` at
`8220c619…` are the oracle: per-target executable capability plus rule class
not ending `_test`. Pinned `StarlarkRuleClassFunctions#createRule`,
`getTestBaseRule`, and `StarlarkRuleFunction.export` establish exported class
identity and test-implies-executable even when `executable=False` is explicit.
Gate A used the bounded Buck2/provider export-and-freeze shape, V2-owned
`CompactString`/`Arc`/`Allocative`, static native capability values, and a
borrowed public projection; no V1 code or Buck query semantics were imported.
Native genrule behavior remains a separate oracle gate.

Gate B reused the current V2 generic evaluator/candidate algebra and compact
capability projection rather than importing V1 evaluation or Buck2 rule
classification. It filters retained delivery IDs in place, adds no edges or
DICE key, and passes all 32 semantic oracle rows plus same-DICE and
same-daemon lifecycle evidence. The eight representation-only `label_kind`
rows are now activated separately; five ordinary query functions and M3
remain open.

M4 reuses that same V2-owned compact capability without a new representation:
`AnalysisResult` retains the complete `Option<RuleCapability>` in derived
equality/Allocative state, and configured `executables` borrows it through the
shared evaluator. Its adjacent `DefaultInfo(executable)` decoder reuses
existing depset, runfiles, and files-to-run values rather than adding query
metadata. No V1 semantics, Buck query machinery, interner, or duplicate
retained rule-class value entered the configured activation.

### WP-8-m3-query-label-kind-output — landed formatter activation

V2 owns the implementation; no V1/Buck query formatter was imported.
Bazel 9.2 `LabelOutputFormatter`, `AbstractUnorderedFormatter`, and target-kind
methods are the source, while accepted oracles `c8e469f5` and `8dfae99c`
supply eight rule-class and two generated-file rows. A compact request-local
`SmallMap` and optional selected-node kind retain loaded identities.
`LabelKind` alone completes missing cross-package selected kinds through the
existing package-graph key; Standard wrappers preserve text/graph dependency
and ordering behavior.

Focused validation passed 17 query units, 39 loading-query integrations, six
parser tests, the exact one-shot and retained-daemon 8+2 rows, 13 core-runtime
tests, cross-package failure/edit/recovery, Rust formatting, archive status,
and diff checks. Independent correction review returned `ACCEPT`. No query
function, new DICE key, traversal, regex engine, fixture, or new utility was
added. External repositories and other output formats remain deferred.

### Stage 8 `tests` / `visible` feasibility ranking

Status: 29-command tests oracle and 39-command labels metadata oracle accepted
through `57192df9`; exact identity, package-context loading, structural
comparison, and direct duplicate rejection accepted through `5bbc4604`; third
Gate A attempt closed `REPLAN` with no code retained; Gate A retry next

Source ref/commit(s): Bazel
`8220c6198837d5c13d53fea211cf3282aa12408a`
`TestsFunction`, `VisibleFunction`, `TargetUtils`,
`TestSuiteImplicitTestsAccumulator`, `QueryVisibility`, `RuleVisibility`,
`PackageSpecification`, and the named `AbstractQueryTest` themes; Buck2
`088c75c7e36805df99c3de29062baa95db700b8b` generic query environment,
target-set, and simple-function shapes; V1
`e218054d4c796655939b968d90208b185decb352` query test accessors, native
`test_suite` storage, and visibility registry.

Decision: Reuse V2 `RuleCapability.rule_class`, typed Starlark values, DICE
package graph, and Buck2 compact request-local evaluator patterns. Reference
only V1's accessor/storage shapes. Reject Buck2/V1 test semantics as incomplete
for Bazel suite expansion, and reject V1 visibility semantics and global
package-group registry. Commit `8212afd6` checks in the oracle-only
`tests-query-expansion` fixture with 16 passing Bazel commands and no query
activation or representation change.

Required later boundary: immutable loading/query metadata for native
`test_suite`, explicit and implicit members, scalar tags/size/manual state, and
semantic equality with same-daemon invalidation. Plumb strict mode separately
as request/query-environment policy. `visible` follows only after a separate
design covers explicit/default visibility, package-group
packages/includes/excludes, same-package access, and the `javatests`/`java`
asymmetry. The first broader design correctly retained the existing DICE path
but exhausted its correction budget: Sol required order-independent natural
sorting with duplicate preservation, common `tags` on all Starlark rules, and
one invariant-safe explicit-or-implicit membership source with derived
`manual`. It closed `REPLAN` without implementation. The replacement review is
loading-metadata-only; strict plumbing and activation remain later packets.
That narrower review also closed `REPLAN`: omitted and explicit-empty `tests`
produce the same implicit membership but Bazel retains their different
attribute explicitness for formatter output. The next oracle-only packet pins
that distinction before another representation proposal. Commit `fd4c5da0`
now proves the same membership and distinct `--output=build` provenance. The
next design must retain an explicitness bit orthogonally within one exclusive
membership representation; no V1/Buck extraction decision changes.

The accepted third design does so: nonempty explicit membership is one variant;
implicit membership carries the omitted-versus-explicit-empty bit. Typed
inherited attrs remain the only Starlark test metadata source, and graph
capability, scalar metadata, attributes, provenance, and edges derive from
finished target state. Existing DICE keys and compact utilities are sufficient.
Sol-low returned `ACCEPT`; implementation Gate A imports no V1/Buck test
semantics and leaves strict policy/function activation later.

The first implementation attempt exposed an adjacent V2-owned representation
gap and retained no code. After root corrected generated `$implicit_tests`
explicitness, Sol-low found that a general query-attribute explicitness boolean
would falsely mark omitted native `filegroup.srcs` explicit because loading
currently stores only the normalized list. This was the packet's second
material correction, so it closed `REPLAN`. No V1/Buck extraction decision
changes. A design-only replacement must close provenance for every current
native and Starlark query-attribute producer before suite metadata is retried.

The replacement design is now Sol-accepted. `QueryAttribute.explicit` has one
total Bazel meaning. Loading retains the missing native filegroup input bit;
mandatory alias and retained Starlark provenance project exact values; suite
`tests` and generated `$implicit_tests` keep the already accepted semantics.
No V1/Buck implementation is imported and no new DICE owner is introduced.
The next packet is oracle-only: add exact filegroup omitted/explicit-empty
build-output rows before retrying Gate A.

Commit `e1d3f910` now supplies that missing native discriminator by extending
`query-labels-attribute-metadata` to 33 passing Bazel commands. Omitted
filegroup `srcs` is absent from build output while explicit empty is printed.
The result changes no reuse decision: Gate A remains V2-owned loading/query
metadata with compact values and existing DICE ownership.

The second Gate A implementation attempt also retained no code. After one
downstream enum-pattern correction, Sol-low found that routing native suite
members through V2's existing dependency-label helper rejects Bazel's accepted
bare source spelling (`tests = ["a.txt"]`). The same restriction exists for
Starlark label-bearing attrs. This second material correction closed the packet
`REPLAN`. No V1/Buck decision changes; the next design must choose the exact
shared loading label-coercion boundary and oracle before metadata work resumes.

The replacement foundation design is Sol-accepted and remains V2-owned. One
package-context converter serves native and Starlark dependency labels;
explicit values use the target package, while label defaults are canonicalized
against the defining `.bzl` package. Native filegroup/alias storage becomes
canonical, and output ownership remains a wrapper. No Buck/V1 grammar,
repository mapping, or DICE owner is imported. Two focused oracle extensions
are next before this foundation is implemented.

Commit `3621b3e7` accepts those two oracle extensions at 37 labels commands and
25 tests commands. Bare/slash labels become canonical implicit source nodes,
cross-package rule use preserves the defining `.bzl` default package, invalid
relative package syntax is pinned, and native suite source members remain
ordinary edges. The implementation packet is now the V2-owned shared converter
and canonical loading storage only; Gate A remains deferred.

The loading foundation then stopped cleanly before implementation because the
V2 `TargetName` type permits extra colons. A loading-local workaround is
rejected; Stage 3 must first make the central target-name validator Bazel-shaped.
No V1/Buck extraction decision changes and no code was retained.

The Stage 3 design is now accepted: implement the pinned Bazel target-name
matrix and trailing `/.` normalization centrally, without importing V1/Buck
identity code. `PackagePath` and package-context raw-label classification stay
outside this packet; loading resumes after the identity boundary is accepted.

Commit `22313daa` accepts that central identity boundary after source-derived
diagnostic and normalization tests. No V1/Buck decision changes. The next
packet resumes the V2-owned package-context loading converter.

Commit `40ac1cd2` accepts that converter and canonical loading storage after
full loading/query validation, including retained-DICE transitions and exact
triple-dot package rejection. No V1/Buck semantics were imported. Gate A may
now retry the accepted suite/test metadata and total explicitness design;
strict policy and function activation remain separate.

That third Gate A attempt retained no code. After one suite-tag correction,
Sol found Rust ordering diverges from Java UTF-16 natural order for
supplementary Unicode, affecting both string tags and labels. The next packet
must design and oracle the V2 comparator boundary; no V1/Buck ordering
semantics are authorized by this replan.

The follow-up source/executable audit disproved that UTF-16 premise for valid
BUILD literals: Bazel's default internal byte-string path and Rust strings
both compare UTF-8 bytes. It instead found generic post-conversion duplicate
label-list rejection, contradicting retained native filegroup and Starlark
duplicate behavior. Sol accepted the structural comparator design but
required oracle coverage for filegroup, direct Starlark label-list, and suite
errors before implementation. No V1/Buck duplicate or ordering behavior is
authorized; configurable selector duplicates and malformed bytes remain
separate.

Commit `57192df9` accepts the corrected append-only oracle at 39 labels and 29
tests commands. Native filegroup, direct Starlark, and native suite duplicate
diagnostics plus string/structural-label ordering are pinned without importing
V1/Buck behavior. The next V2-owned prerequisite corrects direct-list
validation and adds the borrowed identity comparator; selector permutations
and malformed byte strings remain outside it.

Commit `5bbc4604` accepts the V2-owned borrowed structural comparator and
direct native/Starlark duplicate rejection after 70 focused tests and Sol
review. No V1/Buck test model or selector behavior was imported. Native suite
metadata may now reuse these foundations in the next Gate A retry; strict
policy and function activation remain separate.

Commit `7abcbdce` accepts the V2-owned tests loading/query metadata Gate A.
Native suite membership, typed Starlark test metadata, exact capability,
implicit filtering, query provenance, and ordinary graph edges use the
existing V2 package/query DICE owners plus compact `Arc`/`SmallSet` storage.
Full owning-crate tests and independent Sol review passed. No V1/Buck test
evaluator, registry, graph, DICE key, or policy semantics were imported.
Request-local strict-suite plumbing and `tests()` activation remain a separate
design and implementation boundary.

The first post-Gate-A activation design retained that V2-owned boundary:
request-local policy, the current generic evaluator/candidate algebra, and
accessor-shaped loading methods only. Sol required three additional Bazel
oracle discriminators before implementation—nested filter isolation,
filter-before-uniqueness, and literal `-+tag`. No V1/Buck test evaluator or
error taxonomy is authorized by this `REPLAN`; the next packet changes only
the Bazel fixture.

Commit `1edb2775` accepts the three source-critical Bazel rows without changing
the extraction decision. The future activation remains V2 generic evaluator
and request-local policy work; Buck2 contributes only compact set/worklist
shapes, and V1 test-suite evaluation remains rejected.

The corrected activation design is Sol-accepted with the same decision:
implement the Bazel algorithm in V2's generic evaluator and request-local
loading accessor, reuse only compact set/worklist shapes, and import no
V1/Buck test semantics or DICE ownership.

Commit `3a8ae78a` accepts that V2-owned activation. The implementation uses the
existing generic evaluator, candidate arena, loading accessor, retained runtime,
and DICE package graph; Buck2 contributes only compact `SmallSet` uniqueness
and the ordinary iterative worklist shape. No V1 test evaluator, suite model,
strict-policy semantics, registry, diagnostic taxonomy, or DICE ownership was
ported. Exact one-shot/daemon oracle gates, request-local invalidation and graph
reuse, full affected tests, and Sol-low review passed.

The next `visible()` packet remains source/representation audit only. Inspect
V1/Buck visibility code as reference only after the pinned Bazel 9 rule,
package-group, and accessor semantics are fixed; no extraction decision or Rust
reuse is authorized in advance.

The visibility audit rejects V1's implementation semantics. In particular,
`app/slug_node/src/visibility.rs` uses a process-global locked string registry,
target-pattern matching, permissive parse/unknown-repository fallback, and
recursive group lookup without Bazel's DICE ownership, target inheritance,
Java asymmetry, dependency edges, or exact diagnostics. V1 coercion can
silently skip or broaden invalid values. None of that may be ported.

Compact immutable enum/list shapes and the already V2-owned generic filtering
and set patterns remain reference material only. The accepted next packet is a
new Bazel oracle fixture; only after it lands may a reviewed Stage 4 design
choose V2-native compact storage. No V1/Buck visibility code or global
registry is authorized.

Commit `3ecfbfce` lands that fixture with 32 future Slug rows and two
Bazel-only structural rows. The executable evidence corrected the earlier
`labels(visibility)` interpretation: raw explicit loadable group labels
project, omitted/default visibility is empty, and raw direct
`__pkg__`/`__subpackages__` values fail non-loadable lookup; effective
loadable group labels remain ordinary `deps` edges. Independent clean Bazel
runs and final Sol review accepted the corrected 34-command matrix.

The next extraction decision remains design-only and V2-native. Audit current
Stage 4 target/package/default/provenance/equality/edge ownership and specify
typed visibility and package-group storage before implementation. V1/Buck may
inform compact immutable collection shapes only; no registry, string-pattern
semantics, fallback, DICE ownership, or query behavior is authorized.

The first Stage 4 design review returned `REPLAN` for uncovered Bazel 9
`config_setting` default-public behavior and for unordered edge buckets. The
next packet adds only two oracle rows; the corrected design will use one
ordered tagged immutable edge slice. Neither correction authorizes V1/Buck
visibility semantics. Buck-derived compact sets/slices remain utility shapes
only.

Commit `a11b43da` accepts the two `config_setting` discriminators and preserves
all earlier normalized evidence. The corrected Stage 4 design is now
Sol-accepted: implement V2-owned typed visibility, direct package contents,
unresolved group/include labels, explicit provenance, and one ordered tagged
edge slice using compact Buck-derived utility shapes only.

The accepted ownership deliberately rejects eager recursive resolution in
package or unconfigured-graph DICE computes. Missing/wrong-kind references and
include cycles remain graph data; Stage 8's future request-local accessor owns
iterative cross-package resolution, diagnostics, and per-walk cycle state. No
V1 registry, string-pattern semantics, fallback, group evaluator, DICE owner,
or query-command implementation is imported.

Commit `f9ae7337` accepts the V2-owned representation with compact immutable
sets/slices and the existing package/query DICE owners. No V1 visibility
registry, target-pattern parser, permissive fallback, recursive evaluator,
global lock, or command implementation was imported. The only extracted shape
remains the already approved Buck-derived compact collection pattern.

Stage 8's first design audit retained the V2-only ownership and rejected any V1
import, but found three missing Bazel discriminators before activation:
cross-package group/include lookup, real-first real/fake same-label input
identity, and label-keyed materialization of two same-label fake callers while
retaining the first representative's consuming package. The fixture must grow
from 22 to 25 `visible()` rows first.

The corrected design uses the existing candidate arena, compact request-local
sets, typed visibility graph, and package-graph DICE keys. It passes predicate
callers through the existing printed-label `eval_all`, leaves streamed input
batches unmaterialized, performs no query-topology recording during visibility
lookup, and imports no V1 registry, pattern parser, fallback, evaluator, lock,
or cache.

Oracle commit `a376e30e` accepts the three missing discriminators and imports
no production or V1 code. It also corrects the source audit: ordinary Bazel
query uses label-keyed predicate materialization, not `FakeLoadTarget` object
equality. The future implementation can reuse V2's existing label-materialized
`TargetSet`, streamed candidate batches, compact cycle sets, and package-graph
DICE keys without changing provenance representation or extracting V1.

The corrected Stage 8 design is accepted for implementation with no V1 reuse
beyond already approved compact worklist/set shapes. It uses existing V2
candidate IDs, label materialization, typed graph nodes, and package-graph DICE
ownership. No V1 visibility parser, registry, evaluator, global state, or
fallback enters the bounded activation.

Commit `76025ede` lands that activation without importing V1 code or adding a
DICE key, cache, lock, provenance representation, formatter, or fallback.
Existing V2 candidate IDs, label materialization, streamed batches, typed
visibility nodes, compact `SmallSet` cycle state, and package-graph DICE
ownership were sufficient. The exact 25-row one-shot/daemon gate, ordered
singleton-delivery regression, no-topology assertion, and cross-package
format/restrict/delete/recreate lifecycle passed; independent final review
returned `ACCEPT` after one evidence-only test strengthening.
### Stage 8 TestRunner semantic prerequisite decision (2026-08-11)

Source inspected: Bazel 9.2 `RuleConfiguredTargetBuilder`,
`TestActionBuilder`, and `TestRunnerAction`; V1 commit
`e218054d4c796655939b968d90208b185decb352` test provider, external runner,
orchestrator, client, event, and exit-code paths; current V2
`ActionSpec`/`ConfiguredNodeResult`/DefaultInfo/runfiles/Run view/REAPI
boundaries.

Decision: reject V1's Buck-specific external test-runner protocol, provider
shape, orchestration graph, event stream, and client exit machinery. Retain
only the already adopted V2 action-registry, immutable `Arc<[T]>`,
`CompactString`, deterministic small collection, `Dupe`, and
`Allocative` patterns for a future Bazel-owned TestRunner semantic action.
Do not add an interner, cache, weak identity hash, parallel action graph, or
command-owned reconstruction.

The future action is blocked on the real embedded `@bazel_tools//tools/test`
repository/content closure and generated-input/multi-output REAPI breadth.
Design that prerequisite first. Any later import must cite exact Bazel 9.2
verbatim files and preserve structural DICE equality plus separated
configured/action/REAPI/result identity domains.

### Stage 6 contextual command-configuration preparation utility decision (2026-08-28)

Source inspected: retained V2 configuration/DICE/Starlark/loading owners;
Buck2 immutable `Arc` slice, `Dupe`, `Allocative`, compact string and small
deterministic-map patterns; clean Zabel `0795445f…` request-session occurrence
and typed-final-option ownership as concept guidance only.

Decision: implement a V2-owned `Arc<[CommandConfigurationOccurrence]>`, one
analysis DICE preparation key and one batch update over the existing native
vector/Starlark map. Reuse retained compact utilities only. Do not import a V1
or Zabel parser, option table, scheduler, cache, identity, diagnostic or rule
engine; do not add an interner or retained standard map/set. Exact behavior is
anchored in pinned Bazel 9.2 `StarlarkOptionsParser`, `PlatformOptions` and
their tests. Retained-size, Arc-clone, equal-allocation, structural identity,
invalidation and lifecycle tests are the acceptance evidence.

### Stage 6 command-registration overlay utility decision (2026-08-28)

Source inspected: the accepted V2 contextual registration expander and typed
configuration projection; Buck2 immutable `Arc` slice, `Dupe`, `Allocative`
and compact `SmallMap`/`SmallSet` patterns; clean Zabel `0795445f…` separate
final-option/MODULE ownership as concept guidance only.

Decision: reuse the existing immutable registration result and phase-scratch
compact maps/sets for a configuration-keyed signed command expansion. Add no
V1/Buck2/Zabel import, retained standard collection, parallel registration
store, cache or interner. Exact sign, ordering and command-before-MODULE
behavior is anchored in pinned Bazel 9.2 `RegisteredToolchainsFunction`,
`RegisteredExecutionPlatformsFunction`, `SignedTargetPattern` and
`TargetPatternUtil`; focused ordering, structural equality, observation and
lifecycle tests are the acceptance evidence.

### Stage 6 configured platform/toolchain utility decision (2026-08-28)

Source inspected: current V2 structural configuration, configured node,
platform, condition, registration and marker owners; Buck2 execution-platform
immutable values and ordered scratch maps; V1 toolchain resolution; clean
Zabel `0795445f…` target-platform and requested/actual type separation.

Decision: retain configured platforms and resolution rows in immutable
`Arc` slices with canonical configured keys, `Dupe` and `Allocative`; use
compact ordered maps/sets only as compute scratch. Reject V1 string labels,
host-as-target fallback, standard hash collections and selector ownership;
reject Buck2/Zabel semantic ownership, IDs, stores and schedulers. Add no
cache, interner, retained standard collection or provider-shaped placeholder.
Pinned Bazel 9.2 alone fixes selection behavior; retained-size, unchanged-Arc,
invalidation, cancellation and repair checks are required implementation proof.

### Stage 4/5/7A configured-fragment utility decision (2026-08-30)

Source inspected: pinned Bazel 9.2 `FragmentCollection`, `RuleContext`,
`StarlarkSubrule`, `CppConfiguration`, `BuiltinRestriction`, and Exec-transition
sources; authenticated rules_cc 0.2.17 `fdo_context.bzl`; the retained
starlark-rust evaluator/method-dispatch and Buck2-derived compact collection
owners; and Zabel's evaluator-local fragment projection as peer guidance only.

Decision: implement one V2-owned evaluator-local `CppFragmentProjection` over
the sole structural `SlugConfiguration`, one shared frozen `cpp` value, and
separate cached rule/subrule collection facades with their own declaration
authorization. Reuse `Arc`, `Dupe`, `CompactString`, and `SmallSet`; import no
V1, Bazel Java, Buck2 semantic owner, or Zabel code/layout. Add no parser,
interner, DICE key, cache, lock, parallel C++ options store, rules_cc branch, or
`cc_common`/`cc_internal` special case. Bazel 9.2 alone fixes the 12-name active
inventory, complete default private-API allowlist, six admitted FDO-facing
methods, and bounded host-compilation-to-Exec projection. Absolute-path profile
producers, other fragments, other Exec rewrites, and action builtins remain
explicit successors. Hash/ledger, caller provenance, target/Exec separation,
stable-daemon C0/C1/C0, retained-size, cap, and staged-only review are the
acceptance evidence.

Implementation status: complete and terminally accepted at 665 production and
648 proof additions. The retained utility use remains bounded to the reviewed
compact/shared ownership shapes; no V1 or peer-implementation semantics entered
the production path.

### Stage 6 typed runfiles/DefaultInfo utility decision (2026-08-31)

Source inspected: the accepted V2 Artifact, dense depset, provider and action
owners; pinned Bazel 9.2 Runfiles, SymlinkEntry, DefaultInfo and configured-
target construction sources; Buck2-derived retained compact utilities; and
clean Zabel `0795445f…` runfiles phase ownership as peer guidance only.

Decision: reuse V2 `AnalysisArtifact`, dense `Depset`/`AnalysisDepset`, `Arc`,
`CompactString`, `Dupe`, `Allocative`, and phase-scratch Fx maps for a typed
immutable runfiles category. Import no V1/Buck2/Zabel semantic owner, parser,
rule implementation, scheduler, cache, interner, layout or compatibility
claim. The private SymlinkEntry occurrence token preserves Bazel identity-
sensitive leaf behavior; iterative topology conversion and bidirectional alias
maps preserve structural publication identity without a parallel retained
graph. Exact behavior remains pinned to Bazel 9.2, while Rust Unicode/layout
and DICE identity stay Slug-native.

Implementation status: terminally accepted at 837 net / 1,001 gross production
and 255 net / 286 gross proof lines. No V1 extraction was required. The next
support-action successor must continue to reuse the accepted typed owners and
must not add a parser, global cache, rule-family branch or second runfiles
representation.

### Stage 6 runfiles support-action utility decision draft (2026-08-31)

Proposed reuse is one shared `Arc<RunfilesSupport>` across the complete provider
and four typed recipes, with dense runfiles/package depsets retained once and stable
Artifact deduplication kept as iterative phase scratch. A narrow private typed
FilesToRun carrier corrects nested/subrule transport without a generic payload,
second graph, interner, cache, task, or path-based association. Atomic registry
preflight owns publication; no lock crosses evaluation or computation.

The former three-action proposal is rejected because Bazel 9 Bzlmod registers
`RepoMappingManifest` first. Commits `80a6bfd3a` and `2483dd7e2`
terminally accept its complete package-metadata/collector prerequisite. The
corrected four-action proposal also adds one private source-manifest Artifact,
one typed action-family enum and a special RunfilesTree output discriminator;
it imports no V1/Buck2/Zabel semantic owner, representation or layout.

Independent retained-representation review returned `ACCEPT` for R2. R3 adds
only the pinned-source-required unresolved-symlink Artifact admission to the
existing runfiles importer; focused correction rereview returned `ACCEPT`.
Bazel 9.2 fixes behavior; Zabel is peer ownership/optimization guidance only.
Existing `Arc`, `Dupe`,
`Allocative`, compact maps, retained Artifact, action registry, and dense
depset utilities are sufficient; add no interner, cache, global state, task,
lock, flattened repository list, full-child retention or second runfiles
graph.

### Stage 4/6 transitive runfiles package-mapping correction (2026-08-31)

Independent review rejects the three-action/no-mapping draft. Under Bzlmod,
Bazel's external-repository server setting makes transitive package tracking
mandatory and produces `RepoMappingManifest` first. A caller-created absence
token is not semantic evidence.

The corrected prerequisite proposes one dense configured-result package closure
whose leaves retain canonical package identity, selected mapping entries, and
generated-owner cohort identity. This follows Zabel's useful separation between
configured package closure and later manifest derivation while deliberately not
copying its digest/store representation. No action, mapping bytes, cache,
interner, new DICE key, or V1 owner is admitted before independent review.

### Stage 4/6 transitive package collector utility correction (2026-08-31)

Source inspected: the complete Legacy/Observed root mapping keys and views,
root/external PackageRecorder paths, configured condition/platform/toolchain
preparation, all configured edge producers, pinned Bazel 9.2
`TransitiveDependencyState`, `TargetProducer`,
`ConfiguredTargetAndDataProducer`, dependency/config-condition/native-
toolchain rules, and Zabel `0795445f…` configured/nonconfigured transitive
package owners as peer guidance.

Decision: reuse Slug's existing root-mapping DICE owner, immutable `Arc` values,
compact scratch maps/sets and Buck2-derived dense depset. Add one neutral
package/mapping leaf and one phase-local collector with separate direct-package
and configured-child inputs; publish only the dense closure on existing
loading/analysis results. Selector conditions are configured rows, selected
toolchain implementations contribute, and requested types/candidate platforms
remain noncontributing resolution topology. Do not import Zabel's repository
IDs, stores, physical-fingerprint digests, registration family, layout or
scheduler, and do not add a V1 owner, flattened repository set, cache,
interner, task, lock or new DICE key. Bazel 9.2 alone fixes package contribution
and mapping behavior; Zabel informs only phase ownership and compact
representation.

Implementation status: independent R3 correction rereview returned `ACCEPT`.
The loading/metadata implementation candidate reuses the existing immutable
mapping slices, `Arc` ownership, `CompactString`, `Allocative`, and accepted
dense depset alias. It adds no flat set, cache, interner, global owner, lock, or
new DICE key. The configured collector remains the separate reviewed successor;
neither implementation may create an action or complete FilesToRun.

R4 correction: terminal implementation review found that the older
`PackageLoadKey` could still construct an empty-mapping `LoadedPackage` for
legacy query/runtime adapters. Extract the existing immutable evaluation
fields into one `Allocative` core and use distinct complete and legacy wrappers;
only the complete wrapper retains `Arc<RunfilesPackageMetadata>`. This is a
bounded type-isolation refactor, not a second retained graph or mapping owner.
It preserves Arc-owned frozen-module lifetimes, adds no collection, interner,
cache, task, lock, key, or deep clone, and prevents the legacy result from
entering configured package-closure consumers at compile time.
Independent R4 design correction review returned `ACCEPT`; implementation may
now apply only this type isolation and the missing innate-owner proof before
terminal correction review.

Implementation candidate: the common evaluation core preserves the prior
structural equality cutoff and frozen-module lifetime, the complete wrapper
adds one `Arc<RunfilesPackageMetadata>`, and the legacy wrapper adds no bytes.
The existing 28-test bzl-invalidation suite and full loading/query dependents
pass; no retained collection, clone, cache, interner, task, lock, or key was
added. Independent terminal correction review returned `ACCEPT`. The
configured collector landed in `2483dd7e2` with one dense closure per result,
typed prepared carriers, iterative composition, and no second graph. The
corrected four-action support design is now the active successor.
Its implementation candidate uses only the accepted Rust-native typed owners,
passes the required serial validation and cap/hygiene gates, and introduces no
V1 extraction or semantic owner. R3 terminal review nevertheless returned
`REPLAN`: the analysis seam discarded `HostPathFlavor`, permitting the
non-Windows graph for Windows. R4 retains the existing typed
flavor/environment pair and fails closed before provider/registry publication.
The flavor is an eligibility gate, not a new retained recipe field; no new
collection, clone, cache, interner, task, lock, key, graph, or memory owner is
added. Focused R4 review returned `ACCEPT`; the corrected candidate passes the
full serial gates within the unchanged caps. Terminal implementation review
also returned `ACCEPT`; no V1, Buck2, or Zabel semantic owner was introduced.

### Stage 6 FilesToRun Spawn expansion utility decision (2026-08-31)

Status: implementation terminally `ACCEPTED` in commit `21db5d7b8`; the
initial design review's sole `REPLAN` was a mistyped pinned
`SpawnAction.java` digest, which focused correction rereview accepted.

Decision: reuse the existing Rust-native `FilesToRunProvider`,
`AnalysisArtifact`, dense `AnalysisDepset`/`RetainedArtifactInputs`,
`SmallMap`, `Arc<RunfilesSupport>`, `Allocative`, and one shared
`PublicationEqState`. Add no V1 extraction, Buck2 import, second graph,
flattened File vector, interner, cache, task, lock, DICE key, or deep clone.
The provider clone retained by one action is a bounded shallow clone of dense
handles, Artifacts, and an `Arc`; changing provider occurrence identity to add
another `Arc` is deferred until a measured need exists.

Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a`
`ARCHITECTURE.md`, `providers.zig`, `starlark_action_registration.zig`, and
`logical_actions.zig` are reference-only peer guidance for keeping provider
authentication through action lowering and sharing one invocation-scoped
depset import context. Copy no Zig code, row layout, IDs, errors, action
representation, scheduler, cache, digest, or compatibility claim. Bazel 9.2
`FilesToRunProvider`, `RuleConfiguredTargetBuilder`, `SpawnAction`, and
`StarlarkActionFactory` remain the sole exact behavior authority.

Implementation preserves the frozen utility decision: one dense provider root,
one shared `PublicationEqState`, shallow provider clones and no flattened File
vector or second graph. Final growth is 152 net / 604 gross production and 254
net / 304 gross proof Rust lines. Full serial owner/downstream and terminal
review pass. The next Bzlmod declaration-signature packet changes only generated
call binding and has no retained utility, hashing, collection, interning, clone,
memory-accounting or V1 extraction decision.

### Stage 6 assigned-global Starlark module utility decision (2026-08-31)

Status: implementation terminally `ACCEPTED` after a focused design correction
requiring assigned origin and public visibility together for `use_repo_rule`,
in `WP-6-7A-bzlmod-declaration-selection-identity-parity-r1`; commit pending.

Source inspected: Buck2 `088c75c7e36805df99c3de29062baa95db700b8b`
starlark-rust `environment/{names,modules}.rs`, evaluator module stores, and
load evaluation; pinned Bazel 9.2 net.starlark `Module.getGlobals`,
`BzlLoadFunction.execAndExport`, `RegularRunnableExtension`,
`InnateRunnableExtension`, `StarlarkRepoRule`, and the named runfiles/Bzlmod
tests. V1 supplies no applicable owner.

Decision: retain Buck2's `SmallMap` name/slot table and slot-indexed assignment
stores. Add one evaluation-scratch packed assignment bit per slot and fold it
into the existing frozen name entry, with a layout assertion that the retained
tuple does not grow. Public/private visibility remains unchanged and distinct
from assigned/load origin. One hidden `FrozenModule::get_assigned` returns an
assigned binding together with its unchanged visibility. Module-extension
selectors accept either visibility; `use_repo_rule` requires `Public` from the
same origin-aware lookup so public-named raw loads remain excluded. No
parser/source scan, second map/set, interner, cache, global state, DICE key, or
producer-side table is admitted.

Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a`
`module_extension_declaration_host.zig` and
`module_extension_execution_capture.zig` are concept/test-only guidance for
first producer label/name retention through repository-rule aliases. Copy no
Zig code, layout, allocator, evaluator, scheduler, cache, error, or behavior.
Bazel 9.2 alone fixes assigned-global selection and alias semantics.

Memory class: the packed mutable bits are evaluator scratch and die at freeze;
the folded bit is DICE-retained frozen-module semantic metadata with no new
allocation and no evaluator borrow. Existing source/manifests own invalidation,
existing frozen heaps own values, and existing DICE publication/cutoff,
cancellation, eviction and shutdown lifetimes remain unchanged.
