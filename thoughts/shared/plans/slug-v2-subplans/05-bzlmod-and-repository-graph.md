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

- `slug-v1-archive:app/slug_bzlmod/src/parser.rs`
- `slug-v1-archive:app/slug_bzlmod/src/dice_graph.rs`
- `slug-v1-archive:app/slug_bzlmod/src/extension_execution_dice.rs`
- `slug-v1-archive:app/slug_bzlmod/src/lockfile.rs`
- `slug-v1-archive:app/slug_bzlmod/src/repo_mapping.rs`
- `slug-v1-archive:app/slug_bzlmod/src/repo_spec.rs`
- `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py`

Each extraction needs an oracle fixture or direct Bazel source citation.
These paths are absent from the active clean root: inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree, not by
searching or importing from the active root. The matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row owns the import
mode, oracle, validation, and residual-risk decision.

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

## Current Priority Hold: Integrate Before Expanding

Stage 5 has substantial landed parser/value/key substrate. Until M5 exact
`aquery` is accepted, add no new standalone bzlmod breadth unless it is the
smallest missing dependency for the shared DICE spine, configured-target
analysis, or a query/cquery/aquery oracle.

The immediate Stage 5 work is integration:

- make module, lockfile, environment, repository mapping, and materialization
  inputs real keys/dependencies in the single daemon-owned DICE graph;
- remove fresh per-request graph construction and scanner/marker ownership;
- feed the resolved repository mapping and toolchain/platform registration
  order into Stage 4 loading and Stage 6 analysis; and
- prove create/edit/delete and command/environment changes in the same daemon.

Existing Stage 5 evidence remains a regression inventory. A `*DiceKey` input
record, parser result, or isolated policy helper is not completion until the
analysis/query path computes through it. Refresh any fixture used for current
acceptance from historical Bazel 9.1.1 to the Stage 1 Bazel 9.2.0 baseline.

### Current prerequisite — `WP-5-m1-bzlmod-runtime-input-oracle` (2026-07-23)

The M1 exit-gate audit returned `REPLAN`: `BzlmodDiceInputs` and
`ResolvedBzlmodGraphDiceKey` are value/identity records, not real DICE
dependencies in `WorkspaceRuntime`, and current Stage 5 fixtures cannot
authorize the bridge because they retain Bazel 9.1.1 evidence.

Refresh only `module-include-change-invalidation`,
`module-root-dev-dependency-visibility`, `lockfile-mode-update-refresh`,
`lockfile-version-error`, `yanked-version-command-env-union`, and
`repo-mapping-canonical-names` at Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Strengthen the include fixture
through edit/delete-error/recreate and the command-policy fixture through
default/ignore/default in one output base. Pin visible lockfile
absence/presence/version/modes, command/environment yanked-policy union, and
root repo-mapping identity with exact diagnostics, manifests, immutable
provenance, and source anchors.

Hidden lockfile ownership is deferred: it belongs to output-base and
module-extension replay and cannot be inferred from visible lockfile rows.
The exact allowlist is those six fixture directories plus compact plan/routing
evidence after acceptance. No Rust, Cargo, dependency, registry networking,
module extension, materialization, repository fetching, cquery/aquery, or
command activation is authorized. Stop on an unpinned source, nonlocal network
dependency, hidden-lockfile claim, or need to broaden semantics. After
independent oracle acceptance, review the exact raw-content/digest input
owners, real DICE keys/equality, Starlark module evaluator boundary, policy
source, minimal resolution value, and same-runtime activation expectations
before implementing a bridge.

## Implementation Slices

### 5.1 MODULE.bazel Evaluation

- Implement root, registry, and non-registry `MODULE.bazel` evaluation through
  starlark-rust with V2-owned Bazel module globals.
- A handwritten directive recorder may support fixture scaffolding only. It is
  not the production evaluator or acceptance evidence, because Bazel compiles
  and executes module files and includes as Starlark.
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
- Add actual DICE `Key` implementations for discovery, MVS resolution, yanked
  versions, registry file hashes, and `source.json` repo specs. Plain
  hash/equality input records are useful key inputs, but are not DICE keys until
  a `DiceComputations` implementation owns their dependencies and invalidation.
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
- Rewrite the V1 thread-local repo-spec registry in
  `slug-v1-archive:app/slug_bzlmod/src/repo_spec.rs` into explicit
  per-evaluation state.
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

Mine `slug-v1-archive:tests/core/bzlmod/test_plan61_guardrails.py` for fixture
themes only: root, local, registry invalidation, included module files,
lockfile writer modes, extension replay, repo mapping, recorded inputs,
materialization markers, and same-daemon generation tests. Do not port exact
V1 counters as truth.

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

Detailed checkpoint evidence for this stage lives in these companion files:

- [05-bzlmod-checkpoint-evidence.md](./05-bzlmod-checkpoint-evidence.md)
  records checkpoints through `c65dedee Stage 5 preserve registry module
  digests`.
- [05-bzlmod-checkpoint-evidence-2.md](./05-bzlmod-checkpoint-evidence-2.md)
  is the active destination for new Stage 5 checkpoint entries.

Keep new entries in the active evidence file so this owner plan and every
companion evidence file remain below the 1000-line cap.

## Exact Test Criteria

- Unit tests cover evaluator results and diagnostics for every directive above,
  including order-sensitive registration lists. Parser round-trips alone are
  scaffold evidence only.
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
- `MODULE.bazel` behavior is produced by starlark-rust evaluation and real DICE
  keys, not a directive recorder or key-shaped value structs.
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
