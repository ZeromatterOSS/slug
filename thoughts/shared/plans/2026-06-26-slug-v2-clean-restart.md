# Slug V2 Clean-Restart Implementation Plan

## Canonical Status

This is the canonical Slug implementation plan after the V1 archive decision.
The January roadmap and numbered V1 subplans remain valuable reference material,
but new implementation work should start from this plan and the V2 subplans
under [slug-v2-subplans](./slug-v2-subplans/).

Slug keeps its name and repository. The archived V1 implementation is a Buck2
fork migrated toward Bazel compatibility. V2 keeps the proven lessons and
selected code from V1, but the active trunk is a Bazel-shaped Rust
implementation from the first architectural boundary.

## Live Status

This table is the scheduling authority for a clean root agent. Historical
checkpoint sections below are evidence only. A request such as
`/goal follow the implementation plan` uses the repo orchestration skill and
advances the **Current packet**, not an older `next` paragraph.

| Milestone | Status | Accepted evidence | Blocking gap | Current or next packet |
|-----------|--------|-------------------|--------------|------------------------|
| M0: archive and baseline health | **accepted** | both archive refs peel to `e218054d…`; clean-root checker green in `9897e940` | none | preserve the refs and checker gate |
| M1: one semantic spine | partial | retained `WorkspaceRuntime`, injected file/directory observations, DICE-prepared loading/glob transitions; serialized validation wrapper `0618a007`; six-fixture Bazel 9.2 bzlmod runtime-input oracle accepted in `911f16f2`; neutral workspace-file owner `00422fdc`; root-module evaluator/DICE core `58e9faa4`; request-local command/daemon transport and loading mapping dependency `3f84e34d`; semantic visible-lockfile v28 DICE read `6d354e10`; registry/yanked owner audit accepted as an oracle-first replan; deterministic remote update/refresh/error oracle `2e9a3a56`; registry policy/IO substrate accepted in `f71ef02d`; Bazel 9.2 registry-command transport oracle `3bc88fd9`; command/daemon registry transport accepted in `2777b6f8`; local registry replay oracle accepted in `0211982c`; Bazel-shaped local replay ownership accepted in `6491a55a`; root override routing oracle accepted in `256c02e2`; compact root override owner accepted in `a5f13bf9`; portable workspace-URI harness accepted in `de58ba16`; nine-row patch/local/archive/Git source-preparation oracle accepted in `183970d9`; raw/local/immutable source-input materialization accepted in `9c2a6814`; registry/non-registry MODULE-byte preparation and ordered root patches accepted in `0445cafd`; eleven-row local replay and nonroot evaluation-ordering oracle accepted in `51bfc915`; three-fixture complete nonroot semantic evidence design accepted; nonroot graph/repo-mapping oracle accepted in `908c7c62`; nonroot extension-semantics oracle accepted in `8824135a`; nonroot registration/flag-alias consumer oracle accepted in `eeea40a6`; complete compact evaluator/schema design accepted; compact nonroot schema and MODULE syntax inspector accepted in `c663fe46`; bounded raw-attribute oracle design accepted; 12-row raw-attribute oracle accepted in `cffc39b0`; post-file bounded snapshot design accepted; compact deferred-attribute snapshot and ordered adapter projection accepted in `d4fb5d65`; GC-rooted private single-file directive-evaluator design accepted; private one-file directive evaluator accepted in `b738547d`; bounded nonroot include-composition oracle design accepted; six-row nonroot include-composition oracle accepted in `203cdaac`; prepared-program include-composition redesign accepted; `extra` and borrowed-slice dispatcher attempts stopped cleanly on distinct lifetime boundaries; evaluator-owned `Rc`-slice opaque-index dispatcher accepted; private supplied-file include composition accepted in `72e132a1`; compact typed preparation-provenance design accepted; typed preparation provenance accepted in `0494db65`; discovery rereview preserved the stable composition contract but replanned on three unowned Bazel boundaries; discovery-boundary oracle design corrected after executable nonreplay evidence; eight-row package/print/default discovery-boundary oracle accepted in `12bb70a1`; package-lookup design retained identity/priority/order but replanned on directory and policy ownership; neutral workspace-directory owner lift accepted in `e440e707`; focused nonroot package-policy oracle design accepted and corrected after isolated HEAD exposed seven stale manifest modes; 15-row package-policy oracle accepted in `60c24045`; package-lookup rereview froze a four-owner sequence and replanned on unresolved repository path state; repository path-state design proved the eager one-commit snapshot cannot boundedly cover Bazel-compatible ancestor symlink escapes and replanned to exact retained-daemon evidence before a request-boundary redesign; focused 16-row repository path-state oracle design and oracle accepted; five-packet fixture-growth checkpoint accepted with no pruning or replay; demand-boundary design preserved a typed sparse owner/runtime restart architecture but replanned before Rust; filesystem-authority feasibility accepted Bazel's command-boundary invalidation plus live lazy reads and removed the invented request-atomic premise; command-scoped final-root effect sidecar design accepted; corrected typed observation/retry/event/immutable-instance architecture accepted; activation-substrate first attempt stopped cleanly and its version-lifecycle/API scope correction was accepted; exact-version DICE activation closure accepted in `d775986d`; split neutral event/path substrate contract accepted; structural path/sparse owner accepted in `c26d14c6`; neutral evaluation event batches accepted in `19366fbe`; operational path-resolution/byte-projection design accepted; first resolver implementation attempt cleanly replanned before retained Rust; corrected operational resolver state machine accepted; second resolver implementation attempt cleanly replanned before retained Rust; corrected operational/semantic error and integrated retained-engine evidence gate accepted; third combined implementation attempt stopped cleanly before retained Rust; serial operational/semantic implementation checkpoint split accepted; operational resolver Owner A and semantic byte projection Owner B accepted; corrected Host root-patch migration design accepted; eleven-row root-patch special-file/two-phase oracle accepted; Host root-patch observation migration accepted; local repository source-read migration design and Host implementation accepted; immutable materialization-instance authority design and implementation accepted; immutable repository source-read migration design and implementation accepted; runtime materialization retry design replanned before Rust on pinned Bazel cache semantics and the inside-DICE IO boundary; pure repository materialization request/result design accepted; repository materialization request/result ownership accepted in `5150dd8f`; runtime preflight/retry contract frozen and split into oracle-first prerequisites after terminal replan; six-state Bazel 9.2 Local repository lifecycle oracle accepted in `dcc19327`; private two-file path-producer design retained but replanned before Rust on exact desktop-Windows path/query semantics; its first correction fixed long paths and metadata staging but replanned on Bazel's all-reparse-point link classification; exact Windows lstat correction accepted; first producer draft reached 15 focused tests but was removed after native Windows ReadLink and iterator-only Unix EIO gaps; native ReadLink design preserved but replanned on a public NotALink discriminator and raw same-handle Unix readdir; fieldless NotALink schema and raw same-handle Unix directory design accepted; three-file NotALink schema and equality propagation accepted; second producer draft removed after operation-order, Windows formula/safety, deterministic-order, authority, and evidence gaps; final primary-operation-first, compact-authority, raw Unix/Windows owner, exact platform-formula, and exhaustive-evidence correction accepted by three terminal rereviews; final combined implementation linked on Windows but was removed after its single correction still lacked the required platform-owner, operation-race, preflight, parser, and lifecycle matrices; serial dormant-neutral, complete-Unix, and complete-Windows implementation checkpoint split accepted; dependency-free neutral authority/preflight/refinement kernel accepted with 15 scripted tests and cross-target compilation; cfg-Unix native operations, raw same-handle directory owner, and complete real lifecycle accepted with 10 focused tests; cfg-Windows copied ABI, safe scripted semantics, native handle owners, and real kernel32-linked build accepted with 10 focused tests; first retained-materializer design replanned before Rust on overlapping-session and exact archive-order/normalization gaps; single-session lease and exact captured-archive ordering/normalized-prefix correction accepted; combined retained-materializer/archive implementation replanned before Rust on independently large raw-USTAR and session-state evidence surfaces; serial HTTP-USTAR, pure retained-session, and native-owner bridge split accepted; first HTTP USTAR implementation attempt cleanly replanned before retained Rust on pinned short-record/numeric/path semantics and inspect-before-extract/capture-stage ownership; exact HTTP USTAR capture/parser/extraction correction accepted; corrected HTTP captured-USTAR implementation accepted; pure retained-session kernel accepted; retained native-owner bridge accepted; dormant attempt/effect sidecar accepted; root-MODULE producer replanned on inline include execution ownership; compact root include-composition event oracle design and eight-row Bazel 9.2 oracle accepted; aggregate root-MODULE composed evaluator/event design accepted; composed root-MODULE evaluator/schema and marker-conditional composite event producer accepted; corrected workspace-lifetime runtime-native demand-producer design, demand-key metadata, and workspace demand provenance bootstrap and retained native materialization bridge accepted; dormant runtime native demand session accepted; shared retry/publication driver checkpoint accepted; terminal event/execution oracle design accepted; generic terminal-oracle REAPI applicability accepted; first six-file terminal oracle implementation replanned cleanly on a stale direct parser consumer; corrected seven-file allowlist and nine-row terminal oracle accepted; first loading event producer implementation replanned after its copied-globals owner caused a downstream SIGSEGV; one-heap loading globals owner correction design, corrected local loading event producer, and local analysis event producer accepted; private shared retry driver and Host directory semantic projection accepted; root/raw Host migration design replanned before Rust on private registry policy/transport/error ownership; corrected root/raw Host registry-boundary design and one-file registry remote bridge accepted; corrected Host root-module-file owner accepted; exact Bazel v28 lockfile schema and private-owner/cutover design accepted; exact 15-row Bazel 9.2 lockfile-v28 schema oracle accepted; private Bazel 9.2 v28 lockfile owner accepted; corrected raw-read v28 live-cutover design and complete direct-DICE closure correction accepted | production retry/publication activation remains gated on Host migrations and typed production roots; HTTP and Git PAX/GNU/link/special parity, resolved repository path state, deleted-package request ownership, repository-ignore ownership, package lookup, omitted-`module()` defaults, and discovery composition remain | resume only the accepted sixteen-file raw-read v28 live cutover |
| M2: analysis graph | partial | recursive custom-rule configured analysis, returned providers, target-local actions | configuration, transition, toolchain/platform, repository-mapping, and broader action ownership gates remain | no new M2 packet while the M1 source-input owner is current |
| M3: `query` | **active** | parser/evaluator/loading graph; 13 of 16 Bazel default functions; `executables` accepted in `69565a29`; evaluator ownership split accepted in `65c6c54f`; Java `Pattern` feasibility completed and `java_regex` 0.1.0 rejected against `5e78abc1`; `tests(EXPR)` 32-command oracle through `1edb2775`, loading/query metadata through `7abcbdce`, and request-local activation through `3a8ae78a`; labels metadata 39 through `57192df9`; identity, package-context normalization, structural comparison, and direct duplicate rejection through `5bbc4604`; 39-command visibility oracle through `a376e30e`; typed visibility/package-group graph through `f9ae7337`; request-local `visible()` activation through `76025ede` | three Java `Pattern`-dependent functions, external repositories/pattern breadth, and remaining command breadth | pause function activation until an exact Java-compatible engine is accepted; the M1 source-input owner is current |
| M4: `cquery` | not started | command/parser placeholder only | M3 and configured-target breadth | none |
| M5: `aquery` | not started | retained narrow action fixtures only | M4 and exact Stage 6 action graph/formatters | none |
| M6: execution and caching | gated | retained REAPI/NativeLink regression fixtures | exact `aquery` handoff | preserve regressions only |
| M7: command/ruleset breadth | gated | early command/ruleset scaffolding | M5 and M6 | none |
| M8: bootstrap | split | Stage 10 plan permits an independent Bazel developer graph | Bazel graph not started; self-hosting awaits M5/M6 | now eligible as a disjoint proposal, but not ahead of the validation-wrapper critical path |

