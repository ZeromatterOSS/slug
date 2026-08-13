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

Fresh review accepted the corrected
`WP-5-m1-module-source-preparation-design`. Stable source-file keys depend on a
stable materialization key; local repositories remain live exact-file views,
while fixed archive/Git sources retain immutable operational roots behind
generation-independent semantic equality. Registry patch preparation remains
a separate serial owner.

Commits `9c2a6814` and `0445cafd` accept the two serial source-preparation
owners. Raw/local/immutable materialization and registry/non-registry
MODULE-byte routing with ordered root patches are now DICE-owned.

The subsequent
`WP-5-m1-registry-module-discovery-design-rereview` returned `REPLAN` before
Rust. `ModuleSourcePreparation` currently returns only bytes, so a discovery
consumer cannot preserve Bazel's selected registry and ordered
URL-to-SHA-or-absence evidence without illegally duplicating registry
iteration. The corrected boundary widens preparation success to compact
registry/non-registry provenance and keeps discovery dependent on that sole
owner.

Existing oracles also do not directly discriminate registry include rejection
after compile but before execution, execution before declaration validation,
or name before version. The current packet is the five-row, fixture-only
`WP-5-m1-nonroot-module-evaluation-ordering-oracle`; the canonical plan owns
its exact allowlist and stop conditions. No discovery Rust is authorized until
that evidence and a fresh implementation rereview are accepted.

Commit `51bfc915` accepts that oracle. The original six local replay rows and
their normalized records remain unchanged; five isolated validation rows now
pin include → execution → name → version → success under root-version
invalidation. Generation, independent replay, source/diff/archive checks, and
fresh evidence review passed.

The current packet is the read-only
`WP-5-m1-registry-module-discovery-implementation-rereview`. It must either
accept or replan the frozen five-file key/provenance/evaluator design before
any discovery Rust.

That rereview returned `REPLAN`. The live `ModuleFileEvaluation` is not a
Bazel-shaped `InterimModule`: it omits compatibility fields, max-compatibility
and distinct nodep/original deps, registrations, extension usages, flag
aliases, built-in collision behavior, and nonroot dev-dependency suppression.
The eleven-row oracle pins evaluation ordering but not those values.
Preparation also drops ordered exhaustion attempts and flattens fatal registry
causes to strings.

The current packet is the read-only
`WP-5-m1-nonroot-interim-module-oracle-design`. It must design complete
observable Bazel 9.2 evidence and the serial evaluator/schema → typed
preparation provenance → discovery split. No dependency-only discovery subset
is authorized.

Fresh review accepted a three-fixture serial design. Graph/repo-mapping,
extension usage/generated repos, and toolchain/platform/flag consumers have
different observability and stop conditions and must not be combined into one
fixture. Hidden Bazel 9 no-op constants remain pinned-source-backed structural
owner tests rather than false graph claims.

The current packet is only
`WP-5-m1-nonroot-interim-module-graph-oracle`. The canonical plan owns its new
fixture-directory allowlist and exact retained rows. No Rust or later
extension/consumer fixture work is authorized in that packet.

Commit `908c7c62` accepts the six-row graph/repo-mapping fixture. Three Bazel
9.2 runs, exact mapping/output inspection, source/closure/diff/archive checks,
and fresh evidence review passed without claiming hidden no-op constants.

The current packet is only
`WP-5-m1-nonroot-module-extension-semantics-oracle`. It owns one new fixture
directory and must stop rather than widen the harness or rely on unstable
extension output. The consumer fixture and all Rust remain deferred.

Commit `8824135a` accepts the five-row extension-semantics fixture. Three
Bazel 9.2 runs, exact detailed/aggregate extension output, deterministic
archive and local-closure checks, generated-repository build evidence, and
fresh evidence review passed. The rows distinguish isolated flag gating,
ordered nondev tags/imports, suppressed dev usage, ignored nonroot
override/inject redirections, and duplicate-import collision.

The current packet is only
`WP-5-m1-nonroot-module-consumers-oracle`. It owns one new local-only fixture
for host-independent nonroot platform/toolchain registration, independent dev
suppression, and globally consumed flag aliases. All Rust remains deferred
until this third serial fixture is accepted.

Commit `eeea40a6` accepts the eight-row consumer fixture. Three Bazel 9.2
runs, exact per-command manifests, pinned-source and local-closure checks, and
fresh evidence review passed. Custom constraint values exclude the default
host, each successful action reads the resolved `ToolchainInfo`, the two dev
registration fields fail and recover independently, and the shared
`compilation_mode` alias produces exact subject → root → subject markers.

All three complete-nonroot evidence fixtures are now accepted. The current
packet is the read-only
`WP-5-m1-nonroot-interim-module-evaluator-schema-design`. It must freeze the
complete compact value and supplied-byte Starlark seam before any Rust.
Typed preparation provenance and stable discovery composition remain later
serial owners.

Fresh independent review accepts that design after one correction: extension
tag and innate repo-rule attributes retain arbitrary-precision Starlark
integers, while the existing root override owner remains exactly i32-bounded.
The evaluator value therefore uses canonical heap-independent small/large
integers and remains distinct from the later provenance-bearing interim
module. Supplied includes cross the seam in BFS order, are compiled before
execution, and later execute inline with isolated bindings and one shared
evaluator-local semantic context. Logical source IDs and spans participate in
equality; bytes, physical paths, registry provenance, attempts, IO, and DICE
do not.

Commit `c663fe46` accepts that implementation. The new evaluator-owned compact
schema preserves every reviewed semantic field, opaque canonical
arbitrary-precision integers, exact `-1` nodep dependencies, singleton
`bazel_tools` finalization, and logical source spans. The supplied-byte
MODULE inspector enforces the restricted dialect and exact direct-include
classification without retaining bytes or physical paths. Focused nonroot
tests, existing root evaluator tests, all `slug_bzlmod_v2` tests, formatting,
diff checks, and two fresh independent reviews passed.

The current packet is the read-only
`WP-5-m1-nonroot-directive-evaluator-design`. It must freeze only the
single-supplied-file directive evaluator, exact validation/mutation ordering,
dynamic extension proxy/tag behavior, dev suppression, and public
starlark-rust seam. Include composition, preparation provenance, and discovery
remain later serial owners; no Rust is authorized before fresh design
acceptance.
That design returned `REPLAN` before Rust. Pinned `TagCallable` retains raw
Starlark kwargs directly in `AttributeValues`; the adapter-backed
`None`/bool/int/string/label/iterable/dict domain is only the serializable
subset. A confined Bazel 9.2 nonroot probe accepted both `3.14` and the builtin
`print` callable through MODULE evaluation, then rejected each during later
string-tag validation with its exact value and Starlark type. Rejecting either
inside the directive evaluator would change the phase and diagnostic, while
the current compact schema cannot retain them.

The current packet is the read-only
`WP-5-m1-nonroot-raw-attribute-oracle-design`. It must define bounded
network-free evidence for supported, deferred-invalid, nested, cyclic, tag,
and innate repo-rule attribute values and then decide whether a compact
heap-independent deferred-invalid representation is sound. No schema or
directive-evaluator Rust is authorized before that evidence design is freshly
accepted.

That design is accepted. Bazel retains raw kwarg references through the end of
MODULE evaluation, validates ordinary tags in `TypeCheckedTag` and innate tags
in `RepoRule.instantiate`, and separately hashes usages through the narrower
`AttributeValuesAdapter`. Retaining live or frozen Starlark values across DICE
is rejected. The only future candidate is a post-file heap-independent
snapshot: exact structural nodes for later-valid values and bounded
deferred-invalid nodes only where the oracle proves stable, identity-insensitive
failure. Snapshotting at the tag call is forbidden because later list/dict
mutation remains visible.

Commit `cffc39b0` accepts the 12-row local retained-daemon raw-attribute
oracle. Two success rows and ten failure rows pin post-call list/dict mutation,
ordinary float/callable/proxy/nested/cyclic validation, innate
float/callable/nested validation, and update-mode adapter hashing before tag
schema validation. Independent and root Bazel 9.2 semantic replays passed; the
prime and final failed update retain the same 25,883-byte visible lockfile
digest. Exact source anchors, archive status, unique never-successful outputs,
mutation restoration/version sequence, source closure, and credential checks
passed. The fixture adds 15 files/1,183 lines, taking the recorded accepted
fixture tree from 1,231/27,626 to 1,246/28,809; it is packet one after the
fixture-hygiene checkpoint. Its copied six-file/50-line platforms module is
required for fixture-local override isolation, and no redundant or
nondiscriminating material was established. Harness pytest remains unavailable
because the system Python lacks `pytest`; this did not weaken the two executable
Bazel replays.

The current packet is the read-only
`WP-5-m1-nonroot-deferred-attribute-snapshot-design`. It must freeze the
post-file, heap-independent raw-kwarg snapshot and the compact
deferred-invalid nodes required by the accepted oracle, including structural
sharing/cycles, semantic equality, identity-free diagnostics, ordinary versus
innate later validation, and update-mode adapter ordering. It must not retain a
live/frozen evaluator heap, generalize unsupported opaque values, edit Rust,
add DICE/public seams, activate directive evaluation, compose includes, or
change preparation/discovery ownership.

Fresh pinned-source, live-schema/utility, and independent architecture reviews
accept the bounded design. Raw kwargs remain evaluator-local through successful
file execution and are then copied into the compact retained value. Bazel's
`ListType`/`DictType` conversion allocates fresh containers, so acyclic alias
topology is not semantic after final contents are observed; list/tuple identity
and tag order remain semantic, while dict equality ignores insertion order.
Adapter-compatible evaluation hashing separately preserves kwargs/dict
iteration order and omits locations. The accepted one-element self-list is an
explicit lockfile-off diagnostic form, not a general cycle graph; distinct
cycles, other opaque values, and callable/proxy/cycle update/error modes remain
unsupported. Existing `SmallMap`, `CompactString`, `Arc`, `Allocative`, and
evaluator-local `ValueIdentity` are sufficient; no new interner, retained heap,
DICE owner, or cross-crate seam is justified.

The current packet is only
`WP-5-m1-nonroot-deferred-attribute-snapshot-schema`. It may edit
`interim_module.rs`, `module_eval.rs`, and `tests/nonroot_module_eval.rs`.
Replace the conflated iterable with distinct list/tuple forms, extend compact
keys/values only for the proven deferred-invalid tokens, add a private
post-execution snapshot helper and ordered adapter projection, and test final
mutation, alias-insensitive equality, dict order versus adapter order,
identity-free invalid values, exact self-cycle rejection boundaries, and
adapter-before-schema failure. Do not edit `lib.rs`, activate directives,
compose includes, or touch DICE, Cargo, parser, lockfile, preparation,
discovery, command, server, or oracle files.

Commit `d4fb5d65` accepts that packet. The retained tree now distinguishes
lists and tuples, keeps ordered `SmallMap` dictionaries with structural
order-insensitive equality, and admits only the exact oracle-proven float,
builtin `print`, extension proxy, float-key, and one-element self-list
deferred forms. A private evaluator-local walker copies final mutated contents
through transient `ValueIdentity`; a separate location-free adapter projection
preserves kwargs/dict order and rejects exact float and out-of-i32 integer
values before later schema validation. Other floats, callables, opaque values,
keys, and cycle shapes fail closed. Focused snapshot 6, public schema 9, full
`slug_bzlmod_v2` 195, formatting, and diff checks passed. Independent review
required and then accepted exact adapter integer bounds, arbitrary-precision
snapshot coverage, nested ordering, allocation accounting, and negative
identity/cycle/key tests. The helper remains intentionally uncalled until the
directive evaluator owns the source identities and post-file boundary.

The current packet is read-only
`WP-5-m1-nonroot-directive-evaluator-redesign`. Revisit the previously
replanned single-supplied-file evaluator now that raw-value evidence and the
bounded snapshot exist. Freeze exact globals, evaluator-local proxy/usage/tag
state, validation and mutation order, nonroot dev suppression, finalization,
source-identity plumbing, and adapter-versus-schema phase ordering. Do not edit
Rust, compose includes, add DICE/public seams, change preparation/discovery,
or implement lockfile hashing. Stop for a new oracle if any value or cycle
outside the accepted bounded domain is required.

Fresh pinned-source, live-API, and independent reviews accept the corrected
design. A private context keeps only compact draft metadata and syntax-
inaccessible root names. Each source-visible extension proxy and raw kwargs
dict is stored directly in a dynamically appended
`"\0slug:nonroot:..."` module slot: `Module::set` does not call `export_as`,
compiled source slots are fixed before execution, every module slot is traced
by GC, source cannot spell a NUL/colon identifier, and `Module::get` rereads
fresh post-GC values. The evaluator therefore needs neither disabled GC,
unsafe lifetime erasure, a frozen heap, nor a new public owner.

The current packet is only `WP-5-m1-nonroot-directive-evaluator` and may edit
`module_eval.rs`. Implement the complete single-file nonroot globals, including
all five module overrides as validation-then-discard, exact dev suppression,
ordinary/isolated/no-op extension proxy and import behavior, redirection
no-ops, innate repo-rule calls, source spans, final snapshot, and builder
finalization. Ordinary tag and repo-rule dynamic callables reject positionals;
innate `name` is appended after extra kwargs. Register only the exact `print`
extension, with invocation failing closed through a private handler while its
identity remains snapshot-able. Fresh identities include every source-visible
module-extension proxy but exclude the distinct repo-rule proxy. Tests must
force GC between execution and reread and prove final mutations, identities,
order, errors, and the complete compact result. Do not edit any other file,
compose includes, validate later schemas, hash/write lockfiles, or add
DICE/public/preparation/discovery ownership.

Commit `b738547d` accepts that implementation. The private one-file evaluator
uses syntax-inaccessible `Module` roots, forced-GC rereads and fresh
identities, the exact compact directive surface and ordering, normalized
versions/labels, nonroot dev and collision behavior, isolated and innate
usages, exact logical spans, and post-file bounded snapshots. Focused 9/9 and
all 204 `slug_bzlmod_v2` tests passed with formatting, diff, archive, and fresh
independent final review `ACCEPT`.

The current packet is the read-only
`WP-5-m1-nonroot-include-composition-design`. It must freeze only supplied-file
closure compilation and inline include execution over the landed inspector and
private evaluator, including isolated bindings, shared semantic state,
multi-Module rooting/reread, repeated/cyclic include behavior, source spans,
and the registry-include ordering boundary. Typed preparation provenance,
discovery composition, DICE, IO, public activation, and Rust edits remain
deferred.

That design returned `REPLAN` before Rust. Pinned Bazel executes every compiled
fragment on one `StarlarkThread`, while the feasible private starlark-rust seam
would use nested `Evaluator`s with distinct call stacks. Starlark-rust also
performs scope compilation inside `eval_module`, so parsing the supplied
closure alone does not prove Bazel's compile-all-before-execution ordering.
Exact repeated raw labels reuse one stored file `Module`; multi-heap rooting
and final reread remain viable. Bazel's BFS has no explicit include-cycle
termination or diagnostic, so Slug may not invent one.

The current packet is the read-only
`WP-5-m1-nonroot-include-composition-oracle-design`. It must freeze a bounded
nonregistry characterization for nested runtime diagnostics, later-fragment
compile-before-execute ordering, repeated-label binding/execution behavior,
and successful inline order, plus a separate hard-timeout Bazel cycle probe.
No fixture, Rust, public API, provenance, discovery, or DICE edit is authorized
before fresh review.

