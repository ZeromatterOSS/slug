# Current Slug V2 Work Packet

Packet: WP-7-52-m7a-artifact-file-symlinks-r1
Status: accepted

## Outcome and source basis

Ordinary requested Build executes and publishes regular-File artifact-target Symlink and
ExecutableSymlink actions. Remote consumers (including FilesToRun) receive target bytes;
requested aliases are physical links with complete durable backing. Nested aliases retain
terminal provenance and executable checks. Predecessor WP751 is accepted at 576818e94;
its contract/evidence remain in that commit's current-packet.md. Preserve WP750/751 behavior.

Demanded by authenticated target/wp714/actions.json SHA256
ffc50a44648f85b464988eb5dd6c6dd152269f342121a006839d3e8c7b4c0be8:
562 Symlink (412 generated Rustc rlib aliases, 94 external sysroot files, 54 external stdlib
rlib aliases, two redacted-header templates) and 17 ExecutableSymlink (16 configured sysroot
executables and one bootstrap wrapper). Each has one non-tree input/output. Pinned rules_rust
0.73 rust/private/toolchain.bzl:128-187, rust/private/utils.bzl:479-518,
rust/private/rustc.bzl:2334 and util/process_wrapper/private/bootstrap_process_wrapper.bzl:22-25
use target_file, including intermediate aliases across package boundaries. Two SolibSymlink
CppLink consumers are a separate native family; no Directory or unresolved-path demand is
established. Saved declarations do not prove full live-root execution. F3 stays closed.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
analysis/starlark/StarlarkActionFactory.java:294-325 validates artifact kinds;
analysis/actions/SymlinkAction.java:56-58,137-142,218-276 creates an artifact-backed link
without a spawned process, :295-339 checks regular-file owner-execute permission, :392-413
forwards target metadata. SymlinkActionTest.java:130-173 checks physical link behavior.
remote/RemoteActionFileSystem.java:509-516,596-605 resolves links before executable checks;
remote-only files count executable, whereas aliases to sources preserve source permissions.
remote/merkletree/MerkleTreeComputer.java:610-669,1032-1047 projects ordinary artifact aliases
as digest-backed executable FileNodes, not REAPI SymlinkNodes. UnresolvedSymlinkAction is a
different family; retained AbsolutePath is deprecated nonhermetic SymlinkAction behavior.
ActionOutputMetadataStore.java:219-225,590-600 establishes generated output permissions.
Reuse existing observed mode rows, native certificates/generation leases, typed local results,
source backing and confined publication. Read docs/developers/dice.md and Stage 9 retained
artifact/runfiles/observed-source utility dispositions; no donor semantic owner is imported.

## Contract

Exact named behavior: artifact input edge, file digest forwarding, executable validation,
remote regular-file projection and physical local alias behavior. Structural artifact/config
identity, native execution routing and durable backing/path spellings remain Slug-native.
Only SymlinkTarget::Artifact with File output and source File or generated File target is
admitted. Reject AbsolutePath, use_exec_root_for_source=true, Directory/RunfilesTree targets,
unresolved Symlink output kinds, SolibSymlink and unsupported platforms before effects.
Run, exact ActionKeys, broader aquery, source directories and Windows remain deferred.

Core adds one exact artifact edge to the existing validated forest. Preserve owner/config
lookup, canonical shared FileWrite representatives, cycle detection and unsupported-family
preflight. In requested mode, every reachable public-MANIFEST SymlinkTree also schedules its exact virtual tree
as a deferred completion root, including through alias chains; preserve declared edges and
avoid the tree-to-MANIFEST cycle. Complete these deferred roots before returning the forest,
keeping original selected-artifact producer ordinals. The legacy single-selected-action API
rejects a newly admitted topology if deferred completion requires actions after its selected
step; it must not misidentify the selected action or reorder true prerequisites. Legacy
ordinary Spawn roots retain their previous MANIFEST-file consumption behavior without these
additional publication completion roots; no alias output is selected for publication there. Ordinary
requested Build owns that topology. Source staging remains source-only. ActionChainStagingKey observes alias targets
through the same source owner and full epoch/certificate. A focused pure prepared-symlink
projection may expose output, exact target, require-executable and direct-source executable
status from the already-certified real-path Lstat row. Missing permissions fail closed;
no direct filesystem stat or new DICE key/semantic cache. This metadata confers no authority.

