# Stage 5: Bzlmod Checkpoint Evidence, Part 2

This companion file continues detailed landed evidence for
[05-bzlmod-and-repository-graph.md](./05-bzlmod-and-repository-graph.md).

Use this file for new Stage 5 checkpoint entries after
`c65dedee Stage 5 preserve registry module digests`. The first evidence shard
is [05-bzlmod-checkpoint-evidence.md](./05-bzlmod-checkpoint-evidence.md).
Keep each evidence shard below 1000 lines.

## Checkpoint Evidence

### Stage 5 registry source.json digest substrate

Status: Partially landed
V2 commit: `b9f55c4f Stage 5 key registry source specs`
V1 source inspected: None for implementation; existing Stage 5 source.json parser behavior remains grounded by the prior V1-reference ledger entry
Bazel oracle: Bazel 9.1.1 `registry-source-json-policy` fixture using a workspace-local registry and `bazel mod graph`
V2 fixture: `registry-source-json-policy`
Expected evidence artifact: Existing Stage 1 oracle expected output for valid, missing-url, missing-integrity, and invalid-json source.json cases
Implementation summary: Added explicit registry `source.json` digest identity beside registry MODULE.bazel digests: `BzlmodRegistrySourceSpecDigest`, deterministic `digest_registry_source_specs`, ordered source-spec selection that preserves first-registry wins plus source.json content digests, selected-source digest aggregation, and a `registry_sources=` component in `BzlmodDiceInputs`; no downloader, archive extractor, patch applier, repository materializer, lockfile writer, or V1 registry client was imported
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture registry-source-json-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: `source.json` digests are now modeled for semantic invalidation, but fetch/extract/patch behavior, registry hash enforcement, visible/hidden lockfile replay, and repository materialization remain later Stage 5.2/5.5/5.6 work

### Stage 5 registry hash URL derivation substrate

Status: Partially landed
V2 commit: `8ec578af Stage 5 derive registry hash urls`
V1 source inspected: None for implementation; URL shape is grounded by existing Bazel 9 lockfile registry-hash oracle fixtures and visible lockfile output
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-registry-hash` and `lockfile-error-missing-registry-hash` fixtures
V2 fixture: `lockfile-error-mode-registry-hash`, `lockfile-error-missing-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output proving Bazel reports full `https://bcr.bazel.build/modules/<name>/<version>/MODULE.bazel` registry file URLs, plus missing-checksum error text
Implementation summary: Added Bazel-shaped registry file URL helpers for `bazel_registry.json`, registry MODULE.bazel, and registry source.json, plus deterministic selected-registry hash URL derivation across selected MODULE and source specs; this feeds existing lockfile registry hash validators without adding network fetches, cache lookup, lockfile writes, or materialization behavior
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-missing-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The helper derives the registry file identities that error-mode validation requires, but actual registry fetching, observed hash production, visible lockfile updating, local-registry refresh semantics, and same-daemon stale rejection remain later Stage 5.2/5.6 work

### Stage 5 observed registry hash map substrate

Status: Partially landed
V2 commit: `c2dbdce4 Stage 5 map observed registry hashes`
V1 source inspected: None for implementation; derived from existing V2 registry digest substrates and Bazel 9 lockfile registry-hash oracle fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-registry-hash` and `lockfile-error-missing-registry-hash` fixtures
V2 fixture: `lockfile-error-mode-registry-hash`, `lockfile-error-missing-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output for lockfile error-mode registry hash validation and missing registry hash diagnostics
Implementation summary: Added `observed_registry_file_hashes` to convert selected registry MODULE.bazel and source.json content digests into the URL-to-digest map consumed by visible lockfile registry-hash validators; the helper requires explicit observed digests and does not add network fetching, filesystem registry scans, cache lookup, lockfile writes, or repository materialization
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-missing-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Observed registry digests are now shaped for lockfile validators, but producing those digests from actual registry fetches or local-registry reads, visible lockfile updating, refresh/error mode lifecycle, and same-daemon stale rejection remain later Stage 5.2/5.6 work