That design is accepted. The next packet is only
`WP-5-m1-nonroot-include-composition-oracle`, with the allowlist
`tests/v2_oracle/fixtures/nonroot-include-composition/**`. A self-contained
local-path subject module must produce one extension-generated marker proving
`outer-before|nested-a|outer-after|repeat-a|repeat-a`, then retain one Bazel
daemon across direct nested-fragment A→B→A edits, a nested runtime failure, a
later-fragment scope failure that precedes an earlier invalid directive, and a
final recovery. The runtime row must retain Bazel's observable include-parent
stack/location; stop if that frame is not stable enough for a narrow shape.
Identical raw-label execution is black-box evidence, while reuse of one stored
`CompiledModuleFile` and predeclared `Module` remains a pinned-source invariant
because the restricted MODULE language cannot distinguish it from a fresh
module.

The cycle probe remains outside `v2_oracle`: run Bazel 9.2 in `--batch` mode
against a temporary root→A→B→A workspace, with a fresh output root and a hard
process-group timeout, and record the non-normative result only in the handoff.
No existing fixture, harness, Rust, Cargo, DICE, preparation, discovery,
command, server, lockfile, or Slug replay is authorized.

Commit `203cdaac` accepts that oracle. Six retained-daemon rows pin the exact
A/B marker digests, same-daemon A→B→A direct fragment invalidation, the
root→outer→nested runtime traceback and call sites, later-fragment scope
compilation before an earlier invalid directive, and final recovery. The
fixture adds 12 files and 438 lines, below the growth-review threshold. A
separate confined Bazel probe timed out after 10 seconds without an include
cycle diagnostic; it is source/behavior characterization only.

The current packet is read-only
`WP-5-m1-nonroot-include-composition-design-rereview`. It must replace the
rejected nested-evaluator seam with an exact common-call-stack,
compile-complete-closure-before-execution design over supplied files. Preserve
per-file bindings, shared semantic state, exact raw-label-keyed stored
Program/Module reuse, file spans, multi-heap rooting/reread, and registry
ordering. Inspect supported starlark-rust APIs or a bounded upstream seam and
return `REPLAN` if exact behavior is infeasible; no diagnostic divergence or
invented cycle rejection is acceptable.

That rereview is accepted. The implementation packet is only
`WP-5-m1-nonroot-include-composition`, with the three-file allowlist
`starlark-rust/starlark/src/eval.rs`,
`starlark-rust/starlark/src/eval/compiler/module.rs`, and
`app/slug_bzlmod_v2/src/module_eval.rs`.

The upstream seam owns an opaque module-bound prepared program: exact
`ModuleScopes` resolution, reusable load-free top-level bytecode, root
execution, and same-evaluator nested execution in a different Module without a
second sentinel. Module environment, `DefInfo`, current frame, scoped GC
suspension, and current-file state restore before success or error crosses the
native include frame. Cross-heap automatic GC is suspended only while a foreign
Module is active; after evaluation each Module is collected and reread
independently from its hidden slots.

Slug owns only supplied-file horizon composition, exact raw-label-keyed
prepared Module reuse, per-file bindings and spans, one compact Value-free
semantic state, file-indexed roots, repeated inline execution, and a typed
include-site/leaf diagnostic adapter. Cycles retain Bazel's nonterminating
horizon behavior and gain no visited set or diagnostic. Add exact upstream and
Slug regressions first; no Cargo, public consumer, DICE, preparation,
discovery, command, server, fixture, expected-artifact, or lockfile edit is
authorized.

The implementation packet returned `REPLAN` with no Rust retained. A compiling
module-bound `PreparedModule<'v>` prototype proved preparation and reusable
execution, but the private native `include()` could not safely recover a
context containing that program through `Evaluator.extra`: its independent
`AnyLifetime<'e>` cannot prove the invariant evaluator value lifetime `'v`.
The downstream check reported the explicit-lifetime, invariance, and context
drop-order failures. Unsafe lifetime erasure, self-referential context storage,
and a speculative upstream-only API were rejected; all three implementation
files were restored to `HEAD`.

The current packet is read-only
`WP-5-m1-nonroot-include-dispatcher-design`. Design the smallest
evaluator-owned, lifetime-coupled prepared-program registry/dispatcher so a
native callback passes only an exact raw key or opaque index and never
downcasts a prepared program from `extra`. Freeze the required runtime
allowlist and exact upstream/downstream tests while retaining the accepted
compile-first, common-stack, scoped-GC, restoration, diagnostic, repeated-key,
and cycle-horizon contracts. No Rust or other production edit is authorized.

That replacement design is accepted. The implementation packet is only
`WP-5-m1-nonroot-include-dispatcher`, with the four-file allowlist
`starlark-rust/starlark/src/eval.rs`,
`starlark-rust/starlark/src/eval/compiler/module.rs`,
`starlark-rust/starlark/src/eval/runtime/evaluator.rs`, and
`app/slug_bzlmod_v2/src/module_eval.rs`.

The evaluator borrows a one-shot `&'a [PreparedModule<'v>]` registry. The app
keeps only exact raw-label-to-opaque-index and logical-file metadata in its
Value-free `extra` context. The dispatcher copies the external slice reference
before mutable execution, while modules, prepared storage, and the execution
evaluator are declared in safe drop order. A separate preparation evaluator
must finish exact closure-wide scope resolution/bytecode compilation before
the one execution evaluator runs root or included effects. Preserve every
previous common-stack, scoped-GC, restoration, per-file root, diagnostic,
repeated-key, finalization, and cycle-horizon constraint.

That implementation attempt also returned `REPLAN` with no Rust retained.
`Option<&'a [PreparedModule<'v>]>` necessarily implies `'v: 'a`; the current
evaluator deliberately has no such relation, and the focused starlark check
failed in existing optimizer code through mutable lifetime invariance. Adding
that bound or editing the optimizer would exceed the reviewed allowlist.

The corrected owned-dispatcher design is accepted. The current implementation
packet is `WP-5-m1-nonroot-owned-include-dispatcher` under the same four-file
allowlist. `Evaluator<'v>` owns one `Rc<[PreparedModule<'v>]>`, installed by a
one-shot setter that consumes the prepared vector. Opaque-index dispatch dupes
that single `Rc`, ending the field borrow before mutable execution. This
introduces neither the hidden `'v: 'a` bound nor per-program allocation, and
each program remains safely bound to fixed external Modules for `'v`. All
other preparation, stack, GC, restoration, app-state, diagnostic, finalization,
and cycle-horizon constraints remain unchanged.

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

### 5.6A Repository semantic owner, materializer, and remote reuse contract

Preserve a strict boundary between repository semantics and physical
realization as Stage 5 broadens:

- bzlmod and repository-rule producers own module/repository identities,
  canonical mappings, semantic descriptors, recorded inputs, reproducibility,
  immutable repository views, and lockfile participation;
- repository materializers own archive/Git/local/rule realization, durable
  roots, manifests, atomic publication, sparse physical projections, and
  recovery; and
- Stage 7 cache/CAS clients may accelerate physical realization but never own
  semantic repository truth.

A filesystem path, cache hit, marker file, materialized directory, or cache
availability is not repository identity. Repository-rule keys structurally
include the producing `.bzl` closure, canonical attrs, mapping, declared
environment policy, watched file/directory/tree observations, process effects,
downloads, and every other admitted semantic input. Unmodeled inputs fail
closed.

The evaluated `.bzl` producer should remain the natural owner of source
selection, canonical identity, containing package, direct load resolution,
child demands, evaluation, exports, and compact semantic facts. Parse trees,
bytecode, evaluator heaps, and callable values are scratch or lifetime state,
not independent semantic authorities. Before adding another bzl source/load
key family, audit whether the existing evaluated-module key and narrow
projection can own the fact without merging Host and external compatibility
classes incorrectly.

#### Deferred sparse remote repository-output cache

After generated-repository execution, exact recorded inputs, manifests, and
atomic physical publication are accepted, design a cross-process/workspace
repository-output cache using REAPI ActionCache and CAS where practical.

The cache contract must:

- be a physical accelerator, never a DICE key or semantic authority;
- authenticate a reproducible repository invocation and its ordered typed
  recorded inputs;
- revalidate current observations before accepting a hit;
- bound lookup traversal, marker bytes, tree entries, demanded control bytes,
  and alternatives;
- retain metadata and fetch `MODULE.bazel`, included module files, `.bzl`,
  `.scl`, `REPO.bazel`, and BUILD files only as their semantic producers demand
  them, leaving ordinary source bytes in CAS until physical demand;
- treat cache miss, missing CAS data, transport failure, malformed records,
  mutation/reversion, and stale observations as explicit miss/rejection paths
  with no semantic corruption;
- publish a complete physical root atomically while preserving the prior
  accepted generation on failure; and
- prove cancellation, retry, cutoff, eviction, and shutdown release of sparse
  and complete retained owners.

Bazel has an experimental remote repository contents cache. Claim **exact**
interoperability only after Slug reproduces the pinned Bazel 9.2 initial
identity, ordered observation hashing, Action/ActionResult shape, marker, and
Tree validation. Until then, any implementation and cache namespace are
explicitly **Slug-native** while repository semantics, content digests, and
recorded-input validation remain exact for Slug's admitted graph.

The required mutation/reversion, alternative-input, missing-CAS, retry,
dependent-materialization, sparse-control-file, and symlink fixture themes are
owned by Stage 1's Zabel-derived Wave B backlog. This section is a future
design constraint and does not widen the active module-extension packet.

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
  records checkpoints through the accepted repository materialization
  request/result design.
- [05-bzlmod-checkpoint-evidence-3.md](./05-bzlmod-checkpoint-evidence-3.md)
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
### Built-in bazel_tools repository-owner entry (2026-08-11)

`RootRepositoryRouteKey` currently owns only direct `local_path_override`
routes backed by `RepoSpec`; root mappings merely reserve
`bazel_tools -> bazel_tools`. Design next one structurally distinct immutable
`BuiltinBazelTools` route/source owner with versioned manifest identity,
checked-in verbatim bytes, canonical `@@bazel_tools` routing, exact SHA-256,
and no Host/Bazel-install/network/workspace selection. Package evaluation and
the embedded MODULE dependency graph remain deferred.

### Built-in source-kind implementation REPLAN (2026-08-11)

The first owner implementation was discarded after final review: a
snapshot/path-only source-file key could not satisfy the frozen distinct
directory/wrong-kind contract, and the sole correction had already been used
by the pre-`RepoSpec` Host guard. Design the typed kind/error algebra before
retrying Rust or verbatim assets. No implementation from the failed packet is
retained.

### Built-in bazel_tools repository/source owner accepted (2026-08-12)

The accepted file-only owner adds a structurally distinct
`RootRepositorySource::BuiltinBazelTools` route after the root carrier
succeeds. Its versioned partial catalog owns seven verbatim Bazel 9.2 archive
files, exact SHA-256 and executable state, a domain-separated manifest, and
typed invalid-path, wrong-directory-kind, unsupported-catalog, and integrity
terminals. The source key has immutable complete-only DICE equality/validity.

Both Host materialization paths fail before `repo_spec()`; no Host
observation, install scan, runtime source choice, package evaluation, or
consumer dispatch is admitted. The active cross-stage closure design must
freeze the complete embedded test-tools dependency graph before widening the
catalog or routing existing package/Bzl consumers.


### Built-in MODULE injection design REPLAN (2026-08-12)

Pinned Bazel 9.2 injects `bazel_tools@<empty>` into every other module,
reserves its apparent name, and lets a root/user command override replace the
default non-registry sentinel. The embedded MODULE then participates in
ordinary discovery/MVS. Only after selection does Bazel derive canonical and
full mappings, collision-disambiguated extension names, repo overrides, and
registration consumers.

Slug's compact `EvaluatedNonrootModule` can represent the complete embedded
MODULE, but no Host discovery/MVS owner yet joins it to root, registry, and
direct-nonregistry inputs. The legacy supplied-file `ResolvedGraph` cannot
become a second production graph. Full injection therefore ends `REPLAN`;
partial dependency selection, fabricated RepoSpecs/mappings, and a synthetic
root-only merge are rejected.

Implement next only one callerless built-in module value over the exact source
key and existing complete evaluator. Retain route-manifest identity, MODULE
SHA-256, and the complete evaluated value; defer all injection, selection,
mapping, lockfile, package/Bzl, configured toolchain, and command behavior.

### Built-in bazel_tools module value accepted (2026-08-12)

Commit `3bc745de` accepts the crate-private callerless module-value key. It
computes only the exact embedded `MODULE.bazel` source key and existing
complete nonregistry evaluator, retains distinct route-manifest and source
SHA-256 identity plus the full `EvaluatedNonrootModule`, and proves cold/warm
DICE reuse with no event data. Exact dependency aliases/versions, nodep set,
extension usages/imports/innate tag, four-item toolchain order, no self edge,
typed terminals, equality, caps, full crate/downstream checks, and independent
review passed. No root, override, registry, lockfile, discovery, selection,
mapping, package, command, or consumer edge landed.

Run next only `WP-5-builtin-bazel-tools-selected-graph-owner-design`. Freeze
the sole future Host discovery-to-MVS key and selected-graph value, including
default sentinel precedence and explicit override bypass. If the accepted
root, registry, direct-nonregistry, and embedded owners cannot compose exactly,
return `REPLAN` into the first missing prerequisite. No Rust, legacy
`ResolvedGraph` activation, fabricated RepoSpec/mapping, root-only merge, or
consumer work is authorized.

### Selected Host graph owner design REPLAN (2026-08-12)

The reviewed source audit ends `REPLAN` before Rust. Slug has no uniform
per-module discovery value: the embedded leaf retains a complete module and
immutable route/hash identity; registry preparation retains bytes, selected
registry, and ordered URL/SHA-or-absence attempts but no evaluated semantic
module; and the private direct-local evaluator retains a module/route only for
a main-repository-visible `local_path_override`. Normalized command override
state is absent. The legacy handwritten `ResolvedGraph` remains an inexact
second graph.

The first missing prerequisite is
`WP-5-host-discovered-module-owner-design`. Freeze one crate-private
workspace/module-key leaf that computes root files first, bypasses the
embedded key for every explicit `bazel_tools` override, and returns the
complete evaluated module paired with built-in or ordered selected-registry
provenance. Admit only unoverridden `bazel_tools@<empty>` and versioned
registry modules. Nonregistry and command override discovery, recursion/MVS,
post-selection mappings/extensions/registrations/RepoSpecs/yanked/hashes,
lockfile writing, package/Bzl, configured toolchains, commands, Test, and
execution remain fail-closed/deferred. No Rust is authorized until this
smaller design is independently accepted.

### Host discovered-module owner design accepted (2026-08-12)

Independent review accepts a single-file built-in/registry-only leaf.
Root files and explicit override category are computed before embedded lookup;
versioned registry preparation supplies selected bytes and complete ordered
attempt/hash provenance to the existing evaluator. Nonregistry, command
override, recursion/MVS, post-selection, and consumer breadth remain
fail-closed/deferred. Implement next only
`WP-5-host-discovered-module-owner-implementation` in
`source_preparation.rs` under the canonical packet's caps and stops.

### Host discovered-module owner accepted (2026-08-12)

