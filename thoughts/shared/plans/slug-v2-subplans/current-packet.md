# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-evaluation-upper-source-owner-audit`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/Rust base: `1815c019`
Result: audit only the first complete owner above accepted observed direct-local
evaluation; choose one bounded design, one uniquely smaller prerequisite, or
formal REPLAN.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`

Against `1815c019`: at most 40 canonical net lines and 80 physical lines;
180 current-manifest net lines and 220 physical lines; 180 Stage 2 net lines
and 5,000 physical lines; 400 aggregate net lines. This packet authorizes no
Rust, Cargo/BUILD, fixture, oracle or generated-artifact write.

## Required audit

Trace the accepted callerless observed support outcome through
`RepositoryPackageSourceKey`, its package-lookup and selected BUILD-source
children, recursive `ExternalBzlModuleEvalKey`, `RepositoryPackageLoadKey`,
and the remaining loading-query/build consumers. For every candidate record:

- the smallest DICE key or reusable driver that owns the complete semantic
  terminal and every mutable path dependency;
- matching legacy/observed family selection, exact shared Result-Arc and epoch
  order, terminal prefixes, Need/typed-outer/semantic precedence, validity and
  equality;
- the sole owner and order of each Complete event batch, plus cancellation and
  failed-attempt publication behavior;
- retained semantic graphs/epochs versus compute-local AST, frontier, load,
  event and outcome scratch;
- cold/warm, create/edit/delete/recreate, A/B/A, family-nonactivation and public
  retry consequences.

Determine whether `RepositoryPackageSourceKey` can be the first complete
observed owner using the accepted support, external-package lookup and selected
source carriers; whether recursive external `.bzl` source/evaluation requires
one uniquely smaller sibling frontier first; or whether the constraints require
formal REPLAN. Do not select `RepositoryPackageLoadKey` merely because it is
higher: it also owns recursive load evaluation and a BUILD event batch.

## Compatibility and terminal

Exact surfaces to preserve are admitted direct-local support/source values and
errors, BUILD and `.bzl` bytes and Starlark semantics, load order, package
values, and existing child event text/order. Slug-native candidates are
structural sibling keys, compact path epochs, typed observed outer errors and
retry association. Recursive external evaluation, package load, loading-query
and build publication remain unsupported/deferred until an accepted design and
implementation activate them.

Terminate in exactly one of:

1. one docs-only bounded design for the smallest complete natural owner;
2. one docs-only uniquely smaller prerequisite design that returns directly to
   this audit after its implementation; or
3. formal REPLAN with the concrete incompatible ownership constraints.

Any selected design must freeze exact future Rust files and measured semantic/
physical caps, full carrier and terminal algebra, event and lifetime authority,
family isolation, discriminating proof, validation, Buck2 retention review, AI
cleanup and independent review. Only after independent design ACCEPT may one
implementation packet follow.

## STOP / REPLAN

STOP on Rust, Cargo/BUILD, fixtures/oracles, caller or public activation,
identity-byte claims, mixed legacy/observed families, reconstructed or partial
epochs, duplicate/moved events, retained frontier/AST/outcome scratch, new
stores/locks/tasks/direct Host reads, unmeasured future caps, multiple
successors or M1 closure. `REPLAN` if no bounded owner can retain the complete
selected dependency epoch and existing event semantics without crossing an
unaccepted family or adding a new ownership boundary.
