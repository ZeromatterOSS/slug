# Stage 5: Bzlmod and Repository Graph

## Independent generated Label path boundary

WP885 admits module_ctx.path(Label) for an independent generated owner only
through its completed source-owned certificate, guarded canonical load route
and actual immutable materialization. Metadata rejects the same owner before
evaluation; selected seed owner must match. Existing BUILD/package lookup,
path namespace and writer/content contracts remain exact. Reciprocal, three-
owner and generated defining-.bzl cycles fail without partial effects/events.
The accepted manifest records45 focused gates and the default CLI's next
unsupported scratch_path.get_child frontier; no scratch I/O or full bootstrap
is inferred.

## Selected extension scratch/process decision

WP886 audits the next consumer rather than admitting only its lexical method.
Rules_rust0.73.0 release SHA256
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`
matches the requested descriptor retained by WP885. An exact existing cached
archive was streamed/verified after the single16MiB network acquisition hit
its cap; no retry or cap increase. Selected51 MODULE/.bzl files697959bytes
and49 Rust source files757305bytes were hashed independently. These are
authenticated static branches, not execution beyond the observed get_child.

The demanded closure in extensions.bzl576–766 is scratch config/splicing
writes, branch selection, query/generate execution, sidecar reads/string watches,
hub/subpackage/per-crate reads and repository declarations. Root MODULE.bazel
provides both lockfiles, but environment can force repin. generate_utils.bzl
363–436 queries a present lock unless repin was requested; a nonzero query
errors rather than implicitly repinning. query.rs39–91/lockfile.rs60–110 read
lock/config/manifest bytes and run cargo/rustc --version. lockfile.rs174–205
forwards HOME/HOMEDRIVE/PATHEXT/NIX_LD/NIX_LD_LIBRARY_PATH to those children.
generate.rs133–167 renders directly from a present lock when repin is false;
its generated files and three JSON sidecars are real process outputs. Repin
adds Cargo resolution/network and possible workspace lockfile writes. The
actual root branch/query result remains unproved.

The authenticated executable must own compiled package-version, serialization
and bundled template bytes. Authenticating Rust control flow alone does not
prove that downloaded binary's build identity; a native algorithm/rendering
port would need those version/template/serialization inputs pinned separately.

Bazel9.2 at8220c6198837d5c13d53fea211cf3282aa12408a is the authority:
StarlarkPath.java139–155 joins multiple child strings before normalization;
BaseExternalContext.file1443–1501 overwrites scratch files with explicit bytes
and executable mode; read's auto-watch skips self-watch while explicit yes
rejects it (1580–1644); execute1938–2077 merges
inherited environment with set/remove overrides and captures code/streams.
ModuleExtensionContext99–103 deletes scratch on completion/restart/error after
BaseExternalContext226–245 drains async work. Public PathTest49–55 and
RepositoryContextTest259–308/354–361 supply lexical/file/read themes; the named
module-extension tests do not establish process/output-capture lifecycle.

Current ScratchPath relative spelling plus opaque Arc validates invocation
ownership only. Selected download owns verified shared bytes, not a staged
scratch file. Label paths own observed namespace/location, not implicit file
content. The separate repository execute owner includes a repository ordinal,
admits only confined GCC probes and returns no generated output tree; a module
invocation must never impersonate that owner. Duplicate-path rejection in a
repository file plan must not replace scratch overwrite semantics.

The faithful candidate is a distinct selected-extension scratch/content and
execution certificate under its full semantic extension owner, with ordered
calls, input/tool bytes and modes, path provenance, cwd and effective environment.
Pure ordered writes/download bindings and reads of produced bytes precede typed
Need → heap/transaction drop → native fulfillment → immutable epoch → replay.
Captured dynamic output bytes/modes must explain every subsequent read;
temporary Host paths/display tokens cannot serve as semantic identity. Reuse
existing source observations, lease/revocation, accepted-closure pruning and
cancel/drain mechanics without a command cache or global registry. No lock spans
DICE compute; failed/cancelled attempts publish no repos/events/current outputs.

Implementation is not yet bounded: tool runtime libraries/child inputs,
inherited environment and complete bounded output capture remain unresolved.
The next separate design packet must choose a confined query + locked-generate
profile or another complete Rust-native contract, including containment,
dynamic string-watch source resolution, byte/path spelling policy, input mutation
refusal, stream/tree limits and resource-limit errors. Repin/splice/workspace
writes and later imported repository materialization stay deferred unless their
owners are separately resolved. Do not claim full get_child or I/O parity from
a normalized one-string subset; namespace/physical identity stays Slug-native.

Design must classify shared certificate bytes as DICE-retained semantic state,
authorized epochs as command-retained and evaluator/staging/capture buffers as
bounded phase/transfer scratch. Account for descriptor-prefix growth, sharing,
equality cutoff, deselection, abort/restore and shutdown; preserve WP880 reporting.
Use Stage9's selected-download/Arc/compact-utility policy before any retained
representation is implemented. Required later proof is genuine input-consuming
execution/output reads, warm A/B/A content/tool/env/order changes, foreign-owner
refusal, dynamic output/missing/oversize/nonzero cases and cleanup on Need,
timeout/cancel/restore/concurrent requests before another actual-root replay.

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

The accepted WP828 built-in source catalog now includes the complete 13-file
Bazel 9.2 runtime `tools/jdk` subtree, with `BUILD.tools` mapped verbatim to
runtime `BUILD` and all executable bits false. Stage 5 owns the pinned bytes,
sorted catalog-derived listings and Slug-native manifest route identity;
`tools/jdk/BUILD.bazel` is only Slug's Bazel compile-data sidecar. The source
route does not admit JDK BUILD Loading, configured Java toolchains or the full
embedded-tools archive. The next authentic Loading stop belongs to Stage 4's
Java `launcher` configuration-field semantics, not source materialization.

WP833 extends that same immutable source owner with the selected seven-file
Bazel 9.2 runtime `tools/python` package closure: upstream `BUILD.tools`
becomes catalog `BUILD`, accompanied verbatim by its one local `.bzl` load
and six exported files. Exact bytes, SHA-256, nonexecutable modes, sorted
listings, parent-package exports and eight Loading target kinds are accepted;
the catalog-derived route manifest remains Slug-native and changes with the
new bytes. No new source key, filesystem observation, cache or DICE owner is
introduced. Fourteen focused/protected tests and a direct rebuilt Slug Query
pass. The broader embedded Python directory, separate runfiles package,
configured Python semantics and complete root build remain deferred. The
authentic `//:probe` first reports missing `module_version()` in
`@@platforms//:BUILD:21`; parallel loading does not prove all rules_python
paths completed. That next semantic stop belongs to Stage 4 Loading, not
Stage 5 source ingress.

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

