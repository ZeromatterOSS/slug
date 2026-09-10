# Current Slug V2 Work Packet

Packet: WP-6-7A-selected-request-and-output-conflict-implementation-r1

Status: independently reviewed architecture ACCEPT; implementation selected.
The completed design milestone changed docs only; no runtime acceptance claimed.

## Observable result and authoritative contract

Complete the selected-toolchain request correction AND producer-owned output
integrity together: two preference variants retain parent configuration and full
owner identity, but different FileWrite contents at one materialized path reject
before RPC/materialization. Equivalent scalar FileWrites remain shareable, retain
both owners in analysis/aquery, and execute through one producer-chosen action.

Read Stage 6's "Selected-toolchain request correction before group activation"
and "Configured-action closure integrity contract" sections. Together they own
the full edge/selection, sharing, scope, publication, lifetime and proof contract.
The latter supersedes the old conflict reservation and old 900/1800/2700 caps.
No named/automatic group, computed-default, C++/Java or configured-aspect guard
is removed. Resume shared named/automatic group design after this implementation.

Predecessor 8f082eeed preserved an unaccepted candidate against ca284c2ae:
/tmp/slug-selected-request.yZYoSi/candidate.patch, SHA-256
d184284baa4dd28e37c248a4c1fb94b69e26b64a1c54f6c66ef61247b617bfe0.
Adjacent validation.txt records passed, failed and unrun gates. Recheck the hash
and applicability, then reuse the candidate and red core regression after review;
do not repeat the general group audit, archives or authenticated replay.
The red test selected_toolchain_request_conflicting_outputs_require_preexecution_rejection
proved distinct owners/identities and equal output roots/paths with A/B contents,
then failed only missing rejection (exit 101, 0.04s); no RPC/overwrite occurred.
Its correct successor assertion is producer failure, not a later accessor error.
Prior focused passes are evidence to reuse, not acceptance of the combined diff.
REAPI/full dependent gates and saved observed cancellation proof remain unrun.

## Source basis, decisions and non-decisions

Bazel 9.2 authority: local /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a, not checkout HEAD. Stage 6 records
Actions.canBeShared (including the both-unshareable ownerful-alias branch),
MapBasedActionGraph, Artifact ownerless identity, FileWrite factory/key, empty
FileWrite action exec properties, raw PlatformInfo inputs and prefix exemption.
Relevant OutputArtifactConflictTest themes: invalidation, new/overlapping roots,
unused actions, repeated/null builds and directory nesting. CqueryCommand:191
explicitly disables action-conflict checking; preserve configured query admission.
Reuse the selected-request ToolchainResolutionFunctionTest/ToolchainsForTargetsTest
anchors and retained Buck2 DICE worker tests named in Stage 6.

Exact named behavior is admitted scalar FileWrite equivalence and generic output
prefix conflicts. Structural output/configuration identity, first-error order and
display bytes are Slug-native. Exact Bazel checksum/path/ActionKey bytes remain
deferred. Inventory every registered action output, not only requested files.
Non-FileWrite exact-output sharing is explicitly unsupported, with its own
terminal classification, not a false Bazel-conflict claim; noncolliding typed
analysis remains admitted. Do not activate broader execution/formatter families.
No root suffix, whole-owner-key/ActionSpec/REAPI-digest equivalence, skipped owner,
command-side scan, global registry/cache, filesystem guess or new key family.

The existing DICE compute_build_action_closure constructs a private immutable
ValidatedActionClosure after all tracked children complete. BuildCommandEvaluation
retains it at every constructor, including explicit empty/loading-only paths.
Only successful validated closures reach build/run/aquery consumers. Cquery is
unchanged. Retain all owners and only duplicate action coordinates for execution
sharing; CLI/server switch to the core execution-view accessor, not own dedup.

ConfiguredActionOwnerContext additionally retains raw platform facts before its
existing property merge. PlatformSemanticFact gains a normalized optional shared
missing_toolchain_error message from the loaded native attribute (empty -> None);
explicit values remain admitted and merges preserve it. Raw and merged properties
remain distinct; structural equality/Allocative include all these source facts.
FileWrite semantic bytes add a conditional raw-property tag when raw differs
from already encoded merged properties, plus a nullable-message tag when unequal
to the pinned native default. Existing None-owner/default-platform bytes stay
stable. Source edits masked by target overrides and default/empty/custom message
edits still invalidate sharing and semantic identity. No diagnostic breadth added.
Stage 6 freezes the comparison inputs, constructor invariants and error order.

## Exact implementation/proof allowlist and gross caps

Production, paths relative to app/:

- slug_analysis_v2/src/key.rs
- slug_analysis_v2/src/dice.rs
- slug_analysis_v2/src/analysis_value.rs
- slug_analysis_v2/src/starlark_rule.rs
- slug_analysis_v2/src/result.rs
- slug_build_api_v2/src/analysis_value.rs
- slug_core_v2/src/runtime/file_write_identity.rs
- slug_core_v2/src/runtime/configured_action_closure.rs (new private owner)
- slug_core_v2/src/runtime/mod.rs (module declaration only)
- slug_core_v2/src/runtime/dice.rs (producer, carrier, errors and view plumbing)
- slug_cli_v2/src/commands/build.rs (execution-view call only)
- slug_server_v2/src/reapi.rs (execution-view call only)

