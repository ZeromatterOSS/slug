# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-module-inspection-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private callerless inspection owner before selected evaluation
Evidence: accepted direct `local_path_override` route,
`HostRootModuleFileKey`, `HostRepositorySourceFileKey`, and the accepted
direct-local handoff design/cap correction in the owner plan.

Implement one private, callerless
`DirectLocalModuleInspectionKey { workspace, apparent_repo }` in
`app/slug_bzlmod_v2/src/source_preparation.rs`. It depends only on the accepted
`DirectLocalModuleFileKey` and, after a complete Present input, calls the
existing pure `inspect_nonroot_module_file`. It forwards Needs byte-for-byte,
uses complete-only equality/validity, and owns no event or bootstrap effect.

The complete value retains the full direct input plus `None` for Absent or the
existing `NonrootModuleFileInspection` for Present, so byte/path/route edits
cannot be hidden by an equal syntax projection. Build the inspection logical ID
from the retained requested Host logical path using the existing Host-module
path-display convention; do not invent a canonical label or final evaluator
identity. Errors remain typed as input compute, input semantic, or inspection
with logical path plus stable message. Includes may be discovered and retained
but are not acquired or evaluated.

Freeze tests for key identity/root rejection; complete-only equality; exact
bootstrap/materialization/path Need forwarding; Present route/bytes/path plus
logical ID/includes; complete Absent without inspection; invalid UTF-8 and
parse errors; typed real direct input errors; same-key A-to-B-to-A and
Present/edit/Absent/recreate; capture-enabled cold/warm no-data activations;
and a structural no-evaluator/no-legacy/no-fault scan.

Caps are 100 production, 350 tests, and 450 total. The reviewed isolated
estimate is 94 production/326 tests; the remaining 6/24 lines are formatting-
and compaction-only slack. Preserve every accepted Direct test unchanged and
use isolated inspection helpers plus an activation tracker filtered only to
`DirectLocalModuleInspectionKey`. `InputCompute` receives structural shape/
equality evidence only; no fault hook or runtime compute-error family is
authorized. Stop with
**REPLAN** on a selected or root-requested version, `NonrootModuleKey`,
`EvaluatedNonrootModule`, include acquisition, evaluator call, contextual
mapping, MVS/registry/discovery edge, public export/caller, direct filesystem
IO, new dependency, second file, cap excess, or event ownership. Run focused
owner tests, formatting, scope/cap/diff checks, then independent latest-diff
review. Do not run Bazel or change an oracle.
