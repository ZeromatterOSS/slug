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

### Stage 5 generated repo spec DICE identity

Status: Partially landed
V2 commit: `2d224a04 Stage 5 key generated repo specs`
Bazel source inspected: Existing generated-repo lockfile citations still apply; `docs/developers/dice.md` was reread before editing DICE-owned key state and this checkpoint follows the explicit-key/no-global-cache ownership rule
Bazel oracle: Bazel 9.1.1 `module-extension-lockfile-replay` and `module-extension-lockfile-error-usage` fixtures
V2 fixture: `module-extension-lockfile-replay`, `module-extension-lockfile-error-usage`
Expected evidence artifact: Stage 1 oracle expected output proving generated repository replay is part of module-extension lockfile behavior and stale usage still rejects before silent replay
Implementation summary: Added `BzlmodGeneratedRepoSpecDigest`, `digest_generated_repo_specs`, and an explicit generated-repo-spec digest field in `BzlmodDiceInputs`; existing constructors default this input to the deterministic empty digest, while `new_with_generated_repo_specs` and `new_with_hidden_lockfile_and_generated_repo_specs` let later extension/repository-rule execution wire real generated repo spec identity without process-global state
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-replay --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-extension-lockfile-error-usage`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Generated repo spec identity is now keyable, but V2 still needs actual extension execution to produce those digests, repository-rule attr and repo-mapping digest production, hidden lockfile persistence, materialization, registry refresh policy, and same-daemon invalidation

### Stage 5 repo mapping DICE identity

Status: Partially landed
V2 commit: `a81881eb Stage 5 key repo mappings`
Bazel source inspected: Existing Stage 5 repo-mapping citations still apply; `docs/developers/dice.md` was reread before editing DICE-owned key state so repo-mapping identity stays explicit in the key instead of process-global state
Bazel oracle: Bazel 9.1.1 `repo-mapping-canonical-names` fixture using `bazel mod dump_repo_mapping --output=json`
V2 fixture: `repo-mapping-canonical-names`
Expected evidence artifact: Stage 1 oracle expected output proving root, dependency, generated repository, and multiple-version repository mappings after normalization
Implementation summary: Added `BzlmodRepoMappingDigest`, deterministic repo-mapping digest helpers, and an explicit `repo_mapping_digest` field in `BzlmodDiceInputs`; existing constructors default to the deterministic empty mapping digest while `new_with_repo_mappings` and the combined hidden/generated/repo constructor let later resolver and extension execution wire real mapping identity without process-global state
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture repo-mapping-canonical-names --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Repo mapping identity is now keyable, but V2 still needs actual DICE producers from resolver and extension execution, stale lockfile repo-mapping rejection, materialization, and same-daemon mapping invalidation.

### Stage 5 repo mapping digest producer bridge

Status: Partially landed
V2 commit: `1b3ed5a6 Stage 5 produce repo mapping digests`
Bazel source inspected: Existing Stage 5 repo-mapping citations still apply; this checkpoint wires the V2 resolver-side mapping substrate to the DICE digest helpers landed in `a81881eb`
Bazel oracle: Bazel 9.1.1 `repo-mapping-canonical-names` fixture using `bazel mod dump_repo_mapping --output=json`
V2 fixture: `repo-mapping-canonical-names`
Expected evidence artifact: Stage 1 oracle expected output proving Bazel-shaped root, dependency, multiple-version, and extension-generated repo mapping content
Implementation summary: Added `ResolvedGraph::module_repo_mapping_digests`, `ResolvedGraph::module_repo_mapping_digest`, and `ResolvedGraph::extension_generated_repo_mapping_digest` so resolved module graphs can produce the explicit repo-mapping identity expected by `BzlmodDiceInputs`; extension-generated digests are still helper-only until actual module-extension execution produces generated repositories
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture repo-mapping-canonical-names --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Resolver-produced module repo mappings can now feed DICE identity, but V2 still needs actual extension execution to produce generated repo mappings, visible/hidden lockfile replay validation for stale mappings, materialization, and same-daemon invalidation wiring.

### Stage 5 module directive oracle expansion

Status: Partially landed
V2 commit: `dc2bf418 Stage 5 expand module directive oracle`
Bazel source inspected: Existing Stage 5 module-file evaluation citations still apply; this is an oracle-only fixture expansion anchored by Bazel 9.1.1 behavior
Bazel oracle: Bazel 9.1.1 `module-file-directives` fixture
V2 fixture: `module-file-directives`
Expected evidence artifact: Stage 1 oracle expected output now covers local override build, direct repository creation through `use_repo_rule`, and `override_repo` mapping `@generated` to the direct repo while the MODULE file also contains `include`, `use_extension`, `use_repo`, `inject_repo`, root dev-dependency flags, and registration directives
Implementation summary: Expanded the local-only module directive fixture with `repo.bzl` and `ext.bzl`, regenerated Bazel expected output, and kept this checkpoint oracle-only; no parser, resolver, extension execution, repository rule execution, or materialization code changed
Validation: `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-file-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120 --update-expected`; same command without `--update-expected`; `py -3 -B -m tools.v2_oracle list`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; diff checks before commit
Residual risk: The fixture now names the directive surface more completely, but V2 still needs evaluator validation for root/non-root dev-dependency behavior, include restrictions, override/inject execution semantics, registration order feeding toolchain/platform resolution, and same-daemon invalidation.

### Stage 5 MODULE include expansion substrate

Status: Partially landed
V2 commit: `62da47d6 Stage 5 expand MODULE include fragments`
Bazel source inspected: Existing Stage 5 `ModuleFileFunction.java` / `ModuleFileGlobals.java` citations still apply; this checkpoint is anchored by the local Bazel 9 include fixtures and keeps filesystem reads outside the helper
Bazel oracle: Bazel 9.1.1 `module-file-directives` and `module-include-change-invalidation` fixtures
V2 fixture: `module-file-directives`, `module-include-change-invalidation`
Expected evidence artifact: Stage 1 oracle expected output proving included MODULE fragments affect dependency/local-override behavior and same-output-base rebuilds observe included-fragment edits
Implementation summary: Added `expand_included_module_files` to splice already-parsed root-package include fragments into directive order, with missing include, unsupported label shape, module-header-in-fragment, and cycle diagnostics; this performs no filesystem IO and expects later DICE-owned readers to supply included module contents and digests
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-file-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-include-change-invalidation`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Include fragments can now be expanded once supplied, but V2 still needs DICE file-dependency reads, Bazel-exact include restriction diagnostics, root/non-root directive validation, same-daemon create/edit/delete invalidation wiring, and integration into registry/local resolution.

### Stage 5 include-aware local graph resolution

Status: Partially landed
V2 commit: `6fd68e2d Stage 5 resolve local graph includes`
Bazel source inspected: Existing Stage 5 include/module-file citations still apply; this checkpoint composes the V2 include-expansion helper with the existing local-resolution substrate without adding filesystem IO
Bazel oracle: Bazel 9.1.1 `module-include-change-invalidation` fixture
V2 fixture: `module-include-change-invalidation`
Expected evidence artifact: Stage 1 oracle expected output proving included fragment edits change the selected local override in the same output base
Implementation summary: Added `resolve_local_module_graph_with_includes` so already-parsed include fragments can be spliced before local graph resolution; the original `resolve_local_module_graph` remains available for pre-expanded/no-include callers
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-include-change-invalidation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Local graph resolution can now consume supplied include fragments, but V2 still needs actual DICE file reads/watch edges, Bazel-exact include diagnostics, non-root module/include validation, and same-daemon create/edit/delete invalidation wiring.

### Stage 5 module registration extraction substrate

Status: Partially landed
V2 commit: `94319719 Stage 5 extract module registrations`
Bazel source inspected: Existing Stage 5 module-file directive citations still apply; this checkpoint is anchored by the Bazel 9 registration fixture and does not execute toolchain or platform resolution
Bazel oracle: Bazel 9.1.1 `module-registration-dev-dependency` fixture
V2 fixture: `module-registration-dev-dependency`
Expected evidence artifact: Stage 1 oracle expected output proving `register_toolchains` and `register_execution_platforms` accept `dev_dependency` flags while preserving normal package loading for the registered labels
Implementation summary: Added `RegistrationKind`, `ModuleRegistrationDirective`, and `module_registration_directives` so parsed MODULE files can produce an order-preserving registration list with labels and `dev_dependency` state intact; this remains parser-side substrate and adds no toolchain resolution, repository materialization, process-global state, or V1 resolver behavior
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registration-dev-dependency --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Registration directives can now be extracted in Bazel-observed order, but V2 still needs root/non-root dev-dependency filtering, DICE-owned module file reads, wiring into Stage 6 toolchain/platform resolution, and same-daemon invalidation for registration edits.

### Stage 5 dev dependency visibility substrate

