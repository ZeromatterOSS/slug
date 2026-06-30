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
- `BazelLockFileValue.java` is the schema source; local Bazel 9.1.1 oracle
  fixtures currently emit `lockFileVersion` 26 for visible lockfiles, and this
  must be rechecked before broader replay/error-mode implementation.
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
Stage 5 use_repo_rule dev-dependency checkpoint:

- Added `module-use-repo-rule-dev-dependency`, a self-contained Bazel 9.1.1
  oracle fixture proving `dev_dependency` belongs on the repository-rule
  invocation created by `use_repo_rule`, while the `use_repo_rule(...)` factory
  call rejects that keyword.
- Updated `slug_bzlmod_v2` so `RepoRuleInvocation` records
  `dev_dependency` separately from repository-rule attrs, and `UseRepoRule`
  rejects extra factory keywords. V1 archive worktree references inspected:
  `app/slug_bzlmod/src/parser.rs`, especially the use_repo_rule parser tests.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-use-repo-rule-dev-dependency --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 multiline MODULE directive parser checkpoint:

- Added `module-multiline-directives`, a self-contained Bazel 9.1.1 oracle
  fixture proving multiline `module`, `use_repo_rule`, repository-rule
  invocation, `register_toolchains`, and `register_execution_platforms`
  calls parse before repository materialization and package query.
- Updated `slug_bzlmod_v2` to collect Starlark-shaped logical statements across
  physical lines before applying the existing directive parsers, including
  comment stripping outside strings and unterminated-directive diagnostics. V1
  archive reference inspected: `app/slug_bzlmod/src/parser.rs`; the V2 change
  remains an independent lightweight parser slice, not a Starlark evaluator
  port.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-multiline-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 single-quoted MODULE string parser checkpoint:

- Added `module-single-quoted-directives`, a self-contained Bazel 9.1.1 oracle
  fixture proving MODULE.bazel accepts single-quoted strings for `module`,
  `use_repo_rule`, repository-rule invocation, and registration arguments.
- Updated `slug_bzlmod_v2` string scanning so logical statement collection,
  comment stripping, argument splitting, and string literal parsing accept both
  Bazel/Starlark quote forms. V1 archive reference inspected:
  `app/slug_bzlmod/src/parser.rs`; implementation remains a scoped V2 parser
  change, not an import of the V1 Starlark evaluator.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-single-quoted-directives --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 local module graph substrate checkpoint:

- Strengthened `module-resolution-basic` with a Bazel 9.1.1 `cquery` command
  proving the apparent `@aaa//:from_bbb` label resolves while analysis reports
  the canonical `@@aaa+//:from_bbb` local-module repo shape.
- Added a `slug_bzlmod_v2::resolution` substrate with typed `ModuleKey`, root
  and local-path module sources, Bazel-shaped canonical module repo names, and
  deterministic repo mappings for a root plus transitive local override graph.
  V1 archive reference inspected: `app/slug_bzlmod/src/resolution.rs`; the V2
  implementation intentionally stops before registry MVS, yanked policy,
  lockfile replay, or DICE ownership.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-resolution-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 local override declared-version checkpoint:

- Added `module-local-override-version-selection`, a Bazel 9.1.1 oracle
  fixture proving `local_path_override` accepts the overridden module file's
  declared version even when a dependency requested a lower version.
- Updated `slug_bzlmod_v2::resolution` so local override resolution selects
  the local module header's `module(version = ...)` for the `ModuleKey` instead
  of rejecting version mismatches against individual `bazel_dep` requests. This
  is deliberately narrower than registry MVS; registry version selection remains
  owned by Stage 5.2.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-local-override-version-selection --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 local override request-order checkpoint:

- Added `module-local-override-request-order`, a Bazel 9.1.1 oracle fixture
  proving repeated requests for the same locally overridden module are
  order-independent: `ccc` requests `bbb@2.0.0` before `aaa` requests
  `bbb@1.0.0`, and both targets analyze through the same local override module.
- Updated `slug_bzlmod_v2::resolution` so once a local override module is
  selected, later requests for that module name do not reject solely because the
  requested version differs from the selected local module header version.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-local-override-request-order --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 registry MVS substrate checkpoint:

- Added `module-registry-mvs-basic`, a Bazel 9.1.1 oracle fixture using a
  workspace-local registry (`file:///%workspace%/registry`) plus BCR fallback
  for Bazel embedded modules. `bazel mod graph` proves `aaa@1.0.0` requests
  `bbb@1.0.0`, `ccc@1.0.0` requests `bbb@2.0.0`, and MVS selects
  `bbb@2.0.0` for both dependency edges.
- Added `slug_bzlmod_v2::registry` with typed registry module records and a
  focused MVS resolver that selects the highest requested version and produces
  the existing V2 `ResolvedGraph` shape with registry-backed `ModuleSource`
  records. V1 archive references inspected: `app/slug_bzlmod/src/registry.rs`,
  `app/slug_bzlmod/src/resolution.rs`, and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from behavior.
- This checkpoint intentionally stops before registry hash enforcement, repo-spec
  fetching/materialization, multiple-version overrides, lockfile replay, and
  DICE ownership.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registry-mvs-basic --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 yanked-version policy checkpoint:

- Added `yanked-version-policy`, a Bazel 9.1.1 oracle fixture using a
  workspace-local registry where `yyy@1.0.0` is marked yanked with reason
  `bad release`. Bazel rejects `bazel mod graph` with exit code 37 by default
  and accepts the same graph with `--allow_yanked_versions=yyy@1.0.0`.
- Added yanked-version policy validation to `slug_bzlmod_v2::registry`, with
  default reject, explicit allowlist, and allow-all modes over the V2 resolved
  graph. V1 archive references inspected: `app/slug_bzlmod/src/registry.rs`
  and `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from behavior.
- This checkpoint intentionally stops before registry hash reuse/enforcement,
  environment-sourced allowlists, lockfile selected-yanked-version recording,
  and DICE-owned registry metadata keys.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture yanked-version-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  and `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 registry source.json policy checkpoint:

- Added `registry-source-json-policy`, a Bazel 9.1.1 oracle fixture using a
  workspace-local registry where `bazel mod graph` accepts archive `source.json`
  with `url` plus `integrity`, and rejects missing URL, missing integrity, and
  invalid JSON with Bazel exit code 37.
- Added `slug_bzlmod_v2::registry` source metadata parsing for archive
  `source.json`: `url`/`urls`, `integrity`, `type`, `strip_prefix`, `patches`,
  and `patch_strip`, with Bazel-shaped diagnostics for missing source URL,
  missing integrity, and malformed JSON. V1 archive references inspected:
  `app/slug_bzlmod/src/registry.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from behavior using structured JSON parsing.
- This checkpoint intentionally stops before archive download/extraction,
  registry hash enforcement, patch application, repository materialization,
  lockfile replay, and DICE-owned registry metadata keys.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture registry-source-json-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 registry metadata parser checkpoint:

- Reused the Bazel 9.1.1 `yanked-version-policy` oracle fixture as the
  registry `metadata.json` anchor: the local registry exposes
  `versions = ["1.0.0"]` and `yanked_versions = {"1.0.0": "bad release"}`;
  Bazel rejects the selected yanked version by default and accepts it with
  `--allow_yanked_versions=yyy@1.0.0`.
- Added `slug_bzlmod_v2::registry` metadata parsing for `metadata.json`:
  required `versions`, optional `homepage`, optional `repository`, and
  `yanked_versions`, plus conversion from yanked version strings into the
  existing `ModuleKey -> reason` policy input. V1 archive references inspected:
  `app/slug_bzlmod/src/registry.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from behavior using structured JSON parsing.
- This checkpoint intentionally stops before registry HTTP/file clients,
  registry file hash enforcement, lockfile selected
  yanked-version recording, environment-sourced allowlists, and DICE-owned
  registry metadata keys.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture yanked-version-policy --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 multiple-version override resolver checkpoint:

- Added `module-registry-multiple-version-override`, a Bazel 9.1.1 oracle
  fixture using a workspace-local registry where `multiple_version_override`
  keeps both `bbb@1.0.0` and `bbb@2.0.0` selected. `bazel mod
  dump_repo_mapping` proves `aaa+` maps apparent `bbb` to `bbb+1.0.0`, while
  `ccc+` maps apparent `bbb` to `bbb+2.0.0`.
