# Stage 5: Bzlmod and Repository Graph

## Goal

Implement bzlmod as DICE-owned semantic state: module parsing, resolution,
repo mappings, repository specs, module extensions, lockfile policy, and
materialization manifests.

## Scope

- `MODULE.bazel` parsing and validation.
- MVS resolution and yanked-version policy.
- Bazel Central Registry and override handling.
- repository mappings for root, module repos, and extension-generated repos.
- module extension usages, aggregation, execution, facts, and generated repos.
- `MODULE.bazel.lock` read/write/update/error modes.
- repository-rule execution and materialization.

## V1 Extraction Candidates

Review and selectively extract from:

- `app/slug_bzlmod/src/parser.rs`
- `app/slug_bzlmod/src/dice_graph.rs`
- `app/slug_bzlmod/src/extension_execution_dice.rs`
- `app/slug_bzlmod/src/lockfile.rs`
- `app/slug_bzlmod/src/repo_mapping.rs`
- `tests/core/bzlmod/test_plan61_guardrails.py`

Each extraction needs an oracle fixture or direct Bazel source citation.

## Bazel Oracle Anchors

- `ModuleFileFunction.java` and `ModuleFileGlobals.java` own module-file
  parsing/evaluation and directive validation.
- `BazelModuleResolutionFunction.java` owns MVS resolution.
- `IndexRegistry.java`, `RepoSpecFunction.java`, and `YankedVersionsFunction.java`
  own registry metadata, repo specs, and yanked-version policy.
- `ModuleKey.java`, `BazelDepGraphFunction.java`, and `BazelDepGraphValue.java`
  own module keys, canonical repo names, and repo mappings.
- `BazelLockFileFunction.java` and `BazelLockFileModule.java` own lockfile
  read/update behavior.
- `SingleExtensionEvalFunction.java`, `SingleExtensionFunction.java`, and
  `ModuleExtensionRepoMappingEntriesFunction.java` own extension execution and
  repo-mapping behavior.
- `BazelLockFileValue.java` is the schema source; local Bazel currently reports
  `LOCK_FILE_VERSION` 28 and this must be checked before implementation.
- Bazel lockfile tests under `src/test/py/bazel/bzlmod/` are the first oracle
  source for replay/error-mode behavior.

## Implementation Slices

### 5.1 MODULE.bazel Evaluation

- Implement a parser/evaluator for root, registry, and non-registry
  `MODULE.bazel` files.
- Capture module name, version, compatibility level, bazel compatibility,
  `bazel_dep`, overrides, `include`, `use_extension`, `use_repo`,
  `override_repo`, `inject_repo`, `use_repo_rule`, `register_toolchains`,
  `register_execution_platforms`, and ignored directives.
- Preserve declaration order where Bazel order is semantically relevant.
- Root and non-root dev-dependency behavior, include restrictions, override
  validation, and registered toolchain/platform labels must match Bazel.

### 5.2 Resolution, Registries, and Overrides

- Define registry client traits for BCR, local registries, file URL, HTTP
  registry, archive override, git override, local path override, and single or
  multiple version overrides.
- Add DICE keys for discovery, MVS resolution, yanked versions, registry file
  hashes, and `source.json` repo specs.
- All fetched content must produce content digests and watched inputs.
- Cache directory paths are not semantic identity; content and policy are.
- Registry hash reuse/enforcement, yanked policy, and repo specs must match the
  Bazel oracle.

### 5.3 Canonical Repos and Repo Mappings

Create DICE keys for:

- root module file;
- non-root module file by module key;
- registry metadata by module/version;
- resolved dependency graph;
- canonical repository names;
- root and module repo mappings;
- generated repo specs.

The resolved graph preserves Bazel MVS ordering and feeds toolchain/platform
registration in the same order Bazel observes.

Implement `ModuleKey`, canonical repo names, full repo mappings,
apparent-to-canonical lookup, well-known modules, multiple-version naming,
extension-generated repo mappings, and root-only override scoping. Replace V1's
single-`@` storage with unambiguous canonical label rules from Stage 3.