### Stage 5 visible lockfile renderer substrate

Status: Partially landed
V2 commit: `c388e402 Stage 5 render visible lockfiles`
V1 source inspected: None for implementation; renderer shape is grounded by Bazel 9 visible lockfile oracle fixtures and existing V2 lockfile parser structs
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-selected-yanked-version`, and `module-extension-lockfile-shape` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-selected-yanked-version`, `module-extension-lockfile-shape`
Expected evidence artifact: Stage 1 oracle expected output and run artifacts proving top-level visible lockfile field order, selected yanked version entries, and module extension replay fields (`bzlTransitiveDigest`, `usagesDigest`, `recordedInputs`, `generatedRepoSpecs`)
Implementation summary: Added `render_bazel_lockfile` for deterministic Bazel-shaped visible lockfile JSON over the fields V2 already parses and validates: registry file hashes, selected yanked versions, module extension replay data, facts, and factsVersions; this is a pure renderer and does not add filesystem write policy, hidden lockfile caching, registry refresh, repository materialization, or same-daemon invalidation behavior
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-shape --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-selected-yanked-version --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 can now render parsed lockfile data deterministically, but deciding when to read/write visible or hidden lockfiles, updating registry hashes from fetches, replaying stale entries in error mode, and preserving same-daemon invalidation semantics remain later Stage 5.5/5.6 work

### Stage 5 visible lockfile mode planner substrate

Status: Partially landed
V2 commit: `9d712ff2 Stage 5 plan visible lockfile writes`
V1 source inspected: None for implementation; planner behavior is grounded by Bazel 9 lockfile-mode oracle fixtures and existing V2 `LockfileMode` policy methods
Bazel oracle: Bazel 9.1.1 `lockfile-mode-off`, `lockfile-mode-update-refresh`, `lockfile-mode-flag-validation`, and `lockfile-version-error` fixtures
V2 fixture: `lockfile-mode-off`, `lockfile-mode-update-refresh`, `lockfile-mode-flag-validation`, `lockfile-version-error`
Expected evidence artifact: Stage 1 oracle expected output proving `off` does not write, `update`/`refresh` write or preserve visible lockfiles, accepted mode names and invalid-mode diagnostics, and unsupported lockfile-version diagnostics in error mode
Implementation summary: Added `VisibleLockfilePlan` and `plan_visible_lockfile` to choose ignore/keep/write/error for visible lockfile content using the deterministic renderer and parsed-lockfile equality; the planner is pure data flow and does not perform filesystem IO, hidden lockfile cache updates, registry refresh, or same-daemon invalidation
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-off --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-flag-validation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-version-error --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 can now decide visible lockfile content actions without touching the filesystem, but actual read/write integration, hidden lockfile persistence, registry refresh/re-fetch policy, action on stale registry hashes, and same-daemon invalidation remain later Stage 5.5/5.6 work

### Stage 5 registry index observed hash substrate

Status: Partially landed
V2 commit: `ed7ebb23 Stage 5 map registry index hashes`
V1 source inspected: None for implementation; derived from existing V2 registry policy digest identity and Bazel 9 visible lockfile registry hash artifacts
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh` and `lockfile-error-mode-registry-hash` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-error-mode-registry-hash`
Expected evidence artifact: Stage 1 oracle run artifacts showing `registryFileHashes` entries for `https://bcr.bazel.build/bazel_registry.json` alongside registry MODULE.bazel and source.json entries
Implementation summary: Added `observed_registry_policy_file_hashes` to convert ordered registry policy content digests into observed `bazel_registry.json` URL-to-digest entries for visible lockfile validators; the helper reuses the existing registry URL canonicalization and conflict checks without adding network fetching, registry cache lookup, or lockfile writes
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Observed registry index hashes can now feed validation, but producing the policy digest from actual fetched `bazel_registry.json`, local-registry refresh semantics, hidden lockfile persistence, and same-daemon stale rejection remain later Stage 5.2/5.6 work

