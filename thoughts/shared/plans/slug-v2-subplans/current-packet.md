# Current Slug V2 Work Packet

Packet: WP-5-7A-registration-error-identity-presentation-design-r1

Status: independent review ACCEPTS the source audit and this reserved-design scope.
Docs/source only; no implementation, runtime retry, acquisition or cap increase.

## Observable result and learned facts

Freeze one bounded implementation contract that preserves registration failure
identity across Loading -> Analysis -> Core while rendering a cause-first,
bounded Slug-native diagnostic. This is a cross-crate retained-error/public-kind
decision, not permission for a formatter-only truncation patch.

Source audit proves the captured selected_missing field is the expected
selected-module lookup miss before generated lookup, not its failure cause.
canonical_repository_route.rs:270-295 preserves it beside the actual generated
error, but derived Debug prints its retained graph first. Generated narrows the
unseen branch to Demand/DemandCompute/Loading/LoadingCompute/Duplicate; it does
not identify which branch or a missing source. The8192byte prefix cannot recover
the exact terminal cause. No Windows-specific bypass or semantic fix is selected.

registration_expansion.rs:172-179 Debug-renders the nested kind; analysis
dice.rs:3062-3071,3174-3186 converts it to AnalysisError::Message(String).
That conversion is eager even before the later Need precedence check. Core
BuildCommandError::Debug at3757-3762 calls to_string again. Merely changing the
probe's Debug to Display is too late: the large string already exists.

AnalysisError and BuildCommandError derive Eq/PartialEq. Analysis/resolution keys
use success-only equality, but Core's completed build terminal retains
Result<BuildCommandEvaluation,BuildCommandError> and complete_eq compares completed
values, including errors (core dice.rs:2474-2485,5247-5284; Bzlmod
source_preparation.rs:158-163). Shortened messages must not collapse distinct
failures into equal retained errors or replace structural identity with a hash.
The exact14GB allocation and measured timing costs remain unattributed.

## Required design decisions

- Choose the natural Loading-owned typed error handoff into Analysis and Core.
  Preserve full semantic distinctions/causal identity and existing observation,
  Need/Complete, first-error ordering, cutoff, validity and publication behavior.
  No new cache/key/lock, graph rebuild or command-side repair.
- Separate a borrowed/phase-scratch presentation from retained semantic identity.
  Prefer a typed registration error over an eagerly serialized graph string,
  but resolve concrete public enum/adapter ownership and all affected match sites.
  Do not retain a second graph or silently extend predecessor lifetimes; specify
  Arc ownership, Allocative accounting, terminal release and A/B/A equality.
- Freeze cause-first typed traversal: wrapper kind/repository/row then actual
  failing child. Successful predecessor graphs, pure plans, mappings and raw
  source bytes are not diagnostic causes and must not be traversed/formatted.
  Audit the Demand and Loading owner error shapes before choosing an adapter;
  an unseen branch stays unknown, never guessed from the winsdk repository name.
- Bound work and allocation before serialization, with UTF-8/escaping-aware
  output limits below the unchanged8192byte runtime stderr cap. No full
  format/to_string followed by truncation; no arbitrary Debug implementation
  that can allocate/scan retained graphs before a bounded writer can stop it.
  If a causal chain exceeds the declared bound, expose an explicit incomplete
  diagnostic, not a claimed complete leaf. Do not change existing global Debug/
  Display consumers without auditing their retained-error/string identity use.
- Name one implementation allowlist, caps, exact error families and direct
  dependents. Add only a borrowed adapter/type needed by this surface; no
  repository-wide diagnostic framework or migration layer.
- Define discriminating proof: same bounded display but different typed causes
  remain unequal; A/B/A exact restoration; no successful-state Debug traversal;
  multibyte/escaping/depth/large-message bounds; first-error/Need and publication
  invariants. Use a focused synthetic error fixture, not copied source trees.
  Required owner/direct-dependent builds remain timeout60 and serialized.
  A later authentic probe requires its own explicit reviewed run authorization.

## Read scope, compatibility and exclusions

Root owns design; one independent reserved-decision review is required.
Read docs/developers/dice.md and the authoring guide before designing. Start with
the Stage5 audit anchors: registration_expansion.rs, canonical_repository_route.rs,
canonical_repository_load_route.rs, generated_repository_definition.rs,
module_extension_repository_validation.rs, selected_repo_spec.rs and
selected_repo_spec/selected_extension_demand.rs; Analysis/Core dice.rs.
At most12 additional source/test files/2MiB excerpts, only to resolve the concrete
typed-error/diagnostic handoff and direct public-kind consumers.
Inspect complexity before selecting edits: module_extension_repository_validation
already2314lines; Analysis/Core dice.rs and selected_repo_spec.rs are oversized.
Prefer focused helper files plus small owner delegations, with a cohesion decision
rather than appending a generic framework to those modules.

Presentation is Slug-native, not Bazel diagnostic parity. Existing exact semantic
identity/integrity stays unchanged. No new Bazel/Buck2 oracle execution is needed
for design; DICE ownership guidance governs cutoff/retention, and local tests must
be named before implementation. No donor utility/runtime or fallback selected.
No CLI/Bazel/test/compiler/probe/daemon/replay, source-cache walk or acquisition.
Do not simplify the sentinel, change registrations/platforms, restore R2, or
increase time/memory/output limits. No graph omission in identity to reduce work.

Writable: this manifest, canonical Live Status, relevant Stage5/6 and routing
REPLAN only; <=120 added doc lines outside manifest; PROGRESS.md<=500.
Implementation/proof additions0; accepted probe and both preserved patches unchanged.
Validate diff, R2 hash/forward-apply, old draft hash and archive checker (known3).
Independent design/terminal review; commit/push the accepted implementation contract,
or explicit unresolved decision. A new owner, public boundary beyond this error
handoff or read-cap overflow requires REPLAN, not scope expansion.

## Evidence boundary

Actual probe /tmp/slug-sentinel-demand.PXDhxy/logs remains INCONCLUSIVE:
publication1/stderr overflow14.4498s/no abseil open; complete cleanup, no survivors.
Leaf cause is not present in the retained prefix. No runtime repeated in the audit.
Complete R2 /tmp/slug-conflict-r2.XZJWwv/candidate.patch SHA256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e
remains unaccepted; actual CLI conflicts, positive sharing and full gates stay open.
Never partially restore/ship it or inspect/print/copy ~/.bazelrc or secrets.
