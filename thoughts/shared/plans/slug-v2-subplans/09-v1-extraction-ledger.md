# Stage 9: V1 Extraction Ledger

## Goal

Track every deliberate extraction from Slug V1 into V2 so useful work is
preserved without importing V1 architectural debt blindly.

## Extraction Rule

An extraction is acceptable only when it has:

- a named V2 owner stage;
- the V1 source path or test path;
- the Bazel source/test oracle or oracle fixture that justifies it;
- a clear import mode: copy, port, rewrite from behavior, reference-only, or
  reject;
- validation evidence after landing.

The current mixed-root `codex/slugv2` branch is treated as an extraction source
too. Importing code or fixtures from that branch into a clean-root V2 line must
record whether the import is a direct cherry-pick, a port, a rewrite from
behavior, or reference-only. Do not treat mixed-root commits as already accepted
V2 trunk work merely because they compile in the old workspace.

The clean-root remediation branch keeps the already-landed V2-only stage
artifacts from `codex/slugv2` only where the owner subplan records the oracle
fixture or Bazel source citation. V1-only source/test trees and the unwrapped
`remote_execution` source candidate are reference material through archive refs,
not active-root content.

## Workflow

1. Open the V2 owner plan and identify the exact behavior needed.
2. Inspect V1 source and tests, plus the Bazel oracle source/test.
3. Choose an import mode:
   - `copy`: code is infrastructure with no V1 semantic leak;
   - `port`: code is useful but names/types/path assumptions must change;
   - `rewrite from behavior`: keep tests/lessons, not implementation shape;
   - `reference-only`: external implementation or backend contract only;
   - `reject`: document why the V1 surface should not enter V2.
4. Add or update an oracle fixture before landing the extraction.
5. Update this ledger and the owner plan with validation evidence.

## Review Checklist

Before moving an entry out of `Proposed`, answer:

- Does it contain Buck cell identity, `buck-out`, `BUCK`, `.buckconfig`, or
  direct-local assumptions?
- Does it rely on process-global state or fallback scanners?
- Which DICE key owns the semantic value in V2?
- Which Bazel source/test proves the behavior?
- Which command proves the extraction after landing?

## Ledger