### Stage 5 fetched registry content snapshot substrate

Status: Partially landed
V2 commit: `f0483107 Stage 5 snapshot fetched registry contents`
V1 source inspected: None for implementation; derived from existing V2 registry parsing/digest substrates and Bazel 9 local-registry fixtures
Bazel oracle: Bazel 9.1.1 `registry-source-json-policy`, `module-registry-mvs-basic`, and `lockfile-mode-update-refresh` fixtures
V2 fixture: `registry-source-json-policy`, `module-registry-mvs-basic`, `lockfile-mode-update-refresh`
Expected evidence artifact: Stage 1 oracle expected output proving registry MODULE/source parsing, local registry MVS, and visible lockfile registryFileHashes including `bazel_registry.json`
Implementation summary: Added `RegistryContentSnapshot` and `snapshot_registry_contents` to turn already-fetched registry index, MODULE.bazel, and source.json contents into parsed catalogs plus observed lockfile hashes; it validates registry index JSON and MODULE path/header agreement without adding filesystem IO, network fetching, cache lookup, hidden lockfile state, or repository materialization
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture registry-source-json-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture module-registry-mvs-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Actual registry client IO, watched input production, refresh re-fetch policy, hidden lockfile persistence, and same-daemon invalidation remain later Stage 5.2/5.6 work

### Stage 5 registry snapshot aggregate DICE digest substrate

Status: Partially landed
V2 commit: `25dfd823 Stage 5 aggregate registry snapshot digests`
V1 source inspected: None for implementation; this extends the V2 fetched-content snapshot around existing DICE registry digest identities
Bazel oracle: Bazel 9.1.1 `registry-source-json-policy`, `module-registry-mvs-basic`, and `lockfile-mode-update-refresh` fixtures
V2 fixture: `registry-source-json-policy`, `module-registry-mvs-basic`, `lockfile-mode-update-refresh`
Expected evidence artifact: Stage 1 oracle expected output proving registry source parsing, selected registry module graph behavior, and visible lockfile registryFileHashes consumed by the aggregate digest path
Implementation summary: Added `RegistryContentDigests` plus `RegistryContentSnapshot::dice_input_digests` so already-fetched registry index, MODULE.bazel, and source.json contents produce the three aggregate registry policy/module/source digests expected by `BzlmodDiceInputs`; the method remains pure data flow and adds no registry IO, cache lookup, hidden lockfile state, or repository materialization
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture registry-source-json-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture module-registry-mvs-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The aggregate digests are ready for graph-key plumbing, but actual registry client IO, watched input recording, refresh re-fetch policy, hidden lockfile persistence, and same-daemon invalidation remain later Stage 5.2/5.6 work

### Stage 5 repo mapping canonical names oracle fixture

Status: Partially landed
V2 commit: `5793958a Stage 5 add repo mapping oracle fixture`
V1 source inspected: None for implementation; this is a fresh Bazel 9 oracle fixture for the Stage 5 exact repo-mapping criterion
Bazel oracle: Bazel 9.1.1 `repo-mapping-canonical-names` fixture using `bazel mod dump_repo_mapping --output=json`
V2 fixture: `repo-mapping-canonical-names`
Expected evidence artifact: Stage 1 oracle expected output proving dependency repo mappings (`aaa+`, `ccc+`), multiple-version canonical repos (`bbb+1.0.0`, `bbb+2.0.0`), and extension-generated repo mapping (`+ext+generated`) including the root apparent mapping entries Bazel emits
Implementation summary: Added a self-contained local-registry plus module-extension oracle fixture that captures Bazel repo mapping JSON for registry dependencies, multiple selected module versions, and an extension-generated repository; no V2 resolver code, V1 repo mapping implementation, process-global registry, or materialization behavior was imported in this checkpoint
Validation: `py -3 -B -m tools.v2_oracle list`; `py -3 -B -m tools.v2_oracle run --fixture repo-mapping-canonical-names --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120 --update-expected`; same command without `--update-expected`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; fixture legacy-surface scan and diff checks before commit
Residual risk: V2 still needs resolver-side repo mapping comparison against this oracle, generated repo mapping DICE ownership, root mapping coverage outside `dump_repo_mapping` argument limitations, and same-daemon mapping invalidation for extension repo changes