- Updated `slug_bzlmod_v2::registry` resolution so root
  `multiple_version_override` directives preserve allowed requested versions
  side by side, emit Bazel-shaped canonical repo names for multiple selected
  versions, and keep ordinary MVS behavior for modules without the override.
  V1 archive references inspected: `app/slug_bzlmod/src/registry.rs`,
  `app/slug_bzlmod/src/resolution.rs`, and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from observed Bazel behavior.
- This checkpoint intentionally stops before multiple-version override lockfile
  data, registry client/fallback integration, DICE-owned graph keys, and repo
  mapping byte-for-byte acceptance fixtures beyond the focused `mod` oracle.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registry-multiple-version-override --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 single-version override resolver checkpoint:

- Added `module-registry-single-version-override`, a Bazel 9.1.1 oracle fixture
  using a workspace-local registry where `single_version_override(module_name =
  "bbb", version = "2.0.0")` replaces a transitive `bbb@1.0.0` request with
  `bbb@2.0.0`. `bazel mod dump_repo_mapping` proves the selected module still
  uses the normal `bbb+` canonical repo name.
- Updated `slug_bzlmod_v2::registry` resolution to apply root
  `single_version_override` requested versions before registry module lookup,
  while preserving ordinary MVS behavior and the already-landed
  `multiple_version_override` selected-version set. V1 archive references
  inspected: `app/slug_bzlmod/src/registry.rs`,
  `app/slug_bzlmod/src/resolution.rs`, and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from observed Bazel behavior.
- This checkpoint intentionally stops before single-version override patches,
  alternate override registry selection, lockfile data, registry client/fallback
  integration, and DICE-owned graph keys.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-registry-single-version-override --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 ordered registry fallback checkpoint:

- Added `registry-fallback-order`, a Bazel 9.1.1 oracle fixture using two
  workspace-local registries where the first registry supplies `aaa@1.0.0`
  and the second supplies an alternate `aaa@1.0.0` plus `ccc@1.0.0`. `bazel
  mod graph` proves Bazel takes `aaa` from the first registry and falls
  through to the second registry for `ccc`.
- Added `slug_bzlmod_v2::registry` catalog selection that preserves ordered
  registry fallback: earlier registries win per module key and later registries
  fill missing module keys. V1 archive references inspected:
  `app/slug_bzlmod/src/registry.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 rewrite from observed Bazel behavior.
- This checkpoint intentionally stops before HTTP/file registry clients,
  registry hash enforcement, refresh/error lockfile modes, DICE-owned metadata
  keys, and same-daemon registry mutation replay.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture registry-fallback-order --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 selected-yanked lockfile checkpoint:

- Added `lockfile-selected-yanked-version`, a Bazel 9.1.1 oracle fixture using
  a workspace-local registry with `yyy@1.0.0` yanked. A generated executable
  prints only the `MODULE.bazel.lock` `selectedYankedVersions` lines after
  `--allow_yanked_versions=yyy@1.0.0` permits the graph, proving Bazel records
  `"yyy@1.0.0": "bad release"` in the visible lockfile.
- Added `slug_bzlmod_v2::lockfile` visible-subset parsing for
  `lockFileVersion`, `registryFileHashes`, and `selectedYankedVersions`, with
  selected yanked keys converted to `ModuleKey`. V1 archive references
  inspected: `app/slug_bzlmod/src/lockfile.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 parser from observed Bazel 9.1.1 lockfile shape.
- This checkpoint intentionally stops before lockfile writing, refresh/error
  modes, hidden lockfiles, module extension replay entries, facts/factsVersions,
  environment-sourced allowlists, registry hash enforcement, and DICE ownership.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-selected-yanked-version --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 registry-hash lockfile error checkpoint:

- Added `lockfile-error-mode-registry-hash`, a Bazel 9.1.1 oracle fixture
  with a BCR `rules_cc@0.2.17` dependency and an intentionally stale
  `registryFileHashes` entry in `MODULE.bazel.lock`. `bazel mod graph
  --lockfile_mode=error` rejects the graph with exit code 37 and a checksum
  mismatch for the BCR `MODULE.bazel` file.
