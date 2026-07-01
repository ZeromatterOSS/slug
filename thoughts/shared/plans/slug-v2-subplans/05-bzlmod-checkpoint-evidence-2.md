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
V2 commit: Pending checkpoint on `codex/slugv2-clean-root-remediation`
V1 source inspected: None for implementation; derived from existing V2 registry digest substrates and Bazel 9 lockfile registry-hash oracle fixtures
Bazel oracle: Bazel 9.1.1 `lockfile-error-mode-registry-hash` and `lockfile-error-missing-registry-hash` fixtures
V2 fixture: `lockfile-error-mode-registry-hash`, `lockfile-error-missing-registry-hash`
Expected evidence artifact: Stage 1 oracle expected output for lockfile error-mode registry hash validation and missing registry hash diagnostics
Implementation summary: Added `observed_registry_file_hashes` to convert selected registry MODULE.bazel and source.json content digests into the URL-to-digest map consumed by visible lockfile registry-hash validators; the helper requires explicit observed digests and does not add network fetching, filesystem registry scans, cache lookup, lockfile writes, or repository materialization
Validation: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; `py -3 -B -m tools.v2_oracle run --fixture lockfile-error-missing-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`; bundled `pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; Stage 5 guardrail grep and diff checks before commit
Residual risk: Observed registry digests are now shaped for lockfile validators, but producing those digests from actual registry fetches or local-registry reads, visible lockfile updating, refresh/error mode lifecycle, and same-daemon stale rejection remain later Stage 5.2/5.6 work