Latest M1 accepted evidence: the corrected Host visible-lockfile oracle passed
pinned Bazel 9.2 generation and two distinct fresh-root replays with exits
`0,0,48,0,48,48,0,48,37`. Its nine cumulative query rows prove absent and
populated Off behavior, caught malformed input, first-marker version gating,
unsupported Error mode, merge advice, Java UTF-8 replacement, absent-Error
progress through a deterministic scheme-less URL sentinel, and the direct
adapter crash. The existing fixture changed in exactly three paths and now has
six regular files, zero links, and 553 lines; all terminal source,
implementation/evidence, and orchestration reviews returned `ACCEPT`. No
failed draft, registry scaffold, network claim, harness, Rust, Cargo,
production edge, or activation is retained.

The corrected Host RegistryFunction boundary design is accepted. The private
construction key is root-free and ordered exactly as Bazel 9.2; later
composition owns root first, override/list choice, all-registry construction,
and sequential fetch. The design freezes unconditional full visible-lockfile
consumption, raw mirror identity, a dedicated Refresh invalidation token, the
exact HTTP(S)/file hash table, selected-yanked and vendor behavior, sticky
local RegistryIo reuse, complete-only selective equality, and an oracle-first
serial owner sequence. All three terminal latest-text reviews returned
`ACCEPT`; no Rust, fixture, Cargo, IO behavior, consumer, or activation
changed.

### Current packet

Run only `WP-5-m1-host-registry-function-oracle-design`.

This packet is design only: do not edit Rust or fixtures and do not run Cargo.
Reuse the accepted registry-yanked/transport scaffolds where bounded and
freeze the smallest discriminating rows for remote Off recorded
absence/SHA/yanked reuse, Refresh invalidation, module-mirror precedence and
invalid mappings, and exact registry-vendor hit, fatal read with no network
fallback, and non-vendored/checksum-absent network request behavior. Preserve
every implementation and activation gate.

### Replanned semantic-error/evidence contract (preserved for correction)

Run only
`WP-5-m1-operational-path-resolution-semantic-error-evidence-design-correction`.

Perform a read-only correction of the accepted resolver contract before any
more Rust. Preserve the independently validated explicit parent/target frame
machine, exact route splitting, portable roots, raw provenance, marker
asymmetry, fail-fast DICE boundary, and two-file implementation scope.

Freeze one noncontradictory error/equality model for the operational resolver
and semantic byte projection. In particular, decide how `WrongKind` retains an
OS-native diagnostic path and how observation, inconsistent-state, cycle, and
expansion errors remain fully typed without leaking namespace, materialization
instance, physical root/path, or operational route through byte-value
equality. Specify exact public variants and field-by-field operational and
semantic comparisons rather than relying on derived equality.

Freeze an executable test harness before retrying implementation. It must use
observation-backed `ResolvedPathKey` computations for exact self, A→B→A,
prefixed-cycle, relative/absolute descendant-expansion, successful ancestor
marker, ancestor-symlink, transitive-link, dangling-target, raw non-UTF-8, and
parent provenance/marker cases. It must also use stable test-only selector and
downstream counter keys on one retained DICE engine to prove resolver
recomputation and byte-consumer pruning across symlink retarget,
materialization-instance, real-root, metadata, and route changes, plus exact
A→B→Missing→typed-error→A invalidation and restoration.

Name the exact schemas, selector/counter key topology, epochs, expected
`path_to`/`chain`/route/provenance values, and staged assertions. Stop on a new
production owner, dependency, file, runtime/consumer migration, or any attempt
to replace the missing integrated evidence with direct helper/equality calls.

### Prior accepted implementation contract (blocked on the correction above)

Run only `WP-5-m1-operational-path-resolution-byte-projection`.

Add new `app/slug_workspace_v2/src/path_resolution.rs` and only its public
reexports in `src/lib.rs`; add no dependency and do not edit the accepted
observation owner. Implement the exact operational `ResolvedPathKey` and
semantic `PathFileBytesKey` schemas/equality from the accepted owner plan.
Cycle, infinite-expansion, and ancestor-marker values each retain separate
ordered shared `path_to` and `chain`; byte wrong-kind is a dedicated
projection error, not a fabricated observation failure.

Implement one private iterative `ResolutionMachine` over a `Vec` of
independent resolver frames. Each frame owns one requested path, its
Begin/parent-wait/route-replay/lstat/terminal-link/readlink phase, ordered and
sorted logical chains, physical raw-link provenance, and first ancestor
marker. The pure transition seam returns PushParent, exact Observe, or
Complete; the async adapter alone services DICE observations and must hold no
frame borrow across await.