### Stage 5 resolver-side Bazel repo mapping substrate

Status: Partially landed
V2 commit: `89e27a6e Stage 5 map Bazel repo mappings`
V1 source inspected: None for implementation; derived from the V2 resolver graph and the fresh Bazel 9 `repo-mapping-canonical-names` oracle fixture
Bazel oracle: Bazel 9.1.1 `repo-mapping-canonical-names` fixture using `bazel mod dump_repo_mapping --output=json`
V2 fixture: `repo-mapping-canonical-names`
Expected evidence artifact: Stage 1 oracle expected output proving module repo mappings include self apparent names, dependency apparent mappings, `bazel_tools`, multiple-version canonical repo names, and extension-generated repo mappings inheriting root apparent mappings
Implementation summary: Added `ResolvedGraph::bazel_repo_mapping_for` and `ResolvedGraph::extension_generated_repo_mapping` to derive Bazel-shaped mapping content from the V2 resolved graph; the older dependency-only `repo_mapping_for` remains available, and this slice adds no extension execution, repository materialization, V1 repo mapping code, process-global registry, or hidden state
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture repo-mapping-canonical-names --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Generated repository mapping still needs real module-extension execution ownership, root mapping coverage is currently inferred from generated-repo mappings because `dump_repo_mapping` rejects root repo arguments, and same-daemon mapping invalidation remains later Stage 5.4/5.8 work

### Stage 5 repo mapping oracle normalization substrate

Status: Partially landed
V2 commit: `60eb87b3 Stage 5 normalize repo mapping oracle output`
V1 source inspected: None for implementation; derived from the Bazel 9 `repo-mapping-canonical-names` oracle output shape and the V2 resolver mapping substrate
Bazel oracle: Bazel 9.1.1 `repo-mapping-canonical-names` fixture using `bazel mod dump_repo_mapping --output=json`
V2 fixture: `repo-mapping-canonical-names`
Expected evidence artifact: Stage 1 oracle expected JSON lines for dependency, multiple-version, and extension-generated repo mappings
Implementation summary: Added `parse_bazel_dump_repo_mapping_json_lines` to normalize Bazel `dump_repo_mapping` JSON-line output into deterministic string maps, and updated the registry MVS repo-mapping test to compare V2-derived mappings against the normalized Bazel oracle shape; this remains a comparison substrate and does not execute module extensions, materialize repositories, or import V1 repo mapping code
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture repo-mapping-canonical-names --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Full byte-for-byte repo mapping comparison still needs integration into an executable V2 command/oracle path, generated repo mapping still needs real extension execution ownership, and same-daemon mapping invalidation remains later Stage 5.4/5.8 work

### Stage 5 visible lockfile apply substrate

Status: Partially landed
V2 commit: `210216f1 Stage 5 apply visible lockfile plans`
V1 source inspected: None for implementation; the write boundary is derived from the existing V2 visible lockfile planner and Bazel 9 lockfile-mode oracle fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh` and `lockfile-mode-off` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`
Expected evidence artifact: Stage 1 oracle expected output proving `update`/`refresh` visible lockfile writes and `off` leaves `MODULE.bazel.lock` absent
Implementation summary: Added `apply_visible_lockfile_plan` to apply an already-computed `VisibleLockfilePlan`: ignore/keep/error plans perform no filesystem writes, and write plans publish content through a same-directory temporary file before persisting to `MODULE.bazel.lock`; the helper does not read lockfile content, select policy, compute digests, update hidden lockfiles, or discover semantic inputs
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-off --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 now has a narrow visible-lockfile apply boundary, but actual lockfile read integration, DICE-owned lockfile digest production, hidden lockfile persistence, registry refresh re-fetches, directory durability policy, and same-daemon stale rejection remain later Stage 5.5/5.6 work

