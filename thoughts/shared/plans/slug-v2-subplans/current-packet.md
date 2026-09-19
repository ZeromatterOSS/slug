# Current Slug V2 Work Packet

Packet: WP-7-50-m7a-runfiles-build-publication-r1
Status: accepted

## Outcome and basis

Complete ordinary requested binary Build for the existing four retained runfiles support
families on Linux GNU, including hidden generated backing and materialized repository
sources. WP749 (307048562) supplies selected native generation leases, WP748 supplies
observed manifest encoders, and WP747 supplies the artifact-owned layout. Retain exact
manifest algorithms and graph identities. Persistent source path projection, storage,
publication and local execution routing are explicitly Slug-native. Exact ActionKeys,
Run activation, FilesToRun-as-Spawn-input, arbitrary Symlink outputs, nested runfiles trees,
Windows and unrelated Args action families remain unsupported/deferred. Legacy single-selected-
action execution/publication continues to reject selecting a runfiles support family; its
ordinary remote action behavior is unchanged. Requested public-MANIFEST roots additionally
seed their exact matching virtual tree as a completion root without changing declared edges.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a sources:
RunfilesSupport.java:467-543 (four artifacts); RunfilesTreeAction.java:107-138 (virtual
metadata completion, not remote Directory); SymlinkTreeHelper.java:87-95,167-237 (absolute
links, public MANIFEST and writable directory structure); SourceManifestAction.java:249-424
(manifest bytes and diagnostics); RepoMappingManifestAction.java:186-332 (mapping bytes).
RemoteOutputChecker.java:165-211 and RemoteImportantOutputHandler.java:135-148 establish
that runfiles backing must actually be available. Reuse pinned-source regression evidence
from WP747/748 and native lifetime evidence from WP749. No fresh Bazel process is required
unless integration exposes an uncovered compatibility question. Read docs/developers/dice.md.

## Contract

Preserve the single exact-owner prerequisite forest and native attempt. Admit the four
RunfilesSupportActionSpec variants through their retained declared-input edges. Reject
unsupported artifacts before effects, preserving exact owner/configuration lookup and
cycle/conflict checks. Observe every raw runfiles source constituent (including obscured
entries) in the existing action-chain DICE staging key. Retain workspace as a semantic
input for pure path projection. Share existing manifest/path helpers. No filesystem owner,
source bytes or execution results enter DICE; new action/layout/result metadata is request
scratch. Reuse Arc, SmallMap/SmallSet, Allocative and Stage 9 observed-source ownership rows.

Represent results in plan ordinal order with typed remote, local manifest, physical
symlink-tree and virtual runfiles-tree outcomes. A virtual result never creates an Execute,
REAPI ActionResult or Directory output. Remote evidence counts only remote actions. Generated
file consumers may bind actual local manifest bytes/digests; generic runfiles-tree consumers
remain rejected. Whole-plan preflight precedes remote work. Existing per-step and final
full-source certificate validation remain mandatory. Completion retains exact selected native
generation leases for materialized sources for the duration of copying/execution.

Materialized sources used as link targets project to reserved
bazel-out/.slug-runfiles-sources/v1/<sha256>-<size>-<mode> paths, outside configured roots.
Use read-only mode 0444 plus the source's observed execute bits (0111); reject unavailable
permissions. The exact namespaced real-path Lstat row in the source observation epoch owns
mode. Copy via observed-source authority and verify SHA-256/length before sealing. Existing
entries are not trusted by name: validate or replace through a confined staged file. The
same pure projection supplies source manifest bytes and physical link targets. Host sources
keep their observed requested paths. Durable backing is an output cache, never semantic
input or freshness authority. Retain installed backing until explicit output cleanup;
there is no automatic GC in this packet. Harmless immutable orphan blobs after an aborted
partial publication are allowed. Paths survive result/runtime/process exit. No fsync/power
loss guarantee beyond existing output publication; no permanent TempDir targets.

Core owns all local filesystem effects. Reuse descriptor-confined no-follow parents and
staged sibling swaps. Add an internal typed tree writer, never a public arbitrary symlink
capability. Create only validated layout links/empty files and the public MANIFEST link;
validate symlink text without opening or chmodding targets. New tree directories use 0755
and empty files 0444 (Slug-native mode normalization); create the mandatory main workspace
subdirectory. Overlay physical MANIFEST after authored entries as the pinned helper does,
replacing any authored MANIFEST leaf while preserving it in source manifest bytes. Whole-plan
preflight rejects physical leaf/descendant conflicts preserved by late root/empty overlays,
including MANIFEST/child; it never follows a leaf symlink or silently drops a child.
Bind tree and MANIFEST to the
same exact retained support and publish them as one directory unit. Account for MANIFEST's
separate producer in accepted publication metadata. Expand selection to complete raw derived
backing plus input/repository manifests using exact producer bindings, including artifacts
absent from DefaultInfo. No undeclared files enter the logical runfiles tree.

Stage/seal all bytes and links before final source validation; preflight the entire batch,
install durable/derived backing before visible trees, then publish under existing native
revision validation. Preserve explicit partial-publication errors and release retired/private
stages outside the revision lock. Rejection/retry drops provisional stage owners; accepted
backing has persistent output ownership. Preserve existing output modes outside this explicit
source-backing policy. No mutex across DICE compute or asynchronous work.

## Scope and evidence