### 5.4 Module Extensions

- Aggregate extension usages by extension id.
- Track unique extension names, isolated usages, generated repos, repo
  overrides, lockfile replay entries, facts, factsVersions, `.bzl` transitive
  digest, usages digest, and recorded-input validation.
- Execute extension implementation with prepared module data and repo mapping.
- Rewrite V1 thread-local repo-spec registry into explicit per-evaluation
  state.
- `repository_ctx` and `module_ctx` methods must not perform hidden semantic
  discovery; label paths, reads, downloads, and env lookups route through named
  async bridges or DICE keys.
- One extension usage change should invalidate only the owning extension.
- Stale `.bzl`, usage, recorded-input, and facts lockfile entries must fail in
  error mode.

### 5.5 Repository Rules and Materialization

- Convert `RepoSpec` to repository-rule invocation through DICE-owned semantic
  state.
- Track `repository_ctx` and `module_ctx` file, directory, tree, env,
  repository mapping, and download inputs as recorded inputs.
- Publish materialized repositories atomically with output digests and
  generation markers.
- Do not port V1 blocking locks across awaits, remove-then-rename publish gaps,
  direct-local bridges, or WORKSPACE scaffolding unless a Bazel oracle requires
  them.
- Failed publish preserves the previous generation, and same-daemon
  external-tree edits invalidate.

### 5.6 Lockfile Lifecycle

- Implement read, update, refresh, and error modes.
- Implement visible and hidden lockfile keys, version handling, registry hashes,
  selected yanked versions, module extension entries, facts, factsVersions, and
  `AttributeValues` serialization.
- Lockfile writes must be atomic and deterministic.
- Lockfile replay inputs include module files, extension usages, repo mappings,
  repository rule attrs, environment policy, OS/arch where relevant, and
  watched file digests.
- Error mode must reject stale or missing data instead of silently
  re-evaluating hidden state.
- `off` does not read/write, `update` writes changed visible lockfile data,
  `refresh` refreshes mutable registry state, and `error` rejects stale or
  unsupported entries.

### 5.7 V1 Guardrail Fixture Migration

Mine `tests/core/bzlmod/test_plan61_guardrails.py` for fixture themes only:
root, local, registry invalidation, included module files, lockfile writer
modes, extension replay, repo mapping, recorded inputs, materialization
markers, and same-daemon generation tests. Do not port exact V1 counters as
truth.

Every imported fixture must name its Bazel source/test oracle and have a V2
regression before code extraction, matching the Stage 9 extraction rule.

### 5.8 Same-Daemon Replay Matrix

Add oracle fixtures for:

- create/edit/delete root `MODULE.bazel`;
- registry metadata change under refresh mode;
- local override target file edit;
- extension tag change;
- extension-generated repo mapping change;
- `use_repo` add/remove;
- yanked version with and without allowlist;
- lockfile deleted, stale, and error-mode stale.


## Checkpoint Evidence

Stage 5 initial bzlmod parser checkpoint:

- Added oracle fixture placeholders for `module-file-directives` and
  `module-resolution-basic` before implementation.
- Added `slug_bzlmod_v2` with an independent, order-preserving parser for the
  initial `MODULE.bazel` directives: `module`, `bazel_dep`,
  `local_path_override`, `register_toolchains`, and
  `register_execution_platforms`.
- No V1 bzlmod source was extracted in this checkpoint.
- Local validation passed: `cargo test -p slug_bzlmod_v2`, `py -3 -B
  tools/v2_oracle list`, and the Stage 5 shortcut grep over
  `app/slug_bzlmod_v2` returned no matches.

Stage 5 local bzlmod oracle refresh:

- Generated Bazel 9.1.1 expected oracle output for the local-path bzlmod
  fixtures `module-file-directives` and `module-local-override`. Corrected the
  fixture labels to Bazel-visible source-file targets: `@dep_alias//:target.txt`
  for the `repo_name` case and `@dep//:target.txt` for the plain local override.
