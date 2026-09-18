# Current Slug V2 Work Packet

Packet: WP-7-33-m7a-source-spawn-reapi-plan-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and demand

Construct an opaque, closure-owned typed Spawn REAPI plan containing canonical
Command/Action bytes and the already-owned source/parameter Merkle tree. This is
the missing serialization prerequisite for the compiler/build-script Spawn row in
bootstrap-readiness.md; WP-7-31 already supplies complete declared source staging.
The plan must preserve one configured action's arguments, fixed environment,
selected execution context, declared regular outputs and input digests together.
No Execute, AC request, build publication or CLI activation in this packet:
execution-representative selection, completed verified staging and full-frontier
revalidation remain required before execution.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
remote/RemoteExecutionService.java buildCommand/buildRemoteAction; remote/util/Utils.java
buildAction; analysis/platform/PlatformUtils.java getPlatformProto. Reuse WP-7-23
CommandLines.expand and WP-7-31 Merkle/path/content evidence. Exact UTF-8 argv,
fixed environment, sorted normal output paths, selected/default property semantics
and SHA-256 of canonical protocol bytes for Slug's actual graph. Slug-native output
identity and existing REAPI encoding profile (both legacy file outputs and modern
output_paths, present empty platform, empty salt); no Bazel Action digest/ActionKey
identity claim. No Java execution, donor implementation or new oracle workspace.

## Owners and invariants

Core PreparedSourceActionInputs exposes a borrow of the exact ConfiguredAction
already selected from its retained ValidatedActionClosure. Its constructor, DICE
key/dependencies, full epoch, certificate, equality and rejection remain unchanged.
Do not generalize the FileWrite-only ConfiguredActionView or accept a raw Spawn as
public plan input. Core owns selection; configured action context owns execution
platform and combined target/group properties. No reconstructed owner/platform.

REAPI SourceSpawnReapiPlan is operation-owned serialization scratch: existing
SourceInputReapiPlan plus Command and Action identity. SourceInputReapiPlan retains
replacement argv from its one ExpandedSpawnCommandLine and moves it into Command;
the tree owns virtual bytes from that same expansion. Drop expansion scratch after
tree construction so parameter-file bytes are not retained twice. No second semantic action, cache, interner, lock or retained DICE
field. Arc retains the producer until the plan drops; ordinary Vec/BTreeMap wire
scratch is appropriate, without a performance claim.

Require a selected execution platform and its raw and effective platform facts.
When raw platform properties are empty, apply request-local remote defaults then
overlay effective configured target/group properties. When raw properties exist,
nonempty effective configured properties are authoritative and defaults are
excluded. If the effective map is empty, fall back to raw properties (which can
contain only other-group keys filtered by the effective owner); use defaults if
both maps are empty, per PlatformUtils. Defaults affect REAPI identity only, not configured DICE identity.
Read fixed environment from retained Spawn; inherited names fail closed pending
immutable client-environment resolution. Nonempty execution requirements fail
closed pending explicit timeout/cache/remote-policy ownership. Reject wrapper-level
legacy exec_properties, tree/symlink outputs, empty outputs and any source/virtual
input-output equality or prefix collision. Source staging's existing derived/tree,
runfiles, undeclared executable and conditional-param boundaries remain intact.
Arguments stay ordered and parameter replacement is never flattened back to inline
arguments. Output files are sorted; environment/properties are sorted by existing
wire owners. Action timeout absent, do_not_cache=false and empty salt are admitted
only with empty requirements. Remote transport timeout/endpoints never enter this
plan API or action digest. Mnemonic/progress text remains diagnostic, outside bytes.

## Scope and proof

Allowlist: Core runtime/source_staging.rs borrow accessor; REAPI source_staging.rs
expansion ownership; new source_spawn.rs/tests.rs and lib.rs reexport; existing
command.rs only if a bounded shared helper is necessary. Existing executor behavior
and public raw typed-action rejection remain unchanged. Plans: canonical, manifest,
Stage 7. No deps, copied fixtures, generated outputs, scheduler or materializer.
Expected growth roughly200 production/300 focused-test lines; review responsibility
boundaries if significantly exceeded.

Reuse the tiny authored Core source-staging workspace via its existing shared
fixture. Public native preparation -> new plan proof inspects encoded Command and
Action, exact environment, ordered replacement argv/param bytes, source-root digest,
sorted multiple outputs and selected/raw/combined platform semantics. Same-runtime
source/content/argument/environment/platform A/B/A changes discriminate Action
identity while diagnostic-only changes preserve it. Reject inherited environment,
nonempty requirements and source/output collisions through native preparation.
The Starlark producer lacks declare_directory, so tree/symlink/empty/malformed
output guards use the same private output projection directly. Test default
properties merging with target properties on an empty raw platform and exclusion
on a nonempty raw platform; also prove the raw-nonempty/effective-empty fallback
and the both-empty default case. Preserve ordinary FileWrite canonical bytes and raw
typed-action rejection selectors. No backend needed for pure serialization.