- Added `slug_bzlmod_v2::lockfile` registry-hash validation against an explicit
  observed digest map. The helper emits Bazel-shaped checksum diagnostics while
  staying independent from registry fetching, file hashing, lockfile writing,
  and DICE ownership. V1 archive references inspected:
  `app/slug_bzlmod/src/lockfile.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation remains a
  scoped V2 helper from observed Bazel 9.1.1 error behavior.
- This checkpoint intentionally stops before computing registry file digests,
  HTTP/file registry clients, lockfile write/update/refresh flows, hidden
  lockfiles, same-daemon stale-registry replay, and DICE-owned registry inputs.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-error-mode-registry-hash --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`.

Stage 5 yanked-version environment allowlist checkpoint:

- Added `yanked-version-env-allowlist`, a Bazel 9.1.1 oracle fixture using the
  existing local yanked registry shape and `BZLMOD_ALLOW_YANKED_VERSIONS` set
  through the Stage 1 per-command environment override support. Bazel accepts
  `yyy@1.0.0` without the command-line `--allow_yanked_versions` flag.
- Added `YankedVersionPolicy::from_env_value` for the Bazel env value shape:
  empty or absent rejects, `all` allows all selected yanked modules, and
  comma-separated `module@version` entries build an explicit allowlist.
  V1 guardrail references inspected for env-specific behavior but none were
  imported; implementation remains a scoped V2 parser from observed Bazel 9.1.1
  behavior.
- This checkpoint intentionally stops before wiring process environment into
  DICE-owned bzlmod keys, same-daemon environment invalidation/replay, lockfile
  selected-yanked write policy, and command-line/env precedence checks.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture yanked-version-env-allowlist --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.

Stage 5 bzlmod DICE environment key checkpoint:

- Added `yanked-version-env-change`, a Bazel 9.1.1 oracle fixture proving
  `BZLMOD_ALLOW_YANKED_VERSIONS` is command-environment semantic state: the
  same workspace accepts `yyy@1.0.0` while the env value is set, then rejects
  the same graph after the env value is absent.
- Added `slug_bzlmod_v2::dice` key-shaped inputs for resolved bzlmod graphs:
  root module digest, included-module digest, registry policy digest,
  lockfile digest, lockfile mode, and environment policy. The environment
  policy serializes the parsed yanked-version allowlist so env changes alter
  key equality and stable serialization. V1 archive references inspected:
  `app/slug_bzlmod/src/dice_graph.rs` and
  `tests/core/bzlmod/test_plan61_guardrails.py`; implementation follows the
  V2 Stage 6 DICE input pattern and does not import V1 compute code.
- This checkpoint intentionally stops before actual DICE `Key` trait wiring,
  filesystem/env digest production, registry clients, lockfile replay,
  module-extension keys, or same-daemon materialization replay.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture yanked-version-env-change --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 yanked command/environment policy checkpoint:

- Added `yanked-version-command-env-union`, a Bazel 9.1.1 oracle fixture
  proving `--allow_yanked_versions` and `BZLMOD_ALLOW_YANKED_VERSIONS` combine
  instead of overriding each other: either source can allow `yyy@1.0.0` while
  the other names only `zzz@2.0.0`.
- Added `BzlmodCommandPolicyKey` beside `BzlmodEnvironmentPolicyKey` and made
  `BzlmodDiceInputs` include both policies in equality, hash, and stable
  serialization. The effective yanked-version policy is the union of command
  and environment allowlists, matching the Bazel oracle. This extends the prior
  key-shaped DICE substrate without importing V1 compute code.
- This checkpoint intentionally stops before actual DICE `Key` trait wiring,
  CLI flag plumbing into bzlmod evaluation, lockfile selected-yanked writes,
  or same-daemon command/env replay.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture yanked-version-command-env-union --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 lockfile-mode flag parser checkpoint:

- Added `lockfile-mode-flag-validation`, a Bazel 9.1.1 oracle fixture proving
  `--lockfile_mode=off|update|refresh|error` are accepted and an unknown value
  fails before graph resolution with exit code 2 and Bazel's
  `Not a valid Lockfile mode` diagnostic.
- Added `LockfileMode::from_bazel_flag_value` so the V2 bzlmod DICE input
  substrate can parse the command flag into the existing `LockfileMode` enum
  without accepting unsupported values.