Commit `e7e4a772` accepts the callerless embedded/registry module leaf.
Root-first classification bypasses embedded evaluation for explicit overrides;
successful values retain complete evaluation plus immutable built-in or
selected-registry ordered attempt/hash provenance. A real-DICE test proves
override bypass, registry A/B/A restoration, cold captured evaluation, and
warm reuse. Focused 4/4, the full crate, downstream checks, formatting, caps,
and independent review passed. No graph or consumer landed.

The remaining first gap is general nonregistry discovery. Existing direct-local
evaluation is rooted in a main-repository apparent name and accepts only direct
`local_path_override`; it cannot represent a transitive override target,
archive/Git, or other RepoSpec. Run next only
`WP-5-host-nonregistry-discovered-module-owner-design`. Audit the sole
materialization/include/evaluation identity for admitted nonregistry shapes and
return `REPLAN` at the first missing prerequisite. Command override
normalization and recursive discovery/MVS remain later. No Rust is authorized
before independent design acceptance.

### General nonregistry discovery design REPLAN (2026-08-12)

The reviewed audit ends `REPLAN` before Rust. General root RepoSpec
materialization and root MODULE byte ownership already exist by workspace and
module name, and the complete evaluator accepts a supplied closure. The
missing bridge is closure preparation: all accepted direct-local inspection,
package-horizon, and fragment owners enter through
`RootRepositoryRouteKey` and a root apparent name. That cannot identify a
transitive override or preserve immutable archive/Git materialization without
invented routing.

Run next only `WP-5-host-nonregistry-module-closure-design`. Freeze a
route-independent closure key over the exact module key and root RepoSpec,
reusing the sole materialization/source/package-policy owners for root and
included files. Local and immutable identities, Need/error/order semantics,
and complete closure equality must remain structural. Evaluation, command
overrides, discovery/MVS, mappings, consumers, and Rust remain deferred until
independent acceptance.

### Route-independent MODULE closure design REPLAN (2026-08-12)

The reviewed closure audit ends `REPLAN` before Rust. Root and fragment
source bytes can use the existing route-independent materialization/source
owners, but exact include package preflight cannot. Every accepted external
package, repository-ignore, REPO.bazel, and marker-path key carries
`RootRepositoryRoute`; the root package boundary is main-repository-only.
Fabricating a route would corrupt transitive override identity, while direct
source reads would duplicate policy/lookup ownership.

The first missing prerequisite is
`WP-5-host-nonregistry-package-preflight-design`: freeze one
route-independent package-policy and BUILD-marker owner over workspace,
module key, root RepoSpec, and package path, using only
`RepositorySourceFileKey`. Preserve REPO.bazel, .bazelignore,
BUILD.bazel-before-BUILD, source kind/Need/error ordering, and local/immutable
invalidation. Root deleted-package policy needs special care because final
canonical repository identity is post-MVS; do not guess `name+` or a
multiple-version suffix. Closure preparation, evaluation, command overrides,
selection, mappings, and consumers remain deferred. No Rust is authorized
before independent design acceptance.

### Nonregistry package-preflight design accepted (2026-08-12)

Independent review accepts the route-independent package-policy and
BUILD-marker design. Implement next only
`WP-5-host-nonregistry-package-preflight-implementation`: crate-private REPO
and ignore keys reuse `RepositorySourceFileKey`; one package-preflight key
preserves invalid/ignore/BUILD.bazel/BUILD/no-marker order and fails closed
before source work for every nonempty canonical deleted-package set. This
admits no guessed preselection canonical repository identity. The five-file
Rust allowlist, caps, lifecycle/error/event proof, and stops are in the
canonical packet. MODULE closure/evaluation, discovery/MVS, mapping, loading,
and command/Test/execution consumers remain deferred.

The first package-preflight implementation attempt ended `REPLAN` on its
explicit production-line cap: the accepted three-key ownership model requires
about 440 formatted production lines, while the frozen cap was 360. The
uncommitted diff passes the full owner suite and independent review found no
semantic defect. `WP-5-host-nonregistry-package-preflight-cap-replan` may
correct only that cap and evidence budget before the same five-file
implementation resumes.

`WP-5-host-nonregistry-package-preflight-implementation-r2` resumes the
retained five-file Rust diff after accepted cap correction `0d3e03e8`.
Production semantics and ownership are unchanged; acceptance now requires the
remaining immutable, event/reuse, source-kind, and terminal-order proofs within
460/520/980 formatted bounds.

### Nonregistry package preflight accepted (2026-08-12)

Commit `411af144` accepts the route-independent package-policy leaf. The new
crate-private REPO, ignore, and marker keys use only the retained
materialization/source owners, preserve local and immutable invalidation, and
fail closed for every nonempty deleted-package set before repository source
work. Real-key local/immutable/category A/B/A, precedence, source-kind, Need,
event, and reuse proofs passed the full owner and downstream suites.

Run next only `WP-5-host-nonregistry-module-closure-resume-design`. Reopen the
closure design now that its sole package-preflight blocker is owned, and freeze
one shared route-independent preparation core plus exact implementation bounds.
No closure Rust, evaluation, graph, mapping, or consumer is yet authorized.

### Nonregistry MODULE closure resume design accepted (2026-08-12)

Commit `5757ea1d` accepts one route-independent closure key and shared
parsing/BFS core, with direct-local owners retained as unchanged adapters.
Run next only `WP-5-host-nonregistry-module-closure-implementation` in
`source_preparation.rs` and the crate-private parser projection in
`module_eval.rs`, under 420/480/900 caps and the canonical proof/stops. No
evaluation, graph, mapping, loading, or command consumer is authorized.

### Nonregistry MODULE closure implementation cap REPLAN (2026-08-12)

The first implementation attempt ended REPLAN solely on its explicit line-cap
stop. Exact section accounting measured 587 formatted net production lines and
468 test lines, 1,055 total, against 420/480/900. Focused closure proof, the
complete owner suite, downstream checks, formatting, and diff checks pass;
independent latest-diff review found no semantic, identity, ownership, order,
or lifecycle defect. Compressing the retained value/error identities and
materially distinct Host/direct adapters to the original production bound
would make the accepted shared BFS less auditable.

Run next only WP-5-host-nonregistry-module-closure-implementation-r2. Preserve
the same two Rust files, complete retained diff, behavior, evidence, and stops
under corrected 620/500/1,120 bounds. Account the production/test split exactly
and use no margin for another owner, diagnostic, behavior, evaluator, graph
consumer, or public surface.

### Nonregistry MODULE closure accepted (2026-08-12)

Commit `0231936f` accepts the callerless route-independent nonregistry closure
under the independently corrected caps in `f5742c3e`. It computes root files
and the exact nonregistry override before retained materialization/source/
package work, shares one breadth-first core with unchanged direct-local
adapters, and retains route-free `RepoSpec`/category/immutable-source identity,
exact root and ordered repeated-fragment bytes/inspections/labels/spans, logical
identities, and explicit cycle capability. Focused Host 2/2 and direct-adapter
13/13 tests, the full owner suite, both downstream checks, formatting, exact
587/468/1,055 accounting, structural scans, and independent review pass. No
evaluation, graph, mapping, loading, or consumer landed.

### Nonregistry discovered-module resume design accepted (2026-08-12)

Independent review accepts the existing `HostDiscoveredModuleKey` as the sole
composition seam. For an exact root `NonRegistry` override after the protected
built-in branch, it must require Bazel's empty effective module version, borrow
`HostNonregistryModuleClosureKey`, reject cycle capability before evaluation,
adapt retained root/fragments to the existing evaluator, publish one captured
event batch for complete evaluation, and retain the complete closure once as a
new nonregistry provenance variant. The closure already owns all exact source
identity and must not be duplicated.

Run next only
`WP-5-host-nonregistry-discovered-module-implementation` in
`source_preparation.rs` under the canonical 220/500/720 caps and stops.
Registry/built-in behavior is protected; recursion/MVS, command overrides,
post-selection mappings, lockfile products, package/Bzl loading, configured
toolchains, Test, execution, and every graph/consumer remain deferred.


### Nonregistry discovered-module composition accepted (2026-08-12)

Commit `6b2967c7` accepts the one-file Host composition under independently
reviewed +117 production/+244 test/+361 total growth. Exact root nonregistry
overrides now take the empty effective version branch before the registry-only
missing-version guard, borrow the retained closure, reject its cycle capability
before evaluation, retain ordered repeated logical inputs and the complete
closure once as provenance, and publish one captured event batch only for
complete evaluation success/failure. Protected built-in and registry behavior,
Need/error validity, cold/warm reuse, the full owner suite, downstream checks,
formatting, structural scans, and independent review pass. The leaf remains
callerless and no graph, mapping, loading, or consumer is active.

Run next only `WP-5-host-selected-module-graph-owner-design`. Freeze the sole
future Host discovery-to-MVS selected-module graph owner, its exact
override/policy/discovery identity and error lifecycle, or REPLAN into the first
missing normalized production input. This is a three-document design packet:
no Rust, legacy `ResolvedGraph` reuse, canonical-name synthesis, mapping, or
consumer activation is authorized.

### Selected-module graph owner design REPLAN (2026-08-12)

The live owner audit and independent reserved-architecture review found the
first missing semantic input before a selected graph. Slug has no normalized
command-line module override in its command policy or DICE graph:
`BzlmodCommandPolicyKey` owns only yanked-version and dev-dependency policy,
while `RootModuleOverrides` owns only root-MODULE declarations. A selected
graph therefore cannot preserve command-over-root/default-sentinel precedence
or explicit `bazel_tools` built-in bypass. Reusing either map or legacy
`ResolvedGraph` would create false equality and a second graph.

Run next only `WP-5-host-command-module-override-owner-design`. Freeze the
exact Bazel 9.2 flag grammar/path/precedence contract and one normalized
command/request/server/runtime/DICE input owner, or REPLAN at the first smaller
missing request/wire prerequisite. This is a three-document design packet. No
Rust, wire/schema change, RepoSpec, filesystem/materialization observation,
discovery, graph, mapping, or consumer is authorized.

### Command module override owner design accepted (2026-08-12)

Pinned Bazel 9.2 source and independent reserved-architecture review accept one
normalized `--override_module` value inside the existing command policy.
Client-owned parsing preserves raw occurrence/error order, folds the effective
module-to-absolute-path map with replace/remove/re-add semantics, and normalizes
the admitted workspace-root invocation without filesystem observation. The
existing Root command policy retains that compact `SmallMap` separately from
root-MODULE overrides; no merge, RepoSpec, discovery, or consumer is active.
One serde-defaulted effective-map wire field is required because the current
wire has no generic carrier; the daemon revalidates absolute paths/names and
rejects duplicates rather than owning normalization.

Run next only
`WP-5-host-command-module-override-owner-implementation` under the exact
17-file allowlist, 620/700/1,320 caps, lifecycle/DICE/wire proofs, and terminal
stops frozen in the accepted manifest. After implementation acceptance, resume
the selected-module graph owner design; do not activate that graph early.


### Command module override owner accepted (2026-08-12)

Commit `b319b551` accepts the normalized command override owner. The existing
command policy now retains one canonical compact
`SmallMap<CompactString, NormalizedAbsolutePath>` behind an immutable Arc;
raw occurrence order selects parser errors and fold results, while structural
equality/hash/order use the sorted effective map. Root command policy retains
that value separately from root-MODULE overrides.

Build, Run, Query, Cquery, and Aquery normalize at the client-owned workspace
boundary. The sole serde-defaulted wire field contains only effective absolute
paths; the server independently rejects invalid names, relative paths, NUL,
and duplicates. Real-DICE absent/present/path A/B/A, cold/warm reuse,
one-shot/stable-daemon Build and query, Run admission, wire compatibility, and
all five decoders are covered. The exact 17-file diff is 398 production, 612
test, and 1,010 total net lines. Relevant full suites, formatting, scope,
archive, forbidden-edge scans, and independent review pass; the two broad
failures are documented untouched baselines.

Run next only `WP-5-host-selected-module-graph-owner-design-r2`. Reopen the
sole Host discovery-to-MVS design now that normalized command overrides exist.
Freeze one exact crate-private selected graph and future implementation packet,
or return `REPLAN` at the first still-missing production leaf. No Rust,
legacy graph activation, canonical-name/RepoSpec synthesis, mapping, loading,
or consumer work is authorized before independent design acceptance.

### Selected-module graph owner design r2 REPLAN (2026-08-12)

The normalized command input closes only the request/DICE half of the prior
gap. The live discovery/source owners still classify only
`RootModuleFiles.overrides`: `HostDiscoveredModuleKey`,
`HostNonregistryModuleClosureKey`, `HostNonregistryPackagePreflightKey`, and
`RepositoryMaterializationRequestKey` cannot see a winning command override.
An explicit command path therefore cannot replace a root declaration or route
`bazel_tools` away from its built-in default. Repairing this inside MVS would
duplicate override and source ownership.

The first missing prerequisite is one effective module-override owner. Run
next only `WP-5-host-effective-module-override-owner-design`. Freeze a compact
crate-private leaf that overlays the accepted immutable root and command
inputs, retains root-versus-command provenance, projects a command path once
into the exact local-path `RepoSpec` shape, and becomes the sole classification
edge for discovery/materialization. It must not observe files or activate a
selected graph. This remains a three-document design packet; no Rust, legacy
resolver, mapping, loading, or consumer is authorized before independent
design acceptance.

### Effective module override owner design accepted (2026-08-12)

The live two-file composition and independent reserved-architecture review
accept one crate-private effective leaf over the immutable root and command
inputs. It retains `Root` versus `Command` provenance, projects a command path
once into the exact local-path `RepoSpec`, preserves root-only values unchanged,
and leaves effective absence for the downstream built-in `bazel_tools` default.
Every discovery, source-preparation, materialization, package-preflight,
closure, and repository-ignore classifier must use this sole edge; the leaf
itself performs no filesystem work.

Run next only `WP-5-host-effective-module-override-owner-implementation` in
`module_eval.rs` and `source_preparation.rs` under 280 production/420 test/700
total caps. Require command-over-root and root-name timing, explicit
`bazel_tools` bypass, command source lifecycle, real-DICE A/B/A and reuse,
root-only regression proof, complete structural replacement of direct root-map
classification, and independent implementation review. No graph, public API,
mapping, loading, or consumer work is authorized by that implementation packet.

### Effective module override owner implementation accepted (2026-08-12)

Commit `dbeb1fb9` accepts the two-file effective override implementation at
+189 production/+416 test/+605 total. One crate-private DICE key computes root
files before command policy, rejects a winning command override of the root
module name, retains `Root`/`Command`/`None` provenance, and projects command
paths once into the exact local-path `RepoSpec`. All five source/discovery
classifiers now depend on that leaf; explicit command `bazel_tools` bypasses
the builtin, root overrides preserve their accepted behavior, and effective
absence preserves the default builtin. Full owner tests, real-DICE A/B/A,
source lifecycle, formatting, scope/cap/forbidden-edge checks, downstream
baseline classifications, and independent review pass.

### Selected-module graph owner design r3 REPLAN (2026-08-12)

The override/source gap is closed, but the live graph still lacks one reusable
exact version domain. Root `module`, dependency, and registry-override values
validate but retain `+build`; nonroot values strip it; unchecked string-shaped
discovery keys assume a normalized caller. The accepted exact Bazel 9.2
parser/order is private to lockfile v28, while the old registry comparator is
semantically different and belongs to the forbidden supplied-file
`ResolvedGraph`. Selection cannot truthfully choose maxima or multiple-version
ceilings by copying either interpretation.