Core: action_prerequisites.rs, action_chain_staging.rs with focused runfiles child,
action_chain_execution.rs, runfiles_manifest paths visibility/reuse, action_output_staging.rs,
plan.rs and Linux children, configured_output.rs narrow wiring, runtime exports. REAPI:
action_chain.rs and focused children, output_staging.rs, requested_build.rs, public exports
and directly affected tests. Add focused test children reusing existing hermetic native
Workspace and local REAPI backend scaffolding. CLI/daemon only for direct wrapper coverage
and necessary result API adaptation. Root owns this manifest, canonical and Stage 7/bootstrap
owner notes. Avoid central-file growth: new source backing/tree/result logic belongs in
focused children. Expected 900-1500 production and 700-1200 proof lines; review actual
responsibility boundaries when exceeded, not permission based on a line count.

Discriminators: mixed local/remote plan ordinals and remote evidence; exact support pairing;
hidden generated backing and cooutput selection; public MANIFEST coupling including a MANIFEST-only requested root and authored root-name
overlay and rejected late prefix conflicts; executable and
non-executable materialized source backing; Host requested symlink spelling; obscured source
certification; source/generated A/B/A and missing repair; result/runtime shutdown lifetime;
corrupt/missing backing, pre-publication mutation, stale destination, confined link/cleanup
without touching outside targets. Reuse existing retry/final-validation/partial-publication
controls where unchanged. Public requested Build integration must produce a usable binary
runfiles tree, not merely a metadata preparation result. Keep M7A partial/M8 unproved until
their full gates pass. WP746 baseline diagnostic defect remains separately open.

Independent design and final review. Pinned nightly-2025-09-14, offline no-run compilation
separate from execution, preparation cap 60s per operation. Shared-target Cargo and native
tests observing shared ancestor paths serial. Exact-selector preflight; focused tests expected
a few seconds, >30s requires strict necessity. Compile named direct dependents, rebuild CLI
before wrapper smoke; no broad suites without milestone necessity. Format/diff/plan/archive
checks, receipts target/wp750, commit and authorized main push at acceptance. Preserve
unfinished coherent work on a review branch if a genuine design prerequisite interrupts
completion. REPLAN only if the exact support owner, durable lifetime or atomic tree unit
cannot be implemented without weakening native validation or broadening compatibility.

Independent design review: ACCEPT. Typed result ownership, persistent source backing,
physical MANIFEST coupling and explicit mode/conflict boundaries are frozen for implementation.

Reviewed design amendment: ActionChainStagingKey stores Warn obscuring diagnostics in its
existing evaluation EventBatch once per SourceSymlinkManifest, after complete successful
preparation. Need/errors use empty batches; no warning text enters prepared semantic equality.
Existing native event selection owns retry, warm suppression and restoration. Message shape
follows pinned Runfiles.java:289-307 with Slug-native artifact exec-path spelling. Independent
design review ACCEPT; exact warning A/warm-A/remove/restore is required.

## Acceptance evidence

The implementation now completes ordinary binary Build with a plan-indexed typed result
table and no fabricated remote outcomes. Native preparation certifies all raw runfiles
sources and rejects mismatched support objects/outputs and physical topology conflicts.
Core stages verified persistent materialized sources, exact generated backing subsets and
one physical tree/MANIFEST unit, with all backing installed before trees after full native
validation. Shared-source mode/content identities preserve executable and non-executable
backing independently. Existing arbitrary symlink rejection and legacy selected-family
admission remain unchanged. Warnings use existing DICE event reconciliation, not transport
or result-local stderr. Pure projections use the borrowed action on the ordinary path to
avoid repeated forest planning.

Fifty distinct portable checks pass: 34 Core (six new confined staging/backing checks,
three new native binary/overlay/warning checks including forged support/output negatives,
and existing planner, manifest generation, requested output, retry and partial-publication
controls), plus 16 REAPI result/binding/schema/evidence/preflight checks. Portable batches
completed in 0.284-4.661s. New real-backend selectors prove binary A/warm-A/B/restored-A with
exact two-action remote evidence and hidden backing, durable source modes/targets after
runtime shutdown (1.519s), and an initially unpublished MANIFEST-only root with complete
backing (0.626s). The rebuilt production CLI's one-shot Build exits, then its binary still
runs from the published runfiles tree (0.654s). All three supervised backends terminate and
remove their temporary roots; no daemon or fresh Bazel process was required. These tests
prove the admitted family, not full production closure or Run.

Pinned offline Core/REAPI compilation and CLI/server direct-dependent build pass; no-run
preparation and runtime were separate. The largest preparation was 46.061s, below its 60s
operation cap. Initial Core compilation found Arc<str>-to-io::Error conversion errors,
corrected without changing the contract. Three fixture corrections preserved the gates:
raw-file overlap selects Warn while explicit Starlark symlinks select Error; no-output
checks must permit pre-existing configured-output infrastructure; and a source executable
needs the established tools/tool path rather than a bare PATH-searched name. Failed
receipts remain alongside passing corrections in target/wp750. Formatting, diff, plan and
archive checks pass; exact selectors and accepted Rust source hashes are recorded there.
No performance speedup is claimed. Independent final review: ACCEPT, verifying ownership, complete freshness, durable backing,
coupled publication, honest remote evidence and the full named validation.

This closes ordinary requested binary runfiles execution/publication. M7A remains partial
and M8 unproved; continue the finite production closure's demanded Args/FilesToRun/other
family gates. Exact support ActionKeys, aquery projection, arbitrary Symlink outputs,
Run/Windows and WP746's baseline diagnostic defect remain open. Durable backing is retained
until explicit output cleanup; automatic GC and power-loss persistence are not introduced.