| Status | V2 Stage | V1 Surface | Import Mode | Oracle / Validation |
|--------|----------|------------|-------------|---------------------|
| Proposed | Stage 5 | `app/slug_bzlmod/src/parser.rs` | Port selectively | `MODULE.bazel` parser fixtures against Bazel |
| Proposed | Stage 5 | `app/slug_bzlmod/src/extension_execution_dice.rs` | Rewrite from behavior plus selective port | module extension replay fixtures |
| Proposed | Stage 5 | `tests/core/bzlmod/test_plan61_guardrails.py` | Port tests after fixture cleanup | bzlmod same-daemon replay fixtures |
| Proposed | Stage 7 | `tests/plan34/test_reapi_local_executor_smoke.py` | Port harness | NativeLink REAPI fixture |
| Proposed | Stage 7 | `tests/plan34/validate_reapi_evidence.py`, `tests/plan34/test_ci_gate.py`, `.github/actions/setup_plan34_nativelink/action.yml`, `.github/actions/run_plan34_reapi/action.yml`, `.github/workflows/plan34-reapi.yml` | Rewrite behavior into V2 oracle harness; copy only small validator schema ideas | `shell-action-reapi` plus evidence validator |
| Proposed | Stage 7 | `app/slug_execute/src/execute/action_digest.rs`, `app/slug_execute/src/execute/action_digest_and_blobs.rs` | Port concepts, rewrite implementation around Bazel action declarations | REAPI action identity fixtures |
| Proposed | Stage 7 | `app/slug_execute_impl/src/executors/re.rs`, `app/slug_execute/src/re/client.rs`, `app/slug_execute/src/re/remote_action_result.rs`, `app/slug_execute_impl/src/re/download.rs` | Selective rewrite; reject Buck executor/config assumptions | upload/execute/download REAPI fixtures |
| Proposed | Stage 7 | Plan 31 persistent action-cache tests | Port tests and materializer checks | REAPI AC hit/stale-entry fixtures |
| Proposed | Stage 7 | `app/slug_execute_impl/src/sqlite/action_cache_db.rs`, `app/slug_execute_impl/src/sqlite/tables/action_cache_table.rs`, `app/slug_execute_impl/src/executors/action_cache.rs`, `app/slug_execute_impl/src/executors/caching.rs`, `app/slug_server/src/daemon/state.rs`, `tests/plan31/test_persistent_re_action_cache.py` | Port schema/value semantics and stale-entry behavior; Stage 3 owns output/cache layout | durable RE `ActionDigest -> ActionResult` fixtures |
| Proposed | Stage 7 | Plan 34 paramfile/generated-output fixtures | Port fixtures after Stage 6 can declare matching action graphs | `reapi-paramfile-input-tree` and `reapi-generated-output-reupload` |
| Proposed | Stage 7 / Stage 8 | Plan 34 `cc_actions` and `rules_cc` fixture themes | Stage 7 owns execution evidence; Stage 8 owns ruleset conformance breadth | `rules-cc-reapi-basic` plus Stage 8 rules_cc fixtures |
| Reference only | Stage 7 | NativeLink source checkout | Backend contract reference, not Slug import | same REAPI evidence with `remote_service=nativelink` |
| Reference only | Stage 7 | actiond source checkout | Optional REAPI backend validation only | same REAPI evidence with `remote_service=local_actiond` |
| Landed | Stage 6 | `app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`; `app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs` | Rewrite from behavior | V2 commits `9e519f97`, `ed636308`, `aa9b820f`; fixtures `depset-orders-and-rejections`, `custom-rule-analysis-basic`, `ctx-attrs-files-executable`, `default-info-runfiles-executable`, `provider-output-group-basic`; validation in Stage 6 plan |
| Proposed | Stage 3 | V1 label/repo mapping helpers | Rewrite from behavior | Bazel label/output path oracle fixtures |
| Proposed | Stage 4 | V1 `slug_interpreter_for_build` globals/loading tests | Port tests, rewrite loading boundary | Bazel `PackageFunction`/`BzlLoadFunction` fixtures |
| Proposed | Stage 6 | V1 `cc_common` feature and provider work | Port selectively | rules_cc public fixture plus Bazel analysis oracle |
| Proposed | Stage 8 | V1 public ruleset smoke fixtures | Port tests after sanitizing versions and paths | ruleset oracle fixtures |
| Proposed | Reject by default | Direct-local executor success, copied-output bridge hits, Buck output-root spelling, and old compiled NativeLink/actiond adapter artifacts | Reject | Does not prove V2 Bazel REAPI parity |
| Proposed | Reject by default | V1 Buck cell graph and fallback cell machinery | Reject unless Stage 5 proves a Bazel equivalent | Stage 3/5 identity and bzlmod tests |
| Proposed | Reject by default | V1 BXL user surface | Defer as Slug extension | Not part of Bazel compliance |
| Reference only | Stage 0 | Mixed-root `codex/slugv2` V1 source/test paths, old docs/plans, root Bazel/Buck metadata, shims, wrappers, and `remote_execution/` | Reject from active root; inspect through archive refs or prototype branch only | `scripts/v2_archive_status.sh`; clean-root tracked-file grep in Stage 0 |
| Landed | Stage 1 | `codex/slugv2` Stage 1 oracle harness and fixtures | Retain as V2-only scaffold | Owner plan `01-compliance-oracle-harness.md`; `python3 -B -m tools.v2_oracle list`; `python3 -m pytest -q tests/v2_oracle/test_v2_oracle.py` |
| Landed | Stage 2 | `codex/slugv2` Stage 2 CLI/core V2 crates | Retain as V2-only scaffold | Owner plan `02-rust-skeleton-and-runtime-substrate.md`; `cargo check -p slug_cli_v2 -p slug_core_v2`; `cargo test -p slug_cli_v2` |
| Landed | Stages 3-8 | `codex/slugv2` V2 crate and fixture checkpoints under `app/slug_*_v2` and `tests/v2_oracle/` | Retain only where owner stage records fixture/citation evidence | Owner plans `03` through `08`; focused cargo tests and oracle fixture commands recorded per stage |

## Evidence Template

Use this when updating a ledger row after landing:

```text
Status:
V2 commit:
V1 source inspected:
Bazel oracle:
V2 fixture:
Expected evidence artifact:
Implementation summary:
Validation:
Residual risk:
```

### Stage 5 MODULE.bazel parser directive records