Run next only `WP-5-host-module-version-owner-design`. Freeze one shared
crate-private Bazel 9.2 grammar/normalization/equality/hash/order owner, its
root/nonroot/lockfile adapters, and a bounded future implementation packet.
No Rust, legacy graph activation, discovery/MVS, mapping, loading, or consumer
work is authorized before independent design acceptance.

### Module version owner design accepted (2026-08-12)

Pinned Bazel 9.2 source, the live version-field/comparator inventory, the
compact retained-representation audit, and independent reserved-architecture
review accept one crate-private `BazelModuleVersion`. Normalized
`CompactString` owns equality/hash, Arc-backed parsed identifiers own exact
ordering without reparsing, and an allocation-free empty sentinel orders last.
Root/nonroot evaluators, lockfile v28, and a checked Host discovered-key
constructor share the sole parser/order while retaining their existing typed
adapter diagnostics and public string scaffolding. The legacy resolver stays
isolated and no graph is activated.

Run next only `WP-5-host-module-version-owner-implementation` in the six files
and under the 240 production/360 test/600 total caps frozen in
`current-packet.md`. Require exact grammar/order/equality, root/nonroot/override
normalization, checked Host-key preactivation rejection, unchanged lockfile
bytes/errors, real-DICE A/B/A/reuse, structural single-owner proof, full owner
and direct-dependent validation, and independent implementation review.

### Module version owner implementation accepted (2026-08-12)

Commit `c997f7e7` accepts the exact shared Bazel 9.2 module-version owner.
One crate-private compact value now owns grammar, build-suffix normalization,
empty-sentinel behavior, unsigned numeric bounds, equality/hash, and parsed
identifier ordering. Root and nonroot retained fields normalize at their
directive adapters; lockfile v28 reuses the same parser/order without changing
its bytes or typed diagnostics; and `HostDiscoveredModuleKey::try_new`
rejects or normalizes before any DICE lookup.

Focused truth-table/property tests, retained root/nonroot surfaces, checked Host
construction, real-DICE spelling-equivalent and semantic A/B/A/reuse, all 100
lockfile-v28 regressions, the complete 303-test owner suite with integrations
and docs, loading, formatting, scope, and independent representation review
pass. The two broad core failures are unchanged unrelated baselines.

Run next only `WP-5-host-selected-module-graph-owner-design-r4`. Reopen the
sole Host discovery-to-MVS design now that normalized command/effective
overrides and exact versions exist. Freeze one bounded crate-private selected
graph owner and implementation successor, or return `REPLAN` at the first
still-missing semantic leaf. No Rust, legacy graph activation, canonical
mapping, loading, or consumer work is authorized before independent design
acceptance.

### Selected module graph owner design accepted (2026-08-12)

Pinned Bazel 9.2 discovery and selection source, the complete accepted Host
leaf inventory, the compact retained-representation audit, and independent
reserved-architecture review accept one callerless crate-private selected
graph owner. Candidate override names form only an audit horizon; every
classification and precedence result comes from
`HostEffectiveModuleOverrideKey`, and every discovered value comes from
`HostDiscoveredModuleKey`, so the owner neither re-merges inputs nor duplicates
source preparation. The shared `BazelModuleVersion` is the sole checked key and
ordering domain.

The accepted slice owns roots-first discovery horizons, complete-error-over-
Need behavior, exact-key cycle termination, whole-graph nodep fixed points,
multiple-version ceilings, per-name maxima, validation including fulfilled
nodep, and final BFS excluding nodep reachability. Pinned no-op compatibility
levels need no missing owner. Post-selection direct-dependency, yanked,
compatibility-diagnostic, mapping, final-module, loading, and consumer work
remains explicitly deferred.

Run next only `WP-5-host-selected-module-graph-owner-implementation` in the new
private `selected_graph.rs` plus the private `lib.rs` declaration, under the
760 production/1,050 test/1,810 total caps frozen in `current-packet.md`.
Require the frozen discovery, selection, retained-provenance, DICE lifecycle,
scope, and independent implementation proofs. A third Rust file, public API,
second graph/override merge, recursive DICE graph, lock across compute, raw
filesystem/network observation, or post-selection consumer is `REPLAN`.

### Selected module graph implementation cap REPLAN (2026-08-12)

The first compiling two-file implementation is retained but unaccepted at 875
net formatted production lines before tests, exceeding the frozen 760-line
cap by 115. Independent implementation and AI-cleanup review found no safe
mechanical reduction of that size: the override/root adapters, breadth-first
horizons, nodep fixed point, MVO selection, two validation/reachability walks,
and resolved/unpruned rewrite are distinct required phases.

One semantic correction is also required before implementation acceptance:
completed `HostDiscoveredModuleError` leaves must remain typed and structural
inside the selected-graph error. Only DICE compute failures may use a
Slug-native message. Flattening accepted leaf errors into `CompactString`
would lose predecessor identity and violate the accepted invalidation
contract.

Run next only the docs-only
`WP-5-host-selected-module-graph-owner-implementation-r2-cap-design`. Freeze
the same two-file implementation at corrected 920 production/1,050 test/1,970
total caps, with no new file, owner, policy, consumer, or behavior family.
Require a distinct typed leaf-error variant and focused equality proof. Retain
the unaccepted Rust diff; authorize no Rust until independent correction
acceptance and explicit r2 implementation activation.

### Selected module graph cap correction accepted (2026-08-12)

Independent correction review accepts the same two-file implementation at
920 production/1,050 test/1,970 total formatted net lines. The increase grants
no new file, owner, policy, consumer, or behavior family. R2 must retain
completed `HostDiscoveredModuleError` leaves structurally in a distinct typed
variant and prove that identity with a focused equality discriminator.

Run next only
`WP-5-host-selected-module-graph-owner-implementation-r2`. Resume the retained
unaccepted Rust diff, apply the typed leaf-error correction, complete the
frozen proof matrix, and obtain independent implementation review. All prior
third-file, public, second-graph/merge, recursive-DICE, lock, raw-observation,
post-selection, and cap stops remain terminal.

### Selected module graph owner implementation accepted (2026-08-12)

Commit `216a0be8` accepts the sole callerless Host discovery-to-MVS selected
graph. The private owner composes root files, normalized command/effective
overrides, exact `BazelModuleVersion` keys, and typed discovered-module leaves
without a second merge, source-preparation path, recursive DICE graph, lock,
raw observation, or public consumer. It retains roots-first resolved and
unpruned Arc-backed entries, transformed and original ordered dependencies,
fulfilled nodep edges, complete evaluated modules, and structural source
provenance.

The owner performs first-seen breadth-first horizons, complete typed
error-over-Need precedence including incompatible Need unions, whole-graph
nodep fixed points, highest-version selection, multiple-version existence and
lowest-ceiling rewrites, validation including nodep, and final BFS excluding
nodep reachability. Default built-in, command bypass, explicit-root failure,
cycles/diamonds, requested-edge retention, normalized spelling, semantic
A/B/A, Need invalidity, and cold/warm behavior are discriminated.

Formatted growth is 907 production, 686 tests, and 1,593 total within the
corrected 920/1,050/1,970 caps. Twelve focused tests, all 315 owner unit tests
plus integrations/docs, the full loading suite, formatting/diff/scope scans,
the known missing-V1-ref archive classification, AI cleanup, and independent
implementation review pass.

Run next only `WP-5-host-selected-module-route-owner-design`. Audit the first
post-selection canonical repository identity, contextual mapping, selected
route/RepoSpec, and `RootRepositoryRouteKey` composition boundary. Freeze a
bounded future owner or return `REPLAN` at the first missing semantic leaf.
No Rust, legacy graph activation, route conversion, loading consumer, mapping
consumer, extension execution, or public API is authorized before independent
design acceptance.

### Selected module route owner design REPLAN (2026-08-12)

Pinned Bazel 9.2 source and the live accepted graph prove that canonical module
identities and Bazel-dependency contextual mappings are derivable without a
new prerequisite. Root is main; `bazel_tools` and `platforms` are well-known;
single selected versions use `<name>+`; MVO versions use
`<name>+<normalized-version>`; and collisions remain terminal. Root/self and
resolved ordinary-dependency apparent names are already retained. Extension
imports and overrides remain a later additive mapping layer.

The route itself stops at selected registry RepoSpec ownership. Built-in
`bazel_tools` has no RepoSpec and accepted nonregistry provenance already owns
one, but registry provenance retains only the selected registry plus ordered
MODULE attempts/hash. Bazel fetches source.json for selected registry modules
after selection, combines registry and command mirrors plus
bazel_registry.json policy and the winning MODULE hash, projects an exact
archive/local-path/Git RepoSpec, and finally applies root single-version patch
fields. No accepted Host key owns that value or all of those dependencies.

The legacy `RegistrySourceCatalog` is caller-supplied scaffolding with no
Host/DICE observation edge and incomplete pinned semantics; activating it
would create a second registry graph. `RootRepositoryRouteKey` therefore keeps
its accepted built-in/direct-local behavior unchanged.

Run next only `WP-5-host-selected-registry-repo-spec-owner-design`. Audit and
freeze the smallest post-selection registry source/RepoSpec owner over
`RegistryFileKey`, `HostRegistryFunctionKey`, retained MODULE provenance,
`HostEffectiveModuleOverrideKey`, and the exact `RepoSpec` algebra, or return
`REPLAN` at a smaller missing registry-policy/source leaf. No Rust, registry
I/O, legacy catalog activation, mapping/route conversion, materialization,
loading, lockfile production, public API, or consumer work is authorized
before independent design acceptance.

### Selected registry RepoSpec owner design proposed (2026-08-12)

The owner audit finds no smaller prerequisite. One callerless
`HostSelectedRegistryRepoSpecsKey { workspace }` can compute the accepted
selected graph, skip root/built-in/nonregistry entries, and derive only
resolved registry RepoSpecs. `HostRegistryFunctionKey` supplies compact
resolved URL/mirror/vendor/hash-mode policy; `RegistryFileKey` remains the sole
source.json and optional bazel_registry.json byte/hash/lockfile observation;
the selected discovered provenance supplies ordered MODULE attempts and its
winning hash; and `HostEffectiveModuleOverrideKey` supplies final root
single-version patch augmentation without a second map merge.

The admitted exact projection covers pinned archive, local_path, and
git_repository source.json families, default archive type, mirror priority and
dedupe, remote patches/overlay, registry module-base policy, exact MODULE
SHA-256 SRI injection, and final patch/patch_cmd/patch_args fields. The existing
private-compatible `RepoSpec` recursive attribute algebra is sufficient. The
legacy public `RegistrySourceCatalog` remains untouched: it owns no Host/DICE
observation and lacks required fields.

The future retained Arc-backed BFS slice structurally owns module, registry
policy, ordered MODULE attempts/hash, complete source and optional registry
JSON observations, relevant effective override provenance, and final RepoSpec.
Need remains invalid; complete typed predecessor/parse/projection failures are
stable; completed errors beat compatible Need and first-error order across
selected modules is Slug-native.

After independent design acceptance, run only
`WP-5-host-selected-registry-repo-spec-owner-implementation` in the new private
`selected_repo_spec.rs` and one private `lib.rs` declaration, under 780
production/1,050 test/1,830 total formatted net lines. Require pure field and
projection truth tables, selected-only/zero-extra-fetch proof, source/policy/
MODULE/override A/B/A, error-over-Need and lifecycle evidence, full owner and
dependent validation, structural sole-edge scans, caps, and independent
implementation review. A third Rust file, public or RepoSpec-algebra widening,
legacy catalog/graph edit, second I/O/policy owner, raw I/O, route,
materialization, loading, lockfile publication, consumer, or cap excess is
`REPLAN`.

### Selected registry RepoSpec owner design accepted (2026-08-12)

Pinned Bazel 9.2 source, the live registry/graph/override owner audit, compact
representation review, and independent reserved-architecture review accept one
callerless selected-only aggregate. It borrows the accepted selected graph,
Host registry policy, registry-file observations, MODULE provenance, effective
override, and recursive RepoSpec algebra without a second catalog, I/O owner,
map merge, raw observation, route, or consumer.

Run next only
`WP-5-host-selected-registry-repo-spec-owner-implementation` in the new private
`selected_repo_spec.rs` plus one private `lib.rs` declaration, under the frozen
780 production/1,050 test/1,830 total caps. Require exact admitted archive,
local_path, Git, mirror, registry-json, MODULE SRI, and RegistrySingle patch
projection; selected-only file access; typed identity/error-over-Need; semantic
A/B/A and lifecycle proof; full owner/dependent validation; structural scope;
and independent implementation review. All frozen third-file, public, legacy,
second-I/O, raw-I/O, route/materializer/loading/consumer, and cap stops remain
terminal.

### Selected registry RepoSpec implementation cap REPLAN (2026-08-12)

The first compiling two-file implementation is retained but unaccepted at 980
formatted production lines before tests, exceeding the frozen cap by 200.
Independent architecture review found no safe mechanical reduction of that
size: typed JSON validation, archive/local/Git RepoSpec construction, selected
DICE orchestration, predecessor errors, and retained identity are materially
separate.

Two exact corrections are required before implementation acceptance. Pinned
`IndexRegistry.grabJson` treats whitespace-only bazel_registry.json as absent.
Pinned local_path projection lexically normalizes through PathFragment and
anchors a relative module base to the decoded path of a file-registry URI; raw
string concatenation and stripping a literal file:// prefix are insufficient.

Run next only the docs-only
`WP-5-host-selected-registry-repo-spec-owner-implementation-r2-cap-design`.
Freeze the same two Rust files at 1,020 production/1,050 test/2,070 total caps,
require whitespace/dot-separator/encoded-file-URI discriminators, and preserve
every existing behavior, proof, and terminal stop. The retained Rust diff is
unaccepted; no Rust may resume until independent correction acceptance and
explicit r2 activation.

### Selected registry RepoSpec cap correction accepted (2026-08-12)

Independent correction review accepts the same two-file owner at 1,020
production/1,050 test/2,070 total caps, with no new owner, source family, or
consumer. R2 must treat whitespace-only bazel_registry.json as absence and
must lexically normalize local paths while anchoring relative module bases to
the decoded path of a parsed file-registry URI.

Run next only
`WP-5-host-selected-registry-repo-spec-owner-implementation-r2`. Resume the
retained unaccepted diff, apply those focused corrections and discriminators,
complete the original proof matrix, and obtain independent implementation
review. Every prior file, public, legacy, second-I/O, raw-I/O, route,
materializer/loading/consumer, and cap stop remains terminal.

### Selected registry RepoSpec owner implementation accepted (2026-08-12)

Commit `e8ad58dd` accepts the private callerless selected-only registry
RepoSpec aggregate at 1,010 production, 844 tests, and 1,854 total formatted
lines, within the corrected 1,020/1,050/2,070 caps. It composes the accepted
selected graph, Host registry policy, registry-file observations, winning
MODULE provenance, effective override, and recursive compact RepoSpec algebra
without a second catalog, I/O owner, map merge, raw observation, route, or
consumer.