### Stage 5 visible lockfile digest substrate

Status: Partially landed
V2 commit: `9046145d Stage 5 model visible lockfile digests`
V1 source inspected: None for implementation; this is V2-owned DICE key plumbing derived from the existing visible-lockfile mode/update/error fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh` and `lockfile-mode-off` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`
Expected evidence artifact: Stage 1 oracle expected output proving visible lockfile presence/absence under Bazel lockfile modes
Implementation summary: Added `BzlmodVisibleLockfileDigest` so resolved-graph DICE inputs can distinguish absent visible lockfiles from present lockfile content digests, and updated DICE-input tests to prove lockfile content changes affect key identity; this does not read the filesystem, compute hidden lockfile state, write lockfiles, or decide mode policy
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-off --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The digest token is ready for DICE keys, but actual visible lockfile reads, hidden lockfile persistence, refresh/error lifecycle integration, registry re-fetch policy, and same-daemon stale rejection remain later Stage 5.5/5.6 work

### Stage 5 visible lockfile input bridge

Status: Partially landed
V2 commit: `b24227cb Stage 5 bridge visible lockfile inputs`
V1 source inspected: None for implementation; this is V2-owned input plumbing derived from the existing visible lockfile planner, digest token, and Bazel 9 lockfile-mode fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh` and `lockfile-mode-off` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`
Expected evidence artifact: Stage 1 oracle expected output proving visible `MODULE.bazel.lock` presence/absence and update/refresh/off behavior under Bazel lockfile modes
Implementation summary: Added `VisibleLockfileInput` so future DICE file-dependency reads can hand optional visible lockfile bytes to the planner as an absent/present digest plus optional UTF-8 content; the helper performs no filesystem read, hidden lockfile lookup, mode selection, lockfile rendering, or write
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-off --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The bridge is ready for DICE-owned file reads, but actual visible lockfile file dependency wiring, hidden lockfile persistence, refresh/error lifecycle integration, registry re-fetch policy, and same-daemon stale rejection remain later Stage 5.5/5.6 work

### Stage 5 module extension replay input aggregate

Status: Partially landed
V2 commit: `70a1ada2 Stage 5 validate module extension replay inputs`
V1 source inspected: None for implementation; this aggregates existing V2 visible lockfile validators that were each grounded by Bazel 9 stale-replay oracle fixtures
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-error-usage`, `module-extension-lockfile-error-bzl`, `module-extension-lockfile-error-recorded-file`, and `module-extension-lockfile-error-recorded-env` fixtures
V2 fixture: `module-extension-lockfile-error-usage`, `module-extension-lockfile-error-bzl`, `module-extension-lockfile-error-recorded-file`, `module-extension-lockfile-error-recorded-env`
Expected evidence artifact: Stage 1 oracle expected output proving error mode rejects stale module-extension usage digests, `.bzl` transitive digests, recorded file inputs, and recorded environment inputs
Implementation summary: Added `ModuleExtensionReplayInputs` and `validate_module_extension_replay_inputs` to validate all currently modeled module-extension replay inputs against parsed visible lockfile data through explicit observed maps; the aggregate does not compute digests, read files, read process environment, execute extensions, consult hidden lockfiles, or materialize generated repositories
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-error-usage --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-extension-lockfile-error-bzl`, `module-extension-lockfile-error-recorded-file`, and `module-extension-lockfile-error-recorded-env`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The aggregate covers only the visible module-extension replay inputs modeled so far; hidden lockfile persistence, extension execution, repo mapping and repository-rule attr replay, OS/arch policy, watched-file digest production, and same-daemon stale rejection remain later Stage 5.4/5.6/5.8 work