Status: Partially landed
V2 commit: Stage 5 parser checkpoints on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\parser.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 oracle fixtures for MODULE directive shape
V2 fixture: `module-repo-directives`, `module-extension-tags`, `module-registration-dev-dependency`, `module-use-repo-rule-dev-dependency`, `module-multiline-directives`, `module-single-quoted-directives`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected` for each fixture
Implementation summary: Ported behavior selectively by rewriting directive capture in the independent V2 parser: repository-rule proxies, extension repo overrides/injections, extension tags, registration `dev_dependency` flags, `use_repo_rule` invocation-vs-factory `dev_dependency` validation, multiline logical statement collection, and Starlark single-quoted string handling; no V1 Starlark evaluator, Buck cell, or process-global machinery was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle runs for each listed fixture; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: parser remains a lightweight directive recorder; full Starlark MODULE evaluation, extension execution, repo mapping replay, and lockfile lifecycle are still later Stage 5 DICE slices

### Stage 5 local module graph substrate

Status: Partially landed
V2 commit: Stage 5 local resolution checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\resolution.rs`
Bazel oracle: Bazel 9.1.1 `module-resolution-basic` fixture with build plus cquery evidence
V2 fixture: `module-resolution-basic`, `module-local-override-version-selection`, `module-local-override-request-order`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the local-override graph behavior into a small V2 substrate: typed module keys, root/local sources, Bazel-shaped `<module>+` canonical repo names, deterministic apparent-to-canonical repo mappings, local override declared-version selection, and order-independent repeated local override requests; registry MVS, yanked policy, lockfiles, DICE keys, and repository materialization remain later Stage 5 work
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle runs for `module-resolution-basic`, `module-local-override-version-selection`, and `module-local-override-request-order`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: local graph substrate still does not implement multiple_version_override, registry-backed modules, yanked policy, or lockfile replay; those remain owned by Stage 5.2, 5.3, and 5.6

### Stage 5 registry MVS substrate

Status: Partially landed
V2 commit: Stage 5 registry MVS checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\resolution.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-mvs-basic` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `module-registry-mvs-basic`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the first registry-backed MVS behavior into V2: typed registry module records, max requested-version selection, registry-backed module sources, and repo mapping through the existing `ResolvedGraph`; no V1 registry client, cache, lockfile, or fetch/materialization implementation was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-mvs-basic`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry file hashes, source.json repo spec materialization, multiple-version overrides, lockfile replay, and DICE keys remain later Stage 5.2/5.6 work

### Stage 5 yanked-version policy substrate

Status: Partially landed
V2 commit: Stage 5 yanked-version policy checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `yanked-version-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the yanked selected-version policy into V2: selected yanked versions are rejected by default and accepted through an explicit allowlist or allow-all policy over the resolved graph; no V1 registry client, lockfile writer, environment policy, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry file hashes, lockfile selected-yanked-version recording, `BZLMOD_ALLOW_YANKED_VERSIONS`, multiple-version overrides, source.json repo specs, and DICE keys remain later Stage 5.2/5.6 work

### Stage 5 registry source.json policy substrate

Status: Partially landed
V2 commit: Stage 5 registry source.json checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `registry-source-json-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `registry-source-json-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only archive `source.json` metadata validation into V2: structured JSON parsing for `url`/`urls`, `integrity`, `type`, `strip_prefix`, `patches`, and `patch_strip`, with Bazel-shaped diagnostics for missing source URL, missing integrity, and malformed JSON; no V1 registry client, downloader, repository materializer, lockfile writer, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `registry-source-json-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: archive download/extraction, patch application, registry hash enforcement, repository materialization, lockfile replay, and DICE-owned registry metadata keys remain later Stage 5.2/5.5/5.6 work

### Stage 5 registry metadata parser substrate