REAPI retains a typed local alias result at its plan ordinal: exact output, target content
digest, effective executable status and terminal backing provenance (certified source index,
verified remote CAS, or completed local bytes). Alias chains preserve that terminal provenance;
a Derived alias to a nonexecutable source does not become executable merely by being derived.
Use owner execute bit for source targets, remote-only generated files count executable even
when REAPI output mode is false, and local generated manifest files follow existing 0555
publication policy. Required executable validation occurs before completing the alias and
before its downstream consumers/publication. Generated target content must still exist and
verify in CAS at alias completion. Source/local equal bytes cannot repair missing generated
CAS, including through alias chains. No fabricated Execute/ActionResult/AC hit is introduced;
all-local source-alias plans need no backend connection.

Regular Spawn/executable/runfiles bindings consume alias digests and terminal provenance
through the same source/local/generated upload rules. Never create a REAPI SymlinkNode for
this resolved artifact family. Keep exact session identity/ordinal checks, native per-step
and final source validation, complete result reconciliation and honest requested-build digest
accounting. New result variants require direct CLI/server compilation and wrapper proof.
All new maps/results are request/session scratch; existing Arc/compact owners are reused.
No retained depset flattening, unbounded global state, async lock or execution outside Core.

Core owns physical alias effects. Selected aliases and aliases included in selected runfiles
trees expand publication to all transitive alias/backing prerequisites, even when not in
DefaultInfo. Use an iterative finite worklist so aliases to generated support manifests and
runfiles containing aliases reach a complete backing closure without repeated full replanning.
Preserve existing tree/MANIFEST coupling. Tool-only aliases require no local publication.
Host targets use observed requested paths; materialized sources use the existing verified
persistent source-backing store, not temporary repository paths. Derived links point at exact
configured output paths. Source-generation leases cover alias preparation/copying/execution
where needed, and immutable backing survives result/runtime/process exit.

Introduce an internal typed alias stage (no public arbitrary symlink writer). Use descriptor-
confined no-follow parents, exact link-text seal checks, stale destination detection and
same-parent replacement. Symlink leaves may be replaced/unlinked without following targets;
symlink ancestors and special files remain rejected. Preserve safe replacement between regular
File outputs and alias leaves while continuing to reject symlink nodes from remote outputs.
Never chmod, read through or recursively clean a link target. Stage/seal all backing, links
and outputs, preflight the full batch, install durable backing then ordinary generated backing
then aliases in prerequisite order then runfiles trees under existing final revision validation.
Individual output swaps do not provide batch atomicity: an alias to a public MANIFEST may
briefly precede its tree replacement, but all terminal bytes are already installed and success
requires the complete batch. Preserve explicit partial-publication failure if a later swap
fails; never report such a batch as complete. Release retired owners outside the lock.
No new GC or power-loss durability guarantee.

## Scope and proof

Core: action_prerequisites.rs and focused alias helper/tests; action_chain_staging.rs and
focused symlink projection/reused observed-mode helper; action_chain_execution.rs lease gate;
action_output_staging.rs, plan.rs/focused backing-closure child, Linux staging/alias child,
configured_output.rs and runtime exports. REAPI: action_chain.rs, result.rs, binding.rs,
focused alias child, output_staging.rs, requested_build.rs and directly affected tests.
CLI: focused wrapper proof reusing an authored fixture, no new command API. Root owns this
manifest, canonical and Stage7/bootstrap notes. No analysis action expansion is required.
Expected 600-1000 production and 600-1100 proof lines; keep new behavior in focused children,
review responsibility boundaries if exceeded. No new fixture corpus or fresh Bazel process.

Discriminators: Host/materialized source and generated regular File aliases; source symlink
requested-path preservation; two-link chains/shared producers and exact configuration/owner
rejection; source owner-x versus group/other-x, chmod-only failure/restoration; generated
remote modefalse acceptance; local manifest aliases including an alias-only request for a
public MANIFEST that schedules/publishes its coupled tree; missing/corrupt generated CAS with equal
source/local bytes; consumer-only FilesToRun execution with no incidental tool publication;
requested physical aliases + hidden backing and source durability after CLI/runtime exit;
A/warm-A/B/A content and mode changes, regular-file/alias replacement, malicious leaf/ancestor
links, seal/stale destination failure and cleanup that never touches link targets. Preserve
WP750/751 publication and source-only controls. Reuse existing retry/final-validation and
partial-publication proof where unchanged. Full production closure, M7A/M8 remain unproved.