### Stage 5 hidden lockfile digest identity

Status: Partially landed
V2 commit: `89fe6aac Stage 5 key hidden lockfile digests`
V1 source inspected: None for implementation; this is V2-owned DICE key plumbing derived from the Stage 5 visible lockfile digest/input work and Bazel 9 lockfile-mode plus module-extension replay fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-mode-off`, and `module-extension-lockfile-error-usage` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`, `module-extension-lockfile-error-usage`
Expected evidence artifact: Stage 1 oracle expected output proving visible lockfile mode behavior and stale module-extension replay behavior that hidden lockfile state will later cache/replay
Implementation summary: Added `BzlmodHiddenLockfileDigest` and an explicit `BzlmodDiceInputs::new_with_hidden_lockfile` constructor so hidden lockfile content can affect resolved-graph DICE identity when a later slice wires the actual file dependency; the existing constructor defaults hidden state to `absent` and no filesystem IO, hidden cache read, lockfile parsing, or write behavior was added
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `lockfile-mode-off` and `module-extension-lockfile-error-usage`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Hidden lockfile content is now keyable, but actual hidden lockfile path ownership, DICE file-dependency reads, cache persistence, replay/write lifecycle, registry re-fetch policy, and same-daemon stale rejection remain later Stage 5.5/5.6/5.8 work

### Stage 5 hidden lockfile input bridge

Status: Partially landed
V2 commit: `8403d66e Stage 5 bridge hidden lockfile inputs`
Bazel source inspected: `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileValue.java` documents the visible workspace lockfile and hidden output-base lockfile split plus hidden lockfile cache semantics; `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileFunction.java` reads both through file dependencies, uses the output base for `HIDDEN_KEY`, parses UTF-8 JSON, and fail-opens hidden parse/read errors to an empty lockfile; `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileModule.java` writes reproducible extension results to the output-base hidden lockfile
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-mode-off`, and `module-extension-lockfile-error-usage` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`, `module-extension-lockfile-error-usage`
Expected evidence artifact: Stage 1 oracle expected output proving visible lockfile modes and stale module-extension usage replay behavior that hidden lockfile state will later cache/replay
Implementation summary: Added `HiddenLockfileInput` so future DICE output-base file reads can hand optional hidden `MODULE.bazel.lock` bytes to replay parsing as an absent/present hidden digest plus optional UTF-8 content; this mirrors the visible input bridge while keeping hidden path discovery, DICE file dependency wiring, hidden parse fail-open policy, persistence, and replay/write lifecycle out of this slice
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `lockfile-mode-off` and `module-extension-lockfile-error-usage`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Hidden lockfile bytes are now bridgeable into V2-owned keys and parsers, but actual hidden lockfile path ownership, DICE file-dependency reads, Bazel-style hidden parse fail-open integration, hidden cache persistence, registry re-fetch policy, and same-daemon stale rejection remain later Stage 5.5/5.6/5.8 work

### Stage 5 hidden lockfile fail-open parsing

Status: Partially landed
V2 commit: `230138a5 Stage 5 model hidden lockfile fail-open parsing`
Bazel source inspected: `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileFunction.java` passes `LockfileMode.UPDATE` for `BazelLockFileValue.HIDDEN_KEY` and returns `EMPTY_LOCKFILE` on hidden lockfile parse/read failures; `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileValue.java` defines the empty lockfile defaults and hidden output-base cache semantics
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-mode-off`, and `module-extension-lockfile-error-usage` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`, `module-extension-lockfile-error-usage`
Expected evidence artifact: Stage 1 oracle expected output proving lockfile mode behavior plus stale module-extension usage replay errors used to ground hidden cache replay work
Implementation summary: Added `empty_bazel_lockfile` and `parse_hidden_lockfile_fail_open`, with `HiddenLockfileInput::parse_fail_open`, so absent, malformed, or stale-version hidden content resolves to an empty Bazel 9 lockfile while current-version hidden content is preserved; this still performs no filesystem read, output-base path discovery, write, or extension execution
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `lockfile-mode-off` and `module-extension-lockfile-error-usage`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 can now model Bazel's hidden parse fail-open boundary once bytes are supplied, but actual DICE output-base reads, IO-error fail-open integration, hidden lockfile persistence, repository-rule attr/repo-mapping replay, and same-daemon stale rejection remain later Stage 5.5/5.6/5.8 work

