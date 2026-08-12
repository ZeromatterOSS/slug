# Current Slug V2 Packet

Packet: `WP-5-host-selected-extension-definition-load-request-owner-implementation`
Milestone: M7 Bzlmod-to-loading prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement the accepted hidden selected definition-load request
projection without widening source/load/evaluation ownership.

## Active implementation contract

Implement the accepted cross-crate input after commit `75a431d6`. The private
selected-extension mapping owner in
`slug_bzlmod_v2` must remain the sole routes/usage/mapping computation. Expose
only one narrow, `#[doc(hidden)]`, heap-independent request projection for the
accepted root-main-repository, ordinary nonisolated extension slice so a later
`slug_loading_v2` key can load and validate the definition without reversing
the crate dependency or recreating selected state.

Freeze:

- one Bzlmod-owned aggregate request key identified by normalized workspace;
- selected-extension mappings first, with Need invalid and completed
  routes/root-files/projection errors terminal before any request publication;
- deterministic first-encounter request order and exact deduplication by the
  accepted extension ID;
- one compact request per admitted extension retaining its exact ID, canonical
  root bzl label, exported extension name, complete selected mapping context,
  and the predecessor identity needed for structural equality/invalidation;
- a deliberately narrow cross-crate surface exported only for loading, with no
  mutable fields, caller-supplied mapping, RepoSpec, source path, callable, or
  generic route consumer;
- proof that absent/changed/restored root usage, label/export/mapping changes,
  Need, predecessor error, warm reuse, and order/dedup all flow through the
  accepted private owner exactly once.

Exact: selected mapping/error order, canonical label and extension identity,
ordinary grouping/deduplication, and request encounter order for the admitted
Bazel 9.2 slice. Slug-native: the hidden Rust surface, compact containers,
diagnostic wording, and internal identity bytes. Unsupported/deferred:
isolated, MVO-owner, innate, nonroot/registry definition loading, bzl source or
load observation, `module_extension`/`tag_class` globals, export/schema lookup,
Starlark heaps/callables, execution/factors, generated repos/existence,
environment/OS, lockfile, materialization, loading consumers, and commands.

## Scope, proof, and stops

This implementation packet may edit only:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

Cap formatted Rust net growth at 160 production, 260 tests, and 420 total,
measured against `0552dcf3`. Require focused pure projection/order/dedup/error
tests; real-DICE Need invalidity/non-self-equality, predecessor error, cold/warm
reuse, and label/export/mapping/order A/B/A restoration; full Bzlmod owner and
loading integration suites; formatting/diff/scope/compact/cleanup audits; and
independent implementation review.

No Cargo/BUILD, fixture mutation, selected-owner semantic mutation, loading
source or evaluator work, retained Starlark heap/callable, extension execution,
generated-name/spec/existence fabrication, registry/network/environment I/O,
materialization, lockfile write/replay, command/loading consumer, JVM/Java, or
generic public API is authorized. The only exported surface is the exact
`#[doc(hidden)]` loading request key/value/error boundary. `REPLAN` on a
generic public consumer, second selected projection, source/load work, loading
dependency, third Rust file, or cap excess.

## Accepted request-projection design

This section is historical and grants no separate file, action, cap, or
scheduling authority. Independent review accepted the prior docs-only design,
two-file seam, representation, proof matrix, compatibility classification,
160/260/420 caps, and stops.

## Completed definition-owner audit

This section is historical and grants no separate file, action, cap, or
scheduling authority.

Pinned Bazel 9.2 `RegularRunnableExtension.load` loads the mapped bzl file and
then performs exported `ModuleExtension` lookup; `SingleExtensionEvalFunction`
uses that definition before execution and generated-repository validation.
Slug's reusable Host/External Bzl evaluators and frozen module lifetime closure
are private to `slug_loading_v2`, whose semantic equality is the complete
`BzlLoadManifest`. The selected extension mappings and key are private to
`slug_bzlmod_v2`; loading already depends on Bzlmod.

A private definition key in Bzlmod would create a reverse crate dependency or
a second loader. A private key in loading cannot establish selected-mapping-
first ordering. The current loading globals also lack `module_extension` and
`tag_class`. Therefore the prior private-only packet returns `REPLAN`. The
smallest prerequisite is the hidden heap-independent request projection in the
active contract. Once accepted, a later loading owner may borrow the existing
cached frozen module only as lifetime state and publish a compact definition
value whose equality retains the transitive manifest and exported schema, not
a callable.

## Accepted predecessor evidence

This section is historical and grants no separate file, action, cap, or
scheduling authority.

Commit `75a431d6` is independently accepted. Its private routes-first owner
uses only resolved selected entries and the root/nonroot retained usage
owners; groups ordinary and isolated IDs; assigns exact first-encounter names
including non-`extension` isolated collision suffixes; builds complete
no-overrides mappings; resolves root targets through the completed root
mapping; and performs final substitution while retaining `must_exist`.
Growth is 454 production and 516 net test lines in one file, within
520/800/1,320. Five new focused rows, all 345 owner tests plus integrations,
the full loading suite, formatting/diff/scope/compact/cleanup audits, real-DICE
Need/error/A-B-A/reuse, and independent review pass.

Generated repository existence remains deliberately unknown. It may be
validated only after a future exact extension execution owner returns its
generated name set.