- This checkpoint intentionally stops before implementing lockfile read/write
  mode behavior, visible/hidden lockfile replay, refresh fetching, or
  error-mode stale-data checks beyond the existing registry-hash fixture.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture lockfile-mode-flag-validation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 included-module digest checkpoint:

- Added `module-include-change-invalidation`, a Bazel 9.1.1 oracle fixture
  proving an included `deps.MODULE.bazel` fragment is semantic same-output-base
  state. The fixture edits only the included fragment from `modules/dep_one` to
  `modules/dep_two`; Bazel rebuilds the same `bazel-bin/version.out` output
  with a different manifest digest in the next command.
- Added pure V2 digest helpers for bzlmod DICE inputs:
  `digest_module_file_content`, `BzlmodModuleFileDigest`, and
  `digest_included_module_files`. The helpers use SHA-256 over supplied bytes
  and normalized relative include paths; they do not read the filesystem, so
  later DICE keys can own file-read dependencies explicitly.
- This checkpoint intentionally stops before actual DICE `Key` trait wiring,
  include discovery, file watching, root/included digest production from live
  paths, lockfile replay, or same-daemon materialization replay.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-include-change-invalidation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 registry policy digest checkpoint:

- Added `registry-order-change-invalidation`, a Bazel 9.1.1 oracle fixture
  proving ordered registry policy changes selected module graph state across a
  bzlmod resolution startup/replay boundary. The fixture starts with
  `first, second` registry order where `bbb@1.0.0` is absent from `bazel mod
  graph`, edits only `.bazelrc` to `second, first`, shuts down the warm Bazel
  server, then reruns the same output base with `--lockfile_mode=off`; Bazel
  reports `bbb@1.0.0` after restart.
- The failed warm-server probe before adding the shutdown command showed Bazel
  reusing the prior graph despite the `.bazelrc` mutation. Same-daemon registry
  option replay remains a named residual for the later DICE wiring instead of
  being claimed by this fixture.
- Added pure V2 digest helpers for the registry-policy portion of
  `BzlmodDiceInputs`: `BzlmodRegistryPolicyEntry` and
  `digest_registry_policy`. The helper preserves caller order instead of
  sorting, uses SHA-256 over supplied registry identity/digest tokens, and does
  not fetch, read, or trust cache paths.
- This checkpoint intentionally stops before HTTP/file registry clients,
  registry metadata/source digest production, lockfile replay, refresh/error
  registry behavior, or same-daemon registry option invalidation.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture registry-order-change-invalidation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 module-extension usage digest checkpoint:

- Added `module-extension-tag-change-invalidation`, a Bazel 9.1.1 oracle
  fixture proving extension tag values are semantic generated-repository state.
  The fixture queries `@tagged//:one.txt`, edits only the root MODULE tag value
  from `message = "one"` to `message = "two"`, then queries
  `@tagged//:two.txt` in the same output base.
- Added pure V2 digest helpers for the module-extension usage portion of
  `BzlmodDiceInputs`: `BzlmodExtensionUsageDigest` and
  `digest_module_extension_usages`. The helper sorts by extension id, rejects
  duplicate ids, uses SHA-256 over supplied usage digests, and does not execute
  extensions, read `.bzl` files, or inspect generated repositories.
- Extended `BzlmodDiceInputs` equality/hash/stable serialization with the
  extension usage digest so tag changes can invalidate resolved-graph keys for
  a clear DICE-owned reason once actual key wiring lands.
- This checkpoint intentionally stops before extension usage aggregation,
  `.bzl` transitive digest production, module extension execution, facts and
  factsVersions, generated-repository mappings, lockfile replay, or extension
  isolation semantics.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-tag-change-invalidation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 module-extension definition digest checkpoint:

- Added `module-extension-bzl-change-invalidation`, a Bazel 9.1.1 oracle
  fixture proving extension `.bzl` implementation content is semantic
  generated-repository state. The fixture queries `@generated//:impl_one.txt`,
  edits only `ext.bzl` from `_OUTPUT_NAME = "impl_one"` to
  `_OUTPUT_NAME = "impl_two"`, then queries `@generated//:impl_two.txt` in the
  same output base.