- At this checkpoint, `module-resolution-basic` still named registry modules and
  remained deferred; the later local transitive-resolution checkpoint below
  supersedes that for non-registry basic graph coverage.
- Validation passed: `py -3 -B -m tools.v2_oracle run --fixture module-file-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`;
  same command for `module-local-override`; bundled `python.exe -m pytest -q -p
  no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 5 include directive checkpoint:

- Added `include()` to the `slug_bzlmod_v2` parser directive stream and updated
  `module-file-directives` so its `bazel_dep` and `local_path_override` come
  from `include("//:deps.MODULE.bazel")`. A scratch Bazel 9.1.1 probe verified
  this include form before the fixture was updated.
- Regenerated the `module-file-directives` Bazel oracle after the include move;
  the fixture still proves the visible `repo_name` mapping via
  `@dep_alias//:target.txt`.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`; `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `py -3 -B -m tools.v2_oracle run --fixture module-file-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`; bundled
  `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`; and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 local transitive module-resolution checkpoint:

- Reworked `module-resolution-basic` from unresolved registry module names into
  a local transitive graph: root depends on `aaa`, `aaa` depends on `bbb`, and
  root `local_path_override` entries provide both module directories. The
  fixture builds `@aaa//:from_bbb`, which consumes `@bbb//:target.txt`.
- Generated Bazel 9.1.1 expected oracle output for the local graph. Registry
  hash/yanked/version-selection policy remains owned by the later dedicated
  registry fixtures.
- Validation passed: `py -3 -B -m tools.v2_oracle run --fixture module-resolution-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe`; bundled
  `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 5 override directive parser checkpoint:

- Added `module-override-validation`, a Bazel 9.1.1 oracle fixture proving
  `single_version_override`, `multiple_version_override`, `archive_override`,
  and `git_override` are parsed before Bazel rejects root overrides for
  nonexistent modules with query exit code 48.
- Extended `slug_bzlmod_v2` parsing with typed override directive structs,
  string-list values, boolean values, integer values, and order-preserving
  directive capture. No V1 bzlmod implementation was extracted.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `py -3 -B -m tools.v2_oracle run --fixture module-override-validation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.
Stage 5 module header parser checkpoint:

- Added `module-header-compatibility`, a Bazel 9.1.1 oracle fixture proving
  `module(repo_name = ...)` participates in root repository mapping while
  `bazel_compatibility = [">=9.0.0"]` is accepted by the local Bazel 9 oracle.
- Extended `slug_bzlmod_v2::ModuleHeader` with `repo_name` and
  `bazel_compatibility` fields while preserving the existing
  `compatibility_level` parse path and directive ordering. No V1 bzlmod
  implementation was extracted.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `py -3 -B -m tools.v2_oracle run --fixture module-header-compatibility --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.
Stage 5 module extension use_repo parser checkpoint:

- Added `module-extension-use-repo`, a self-contained Bazel 9.1.1 oracle
  fixture proving `use_extension` plus `use_repo` imports a repository generated
  by a local `module_extension` and repository rule, with no network dependency.
- Extended `slug_bzlmod_v2` parsing with assignment-aware `use_extension`
  directives, `use_repo` directives, extension proxy names, `dev_dependency`,
  and `isolate` flags. This records directive shape only; extension execution
  remains owned by the later Stage 5 module-extension DICE slices.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `py -3 -B -m tools.v2_oracle run --fixture module-extension-use-repo --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 5 repo directive parser checkpoint:

- Added `module-repo-directives`, a self-contained Bazel 9.1.1 oracle fixture
  proving `use_repo_rule` root repository creation, `override_repo` redirecting
  an extension-generated repo to a root repo, and `inject_repo` directive
  acceptance without registry or network input.
