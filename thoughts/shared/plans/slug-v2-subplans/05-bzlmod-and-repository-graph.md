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

### Accepted prerequisite — `WP-5-m1-bzlmod-runtime-input-oracle` (2026-07-23)

The M1 exit-gate audit returned `REPLAN`: `BzlmodDiceInputs` and
`ResolvedBzlmodGraphDiceKey` are value/identity records, not real DICE
dependencies in `WorkspaceRuntime`, and the then-current Stage 5 fixtures
could not authorize the bridge because they retained Bazel 9.1.1 evidence.

Commit `911f16f2` refreshes only `module-include-change-invalidation`,
`module-root-dev-dependency-visibility`, `lockfile-mode-update-refresh`,
`lockfile-version-error`, `yanked-version-command-env-union`, and
`repo-mapping-canonical-names` at Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a`. Strengthen the include fixture
through edit/delete-error/recreate and the command-policy fixture through
default/ignore/default in one output base. Pin visible lockfile
absence/presence/version/modes, command/environment yanked-policy union, and
root repo-mapping identity with exact diagnostics, manifests, immutable
provenance, and source anchors.

Generation and two independent clean replay sets passed all six fixtures.
All pinned source anchors resolve, the visible lockfile records version 28 and
digest `38731963ff6d7df650a7355090c4388b7218e064bc75f839531902dc92f98023`,
normalized output is host-portable, and independent final review returned
`ACCEPT`. Hidden lockfile ownership is deferred: it belongs to output-base and
module-extension replay and cannot be inferred from visible lockfile rows.

### Current design packet — `WP-5-m1-root-module-dice-bridge-design` (2026-07-23)

Perform a read-only design review before any Rust change. Trace live core,
loading, and bzlmod ownership and produce an exact table for:

- raw root/include module content and absence inputs, with create/edit/delete
  equality;
- command-policy, allowlisted environment, and visible-lockfile keys/equality;
- the minimal resolved module and repository-mapping value consumed by
  loading;
- the starlark-rust module evaluator that replaces the handwritten directive
  recorder in production;
- the retained `WorkspaceRuntime` transaction handoff and exact same-daemon
  activation evidence; and
- one bounded implementation allowlist, dependency direction, focused tests,
  exclusions, and stop conditions.

No production, fixture, Cargo dependency, lockfile implementation, registry
network, extension, materialization, repository fetch, cquery/aquery, or
command activation change is authorized. Hidden output-base lockfile ownership
stays deferred. Reject any design that creates a fresh graph/scanner, treats a
digest-only record as the semantic owner, holds a lock across a DICE compute,
or promotes the handwritten parser to production. Independent review must
return `ACCEPT` or `REPLAN` before implementation.

Design result: `REPLAN`. The accepted Bazel oracle is sufficient, and the
future bridge shape is sound: a bzlmod-owned starlark-rust module evaluator and
real DICE key consume raw file values plus normalized request inputs, return an
immutable graph and `slug_identity_v2::RepositoryMapping`, and hand any visible
lockfile write plan to the command boundary after compute. The live file key,
however, is owned by `slug_loading_v2`. A bzlmod dependency on loading followed
by loading's resolved-mapping dependency on bzlmod would cycle; owning the key
in loading would invert Stage 5 semantics.

The current prerequisite is the Stage 2
`WP-2-m1-shared-workspace-file-input-owner` packet. It moves only the existing
file snapshot/value/key into neutral `slug_workspace_v2` with unchanged
equality and loading re-exports. Hidden lockfile, module evaluation, policy,
lockfile version 28, repo mapping, extensions, network/fetch/materialization,
and cquery/aquery remain deferred until that mechanical move is accepted.

Commit `00422fdc` accepts the prerequisite with exact re-exports and unchanged
file equality/compute behavior. The current packet is the read-only
`WP-5-m1-root-module-dice-vertical-final-design`: turn the reviewed bridge
shape into one exact, cycle-free implementation allowlist and acceptance
matrix before editing Stage 5 or command/runtime code.

Final design result: `REPLAN` the six-fixture bridge into serial packets.
Independent review accepts `WP-5-m1-root-module-dice-core` first: a
bzlmod-owned starlark-rust root/include evaluator, fail-closed injected request
values, real file/root graph keys, and an immutable mapping exposed as
`Arc<RootModuleGraph>` on `WorkspaceBuildEvaluation`. It does not yet alter
`PackageLoadKey`; that avoids forcing unmodeled defaults into every standalone
loading transaction while keeping the first packet observable end to end.

Follow serially with `WP-5-m1-root-module-command-daemon-handoff` for request
transport and the loading mapping dependency, then
`WP-5-m1-visible-lockfile-v28`, registry/yanked resolution, and finally
MVS/extension repo mapping. The six accepted fixtures remain the terminal
matrix; no current packet may claim rows whose required owner is still
deferred.

Commit `58e9faa4` accepts `WP-5-m1-root-module-dice-core`. The bzlmod-owned
starlark-rust evaluator implements the authorized Bazel 9 root/include globals
without the handwritten parser, and real module-file/root-graph DICE keys
consume the neutral workspace file input. Workspace-scoped command,
environment, and lockfile-mode values fail closed when absent; the retained
runtime injects them on its existing updater before the sole commit, computes
the immutable graph before package loading, and exposes
`Arc<RootModuleGraph>`.

Focused tests prove root/include present, absent, read-error,
edit/delete/recreate, unchanged reuse, normalized A→B→A with module-evaluation
reuse, breadth-first package includes, root/included/aliased/nodep/dev mapping,
and each missing injected value. Full `slug_bzlmod_v2` and `slug_core_v2`
suites, server/CLI checks, formatting, diff checks, and the archive guard
passed; independent review returned `ACCEPT` after one bounded semantic
correction. The current packet is the read-only
`WP-5-m1-root-module-command-daemon-handoff-design`; it must fix exact request
transport, standalone-loading injection, and the cycle-free loading mapping
edge before any Packet B Rust edit.

The design packet returned `ACCEPT` after one representation correction.
`slug_commands_v2` owns pure flag/environment-value normalization;
`slug_cli_v2` captures `BZLMOD_ALLOW_YANKED_VERSIONS` once per implemented
request; and `slug_server_v2` owns a backward-compatible primitive wire DTO
that normalizes each request without retaining policy in the daemon.
Build/query runtime entry points inject all three fail-closed values on their
existing updater before its sole commit.

The cycle-free production edge is
`slug_loading_v2 -> slug_bzlmod_v2 -> slug_workspace_v2/slug_identity_v2`.
A bzlmod-owned helper performs only the three `changed_to` calls on a caller's
updater and supplies neither defaults nor a commit. `PackageLoadKey` enters,
computes `RootModuleGraphKey` as its first dependency, and only after that
succeeds may package listing or BUILD observation/parsing begin; this holds
even for a BUILD with no `load()`. Loading, analysis, and query tests inject
explicit inputs rather than hiding defaults in a key.

The implementation packet is
`WP-5-m1-root-module-command-daemon-handoff`. Its bounded surface is the
bzlmod injection helper; the loading edge; commands/build/query normalization;
core build/query runtime wrappers; server build/query DTO and daemon handoff;
CLI build/query capture; the necessary loading/server and downstream test-only
Cargo edges; and focused bzlmod/loading/commands/core/server/CLI plus
analysis/query tests. It must prove primitive protocol default/override/default
round trips, same-runtime and same-daemon A→B→A without leakage, request-local
malformed-input recovery, and
`PackageLoadKey -> RootModuleGraphKey -> PackageListingKey/BUILD`.

No fixture refresh, lockfile implementation, registry/network/fetch, yanked
resolution, MVS/extensions, repository materialization, external-repository
load resolution, cquery/aquery activation, run/test transport, second DICE
graph/commit, new DICE key, or semantic output field is authorized. Stop and
replan on a Cargo/DICE cycle, default/environment/filesystem read inside a key,
retained daemon request policy, direct serde on semantic DICE types, required
external mapping activation, or inability to observe transport without public
output expansion.

Commit `3f84e34d` accepts
`WP-5-m1-root-module-command-daemon-handoff`. Build and query now normalize
command/environment/mode values per request, capture the allowlisted
environment once in the CLI, carry a backward-compatible primitive daemon DTO,
and inject all three values on the retained runtime's existing updater before
its sole commit. The daemon retains no semantic request policy.

`PackageLoadKey` computes `RootModuleGraphKey` as its first dependency before
listing or BUILD observation, including load-free BUILD files; standalone
loading, analysis, and query transactions supply explicit inputs. Retained
runtime, package-load, and same-daemon A→B→A regressions prove mapping and all
three request values restore without leakage. Protocol defaults and malformed
command/environment/lock-mode recovery are request-local. Focused bzlmod,
loading, commands, core, server, analysis, query, and CLI validation passed,
as did formatting, diff, daemon cleanup, and archive checks; independent final
review returned `ACCEPT` after the one permitted evidence correction.

The current packet is the read-only
`WP-5-m1-visible-lockfile-v28-design`. It must bind the accepted Bazel 9.2
visible-lockfile rows to the live parser/renderer/planner substrate, neutral
workspace-file observation, root-graph dependency, request modes, and a
command-owned atomic write plan. No Rust, fixture, Cargo, or lockfile edit is
authorized until the design fixes exact ownership/equality, same-daemon
transitions, an implementation allowlist, and stop conditions. Hidden
output-base lockfiles, registry-produced hashes/fetching, yanked selection,
MVS/extensions, materialization, external loading, cquery/aquery, and run/test
remain deferred.

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
