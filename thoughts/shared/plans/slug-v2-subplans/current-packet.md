# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-mapping-owner-design-r3-correction`
Milestone: cross-stage M7 prerequisite design correction
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: correct the root override target-resolution phase before any selected
extension-mapping Rust begins.

## Active design correction contract

The first implementation audit stopped before Rust because the accepted r2
design described root override/inject targets as resolving through a deps-only
root mapping. Pinned Bazel 9.2
`BazelDepGraphFunction.resolveRepoOverrides` instead calls
`BazelDepGraphValue.getRepositoryMapping` with all selected extension usages
and unique names, but with the resolved override table empty. Therefore an
override target may be a root import from another extension. The checked-in
`root-extension-usage-semantics` fixture discriminates this: `replacement`
is imported from the innate `use_repo_rule` usage and overrides a repository
from the ordinary extension.

Correct the frozen owner into an explicit two-phase projection:

1. resolve IDs and unique names, then construct every module's full
   no-overrides mapping from Bazel dependencies plus all proxy imports;
2. resolve every root override/inject target through that completed root
   no-overrides mapping, then construct final full mappings by replacing
   matching exported destinations with resolved targets.

The correction must freeze ordering and error precedence for duplicate
no-overrides imports, missing override targets, override chains/cycles, and
the final replacement pass. It must revalidate that one private
`selected_repo_spec.rs` owner and the existing 520/800/1,320 caps remain
credible, or revise caps/REPLAN truthfully. Generated repository existence and
`must_exist` validation remain post-evaluation and deferred.

This packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap formatted net growth at 180 manifest lines, 160 owner-plan lines, 30
canonical lines, and 370 total. Inspect only pinned Bazel 9.2 source, accepted
owners, and checked-in evidence read-only. Obtain fresh independent
reserved-architecture correction review.

No Rust, Cargo/BUILD, fixture mutation, public API, predecessor mutation,
extension evaluation, generated-existence validation, I/O, RepoSpec/
materializer/loading/consumer, command, analysis, execution, or JVM/Java work
is authorized. Return `REPLAN` if exact two-phase mapping needs an absent
input, a second owner/file, extension evaluation, I/O, or cap excess. No Rust
may resume before independent `ACCEPT` and explicit r3 implementation
activation.

## Historical evidence

This section grants no file, action, cap, or scheduling authority.

Commit `a3c0d458` accepted every other r2 design boundary: resolved selected
route order, canonical extension/isolation IDs, first-encounter unique-name
collision handling, root/nonroot usage ownership, structural equality,
complete-error-over-Need, compact representation, and the pre-evaluation stop.
Commit `14f4288f` activated implementation, but no Rust was written before
the target-resolution mismatch was found.

The correction does not require generated repository names. All root proxy
imports already map exported names to `<unique-name>+<exported>`; this is
enough to form the no-overrides root mapping and resolve cross-extension
override targets. Bazel prevents an overriding apparent repo from itself being
overridden, so the resolution is nonrecursive; the final pass substitutes the
already resolved canonical target.