Status: Partially landed
V2 commit: Stage 5 registry metadata checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `yanked-version-policy`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only registry `metadata.json` parsing into V2: required `versions`, optional `homepage`, optional `repository`, and `yanked_versions`, with conversion from version-string yanked entries into the existing `ModuleKey -> reason` validation input; no V1 registry client, fetch cache, lockfile writer, environment policy, or DICE key was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-policy`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: registry clients, registry file hashes, lockfile selected-yanked-version recording, `BZLMOD_ALLOW_YANKED_VERSIONS`, multiple-version overrides, and DICE-owned metadata keys remain later Stage 5.2/5.6 work

### Stage 5 multiple-version override resolver substrate

Status: Partially landed
V2 commit: Stage 5 multiple-version override checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\resolution.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-multiple-version-override` fixture using a workspace-local registry, `bazel mod graph`, and `bazel mod dump_repo_mapping`
V2 fixture: `module-registry-multiple-version-override`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only selected registry graph behavior for root `multiple_version_override`: allowed requested versions remain selected side by side, ordinary modules still use MVS highest-version selection, and canonical repos for multiple selected versions use Bazel-shaped `<name>+<version>` names; no V1 registry client, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-multiple-version-override`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: multiple-version override lockfile data, exact full repo-mapping fixture coverage, registry client fallback, DICE-owned graph keys, and materialization remain later Stage 5.2/5.3/5.6 work

### Stage 5 single-version override resolver substrate

Status: Partially landed
V2 commit: Stage 5 single-version override checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\resolution.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `module-registry-single-version-override` fixture using a workspace-local registry, `bazel mod graph`, and `bazel mod dump_repo_mapping`
V2 fixture: `module-registry-single-version-override`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only selected registry graph behavior for root `single_version_override`: override-provided versions replace requested versions before registry module lookup, normal canonical repo names are preserved, and ordinary MVS plus multiple-version override behavior remains intact; no V1 registry client, patch application, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-registry-single-version-override`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: single-version override patches, alternate registry selection, lockfile data, registry client fallback, DICE-owned graph keys, and materialization remain later Stage 5.2/5.3/5.6 work

### Stage 5 ordered registry fallback substrate

Status: Partially landed
V2 commit: Stage 5 ordered registry fallback checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `registry-fallback-order` fixture using two workspace-local registries and `bazel mod graph`
V2 fixture: `registry-fallback-order`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only ordered registry module selection into V2: earlier registries win per module key and later registries fill misses before the existing registry resolver consumes the selected module set; no V1 HTTP client, cache, lockfile writer, DICE graph, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `registry-fallback-order`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: HTTP/file registry clients, registry file hashes, refresh/error lockfile modes, same-daemon registry mutation replay, DICE-owned registry metadata keys, and materialization remain later Stage 5.2/5.6 work

### Stage 5 selected-yanked visible lockfile substrate

Status: Partially landed
V2 commit: Stage 5 selected-yanked lockfile checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\lockfile.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `lockfile-selected-yanked-version` fixture using a workspace-local yanked registry, `--allow_yanked_versions=yyy@1.0.0`, and generated lockfile-line printing
V2 fixture: `lockfile-selected-yanked-version`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only visible lockfile subset parsing into V2: `lockFileVersion`, `registryFileHashes`, and `selectedYankedVersions` are parsed from JSON and selected yanked keys become `ModuleKey` values; no V1 writer, hidden lockfile, replay/error-mode state machine, extension cache, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-selected-yanked-version`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: lockfile write/update/refresh/error modes, hidden lockfiles, extension replay entries, facts/factsVersions, environment-sourced allowlists, registry hash enforcement, DICE ownership, and same-daemon stale-data rejection remain later Stage 5.6 work

### Stage 5 registry-hash lockfile error substrate

Status: Partially landed
V2 commit: Stage 5 registry-hash lockfile checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\lockfile.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-registry-hash` fixture using a stale BCR `registryFileHashes` entry and `bazel mod graph --lockfile_mode=error`
V2 fixture: `lockfile-error-mode-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only registry-hash comparison over parsed visible lockfile data into V2: expected hashes from `MODULE.bazel.lock` are compared with an explicit observed digest map and mismatches produce Bazel-shaped checksum diagnostics; no V1 registry client, hasher, writer, hidden lockfile, replay/error-mode state machine, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-error-mode-registry-hash`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: computing registry file hashes, HTTP/file registry clients, lockfile write/update/refresh modes, hidden lockfiles, same-daemon stale-registry replay, DICE-owned registry inputs, and materialization remain later Stage 5.2/5.6 work

### Stage 5 yanked-version environment allowlist substrate

