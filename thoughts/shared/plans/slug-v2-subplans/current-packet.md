# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design and freeze exactly one callerless Bzlmod-private observed
root-MODULE/include frontier sibling. It must retain every exact Host
observation that selects and evaluates the dynamically sealed root module
closure while preserving legacy keys, events, diagnostics, and public
behavior. This packet is documentation-only.

## Accepted predecessor

Commit `0875728b` accepts the callerless
`HostRootPackageLookupObservationKey` and
`ObservedHostRootPackageLookup` from design `2c174ca1`. The sibling preserves
policy and early exits, consumes only the accepted observed repository-ignore
frontier, and probes `BUILD.bazel` before `BUILD` in configured root order.
Every complete child epoch is unioned before semantic interpretation; Need,
outer error, and cancellation publish no parent carrier.

The one-file change is +640/-0 raw and net: 211 production plus 429 in-module
test lines, with 3,995 physical lines. Focused observed proof passes 6/6; all
574 Bzlmod unit/integration tests pass; `slug_core_v2` checks; formatting and
diff hygiene pass. Strict Clippy stops in inherited workspace/crate warnings,
and the archive checker reproduces the inherited archive-ref/non-V2-thoughts
baseline. Independent ownership and AI-cleanup review accepts the large file
as one cohesive package lookup/source owner. Retained state is exactly one
semantic-result Arc plus the accepted Arc-backed epoch.

This accepted lower producer remains private and callerless. It does not yet
certify root MODULE evaluation, package-source bytes, BUILD evaluation,
loading, core finalization, or any public command.

## Live source boundary

`RootModuleLoadingAnchorKey` is the outward loading anchor over the
crate-private `HostRootModuleFileKey`. The Host key currently reads the root
`MODULE.bazel` through legacy `HostFileBytesKey`. A missing root module is a
bootstrap `Need` rather than a completed value.

After a root file is present, the key discovers include labels and repeatedly
extends a first-seen include horizon. `preflight_root_include_horizon` resolves
each discovered include package through legacy `HostRootPackageLookupKey`;
Need suppresses completion, while invalid/deleted/no-build-file/operational
outcomes preserve their existing completed error order. Each selected include
is then read through the legacy Host-file path. The horizon is complete only
when evaluation discovers no new include work; completed success and semantic
evaluation errors are published only after that seal.

The accepted `HostFileBytesObservationKey` and
`HostRootPackageLookupObservationKey` now provide exact lower frontiers without
reconstructing demands. The design must determine whether one private observed
root-module sibling can consume them while sharing the existing parser,
preflight, evaluation, event, and diagnostic ownership.

`HostVisibleLockfileKey` is orthogonal to this loading anchor and belongs to
later registry/lockfile resolution. `RootPackageSourceKey` and BUILD/.bzl
loading occur downstream of the anchor. Neither is part of this design.

## Required design output

Freeze one bounded architecture or record `REPLAN`. The accepted design must
answer all of the following:

1. Name the exact crate-private key, carrier, visibility boundary, and
   semantic-result/error Arc. Preserve `HostRootModuleFileKey` and every legacy
   caller; neither sibling may compute the other.
2. Specify the exact dependency order: structural root-module inputs, observed
   root `MODULE.bazel` bytes, each first-seen include label, observed package
   lookup, and observed include bytes. Include every decisive positive,
   negative, symlink, and error prefix supplied by the accepted children.
3. Define dynamic horizon sealing and deterministic union order. Complete
   success or semantic error may carry a frontier only after no undiscovered
   include can affect that terminal. Need and cancellation must retain no
   partial carrier.
4. Preserve the existing missing-root bootstrap Need, include label/package
   error precedence, cycle/nonprogress behavior, evaluator result, diagnostics,
   and event order. Events may remain owned by the existing evaluation path but
   cannot become certificate authority or be retained in the frontier.
5. Separate completed inner semantic errors from
   `ObservedPathFrontierError`. Child outer errors and union
   mismatch/conflict must remain complete outer errors with no carrier; no
   panic, string matching, or laundering into legacy errors is allowed.
6. Reuse `PathObservationEpoch::from_shared` and exact child result Arcs.
   Classify transient include horizon/evaluator state separately from the
   DICE-retained semantic Arc and epoch. Retain no evaluator, transaction,
   event batch, matcher, resolved child, second collection, cache, interner,
   graph, or historical Host state.
7. Define complete-only equality/validity, A/B/A and warm behavior, Arc clone
   boundaries, cancellation release, and zero legacy-key activation proof.
8. Name the smallest future Rust allowlist, production/test/total caps,
   physical ceilings, platform gates, direct dependent validation, and the
   mandatory cohesion decision for every touched file above 2,000 lines.
   Select exactly one implementation packet or one smaller prerequisite.

No new Bazel oracle is required for a private representation-only design.
Existing serial root MODULE/include behavior, diagnostics, event order, and
admitted Host observations remain exact regression/non-widening invariants.
Frontier aggregation, dynamic sealing identity, equality, and retry ownership
are Slug-native. Lockfile/registry resolution, package source, BUILD/.bzl/glob
evaluation, loading/core/public activation, overlap/final validation,
routed/materialized repositories, and exact Bazel identity bytes remain
unsupported/deferred.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `slug-v2-subplans/current-packet.md`; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet and owner section,
`thoughts/shared/plans/slug-v2-plan-authoring-guide.md`,
`docs/developers/dice.md`,
`.codex/skills/slug-buck2-utility-reuse/SKILL.md`, the matching Stages-3/6 row
of `slug-v2-subplans/09-v1-extraction-ledger.md`,
`gazebo/dupe/src/lib.rs`, `allocative/allocative/src/lib.rs`, Bzlmod
`src/{lib,host_module,host_include,host_file,host_package,root_bootstrap,module_eval,package_policy,repository_ignore}.rs`,
loading `src/{lib,bzl_module}.rs`, workspace
`src/{lib,path_observation,path_resolution}.rs`, the four relevant Cargo
manifests, and directly referenced focused tests in those files.

This docs-only packet is capped at 40 net canonical lines, 340 current-packet
lines, 300 Stage 2 lines, and 680 total net ledger lines. It authorizes no Rust,
Cargo, oracle, fixture, generated-file, or archive write.

## STOP / REPLAN

STOP on code, Cargo, oracle, fixture, public export/API/output, loading/core
activation, lockfile/registry/package-source/BUILD/.bzl/glob implementation,
routed/materialized repositories, legacy key/value/error changes, reverse
dependency, generic certificate framework, new retained container/cache/
interner/graph/store, reconstructed/direct/historical Host reads, watcher, JVM,
or combining another consumer.

REPLAN to one smaller docs-only prerequisite if the dynamic include horizon
cannot be sealed before every completed semantic terminal; a completed error
cannot retain its entire mutable predecessor frontier; include discovery
requires a reconstructed Host demand or legacy-key rewrite; event/evaluator
ownership would have to enter the certificate; exact child epochs cannot be
unioned without new retained storage; visibility escapes Bzlmod; or one
independently bounded implementation cannot be named.

## Immediate successor

On design acceptance, activate exactly one bounded Bzlmod-private root-module
frontier implementation, or the single smaller prerequisite proved necessary.
Do not combine lockfile, registry, package source, BUILD/.bzl/glob, loading,
core, request-revision, or public activation.