Status: Partially landed
V2 commit: `edab8efd Stage 5 model dev dependency visibility`
Bazel source inspected: Existing Stage 5 module-file directive citations still apply; this checkpoint is anchored by local Bazel 9 dev-dependency visibility fixtures
Bazel oracle: Bazel 9.1.1 `module-root-dev-dependency-visibility` and `module-nonroot-dev-dependency-visibility` fixtures
V2 fixture: `module-root-dev-dependency-visibility`, `module-nonroot-dev-dependency-visibility`
Expected evidence artifact: Stage 1 oracle expected output proving root `bazel_dep(dev_dependency = True)` is visible by default, hidden under `--ignore_dev_dependency`, and non-root dev dependencies are absent from the dependent module's repository mapping
Implementation summary: Added `DevDependencyMode` and mode-aware local/registry graph entrypoints; default graph resolution includes root dev dependencies while `IgnoreRoot` drops them, and both local and registry substrates ignore non-root dev dependencies before module discovery and repo-mapping construction
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-root-dev-dependency-visibility --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-nonroot-dev-dependency-visibility`; `py -3 -B -m tools.v2_oracle list`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Dev-dependency visibility now matches the observed graph-selection cases, but V2 still needs command-line flag plumbing from the CLI into bzlmod graph keys, exact diagnostics for override-on-nonexistent-module cases, actual DICE file reads/watch edges, and same-daemon invalidation for dev-dependency edits.

### Stage 5 root override target validation

Status: Partially landed
V2 commit: `c8cc47d3 Stage 5 validate root override targets`
Bazel source inspected: Existing Stage 5 override directive citations still apply; this checkpoint is anchored by observed Bazel 9.1.1 behavior for missing root override targets
Bazel oracle: Bazel 9.1.1 `module-override-validation` fixture
V2 fixture: `module-override-validation`
Expected evidence artifact: Stage 1 oracle expected output proving Bazel reports missing `local_path_override`, `single_version_override`, `multiple_version_override`, `archive_override`, and `git_override` targets in directive order before any fetch/materialization
Implementation summary: Added shared root-override target validation after local/registry module discovery so valid transitive overrides remain accepted while overrides for names absent from the discovered module graph produce the Bazel-shaped missing-module diagnostic; known target names include declared deps from discovered module files so ignored dev-dependency edges do not become false missing-override errors
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-override-validation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; `py -3 -B -m tools.v2_oracle list`; Stage 5 guardrail grep and diff checks before commit
Residual risk: V2 still needs exact root/non-root override placement diagnostics, registry archive/git override fetching and source verification, CLI flag plumbing for dev-dependency modes, DICE-owned module file reads, and same-daemon invalidation for override edits.

### Stage 5 registration dev-dependency filtering

Status: Partially landed
V2 commit: `da16ebcc Stage 5 filter module registrations by dev dependency`
Bazel source inspected: Existing Stage 5 module-file directive citations still apply; this checkpoint is anchored by observed Bazel 9.1.1 `--ignore_dev_dependency` behavior for root `register_toolchains`
Bazel oracle: Bazel 9.1.1 `module-registration-dev-dependency` fixture
V2 fixture: `module-registration-dev-dependency`
Expected evidence artifact: Stage 1 oracle expected output proving a root `register_toolchains(..., dev_dependency = True)` toolchain is available by default and removed under `--ignore_dev_dependency`, producing Bazel's toolchain-resolution failure for `//:tc_type`
Implementation summary: Added `ModuleDirectiveOwner` and `active_module_registration_directives` so parsed registration directives can be filtered by root/non-root ownership and `DevDependencyMode`; root dev registrations are included only in `IncludeRoot`, while non-root dev registrations are filtered out even when root dev dependencies are included
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registration-dev-dependency --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; `py -3 -B -m tools.v2_oracle list`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Registration filtering is now explicit substrate only; V2 still needs command-line flag plumbing into bzlmod graph keys, DICE-owned module reads, registration ordering integration with Stage 6 toolchain/platform resolution, and same-daemon invalidation for registration edits.

### Stage 5 ignore-dev-dependency command policy key

Status: Partially landed
V2 commit: `6b323e86 Stage 5 key ignore dev dependency policy`
Bazel source inspected: Existing Stage 5 module-file directive citations still apply; `docs/developers/dice.md` was reread before editing DICE-owned command policy identity
Bazel oracle: Bazel 9.1.1 `module-root-dev-dependency-visibility` and `module-registration-dev-dependency` fixtures
V2 fixture: `module-root-dev-dependency-visibility`, `module-registration-dev-dependency`
Expected evidence artifact: Stage 1 oracle expected output proving `--ignore_dev_dependency` changes root dev-dependency visibility and root dev-only toolchain registration availability
Implementation summary: Extended `BzlmodCommandPolicyKey` with an explicit `ignore_dev_dependency` bit, a `from_flags` constructor, accessor, and stable serialization so resolved bzlmod graph keys can distinguish default root-dev behavior from `--ignore_dev_dependency` runs without process-global command state
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-root-dev-dependency-visibility --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; same command for `module-registration-dev-dependency`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: The key identity is ready, but V2 still needs CLI flag parsing and command plumbing into bzlmod graph construction, DICE compute producers for resolved graphs, and same-daemon invalidation proving flag flips replay the affected graph and registrations.

### Stage 5 root-module DICE core

Status: Accepted
V2 commit: `58e9faa4 feat: add root module dice core`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction.java`, `ModuleFileGlobals.java`, `Version.java`, `LabelValidator.java`, and `RepositoryName.java`
Bazel oracle: Accepted six-fixture runtime-input oracle in `911f16f2`; this packet claims only its root/include and normalized-input owner rows
V2 fixture: owner-local `root_module_dice` plus retained `slug_core_v2` runtime tests
Expected evidence artifact: real starlark-rust module evaluation and DICE invalidation/reuse for raw root/include file values and explicit normalized request inputs, with a typed repository mapping observable before package loading
Implementation summary: Added bzlmod-owned `ModuleFileEvaluationKey` and `RootModuleGraphKey`, fail-closed workspace-scoped injected command/environment/lockfile-mode keys, breadth-first repo-relative includes, Bazel-shaped basic call/validation behavior for `module`, `include`, `bazel_dep`, and `local_path_override`, and immutable root/included declarations plus typed mapping. `WorkspaceRuntime` injects every value on its existing updater, commits once, computes the graph before loading, and returns `Arc<RootModuleGraph>`; production does not call `ModuleFile::parse`.
Validation: `cargo test -p slug_bzlmod_v2` (all owner tests, including 6 root-module DICE tests); `cargo test -p slug_core_v2` (3 unit and 12 runtime tests); `cargo check -p slug_server_v2 -p slug_cli_v2`; `cargo fmt --all -- --check`; `git diff --check`; `scripts/v2_archive_status.sh`; independent final review `ACCEPT`
Residual risk: Command/daemon request transport and loading's mapping dependency remain Packet B; visible lockfile v28, registry/yanked resolution, MVS/extensions, hidden lockfile, fetch/materialization, cquery, and aquery remain later serial packets.

### Stage 5 root-module command/daemon/loading handoff

Status: Accepted
V2 commit: `3f84e34d feat: hand off root module request inputs`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`; this packet consumes the previously accepted request-policy, dev-dependency, and root-repository-mapping behavior without extending the oracle
Bazel oracle: Accepted six-fixture runtime-input oracle in `911f16f2`; this packet claims the command/environment transport and root mapping dependency rows only
V2 fixture: owner-local bzlmod/loading/commands/core/server/CLI tests plus downstream analysis/query transaction coverage
Expected evidence artifact: request-local primitive transport and retained DICE invalidation/reuse for command policy, allowlisted environment, and lockfile mode, with `PackageLoadKey` depending first on the root module graph before listing or BUILD observation
Implementation summary: Added an injection-only bzlmod updater helper, explicit build/query runtime seams, pure environment normalization, one-capture CLI transport, backward-compatible primitive daemon DTOs, and the cycle-free `slug_loading_v2 -> slug_bzlmod_v2` root-graph dependency. Standalone transactions inject explicit values; no defaults, environment reads, filesystem reads, commits, semantic serde, or retained daemon policy occur inside the helper or DICE keys.
Validation: `cargo test -p slug_bzlmod_v2 --test root_module_dice` (7); `cargo test -p slug_loading_v2` (49, including retained PackageLoad A→B→A); `cargo test -p slug_commands_v2` (15); `cargo test -p slug_core_v2` (4 unit and 13 integration after correction); `cargo test -p slug_server_v2` (19); `cargo test -p slug_analysis_v2 --test starlark_rule` (3); `cargo test -p slug_query_v2 --test loading_query` (38); `cargo build -p slug_cli_v2`; focused one-shot/daemon environment and non-Unicode CLI tests; `cargo fmt --all -- --check`; `git diff --check`; daemon cleanup; `scripts/v2_archive_status.sh`; independent final review `ACCEPT`
Residual risk: Visible lockfile v28 observation/replay/write ownership, registry-produced hashes and yanked resolution, MVS/extensions, hidden lockfiles, fetch/materialization, external loading, cquery, and aquery remain later serial packets.

