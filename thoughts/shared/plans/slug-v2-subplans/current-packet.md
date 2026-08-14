# Current Slug V2 Packet

Packet: `WP-2A-m1-host-package-marker-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the smallest callerless Bzlmod-private observed root
package-marker/lookup sibling over immutable policy, the accepted observed
repository-ignore frontier, and ordered `BUILD.bazel`/`BUILD` path-resolution
frontiers. This packet is docs-only and cannot implement or activate a caller.

## Accepted predecessor

Commit `43adf74b` accepts the callerless
`HostRepositoryIgnoreObservationKey` from design `8ac5c30f` and exact
activation `c4ecd395`. It
preserves legacy REPO -> policy -> ordered `.bazelignore` semantics and
retains the complete accepted Host epoch with no parent event authority.

The sole Rust file changed by +708/-16 raw lines: 243 production and 449
in-module test lines, 692 net total, with 2,783 physical lines. These are within
280/450/730 and 2,821. Focused observed proof passes 4/4; all 568 Bzlmod
unit/integration tests pass; `slug_core_v2` checks; formatting and diff hygiene
pass. Strict Clippy stops first in unchanged `allocative_derive`; its
no-dependency run reports only pre-existing crate warnings after the new local
warning was removed. The archive checker reproduces the inherited missing
archive-ref/non-V2-thoughts baseline. WSL has no Windows Rust target, so the
cfg-windows duplicate/operation-mismatch proof is source-checked but not
executed. Independent ownership and AI-cleanup review accepts the 2,783-line
repository-ignore file as one cohesive owner.

This predecessor is still a private producer. It does not certify package
lookup, MODULE loading, source bytes, BUILD evaluation, or any public command.

## Live source boundary

`HostRootPackageLookupKey { workspace, package }` in `host_package.rs`
currently owns one exact ordered terminal:

1. compute `RootPackageLookupInputsProjectionKey`;
2. return structural policy errors, invalid package names, configured deleted
   packages, or the special `external` no-build-file terminal before Host work;
3. compute legacy `HostRepositoryIgnoreKey`, preserving Need/error and mapping
   a match to `Deleted`;
4. for each configured package root in order, probe `BUILD.bazel` and then
   `BUILD` through legacy `ResolvedPathKey`;
5. select the first regular/special marker, preserve resolution error
   precedence, or return `NoBuildFile` after every negative probe.

The accepted `HostRepositoryIgnoreObservationKey` supplies the complete
predecessor epoch. The accepted `ResolvedPathObservationKey` supplies each
exact symlink/lstat/negative resolution prefix. Policy, package identity,
deleted packages, root order, and build-file-name order are immutable semantic
inputs, not reconstructed Host observations.

Root MODULE anchoring, BUILD bytes/evaluation, recursive `.bzl`/glob work,
package-source selection, loading, core finalization, and public commands are
downstream of this lower lookup terminal and remain outside this packet.

## Required design output

Freeze one design or record `REPLAN`; do not write Rust.

- Name the natural type/key/value owner, crate visibility, Display identity,
  and exact relationship to legacy `HostRootPackageLookupKey`. Prefer one
  callerless sibling in `host_package.rs`; legacy key/value/callers remain
  unchanged and neither sibling computes the other.
- Decide whether
  `HostRootPackageLookupObservationKey { workspace, package }` returns
  `PathOutcome<Result<ObservedHostRootPackageLookup,
  ObservedPathFrontierError>>`, with one
  `Arc<Result<HostRootPackageLookup, HostRootPackageLookupError>>` plus the
  accepted `PathObservationEpoch`. If another shape is necessary, justify the
  smaller ownership boundary.
- Freeze exact complete-only equality and validity. Need and cancellation must
  retain no partial carrier; completed outer aggregation errors retain no
  partial carrier and never become legacy semantic errors.
- Preserve policy-first and early-exit order. Policy error, invalid package,
  configured deletion, and special `external` must have an exact empty Host
  epoch because no Host predecessor ran.
- Compute only the observed repository-ignore sibling after early exits. Its
  complete semantic success/error must retain its exact epoch; an ignore match
  completes `Deleted` without marker activation.
- Probe only `ResolvedPathObservationKey` for each root-major,
  `BUILD.bazel`-then-`BUILD` candidate. Union each completed epoch before
  interpreting it. Missing and non-file terminal kinds remain negative probes;
  regular/special selects; resolution semantic error retains the complete
  prefix; all-negative completes `NoBuildFile`.
- Freeze deterministic union, duplicate/conflict/operation-mismatch algebra,
  source order, exact first-Arc retention, cardinality/overflow handling, and
  proof that no demand is reconstructed above its workspace owner.
- Preserve legacy result/error text, source ordering, equality, public output,
  and all callers. The observed carrier is private, callerless, and cannot
  become accepted-snapshot or public certificate authority.
- Classify memory as DICE-terminal only: one semantic-result Arc plus one
  existing Arc-backed epoch. Policy, matcher, resolution machine, transactions,
  events, evaluators, workers, and scratch must not be retained.
- Freeze one-file implementation/test caps, the physical-file ceiling from the
  live 3,355-line `host_package.rs`, a mandatory >2,000-line cohesion
  decision, focused proof, direct-dependent validation, and the unique next
  docs-only hierarchical consumer.

Focused proof must discriminate policy/invalid/deleted/external empty epochs;
repository-ignore success/error/Need/outer and ignored deletion; root/name
precedence; missing/wrong-kind/selected/resolution-error prefixes; exact Arc
retention; duplicate/conflict/mismatch; no legacy sibling activation;
complete equality/validity; A/B/A/warm; Need/cancellation; and unchanged legacy
diagnostics/order. No new Bazel oracle is required because existing serial
package selection and admitted Host observations remain exact regression
invariants. Frontier aggregation/equality are Slug-native. Higher loading,
overlap/final-validation, directory/glob unions, repository/materialization,
and exact Bazel identity bytes remain unsupported/deferred.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet, this owner section,
`docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6
row of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{lib,host_package,package_policy,repository_ignore,host_file}.rs`,
workspace `src/{lib,path_observation,path_resolution}.rs`, the Bzlmod and
workspace manifests, and directly referenced focused tests in those files.

The packet may name exactly one future `host_package.rs` implementation and
its colocated tests, but cannot activate it. Ledger caps are 40 canonical, 320
current-packet, 280 Stage 2, and 640 total net lines. No code, Cargo, oracle,
generated evidence, or Stage 9 write is authorized.

## STOP / REPLAN

STOP on every code or oracle write; a second package consumer; root MODULE,
source bytes, BUILD evaluation, `.bzl`, glob, loading, core, request-revision,
events, public command/API/output, external/routed/materialized repositories,
legacy key/value/error changes, public export, Cargo/dependency change, reverse
edge, generic certificate framework, new container/cache/interner/graph/store,
reconstructed or direct/historical Host reads, watcher, JVM, or cap excess.

REPLAN to one smaller docs-only prerequisite if the accepted observed
repository-ignore or resolved-path carrier cannot be consumed without changing
legacy keys; policy/root/name precedence cannot be represented structurally;
some lookup terminal has an additional mutable predecessor; exact negative
probes are unavailable before completion; union requires another retained
container; visibility escapes Bzlmod; or one cohesive `host_package.rs`
implementation cannot be bounded.

## Immediate successor

On acceptance, activate exactly one implementation of the frozen private
package-marker sibling, or one smaller docs-only prerequisite if a REPLAN
condition is proved. Do not combine root MODULE, package source, loading, core,
or public activation.