The exact admitted projection covers archive, local_path, and git_repository;
mirror priority/deduplication; blank registry JSON; decoded file-registry
anchoring and lexical path normalization; MODULE SRI; and RegistrySingle patch
augmentation. Structural equality retains every semantic input, Need remains
invalid, and completed typed errors beat compatible Need.

Ten pure and five real aggregate DICE tests prove selected-only source access,
unselected-version exclusion, root/built-in/nonregistry zero registry work,
source/registry-json/MODULE/mirror/override A/B/A restoration, warm reuse, Need
validity, and typed-error precedence. The full owner and loading suites,
formatting/diff/scope scans, AI cleanup, and independent review pass.

Run next only `WP-5-host-selected-module-route-owner-design-r2`. Revisit the
accepted route audit now that selected registry RepoSpecs exist, and freeze the
smallest canonical-name, contextual-mapping, and selected-route composition
owner or return `REPLAN` at the first missing leaf. No Rust, route/mapping
consumer, materialization, loading, legacy graph activation, public API, or
JVM/Java work is authorized before independent design acceptance.
### Selected module route owner r2 design proposed (2026-08-12)

The post-selection audit now finds one bounded composition seam. Pinned Bazel
9.2 constructs a canonical-name bi-map from the selected BFS graph before
deriving root/self/resolved-dependency contextual mappings. The accepted Host
selected graph already retains every required key, normalized version, apparent
edge, self repo name, BFS order, and built-in/nonregistry provenance. Commit
`e8ad58dd` supplies the sole selected registry RepoSpec aggregate.

The future callerless `HostSelectedModuleRoutesKey` belongs in the existing
private `selected_repo_spec.rs`. Its Arc-backed BFS entries retain the shallow
selected graph entry, exact canonical identity, a private context-bearing
compact dependency mapping, and an optional whole selected registry RepoSpec
entry. A single borrowed RepoSpec accessor on
`HostNonregistryPreparedClosure` avoids rereading overrides or
materialization. Transient name/collision/match maps are not retained.

Canonical root/well-known/unique/MVO naming, bi-map collision failure,
root/self/resolved ordinary mappings, route source categories, RepoSpecs, and
BFS order are exact. Deterministic Rust completed-error selection and wording
are Slug-native. Extension mappings/routes, post-selection policy, lockfile/
final-module publication, materialization, public root-route replacement,
loading, and consumers remain deferred.

After independent acceptance run only
`WP-5-host-selected-module-route-owner-implementation` in
`selected_repo_spec.rs` and `source_preparation.rs`, under 420 production,
700 test, and 1,120 total formatted net lines. Require the frozen pure and
real-DICE canonical/mapping/source/collision/A-B-A/Need/reuse matrix, full owner
and loading validation, compact-representation and AI-cleanup audits, structural
stops, and independent review. A third file, public API, predecessor mutation,
second graph/I/O/override owner, extension breadth, materialization/loading/
consumer edge, or cap excess is `REPLAN`.

### Selected module route owner r2 design accepted (2026-08-12)

Independent reserved-architecture review accepts the two-file callerless
selected-route seam, exact/Slug-native/deferred classifications, 420/700/1,120
caps, proof matrix, and terminal stops.

Run next only `WP-5-host-selected-module-route-owner-implementation` in
`selected_repo_spec.rs` and `source_preparation.rs`. Add no public API,
predecessor mutation, second graph/I/O/override owner, extension breadth,
materialization/loading/consumer edge, or third file. Obtain fresh independent
implementation review before acceptance.

### Selected module route owner implementation accepted (2026-08-12)

Commit `6f72baaf` accepts the private callerless selected-module route owner at
328 production, 439 tests, and 767 total formatted net lines, within the
420/700/1,120 caps. It computes the accepted selected graph before the accepted
selected registry RepoSpec aggregate and retains roots-first BFS entries with
the shallow graph entry, exact canonical identity, compact context-bearing
Bazel-dependency mapping, and optional whole selected registry RepoSpec.

Exact behavior covers root, well-known, unique-version, normalized MVO
canonical names; canonical collisions; root-empty, self, and transformed
ordinary dependency mappings; registry/nonregistry/built-in source
classification; and whole predecessor identity. Need remains invalid;
completed graph errors precede selected-source work. Slug-native error wording
and deterministic completed-error selection remain explicit.

One borrowed nonregistry RepoSpec projection reuses the retained closure source
identity. Pure and real-DICE tests cover both MVO contexts, every mapping and
registry mismatch terminal, root/built-in/nonregistry zero registry work,
registry source A/B/A, warm reuse, Need, and graph-before-source precedence.
The full owner and loading suites, formatting/diff/scope/cap checks, compact
representation and AI-cleanup audits, and independent implementation review
pass. The public root route and every consumer remain unchanged.

Run next only `WP-5-host-selected-extension-mapping-owner-design`. Audit the
first additive post-selection extension mapping owner over the accepted module
routes. No Rust, extension evaluation, repository rule, materialization,
lockfile/final-module publication, loading, public mapping/route consumer, or
JVM/Java work is authorized before independent design acceptance.

### Selected extension mapping owner audit replanned (2026-08-12)

The read-only audit stops at the first missing semantic leaf.
`EvaluatedRootModule` retains only header, dependencies, and registrations;
`RecordedRootModule` and `root_module_globals` expose no root extension
usage, proxy, import, isolation, tag, override/inject, or innate repo-rule
state. The accepted selected graph retains that incomplete root value, so a
selected extension mapping owner would omit configuration-affecting root
semantics from equality and invalidation.

The accepted nonroot evaluator already retains ordered
`NonrootExtensionUsage` values, proxies and logical locations, import
bijections, isolation keys, tags, and synthetic repo-rule usages. It
deliberately validates then discards nonroot `override_repo`/`inject_repo`
state. `HostSelectedModuleEntry` retains the whole discovered source, so the
graph, selected route, and nonroot representation do not require widening.

Existing extension directive fixtures prove basic success/rejection, and
`repo-mapping-canonical-names` proves one ordinary root generated-repository
mapping. They do not discriminate nonisolated versus isolated usage identity,
proxy/include export ownership, alias bijections, root override versus inject
precedence and must-exist errors, or restoration.

Run next only `WP-5-root-extension-usage-semantic-owner-design`. Pin those
root semantics and the smallest evaluator-owned retained value against Bazel
9.2 source and bounded hermetic oracle evidence. No Rust, selected mapping
owner, extension evaluation/materialization in Slug, lockfile/final-module
publication, loading, public consumer, or JVM/Java work is authorized before
independent design acceptance.

### Root extension-usage semantic owner designed (2026-08-12)

Pinned Bazel 9.2 groups ordinary usages by normalized bzl label and extension
name, gives every isolated usage a containing-module/exported-proxy identity,
retains ordered proxy/tag/import/location state, and models
`override_repo`/`inject_repo` as opposite `must_exist` values. Root dev policy
filters usage effects; nonroot override/inject remain validated then ignored.
`use_repo_rule` is one synthetic nonisolated usage per bzl/rule pair.

The smallest leaf is entirely in `module_eval.rs`: crate-private root usage and
isolation wrappers borrow the accepted compact nonroot proxy/tag/import/
override values, and an Arc-backed ordered slice is retained only by the
private root evaluation result and `RootModuleFiles`. The existing root
evaluation key remains the sole DICE owner. `EvaluatedRootModule`, the public
root graph, selected graph/routes, extension execution, and consumers stay
unchanged.

The new eight-file, 342-line Bazel 9.2 fixture proves nonisolated root/include
aggregation, distinct isolated identities, aliases, override replacement,
stable-daemon A/B/A, and opposite missing-override/generated-injection
terminals. Existing directive fixtures remain protected.

After independent acceptance run only
`WP-5-root-extension-usage-semantic-owner-implementation` in
`app/slug_bzlmod_v2/src/module_eval.rs`, under 520 production, 750 test, and
1,270 total formatted net lines. Require the frozen pure and real-DICE matrix,
oracle/protected fixtures, full owner/direct-loading validation, compact and
cleanup audits, structural stops, and independent implementation review. A
second Rust file, public export, new key/evaluator, selected graph/route edge,
extension execution/materialization, consumer, or cap excess is `REPLAN`.

### Root extension-usage semantic owner design accepted (2026-08-12)

Independent reserved-architecture review accepts the one-file private root
usage leaf, exact/Slug-native/deferred classifications, pinned eight-file
oracle, 520/750/1,270 caps, proof matrix, and terminal stops.

Run next only `WP-5-root-extension-usage-semantic-owner-implementation` in
`app/slug_bzlmod_v2/src/module_eval.rs`. Add no second file, public export,
key/evaluator, selected graph/route mutation, extension evaluation,
materialization, loading/consumer edge, or JVM/Java work. Obtain fresh
independent implementation review before acceptance.

### Root extension-usage semantic owner implementation accepted (2026-08-12)

Commit `11be92b9` accepts the one-file private root extension-usage owner at
386 production, 327 tests, and 713 total formatted net lines, within the
520/750/1,270 caps. Root and nonroot MODULE evaluation share the compact proxy,
tag, import, override, and deferred-attribute machinery while preserving
root-only override/inject retention, per-proxy root dev filtering, and the
accepted nonroot discard/reservation behavior.

The existing root DICE evaluation remains the sole owner. Its private complete
value and `RootModuleFiles` retain the ordered Arc-backed usage slice;
`EvaluatedRootModule`, the public root graph, selected graph/routes, and all
consumers remain unchanged. Root/include logical locations, ordinary
aggregation, isolated proxy identity, alias bijections, ordered tags,
override/inject `must_exist`, synthetic repo rules, event publication,
structural equality, warm reuse, and A/B/A restoration are discriminated.

All 339 owner unit tests and every integration suite pass, as does the complete
direct-loading suite. Pinned Bazel 9.2 passes the new eight-file root fixture
and six protected extension/mapping fixtures. Formatting, diff, scope,
compact-representation, AI-cleanup, and independent implementation review
pass. Archive content checks pass; missing local V1 archive refs remain the
known environment baseline.

Run next only the docs-only
`WP-5-host-selected-extension-mapping-owner-design-r2`. Audit whether the
accepted selected routes plus root/nonroot usages suffice for exact extension
IDs, unique names, imports, and root override/inject mapping before extension
evaluation, or `REPLAN` at the first missing post-selection leaf. No Rust,
fixture mutation, extension evaluation/materialization, loading/consumer,
public API, or JVM/Java work is authorized before independent design
acceptance.

### Selected extension mapping owner r2 designed (2026-08-12)

Pinned Bazel 9.2 proves one bounded pre-evaluation owner. It walks the selected
graph and source-ordered usages, resolves bzl labels through each module's
Bazel-dependency mapping, forms canonical extension/isolation IDs, assigns
first-encounter collision-safe unique names, resolves root override/inject
targets through the deps-only root mapping, and adds proxy imports to each
module's full contextual mapping.

The accepted selected routes provide graph order, canonical module identity,
and deps-only mappings. Commit `11be92b9` supplies the ordered root usage
slice through `RootModuleFiles`; discovered nonroot entries already retain
their usages. No second graph, route, usage evaluator, or I/O owner is needed.

The `must_exist` bit remains structural, but checking override-missing versus
inject-collision requires extension evaluation's generated repository set and
is deferred together with generated RepoSpecs, generated-repository mappings,
materialization, lockfile/final-module products, loading, and consumers.

After independent acceptance run only
`WP-5-host-selected-extension-mapping-owner-implementation` in
`selected_repo_spec.rs`, under 520 production, 800 test, and 1,320 total
formatted net lines. Require the frozen identity/order/collision/mapping/
override/error/Need/A-B-A/reuse matrix, protected suites and oracles, compact
and cleanup audits, structural stops, and independent review. A second file,
public API, predecessor mutation, another graph/route/usage owner, extension
evaluation or generated-existence validation, I/O/materializer/loading/
consumer edge, or cap excess is `REPLAN`.

### Selected extension mapping owner r2 design accepted (2026-08-12)

Independent reserved-architecture reviews accept the one-file private
pre-evaluation owner, exact/Slug-native/deferred classifications,
520/800/1,320 caps, proof matrix, and terminal stops. The accepted routes,
root usage slice, and discovered nonroot usages provide every required input;
generated repository existence remains correctly deferred.