### Stage 5 visible-lockfile v28 DICE read

Status: Accepted
V2 commit: `6d354e10 feat: read visible lockfile through dice`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `BazelLockFileFunction.java` for first-marker scan/read precedence and diagnostics and `BazelLockFileValue.java` for version 28 and semantic EMPTY
Bazel oracle: Accepted `lockfile-mode-update-refresh` and `lockfile-version-error` rows in the six-fixture runtime-input oracle from `911f16f2`; this packet claims read/version behavior only, not produced bytes or exact external-dependency exit 48
V2 fixture: owner-local `lockfile` and `root_module_dice`, loading activation/lifecycle, and retained core runtime tests
Expected evidence artifact: a real workspace-scoped DICE read whose conditional dependency, semantic equality, failure ordering, downstream blocking, and retained recovery match the pinned source
Implementation summary: Added `VisibleLockfileKey` over the injected mode and neutral `WorkspaceFileKey`; `off` returns before acquiring the file dependency, while other modes perform Bazel's first Java-ASCII `"lockFileVersion"` marker and signed-32-bit scan before semantic JSON parsing. `RootModuleGraphKey` retains the Arc-backed parsed value after root/include success and before mapping. Formatting/key-order-only v28, absent, stale-update, and equivalent empty content compare semantically; malformed v28, overflow, read errors, and error-mode stale content fail. No registry fetch, hash production, write, hidden lockfile, new graph/commit, or raw planner activation landed.
Validation: focused bzlmod lockfile/root tests (41 + 9), loading lifecycle tests (6), and core A→B→A test passed; full `cargo test -p slug_bzlmod_v2` (153), `cargo test -p slug_loading_v2` (52), and `cargo test -p slug_core_v2` (17) passed; `cargo fmt --all -- --check`; `git diff --check`; `scripts/v2_archive_status.sh`; independent final review `ACCEPT`
Residual risk: Registry/yanked resolution must consume the parsed value and produce exact observed hashes with real update/refresh/error behavior before command-owned semantic writing. The old raw-text planner, exact exit 48, hidden lockfiles, full MVS/extensions, fetch/materialization, external loading, cquery, and aquery remain deferred.

### Stage 5 registry/yanked resolution owner design

Status: Accepted as an oracle-first replan
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `RegistryFunction.java`, `RegistryFactoryImpl.java`, `IndexRegistry.java`, `ModuleFileFunction.java`, `YankedVersionsFunction.java`, `YankedVersionsUtil.java`, `RepoSpecFunction.java`, `BazelModuleResolutionFunction.java`, `BazelDepGraphFunction.java`, `BazelLockFileModule.java`, and `BazelRepositoryModule.java`
Bazel oracle: Existing pinned update/refresh and yanked-union rows were audited and found insufficient for remote cache/refetch/error ordering; Bazel 9.1.1/version-26 and local-registry rows remain corroboration only
Expected evidence artifact: one deterministic fixture-local HTTP registry oracle pinning version-28 selected-yanked replay, refresh refetch, recorded absence, produced hashes, no-write-on-failure, and checksum-before-yanked ordering
Design summary: Pinned Bazel orders module discovery before MVS and selected-yanked/RepoSpec hash aggregation after MVS, so the prior unified pre-MVS registry/yanked packet is rejected. The accepted replacement uses cycle-free demand-driven DICE registry-file/module observations, explicit unrecorded/recorded-absent/known-hash states, a non-semantic IO capability, exact local observations, retryable transient/404 states, compact semantic values, and serial oracle → policy/IO → transport → discovery → MVS → selected-yanked/RepoSpec/final hashes → write ownership.
Validation: two independent read-only audits, pinned-source verification, live command/server/runtime trace, existing-fixture discrimination audit, `git diff --check`, archive-status check, and independent corrected-design rereview `ACCEPT`
Residual risk: No remote registry oracle, IO owner, registry transport, discovery key, MVS activation, selected-yanked/RepoSpec aggregation, or write landed. The controlled oracle is the only next packet.

### Stage 5 registry/yanked lockfile-mode oracle

Status: Accepted
V2 commit: `2e9a3a56 test: pin remote registry lockfile modes`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `RegistryFactoryImpl.java`, `IndexRegistry.java`, `ModuleFileFunction.java`, `BazelModuleResolutionFunction.java`, and `BazelLockFileModule.java`
Bazel oracle: `registry-yanked-lockfile-mode` with a fixture-local loopback HTTP registry; mutable BCR data is not part of any asserted transition, and the BCR fallback supplies only Bazel's embedded-module closure
Expected evidence artifact: one retained output base proves visible lockfile version 28 and remote hashes, selected-yanked reason A replay under update, A→B metadata refetch under refresh, recorded 404 replay then refresh retry, unchanged lock manifests after failures, and checksum failure before selected-yanked rejection
Implementation summary: Added fixture-scoped dynamic registry startup, endpoint token expansion, cumulative canonical request counts, and endpoint-normalized lockfile manifest comparison without changing Bazel's raw workspace bytes. The source-controlled registry mutates metadata and a previously absent module between commands. Startup has bounded early-exit/timeout handling and terminate-or-kill cleanup; focused tests cover two concurrent isolated services plus occupied-port failure.
Validation: oracle generation and multiple fresh Bazel 9.2 replays passed across different dynamic ports; the focused harness suite passed 34 tests; normalized lock manifests and request counts remained stable; `git diff --check`, archive status, and independent corrected-packet rereview returned `ACCEPT`
Residual risk: No registry policy/IO owner, command transport, discovery key, MVS activation, selected-yanked/RepoSpec aggregation, or semantic lockfile write landed. The first implementation design was subsequently replanned into `WP-5-m1-registry-policy-io-design-correction`.

### Stage 5 registry policy/IO initial implementation design

Status: Replanned before Rust
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `BazelRepositoryModule.normalizeBaseUrls`, `RegistryFactoryImpl.createRegistry`, `IndexRegistry.doGrabFile`, `ModuleFileFunction.getModuleFile`, and `GsonTypeAdapterUtil.OptionalChecksumTypeAdapter`
Bazel oracle: accepted `registry-yanked-lockfile-mode`; pinned source additionally owns ordered trailing-slash-normalized registry dedup, file-registry hash ignore, off-mode lockfile isolation, typed 404/transport boundaries, and checksum enforcement
Design summary: Global DICE computation data is accepted for an immutable non-semantic IO capability, but the proposed implementation key initially embedded request generations, retained ignored file policy, erased fatal errors into strings, and then still applied stale lockfile expectations in off mode. No Rust or Cargo edit began.
Validation: two read-only live/source audits and two independent design-review rounds; the second material correction triggered the orchestration replan rule
Residual risk: The replacement `WP-5-m1-registry-policy-io-design-correction` must freeze a policy-free root-files owner, injected generation dependency, stable file/remote identity, typed errors, and off-mode behavior before implementation.

### Stage 5 corrected registry policy/IO design

Status: Accepted
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, including `BazelRepositoryModule.normalizeBaseUrls`, `RegistryFactoryImpl.createRegistry`, `IndexRegistry.doGrabFile`, `ModuleFileFunction.getModuleFile`, and `GsonTypeAdapterUtil.OptionalChecksumTypeAdapter`
Bazel oracle: accepted `registry-yanked-lockfile-mode`, supplemented by pinned source for URL normalization/dedup, file-registry ignore, off-mode isolation, and typed IO boundaries
Design summary: A policy-free `RootModuleFilesKey` separates root/include/visible-lockfile ownership from registry policy. Ordered URLs and request generation are injected; a normal policy key consumes URLs/mode/root files. One stable exact-resource key bypasses policy for local files and conditionally depends on generation only for retryable remote outcomes. Off ignores all lockfile expectations; values/errors preserve not-found versus fatal transport/checksum identity; only immutable IO plumbing lives in global DICE data. Known-SHA transient 404/transport acquires generation on the failure branch and drops it after verified success.
Validation: fresh independent design review required one focused correction for known-SHA transient retry; corrected rereview returned `ACCEPT`
Residual risk: No Rust landed. Implement only `WP-5-m1-registry-policy-io-substrate`; command transport, external file-demand publication, discovery, MVS, selected-yanked/RepoSpec/final hashes, and writing remain serially deferred.

### Stage 5 registry command/daemon transport