### Selected generated archive effects (WP796)

The authenticated `rust_toolchain_tools_repository` definition in pinned
rules_rust 0.73.0 may publish the six audited Linux x86_64 host-tools
`download_and_extract` intents only in their exact URL/SHA/stripPrefix order,
followed by final file effects. The selected DICE owner validates that ordered
shape before publishing its structural plan. Core captures each archive into
SHA-verified bounded scratch, streams XZ/TAR into one provisional root,
applies Bazel-compatible regular-file overwrite and executable mode, then
publishes only after every effect succeeds. Archive URL/order/SHA/prefix and
final file bytes/modes belong to structural source identity; captured bytes and
temporary roots do not enter DICE. Unknown generated-path observations,
unaudited entries, auth and changed source tuples fail closed. The selected
rule's 18-key fallback reproducibility dictionary is validated and discarded
as a Slug-native metadata branch; Bazel 9.2 `repo_metadata` and its cache
semantics are not admitted. An innate `use_repo_rule` source-authentic smoke
reaches materialization; WP797 also admits the original module-extension
route. Both stop later at generated BUILD `iso_date=None` Loading.

### Selected module-extension Host factors (WP797)

For a DICE-owned selected extension, declared `os_dependent` and
`arch_dependent` flags select independent structural Rust Host observations.
Only the declared components enter the retained pure result and post-host
Complete errors, including outer observation terminals. The selected owner
keeps its existing `RepositoryPlatformKey` dependency and canonical
generated-repository name; Need remains transient. The global pure-extension
aggregate, nonzero `facts_version`, nonempty facts and exact Bazel lockfile
factor replay remain unsupported. Bazel 9.2's conditional flag participation
and stable repository naming are exact; raw JVM architecture factor bytes are
not. A source-authentic rules_rust 0.73 smoke reaches WP796's archive
materializer before the independent generated BUILD Loading stop.

