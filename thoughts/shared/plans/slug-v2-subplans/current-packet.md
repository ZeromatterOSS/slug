# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-evaluation-owner`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private one-file preparation-consuming DICE/event owner
Evidence: accepted support-gated direct-local preparation in `f2b626f2`;
accepted trusted nonregistry evaluator adapter in `c683c239`; pinned Bazel 9.2
empty-key identity and fresh-only nonregistry print behavior; and existing DICE
activation/event regressions. Add no oracle or fixture.

Edit exactly `app/slug_bzlmod_v2/src/source_preparation.rs`. The formatted net
addition may not exceed **230 production lines, 720 test lines, or 950 total
lines**. Add only private, callerless owners:

- `DirectLocalModuleEvaluationKey(NormalizedAbsolutePath, ApparentRepoName)`;
- `DirectLocalModuleEvaluation::{Supported, Unsupported}`;
- one private evaluated route-plus-module value; and
- typed preparation-compute, preparation, root-`Absent`, and evaluator error
  carriers.

The key value remains `SourcePreparationOutcome<Arc<Result<...>>>`. Key identity
is workspace plus nonroot apparent repository. Use complete-only equality and
validity: every Need is invalid and self-unequal. Events and activation data
never enter semantic values or equality.

Consume exactly `DirectLocalModulePreparationKey`. Forward a Need without local
event data and keep preparation-compute failure distinct from typed preparation
failure. Map supported root absence to the private module-not-found boundary;
never invoke the evaluator for `Unsupported`. For a supported present closure,
construct `NonrootModuleKey { name: route.module_name(), version: "" }` and pass
the root plus every ordered fragment occurrence to
`evaluate_direct_nonregistry_module_closure_with_events` only after the whole
preparation has completed. Success retains route provenance plus the compact
evaluated module and preserves the evaluator's declared version.

Own exactly one marker-conditional local batch on every Complete, including an
empty batch for preparation error, root absence, or unsupported capability.
Captured evaluation stores only its own ordered prints, including a prefix
before failure. Uncaptured fresh evaluation prints directly. Warm reuse neither
prints again nor carries evaluation data. Routed-REPO and every other child
batch remain child-owned and are neither copied nor replayed.

Tests must discriminate typed Need, preparation-compute, preparation,
root-absence, supported, unsupported, and evaluator outcomes; empty-key identity
and declared-version preservation; no evaluator activation for `Unsupported`;
complete-only equality; captured and uncaptured print; print-before-failure;
cold `Evaluated` versus warm `Reused`; source edit and downstream pruning; and
distinct child-versus-owner event batches. Structural checks keep the key and
support gate private, prohibit any `lib.rs` export or command caller, and
shorten the preparation-owner structural scan before this evaluator owner so
the preparation key remains independently evaluator- and event-free.

Stops: no second production file, public export/caller/activation/publication,
public unsupported-cycle status, registry/MVS/contextual mapping, fixture/
oracle, direct IO, evaluator semantic change, child-event copying/replay, or cap
breach. Public build/query/one-shot/daemon publication remains frozen pending
explicit user approval. `REPLAN` on any such expansion. Run focused serial
tests, formatting, GNU-Windows no-run, archive/scope/cap/diff gates, and an
independent latest-diff review; do not run Bazel.