### Stage 5 module extension lockfile replay oracle

Status: Partially landed
V2 commit: `869795c1 Stage 5 add module extension lockfile replay oracle`
Bazel source inspected: Existing Stage 5 lockfile source citations still apply: `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileFunction.java` reads lockfile values, and `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileModule.java` combines module-extension results into visible/hidden lockfiles after command execution
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-replay` fixture
V2 fixture: `module-extension-lockfile-replay`
Expected evidence artifact: Stage 1 oracle expected output shows the prime command executing the extension sentinel, replay in `--lockfile_mode=error` succeeding without the sentinel, then a tag mutation rejected as stale usageDigest data
Implementation summary: Added the missing exact-criteria oracle fixture for module-extension lockfile replay; this is an oracle-only checkpoint and does not add V2 extension execution, lockfile replay integration, or generated repository materialization
Validation: `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-replay --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120 --update-expected`; same command without `--update-expected`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; `py -3 -B -m tools.v2_oracle list`; diff checks before commit
Residual risk: The oracle now pins Bazel replay behavior, but V2 still needs actual module-extension execution, visible/hidden lockfile replay selection, repository-rule attr and repo-mapping replay validation, hidden persistence, and same-daemon stale rejection wiring

### Stage 5 lockfile error-mode stale oracle

Status: Partially landed
V2 commit: `231add4a Stage 5 add lockfile error-mode stale oracle`
Bazel source inspected: Existing Stage 5 lockfile source citations still apply: `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileFunction.java` reads visible lockfile data according to lockfile mode, and registry hash enforcement remains grounded by Bazel 9.1.1 oracle behavior
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-stale` fixture
V2 fixture: `lockfile-error-mode-stale`
Expected evidence artifact: Stage 1 oracle expected output proves `--lockfile_mode=error` rejects a visible `MODULE.bazel.lock` with a stale BCR registry file checksum instead of refreshing it
Implementation summary: Added the missing exact-criteria fixture name for error-mode stale lockfiles, using the Bazel-observed registry checksum rejection; a local-registry module edit was probed first and Bazel 9 did not reject it in error mode, so that non-error behavior was not committed as a stale fixture
Validation: `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-stale --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120 --update-expected`; same command without `--update-expected`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; `py -3 -B -m tools.v2_oracle list`; diff checks before commit
Residual risk: The exact stale fixture is now present, but V2 still needs actual error-mode integration across visible/hidden lockfile reads, registry fetch policy, extension replay data, repository-rule attr/repo-mapping replay, and same-daemon invalidation

### Stage 5 lockfile read input boundary