### Selected module-extension dynamic environment (WP800)

Selected `module_ctx.getenv` observes an ordinary string name through the
per-name `RepositoryEnvironmentCellKey`, including names absent from the
extension's declared `environ`. An unprepared read yields a transient Need;
the owner verifies the cell against the immutable request snapshot before
replaying the Starlark invocation. Interrupted passes publish no repository
calls or evaluator events, including when host verification fails. Complete
success and post-host error terminals structurally retain declared inputs
plus actual dynamic name/raw-value reads, including observed absence; their
Debug representation redacts values. This keeps equality/invalidation sound
even when generated calls or error text are unchanged. Only 16 distinct
`getenv` names per selected evaluation are admitted (Slug-native), while
`module_ctx.os.environ` remains declaration-restricted. Nonselected
execution, lockfile recorded-input replay and Bazel Host/identity bytes stay
deferred. The authentic rules_python 1.7.0 smoke advances from undeclared
`RULES_PYTHON_REPO_DEBUG` to a separate generated hub LoadLabel stop.

### Selected-BCR regular `0555` mode accepted (WP803)

The selected Core tar extractor admits ordinary regular entries at `0555` in
addition to its prior finite mode set. The existing writer preserves payload
bytes, `mode | 0400` and header mtime, matching pinned Bazel 9.2 for this
mode. The rules_kotlin 1.9.6 BCR archive's first `BUILD.bazel` header and all
other regular entries use `0555`; `REPO.bazel` was only the request that
triggered extraction. Capture integrity, tar path/type/size and special-bit
rejection, immutable source association, generation errors and provisional
publication remain with their existing owners. The authentic build now passes
this materialization stop and reaches protobuf's distinct `proto`
configuration-fragment Loading boundary. Other archive modes/types and full
repository/Loading closure remain deferred; M7A is partial.

### Associated module declaration projection accepted (WP834, 2026-09-24)

The root Loading anchor and selected canonical/selected-registry routes
expose borrowed evaluated declared name/version to caller BUILD Loading.
The selected nonregistry declaration may differ from its empty lookup-key
version; route equality retains the whole definition and hash projections
include the declaration. Same-DICE selected nonregistry A/B/A and
selected-registry apparent/canonical tests pass. Generated, direct-local and
builtin routes project no admitted metadata; no new key, source read or
retained owner is introduced. Public nonregistry package-loading parity is
not claimed by this route proof.

### Ordered generated symlink effect accepted (WP837, 2026-09-24)

The selected repository-rule plan retains a distinct ordered Symlink effect:
safe relative destination, immutable external Label path/namespace and
DICE-observed source bytes. Structural equality and a separate
domain-separated Slug source association include all three; File/Archive
domains and prior exact slices remain unchanged. The builder rejects
cross-kind duplicate/ancestor destinations and symlink/archive mixtures.
Core's provisional writer creates a real Unix link without pre-deleting a
destination, rejects symlinked ancestors, and publishes only accepted roots.
Accepted source roots remain live across reselection; discard and runtime
teardown release provisional and retained roots. Focused builder, Core
lifecycle, and fresh Bazel/Slug linked-BUILD evidence pass. Full C++/root
bootstrap remains open.

### Guarded GCC repository execute accepted (WP839, 2026-09-24)