Independent design/final review. Read DICE ownership and utility guidance; no new
retained representation or extraction. Pinned nightly compile-only --no-run JSON
preparations capped60s; public Core/REAPI direct consumers compiled by CLI check.
Exact executable preflight; focused runtime tests expected under a few seconds.
Any >~30s test requires strict necessity; none planned. Rust format, diff, archive
and plan checks. Receipts target/wp733. Reuse unaffected earlier wire proofs.

REPLAN if the context is not producer-complete, command projection needs an
unmodeled result-affecting field, or acceptance requires executing outside the
validated closure. Resolve actual requirements before widening their admission.
Generated/tree inputs, inherited environment and execution policy are still needed
for the production closure; this checkpoint cannot close M7A or M8.

Predecessor WP-7-32 accepted/pushed bf0ac3f8e: local CAS write verification,
strict mismatch absence/recovery and protected FileWrite execution/AC hit. Four
focused gates; longest test2.180s. In-flight presence still cannot authorize Execute.


## Acceptance evidence

Baseline bf0ac3f8ecfb9a746c82add2f47209d74923cffe;
review/wp733-source-spawn-plan. Independent design ACCEPT, including the later
source-backed raw-platform fallback correction. Independent final review ACCEPT. No public
raw-action execution was enabled and no DICE state or dependency changed.

Nine focused gates pass. Pinned direct nightly-2025-09-14 toolchain, no-run Cargo
JSON exact executable selection and preflight before ordinary tests. Raw receipts
are under target/wp733. No backend, Slug daemon, Bazel run or build action required.

- REAPI library final selected5/pass5, exit0, elapsed0.545s:
  source_spawn::tests::{native_spawn_plan_preserves_fields_and_content_identity,
  native_spawn_plan_rejects_unmodeled_policy_and_output_collisions,
  output_projection_rejects_non_file_and_malformed_paths};
  source_staging::tests::source_merkle_nodes_are_executable_and_paths_are_structural;
  executor::tests::file_write_plan_owns_canonical_nul_safe_reapi_objects.
- Native public preparation proves the retained configured owner, fixed environment,
  literal/forced-param/literal argv ordering and exact virtual bytes, sorted two-file
  outputs, canonical decoded Command/Action, SHA-256 domains, absent semantic
  timeout and all four raw/effective/default platform cases. Same-runtime source,
  virtual argument, literal argument, environment, output and target-property A/B/A
  change and restore Action identity. Source-only edits preserve Command identity;
  mnemonic/progress changes preserve all wire identity. Defaults affect the digest
  when admitted and are ignored with a nonempty raw+effective platform.
- Native negatives reject inherited environment, nonempty requirements, source/output
  equality and both prefix directions, plus virtual-param/output collision. The
  private output helper rejects directory, symlink, runfiles-tree, empty and malformed
  output declarations. Public declare_directory is not implemented in the Starlark
  producer, so no native tree declaration admission or proof is claimed.
- REAPI integration exact selected4/pass4, exit0, elapsed0.002s:
  typed_actions_reject_command_input_tree_and_execution_projection,
  typed_action_execution_rejects_before_transport,
  forced_param_files_merge_exact_bytes_and_merkle_topology,
  forced_param_files_reject_input_collisions_atomically. Their implementation and
  invariants were unaffected by the later private platform/argv-ownership refinements;
  retained passing evidence. Previous accepted NativeLink transport proofs unchanged.
- Nine separate preparations total46.313s, longest7.782s: six library no-run
  operations (initial import correction exit101, then all exit0), one integration
  no-run exit0 and two CLI checks exit0. Final library preparation3.068s; final
  cargo check -p slug_cli_v2 7.122s covers the public Core/REAPI changes.
- First test batch failed only because the negative fixture called unavailable
  Starlark declare_directory; three other gates passed. Output-kind/path validation
  was factored into the private helper used by the public plan and directly tested,
  leaving declaration breadth unchanged. The platform fallback was corrected from
  pinned source before final review and has a native discriminator. Later literal
  argv and ownership refinements reran affected native proof. Longest test batch
  (including failures)0.607s; total runtime across all batches2.685s.
- Changed Rust formatting, diff, archive and plan checks pass. Production growth is
  the142-line projection plus bounded accessors; the415-line test module comprises
  native fixture setup and three related projection proofs, reusing the existing
  authored workspace with no copied ruleset or new dependency. Continuous packet
  and review elapsed time not recorded; no throughput or benchmark claim.

Gate advanced: closure-selected typed Spawn -> paired source/parameter tree and
Command/Action serialization using its own configured context. Execution policy,
inherited environment, generated/tree inputs, representative admission, completed
staging and full-frontier validation remain required. M7A partial; M8 unproved.