Status: Accepted
V2 commits: `3bc88fd9 test: pin registry command transport`; `2777b6f8 feat: transport registry request policy`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `RepositoryOptions.java`, `BazelRepositoryModule.java`, `RegistryFunction.java`, `RegistryFactoryImpl.java`, and `ModuleFileFunction.java`
Bazel oracle: accepted `registry-command-transport` fixture for absent-only BCR, explicit replacement, raw trim/dedup/order, 404 fallback, fatal no-fallback, `%workspace%`, and invalid URL diagnostics
V2 fixture: owner-local bzlmod, commands, core, server, and CLI transport/recovery tests
Expected evidence artifact: primitive ordered request strings normalize once before any DICE change or generation allocation; malformed requests consume no generation; build/query and one-shot/daemon paths restore default→override→default
Implementation summary: Added repeatable equality-form registry parsing, serde-defaulted primitive daemon transport, one pre-commit `RegistryUrls::from_request`, compact first-occurrence raw dedup, workspace substitution, Java-URI-compatible supported-scheme validation, and request-local injection through the existing registry input owner. The TLS dependency graph now selects Ring consistently with Tonic. No `RegistryIo`, DICE key, discovery, MVS, lockfile writer, or loading owner changed.
Validation: full `slug_bzlmod_v2` (165), `slug_commands_v2` (16), `slug_core_v2` (20), `slug_server_v2` (20), and `slug_cli_v2` (32) suites; clean `slug_cli_v2` build; locked Cargo metadata; Ring-only feature tree; formatting, diff, archive, and daemon cleanup; fresh independent final review `ACCEPT`
Residual risk: Per-module registry discovery, selected registry module evaluation/digests, MVS, selected-yanked/RepoSpec aggregation, semantic lockfile writing, external repository loading, extensions, and materialization remain later serial packets.

### Stage 5 per-module registry discovery design

Status: Replanned before Rust for one oracle gap
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue.java`, `ModuleFileFunction.java`, `RegistryKey.java`, `RegistryFunction.java`, `RegistryFactoryImpl.java`, `IndexRegistry.java`, `BazelRepositoryModule.java`, and their focused tests
Bazel oracle: accepted `registry-command-transport` and `registry-yanked-lockfile-mode` cover normalized registry order, ordered 404 fallback, fatal no-fallback, remote recorded-absence replay, and refresh recovery
V2 fixture: existing `RegistryFileKey` tests cover exact local-file create/edit/delete/recreate and typed remote retry behavior, but no Bazel fixture covers the combined local module-discovery lifecycle
Expected evidence artifact: one `registry-module-discovery-recovery` fixture must retain a Bazel daemon across local absent→created→malformed→repaired→deleted→recreated transitions, prove malformed evaluation does not fall through, and distinguish first versus second registry module bodies
Design summary: One stable `(workspace, ModuleKey)` discovery key depends on ordered `RegistryPolicyKey` URLs and exact `RegistryFileKey` values. It never owns request generation or IO. Its compact value retains selected registry identity, exact URL/SHA/evaluated non-root module data, and every ordered URL-to-SHA-or-absence attempt, including local files. Only typed not-found falls through; file, transport, checksum, UTF-8, evaluation, include, and name/version errors are fatal. The byte evaluator is factored from the existing Starlark owner without connecting discovery to the root graph or loading.
Validation: two independent read-only audits plus root pinned-source and live-path verification; fresh independent review returned `ACCEPT`; no Rust, Cargo, fixture, test, or lockfile edit
Residual risk: Same-daemon local registry recovery lacks Bazel 9.2 evidence. Add only the named oracle, then obtain fresh design rereview before the five-file implementation. MVS, selected-yanked/RepoSpec aggregation, semantic writing, external loading, extensions, and materialization remain deferred.

### Stage 5 local registry discovery first executable probe

Status: Replanned after executable evidence disproved the design
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue.Key`, `ModuleFileFunction.getModuleFile`, `ModuleFileFunctionException`, `RegistryFunction`, `IndexRegistry.grabFile`, and the installed `embedded_tools/MODULE.bazel`
Bazel oracle: uncommitted `registry-module-discovery-recovery` probe with two local file registries, lockfile off, one retained server/output base, and deterministic local-path shims for the unrelated embedded-tools closure
Expected evidence artifact: the preserved six-row run reports ordered absence, then first-registry creation, then the same cached `firstdep` graph after malformed/edit/delete/recreate mutations under unchanged root and registry inputs
Design summary: Bazel retries the initial transient module-not-found failure, but a successful local registry module read has no local file Skyframe dependency and remains cached under its `ModuleKey` while root and registry inputs stay equal. This contradicts the accepted Slug substrate's unconditional local `WorkspaceFileKey` dependency and invalidates the discovery design's exact-file replay premise.
Validation: Bazel 9.2 run `20260723-224004-1531558-bazel`; exact normalized outputs inspected for all six commands; no Rust, Cargo, harness, existing-fixture, or lockfile edit
Residual risk: The fixture's original transition assertions are intentionally not accepted. Correct it to pin transient retry, sticky success, root-input-triggered malformed failure, repair, and delete fallback; then redesign the local registry IO/DICE branch before any discovery Rust.

### Stage 5 local registry module replay oracle correction

Status: Accepted
V2 commit: `0211982c test: pin local registry module replay`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue.Key`, `ModuleFileFunction`, `RegistryFunction`, `IndexRegistry`, `RegistryFactoryImpl`, and installed `embedded_tools/MODULE.bazel`
Bazel oracle: `registry-module-discovery-recovery` with two local file registries, lockfile off, one retained server/output base, and source-controlled local-path shims for the unrelated embedded-tools closure
Expected evidence artifact: ordered absent failure; transient retry after first creation; cached first-registry success after malformed mutation under equal root/registry inputs; unique root-version invalidation exposing fatal malformed content; repair recovery; and invalidated delete fallback to the second registry
Implementation summary: Added only the six-row fixture and expected evidence. Minimal local module declarations, local-path repo specs, and exact rules_java/buildozer/rules_cc redirect stubs prevent BCR or network closure content from becoming evidence; their behavior is not asserted.
Validation: Bazel 9.2 generation, worker replay, and independent root replay passed; fixture listing, six-row TOML/no-BCR checks, all pinned source anchors, normalized/raw record inspection, diff checks, and fresh independent evidence review returned `ACCEPT`
Residual risk: Slug's accepted local `RegistryFileKey` still depends on exact `WorkspaceFileKey` state and therefore invalidates successful reads more eagerly than Bazel. Design the non-semantic local IO, transient-failure generation, and root/registry replay epoch correction before discovery implementation.

### Stage 5 local registry replay correction design

Status: Accepted
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue.Key`, `ModuleFileFunction`, `ModuleFileFunctionException`, `RegistryFunction`, `RegistryFactoryImpl`, and `IndexRegistry`
Bazel oracle: accepted `registry-module-discovery-recovery` from `0211982c`
Expected evidence artifact: local exact-resource values retry absence/read failure on request generation, preserve successful bytes across raw local mutation under equal semantic inputs, and reread after semantic root or ordered registry-policy changes
Design summary: Keep stable `(workspace, URL)` identity. The local branch depends directly on `RegistryPolicyKey` and `RootModuleFilesKey`, then reads through the immutable global IO capability rather than `WorkspaceFileKey`; the direct root-files edge is required because current policy equality projects root semantics away. Found drops generation, while not-found/read failure conditionally acquire it. Core uses nonblocking `tokio::fs`; remote policy remains unchanged. `RootModuleGraphKey` and a raw-file epoch are rejected as overbroad and contrary to Bazel's sticky-success behavior.
Validation: two read-only pinned-source/live-owner audits, root source/equality adjudication, explicit correction of one overbroad raw-file invalidation recommendation, Tokio feature verification, and fresh independent design review `ACCEPT`
Residual risk: No Rust landed. Implement only `WP-5-m1-registry-local-replay-correction` in the three accepted files, prove all named lifecycle transitions, and obtain fresh final review before resuming per-module discovery.

### Stage 5 local registry replay correction

Status: Accepted
V2 commit: `6491a55a fix: match local registry replay semantics`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue.Key`, `ModuleFileFunction`, `ModuleFileFunctionException`, `RegistryFunction`, `RegistryFactoryImpl`, and `IndexRegistry`
Bazel oracle: accepted `registry-module-discovery-recovery` from `0211982c`; retained remote mode and transport oracles remain unchanged
V2 fixture: owner-local `registry_dice` lifecycle tests and core exact-adapter tests
Expected evidence artifact: transient local absence/read failures retry only after generation changes; successful local bytes remain sticky across raw mutations under equal semantic inputs; semantic root A→B→C and registry A→B→A reread
Implementation summary: Preserved stable `(workspace, URL)` identity, replaced the local `WorkspaceFileKey` edge with direct `RegistryPolicyKey` plus `RootModuleFilesKey` dependencies and immutable global IO, and conditionally acquired request generation only after local not-found/read failure. The core adapter dispatches `file://` through nonblocking Tokio filesystem IO before HTTP/TLS initialization state. Remote checksum, recorded-absence, refresh, and retry behavior are unchanged.
Validation: focused registry DICE 12 and core adapter 1; full `slug_bzlmod_v2` 167 and `slug_core_v2` 21; formatting, diff, archive; root full-diff/source adjudication; fresh independent final review `ACCEPT`
Residual risk: Per-module discovery is still absent. Rereview its prior design against the corrected local cache boundary and exact nonroot semantic dependencies before any discovery Rust.

