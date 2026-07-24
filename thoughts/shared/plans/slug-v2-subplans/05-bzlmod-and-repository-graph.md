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

The design returned `REPLAN` after one source-precedence correction. Bazel
9.2's accepted no-dependency update result is not a pre-registry empty
lockfile: it is a 25,647-byte version-28 file containing produced remote BCR
hashes. `refresh` also differs from `update` in registry cache/hash behavior,
not in the visible-only read/write gate. A unified pre-registry read/plan/write
packet would therefore emit fixture-shaped bytes rather than Bazel output and
hit the explicit registry-activation stop.

Proceed serially with `WP-5-m1-visible-lockfile-v28-read`, then
registry/yanked resolution, then a command-owned semantic plan/write packet.
The read packet owns a bzlmod `VisibleLockfileKey` over the injected mode and
conditional neutral workspace-file dependency. `off` does not acquire the
file; update/refresh/error follow Bazel's version-scan-before-JSON precedence.
The first Java-pattern-compatible
`"lockFileVersion":\s*(\d+)` match uses ASCII whitespace/digits and signed
32-bit parsing. Missing/non-28 markers become EMPTY in update/refresh and the
exact unsupported-version error in error mode; a recognized 28 marker then
requires full JSON parsing; overflow and file-read failures remain errors.

`RootModuleGraphKey` computes the visible key only after root/include success
and retains semantic `VisibleLockfileRead` before mapping. Parsed equality
must suppress downstream churn from formatting/key-order-only v28 edits;
absence, stale-version update, and an equivalent empty v28 file compare as
EMPTY. Malformed v28, read errors, and error-mode stale versions block mapping
and `PackageLoadKey` listing/BUILD work, then recover on restoration. Packet A
may edit only bzlmod lockfile/module-eval exports and focused bzlmod,
loading-activation, and core-runtime test sections.

The existing raw-text write planner is not live-authorized: Bazel compares
parsed old/new `BazelLockFileValue`s, while Slug's helper currently compares
rendered text and invents a missing-file error in error mode. Exact desired
registry hashes, update/refresh semantics, typed external-dependency exit 48,
and any post-compute write stay deferred to the next two reviewed packets.
Independent rereview returned `REPLAN` with no remaining blocker to the
bounded read implementation.

Commit `6d354e10` accepts `WP-5-m1-visible-lockfile-v28-read`. A
workspace-scoped bzlmod `VisibleLockfileKey` consumes the fail-closed injected
mode and, except in `off`, the neutral observed `MODULE.bazel.lock` value.
Version 28 and the first Java-ASCII marker/signed-32-bit scan precede semantic
JSON parsing exactly; absence and stale content collapse to EMPTY where Bazel
does, while malformed v28, overflow, read errors, and error-mode stale content
remain failures.

`RootModuleGraphKey` computes root and includes before the visible key, then
mapping. Arc-backed parsed equality prevents formatting/key-order edits,
absence/current-empty/stale-update transitions, and delete/recreate from
dirtying root or package values. Retained loading and runtime regressions prove
the conditional `off` dependency, update→off→update restoration,
module-evaluation reuse, lockfile failure before listing/BUILD observation,
and recovery. Full bzlmod/loading/core suites, formatting, diff/archive
checks, and independent final review passed. No fixture, lockfile write,
registry behavior, or exit-48 claim landed.

The current packet is the read-only
`WP-5-m1-registry-yanked-resolution-design`. Trace pinned Bazel 9.2 registry,
yanked-version, repo-spec, dependency-graph, and lockfile-update owners into
the live V2 registry/digest/policy substrate and parsed visible value. Fix the
real DICE ownership for local/remote registry observations, ordered registry
selection, produced hashes, selected-yanked replay, update/refresh/error
semantics, retained invalidation, semantic equality, and the boundary before
later MVS/extensions and command-owned writing.

The design must explicitly assess whether the accepted
`lockfile-mode-update-refresh` and `yanked-version-command-env-union` fixtures
discriminate every claimed transition. If not, schedule exactly one bounded
Bazel 9.2 oracle correction before Rust. No Rust, Cargo, fixture, expected
output, or lockfile edit is authorized during design; stop on an untracked
filesystem/network read, process-global registry cache, fresh graph, lock-held
DICE compute, pre-registry byte synthesis, or a claim that the existing
raw-text planner is live.

The design returned `REPLAN`, and the corrected independent rereview returned
`ACCEPT`. Pinned Bazel does not have the serial boundary previously implied by
“registry/yanked, then MVS”: ordered registry module discovery produces
`MODULE.bazel` observations first, MVS selects the graph, selected-yanked
evidence and selected RepoSpecs then produce the final selected-yanked and
registry-hash maps, and only a successful resolution can reach lockfile
writing. A pre-MVS packet may therefore return only per-module registry
observations; it must not claim a resolved graph or final lockfile products.