Status: Partially landed
V2 commit: Stage 5 yanked env allowlist checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py` and `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\registry.rs`; no env-specific allowlist implementation or fixture was imported
Bazel oracle: Bazel 9.1.1 `yanked-version-env-allowlist` fixture using `BZLMOD_ALLOW_YANKED_VERSIONS=yyy@1.0.0`
V2 fixture: `yanked-version-env-allowlist`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only env value parsing into V2: absent/empty rejects, `all` allows all, and comma-separated `module@version` entries become a `YankedVersionPolicy::AllowList`; no V1 process environment plumbing, DICE key, lockfile writer, command-line precedence logic, or same-daemon replay logic was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-env-allowlist`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: process env wiring into DICE-owned bzlmod keys, env-change invalidation, command-line/env precedence, selected-yanked lockfile writing, and same-daemon replay remain later Stage 5.2/5.6 work

### Stage 5 bzlmod DICE environment key substrate

Status: Partially landed
V2 commit: Stage 5 bzlmod DICE env-key checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\dice_graph.rs`, `C:\tmp\kuro-v1-archive\tests\core\bzlmod\test_plan61_guardrails.py`
Bazel oracle: Bazel 9.1.1 `yanked-version-env-change` fixture proving command env changes alter yanked-version policy behavior
V2 fixture: `yanked-version-env-change`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Added only key-shaped bzlmod semantic input structs into V2: lockfile mode and parsed environment policy participate in equality, hash, and stable serialization for resolved graph keys; no V1 DICE compute implementation, async locking, registry client, lockfile replay, extension execution, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-env-change`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: actual DICE `Key` wiring, digest producers for files/env/registries, registry clients, lockfile replay modes, module extension keys, and same-daemon materialization replay remain later Stage 5.2/5.4/5.6 work

### Stage 5 bzlmod command/environment policy key substrate

Status: Partially landed
V2 commit: Stage 5 yanked command/environment policy checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: No V1 compute code imported for this slice; it extends the prior Stage 5 yanked-policy/DICE substrate from Bazel-observed behavior
Bazel oracle: Bazel 9.1.1 `yanked-version-command-env-union` fixture proving `--allow_yanked_versions` and `BZLMOD_ALLOW_YANKED_VERSIONS` combine as a union
V2 fixture: `yanked-version-command-env-union`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Added a command-policy key alongside the environment-policy key, made resolved bzlmod DICE inputs include both policies in equality/hash/stable serialization, and modeled the effective yanked-version policy as their union; no V1 DICE compute, CLI execution path, lockfile writer, registry client, or same-daemon replay logic was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `yanked-version-command-env-union`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-and-repository-graph.md`
Residual risk: actual DICE `Key` wiring, command flag plumbing into evaluation, selected-yanked lockfile writes, and same-daemon command/env replay remain later Stage 5.2/5.6 work

### Stage 5 module-extension recorded-input lockfile substrate

Status: Partially landed
V2 commit: Stage 5 recorded file lockfile checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\lockfile.rs`
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-error-recorded-file` fixture proving `--lockfile_mode=error` rejects stale module extension FILE recorded inputs
V2 fixture: `module-extension-lockfile-error-recorded-file`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the visible recorded-input behavior into V2: `FILE:<label> <digest>` lockfile entries parse into typed recorded-file inputs, unknown recorded input values are preserved as raw JSON, and explicit observed digest maps produce the Bazel-shaped stale recorded-file diagnostic; no V1 lockfile writer, hidden lockfile replay, file hashing, module extension execution, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-extension-lockfile-error-recorded-file`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: real recorded-input digest production, hidden lockfile replay, lockfile update/refresh/error lifecycle, module extension execution, and same-daemon invalidation remain later Stage 5.6 work

### Stage 5 module-extension recorded-environment lockfile substrate

Status: Partially landed
V2 commit: Stage 5 recorded environment lockfile checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\lockfile.rs` for recorded-input orientation only
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-error-recorded-env` fixture proving `--lockfile_mode=error` rejects stale module extension ENV recorded inputs
V2 fixture: `module-extension-lockfile-error-recorded-env`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only the visible recorded-environment behavior into V2: `ENV:<name> <value>` lockfile entries parse into typed recorded-env inputs, unknown recorded input values remain raw JSON, and explicit observed environment maps produce the Bazel-shaped stale-env diagnostic; no V1 process environment plumbing, lockfile writer, hidden lockfile replay, module extension execution, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `module-extension-lockfile-error-recorded-env`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: real process-environment capture into DICE keys, hidden lockfile replay, lockfile update/refresh/error lifecycle, module extension execution, and same-daemon invalidation remain later Stage 5.6 work