Run next only
`WP-5-host-selected-extension-mapping-owner-implementation` in
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`. Add no second file, public
API, predecessor mutation, another graph/route/usage owner, extension
evaluation or existence validation, I/O/materializer/loading/consumer edge,
or JVM/Java work. Obtain fresh independent implementation review.

### Selected extension mapping implementation REPLAN (2026-08-12)

The first implementation audit stopped before Rust. R2 incorrectly said root
override/inject targets resolve through the deps-only root mapping. Pinned
Bazel 9.2 first constructs the root's full mapping from all selected extension
imports with an empty override table, resolves override targets through that
mapping, and only then substitutes resolved targets into final mappings. The
checked-in root fixture proves the distinction because the `replacement`
target is imported from an innate extension.

Run next only the docs-only
`WP-5-host-selected-extension-mapping-owner-design-r3-correction`. Freeze the
two-phase no-overrides/resolve/final mapping algorithm, its error order and
proof, and revalidate the one-file caps/stops. No Rust may resume before
independent correction acceptance and explicit r3 activation.

### Root extension override semantics prerequisite found (2026-08-12)

The r3 pinned-source audit found that `11be92b9` accepted one incorrect
review-driven branch. Bazel globally ignores root override/inject under
`--ignore_dev_dependency`; it does not filter per referenced proxy.
Ignored-dev `use_repo` still reserves apparent names before its no-op usage
is discarded. Root usage finalization also rejects missing overriding names,
inject-and-import of one exported repo, and any apparent repo that is both an
override target and an imported overridden repo.

These checks belong to the existing sole root evaluator, before selected
mapping composition. Run next only
`WP-5-root-extension-override-semantic-correction-implementation` in
`module_eval.rs`, under 120 production/220 test/340 total net caps. Require
pinned-source discriminators, protected evaluator/DICE/loading suites,
A/B/A recovery, cleanup/compact review, and independent implementation review.
No selected mapping Rust, second file, public/key/evaluator change, extension
evaluation, I/O/materializer/loading/consumer edge, or JVM/Java work is
authorized. Resume the two-phase r3 selected-mapping design only after this
prerequisite is accepted.

### Root extension override correction design accepted (2026-08-12)

Independent pinned-source and reserved-architecture reviews accept the sole
root-evaluator correction, 120/220/340 caps, proof matrix, and terminal stops.
The prior per-proxy advice is explicitly superseded.

Run next only
`WP-5-root-extension-override-semantic-correction-implementation` in
`app/slug_bzlmod_v2/src/module_eval.rs`. Add no second file, fixture, public
API, key/evaluator, selected mapping work, extension evaluation, I/O/
materializer/loading/consumer edge, or JVM/Java work. Obtain fresh independent
implementation review before resuming the two-phase r3 mapping design.

### Root extension override correction accepted (2026-08-12)

Commit `2644f091` restores the pinned root evaluator boundary. Ignored-dev
`use_repo` validates and reserves names before the inactive usage is
discarded; root override/inject globally no-op when dev dependencies are
ignored; active-usage finalization validates visible replacement names,
inject-and-import conflicts, and overriding/overridden intersections in
retained order. Nonroot behavior and public/selected surfaces remain unchanged.

Focused branch rows and real-DICE failure/restoration pass. All 340 Bzlmod
owner unit tests and every integration suite pass, as does the full loading
suite. Formatting, diff, one-file scope, 120/220/340 caps, compact and cleanup
audits, and independent review pass.

Run next only
`WP-5-host-selected-extension-mapping-owner-implementation-r3` in
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, under the accepted
520 production/800 test/1,320 total caps. Implement the two-phase full
no-overrides mapping, root target resolution, and final substitution
projection. Add no second file, public API, predecessor mutation, another
graph/route/usage owner, extension evaluation or generated-existence
validation, I/O/materializer/loading/consumer edge, or JVM/Java work. Obtain
fresh independent implementation review.

### Selected extension mappings accepted (2026-08-12)

Commit `75a431d6` owns the bounded pre-evaluation projection. The private
routes-first DICE key consumes only resolved selected entries and the retained
root/nonroot usage owners, forms canonical ordinary/isolated IDs, assigns
first-encounter collision-safe names, composes every full no-overrides
mapping, resolves root override/inject targets through the completed root
mapping, and substitutes final mappings while retaining `must_exist`.

The owner uses Arc-backed compact retained values and transient compact
maps/sets. It adds no public surface, Starlark evaluator, I/O, lock,
materializer, loading consumer, or generated-existence claim. Five focused
rows discriminate root/nonroot grouping, root/nonroot isolation, two MVO
contexts, innate naming, isolated collision suffixes, typed errors, real-DICE
Need/error ordering, nonroot mappings, zero I/O, warm reuse, and A/B/A. All
345 Bzlmod owner tests plus integrations and the full loading suite pass.
Formatting, diff, one-file scope, 454/516/970 measured growth, compact/cleanup
audits, and independent review pass.

The next ownership audit returns `REPLAN`: Slug has no production
`module_extension()`/`tag_class()` definition loader or extension-context
evaluator, so generated repository names and override/inject existence cannot
yet be known. Run next only the docs-only
`WP-5-host-module-extension-definition-owner-design`. Freeze the smallest
heap-independent root-local definition leaf, exact source/load/export/schema
ordering, DICE identity, and implementation feasibility. Do not retain a
Starlark heap/callable or fabricate generated names/RepoSpecs.

The design may edit only canonical/current/this Stage 5 plan, under
260/320/45/625 caps. No Rust, fixture, selected-owner mutation, evaluation,
I/O/materializer, lockfile, loading/command consumer, public API, or JVM/Java
work is authorized before independent acceptance.

### Module-extension definition owner audit REPLAN (2026-08-12)

The read-only audit stops at the first cross-crate ownership boundary. Pinned
Bazel 9.2 `RegularRunnableExtension.load` first loads the canonical bzl label,
then selects an exported `ModuleExtension`; `SingleExtensionEvalFunction`
consumes that loaded definition before evaluation and generated repositories.
Slug can preserve that order, but not inside the active private-only packet.

The accepted `HostSelectedExtensionMappingsKey` and its complete value are
private to `slug_bzlmod_v2`. The reusable `HostBzlModuleEvalKey`,
`ExternalBzlModuleEvalKey`, `FrozenBzlModule`, and lifetime closure are private
to `slug_loading_v2`, which already depends on Bzlmod. A definition key in
Bzlmod would reverse that dependency or duplicate the loader. A definition
key in loading cannot compute the selected mapping first without a narrow
cross-crate request. The existing loading globals also contain no
`module_extension` or `tag_class` definitions.

The accepted loader's frozen modules are lifetime-only state whose semantic
equality is the complete `BzlLoadManifest`; a later loading key can borrow that
cached value, validate an export, and return a heap-independent definition
projection without publishing a callable. That later step is blocked only by
the missing selected request and extension globals, not by a need for a second
loader.

Run next only the docs-only
`WP-5-host-selected-extension-definition-load-request-owner-design`. Freeze a
narrow `#[doc(hidden)]` Bzlmod projection, computed from the private selected
mapping owner, for the root-main, ordinary nonisolated extension slice. The
projection must retain deterministic encounter order, exact extension ID,
canonical bzl label, exported name, and the complete selected mapping context;
it must preserve Need invalidity and completed predecessor errors. It owns no
source/load observation, Starlark value, definition schema, execution,
generated repository, or loading dependency.

No Rust may begin before independent design acceptance. A future accepted
implementation may touch only `selected_repo_spec.rs` and `lib.rs`, under 160
production, 260 test, and 420 total net lines. Require pure and real-DICE
ordering/equality/error/Need/A-B-A/reuse proof. Any broader public consumer,
third file, definition/source work, isolation/MVO/innate/nonroot definition,
generated-existence claim, loading dependency, or cap excess is `REPLAN`.

### Selected extension definition-load request design accepted (2026-08-12)

Independent architecture review accepts the hidden heap-independent request
projection as the smallest cross-crate prerequisite. The selected mapping
owner remains sole and private; loading receives only deterministic admitted
requests and gains no generic graph, route, or mapping consumer.

Run next only
`WP-5-host-selected-extension-definition-load-request-owner-implementation`
in `app/slug_bzlmod_v2/src/selected_repo_spec.rs` and
`app/slug_bzlmod_v2/src/lib.rs`, under 160 production, 260 test, and 420 total
formatted net lines. Require the frozen pure/real-DICE identity, ordering,
dedup, Need/error, warm reuse, and A/B/A proof. Add no third file, generic
public consumer, source/load observation, loading dependency, Starlark value,
definition/evaluation/generated-repository work, I/O, or JVM/Java surface.

### Selected extension definition-load request implementation cap REPLAN (2026-08-12)

The first compiling two-file implementation is 205 production and 196 test
lines, 401 total, measured against `0552dcf3`. It preserves the accepted
projection and stops, but exceeds the 160 production cap by 45 lines. The
necessary hidden key/value/error surface, read-only request access, typed
predecessor routing, fail-closed unsupported terminal, and structural equality
make a forced reduction unsafe.

Run next only the docs-only
`WP-5-host-selected-extension-definition-load-request-owner-implementation-r2-cap-design`.
Retain the unaccepted Rust diff and freeze 220 production, 260 tests, and 480
total caps with exactly the same two Rust files, semantics, proof, and stops.
No Rust may resume before independent correction acceptance and explicit r2
activation.

### Selected extension definition-load request r2 activated (2026-08-12)

Independent review accepts the cap-only correction. Resume only
`WP-5-host-selected-extension-definition-load-request-owner-implementation-r2`
in `selected_repo_spec.rs` and `lib.rs`, under 220 production, 260 tests, and
480 total. Preserve the exact accepted semantics, proof, and every prior stop;
obtain independent implementation review before acceptance.

### Selected extension definition-load requests accepted (2026-08-12)

Commit `d0d7bde7` publishes the narrow hidden bridge required by loading. The
callerless Bzlmod key computes selected mappings exactly once, preserves
typed completed predecessor errors and invalid/non-self-equal Need, and emits
first-encounter root-main ordinary nonisolated requests containing only the
workspace, canonical bzl label, exported name, context repo, and immutable
selected mapping. The complete private predecessor remains structural identity;
isolated, MVO-owner, innate, nonroot, and non-root-repository definitions fail
closed.

Growth is 205 production, 236 tests, and 441 total lines against the corrected
220/260/480 caps in the authorized two files. Focused order/dedup/fail-closed
and real-DICE absence/change/restoration/reuse/Need/error rows pass. The full
Bzlmod and loading all-target suites, formatting, diff/scope/compact/cleanup
audits, and independent implementation review pass. No source/load evaluator,
I/O, callable, heap, generated-repository, or consumer edge was added.

The next missing leaf is loading-owned. Run only the docs-only
`WP-4-5-host-module-extension-definition-loading-owner-design`, jointly
recorded in Stage 4. Compose the accepted request first with the existing sole
`HostBzlModuleEvalKey`; freeze exact `module_extension`/`tag_class` globals,
export/schema validation and a heap-independent definition value without a
purpose-split loader or retained callable. The design allowlist is canonical,
current, Stage 4, and this Stage 5 plan under 45/240/220/180/685 caps.

No Rust, Cargo/BUILD, fixture, public API, selected-owner mutation,
source/evaluator key, extension execution/context, environment observation,
generated name/RepoSpec/existence, materializer, lockfile, consumer, or
JVM/Java work is authorized before independent design acceptance. `REPLAN`
on a second loader, retained Starlark heap/callable, repository-rule breadth,
public definition surface, third future Rust file, unresolved exact error
order, or cap excess.

### Module-extension definition loading implementation activated (2026-08-12)

Independent review accepts the cross-stage loading design. Run next only
`WP-4-5-host-module-extension-definition-loading-owner-implementation` in the
two loading files under 440/650/1,090; Bzlmod's accepted hidden request and
private selected owners remain unchanged. No public/consumer/execution/
generated-repository breadth is authorized.

### Definition loading accepted; evaluation-input design active (2026-08-12)

Commit `bf2c36e9` accepts the loading-owned definition prerequisite at
432 production/649 test/1,081 total. It preserves selected-request-first
ordering, the sole Host bzl loader, complete manifest identity, contextual
typed errors, invalid Need, and callable lifetime isolation. Focused lifecycle
proof, the full loading suite, audits, and independent review pass.

The next leaf is not a frozen callable seam: execution can recompute the Host
bzl key without moving a heap handle through DICE. Loading instead lacks the
Bzlmod-owned ordered root module/tag input view; hidden load requests expose
label/export/mapping while selected usage grouping and root tags remain
private.

Run only the docs-only
`WP-5-host-selected-extension-evaluation-input-requests-design`. Freeze one
callerless root-owned ordinary nonisolated projection retaining complete
predecessor/request identity, first-encounter extension order, a root module
view, and source-order raw tags with ordered attributes, dev flags, and
locations. Do not coerce schemas/defaults, construct `module_ctx`, acquire a
callable, execute, or infer generated repositories.

The root module view is exactly the selected Root graph key, accepted root
canonical repository, declared root header name and normalized version,
`is_root = true`, and the ordered tags for this extension ID. The graph/route
and `RootModuleFiles` owners supply those fields. Missing required state or a
usage/request mismatch fails closed; dependencies, registrations, overrides,
mappings, paths, lockfile state, and unrelated usages remain excluded.

The design owns only canonical/current/Stage 4/this Stage 5 plan under
45/220/140/220/625 caps. A future implementation may touch only
`selected_repo_spec.rs` and `lib.rs`, initially 240 production/360
test/600 total. Rust requires independent design acceptance first. Stop on a
loading dependency, generic public consumer, heap/callable,
schema/evaluator/execution, I/O, generated-repository/lockfile/materializer
edge, third Rust file, JVM/Java work, or cap excess.

### Selected extension evaluation-input design accepted (2026-08-12)

Independent review accepts the exact root-view schema, source ownership,
fail-closed joins, field exclusions, and proof matrix. Run next only
`WP-5-host-selected-extension-evaluation-input-requests-implementation` in
`selected_repo_spec.rs` and `lib.rs`, under 240 production/360 test/600
total formatted net lines against `a31cf3d9`. Preserve every accepted stop
and obtain independent implementation review.

### Evaluation-input implementation cap REPLAN (2026-08-12)

The compiling owner fit 240 production, but independent review found that
post-request root/join terminals discarded the accepted request aggregate and
could compare equal across mapping/context changes. The necessary structural
wrapper raises production to about 267 lines before completing field proof;
the frozen stop therefore fired.

Run only the docs-only
`WP-5-host-selected-extension-evaluation-input-requests-r2-cap-design`.
Retain the unaccepted two-file diff and freeze 280 production/360 test/640
total against `a31cf3d9`, with identical semantics, proof, files, and stops.
No Rust resumes before independent correction acceptance and explicit r2
activation.

### Evaluation-input implementation r2 activated (2026-08-12)

Independent review accepts the cap correction. Resume only the same two Rust
files under 280/360/640 against `a31cf3d9`, preserving every proof and stop.

### Selected extension evaluation inputs accepted (2026-08-12)

The r2 owner is accepted at 263 production, 304 test, and 567 total lines
against `a31cf3d9`. It computes the accepted load-request aggregate before root
files, retains full predecessor and exact-request context in every post-request
terminal, and publishes only the exact heap-free root identity plus source-
ordered raw tags. Field-by-field and exclusion A/B/A, Need/error, cold/warm,
full Bzlmod/loading, formatting, scope, cleanup, and independent review pass.

Run next only the cross-stage docs packet
`WP-4-5-host-module-extension-evaluation-input-composition-design`. Stage 5
owns no schema coercion, callable, `module_ctx`, execution, or consumer. The
audit may consume this accepted hidden key from loading but may not mutate
Bzlmod, introduce another raw owner, or authorize Rust before independent
design acceptance.

### Loading composition implementation activated (2026-08-12)

Commit `aee502ff` records independent acceptance of the cross-stage scalar
composition design. Run only
`WP-4-5-host-module-extension-evaluation-input-composition-implementation` in
`app/slug_loading_v2/src/bzl_module.rs` and
`app/slug_loading_v2/src/package.rs`, plus canonical/current/Stage 4/Stage 5
bookkeeping, under 420 production/700 test/1,120 total lines against
`aee502ff`. Stage 5 remains unchanged. Preserve every accepted proof and stop;
no Bzlmod mutation, second raw owner, callable, `module_ctx`, execution, I/O,
generated repository, lockfile, materializer, consumer, or JVM/Java breadth is
authorized.

### Loading composition accepted; pure invocation audit scheduled (2026-08-12)

The cross-stage composition owner is independently accepted at 414 production,
529 test, and 943 total lines against `aee502ff`. It consumes the accepted
Stage 5 raw projection without mutating Bzlmod, preserves complete selected
request/module/tag/mapping identity, and leaves callable reacquisition,
runtime context, invocation, repository rules, generated repositories,
lockfiles, materialization, and consumers absent.

Run next only the docs-only
`WP-4-5-host-pure-module-extension-invocation-owner-design` in canonical,
current, Stage 4, and this Stage 5 plan under 45/260/240/220/765 caps. Audit the
strict root-main singleton, ordinary nonisolated, empty-factor, read-only
context, `None`-returning invocation seam. Stage 5 production and public
surfaces remain unchanged. `REPLAN` on Bzlmod mutation/reverse dependency, a
second selected-input owner, environment/facts observation, repository rules,
generated outputs, retained Starlark heap/callable, or any loading consumer
breadth beyond the callerless receipt. No Rust or fixture is authorized before
independent design acceptance.

### Pure invocation implementation activated (2026-08-12)

