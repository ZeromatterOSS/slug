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
- `BazelLockFileValue.java` is the schema source; accepted Bazel 9.2 visible-lockfile
  evidence records `lockFileVersion` 28. Older 9.1.1/version-26 fixtures are
  historical and cannot establish current acceptance.
- Bazel lockfile tests under `src/test/py/bazel/bzlmod/` are the first oracle
  source for replay/error-mode behavior.

## Current state

M1's shared source/session graph is accepted; the old pre-aquery integration
freeze is historical. Stage 5 now supplies only M7A's demanded repository
prerequisites. The current fixture preparation contract and R2 dependency ledger
are in [configured-cli-fixture.md](./configured-cli-fixture.md).

Accepted September 10 corrections: nodep pruning after discovery fixed point
(`97dffd5d4`), regular 0640/directory 0750 archive admission (`0f45fab28`),
verified local-file capture (`f68091b8b`), typed registration error ownership
(`a06f3ddfc`), bounded observer (`64c7d2475`), source diagnostic (`ac6140f41`)
and run registry propagation (`47163df7b`). None proves complete authentic
configured CLI closure. That remains the first source-acceptance gap.

Natural owners remain immutable Bzlmod/module/repository facts and tracked
Host/source observations. Preserve canonical route identity, lazy source demand,
request/generation isolation, final reobservation and atomic publication. An
error renderer is a borrowed projection of the retained typed error; bounded
text never substitutes for equality or canonical source identity.

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
## Accepted local-file capture contract

Design WP-5-7A-selected-bcr-file-capture-design-r1 and implementation
WP-5-7A-selected-bcr-file-capture-impl-r1 are accepted at `f68091b8b`.
The following source, safety and ownership invariants remain regression gates;
they do not schedule another implementation.
This closes the source and ownership invariants below, not the real CLI source closure.

**Compatibility and grammar.** The pinned IndexRegistry/HttpConnector sources
and tests above establish explicit local payloads, ordered file mirrors and SRI.
These are exact only for the admitted subset; the parser, errors, safety limits
and Rust filesystem observations are Slug-native. File queries present in
IndexRegistryTest's mirror projection remain deferred in capture, not silently
claimed as parity. HTTP-to-file redirects, other schemes and other OS support
remain unsupported. Existing HTTPS auth/redirect semantics do not widen.