### Stage 5 visible lockfile version substrate

Status: Partially landed
V2 commit: Stage 5 lockfile version checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: `C:\tmp\kuro-v1-archive\app\slug_bzlmod\src\lockfile.rs` for lockfile-surface orientation only
Bazel oracle: Bazel 9.1.1 `lockfile-version-error` fixture proving `--lockfile_mode=error` rejects unsupported visible lockfile versions with query exit code 48
V2 fixture: `lockfile-version-error`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools.v2_oracle --update-expected`
Implementation summary: Rewrote only visible lockfile version validation into V2: `lockFileVersion` 26 is exposed as the Bazel 9 supported version and mismatches produce the Bazel-shaped unsupported-version diagnostic; no V1 lockfile writer, hidden lockfile replay, mode state machine, DICE key, or materializer was imported
Validation: `cargo test -p slug_bzlmod_v2`; Bazel oracle run for `lockfile-version-error`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep recorded in `05-bzlmod-checkpoint-evidence.md`
Residual risk: actual lockfile read/update/refresh/error lifecycle, hidden lockfiles, atomic writes, version migration policy, and same-daemon invalidation remain later Stage 5.6 work

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
landed validation command in the owning stage plan.

Doc-only validation:

```bash
git diff --check -- thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md
```

## Landed Evidence

### Stage 6 depset/provider/rule context tests

Status: Landed
V2 commit: `9e519f97`, `ed636308`, `aa9b820f`
V1 source inspected: `app/slug_build_api_tests/src/interpreter/rule_defs/depset.rs`, `app/slug_build_api_tests/src/interpreter/rule_defs/provider/collection.rs`
Bazel oracle: Bazel 9.1 depset/provider probe expectations captured in the V1 tests plus V2 oracle fixture scaffolds
V2 fixture: `depset-orders-and-rejections`, `custom-rule-analysis-basic`, `ctx-attrs-files-executable`, `default-info-runfiles-executable`, `provider-output-group-basic`
Expected evidence artifact: Stage 1 oracle expected output remains placeholder until V2 configured-target analysis can execute fixtures
Implementation summary: Rewrote behavior into V2 depset/provider/context substrates without importing V1 Buck labels, `transitive_set` coercions, or direct-local assumptions
Validation: `cargo test -p slug_build_api_v2`; `cargo test -p slug_analysis_v2`; `py -3 -B tools/v2_oracle list`; Stage 6 shortcut grep recorded in `06-analysis-toolchains-and-actions.md`
Residual risk: Starlark evaluator integration and Bazel-generated oracle outputs are still pending

### Stage 8 public ruleset fixtures

Status: Partially landed
V2 commit: public-ruleset fixture checkpoint containing this entry
V1 source inspected: `tests/plan34/fixtures/rules_cc/MODULE.bazel`, `tests/plan34/fixtures/rules_cc/BUILD.bazel`, `tests/plan34/fixtures/rules_cc/hello.c`
Bazel oracle: Bazel 9.1.1 with BCR metadata for current public ruleset module versions
V2 fixture: `rules-cc-basic`, `rules-cc-run-env`, `rules-cc-test-env-inherit`, `bazel-skylib-basic`, `rules-python-basic`, `rules-python-runfiles`, `protobuf-basic`, `rules-rust-basic`, `rules-oci-basic-no-daemon`
Expected evidence artifact: Stage 1 oracle expected output generated by `tools/v2_oracle --update-expected` for each fixture
Implementation summary: Rewrote Plan34 `rules_cc` behavior into Bazel 9 bzlmod fixtures and added fresh skylib/python/protobuf/rules_rust/rules_oci public ruleset fixtures, including C++ run/test environment and Python runfiles coverage, without importing V1 execution or output-root assumptions
Validation: `py -3 -B -m tools.v2_oracle run --fixture rules-cc-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`; same command for `rules-cc-run-env`, `rules-cc-test-env-inherit`, `bazel-skylib-basic`, `rules-python-basic`, `rules-python-runfiles`, `protobuf-basic`, `rules-rust-basic`, and `rules-oci-basic-no-daemon`
Residual risk: rules_oci full no-daemon image/package build still needs a Linux-backed oracle or upstream Windows wrapper fix; output/runfiles comparisons need platform-aware oracle manifests before upgrading beyond message-shape checks