A nonroot caller suspends immediately below a fresh parent frame. On parent
completion, append its raw-link provenance, derive
`parent.real_path + basename`, never copy its ancestor marker, and propagate
errors unchanged. Missing or non-directory parent means caller-local Missing
at that derived path with no route admission or child lstat. Directory parent
routes replay entry-by-entry with the basename through the caller's chain
before real-child lstat. Root check-and-admits and lstats itself and follows a
synthetic root symlink rather than short-circuiting.

After symlink lstat, demand ReadLink; Missing is inconsistent and Error retains
the exact demand. Retain the physical link/raw OS target, normalize an absolute
target from its filesystem root or a relative target from the physical link
parent, run check-only, then resolve target ancestors in the same outer frame;
only its parent gets a fresh frame. Repeat without a cap. Preserve Windows
Prefix plus RootDir and Unix RootDir anchors.

Maintain one unique sorted vector beside insertion order. Exact repeat splits
the old route before its first equal entry without appending the repeat.
Strict descendant of the predecessor splits `old + candidate` before that
predecessor and terminates. A successor strictly below the candidate records
the first nonterminal split from `old + candidate` and continues. Check-only
never inserts; admission inserts at the known binary-search position and
appends once. Ordinary `/a/b/c` must be only `[/a/b/c]`; `/link -> /x/y`
with `/x -> /z` must be `[/link, /x/y, /z/y]`.

Only an actual `Ok(PathOutcome::Need(_))` may propagate as Need. Pass every
resolver-level `ctx.compute` through a private track-caller fail-fast
invariant helper; never stringify/cache an infrastructure error or invent a
demand. A requirement for recoverable DICE errors is a stop and replan.

Proceed test-first in three checkpoints. First prove pure roots, split logic,
frame push/pop, ordinary and suppressed chains/full suffixes, parent
provenance/marker asymmetry, target-parent replay, and root symlink. Then wire
observations and prove cumulative exact demand order; every relative,
absolute, ancestor, leaf, transitive, escaped, and root-clamped link; dangling
versus races; typed errors; raw non-UTF-8; Need validity; exact cycle and both
expansion shapes; and fail-fast infrastructure evidence. Finally add byte
projection plus same-engine A→B→Missing→typed-error→A, symlink retarget
A→B→A, and materialization-instance/real-root operational-unequal versus
semantic-equal pruning.

Use mutable `Vec` state, one incrementally maintained sorted `Vec`, shared
frozen slices, honest `Dupe`, and `Allocative`. Run full
`slug_workspace_v2` tests/doctests, format, diff, exact two-file allowlist, and
archive guards. Stop on a flattened suffix loop, recursion through
`ResolvedPathKey`, parent-marker copying, raw-provenance loss, fabricated
Need/wrong-kind observation, dependency/file/owner expansion, direct IO,
canonicalization/lossy identity, weakened tests, or any consumer/runtime/
repository/retry/publication work.

### Accepted transport evidence

The implementation must:

1. carry primitive ordered registry strings through both one-shot and daemon
   build/query paths without serializing semantic Rust types;
2. normalizes exactly once into `RegistryUrls` before the sole request commit,
   with Bazel's default BCR behavior and fail-closed diagnostics;
3. restores A→B→A request-local values without leaking between build and query;
4. keeps the already accepted `RegistryFileKey`, generation, IO capability,
   root graph, and loading owners unchanged; and
5. names a narrow implementation allowlist and exact CLI/server/core tests.

1. Add an ordered raw `Vec<String>` registry field to build/query command
   requests and a `#[serde(default)]` primitive registry list to
   `BzlmodRequestInputs`. Empty means unspecified. This packet supports
   repeatable `--registry=URL`; generic `--registry URL` parsing is not
   expanded.
2. `slug_commands_v2` only collects required nonempty values in encounter
   order. Ordinary `query` accepts `registry` in its existing flag validator
   and continues rejecting the other currently unsupported bzlmod flags.
   CLI, JSON, and daemon code do not trim, deduplicate, substitute, validate,
   or carry `RegistryUrls`.
3. Both one-shot and daemon paths pass the primitive list through the existing
   explicit bzlmod methods. The common retained-runtime injection helper calls
   one fallible `RegistryUrls::from_request(workspace, raw)` before allocating
   the request generation or scheduling any `changed_to`.
4. `from_request` supplies only `https://bcr.bazel.build/` when the raw list is
   empty; a nonempty list fully replaces that default. It removes every
   trailing slash and first-occurrence-deduplicates in raw encounter order,
   then performs `%workspace%` substitution and URI validation for each
   surviving entry. Validation accepts only exact lowercase `http`, `https`,
   and `file` schemes with a non-null hierarchical path, preserving host-only
   HTTP(S) and Bazel's factory diagnostic shapes. The stored compact
   `RegistryUrls` are the resolved effective URLs; no later layer repeats
   normalization or substitution.
5. The existing `RootModuleRegistryUrlsKey`, request generation,
   `RegistryPolicyKey`, `RegistryFileKey`, IO capability, root graph, and
   loading ownership remain unchanged. Malformed input fails before the sole
   commit and does not consume a generation.
6. Command tests pin default/override ordering, duplicate raw values, missing
   values, and query acceptance. Server tests pin omitted-field compatibility,
   primitive JSON round trips, malformed recovery, and build/query
   default→override→default isolation. Core tests inspect injected registry
   URLs and generation across the same A→B→A sequence. CLI tests exercise
   both one-shot and daemon equality-form transport.

The implementation allowlist is
root `Cargo.toml`,
root `Cargo.lock`,
`app/slug_bzlmod_v2/src/registry.rs`,
`app/slug_bzlmod_v2/Cargo.toml` for the already-locked `url` parser,
`app/slug_commands_v2/src/common.rs`,
`app/slug_commands_v2/src/build.rs`,
`app/slug_commands_v2/src/query.rs`,
`app/slug_commands_v2/tests/commands.rs`,
`app/slug_cli_v2/src/commands/build.rs`,
`app/slug_cli_v2/src/commands/query.rs`,
`app/slug_cli_v2/tests/cli.rs`,
`app/slug_server_v2/src/server.rs`,
`app/slug_server_v2/src/lib.rs`,
`app/slug_server_v2/src/tests.rs`,
`app/slug_core_v2/src/runtime/mod.rs`,
`app/slug_core_v2/src/runtime/dice.rs`, and
`app/slug_core_v2/tests/runtime.rs`.

Do not edit Rust, add discovery/fallback, fetch registry content, expand rc
handling, or design MVS/yanked/final-hash/writer behavior in this packet.

The rejected regex candidate does not authorize a UTF-16 engine fork.
`filter`, `attr`, and regex-based `kind` remain deferred; any V2-owned engine
requires its own UTF-16, diagnostic, resource, allocation, and
differential-corpus gate.

## Operating Decision

Use the existing repository for continuity, but restart the implementation
shape:

1. Preserve V1 through a tag and archive branch before root-level replacement.
2. Keep V1 source as extraction/reference material, not as the default build
   graph for V2.
3. Build V2 around Bazel 9 semantics, Bazel source/test oracle fixtures, DICE,
   starlark-rust, and REAPI-first execution.
4. Import V1 code only after a small oracle fixture or focused regression proves
   the behavior matches the V2 boundary.

### Future branding TODO

Consider renaming the project to **Rubin**, after Red Rubin basil. The name is
concise, retains the basil theme, and its “red” and initial “R” associations
subtly signal the Rust implementation. Treat this as a future branding decision,
not an implementation milestone or current-packet dependency.

## 2026-07-22 Direction Reset

The immediate goal is not broader build execution. It is one trustworthy,
incremental semantic graph that can reproduce Bazel 9 analysis and expose that
graph through `query`, `cquery`, and `aquery` in increasing order of depth.

The governing order is:

1. Pin all new oracle work to Bazel 9.2.0 at
   `8220c6198837d5c13d53fea211cf3282aa12408a`. The sibling `../bazel`
   checkout may move to Bazel 10 or later; use the tag/commit, not its current
   `HEAD`, for parity evidence.