Independent review accepts the design in `db45d182`. Run only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation` under the
four loading Rust paths and 520/800/1,320 caps frozen by current and Stage 4.
Stage 5 production remains unchanged: the implementation may consume the
accepted hidden selected/prepared inputs but may not mutate Bzlmod, add another
projection, reverse dependencies, or widen the admitted root-main singleton
slice. Preserve every repository-rule, generated-output, observation,
lockfile, materializer, consumer, retained-heap, and JVM stop.

### Pure invocation cap correction scheduled (2026-08-12)

The 520 production stop fired at the first compiling 630-line loading-owned
implementation. Retain the unaccepted Rust diff and run only the four-plan
docs correction `WP-4-5-host-pure-module-extension-invocation-owner-r2-cap-design`.
Freeze 720/800/1,520 against `db45d182` and the optional-None, complete
preflight-before-invocation, foreign-tag rejection, and immutable-list fixes.
Stage 5 production remains unchanged and no Rust, fixture, selected projection,
or public surface resumes before independent acceptance and explicit r2
activation.

### Pure invocation implementation r2 activated (2026-08-12)

Independent review accepts the correction. Resume only the four loading Rust
paths under 720/800/1,520 against `db45d182`; Stage 5 production remains
unchanged. Preserve the accepted hidden input boundary and every Bzlmod,
repository, output, lockfile, materializer, consumer, public API, and JVM stop.

### Pure invocation string-protocol prerequisite scheduled (2026-08-12)

The loading implementation remains callerless and Stage 5 production remains
unchanged. Exact Label ABI proof found that the shared Rust Starlark runtime has
no custom `str` projection distinct from `repr`; this cannot be repaired in the
four loading paths without leaving `%s` and other standard consumers wrong.
Run only `WP-4-starlark-custom-string-protocol-design` in the four plans under
45/220/180/100/545 documentation caps. Retain the unaccepted loading diff; no
Bzlmod, selected-input, public, runtime, or loading Rust edit is authorized
before independent design acceptance and explicit successor activation.

The completed audit freezes only the shared StarlarkValue `collect_str`
protocol and exact eight-file 90/220/310 future successor recorded in current.
Stage 5 stays unchanged; no selected-input, Bzlmod, repository, or mapping edge
is part of that successor.

Independent review accepts and activates
`WP-4-starlark-custom-string-protocol-implementation` against `73b22cec` under
the exact eight-file 90/220/310 boundary frozen in current. Stage 5 production
remains unchanged; only the four bookkeeping plans are authorized here.

Implementation review finds the two app paths inseparable from the unaccepted
invocation owner. Run only the four-plan docs correction
`WP-4-starlark-custom-string-protocol-implementation-r2-scope-design`, freezing
a six-file shared-runtime successor under unchanged 90/220/310 caps against
`73b22cec` and moving Label/loading/DICE proof back to the invocation packet.
Stage 5 production remains unchanged and no Rust resumes before acceptance and
explicit r2 activation.

Independent review accepts the scope correction in `6215fe03` and activates
only `WP-4-starlark-custom-string-protocol-implementation-r2` in the six shared
runtime files plus four plans, under 90/220/310 against `73b22cec`. Stage 5
production and all app/Label/loading/DICE surfaces remain unchanged and
unauthorized in this packet.

Independent implementation review accepts the isolated six-file runtime
delta and passing focused/loading/Bzlmod evidence. Stage 5 remains unchanged;
commit no app or Bzlmod Rust in this packet.

With the shared prerequisite accepted in `40def0e7`, resume only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r2` in the
four loading paths and four plans under 720/800/1,520 against that base. Stage
5 production remains unchanged; no selected-input, repository, mapping,
generated-output, consumer, public, or JVM edge is authorized.

The fully passing invocation diff measures about 724/846/1,570 and fires the
r2 cap. Run only the four-plan cap correction
`WP-4-5-host-pure-module-extension-invocation-owner-r3-cap-design`; retain the
same four loading paths, semantics, proof, and stops at 730/850/1,580 against
`40def0e7`. Stage 5 production remains unchanged and no Rust resumes before
acceptance plus explicit r3 activation.

Independent review accepts `86f478c0` and activates only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r3` in the
same four loading paths plus four plans under 730/850/1,580 against `40def0e7`.
Stage 5 production and all repository/mapping/output/consumer/public/JVM edges
remain unchanged and unauthorized.

Final review requires only an event-contract wording correction: evaluated
invocation activations own batches; reused activations intentionally do not,
and existing command-effect lineage owns later reachable-batch selection. Run
only `WP-4-5-host-pure-module-extension-invocation-event-contract-r4-design`
in the four plans. Keep the same four loading paths, 730/850/1,580 caps,
semantics/proofs/stops, and no Stage 5 or command-consumer change before
acceptance plus explicit r4 activation.

Independent review accepts `f36ec593` and activates only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r4` in the
same four loading paths plus four plans under 730/850/1,580 against `40def0e7`.
Stage 5 and command-consumer production remain unchanged; preserve all event,
repository, output, public, and JVM stops.

Independent implementation review accepts the callerless loading owner and
full loading/Bzlmod evidence at approximately 724/846/1,570. Stage 5 remains
unchanged; commit no Bzlmod or command-consumer Rust.

Pure invocation is accepted in `986ccebd`, but generated-repository capture is
not yet truthful because the sole shared loader has no `repository_rule`
definition owner. Run only the four-plan docs audit
`WP-4-5-host-repository-rule-definition-owner-design` under
45/240/200/120/605. Stage 5 remains unchanged. Do not infer generated names,
RepoSpecs, override/inject existence, mappings, lockfile products, or any
repository/materialization/consumer edge before this definition prerequisite
is independently accepted and explicitly activated.

The pinned definition audit REPLANs to the loading-owned docs design
`WP-4-5-host-module-extension-repository-rule-call-protocol-design` under
45/260/240/180/725. A Bazel repository-rule value is an exported callable, so a
standalone definition key would duplicate loading without owning its first
semantic use. Stage 5 remains unchanged: the future protocol may consume the
accepted selected/prepared request, mapping, and manifest identities only
through existing hidden loading seams. It may not mutate Bzlmod, reverse the
dependency, infer generated names/RepoSpecs or override/inject existence, or
add a second selected projection, graph, materializer, lockfile, consumer,
public API, or JVM edge.

The completed call-protocol design adds no Stage 5 owner or representation.
The existing loading invocation key continues to consume the accepted hidden
prepared predecessor, whose selected request, contextual mapping, root tags,
and transitive definition manifest remain structural identity. Its new
invocation-local repository-rule sink projects only definition identity and
ordered scalar raw calls; no Bzlmod map is reread or recomputed. Future Rust is
limited to the four loading paths and 650/850/1,500 caps frozen in current.
Stage 5 production remains unchanged. Schema application, generated canonical
names/RepoSpecs/existence, override/inject finalization, selected mappings,
lockfile, repository/materialization, consumer/API, and JVM work remain
deferred, and no reverse dependency or second projection/key is authorized.

Independent review accepts `7a49b5cd` and activates only
`WP-4-5-host-module-extension-repository-rule-call-protocol-implementation`
in the four loading paths plus canonical/current/Stage 4/Stage 5 under
650/850/1,500. Stage 5 production remains unchanged. Preserve the accepted
hidden prepared-input boundary and every no-Bzlmod-mutation, no-reverse-edge,
no-generated-state, no-lockfile/materializer/consumer/API/JVM stop.

### Raw repository-rule capture accepted; generated namespace prerequisite proposed (2026-08-12)

Independent implementation review accepts loading-owned raw capture in
`b7c70a1b`; `f5d64085` separately corrects the selected root input to
concatenate all matching usage tags in source order. Neither commit owns the
namespace Bazel later supplies to `RepoRule.instantiate`.

The first missing owner is the existing private selected extension request
projection, not a loading RepoSpec key. `HostSelectedExtensionMappingsKey`
already owns collision-suffixed unique names and ordered root override
projection, while the hidden definition-load request currently exposes only
the contextual/final mapping. Run only the four-plan docs audit
`WP-5-host-selected-extension-generated-namespace-request-design` under
45/220/180/220/665. Freeze a future widening of the existing request with the
exact unique canonical prefix and ordered `{generated name, canonical
replacement, must_exist}` metadata for the same admitted extension ID. The
selected value must retain the route-ordered no-overrides mappings before
substitution and project the root entry beside the existing final request
mapping; never derive it from substitutions or replay the algorithm in loading.

The future ceiling is exactly `selected_repo_spec.rs` and `lib.rs` for the
existing `#[doc(hidden)]` request/accessor, capped at 180 production/300
test/480 total formatted net Rust lines against the accepted design commit,
with no new key or second graph/projection. Require
collision and override joins, empty/ordered overrides, identity A/B/A,
Need/error/reuse and zero-I/O proof. RepoSpec/schema application, generated
call names/set, existence/override validation, loading dependency, I/O,
materialization, lockfile, consumer/API/JVM, third Rust file, or cap excess is
`REPLAN`. No Rust resumes before independent design acceptance and explicit
activation.

### Generated namespace request design accepted and implementation activated (2026-08-12)

Independent review accepts `fff82ecd`. Run only
`WP-5-host-selected-extension-generated-namespace-request-implementation`
in `app/slug_bzlmod_v2/src/selected_repo_spec.rs` and `lib.rs` solely for
the existing `#[doc(hidden)]` request/export accessor, plus canonical/current/
Stage 4/Stage 5 bookkeeping. Caps are mandatory 180 production/300 tests/480
total formatted net Rust lines against `fff82ecd`.

Retain route-ordered pre-substitution mappings in the selected-mapping value;
project the root base mapping, unique prefix, and exact-ID-joined ordered
override metadata beside the existing final request mapping. Preserve full
predecessor identity, fail closed on missing/duplicate/mismatched ownership,
and prove collisions, empty/ordered overrides, target/`must_exist`, A/B/A,
Need/error/reuse, unchanged loading dependents, and zero repository I/O. No
third file, new key/graph/projection, loading edge, reconstructed mapping,
generated call set/existence, schema/RepoSpec, I/O, materializer, lockfile,
consumer/API/JVM breadth, or cap excess.

### Selected namespace request accepted; loading instantiation design scheduled (2026-08-12)

Independent implementation review accepts `c7c55b17` at 106 production/205
tests/311 total within 180/300/480. The selected owner now retains route-ordered
pre-substitution mappings separately from final mappings and projects the root
base context, exact unique prefix, and ordered exact-ID override/inject metadata
through the existing hidden request. Same-ID mismatch, distinct-ID duplicate
namespace ownership, and missing root/base/final context fail with typed
complete errors. Full Bzlmod and loading suites pass; zero registry I/O is
proved.

The next owner is loading-side composition, not another Stage 5 projection.
Run only the four-plan docs packet
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-design`
under 45/260/240/220/765. Stage 5 production remains unchanged. The audit may
consume only the accepted hidden request and raw invocation receipt; it may not
mutate Bzlmod, replay namespace construction, add a reverse dependency, infer
existence/final routes, execute a repository implementation, or add I/O,
materializer, lockfile, consumer/API, or JVM breadth.

### Loading instantiation boundary frozen (2026-08-12)

No further Stage 5 prerequisite is required. The accepted request in
`c7c55b17` carries the exact root base mapping, final mapping, unique prefix,
and ordered substitutions inside each raw invocation receipt. Loading must
exact-join the full embedded request in encounter order, build the base plus all
generated names plus substitutions namespace, and never reconstruct selected
state or reread a Stage 5 owner.

Pinned Bazel performs no `must_exist` verdict in `createRepos`; the later
`SingleExtensionFunction` validates override-missing/inject-collision only
after eval-only RepoSpecs exist. The successor therefore retains `must_exist`
but defers existence validation and final routes. Future Rust is limited to the
three loading paths and mandatory 480/700/1,180 caps frozen in current. Stage 5
production, graph, mappings, materialization, lockfile, consumers, API, and JVM
remain unchanged.

### Loading instantiation implementation activated (2026-08-12)

Independent review accepts `7616136f`. Run only the three loading Rust paths
and four plan ledgers under mandatory 480/700/1,180 caps. Stage 5 production
remains unchanged. Loading may consume the accepted hidden request embedded in
the raw invocation receipt, but may not mutate Bzlmod, reconstruct selected
state, add a reverse edge, validate existence/final routes, or add repository
execution, I/O, materialization, lockfile, consumer/API, or JVM breadth.

### Loading instantiation proof-cap correction scheduled (2026-08-12)

Independent review leaves Stage 5 production accepted and unchanged, but the
loading implementation still owes exact join/namespace/lifecycle/A-B-A proof.
Run only the four-plan docs packet
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-r2-cap-design`
with the same three future loading paths and semantics, corrected mandatory
480/900/1,380 caps against `7616136f`, and no Rust authority before
independent acceptance plus explicit r2 activation. All no-Bzlmod-mutation,
no-reconstruction, no-existence, no-I/O/materializer/lockfile/consumer/API/JVM
stops remain.

### Selected validation-request projection proposed (2026-08-12)

Independent review accepts loading instantiation in `d50f02a2`. Pinned Bazel
checks source-ordered `use_repo` imports before override/inject polarity, but
the existing hidden request drops exported import names and proxy/override
locations. Run only the four-plan docs packet
`WP-5-host-selected-extension-validation-request-projection-design` under
40/220/180/180/620.

Freeze a future widening of the existing request in
`selected_repo_spec.rs` plus `lib.rs` accessors only, capped at mandatory
220 production/380 tests/600 total against the accepted design commit. Reuse
the selected owner, `CompactString`, `LogicalSpan`, immutable `Arc`
slices, `SmallMap` source order, and `Allocative`; aggregate every exact-ID
matching usage/proxy/import in encounter order and retain override locations.
No new key/graph/map owner, loading dependency, validation/generated set,
routes, I/O/materializer/lockfile/consumer/API/JVM breadth, third Rust file, or
cap excess.

### Selected validation-request projection implementation activated (2026-08-12)

Independent review accepts `533a9453`. Implement only
`WP-5-host-selected-extension-validation-request-projection-implementation`
in `selected_repo_spec.rs` and `lib.rs` hidden accessors plus four ledgers,
under mandatory 220 production/380 tests/600 total against `533a9453`.
Aggregate exact-ID root usage/proxy/import rows before request dedup, retain
local/exported names and proxy spans plus override spans, and preserve compact
Arc/CompactString/LogicalSpan/SmallMap/Allocative ownership. No third file,
new key/graph/map owner, loading edge, validation/routes, I/O/materializer/
lockfile/consumer/API/JVM breadth, or cap excess.

### Loading instantiation r2 activated (2026-08-12)