- Added pure V2 digest helpers for the module-extension definition portion of
  `BzlmodDiceInputs`: `BzlmodExtensionDefinitionDigest` and
  `digest_module_extension_definitions`. The helper sorts by extension id,
  rejects duplicate ids, uses SHA-256 over supplied `.bzl`/definition digests,
  and does not execute extensions, read files, or inspect generated repos.
- Extended `BzlmodDiceInputs` equality/hash/stable serialization with the
  extension definition digest, separate from extension usage/tag digests, so
  implementation changes and usage changes can invalidate for distinct reasons.
- This checkpoint intentionally stops before `.bzl` transitive digest
  production, extension usage aggregation, module extension execution,
  facts/factsVersions, generated-repository mappings, lockfile replay, or
  extension isolation semantics.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-bzl-change-invalidation --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.

Stage 5 module-extension visible lockfile checkpoint:

- Added `module-extension-lockfile-shape`, a Bazel 9.1.1 oracle fixture
  proving the visible `MODULE.bazel.lock` `moduleExtensions` shape for a
  generated repository: extension id `//:ext.bzl%ext`, `bzlTransitiveDigest`,
  `usagesDigest`, `generatedRepoSpecs`, `repoRuleId`, and serialized tag attrs.
- Extended `slug_bzlmod_v2::lockfile` parsing with typed visible
  module-extension entries plus raw `facts` and `factsVersions` maps. This is
  still lockfile-schema parsing only; it does not replay extension results,
  execute repository rules, compute AttributeValues, or implement hidden
  lockfile/error-mode lifecycle.
- No V1 bzlmod implementation was extracted for this checkpoint; the parser
  shape is grounded in the generated Bazel 9.1.1 lockfile oracle.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-shape --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`;
  `rg -n "process-global|fallback scanner|marker trust|std::fs::read" app/slug_bzlmod_v2` returned no matches.
Stage 5 module-extension usage lockfile error checkpoint:

- Added `module-extension-lockfile-error-usage`, a Bazel 9.1.1 oracle fixture
  proving `--lockfile_mode=error` rejects stale module-extension usage replay
  data. The fixture primes `MODULE.bazel.lock`, mutates only the extension tag
  value, then observes Bazel query exit code 7 with the diagnostic that the
  usages of extension `@@//:ext.bzl%ext` changed.
- Added `validate_module_extension_usage_digests` over parsed visible lockfile
  data. The helper compares expected `usagesDigest` values with explicit
  observed digest input and emits the Bazel-shaped stale-usage diagnostic;
  it does not compute digests, execute extensions, read files, or implement
  hidden lockfile/error-mode replay.
- No V1 bzlmod implementation was extracted for this checkpoint; the behavior
  is grounded in the Bazel 9.1.1 oracle fixture.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-error-usage --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.
Stage 5 module-extension implementation lockfile error checkpoint:

- Added `module-extension-lockfile-error-bzl`, a Bazel 9.1.1 oracle fixture
  proving `--lockfile_mode=error` rejects stale module-extension implementation
  replay data. The fixture primes `MODULE.bazel.lock`, mutates only `ext.bzl`,
  then observes Bazel query exit code 7 with the diagnostic that the
  implementation of extension `@@//:ext.bzl%ext` or a transitive `.bzl` changed.
- Added `validate_module_extension_bzl_transitive_digests` over parsed visible
  lockfile data. The helper compares expected `bzlTransitiveDigest` values with
  explicit observed digest input and emits the Bazel-shaped stale-implementation
  diagnostic; it does not compute transitive digests, execute extensions, read
  files, or implement hidden lockfile/error-mode replay.
- No V1 bzlmod implementation was extracted for this checkpoint; the behavior
  is grounded in the Bazel 9.1.1 oracle fixture.
- Validation passed: `cargo fmt -p slug_bzlmod_v2`;
  `CARGO_TARGET_DIR=.codex-cargo-target CARGO_BUILD_JOBS=1 cargo test -p slug_bzlmod_v2`;
  `USE_BAZEL_VERSION=9.1.1 py -3 -B -m tools.v2_oracle run --fixture module-extension-lockfile-error-bzl --tool bazel --bazel C:\ProgramData\chocolatey\bin\bazel.exe --timeout 120`;
  bundled `python.exe -B -m pytest -q -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py`.
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