2. Replace split one-shot evaluation and fallback workspace scanning with one
   daemon-owned DICE graph whose injected inputs cover files, directory
   listings, environment and command policy, repository mapping, loading,
   configured targets, and action declarations.
3. Make configured-target analysis real: recursively analyze dependencies,
   execute rule implementations with prepared Bazel-shaped contexts, consume
   returned providers, and retain declared actions without executing them.
4. Implement full unconfigured `query` over the loading graph, then `cquery`
   over configured targets, then exact `aquery` over the same action graph
   Stage 6 produces.
5. Treat matching `aquery` output as the execution handoff. Only after this
   gate should new execution/cache breadth, `run`, `test`, or broad ruleset
   conformance control the next milestone.
6. Maintain a Bazel 9 build graph for Slug itself so Bazel plus BuildBuddy can
   accelerate development. After analysis, action graph, and execution are
   correct, prove a Bazel-built Slug can build Slug and then reach a Slug-built
   fixed point.

The already-landed first-build and NativeLink-backed REAPI fixtures remain
valuable regression tests. They prove a narrow vertical slice; they do not
prove the DICE ownership, configured-target graph, query surface, or bootstrap
architecture described above.

### Integration-first freeze

- Do not expand Stage 5 with more standalone parser/key/value substrate unless
  the packet is required by the analysis/query/aquery path.
- Do not expand Stage 7 cache, materializer, or backend breadth until the
  `aquery` gate is accepted, except to preserve an already-landed regression or
  to enable the Bazel/BuildBuddy developer build.
- Do not use a real-world build as structural acceptance evidence. Convert each
  discovered gap into a focused Bazel 9 oracle first.
- Historical checkpoint sections remain evidence of what landed. The latest
  priority/gate section in this plan and each owning subplan is authoritative
  when older checkpoint prose says `pending`, `next`, or `first`.

Do not physically move the whole V1 tree into `v1-archive/` unless the tag and
branch archive is not enough. A full in-tree archive makes search, codegraph
indexing, and new-agent orientation worse. If a physical archive is required,
exclude it from active build metadata and codegraph indexing.

## 2026-06-29 Branch Review And Remediation Gate

Review of `codex/slugv2` found that the clean-restart archive sequence has not
actually been completed in this checkout:

- `scripts/v2_archive_status.sh` fails because `slug-v1-archive` and
  `v1-archive` are missing, even though Stage 0 docs recorded them.
- `codex/slugv2` adds V2 scaffolding on top of the full V1 root instead of
  resetting the active tree into a clean V2 root. Relative to `main`, the branch
  adds hundreds of files and no root cleanup.
- `Cargo.toml` still includes the V1 `app/slug*` workspace members beside the
  new `app/slug_*_v2` crates, and the active tree still tracks V1-heavy paths
  such as `app/`, `buck2/`, `prelude/`, and `tests/`.
- A focused V2 compile check passed for the new crates, so the branch is useful
  as a prototype and selective patch source, but it is not the V2 trunk shape.

Do not merge or promote the current `codex/slugv2` branch wholesale as the clean
restart. Before implementation proceeds as V2 trunk, do this sequence:

1. Freeze new feature work on the mixed-root branch.
2. Pick the V1 preservation commit from the live checkout, verify the worktree
   state, then create and validate the `slug-v1-archive` tag and `v1-archive`
   branch.
3. Start the active V2 line from a clean root worktree: keep root pointers,
   V2 plans, and intentionally retained infrastructure; remove V1-only source,
   tests, Buck-shaped metadata, and V1 workspace members from the active build.
4. Re-import from `codex/slugv2` one bounded stage at a time. Each import needs
   an owner subplan, an oracle fixture or Bazel source citation, focused
   validation, and a Stage 9 extraction-ledger entry when it came from V1 or
   from the mixed-root prototype.
5. Run `scripts/v2_archive_status.sh`, `git diff --check`, and the touched
   stage validation before calling the root clean.

Plan-following sessions use
`.codex/skills/slug-agent-orchestration/SKILL.md`.

2026-06-29 execution update: the missing local archive refs have been repaired;
`slug-v1-archive^{commit}` and `v1-archive` now both resolve to
`e218054d4c796655939b968d90208b185decb352`. Cargo root metadata now exposes only
V2 app crates as active `app/slug_*` workspace members/dependencies, with V1
app crates removed from that surface.

2026-06-29 clean-root remediation update: the active clean-root branch is
`codex/slugv2-clean-root-remediation`. It removes tracked V1 source/test trees,
root Bazel/Buck metadata, old CI, old docs, old V1 plans, and the unwrapped
`remote_execution` source candidate from the active tree. The retained tracked
root is orientation docs, V2 plans/prompt, Stage 1 oracle harness, V2 crates,
repo-local V2 skills, `docs/developers/dice.md`, and the explicitly retained
infrastructure crates listed in `V1_ARCHIVE.md`. V1 and rejected mixed-root
surfaces remain available through `slug-v1-archive`, `v1-archive`, and
`codex/slugv2` for staged extraction only.

2026-07-22 live-checkout correction: the annotated `slug-v1-archive` tag still
resolves to `e218054d4c796655939b968d90208b185decb352`, but the local
`v1-archive` branch is absent and the archive checker allowlist predates
`app/slug_server_v2`. Stage 0 is therefore not green in the live checkout; its
owner plan records the bounded repair before M0 acceptance.

2026-07-23 baseline-repair acceptance: after a clean read-only ref audit, local
branch `v1-archive` was restored directly at the recorded commit without
moving or replacing any ref. Commit `9897e940` added only the exact
`slug_server_v2`, `slug-agent-orchestration`, and current root-prompt
allowlists. The normal checker, two negative ref-override probes, and
`V2_ARCHIVE_STATUS_REQUIRE_CLEAN=1 scripts/v2_archive_status.sh` all passed;
Sol-low returned `ACCEPT`. M0 is green.

## Non-Negotiables

- Bazel 9 only. No pre-Bazel-9 behavior, no WORKSPACE support, and no legacy
  toolchain-resolution compatibility.
- Bazel source and Bazel tests are the compliance oracle. A parity claim needs
  a local Bazel source citation or an oracle fixture result.
- DICE owns semantic build state. Do not hide semantic discovery inside
  synchronous Starlark-visible APIs.
- REAPI is the execution boundary. BuildBuddy is the primary scaled remote
  development/CI lane; sibling `../actiond` is the preferred hermetic local
  conformance backend; NativeLink remains a useful regression backend. All sit
  behind the same REAPI boundary.
- Bazel invocations may use ordinary RC discovery and consume the user's
  `~/.bazelrc` for BuildBuddy authentication. Agents and inspection tools must
  never read or copy its contents, and credentials or derived secret material
  must never enter this checkout, logs intended for commit, or Git history.
- Slug-local sandbox implementation is deferred until after analysis, exact
  `aquery`, remote execution, and cache correctness. Backend isolation supplied
  by BuildBuddy or actiond does not count as a Slug sandbox implementation.
- Progress is demonstrated by a vertical Bazel-shaped build, not by independent
  identity, parser, DICE-shaped, action, or REAPI data models. A wrapper trait
  or stable-serialization helper is scaffolding until the owner fixture drives
  it through the real runtime boundary.
- V2 output layout targets Bazel-shaped paths. Any deliberate Slug-specific
  divergence must be explicitly documented as an extension, not assumed.
- V1 plans and code are evidence and extraction sources, not the V2 source of
  truth.

## V1 Material Worth Keeping

Preserve and mine these V1 surfaces:

- DICE-owned bzlmod/replay implementation and tests in `app/slug_bzlmod` and
  `tests/core/bzlmod/test_plan61_guardrails.py`.
- REAPI/NativeLink smoke tests, what-ran evidence, upload/materialization
  checks, and remote action-cache tests from Plans 31 and 34.
- Bazel Starlark API work: `rule(implementation=...)`, `attr.*`, providers,
  depset probes, `ctx.actions`, and selected `cc_common` or `proto_common`
  compatibility surfaces.
- Repository-rule and module-extension lessons, especially lockfile replay,
  repo mapping, watched inputs, and materialization guardrails.
- Plan docs as a bug database for known semantic traps.

Do not import these V1 surfaces without redesign:

- Buck cell identity and fallback cell graph machinery.
- `buck-out` or Buck-shaped output-root assumptions.
- Direct-local executor shortcuts used as compatibility proof.
- Process-global semantic registries, hidden bridges, or fallback scanners that
  bypass DICE ownership.
- BXL or other Buck-derived user surfaces unless deliberately scoped as Slug
  extensions after Bazel compatibility is stable.

## Stage Map

| Stage | Owner Plan | Parallelism | Checkpoint |
|-------|------------|-------------|------------|
| 0 | [00-v1-archive-and-clean-root.md](./slug-v2-subplans/00-v1-archive-and-clean-root.md) | Serial | V1 is tagged/branched, V2 root docs and metadata are active, archive policy is clear. |
| 1 | [01-compliance-oracle-harness.md](./slug-v2-subplans/01-compliance-oracle-harness.md) | Parallel | A fixture runner compares Java Bazel and Slug V2 for exit status, outputs, events, and selected diagnostics. |
| 2 | [02-rust-skeleton-and-runtime-substrate.md](./slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md) | Parallel | Minimal Rust CLI/server skeleton uses actual Buck2 runtime crates without exposing Buck semantics. |
| 3 | [03-bazel-identity-and-layout.md](./slug-v2-subplans/03-bazel-identity-and-layout.md) | Parallel after Stage 2 starts | Labels, repositories, packages, target ids, execroot, and output paths are Bazel-shaped. |
| 4 | [04-starlark-loading-and-build-packages.md](./slug-v2-subplans/04-starlark-loading-and-build-packages.md) | Parallel after Stage 3 basics | `BUILD.bazel` and `.bzl` loading work for small packages with Bazel globals. |
| 5 | [05-bzlmod-and-repository-graph.md](./slug-v2-subplans/05-bzlmod-and-repository-graph.md) | Parallel after Stage 3 basics | Starlark-evaluated `MODULE.bazel`, registry, repo mapping, extensions, repo specs, and lockfile policy are DICE-owned. |
| 6 | [06-analysis-toolchains-and-actions.md](./slug-v2-subplans/06-analysis-toolchains-and-actions.md) | Parallel after Stages 4/5 | Configured-target analysis, toolchains, providers, depsets, and action declarations pass focused oracle fixtures. |
| 7 | [07-reapi-native-execution.md](./slug-v2-subplans/07-reapi-native-execution.md) | Parallel with synthetic actions, then after Stage 6 | Shell and ruleset actions execute through REAPI with upload, AC, materialization, and zero direct-local proof. |
| 8 | [08-ruleset-and-command-conformance.md](./slug-v2-subplans/08-ruleset-and-command-conformance.md) | Query after loading/analysis; execution commands after aquery | `query`, `cquery`, and exact `aquery` pass before ruleset, run, test, and BEP breadth. |
| 9 | [09-v1-extraction-ledger.md](./slug-v2-subplans/09-v1-extraction-ledger.md) | Continuous | Every V1 or Buck2-derived extraction has an owner, oracle proof, and cleanup decision. |
| 10 | [10-bazel-build-and-bootstrap.md](./slug-v2-subplans/10-bazel-build-and-bootstrap.md) | Bazel developer graph may start now; self-hosting follows exact aquery and execution | Bazel 9 builds/tests Slug through BuildBuddy, then Slug reaches a stage1/stage2 self-build fixed point. |

## Current Milestone Overlay

The numbered stage files are ownership boundaries, not the implementation
order. Use this overlay for scheduling new packets:

| Milestone | Required result | Owning stages | Exit gate |
|-----------|-----------------|---------------|-----------|
| M0: archive and baseline health | V1 refs and clean-root checker are truthful; Bazel/Buck2/actiond sources are pinned | 0, 1, 9 | Archive status is green and every new fixture carries immutable provenance. |
| M1: one semantic spine | One daemon-owned DICE instance and explicit create/edit/delete inputs serve loading, bzlmod, analysis, and commands | 2, 4, 5 | Same-daemon tests explain invalidation without a fallback scanner or fresh per-request graph. |
| M2: analysis graph | Recursive configured targets return real providers and deterministic declared actions without execution | 3, 4, 5, 6 | Focused Bazel analysis fixtures and upstream test themes match. |
| M3: `query` | Full Bazel 9 unconfigured query language evaluates the loading graph | 8, 9 | Parser, function, set-operation, pattern, ordering, and formatter fixtures match. |
| M4: `cquery` | Configured query reads the same configured-target graph as analysis | 6, 8 | Configuration identity, transitions, providers, and Starlark output match. |
| M5: `aquery` | Action query reads the same Stage 6 action graph and implements Bazel 9.2.0's text, commands, summary, textproto, proto, streamed_proto, and jsonproto formatters | 6, 8 | Normalized `ActionGraphContainer` content and human-readable renderings are identical for the gate matrix. |
| M6: execution and caching | Stage 6 actions execute and replay only through REAPI | 7 | BuildBuddy and local actiond evidence prove upload, execute, AC, and materialization with zero direct-local actions. |
| M7: command/ruleset breadth | `build`, `run`, `test`, BEP, and public rulesets use the accepted graph and executor | 8 | Focused public fixtures match; stress projects remain supplemental. |
| M8: bootstrap | Bazel-built Slug builds Slug and reaches a self-hosted fixed point | 10 | Stage1 and stage2 action graphs and declared outputs match. |

M3 progress: implementation commit `61ca25db` lands the first accepted
DICE-backed loading-query thin vertical over the root repository, with
Buck2-derived parser/evaluator/traversal seams and retained-daemon execution.
It passes the Bazel 9.2 `query-parser-and-sets` and
`query-loading-thin-vertical` oracle fixtures through Slug. M3 remains open for
the remaining functions, repositories and patterns, ordering modes, and
formatters; this checkpoint must not be described as full query parity.
Oracle commit `5b7806d7` now pins the next accepted behavior packet for
root-repository subtree patterns, `rdeps`, and
`same_pkg_direct_rdeps`. Implementation commit `cdc5af41` passes that oracle
through the retained daemon with prefix-local package enumeration and
Buck2-derived reverse traversal. M3 remains open for the other 13 loading
functions, repository/pattern breadth, ordering modes, and formatters.
Oracle commit `2b73c08d` now pins the next 43-command packet for `allpaths`
and `somepath`, including bounded arbitrary shortest paths and Bazel's
source-backed root-node `somepath` AUTO-order exception. Implementation commit
`7d851ce9` passes that oracle with direct unbounded reverse-traversal reuse,
Buck2-derived compact BFS/parent reconstruction, exact DICE transitions, and
retained-daemon execution. M3 remains open for the other 11 loading functions,
repository/pattern breadth, ordering modes, and formatters.
Oracle commit `e8e1d9ef` now pins the next 42-command ordinary-query packet for
`some` and the shared signed Java-`int` boundary used by `deps`/`rdeps`.
Implementation commit `b25c8aff` now passes that packet through the retained
daemon: `some` selects from the existing insertion-ordered `TargetSet`, while
the shared FULL renderer deterministically topologically orders the final
selected portion of the request-local evaluation graph. The siblings packet
below replaced the initial semantic selected-induced approximation with
recorded evaluation edges while preserving all `some` oracle rows. It also
carries signed `i32` depth/count values through
`deps`/`rdeps` and renders bare-negative syntax safely for UTF-8 input. Worker
and root each passed the six-crate 82-test suite and all five accepted query
fixtures (133/133 rows). M3 remains open for the other ten loading functions,
repository/pattern/order/formatter breadth; `filter` stays deferred until an
exact Java `Pattern` substrate exists.

The siblings/BUILD-file vertical is now landed: fixture base `8c28877b`,
attribute correction `20f88c05`, FULL-provenance oracle `1a3dec16`, and
implementation `d19a9b29`. `QueryNodeKind::BuildFile` uses the exact active
loaded basename, coalesces an exported active BUILD target, and remains a
zero-edge non-rule node. `siblings` evaluates once and deduplicates packages;
request-local `u32`/`Vec`/`SmallMap` evaluation edges follow Bazel
`BlazeQueryEnvironment` and the Buck2 graph pattern, while FULL renders only
those recorded edges with no render-time DICE read. Exact retained-DICE and
daemon transition coverage passed without adding a key, cache, protocol,
filesystem, lock, or global boundary.