- Extended `slug_bzlmod_v2` parsing with typed `UseRepoRule`, repository-rule
  proxy invocation, `OverrideRepo`, `InjectRepo`, and apparent-to-source repo
  import records. This records directive shape only; repository-rule execution,
  extension aggregation, repo mapping replay, and lockfile semantics remain
  owned by later Stage 5 DICE slices.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-repo-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 module extension tag parser checkpoint:

- Added `module-extension-tags`, a self-contained Bazel 9.1.1 oracle fixture
  proving a `tag_class` call in `MODULE.bazel` can drive a generated repository
  imported with `use_repo`.
- Extended `slug_bzlmod_v2` parsing with typed `ExtensionTag` records and
  shared `ModuleAttributeValue` storage for repository-rule proxy calls and
  extension tag calls. This captures tag usage shape only; extension usage
  aggregation, lockfile replay, facts, and recorded-input validation remain
  owned by later Stage 5 DICE slices.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-tags --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 registration dev-dependency parser checkpoint:

- Added `module-registration-dev-dependency`, a self-contained Bazel 9.1.1
  oracle fixture proving `register_toolchains` and
  `register_execution_platforms` accept `dev_dependency` keyword arguments while
  normal package loading still succeeds.
- Extended `slug_bzlmod_v2` parsing with typed `Registration` records that
  preserve ordered registration labels and the `dev_dependency` flag. V1
  archive worktree references inspected: `app/slug_bzlmod/src/parser.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a fresh
  V2 parser change anchored by the Bazel oracle.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registration-dev-dependency --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.
## Exact Test Criteria

- Unit tests cover parser round-trips for every directive above, including
  order-sensitive registration lists.
- `module-resolution-basic` fixture resolves at least root plus two transitive
  modules and matches Bazel's selected versions and canonical repos.
- `module-file-directives` fixture covers `include`, `override_repo`,
  `inject_repo`, `use_repo_rule`, dev dependencies, and registration order.
- `repo-mapping-canonical-names` fixture compares root, dep, generated, and
  multiple-version repo mappings byte-for-byte after normalization.
- `registry-hash-yanked-policy` fixture covers registry hash reuse/enforcement
  and yanked-version allowlist behavior.
- `module-local-override` fixture changes an overridden module file and observes
  same-daemon invalidation.
- `module-extension-lockfile-replay` fixture performs prime/replay with no
  extension re-execution, then edits an extension tag and rejects replay.
- Lockfile JSON output is deterministic across two clean runs in separate temp
  directories.
- Lockfile mode fixture proves `off`, `update`, `refresh`, and `error`
  behavior against the Bazel oracle.
- Repository materialization fixture proves failed publish preserves the
  previous generation and external-tree edits invalidate in the same daemon.
- `rg -n "process-global|fallback scanner|marker trust|std::fs::read" <v2-bzlmod-crates>`
  has no production matches unless explicitly documented with a DICE tracking
  edge.
- No V1 bzlmod extraction lands unless it names the owner slice, V1 source
  path, Bazel oracle source/test reference, rejected V1 assumptions, and exact
  V2 fixture or command that proves parity.

## Acceptance Criteria

- No process-global semantic registry is required for bzlmod correctness.
- Same-daemon create/edit/delete transitions replay for clear DICE reasons.
- Lockfile replay rejects stale repo mappings, stale extension facts, and
  changed watched inputs.
- Generated repositories materialize through auditable DICE-owned state.

## Validation

```bash
cargo test -p slug_bzlmod_v2
slug-v2-oracle run --fixture module-file-directives
slug-v2-oracle run --fixture module-resolution-basic
slug-v2-oracle run --fixture repo-mapping-canonical-names
slug-v2-oracle run --fixture registry-hash-yanked-policy
slug-v2-oracle run --fixture module-local-override
slug-v2-oracle run --fixture module-extension-lockfile-replay
slug-v2-oracle run --fixture lockfile-error-mode-stale
slug-v2-oracle run --fixture yanked-version-policy
slug-v2-oracle run --fixture repository-materialization-atomicity
```