### Stage 5 corrected per-module registry discovery rereview

Status: Replanned before Rust for root override evidence
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileValue`, `ModuleFileFunction`, `RegistryOverride`, `NonRegistryOverride`, `RegistryFunction`, and `IndexRegistry`
Bazel oracle: accepted registry transport, remote lockfile-mode, and local replay fixtures cover ordinary registry discovery but not root override routing
Expected evidence artifact: a root semantic value must distinguish default registry order, an override registry/version/patch policy, and non-registry routing before the stable `(workspace, ModuleKey)` discovery key can be implemented
Design summary: Stable discovery identity, direct root/policy dependencies, exact file-key iteration, typed SHA/absence attempts, fatal boundaries, and the factored nonroot Starlark evaluator remain viable. However, Bazel chooses `ModuleOverride` before discovery, while live production root evaluation records only local-path overrides and rejects all registry override globals. A partial default-registry API would not be parity, and adding the missing root Starlark surface inside discovery lacks Bazel 9.2 evidence.
Validation: two read-only pinned-source/live-owner audits, root evaluator/source adjudication, and fresh independent reserved-boundary review `REPLAN`; no Rust, Cargo, fixture, test, or lockfile edit
Residual risk: Add only `registry-root-override-routing`, then design and implement a compact semantic root override owner before returning to discovery.

### Stage 5 compact root override owner

Status: Accepted
V2 commit: `a5f13bf9 feat: own root module overrides`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileGlobals.convertAndValidatePatchLabel`, all five override globals, `RepoRuleId`, `ArchiveRepoSpecBuilder`, `GitRepoSpecBuilder`, and `LocalPathRepoSpecs`
Bazel oracle: accepted `registry-root-override-routing` from `256c02e2`
V2 fixture: owner-local `root_module_dice`
Expected evidence artifact: one compact root aggregate preserves exact registry/non-registry forms, defaults, ordered fields, generic attrs, canonical rule IDs and patch labels, duplicate errors, placement-insensitive equality, and retained A→B→A replay before discovery
Implementation summary: Added an Arc-backed Buck2 `SmallMap` owner with sealed single-version, multiple-version, and non-registry variants; exact canonical `.bzl` label plus rule-name IDs; recursive deterministic i32-bounded attribute values with active-cycle rejection; and private raw per-file contributions normalized and stripped at `RootModuleFilesKey`. Root/include duplicates fail, apparent external patch labels remain invisible, canonical external labels survive, archive/Git patches are validated while their raw kwargs are retained, and no discovery, execution, loading, MVS, or materialization consumer was activated.
Validation: focused `root_module_dice` 12/12; full `slug_bzlmod_v2` 170/170; `cargo fmt --all -- --check`; `git diff --check`; `scripts/v2_archive_status.sh`; pinned-source rule-ID and module-environment checks; fresh independent final rereview `ACCEPT`
Residual risk: Per-module discovery remains absent. Rereview its prior stable-key design against the landed override categories, patch-file DICE/application owner, fatal no-fallback behavior, and non-registry bypass before any Rust.

### Stage 5 post-owner registry discovery rereview

Status: Replanned before Rust
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `Discovery.rewriteDepSpec`, `ModuleFileFunction.getModuleFile`, `maybePatchModuleFile`, `execNonRegistryModuleFile`, `RegistryOverride`, and `NonRegistryOverride`
Bazel oracle: accepted `registry-root-override-routing` proves routing and static patch behavior, but not patch-file lifecycle or archive/Git module preparation
Expected evidence artifact: retained-daemon patch edit/delete/recreate plus independently discriminated local main/include edits and local archive/Git MODULE evaluation
Design summary: The stable registry key and corrected local retry boundary remain viable, but V2 has no exact root patch-file DICE/application owner and no materialized-repository MODULE source for non-registry overrides. Bazel applies root patches before registry nonroot evaluation and obtains local/archive/Git MODULE files only after `RepositoryDirectoryValue`; a registry-only result or deferred non-registry bypass would be a partial owner.
Validation: two independent read-only pinned-source/live-owner audits, root source/live-code adjudication, and fresh independent review returned `REPLAN`; no Rust, Cargo, fixture, lockfile, or existing evidence edit
Residual risk: Add only the nine-row `module-source-preparation` Bazel 9.2 oracle, then design a shared source-preparation boundary before discovery Rust.

### Stage 5 module-source preparation oracle first attempt

Status: Stopped before evidence
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, including registry URL expansion, `http_archive`, and `git_repository` worker paths
Bazel oracle: invalid uncommitted `module-source-preparation` draft; all generated failing records were discarded
Expected evidence artifact: unchanged nine-row retained-daemon patch/local/archive/Git lifecycle oracle
Implementation summary: A fixture-local deterministic archive and ordinary-file Git object store are representable, but the runner cannot inject the copied workspace's absolute local URI into MODULE source or mutation text. Relative archive URLs work; relative Git remotes resolve from the external helper repository, `%workspace%` is not expanded in repo-rule attrs, and `/proc/self/cwd` is both nonportable and wrong for the Git child.
Validation: pinned Bazel executable failures, harness/source inspection, and two independent read-only fixture-support audits; the exact untracked invalid fixture directory was removed
Residual risk: Design and implement a narrowly tested portable `{{workspace_uri}}` harness seam before regenerating this oracle. No source-preparation or discovery Rust is authorized.

### Stage 5 oracle workspace-URI harness design

Status: Accepted before implementation
Bazel source inspected: none; this is a representation-only correction to the existing Bazel 9.2 oracle harness
Bazel oracle: unchanged pending nine-row `module-source-preparation` fixture
Expected evidence artifact: copied-workspace absolute URI expansion without host-path leakage or fixture-confinement regressions
Design summary: Exact `{{workspace_uri}}` becomes `Path.resolve().as_uri()` for the copied workspace only. Initial expansion touches copied UTF-8 regular nonsymlink files; later expansion touches only mutation `find`, `replace`, and `content` operands while provenance retains raw templates. Binary/symlink/outside paths remain untouched, encoded URIs normalize to `file://<workspace>`, and there is no generic unknown-token failure because `{{http_registry}}` is conditional and unsupported operands intentionally remain literal.
Validation: fresh independent representation review returned `ACCEPT` for the three-file runner/normalizer/test allowlist, including space/non-ASCII, initial/mutation, provenance, confinement, binary/symlink, and existing HTTP/registry regressions
Residual risk: Implement only `WP-5-m1-oracle-workspace-uri-scope-correction`, then regenerate the unchanged nine-row source-preparation oracle. No fixture, expected artifact, Rust, Cargo, or lockfile edit is authorized in the correction.

### Stage 5 oracle workspace-URI harness correction

Status: Accepted
V2 commit: `de58ba16 test: add portable oracle workspace uri`
Bazel source inspected: none; this is a representation-only correction to the existing Bazel 9.2 oracle harness
Bazel oracle: unchanged pending nine-row `module-source-preparation` fixture
Expected evidence artifact: copied-workspace absolute URI expansion without host-path leakage or fixture-confinement regressions
Implementation summary: Exact `{{workspace_uri}}` expands to the copied workspace's resolved file URI in initial UTF-8 regular nonsymlink files and mutation text operands only. Raw mutation templates remain recorded, paths/destinations stay literal, binary/symlink/outside paths remain untouched, and encoded workspace URIs normalize to `file://<workspace>`. Raw-byte decode/replace/encode preserves CRLF and every non-token byte; existing `%workspace%` registry argv and conditional HTTP substitution remain unchanged.
Validation: focused oracle harness 38/38; fixture list, exact three-file scope, `git diff --check`, archive check; fresh independent final review found the newline defect and accepted the bounded raw-byte correction
Residual risk: Generate and independently replay only the unchanged nine-row `module-source-preparation` fixture before designing any source-preparation or discovery Rust.

### Stage 5 module-source preparation oracle