The selected repository-rule owner stages prior generated files and records
source-ordered GCC `repository_ctx.execute` requests and results in a
lease-scoped DICE transcript. Repeated argv remains distinct by call index;
changed environment/compiler inputs invalidate the owner, and Need/abort
publishes neither a successful transcript nor a partial repository. Core
admits only the pinned rules_cc Linux GCC probe grammar, executing in a
read-only, network-isolated bwrap sandbox with bounded output and time. The
Linux/GNU launcher uses direct low-commit `posix_spawn`, runtime-gated
close-from actions, isolated status FD 3, and cancellation-safe group
kill/reap. These process limits and Rust Host observations are Slug-native;
no arbitrary repository command, general path text, compiler flags or full
toolchain is admitted. The Bazel/Slug offline fixture passes, and authentic
Rust-toolchain cquery advances through two GCC `-v` probes before the
pre-existing WP838 `which` candidate cap. Later probes and M7A closure remain
open.

### Selected repository which aggregate budget accepted (WP840, 2026-09-24)

Selected Loading now distinguishes a repeated prepared candidate from an
exhausted aggregate budget without printing host paths. It permits at most
832 distinct ordered candidates across one repository-rule invocation,
derived from the pinned Linux rules_cc phase's 13 possible tool names and
WP838's existing 64-candidate per-call bound. The larger aggregate budget
is Slug-native; exact first-hit/None lookup behavior within the admitted
Unix slice, raw PATH-cell and observed Host-path DICE dependencies remain
unchanged. The existing normalized-path fixture gained one independent
65-miss-plus-`sh` row; Bazel generation/verification and rebuilt Slug replay
pass. Authentic cquery passes the prior limit and reaches Core's separate
generated GCC-prefix policy, without establishing the next probe's argv or
full toolchain. No new DICE key, cache or path owner is introduced.

### Selected generated GCC template prefix accepted (WP841, 2026-09-24)

The selected rules_cc `cc_autoconf` owner now rejects a present declared
`HOMEBREW_RUBY_PATH` before its first template can render raw environment
values; the two earlier GCC `-v` requests remain replayable. Core admits
only the source-ordered two links, exact executable `empty.cc`, and three
named executable rendered files before its existing guarded GCC argv, with
64-KiB-per-file and 128-KiB-total Slug-native caps. Loading owns rendered
bytes and source/environment DICE observations; Core attests only ordered
shape, not template provenance. The original offline GCC row and a fresh
six-effect Bazel/Slug row pass. Authentic cquery clears the former prefix
rejection and next stops at unsupported `path.dirname` in pinned
`unix_cc_configure.bzl:500`; full toolchain and M7A remain open.

### Selected repository path dirname accepted (WP842, 2026-09-24)

For selected `repository_ctx.which` executables and already observed-
present absolute Host paths, `dirname` now yields a lexical Host parent;
root yields `None`. Host path string text is plain valid Unicode rather
than the prior debug-quoted Rust `Path` Display. The observed child proves
parent existence, so no new Host probe or DICE key is introduced. Missing
Host, Label-source and Generated path dirname remain unsupported; derived
parents cannot become Which executables or Label sources. A fresh third
normalized-`which` Bazel/Slug row passes alongside the unchanged two
rows. Authentic cquery clears `path.dirname` and reaches a separate Core
prefix stop after the source-authored `builtin_include_directory_paths`
effect; full toolchain and M7A remain open.

### Selected GCC include-path prefix accepted (WP843, 2026-09-24)

After the six-effect rules_cc prefix, Core now admits only one executable
regular file named `builtin_include_directory_paths`, capped at 64 KiB;
wrong kind, order, mode, destination, eighth effect and overflow fail before
staging. Existing 128-KiB wrapper aggregate, Homebrew gate and finite GCC
argv grammar remain unchanged. Loading's selected repository-rule owner
retains the file's exact bytes/mode in its ordered DICE transcript; Core
attests only shape, with a Slug-native cap and confined read-only staging.
A fresh independent seven-effect Bazel/Slug row and adversarial inert-file
test pass. Authentic cquery clears the GCC probes and reaches generated
rules_cc `BUILD` loading, where `native.cc_toolchain_suite` is unsupported;
call-14 argv was not separately captured and full toolchain/M7A remain open.