Status: Partially landed
V2 commit: `b2942561 Stage 5 model lockfile read inputs`
Bazel source inspected: `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileFunction.java` reads visible lockfiles according to `LOCKFILE_MODE`, treats missing files as empty lockfiles, ignores old-version visible lockfiles except in `ERROR`, throws visible parse errors, and parses hidden lockfiles with update/fail-open behavior; `C:\dev\bazel\src\main\java\com\google\devtools\build\lib\bazel\bzlmod\BazelLockFileValue.java` defines the empty lockfile defaults and visible/hidden key split
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-mode-off`, `lockfile-error-mode-stale`, and `module-extension-lockfile-replay` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-mode-off`, `lockfile-error-mode-stale`, `module-extension-lockfile-replay`
Expected evidence artifact: Stage 1 oracle expected output proves mode-specific visible behavior, stale registry-hash rejection, and extension replay/no-reexecution behavior that later DICE reads must preserve
Implementation summary: Added `VisibleLockfileRead`, `parse_visible_lockfile_for_mode`, `LockfileReadInputs`, and `LockfileReadSnapshot` so future DICE file-dependency bytes can be converted into Bazel-shaped visible/hidden lockfile state; this still performs no filesystem read, output-base path discovery, lockfile write, registry fetch, extension execution, or generated repository materialization
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `lockfile-mode-off`, `lockfile-error-mode-stale`, and `module-extension-lockfile-replay`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The read boundary is ready for DICE-owned bytes, but V2 still needs actual visible/hidden file reads, IO error handling, registry refresh policy, extension replay selection, repo-rule attr/repo-mapping replay validation, hidden persistence, and same-daemon invalidation

### Stage 5 deterministic lockfile rendering regression

Status: Partially landed
V2 commit: `5a28a42e Stage 5 test deterministic lockfile rendering`
Bazel source inspected: Existing Stage 5 lockfile source citations still apply; this checkpoint strengthens the V2 rendering regression around the Bazel-shaped lockfile data model rather than adding new source extraction
Bazel oracle: Bazel 9.1.1 `lockfile-mode-update-refresh`, `lockfile-error-mode-stale`, and `module-extension-lockfile-replay` fixtures
V2 fixture: `lockfile-mode-update-refresh`, `lockfile-error-mode-stale`, `module-extension-lockfile-replay`
Expected evidence artifact: Stage 1 oracle expected output proving mode-specific lockfile writes, stale registry checksum rejection, and module-extension replay/no-reexecution behavior
Implementation summary: Added a regression that parses two semantically identical lockfiles with reversed object order across registry hashes, selected yanked versions, module extensions, generated repositories, and repo rule attributes, then proves `render_bazel_lockfile` produces identical sorted output; no parser, filesystem, DICE, registry fetch, or extension execution behavior changed
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-update-refresh --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `lockfile-error-mode-stale` and `module-extension-lockfile-replay`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The regression pins deterministic rendering for the currently modeled visible lockfile fields only; actual visible/hidden file IO, hidden persistence, repository-rule attr/repo-mapping replay validation, registry refresh policy, extension execution, and same-daemon invalidation remain later Stage 5 work

### Stage 5 generated repo spec replay validation

Status: Partially landed
V2 commit: `07b0a02b Stage 5 validate generated repo specs`
Bazel source inspected: Existing Stage 5 lockfile source citations still apply: `BazelLockFileValue.java` owns the generated repo spec schema and `BazelLockFileFunction.java` owns visible lockfile replay/error-mode reads; this checkpoint keeps the comparison at the V2 replay-boundary layer without executing repository rules
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-replay` and `module-extension-lockfile-error-usage` fixtures
V2 fixture: `module-extension-lockfile-replay`, `module-extension-lockfile-error-usage`
Expected evidence artifact: Stage 1 oracle expected output proves module-extension replay without re-execution and stale usage rejection; the new V2 unit coverage extends the replay aggregate to compare parsed generated repo specs against explicit observed extension outputs
Implementation summary: Added generated repo specs to `ModuleExtensionReplayInputs`, exported and wired `validate_module_extension_generated_repo_specs`, and covered matching/stale repository rule ids and attributes through focused lockfile tests; this adds no filesystem IO, repository rule execution, repo materialization, hidden lockfile persistence, or DICE file dependency wiring
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-replay --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-extension-lockfile-error-usage`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 now has an explicit replay comparison for generated repository specs, but producing observed specs from actual extension execution, repository-rule attr/repo-mapping digests, hidden lockfile persistence, materialization, registry refresh policy, and same-daemon stale rejection remain later Stage 5 work