Status: Accepted
V2 commit: `183970d9 test: add module source preparation oracle`
Bazel source inspected: Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction`, `ModuleFileGlobals`, `http.bzl`, `git.bzl`, and `git_worker.bzl`
Bazel oracle: `module-source-preparation`
Expected evidence artifact: nine retained-daemon rows with exits `0,0,37,0,0,0,0,0,0`
Implementation summary: Checked-in local inputs prove patch A, patch-bytes-only B, fatal deletion without fallback, recreation recovery, local-path route bypass ahead of malformed/absent registries, independent main MODULE and include mutations, deterministic SHA-pinned archive evaluation, and an ordinary-file bare Git repository at a fixed commit. A first local registry contains only the graph leaves and bounded embedded-tools closure; it intentionally has no route module in the non-registry rows.
Validation: pinned Bazel generation; worker replay plus multiple root clean-run-root replays; exact nine-row/TOML/list/source-closure checks; archive SHA/content; bare Git `fsck`, fixed commit/tree, and non-gitlink dry-add; focused harness 38/38; diff/archive checks; fresh independent evidence review and final pruned-source rereview `ACCEPT`
Residual risk: Design and implement a DICE-owned shared preparation boundary for exact root patch inputs and complete local/archive/Git repository-root MODULE sources before returning to discovery.

### Stage 5 module-source preparation design

Status: Accepted as two serial implementation owners
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a` via exact Git objects, especially `ModuleFileFunction`, `ModuleFileValue`, `NonRegistryOverride`, `RepoSpec`, `RepositoryDirectoryValue`, repository definition/fetch functions, and `PatchUtil`
Bazel oracle: accepted `module-source-preparation` from `183970d9`
Expected evidence artifact: owner A for raw/local/materialized source files, followed by owner B for registry iteration and ordered root MODULE patches
Design summary: One stable module-source key will eventually route registry bytes or non-registry materialized roots before evaluation. The accepted serial split first adds raw workspace files plus stable materialization/source-file keys. Local repositories are live exact-file views; fixed archive/Git sources use retained immutable generations whose operational paths are excluded from equality. The second owner will iterate typed registry results and apply only ordered root-main single-version patches; all non-not-found and patch errors are fatal, patch commands stay inactive, and parsing/includes remain downstream.
Validation: independent pinned-source and live-owner audits; root verified current Bazel checkout differs from the pinned commit but the source-preparation core files are byte-identical and re-read the pinned Git objects; first review rejected generation-in-key identity and stale copied-local semantics; the focused stable-key/live-local correction received fresh `ACCEPT`
Residual risk: Owner A is accepted in `9c2a6814`; implement only owner B. Fixed archive/Git mutation invalidation, registry repository materialization, and discovery remain unclaimed.

### Stage 5 source-input materialization owner

Status: Accepted
V2 commit: `9c2a6814 feat: materialize module source inputs`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction`, `RepositoryDirectoryValue`, `NonRegistryOverride`, and repository materialization functions
Bazel oracle: accepted `module-source-preparation` from `183970d9`
Expected evidence artifact: raw/local/materialized owner A from the accepted two-owner design
Implementation summary: Added single-read raw/text workspace snapshots injected with directory state on one updater; stable materialization and exact source-file DICE keys; live local source roots; retained immutable fixed tar/Git generations with operational paths excluded from equality; and request-generation retry only for failed materialization. Production materialization is bounded to workspace-contained `local_repository`, SHA-pinned local tar `http_archive`, and local bare-Git exact commits.
Validation: full `slug_workspace_v2`, `slug_bzlmod_v2`, and `slug_core_v2` suites; raw equality, local main/include A→B→A, failure recovery, fixed archive/Git, generation-independent equality, retained-root, timeout-bounded cycle, fmt, diff, and archive checks; fresh independent final review `ACCEPT`
Residual risk: Implement only `WP-5-m1-module-source-preparation-key`; registry patch preparation is not yet consumed by evaluation or discovery, and fixed archive/Git source mutation remains unclaimed.

### Stage 5 module-source preparation owner first attempt

Status: Replan
V2 commit: none; bounded four-file draft preserved uncommitted
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction.maybePatchModuleFile` and `PatchUtil.applyToSingleFile`
Bazel oracle: accepted `module-source-preparation` from `183970d9`
Expected evidence artifact: registry/non-registry MODULE-byte routing and ordered main-repository root patches
Implementation summary: The draft added the stable preparation key, typed routing errors, existing registry/source-key dependencies, and a pure bounded patcher. Root corrected two initial outcome/path-filter defects, but the worker stopped without the required fake-registry retained-DICE matrix.
Validation: focused source-preparation and full bzlmod suites passed before root rereview; fmt, diff, and archive checks passed
Residual risk: Root rereview found the second material defect: strip zero silently removed `a/` and `b/` instead of emitting Bazel's forgotten-strip failure. Run only `WP-5-m1-module-source-preparation-scope-correction`; freeze the owner contract and add the missing matrix before fresh review.

### Stage 5 module-source preparation scope correction

Status: Accepted
V2 commit: `0445cafd feat: prepare module source bytes`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction.getModuleFile`, `maybePatchModuleFile`, and `PatchUtil.applyToSingleFile`
Bazel oracle: accepted `module-source-preparation` from `183970d9`
Expected evidence artifact: registry/non-registry MODULE-byte preparation with ordered main-repository root patches
Implementation summary: Added stable effective-version preparation identity, non-registry source routing, ordered registry lookup with only typed not-found fallback, fatal typed boundaries, exact raw main-repository patch dependencies, and a pure bounded single-file unified patcher. Non-main patches and patch commands remain inactive. The scope correction fixed strip-zero a/b failure, negative/no-strip behavior, unrelated-section skipping after structural validation, complete hunk counts/header grammar, and removed transient registry-vector churn.
Validation: focused retained-DICE 9 covers ordered fallback/fatality, override registry/effective version, non-registry bypass, raw-byte preservation, patch A→B→absent/malformed→recovery→A without local-registry refetch, ordered patches/strip, non-main/command inactivity, empty version, exhaustion, and cycle completion; full `slug_bzlmod_v2`, fmt, diff, archive; fresh independent rereview found one malformed-header gap, whose focused correction passed final `ACCEPT`
Residual risk: Prepared bytes are not yet evaluated or consumed by discovery. Registry identity/hash-attempt evidence, nonregistry includes, name/version checks, MVS, selected-yanked/RepoSpec aggregation, graph resolution, and lockfile writing remain later serial owners.

### Stage 5 per-module discovery design rereview after source preparation

Status: Replanned before Rust for one evaluator-ordering oracle
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a` via exact Git objects, especially `ModuleFileFunction.compute/getModuleFile/execModuleFile`, `ModuleFileValue`, `RegistryFileDownloadEvent`, `ModuleThreadContext`, `InterimModule`, and `Discovery.applyOverrides`
Bazel oracle: accepted registry transport/local replay/remote lockfile-mode/source-preparation fixtures prove routing, typed fallback/fatality, sticky local success, recorded remote absence/hash replay, patches, and non-registry includes; none directly discriminates registry include rejection, execution failure, declared-name mismatch, and declared-version mismatch
Expected evidence artifact: append five retained-daemon rows to `registry-module-discovery-recovery` which force include-before-execution, execution-before-declaration checks, name-before-version, then corrected success
Design summary: Keep stable `(workspace, effective ModuleKey)` discovery identity and consume `ModuleSourcePreparationKey` as the sole routing/materialization/patch owner. Widen preparation success from bytes alone to compact `NonRegistry { bytes }` or `Registry { bytes, selected_registry, ordered_attempts }`, where attempts preserve ordered compact URLs and explicit SHA-256/absence without operational paths or generations. Discovery never re-runs registry policy/file lookup. Factor the existing evaluator into a supplied-bytes compile/execute seam; registry modules reject includes after compile, non-registry include BFS reads exact `RepositorySourceFileKey` values, and failures remain preparation/patch/registry → parse → registry-include restriction → execution → name → nonempty effective version. The later implementation allowlist is `module_eval.rs`, `source_preparation.rs`, new `module_discovery.rs`, `lib.rs`, and new `module_discovery_dice.rs`; use `Arc`, `CompactString`, fixed digests, Buck2 compact maps/sets, and `Allocative`.
Validation: two independent read-only pinned-source/live-owner audits, root fixture-coverage and representation adjudication, and fresh independent review returned `REPLAN`; no Rust, Cargo, fixture, expected artifact, or lockfile edit
Residual risk: Add only `WP-5-m1-nonroot-module-evaluation-ordering-oracle`. MVS, selected-yanked/RepoSpec aggregation, graph resolution, lockfile writing, loading, command activation, filesystem access, and process execution remain deferred.

### Stage 5 nonroot module evaluation-ordering oracle

