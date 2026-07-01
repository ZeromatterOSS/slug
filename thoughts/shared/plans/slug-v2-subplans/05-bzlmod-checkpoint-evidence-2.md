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