The corrected Bazel update/no-update/root runs were `034446-589899`,
`034516-592708`, and `034623-595736`; FULL-provenance discovery/anchored
update/no-update/root runs were `035638-609525`, `035734-612675`,
`035759-615627`, and `035853-619234`. The Slug gate passed 91/91 and all six
query fixtures passed 176/176: worker `040407-626548`, `040411-626572`,
`040414-626601`, `040418-626692`, `040423-626782`, `040427-626870`; root
`040534-628098`, `040540-628123`, `040546-628189`, `040549-628247`,
`040554-628339`, `040558-628428`. M3 now has nine deferred functions;
`filter` remains deferred pending exact Java `Pattern` parity. `buildfiles`
and `loadfiles` remain separate transitive-load/fake-target work.

Gate A of `WP-4-8-m3-build-load-files` is now accepted in `791e26b2`.
The crate-private `app/slug_query_v2/src/provenance.rs` plus its one-line
module declaration provide symmetric real/fake request-local identity in a
checked-`u32` `Vec`/`SmallMap` arena. Each callback delivery is one nonempty
`Arc`-ID batch with a label-first representative; union preserves batches,
intersection retains the left representative, and label-materialized `except`
is symmetric. `siblings` scans every batch for ownership and delayed output
deduplicates labels. Fake `evaluation_graph_label` is `None`; fake nodes remain
printable and zero-edge for later activation. The module is disconnected: no
evaluator, graph, registry, DICE, or function activation changed, so Gate B and
all nine ordinary functions remain deferred. Worker and root independently ran
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2` (32 total: 10 provenance, 16
loading-query, 6 parser/registry); Sol-low final review returned `ACCEPT`.

## Two-Tier Work-Packet Contract

`.codex/skills/slug-agent-orchestration/SKILL.md` is the sole operating
contract. It owns routing, packet/reviewer templates, validation ownership, and
bounded logging. This plan owns only milestone state and acceptance evidence.

## Retained First Real Bazel Build Integration Gate

This was the first integrated implementation proof after the Stage 2 skeleton.
It is owned here because it crosses the Stage 1-7 boundaries; implementation
and detailed evidence remain in their stage owners. As of the 2026-07-22
direction reset it is a retained regression gate, not the current scheduling
gate. The Current Milestone Overlay controls new work.

The gate is:

1. `slug build` opens a real DICE transaction and evaluates a root
   `MODULE.bazel` and `BUILD.bazel` through starlark-rust.
2. A small package resolves a typed label, evaluates one custom rule, and
   produces a provider plus a shared-DAG depset and declared action.
3. The action becomes serialized REAPI `Command`, `Directory`, and `Action`
   protobufs; it uploads, executes through NativeLink, and materializes the
   declared output.
4. The matching Stage 1 fixture has a checked-in Bazel oracle, proves
   `reapi_actions=1` and `direct_local_actions=0`, and compares the declared
   output digest.
5. Once the daemon exists, an edit to the loaded `.bzl` reruns the affected
   computation in the same daemon for named DICE dependencies.

`simple-rule-action`, `shell-action-reapi`, and `load-invalidation` are the
initial fixture chain. A missing-module probe is separate: Bazel 9 creates an
empty `MODULE.bazel` with a warning, so V2 must not treat a missing module file
as a generic WORKSPACE-only failure.

Do not use this narrow build as proof that Stages 5-8 are structurally accepted.
Stage 9 records the concrete V1/Buck2 reuse that made each segment real, and the
analysis/query/aquery overlay now determines what may advance next.

This integration gate is not one implementation packet. Each packet names the
single numbered gate clause and owner stage it advances; detailed evidence
stays in that stage's plan. Cross-stage interface choices require pre-review.
After the contributing packets are accepted, a final integration packet runs
the complete fixture chain and receives Sol review before this gate is marked
complete. Passing substrate-only tests or one stage's isolated fixture cannot
substitute for that integration review.

### Gate status — 2026-07-16

All five clauses have contributing packets accepted:
1. `simple-rule-action` (clause 4, write action via REAPI) — pass
2. `shell-action-reapi` (clause 4, run_shell via REAPI) — pass
3. `bare-remote-executor-reapi` (clause 4, bare executor) — pass
4. `platform-exec-properties-reapi` (clause 4, platform properties) — pass
5. `load-invalidation` (clause 5, same-daemon DICE invalidation) — pass

The fixture chain (`simple-rule-action`, `shell-action-reapi`,
`load-invalidation`) passes end-to-end through the oracle harness with
NativeLink-backed REAPI execution and the `slug_server_v2` daemon. A final
integration review by Sol is required before the gate is marked complete.

## First Commit Scope

The first V2 implementation commit is documentation and ownership only:

- mark this plan as canonical;
- preserve the V1 roadmap as archive/reference;
- create the V2 subplans;
- update `AGENTS.md` so future workers read this plan first;
- avoid moving source code until the V1 archive tag/branch and V2 root policy
  are explicit.

Do not mix source movement, root reset, or implementation code into this commit.

## Validation

For documentation-only changes:

```bash
git diff --check -- AGENTS.md README.md thoughts/shared/plans
```

For the first real implementation slice, use the validation command in that
slice's subplan and record compact evidence in the owning V2 plan.

## Plan Execution

Plan-following sessions use
`.codex/skills/slug-agent-orchestration/SKILL.md`; Live Status above owns
scheduling.

## Reviewed Next M3 Direction: Build and Load Files (2026-07-23)

Status: Gate A and Gate B are accepted. B1.5 landed exact load diagnostics in
`4428df22`, recoverable DICE load-cycle handling in `237e7cac`, and exhaustive
non-graph CLI/retained-daemon evidence in `d25bc8c0`. B2 landed the reviewed
formatter/protocol boundary in `cb514747`; all 64 rows of the shared Bazel 9.2
fixture are now accepted under Slug.

M3 began with nine deferred ordinary loading-query functions. The reviewed
parent
packet is `WP-4-8-m3-build-load-files`, but it is deliberately split into two
commit gates: (A) `load-provenance-fake-target-substrate`, then (B) activation
of `buildfiles()` and `loadfiles()` only after A is accepted. One combined,
immutable Bazel 9.2 oracle fixture must be generated before either code gate.
The B1 core now activates only those two functions, leaving seven ordinary
functions deferred; `filter` stays
blocked on exact Java `Pattern` compatibility, and attribute/kind/label,
test, visibility, and executable functions remain blocked on their missing
metadata surfaces.

This is a loading-only, root-repository packet. It must model Bazel's full
transitive load graph and its `FakeLoadTarget` behavior, not a source-file
approximation: a fake target prints its `.bzl` label but belongs, for query
operations such as `siblings`, to the package that first consumed it. Uniquing
is label-based within each load-function invocation, while real targets, fake
targets, query-graph nodes, and set operations can meet through separate
paths. Request-local state must preserve enough `(printed label, consuming
package, real/fake)` provenance for the oracle-observed winner; it must not
collapse this to a request-global first-owner rule before both operand orders
and two-consumer cases are generated and reviewed.

Stage 4 owns a compact immutable manifest: each node has a canonical root
label/path, direct children, and transitive fingerprint in shared `Arc`
slices; `LoadedPackage` exposes its BUILD's direct roots/reachable closure
while retaining the corresponding `FrozenModule` lifetime separately. Stage
8 owns request-local fake-node/provenance state; it does not change global
`QueryLabel` identity. `LoadedPackage` semantic equality must include its
direct roots and transitive manifest identity/fingerprint, while frozen-module
pointer/lifetime storage remains excluded. The packet may use the existing DICE
`BzlParseKey`, `BzlModuleEvalKey`, load-label resolution, `PackageLoadKey`,
`PackageListing`, and workspace observations. Any new DICE key requires Sol
pre-review.

`buildfiles` must emit the selected package's active BUILD plus every
transitive load label and the active BUILD companion of every load-label
package; `loadfiles` emits only the transitive load labels. Companion basename
discovery is tracked but parse-independent and must not require a successful
`PackageLoad` for that package. The request-local projection retains only
operand-evaluation edges for FULL output: fake nodes never enter package
graphs, `:all`, or recursive patterns, and neither fake nor synthetic edges
may be added merely to render the result. A function-produced fake target is
zero-edge, so `deps(fake)` returns only itself.

Stop and replan on external-repository mapping, a requirement to silently omit
`.scl`, direct filesystem discovery, a global `QueryLabel` identity rewrite,
whole-workspace scanning, a new DICE key without review, or a claim that a
`.bzl` load cycle succeeds. A loaded label's containing-package BUILD may have
broken syntax or a broken `load()` and must still contribute its discovered
companion basename without a successful `PackageLoad` value; missing selected
loads and `.bzl` cycles are explicit failure-oracle cases.

Oracle evidence now ends at `e8014b25` (`test: isolate fake target set
algebra`): `query-build-load-files-provenance` has 64 Bazel 9.2 commands.
The base 58-row evidence is `8f6f02b3`; the correction adds a singleton
package loading only `//shared:two.bzl`. Update `051423-694832`, Terra clean
`051521-700085`, and root clean `051644-705470` passed; Sol-low returned final
`ACCEPT`. At that oracle checkpoint, nine functions remained deferred and
neither implementation gate had landed. Gate A subsequently landed in
`791e26b2`, B1 core activation landed in `ba457999`, and B2 completed Gate B
in `cb514747`. The oracle proves selected active
BUILD/transitive-load/active-companion `buildfiles`, loads-only `loadfiles`,
fallback/dual/diamond/multi-package/empty/idempotent/deps/failure cases, and
broken companion discovery without package loading.