Independent review accepts `7cf2e45f`. Resume only the same three loading
paths and four ledgers as
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-implementation-r2`
under 480/900/1,380 against `7616136f`. Stage 5 production remains unchanged;
the complete join/namespace/lifecycle/A-B-A proof must land without Bzlmod
mutation, reconstruction, existence/routes, execution, I/O, materializer,
lockfile, consumer/API, or JVM breadth.

### Validation request REPLANs at shared import-order identity (2026-08-12)

The first request widening compiles but its required real DICE import reorder
row compares equal. Existing `NonrootRepoImports` owns only two
order-insensitive `SmallMap`s, so selected code cannot reconstruct invalidated
source order by iterating them later. The predecessor activation above is
historical and grants no authority after this REPLAN.

Run only the four-plan docs packet
`WP-5-extension-import-order-identity-owner-design` under
40/220/180/180/620. Freeze one `Arc<[CompactString]>` local-name order spine
on the existing shared import algebra; retain the maps as sole lookup/bijection
owners and duplicate no exported names. The corrected future implementation
is exactly `interim_module.rs`, `selected_repo_spec.rs`, and hidden
`lib.rs` accessors under mandatory 260/450/710. No new key/map/interner/
cache/digest, loading edge, validation/routes, I/O/materializer/lockfile/
consumer/API/JVM breadth, fourth Rust file, or cap excess.

### Extension import-order identity implementation activated (2026-08-12)

Independent review accepts `f14d3d7a`. Run only
`WP-5-extension-import-order-identity-owner-implementation` in
`interim_module.rs`, `selected_repo_spec.rs`, and hidden `lib.rs`
accessors plus four ledgers, under mandatory 260 production/450 tests/710
total against `f14d3d7a`. Add the compact local-name order spine, preserve
existing maps as sole lookup/bijection owners, and complete the validation
request projection/proof. No fourth file, new key/map/interner/cache/digest,
loading edge, validation/routes, I/O/materializer/lockfile/consumer/API/JVM
breadth, or cap excess.

### Extension import identity accepted; loading validator design scheduled (2026-08-12)

Independent review accepts `ff55dcbf`: one compact local-name spine preserves
root/nonroot import order while the hidden request retains aggregated import
and override locations. Stage 5 production is complete for the admitted
validation-input seam and remains unchanged in the next packet.

Run only the four-plan docs packet
`WP-4-5-host-module-extension-generated-repository-validation-owner-design`
under 40/220/180/180/620. Loading may design one private validator over the
accepted instantiation predecessor: imports first against generated apparent
names or override keys, then override/inject polarity. Freeze only the existing
instantiation accessor file, new private validation module, and private
`lib.rs` declaration under mandatory 320/650/970. No Bzlmod edit/reverse edge,
generated route/publication, execution/context, I/O/materializer/lockfile/
consumer/API/JVM work resumes.

### Loading generated-repository validation implementation activated (2026-08-12)

Independent review accepts design 1f7165ed. Loading may implement only
WP-4-5-host-module-extension-generated-repository-validation-owner-implementation
in the accepted three loading paths plus four ledgers under 320/650/970.
Stage 5 production remains unchanged; no Bzlmod edit/reverse edge, route,
publication, execution/context, I/O/materializer/lockfile/consumer/API/JVM
breadth.

### Loading validation accepted; generated-spec publication boundary scheduled (2026-08-12)

Independent review accepts loading commit `b2a153aa`. Stage 5 production is
unchanged: Bzlmod continues to own selected mappings, repository routes, and
materialization and may not depend back on loading.

Run only the four-plan docs packet
`WP-4-5-host-validated-generated-repository-spec-publication-design` under
40/240/200/180/660. Audit a hidden heap-independent borrowed view of loading's
validated canonical-name/`RepoSpec` certificate so a later dependency-neutral
consumer can compose it with Bzlmod routes. No Bzlmod Rust, new DICE key, copied
row store, route/mapping publication, source preparation/materialization,
repository execution/I/O, lockfile, consumer/API, or JVM work resumes. Any
future successor is limited to the three loading files under mandatory
220/420/640 only after independent design acceptance and activation.

### Hidden generated-spec boundary frozen without Stage 5 mutation (2026-08-12)

Pinned validation returns eval-only generated `RepoSpec` rows unchanged;
override/inject substitutions affect later mappings and lookup, not the
generated row or its canonical prefix-plus-name identity. Stage 5 remains
unchanged and may not depend on loading.

The future loading-only successor exports the existing validation key plus
opaque hidden success/error wrappers. Borrowed iteration exposes only
`(&CanonicalRepoName, &RepoSpec)` in request/call order and duplicates no
store. Only a later higher `slug_server_v2` composition owner may consume both
loading and Bzlmod. Exact three-file 220/420/640 limits and all no-route,
no-materialization, no-execution/I/O, no-public-API/JVM stops remain.

### Loading hidden generated-spec publication implementation activated (2026-08-12)

Independent review accepts design `433badeb`. Loading may implement only
`WP-4-5-host-validated-generated-repository-spec-publication-implementation`
in the exact three loading paths plus four ledgers under 220/420/640.
Stage 5 production remains unchanged. No Bzlmod dependency on loading, new key,
copied store, route/mapping publication, materialization, execution/I/O,
lockfile, consumer/API, or JVM breadth is authorized.

### Generated-route ownership audit follows validated spec publication (2026-08-12)

Independent review accepts loading publication `d2ed6ad3`. The hidden
certificate exposes original validated generated canonical-name/`RepoSpec`
rows without copying them. Bzlmod still owns `RootRepositoryRouteKey`, package
source preparation, and materialization and must not reverse-depend on
loading; `slug_server_v2` is the existing higher crate that depends on both.

Run next only the four-plan docs packet
`WP-4-5-host-generated-repository-route-boundary-design` under
45/260/220/220/745 documentation caps. Audit pinned Bazel 9.2 effective
generated-route membership, override/inject `RepoSpec` selection, complete
per-repository mapping/context, collision and error order, then freeze one
dependency-safe bounded successor or `REPLAN`. Stage 5 production remains
unchanged. Do not widen `RootRepositoryRouteKey`, reconstruct selected state,
add a Bzlmod-to-loading edge, or authorize materialization/I/O/lockfile/
consumer/public/JVM work.

### Route audit defers to loading mapping retention (2026-08-12)

Pinned Bazel 9.2 keeps generated canonical-to-internal identity and constructs
each generated repository's mapping from the host module mapping, every
generated name, then ordered overrides/injections keep-last. Loading already
computes that mapping transiently; its publication omits it. Bzlmod cannot
recover it without duplicating loading semantics or a reverse dependency.

Run next only the four-plan docs packet
`WP-4-5-host-generated-repository-mapping-retention-design` under
45/260/220/220/745 documentation caps. The bounded successor may touch only
the existing loading instantiation/validation files and hidden lib exports
under mandatory 280/520/800 Rust caps after acceptance and activation. Stage 5
production remains unchanged. No `RootRepositoryRoute`, Bzlmod/server,
materializer, route, mapping reconstruction, I/O, lockfile, consumer/API, or
JVM work is authorized.

### Loading generated-mapping retention implementation activated (2026-08-12)

Independent review accepts design `9e12fe58`. Loading may implement only the
exact three-file mapping-retention/publication seam plus four ledgers under
mandatory 280/520/800. Stage 5 production remains unchanged. No
`RootRepositoryRoute` widening, selected-state reconstruction, Bzlmod/server
edit, route/materializer, I/O, lockfile, consumer/API, or JVM work is
authorized.

### Canonical generated-definition lookup moves to core design (2026-08-12)

Loading publication `b9a4a3fc` now carries every exact canonical definition
fact needed before route lookup. Core already depends on both loading and
Bzlmod and owns workspace DICE/materialization orchestration; locating this
semantic key in server would strand one-shot consumers and mix graph ownership
with transport.

Run only `WP-4-5-6-host-generated-repository-definition-lookup-owner-design`
in four plans under 45/260/220/220/745 caps. Freeze a private core key in one
new runtime module plus private `runtime/mod.rs` declaration under future
mandatory 260/480/740 after acceptance/activation. Stage 5 production remains
unchanged. No Bzlmod/loading/server edit, apparent route, `RootRepositoryRoute`,
source/materializer/I/O, lockfile, consumer/API, or JVM work is authorized.

### Core canonical generated-definition implementation activated (2026-08-12)

Independent review accepts design `6678f54f`. Core may implement only the
private two-file canonical lookup plus four ledgers under mandatory
260/480/740. Stage 5 production remains unchanged. The key computes only the
accepted hidden loading validation key and borrows its no-copy certificate;
no Bzlmod/loading/server edit, reverse edge, apparent route,
`RootRepositoryRoute`, repository execution/context, source/materializer/I/O,
lockfile, consumer/API, or JVM work is authorized.

### Core generated-definition proof cap correction scheduled (2026-08-12)

The two-file core lookup retains the sole loading-certificate dependency and
all no-route/no-materialization stops, but its complete proof measures
222/541/763 against `6678f54f`. Run only docs-only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-r2-cap-design`;
authorize no Rust and change only future caps to 260/550/800. Stage 5
production remains unchanged. Rust resumes only after acceptance and explicit
r2 activation.

### Core generated-definition lookup implementation r2 activated (2026-08-12)

Independent review accepts cap correction `99a5b898`. Run only
`WP-4-5-6-host-generated-repository-definition-lookup-owner-implementation-r2`
in new core runtime `generated_repository_definition.rs` plus `runtime/mod.rs`
solely for its private declaration and four ledgers, under mandatory
260/550/800 against `6678f54f`. Preserve the sole loading-key dependency,
complete scan, certificate-plus-ordinal/no-copy result, full proof, and every
no-route/no-materialization stop. Stage 5 production remains unchanged; no
Bzlmod/loading/server edit, reverse edge, public API, execution/I/O, lockfile,
materializer, consumer, or JVM work is authorized.

### Core apparent generated-repository mapping design scheduled (2026-08-12)

Independent review accepts canonical lookup `daefe6fc`. The accepted loading
certificate already retains the exact host/generated/substitution mapping per
generated context; the next leaf is a private core direct lookup, not a Bzlmod
route or reverse dependency.

Run only four-ledger docs packet
`WP-4-5-6-host-generated-repository-apparent-mapping-owner-design` under
40/240/200/180/660 documentation caps. Freeze a key over only the accepted
definition key, nonroot apparent input, exact context validation, borrowed
post-substitution target, complete structural errors/equality, and one-file
future 220/450/670 limits. Stage 5 production remains unchanged; root mapping,
`RootRepositoryRoute`, generated definition publication, source/materializer,
execution/I/O, lockfile, consumer/API, reverse edge, and JVM work remain
forbidden.

### Core generated-repository apparent mapping implementation activated (2026-08-12)

Independent review accepts design `0af55eff`. Run only
`WP-4-5-6-host-generated-repository-apparent-mapping-owner-implementation` in
the existing core generated-definition module with colocated tests and four
ledgers, under mandatory 220/450/670 against `0af55eff`. Preserve direct
retained-map lookup, exact context identity, no-copy target access, proof, and
all no-route/no-materialization stops. Stage 5 production remains unchanged;
no Bzlmod/loading/server edit, second Rust file, reverse edge, source/I/O,
consumer/public API, lockfile, materializer, or JVM work is authorized.

### Canonical selected-module lookup design scheduled (2026-08-12)

Independent review accepts generated-context mapping `f468fa30`. The next peer
semantic catalog is already owned by private `HostSelectedModuleRoutesKey`;
freeze canonical selection there before any core generated/selected domain
composition or root route/source work.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-owner-design` under
40/240/180/220/660 documentation caps. Freeze a private one-file lookup that
computes only selected routes, scans to exhaustion for zero/duplicate canonical
ownership, and retains catalog+ordinal with borrowed root/registry/nonregistry
identity, typed builtin fail-closed handling, and warmed-predecessor
zero-additional-source proof under mandatory future 220/500/720 caps. Stage 5 production remains
unchanged; no loading/core/server Rust, public export, new graph, root route,
source/materializer/I/O, lockfile, consumer/API, or JVM work is authorized.

### Canonical selected-module lookup implementation activated (2026-08-12)

Independent review accepts design `dd8ca159`. Run only
`WP-5-host-canonical-selected-module-definition-owner-implementation` in
existing `selected_repo_spec.rs` with colocated tests and four ledgers, under
mandatory 220/500/720 against `dd8ca159`. Preserve the sole selected-routes
dependency, complete canonical scan, predecessor+ordinal borrowed result,
builtin fail-closed terminal, proof, compatibility, and stops. Stage 5
production remains otherwise unchanged; no `lib.rs`, second Rust file, new
graph owner, loading/core/server edit, public route/source/materializer/I/O,
execution, lockfile, consumer/API, or JVM work is authorized.

### Selected-module definition publication design scheduled (2026-08-12)

Independent review accepts private canonical lookup `bd3ab8ee`. Its selected
catalog remains Bzlmod-owned; expose only a hidden borrowed certificate ABI
before any higher core selected/generated composition.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-publication-design` under
35/220/180/180/615 documentation caps. Freeze the existing key constructor,
opaque structural errors, predecessor+ordinal certificate, ordered borrowed
mapping and original selected RepoSpec view, future exact two-file 180/380/560
scope, proof, and stops. Stage 5 production remains unchanged; no new key/store,
loading/core/server Rust, route/source/materializer/I/O, lockfile, public stable
API, consumer, or JVM work is authorized.

### Selected-module hidden publication implementation activated (2026-08-12)

Independent review accepts design `1d8758d5`. Run only the exact hidden ABI
in existing `selected_repo_spec.rs` and `lib.rs` hidden re-exports plus four
ledgers, under mandatory 180/380/560 against `1d8758d5`. Preserve the existing
key/store, predecessor+ordinal certificate, opaque errors, borrowed ordered
mapping/original RepoSpec, proof, compatibility, and stops. Stage 5 production
remains otherwise unchanged; no new key/store, third file, loading/core/server
edit, route/source/materializer/I/O, stable API, consumer, or JVM work is
authorized.

### Selected publication accepted; absence-signal design scheduled (2026-08-12)

Independent review accepts hidden selected-module publication `bc822520`.
Before core can compose selected and generated canonical definitions, it must
fall through only the selected-domain Missing terminal without parsing or
exposing the otherwise opaque error payload.

Run only four-ledger docs packet
`WP-5-host-canonical-selected-module-definition-absence-signal-design` under
mandatory 35/180/140/140/495 documentation caps. Freeze a hidden Copy/Eq
`Missing | Terminal` disposition and accessor on the existing error wrapper;
future Rust is limited to `selected_repo_spec.rs` and `lib.rs` hidden re-export
under mandatory 50/120/170. Keep Routes, RoutesCompute, Duplicate, and
BuiltinDeferred terminal; Need has no disposition. No new key/store,
core/loading/server edit, selected/generated composition, builtin precedence,
route/source/materializer/I/O, stable API, or JVM work is authorized.

### Selected-module absence-signal implementation activated (2026-08-12)

Independent review accepts design `c466d864`. Run only
`WP-5-host-canonical-selected-module-definition-absence-signal-implementation`
in existing `selected_repo_spec.rs` plus `lib.rs` solely for the hidden enum
re-export and four ledgers, under mandatory 50/120/170 against `c466d864`.
Preserve Missing-only fallthrough classification, opaque terminal payloads,
Need/nonpublication, proof, compatibility, and stops. Stage 5 production
remains otherwise unchanged; no new key/store, third Rust file,
core/loading/server edit, composition, builtin precedence, route,
source/materializer/I/O, stable API, or JVM work is authorized.

### Selected absence accepted; core definition composition design scheduled (2026-08-12)

Independent review accepts selected absence signal `35ff14f7`. Run only
four-ledger docs packet
`WP-4-5-6-host-canonical-repository-definition-composition-owner-design` under
mandatory 40/260/220/200/720 documentation caps. Freeze the private core
selected-before-generated owner: selected success/Terminal blocks generated,
only Missing falls through, and success/errors retain original certificates
without copies. Future Rust is only existing core
`generated_repository_definition.rs` under 260/520/780. Stage 5 production
remains unchanged; no Bzlmod/loading/server Rust, second file, builtin
precedence, route/source/materializer/I/O, public API, or JVM work is authorized.

### Core canonical definition composition implementation activated (2026-08-12)

Independent review accepts design `e05a0dfc`. Run only
`WP-4-5-6-host-canonical-repository-definition-composition-owner-implementation`
in existing core `generated_repository_definition.rs` with colocated tests and
four ledgers under 260/520/780 against `e05a0dfc`. Preserve selected-first
short-circuiting, Missing-only generated fallback, retained original
certificates, proof, compatibility, and stops. Stage 5 production remains
unchanged; no second Rust file, Bzlmod/loading/server edit, builtin precedence,
route/source/materializer/I/O, public API, or JVM work is authorized.