Independent design and final review. Pinned nightly-2025-09-14 offline no-run/compile separately
from exact-selector tests; 60s preparation cap per operation. Runtime tests expected a few
seconds; longer tests infrequent and >30s needs strict necessity. Shared-target Cargo and native
ancestor-observing tests serial. Root coordinates all compilation/execution; workers author
focused tests. Rebuild CLI before public wrapper smoke. Supervised local backend only for
named real-transfer proofs with cleanup receipts; no broad suite. Format/diff/plan/archive
checks and target/wp752 receipts, then commit and authorized main push after acceptance.
REPLAN only for a new ownership/compatibility prerequisite that cannot satisfy this contract;
routine corrections remain within the packet. WP746 baseline diagnostic defect stays open.

Independent design review: ACCEPT. Regular-File target scope, terminal provenance,
source permission authority, deferred MANIFEST/tree completion roots and publication
limits are frozen for implementation.

## Acceptance receipt

Implemented Core-owned exact alias edges, certified source-mode projection, generation
leases, deferred MANIFEST/tree completion and finite publication backing expansion. REAPI
keeps typed terminal provenance, verifies generated CAS before alias completion and exposes
resolved aliases as regular consumer inputs. Confined physical links and durable backing
survive runtime/CLI exit. No new retained state, DICE key or dependency was added.

Independent final source review ACCEPT, including two corrections: private alias creation
claims cleanup only after checking symlink identity/text (a substituted directory survives),
and stable publication sorting preserves ordinary selected-group order while sorting aliases
by prerequisite ordinal. The inherited strict-subset control first reproduced [1,2,3] versus
required [2,3,1], then passed after correction. Approximately 732 added production and 1282
proof lines; the proof estimate overrun was reviewed as distinct planner/filesystem,
provenance/CAS/backend and shared-fixture CLI responsibilities, without expanding scope.

Pinned nightly-2025-09-14 offline validation, all exit 0 unless explicitly recorded as the
pre-correction discriminator; receipts under target/wp752:

- Core no-run 24.952s, correction rebuild 5.844s; REAPI no-run 22.016s and final 2.433s;
  CLI/server build 13.627s; requested_build integration no-run 8.263s. Preparation was serial.
- 18 exact Core tests: core-focused-r2 15/15 in 1.010s and core-protected-r1 3/3 in 0.441s.
  New alias planner/filesystem cases plus exact producers, selected subsets/order, shared
  FileWrite owners, runfiles, source-only boundaries, batch preflight and cache confinement.
- 10 exact REAPI portable tests, reapi-portable-r2 10/10 in 1.795s: terminal provenance,
  source owner-x changes, generated modefalse, exact results, all-local completion and
  protected FilesToRun/local-manifest bindings.
- Six supervised backend proofs, one exact ignored selector each: new alias consumers
  3.224s; missing/corrupt alias CAS 1.188s; final physical alias/manifest publication 1.180s;
  rebuilt CLI process-exit 0.657s; inherited binary runfiles publication 1.527s; inherited
  FilesToRun consumers 7.931s. The last is an infrequent checkpoint regression. All backend
  processes terminated/reaped and temporary roots removed. Consumer/CAS evidence predates
  only the publication-order correction, which cannot affect execution-only behavior.
- Rustfmt, diff, plan status and archive checks pass. Source hashes cover 33 changed Rust
  files; target/wp752/source-hashes.json SHA256
  dc27cf9aebd6aeab9b27fc68f3eede43d1522f14e3e09f00cba53ee34fa1888d.
  artifact-hashes.json also identifies the validated binaries. Packet wall time spans a
  user pause and was not measured reliably; individual compile/test durations are above.

Observable gate advanced: ordinary requested Build now executes the saved production
regular-File alias family and publishes complete durable physical backing. Authentic full
production execution, M7A/M8, SolibSymlink and Cargo's _runfiles_map remain open. WP746's
baseline diagnostic defect remains recorded; F3 stays closed. Commit/push this checkpoint,
then select the next production-demanded prerequisite from the retained inventory.