The source basis is `BinaryOperatorExpression`'s `evalPlus`, `evalMinus`, and
`evalIntersect`, `QueryUtil`'s `TargetKeyExtractor`-keyed set,
`TargetKeyExtractor`, and `SiblingsFunction`: intersection retains the left
representative; equal printed-label `except` removes in both directions; and
union streams both provenance callback batches to `siblings`. The older
fake-left `except` real-`one.bzl` row remains nonempty only for unmatched
transitive `two.bzl`, not asymmetric equality. Stage 8 uses symmetric
label removal and explicit callback batches, never an asymmetric `Eq` or
operator rule.

Within one invocation `seenBzlLabels` label-deduplicates; across separately
evaluated functions one printed fake label can have different consuming
packages. Gate A retains `(printed label, consuming package, real/fake)`.
B1 applies the corrected label-keyed set/batch semantics through a crate-private
generic evaluator with associated `E::Set`: the loading environment owns one
request-local candidate arena and evaluates IDs in callback-preserving batches.
Its `seenPackages` key is the printed candidate package, while `PackageLoad`
and load visitation use the retained owner package; `.bzl` uniqueness and
final-output uniqueness are separate sets. Companion discovery receives the
workspace-root absolute package path and remains DICE-only.

Fake candidates have no dependencies, `siblings` scans every callback batch,
and FULL output selects the first label representative before projecting only
recorded real edges. The change activates exactly `buildfiles` and `loadfiles`,
removes unused public evaluator reexports, and adds no DICE key, global label
identity, filesystem seam, or change outside `slug_query_v2`.
Factored FULL uses `--output=graph --graph:factored`: fake nodes are zero-edge,
direct `buildfiles` omits the selected real BUILD unless another graph observer
materializes it, `deps(buildfiles(...))` includes result nodes, and no
synthetic projection edge is allowed.

Stage 4 half evidence landed in `b0670e33` (`feat: retain load provenance
manifests`), and Stage 8 completes Gate A in `791e26b2` (`feat: add fake target
provenance algebra`). B1 core landed in `ba457999`; B2 completed Gate B in
`cb514747`, and seven ordinary functions remain deferred. Public
`BzlLoadManifest`/`BzlModuleIdentity` retain canonical
label/normalized path, source-order label-first direct IDs, first-seen closure,
and `[u8; 32]` SHA-256 fingerprint. `LoadedPackage` equality now includes
direct roots/reachable closure/fingerprint: BUILD comment/format edits remain
equal, but leaf/direct/transitive edge create-delete-recreate changes then
restores the value. Aligned `FrozenBzlLifetimeEntry` retains every transitive
`FrozenModule` outside equality; identity/path are `Allocative`-accounted and
the opaque frozen module is skipped.

The public companion helper uses only `WorkspaceDirectoryKey`, primary before
fallback, regular or symlink entries, `None` for missing, explicit read errors,
and shared normalized-path validation; it is parse-independent and adds no
key/cache/lock/filesystem/package-load boundary. Worker/root loading tests had
27 integrations (the worker reported 26 by omitting pre-existing
`native_removed`); root also passed 11 `slug_analysis_v2` and 22
`slug_query_v2` integrations. Sol-low accepted corrections for symlinks,
shared validation, non-truncating alignment, edge lifecycle/BUILD
non-over-invalidation, and memory accounting.

For B1, the Terra-high worker and root independently passed
`CARGO_BUILD_JOBS=1 cargo test -p slug_query_v2`: 34 tests (10 unit, 18 loading,
6 registry/parser). Root also passed the serial downstream
`slug_commands_v2`, `slug_server_v2`, and `slug_cli_v2` suite: 11 command,
12 server, and 14 CLI tests, with zero doc tests. Sol-low final review returned
`ACCEPT`. Root removed one transient candidate-package `String` allocation
before the final tests.

`4428df22` gives missing loads Bazel's
`cannot load '<label>': no such file` diagnostic and appends
`compilation of module '<path>' failed` to malformed `.bzl` errors.
`237e7cac` adapts Buck2's lazy cycle-detector pattern into a request-scoped
DICE user detector for `BzlModuleEvalKey`. Its typed result retains both the
acyclic BUILD-to-cycle path and the cycle, renders Bazel's multi-node and
self-edge diagram, poisons the cycle computation so a repair invalidates it,
and proves same-DICE recovery plus a non-cycle diamond. Sol-low required the
blocking path-to-cycle result and returned `ACCEPT`.

`d25bc8c0` accepts B1.5: one CLI regression matches all 57 non-graph oracle
rows exactly, including exit/stdout/stderr behavior, and retained-daemon tests
cover leaf edits, direct/transitive edge switch-delete-recreate, and companion
BUILD priority without over-invalidating `loadfiles`. The full CLI suite passed
14 integration plus 1 unit test; the server suite passed 14 tests; Sol-low
returned `ACCEPT`.

`cb514747` accepts B2 and the complete 64-row fixture. `QueryOutput` retains a
request-local structural selected graph from the evaluation that produced the
labels; one-shot and retained-daemon presentation format that value without
reevaluation or a DICE read. The command/protocol surface supports Bazel's
default factored graph mode, explicit true/false and negated factoring, and
the fixed 512-node label limit. Factoring uses exact predecessor and successor
sets, quotient-edge deduplication, Bazel's lexicographical member-sequence
class comparator, reverse-postorder graph visitation, and minimal
always-quoted DOT labels. A dedicated regression distinguishes member-sequence
ordering from the incorrect joined-label ordering at a literal `\\n`
boundary.

Root passed `cargo fmt --all -- --check`, the four focused graph formatter
tests, the exact seven-row CLI graph matrix plus unfactored coverage, and the
serialized `slug_commands_v2`/`slug_query_v2`/`slug_server_v2`/`slug_cli_v2`
suite: 12 command, 14 query unit, 18 loading-query, 6 parser/registry, 15
server, 14 existing CLI integration, 2 graph integration, and 1 CLI unit
tests. Sol-low accepted the final comparator correction. Gate B is complete;
the next M3 packet must address one of the seven still-deferred ordinary query
functions rather than extending this formatter.

## Authoritative Next M3 Packet: Labels Metadata Foundation (2026-07-23)

`WP-4-8-m3-labels-metadata-foundation` is next. It supersedes tentative
`filter()`: Bazel `RegexFilterExpression` uses Java `Pattern.compile` and
`Matcher.find`, and no exact implementation/reusable dependency is known.
Finite oracle or `fancy-regex`/Rust `regex` agreement is not parity, so filter
remains blocked.