### Built-in C++ whitelist package accepted (WP845, 2026-09-25)

Slug's immutable `@bazel_tools` catalog now includes the single exact
Bazel 9.2 `tools/build_defs/cc/whitelists/parse_headers_and_layering_check/BUILD`
file: 241 non-executable bytes with an empty package group and a `:srcs`
filegroup. A local `BUILD.bazel` sidecar supplies compile data but is not
runtime content. Existing source/listing DICE owners expose the file and
its ancestor directories; the ordered Slug-native catalog manifest identity
changes without a new key or hash scheme. Fresh four-row Bazel/Slug queries
and byte/mode tests pass. Authentic Rust-toolchain cquery clears the former
missing-package stop and reaches the configured native-filegroup
nondefault-`licenses` guard. Both this runtime BUILD and rules_cc's
generated `BUILD.tpl` declare `licenses(["notice"])`; the diagnostic omits
the target label, so the exact triggering filegroup and C++ closure remain
open.

### Built-in Xcode extension definition ingress accepted (WP848, 2026-09-25)

The selected-extension owner admits only the pinned built-in
`@@bazel_tools//tools/osx:xcode_configure.bzl%xcode_configure_extension`
definition for the `local_config_xcode` demand. It copies the selected
graph's built-in route identity into the existing Root source route and
retains the complete definition-host mapping in the request. Other built-in
extensions remain typed unsupported, and the public canonical selected-
definition key still returns `BuiltinDeferred`; there is no generic built-in
Bzl-load mapping admission. Direct pinned-source Loading, actual built-in
MODULE demand/warm DICE, route-error and A/B/A mapping tests pass. Authentic
cquery clears the former built-in-route panic and next stops at
`@@apple_support+//xcode:xcode_config.bzl:34` on `ctx.fragments.apple`.
Extension invocation effects, generated repository bytes, Apple providers,
and M7A closure remain deferred.

## Historical evidence

Full checkpoint chronology and exact source/validation receipts remain at
`c5e7414d7`, indexed in [plan-history.md](../plan-history.md). The three
checkpoint companion files are historical indexes, not append destinations.
Update this owner only for changed invariants or gates; ordinary acceptance
belongs in its implementation commit.

### Built-in def_parser package accepted (WP854, 2026-09-29)

The immutable Bazel 9.2 source catalog now owns the complete two-file Linux
installed tools/def_parser package: BUILD.tools renamed verbatim to BUILD
(971 bytes) and no_op.bat (710 bytes), both release-archive mode 100644.
The compile-data BUILD.bazel sidecar is excluded from runtime bytes. Existing
source/listing and manifest route owners expose this package without new DICE
keys, materialization or caches. Exact byte/mode/manifest/directory/package
inventories and real configured source forwarding pass. The installed selector
is preserved; default Linux target/exec returns no_op.bat, while Windows parser
source/prebuilt execution remains deferred. The rebuilt unchanged authentic
cquery clears the missing package and reaches cpp.grte_top in rules_cc.

### Selected BCR relative leaf-file symlinks accepted (WP878, 2026-09-30)

The existing bounded tar extractor now preserves relative symlink text and
installs links after ordinary files, matching pinned Bazel 9.2 source behavior
for the demanded rules_jvm_external 6.7 archive. Symlink mode0777/zero payload,
normalized paths, leading dot/parent components contained within the stripped
root, and a final extracted ordinary-file target are Slug-native admission
guards. Namespace collisions in either archive order, interior dot cancellation,
absolute/escaping/dangling/chained/directory targets and linked transforms fail
closed. Session cancellation and provisional-root disposal remain authoritative;
existing archive/transform hashes own the link bytes without a new DICE key or
source-association domain. Windows retains the unsupported stub.

Independent DESIGN/FINAL accepted the implementation and evidence:16 exact
focused/protective tests, an explicitly selected full559-entry hash-verified
archive probe, and the direct CLI build pass. This fixes an authentic callback
replay's extraction prerequisite; callback activation and full bootstrap remain
unadmitted until their separate gates pass.