Proof, paths relative to app/:

- slug_build_api_v2/tests/analysis_value.rs
- slug_analysis_v2/tests/configured_target.rs
- slug_analysis_v2/tests/starlark_rule.rs
- slug_core_v2/src/runtime/file_write_identity.rs (existing unit module)
- slug_core_v2/src/runtime/configured_action_closure.rs (bounded pure units)
- slug_core_v2/src/runtime/dice.rs (saved proofs, new include and helper wiring)
- slug_core_v2/src/runtime/tests/build_command_tests.rs (carrier updates/controls)
- slug_core_v2/src/runtime/tests/configured_action_conflicts_tests.rs (new)
- slug_reapi_v2/tests/reapi.rs
- slug_cli_v2/tests/cli.rs (one-shot and stable-daemon consumer proof)

Docs: this manifest, canonical Live Status and relevant Stage 6 owner status;
maintain /home/wgray/PROGRESS.md separately, at most 500 lines.
Combined gross additions, including saved patch and moved lines: <=1,400
production, <=3,000 proof, <=4,400 aggregate Rust. New validator <=600 physical
production lines; new core proof module <=1,200 lines; new helpers <=100 lines.
Do not inflate giant files with copied resolver/traversal/test scaffolds.
Analysis dice.rs (6,055 lines) remains resolution/delegation producer; core
dice.rs (12,475 lines) gets only its existing traversal/publication integration.
The new validator owns pure collision/equivalence policy, never DICE/transport.
Other large files receive only cohesive fields/projections/consumer plumbing.
No dependency, vendored, harness, oracle-tree or additional production file edits.

## Proof and validation

Use existing inline temporary BUILD/MODULE/Starlark fixtures with pinned-source
adaptation comments, no new copied oracle assets or external registry inputs.
First restore the saved candidate/red regression and reproduce the producer gap;
then complete sharing/conflict and selected-request behavior as one diff.
Required discriminators are enumerated in Stage 6: content/executable/platform/
raw-versus-merged properties; default/empty/custom missing-toolchain message and
unchanged-message cutoff; long/unicode writes; distinct roots/paths; unused
actions; exact/prefix error order; directory and RunfilesTree/MANIFEST cases;
unsupported non-FileWrite equivalence; A/B/A, separate/combined/overlapping roots,
warm repeated conflict, cquery nonregression, owner-complete aquery, one execution
representative and zero RPC/output mutation on failed closure.
Retain None/A/B full-key identity, edge-clearing/transition/nested preference,
known-platform aliases/optional fallback and observed cancellation/Need/error
proofs from the saved correction. Validate raw-platform equality cutoff even if
merged properties do not change. No partial parent or incomplete closure escapes.

Run one Cargo command at a time, initially timeout 60s for each command:

- cargo test -q -p slug_core_v2 --lib configured_action_conflicts
- cargo test -q -p slug_core_v2 --lib selected_toolchain_request
- cargo test -q -p slug_analysis_v2 --test starlark_rule selected_toolchain_
- cargo test -q -p slug_cli_v2 --test cli configured_action_conflicts
- cargo test -q -p slug_reapi_v2 --test reapi

At integration, full slug_build_api_v2 and slug_analysis_v2 package tests, core
library tests, and CLI aquery_text_keeps_root_order_across_one_shot_and_retained_daemon_restoration
plus the new configured_action_conflicts build/run cases, each bounded and recorded.
Compile direct dependents with cargo check -q -p slug_query_v2 -p slug_server_v2;
rebuild cargo build -q -p slug_cli_v2 BEFORE any changed CLI-binary proof.
Run cargo fmt --all, scripts/v2_archive_status.sh and git diff --check. The
archive checker has three recorded thoughts-path failures; compare, do not waive
new ones. Independently review the actual combined diff and recorded gate limits.
Do not claim prior unrun REAPI/cancellation/full dependent gates are accepted.

Investigate commands over one minute: compilation is distinguished from test
execution, never hidden by an automatic longer retry. Fifteen minutes is an
absolute maximum. No checkout-wide or authenticated workspace replay; local
fixture/loopback transport proof only. Inspect/clean owned test slugd before and
after daemon-sensitive validation, never unrelated processes.

## Lifecycle and stops

Memory: raw facts, optional shared messages and duplicate coordinates are DICE-retained
semantic state; sorted borrowed output rows/ancestor stacks are phase scratch.
No retained output index or new service/cache lifetime. Existing root keys,
tracked full child results and structural equality own invalidation/cutoff.
Independent roots never share a conflict registry. Complete errors retain the
existing observation/certificate terminal path; historical Host states remain
unavailable rather than guessed. Cancellation joins existing scoped work,
publishes no incomplete closure and releases scratch; reachable DICE versions,
command tokens and runtime shutdown own retained release. No lock across awaits.
Reuse Arc/Allocative/compact utilities under the utility skill; no donor import,
new optimization claim or unrelated path-epoch benchmark. No fallback introduced.

Missing retained source facts, broader-family key admission, altered root paths,
new semantic side store, unsafe output publication, cap overflow or a second
material correction is REPLAN. Never partially ship the saved candidate. Finish
the combined milestone, update owner status/log, commit and push to main.