The accepted pair is insufficient. `lockfile-mode-update-refresh` performs no
registry mutation, and `yanked-version-command-env-union` proves only positive
flag/environment union through a local `file:` registry. Older checksum,
selected-yanked, and registry-mutation fixtures remain useful conceptual
evidence, but their Bazel 9.1.1/version-26 or local-registry boundaries cannot
pin remote Bazel 9.2 behavior. File registries use `IGNORE` in every mode and
cannot distinguish remote `USE_AND_UPDATE`, `USE_IMMUTABLE_AND_UPDATE`, and
`ENFORCE`.

Proceed first with exactly one
`WP-5-m1-registry-yanked-lockfile-mode-oracle`. Add a source-controlled
fixture-local loopback HTTP registry and one fixture that, in one workspace
and output base:

- records version-28 hashes and allowed selected-yanked reason A in update;
- changes served metadata A→B, proves update replays A without a metadata
  request, and proves refresh requests and records B;
- records a remote JSON-null absence, proves update preserves the negative
  observation, then changes 404→present and proves refresh retries it;
- corrupts the selected `MODULE.bazel` checksum under error mode while the
  module is disallowed as yanked, proving checksum/discovery failure precedes
  yanked rejection; and
- manifests the unchanged visible lockfile after every failed command plus a
  normalized request log that discriminates reuse from refetch.

The oracle allowlist is `tools/v2_oracle_lib/fixture.py`,
`tools/v2_oracle_lib/runner.py`, one new fixture-local HTTP-service module,
the focused harness test, and one new fixture directory. It may mutate
Bazel-generated lockfile bytes; it must not synthesize the accepted
25,647-byte BCR lockfile. All asserted state and transitions must be
source-controlled; the retained BCR fallback may supply only Bazel's embedded
module closure and must not participate in an asserted transition.

After oracle acceptance, the cycle-free design is:

1. `RootModuleFilesKey` owns only evaluated root/includes and semantic
   `VisibleLockfileRead`. It does not depend on command, environment,
   registry policy, or observation epochs.
2. A downstream discovery key separately consumes root files, normalized
   ordered-unique registry policy, lockfile mode, command/environment policy,
   and demand-driven registry keys. It never depends on `RootModuleGraphKey`
   or `PackageLoadKey`.
3. `RegistryFileKey` owns one exact registry URL observation through a
   non-semantic `RegistryIo` capability installed in DICE computation data.
   The capability may retain only stateless HTTP/file plumbing and a
   content-addressed byte cache; it retains no mode, policy, absence, yanked,
   or selection result. No lock is held across `ctx.compute`, file IO, or a
   network await.
4. Lockfile expectation is typed as `Unrecorded`, `RecordedAbsent`, or
   `RecordedSha256([u8; 32])`. Known hashes are checksum-verified and stable
   across refresh; recorded absence is authoritative in update/error and
   retried in refresh; error rejects an unrecorded required remote file before
   IO.
5. Workspace-local file registries depend on exact neutral file observations.
   External file paths use demanded exact-path observations injected before
   the next request's sole commit. A newly demanded path is read under the
   current request generation, published after compute, and forced to
   recompute before it can persist unobserved. No recursive registry scan is
   authorized.
6. Ordinary remote discovery 404 and transport failure are distinct from
   lockfile-recorded absence and remain retryable request facts. Mutable
   refresh/metadata observations depend on refresh generation; immutable
   known-hash content does not. Yanked metadata failure is typed unavailable
   and fails open. Metadata produces no lockfile hash.
7. `RegistryModuleFileKey(ModuleKey)` tries registries in order, falls through
   only on not-found, evaluates the selected module file, and returns compact
   semantic content, registry identity, and optional observed hash.
   `YankedEvidenceKey` later distinguishes replay, known-source-not-yanked,
   fetched, and unavailable.
8. MVS consumes discovery; selected-yanked and selected RepoSpec keys consume
   the selected graph; final resolution aggregates hashes and mapping; only
   then may `RootModuleGraphKey` expose selected mapping to loading and a
   command-owned writer compare semantic old/new lockfile values.

Values use `Arc`, `CompactString`, compact immutable slices/maps,
`Allocative`, and `Dupe`; SHA-256 is stored as `[u8; 32]`, absence is explicit,
and sorted maps are reserved for canonical digest/JSON boundaries. Equality
compares semantic values, not provenance digests or request generations.

The first post-oracle substrate allowlist is exactly:

- `app/slug_bzlmod_v2/src/registry.rs`;
- new `app/slug_bzlmod_v2/src/registry_dice.rs`;
- `app/slug_bzlmod_v2/src/module_eval.rs`;
- `app/slug_bzlmod_v2/src/lockfile.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- new `app/slug_bzlmod_v2/tests/registry_dice.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- new `app/slug_core_v2/src/runtime/registry_io.rs`;
- `app/slug_core_v2/src/runtime/mod.rs`;
- `app/slug_core_v2/tests/runtime.rs`; and
- `app/slug_core_v2/Cargo.toml`, only for already-workspace HTTP/TLS
  dependencies authorized by the accepted oracle.

CLI/server/`.bazelrc`/`mod` activation requires the separately named transport
packet.

Focused retained evidence must distinguish registry order A→B→A; exact local
create/edit/delete; ordinary 404 from recorded-null absence; remote
404→present only after refresh; known/missing/wrong hashes;
update→refresh→error; lockfile create/edit/delete; metadata A→B replay versus
refetch; command/environment union; and semantic A→B→A reuse.

Hard stops are the old pure MVS helper becoming production, final
selected-yanked/hash aggregation before MVS, synthetic lockfile bytes,
`RootModuleGraphKey`↔resolution cycles, digest-only owners, eager runtime graph
discovery, untracked IO, a global semantic cache, a lock-held compute, old
fixtures treated as pinned proof, or command-surface expansion outside its
named packet.

### Root override routing and owner correction (2026-07-23)

Commit `256c02e2` accepts the Bazel 9.2
`registry-root-override-routing` oracle. Eight retained-daemon rows prove the
registry/non-registry category split, single-version registry/version routing,
ordered root patches and `patch_strip`, fatal patch failure without fallback,
ordered multiple versions, malformed/absent-registry local-path bypass, and
semantic A→B→A replay. Generation, independent replay, source, syntax, diff,
and fresh evidence review passed.

The first owner design reached `REPLAN` before Rust. The sealed override map
and aggregate `RootModuleFilesKey` owner are sound, but frozen Starlark values
have fallible equality and cannot define DICE semantics. The first structural
correction then retained arbitrary integers, exceeding Bazel's exact signed
32-bit `AttributeValuesAdapter` domain and exhausting the packet's correction
budget.

Fresh review accepted
`WP-5-m1-root-module-override-owner-design-correction`, and commit `a5f13bf9`
implements it. The compact aggregate preserves all five override forms, exact
canonical repo-rule IDs, recursive i32-bounded attributes, normalized
single-version patch labels, private stripped file contributions, duplicate
errors, and order-insensitive DICE equality. Focused owner tests passed 12/12,
the full bzlmod crate passed 170/170, and final rereview returned `ACCEPT`.

The design-only `WP-5-m1-registry-module-discovery-design-rereview` returned
`REPLAN`. Registry discovery cannot proceed without an exact root patch-file
input/application owner, while non-registry discovery first materializes the
full local/archive/Git `RepoSpec` and evaluates its MODULE plus includes.
Returning a deferred non-registry bypass would not match Bazel's discovery.

The fixture-only `WP-5-m1-module-source-preparation-oracle` stopped because the
harness cannot inject the copied workspace's absolute URI into MODULE source
or mutation text. Relative archive paths work, but Git resolves its relative
remote from an external-helper directory; `%workspace%` is registry-option
syntax only, and `/proc/self/cwd` is not a portable substitute.

Fresh independent review accepted
`WP-5-m1-oracle-workspace-uri-design`: exact `{{workspace_uri}}` expansion is
limited to copied UTF-8 nonsymlink files and mutation text operands, raw
templates remain in provenance, encoded URIs normalize to
`file://<workspace>`, binary/symlink/outside paths remain untouched, and no
generic unknown-token failure disturbs conditional `{{http_registry}}`.

Commit `de58ba16` implements
`WP-5-m1-oracle-workspace-uri-scope-correction`. Focused harness tests passed
38/38; list, diff, archive, exact-scope checks, and fresh rereview passed after
raw-byte expansion preserved CRLF outside the exact token.

Commit `183970d9` accepts `WP-5-m1-module-source-preparation-oracle`: nine
retained-daemon rows cover patch A→B→error→recovery, local main/include
replay, and deterministic local archive/Git module sources. Generation,
multiple clean replays, source-closure checks, focused harness 38/38, Git fsck,
diff/archive checks, and fresh evidence rereview passed.

The current design-only packet is
`WP-5-m1-module-source-preparation-design`. The canonical plan owns its pinned
source/live-owner audit and no-edit boundary. Discovery Rust remains blocked
until the shared preparation owner is accepted and implemented.

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