The packet has three serial commits: immutable Bazel oracle, Stage 4 metadata
substrate with no activation, then Stage 8 `labels` activation. Stage 4 replaces
`RuleDefinitionGen::has_deps` with ordered immutable, `Allocative` schema and
coerced-value structures. They retain exact attribute kind/name, query spelling
(`_implicit` becomes `$implicit`), mandatory/default/configurability state,
`Explicit | Default | Implicit` provenance, scalar/list labels, non-label
values, and unevaluated `select()` branches/default/concatenation. Canonical
labels are coerced during package construction; values are not flattened to
the aggregate dependency edge list. Output/output-list attributes retain their
exact label form and create Bazel-shaped generated targets owned by the
declaring rule before query activation. All semantic state participates in
`LoadedPackage` equality.

Stage 8 adds a separate compact attribute projection to `QueryNode` and then
activates only `labels`: rule prerequisites resolve through the existing
demand-loaded package graph, absent/non-label attributes and non-rules are
empty, and label uniqueness follows the query set. Authority is Bazel 9.2
`LabelsFunction`, `BlazeTargetAccessor#getPrerequisites`,
`AggregatingAttributeMapper#getReachableLabels`, and
`AbstractQueryTest#testLabelsOperator` at `8220c619…`. The oracle covers
scalar/list, explicit/default/implicit, missing/non-label, every configurable
branch and default, accepted concatenation, source and generated output labels,
cross-package resolution, order/dedup, compositions, and missing prerequisites.
The attribute projection and generated nodes participate in
`QueryNode`/`UnconfiguredPackageGraph` equality. Same-daemon edits cover each
semantic form while semantically equal/non-semantic formatting reuses values.

Own `slug_loading_v2/{attrs,package}.rs`, then query
`{expr,evaluator,graph}.rs`; add no key, scan, global identity, guessed
configuration, visibility, executable, or tests surface. The only generated
surface admitted is the exact output/output-list target representation required
by `labels`; its ownership, kind, and graph edges must be oracle-backed. Stop
before activation for any missing reachable-label form, output-target
ambiguity, coercion/provenance ambiguity, or query-time Starlark/filesystem
work. Reuse only Buck2 compact utility and traversal shapes; V1/Buck2 `labels`
is unimplemented and reference-only.

Oracle Gate 1 landed in `8dfae99c`: 31 generated Bazel 9.2 rows cover all
seven default public label-bearing attrs; experimental documented-false dormant
attrs are excluded. Select keys are false; valid dedup, two output producers,
generated kind/output→own-generator edges, and fail-fast missing/mandatory
errors are pinned. Worker `…/20260723-071512-784968-bazel` and root
`…/20260723-071641-791259-bazel` passed fixture-list, command-set, staged
diff/provenance/generated/credential-pattern checks; pytest unavailable; Sol
`ACCEPT`. This is Bazel evidence only: 29 rows are eventual Slug CLI gate and
two `label_kind` rows require focused `QueryNodeKind::GeneratedFile` assertions.

Stage 4 Gate A is accepted in `1b7c179c` (`feat: retain loading attribute
metadata`) with no `labels` activation: ordered immutable `Allocative`
seven-label-kind-plus-String schema/values retain defaults, configurability,
provenance/select structure, canonical generated identity/owner, outputs
outside ordinary deps, and semantic equality. Same-DICE tracker proves
`BzlModuleEval` → `PackageLoad` → consumer/observer; a preactivation guard
prevents leakage. Root passed fmt/diff, loading 35/query 39/analysis 11. Sol
corrected six initial blockers and rereviewed `ACCEPT`; root added nested
repeated-prefix ordering regression. Next is Stage 8: 29 CLI plus two
generated-kind assertions, never Slug 31/31 prematurely.

Prerequisite `f3e8ad48` (`feat: load config setting values`) is accepted:
the immutable labels fixture required native `config_setting` keys. The narrow
load-only representation retains sorted compact `values`, gives
`config_setting rule` correct zero edges, and has semantic reorder/change
tests; it performs no configuration evaluation and unsupported attrs fail
closed. Sol `ACCEPT`. Define/flag/constraint/common attrs and matching remain
deferred. Stage 8 `labels` now resumes unchanged at 29 CLI plus two
generated-kind assertions.

Stage 8 `8fec2696` activates exactly `labels(attr, expr)`; six ordinary
functions remain deferred. 29 non-label-kind CLI rows, including two complete
graph stdout rows, are exact; two Bazel-only label-kind rows remain formatter
constraints. QueryNode has compact immutable `Allocative` attrs separate from
deps; selectors retain all branches/default, exclude keys, and generated files
only output→own-generator edges. Package-load QueryError alone adds Bazel
`Evaluation of query`, preserving syntax/unrelated diagnostics one-shot/daemon.
Same-DICE/reuse and schema/value/select/default/output daemon transitions pass:
loading 37, query 42, CLI 21 (1 unit/17 CLI/3 graph), server 15, analysis 11,
fmt/diff. Sol corrected global suffix/fragment graph then selected-graph order;
final `ACCEPT` requires structural classification, exact graph rows,
generated-only ordering, ordinary factored/unfactored regression. M3 stays
open: never claim 31/31. This implementation reused the checked-in oracle and
needed no Bazel invocation; no agent or tool accessed `~/.bazelrc`. Future
Bazel commands may consume it through ordinary RC discovery without inspection.
Archive-status baseline failures (v1-archive/stale allowlists) are unrelated.

## Accepted M3 Packet: Executable Rule Capability (2026-07-23)

`WP-4-8-m3-executables-rule-capability` superseded the labels packet and is now
accepted. Oracle commit `c8e469f5`, Stage 4 substrate `c86fc656`, and Stage 8
activation `69565a29` complete the vertical. Bazel authority is
`ExecutablesFunction`, `BlazeTargetAccessor#isExecutableNonTestRule`, and
`TargetUtils#isExecutableNonTestRule` at `8220c619…`: the predicate is the
per-target `Rule.isExecutable()` / `$is_executable` capability *and* a retained
rule-class name not ending in `_test`. It is never inferred from a BUILD target
name or from a frozen implementation identity.

The generated Bazel 9.2 fixture has 40 commands: 32 semantic
`executables()`/composition/order/graph/diagnostic rows and eight Bazel-only
`label_kind` representation rows pinning five exported Starlark and three
supported native rule-class names. The latter are not Stage 8 formatter
acceptance. Terra update `085202-880190`, clean `085213-881221`, and root clean
`085303-889108` passed; Sol returned `ACCEPT`. The
`test=true, executable=false` row proves accepted syntax and `_test` exclusion,
not capability by itself; pinned `StarlarkRuleClassFunctions#createRule` and
`getTestBaseRule` establish that test still implies executable capability.
Ordinary Bazel RC discovery was allowed, but no agent or tool inspected or
persisted `~/.bazelrc` or BuildBuddy credentials.

Stage 4 retains immutable, `Allocative` `RuleCapability { rule_class:
CompactString, executable: bool }` in each Starlark rule instance and in all
semantic equality paths. `RuleDefinitionGen` must retain the exact exported
`.bzl` rule name through `StarlarkValue::export_as`, following the bounded
Buck2 rule shape and the existing V2 provider `OnceCell`/freeze pattern; the
exported rule name, not a target name, is the class. Gate A proves that export
validation requires test classes to end `_test` and non-test classes not to,
test implies executable, and an executable test is excluded. Supported native
`filegroup`, `alias`, and `config_setting` receive exact class names and
`executable=false`; alias never inherits; source/BUILD/generated nodes are
non-rules. Do not add `test_suite` while its global is absent. Native `genrule`
executable true/false is a separate
oracle/substrate gate: the current-loadable-graph boundary must be stated, and
the packet stops if full native-positive coverage is required rather than
inferring it.

Stage 8 evaluates its sole operand once, filters existing selected rules by
that projection, and adds no edges. It adds no DICE key, filesystem scan,
global classification, configured analysis, provider, regex, visibility, or
tests activation. Oracle and retained-daemon rows cover non-rules, executable
and non-executable rules, executable `_test` exclusion, native negatives,
composition/order/graph/diagnostics, false→true executable, false→true test,
export rename, target rename crossing `_test` without classification change,
formatting reuse, and delete/recreate. Root validation passed 45 query tests,
50 downstream CLI/commands/server tests, formatting, diff checks, and a clean
`slug_cli_v2` build; Sol-low returned final `ACCEPT`. M3 remains open with five
ordinary functions deferred. `WP-0-baseline-repair` subsequently passed; the
Live Status table now owns scheduling.