Status: Accepted
V2 commit: `51bfc915 test: pin nonroot module evaluation ordering`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction.compute/execModuleFile` and the upstream registry-include test
Bazel oracle: accepted eleven-row `registry-module-discovery-recovery`
Expected evidence artifact: preserve the original six local replay rows and append five isolated validation rows for include → execution → name → version → success
Implementation summary: Added one local `validation@1.0.0` registry module and five retained-daemon mutations. The root dependency is replaced, not appended, and its version advances on every row. Positive and anchored negative assertions prove registry include rejection before execution/declaration validation, execution before declaration validation, name before version, and corrected success.
Validation: Bazel 9.2 generation `20260724-015736-1645873-bazel`; independent replay `20260724-015802-1647334-bazel`; original six normalized stdout/stderr records unchanged; exact four-file changed scope; fixture list, pinned source anchors, diff and archive checks; fresh independent evidence review `ACCEPT`
Residual risk: No Rust landed. Freshly rereview only `WP-5-m1-registry-module-discovery-implementation-rereview` before the frozen five-file implementation.

### Stage 5 post-ordering discovery implementation rereview

Status: Replanned before Rust for a complete nonroot semantic owner
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `InterimModule`, `ModuleBase`, `ModuleThreadContext.buildModule`, `ModuleFileGlobals`, `CompiledModuleFile`, `ModuleFileFunction`, and `RegistryFileDownloadEvent`
Bazel oracle: accepted eleven-row `registry-module-discovery-recovery` pins replay and include/execution/name/version ordering but observes only dependency-shaped graph output
Expected evidence artifact: a Bazel 9.2 oracle design for the complete nonroot `InterimModule` semantic surface, followed by serial evaluator/schema, typed preparation provenance, and discovery owners
Design summary: The stable effective-`ModuleKey` discovery identity, sole preparation dependency, AST-backed pre-execution include detection, exact non-registry source-file BFS, and compact ordered registry attempts remain viable. The implementation packet is rejected because live `ModuleFileEvaluation` omits compatibility/Bazel-compatibility, max-compatibility and distinct nodep/original deps, registrations, extension usages, flag aliases, built-in collision semantics, and nonroot dev-dependency suppression. Caching that dependency-only subset would leave later MVS/extensions/activation without Bazel's discovery value. Preparation additionally returns bare `ModuleNotFound` on exhaustion and string-erases fatal registry causes.
Validation: root pinned-source/live-value audit and two-stage fresh independent rereview; the first narrow representation verdict was superseded after the complete `InterimModule` comparison returned `REPLAN`; no Rust, Cargo, fixture, expected artifact, or lockfile edit
Residual risk: Design only `WP-5-m1-nonroot-interim-module-oracle-design`. Do not implement discovery until the complete semantic oracle/schema and typed preparation provenance are separately accepted.

### Stage 5 complete nonroot semantic oracle design

Status: Accepted as three serial fixtures before Rust
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `InterimModule`, `ModuleBase`, `ModuleThreadContext`, `ModuleExtensionUsage`, `ModuleFileGlobals`, `CompiledModuleFile`, `ModuleFileFunction`, mod-command formatters, flag-alias aggregation, and registration consumers
Bazel oracle: accepted replay/ordering fixtures remain evidence; older directive/extension/registration fixtures are scaffolding references only
Expected evidence artifact: serial `nonroot-interim-module-graph`, `nonroot-module-extension-semantics`, and `nonroot-module-consumers` Bazel 9.2 fixtures
Design summary: One graph command cannot expose a complete `InterimModule`. Fixture 1 owns ordinary/nodep/dev dependency behavior, apparent aliases, ignored nonroot overrides, Bazel compatibility, and built-in collision. Fixture 2 owns extension usage/proxy/import/tag/isolation/override/inject semantics through exact `mod show_extension`, optional separate graph extension output, and generated-repo builds. Fixture 3 owns host-free nonroot platform/toolchain registrations, dev suppression, and global flag-alias consumption. Bazel 9 stores compatibility/max-compatibility as `0`/`-1` no-ops; CLI rows may prove only no-op behavior, while the future compact owner structurally tests the constants from pinned source. After all evidence, serial owners are complete evaluator/schema, typed preparation provenance for success/exhaustion/fatal causes, then stable discovery composition.
Validation: two read-only pinned-source/live-representation audits, root command/source correction, and fresh independent review `ACCEPT`; no Rust, Cargo, fixture, expected artifact, or lockfile edit
Residual risk: Implement only `WP-5-m1-nonroot-interim-module-graph-oracle`. Stop later extension/consumer fixtures rather than widening the harness or relying on unstable output/host defaults.

### Stage 5 nonroot InterimModule graph oracle

Status: Accepted
V2 commit: `908c7c62 test: pin nonroot module graph semantics`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction`, `ModuleFileGlobals`, `ModuleThreadContext`, `InterimModule`, and `ModuleBase`
Bazel oracle: `nonroot-interim-module-graph`
Expected evidence artifact: six retained-daemon local-registry rows for ordinary/dev/nodep dependency behavior, apparent mapping, ignored nonroot override, Bazel compatibility recovery, and built-in collision
Implementation summary: The root introduces shared@1 while subject's nodep requests shared@2; the resolved graph upgrades shared but subject-scoped deps exclude the nodep edge. Subject's ordinary aliased dep remains dep@1, its dev dep is absent, and its own SVO to dep@2 is ignored. Exact JSON preserves `subject_self` and `dep_alias` mappings. Root-version-invalidated compatibility failure/recovery and a built-in `bazel_tools` apparent-name collision complete the matrix. Nonzero compatibility/max inputs are explicitly no-op source evidence, not claims about hidden stored constants.
Validation: Bazel 9.2 generation `20260724-021408-1654566-bazel`; worker replay `20260724-021412-1655384-bazel`; independent root replay `20260724-021515-1656486-bazel`; fixture list, pinned anchors, no-BCR closure, diff and archive checks; fresh independent evidence review `ACCEPT`
Residual risk: Extension usages and platform/toolchain/flag consumers remain unproven. Add only `WP-5-m1-nonroot-module-extension-semantics-oracle`.

### Stage 5 nonroot module extension-semantics oracle

Status: Accepted
V2 commit: `8824135a test: pin nonroot extension semantics`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileFunction`, `ModuleFileGlobals`, `ModuleThreadContext`, and `ModuleExtensionUsage`
Bazel oracle: `nonroot-module-extension-semantics`
Expected evidence artifact: five retained-daemon local-registry rows for isolated-extension flag gating, detailed nonroot proxy/tags/imports, aggregate isolated identity, executable generated repositories, ignored nonroot redirection, dev suppression, and exact duplicate-import collision
Implementation summary: A deterministic subject archive defines ordinary, dev, isolated, and direct repository usages with independently named marker files. Exact `mod show_extension` output preserves subject location, ordered nondev tags, and imports while excluding the dev usage; separate `mod graph --extension_info=all` exposes the isolated proxy/import. The imported generated-repository build succeeds only because nonroot `override_repo` and `inject_repo` are ignored: their direct replacements deliberately lack the requested marker filenames. A root-version-invalidated duplicate import pins the collision boundary.
Validation: Bazel 9.2 generation `20260724-022519-1662598-bazel`; worker replay `20260724-022527-1663401-bazel`; independent root replay `20260724-022635-1664431-bazel`; fixture list, pinned anchors, deterministic archive/SRI equality, no-BCR local closure, negative assertions, diff and symlink checks; fresh independent evidence review `ACCEPT`
Residual risk: Platform/toolchain registrations and global flag-alias consumption remain unproven. Add only `WP-5-m1-nonroot-module-consumers-oracle`; no evaluator/schema, preparation-provenance, or discovery Rust is authorized before it is accepted.

### Stage 5 nonroot module consumer oracle

Status: Accepted
V2 commit: `eeea40a6 test: pin nonroot module consumers`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially the nonroot `ignoreDevDeps` call sites, `ModuleFileGlobals`, `ModuleBase`, registered toolchain/platform consumers, `SkyframeExecutor.getFlagAliases`, and `BlazeCommandDispatcher`
Bazel oracle: `nonroot-module-consumers`
Expected evidence artifact: eight retained-daemon local-only rows for ordinary platform/toolchain consumption, independently suppressed/recovered dev registrations, and globally consumed root/subject flag-alias replacement/recovery
Implementation summary: A local-path nonroot subject defines fixture-private ordinary/dev constraints, platforms, three toolchain types, and root/subject string settings. The ordinary consumer can resolve only through both nondev subject registrations; each dev registration then fails in isolation and succeeds after only its `dev_dependency` flag is removed. Successful actions write the resolved `ToolchainInfo.marker`. Root and subject both alias the existing native `compilation_mode`; exact output digests prove the subject-relative alias wins, removing it falls back to the root setting, and restoration recovers the subject marker.
Validation: Bazel 9.2 generation `20260724-024827-1695287-bazel`; worker replay `20260724-024853-1698315-bazel`; independent root replay `20260724-024924-1701402-bazel`; exact digest-byte checks, empty failure manifests, pinned source anchors, fixture list, no-BCR local closure, symlink and diff checks; fresh independent evidence review `ACCEPT`
Residual risk: All three complete-nonroot fixtures are accepted, but production still has only a dependency-shaped evaluator. Design only `WP-5-m1-nonroot-interim-module-evaluator-schema-design`, then keep typed preparation provenance and discovery composition serial.

### Stage 5 nonroot evaluator/schema design

Status: Accepted
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleBase`, `InterimModule`, `ModuleThreadContext`, `ModuleFileGlobals`, `ModuleExtensionUsage`, `CompiledModuleFile`, and `ModuleFileFunction`
Design summary: A new compact evaluator-owned semantic value covers every `ModuleBase` field and every non-provenance `InterimModule` field, with logical source spans and complete extension/proxy/tag/import/isolation/innate usage state. Preparation and discovery later wrap it with registry/non-registry provenance and ordered attempts. Nonregistry include inputs are supplied in discovery-owned BFS order, compiled as a complete closure before execution, and later execute inline with isolated bindings and one shared evaluator-local semantic context.
Representation: Retained values use `CompactString`, Arc slices, Buck2 `SmallMap`/`SmallSet`, `Dupe`, and `Allocative`. Extension attributes preserve arbitrary Starlark integers as i32-small or canonical-decimal large values without frozen heaps; root override attributes remain separately i32-bounded. `originalDeps` shares the finalized dependency map only after exact singleton `bazel_tools` insertion and collision handling.
Validation: two read-only pinned-source/live-owner audits, root include/equality/finalization adjudication, independent ownership review `ACCEPT`, initial representation review `REPLAN` for arbitrary-precision tag integers, and fresh corrected representation review `ACCEPT`
Residual risk: The recursive inline include evaluator and full directive surface remain unimplemented. Implement only `WP-5-m1-nonroot-schema-syntax-implementation` under the four-file allowlist; stop before directive evaluation, source preparation, or discovery.