One private runtime/repository_archive_file.rs owns file grammar and capture.
Admit case-insensitive file:/// followed by a nonempty absolute Linux path,
valid Unicode, with at most16384 URL bytes and4095 decoded path bytes. Reject
authority (including localhost/UNC), credentials/port, raw query/fragment,
raw whitespace/control/backslash, malformed percent escapes, decoded NUL/ASCII
controls/backslash/non-UTF8, percent-encoded separators, empty first/trailing
path segments, standalone raw/decoded dot or dot-dot segments and first segments
starting with an ASCII letter plus colon or vertical bar. Interior repeated
slashes are admitted: pinned IndexRegistry.constructUrl:134-142 and mirrored_url
produce them for authority-free file primaries plus mirrors; Linux resolves them
as ordinary separators. Extra leading slash/root-only stays rejected.
Percent-encoded ordinary filename bytes (space, %, #, ?) and
literal Unicode are admitted. Validate raw segments before url2.5.8 normalizes
them; decode exactly once with the existing url crate. No path canonicalization
or cache lookup. Both plan parsing and transport use this same file validator;
the existing HTTPS plan guard and stricter transport validator stay distinct.
Bzlmod archive_repo_spec only relaxes its has_host absoluteness predicate for
parsed file scheme, retaining raw URLs/order/SRI; core remains the filesystem
grammar owner. No new public URL type or core dependency in Bzlmod.

**Linux descriptor safety.** Use existing libc constants through safe std
OpenOptionsExt: first pin the path with O_PATH|O_CLOEXEC and require fstat regular
file, then open the held /proc/self/fd/N read-only with CLOEXEC|NONBLOCK; require
regular type and matching dev/ino before reading. Hold the pin through reopening.
Never reopen the mutable original pathname. Ordinary symlinks may resolve to a
regular file; replacement/unlink after pin cannot retarget the data descriptor.
Directories, FIFOs, sockets and devices fail before data-open, including through
symlinks. Inaccessible/missing procfs fails closed, without a pathname fallback.
This procfd path reopens an owned descriptor, not a source/cache locator.
Linux man-pages6.7 open(2), O_PATH/fstat and proc_pid_fd(5) own this mechanism:
uncompressed SHA-256 respectively
83938a9f95bfbc18b4c8fcb8d42c92b124db421d4e6a750e4ad8ef3448307b11 and
1c8d7e1212b9ea55df406a5c6569a7c00f98f66bbfa61502420791d16711c3ed.
The url crate's to_file_path is decoding machinery, not admission authority.

**Bounded capture.** Preflight descriptor length against the existing role cap;
read at most64KiB per step into existing NamedTempFile, SHA-256 the actual stream,
check actual cumulative bytes without overflow, require EOF/SRI, flush and check
activity before success. Caps remain archive128MiB/MODULE1MiB/patch8MiB/overlay64MiB.
Check cancellation before pin, after reopen, between reads and around finalization;
every error/cancellation drops descriptors and temporary capture. No body-sized
allocation, mmap, worker/task, persistent cache or shared lock across capture.
Symlink/path mutation selects one inode; concurrent byte mutation is acceptable
only if the completed captured stream equals declared SRI. Metadata is not a
digest substitute or historical snapshot. Syscall latency assumes responsive
local regular-file storage; remote/FUSE/pseudo-filesystem latency and forcibly
interruptible kernel calls are deferred. NONBLOCK does not bound regular-file I/O.

**Transport and lifetime.** repository_archive_http remains the common ordered
capture_urls owner; capture_one dispatches explicit file input to the new helper.
All four wrappers preserve their caps and existing transform order. A failed
file may try only the next explicitly supplied URL; first verified success stops.
NativeEnvironment becomes I/O-free to construct, with phase-local
OnceCell<Result<Arc<ClientConfig>, String>> initialized only by prepare_https
after URL/activity validation and before DNS. A default Environment hook preserves
the existing test seam. TLS success/failure lives only for that payload's ordered
capture call; file-only attempts never load certificates, resolve or connect.
No service/DICE-retained representation change: Stage9/donor work and performance
claims do not apply. The existing URL/spec request remains immutable.

**Publication and revisions.** Bzlmod source_preparation.rs:5201-5343 owns exact
RepositoryMaterializationResultKey request equality, immutable roots/observation
instances, and generation-key dependencies for transport/materialization errors.
Unchanged URL/spec/digests may reuse an already verified, still-valid immutable
root after original inputs disappear/change; a new URL/spec/SRI is a distinct
request. Missing/mismatching bytes are generation-scoped transport failure;
newer generation demands retry, not in-place repair of a published root.
repository_io materialize_native_with_runtime captures outside the state lock,
revalidates the session and retains provisional AssociatedImmutable roots;
observe_native/accept and final source certificates remain the only publication
route. Stale session/discard drops provisional ownership. Successful reuse still
requires existing root/source observations, never mere original-path existence.
Relevant DICE guidance is docs/developers/dice.md and vendored
dice/dice/docs/{writing_computations,transients}.md (concept only, no donor code).
No DICE key/lock/equality, injected-input protocol or cross-request cache changes.

**Regression boundary.** Preserve real native file capture, all-role size
limits, rejection and race/cancellation cleanup, mirror/no-TLS behavior,
immutable A/B/A/reuse and native session generation/repair/stale-token
publication tests. Reuse existing DICE request/generation regressions; fake
Materialized values do not prove the native path. Upstream HTTP/JVM details
outside the admitted source subset remain deferred.

The original implementation's file/line caps and timeout60 proof envelope are
historical evidence at `c5e7414d7:thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`,
lines 5229–5365. Current validation uses the orchestration skill's separate
compile/preparation and 12-second/15-second absolute runtime limits. This
accepted capture contract does not prove the complete CLI source closure or
combined R2 sharing gates.

## Historical evidence

Full checkpoint chronology and exact source/validation receipts remain at
`c5e7414d7`, indexed in [plan-history.md](../plan-history.md). The three
checkpoint companion files are historical indexes, not append destinations.
Update this owner only for changed invariants or gates; ordinary acceptance
belongs in its implementation commit.