### Stage 5 nonroot schema and syntax implementation

Status: Accepted
V2 commit: `c663fe46 feat: add nonroot module schema`
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleBase`, `InterimModule`, `ModuleThreadContext`, `ModuleExtensionUsage`, and `CompiledModuleFile`
Expected evidence artifact: evaluator-owned compact semantic schema and pure supplied-byte MODULE syntax/include inspector
Implementation summary: Added `EvaluatedNonrootModule`, its transient builder, fixed `-1` nodep dependencies, opaque canonical arbitrary-precision attribute integers, complete ordered extension/proxy/tag/import/isolation/innate state, exact singleton `bazel_tools` insertion and original-dependency snapshot, and logical source spans. The one-file public-AST inspector enforces Bazel's restricted MODULE dialect and exact direct-include classification and precedence while excluding bytes, physical paths, preparation provenance, IO, and DICE from retained equality.
Validation: focused nonroot structural/syntax tests 8/8; existing root evaluator/DICE tests 12/12; all 188 `slug_bzlmod_v2` tests; `cargo fmt --check -p slug_bzlmod_v2`; `git diff --check`; two fresh independent final reviews `ACCEPT`
Residual risk: Directive execution, include closure composition, typed preparation success/exhaustion/fatal provenance, and discovery remain unimplemented. Design only `WP-5-m1-nonroot-directive-evaluator-design`; no Rust is authorized before fresh acceptance.

### Stage 5 nonroot directive-evaluator design attempt

Status: Replan before Rust for raw attribute-domain evidence
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileGlobals.TagCallable`, `AttributeValues`, `AttributeValuesAdapter`, `ModuleThreadContext`, and later tag-class validation
Observed Bazel probe: a confined local-override nonroot module passed `3.14` and then the builtin `print` callable as a tag value; Bazel reached later tag-schema validation and reported each exact value plus `float` or `builtin_function_or_method`. The probe also encountered unrelated built-in repository disk-quota noise, so it is stop evidence rather than an accepted oracle fixture.
Design summary: The directive/global/finalization surface and a three-file starlark-rust implementation seam are otherwise feasible without locks, frozen retained heaps, Cargo changes, include composition, or root redesign. However, `AttributeValues.create(kwargs)` retains raw Starlark values; `AttributeValuesAdapter` constrains only lockfile serialization. The accepted compact recursive value cannot represent every evaluator result, and rejecting unsupported values during MODULE evaluation would move a later validation failure.
Validation: two read-only pinned-source/live-API audits; root source and executable-probe adjudication; initial independent design acceptance superseded by a fresh evidence-aware review `REPLAN`; no Rust, Cargo, fixture, or expected-artifact edit
Residual risk: Design only `WP-5-m1-nonroot-raw-attribute-oracle-design`. The next evidence must distinguish later-valid values from deferred-invalid nested/cyclic values for both extension tags and innate repo rules, then decide whether stable compact equality/diagnostics are possible without retaining Starlark heaps.

### Stage 5 nonroot raw-attribute oracle design

Status: Accepted before fixture generation
Bazel source inspected: pinned commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileGlobals.TagCallable`, `AttributeValues`, `Tag`, `TypeCheckedTag`, `AttributeUtils`, `InnateRunnableExtension`, `RepoRule`, `SingleExtensionUsagesValue`, `AttributeValuesAdapter`, and Starlark container/printer implementations
Expected evidence artifact: one local-only `nonroot-raw-attributes` retained-daemon fixture with valid structural, post-call mutation/alias, ordinary-tag deferred-invalid, innate repo-rule deferred-invalid, bounded cyclic, and update-mode serialization-order rows
Design summary: Raw tag and innate kwargs retain Starlark references until later schema conversion; extension-usage hashing independently serializes through a narrower adapter. Live/frozen Starlark heaps are forbidden across DICE, and the existing tree-only compact schema remains insufficient. A future post-file snapshot may use transient identities while walking exact structural values and may emit a bounded deferred-invalid token only for oracle-proven identity-insensitive failures. The fixed matrix samples float, builtin callable, extension proxy, nested invalid list/dict-key values, one self-cycle, and shared list/dict mutations without expanding to an arbitrary Starlark-type cross-product. Unique never-successful consumer outputs prevent stale same-daemon artifacts from faking failure, while update rows observe rather than assume lockfile publication.
Validation: three read-only pinned-source/live-representation/fixture audits; root source and boundary synthesis; independent source review `ACCEPT`; fixture review corrected pre-assumed unchanged lockfiles, stale output manifests, and an unreachable duplicate opaque sample, then returned fresh `ACCEPT`; no Rust, Cargo, fixture, expected artifact, lockfile, or harness edit
Residual risk: Generate only `WP-5-m1-nonroot-raw-attribute-oracle` under the new-fixture-only allowlist. Stop on network access, harness expansion, unstable identity formatting, accepted opaque values, cycle timeout/unbounded output, nondeterministic lockfile publication, or alias/cycle behavior requiring a retained evaluator heap.

### Stage 5 nonroot single-file directive evaluator

Status: Accepted
V2 commit: `b738547d feat: evaluate nonroot module directives`
Bazel source inspected: pinned Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `ModuleFileGlobals`, `ModuleThreadContext`, `ModuleFileFunction`, and `Version`
Bazel oracle: accepted complete-nonroot and raw-attribute fixtures from `908c7c62`, `8824135a`, `eeea40a6`, and `cffc39b0`
Implementation summary: Added one private supplied-file starlark-rust evaluator with exact nonroot globals, direct syntax-inaccessible `Module` roots for source-visible proxies and raw kwargs, forced-GC reread with fresh identities, final bounded snapshots, compact ordered output, dev suppression, global apparent-name collisions, normalized labels/versions, ignored redirections/overrides, isolated usages, and aggregated innate repo-rule usages.
Validation: focused evaluator 9/9; full `slug_bzlmod_v2` 204/204; exact spans, first-export aliases, build-metadata normalization, post-call mutation, deferred boundaries, built-in collisions, fmt, diff, archive; independent final rereview `ACCEPT`
Residual risk: Include closure composition, typed preparation provenance, and stable discovery composition remain. Design only `WP-5-m1-nonroot-include-composition-design`; do not edit Rust or combine later owners.

### Stage 5 nonroot include-composition design

Status: Replan before Rust for execution-order and diagnostic evidence
Bazel source inspected: pinned Bazel 9.2.0 commit `8220c6198837d5c13d53fea211cf3282aa12408a`, especially `CompiledModuleFile`, `ModuleFileFunction`, `ModuleThreadContext`, and upstream include tests
Design summary: Separate per-file starlark-rust `Module` heaps, cloneable supplied ASTs, one compact semantic context, file-indexed hidden roots, and final cross-heap reread are feasible. Exact repeated raw labels must reuse their stored file module. However, nested evaluators do not preserve Bazel's common include-parent call stack, and the public Rust evaluator combines scope compilation with execution instead of compiling the complete closure first.
Validation: two independent pinned-source/live-API audits, root source/API adjudication, and fresh independent review `REPLAN`; no Rust, fixture, Cargo, DICE, or public API edit
Residual risk: Bazel's BFS exposes no include-cycle termination/diagnostic. Design only `WP-5-m1-nonroot-include-composition-oracle-design` for bounded runtime-stack, compile-order, repeated-label, inline-order, and hard-timeout cycle characterization.
