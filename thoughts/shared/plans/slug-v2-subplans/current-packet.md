# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-include-progress-semantics-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: freeze the exact direct/indirect root-MODULE `include()` recurrence,
alias, repeated-occurrence, error, and progress contract needed before a
complete observed root-module frontier can exist. This packet is
documentation-only.

## Accepted predecessor and REPLAN

Commit `0875728b` accepts the private callerless observed package-marker
frontier. It changes only `host_package.rs` by 211 production plus 429 test
lines, passes 6/6 focused and all 574 Bzlmod tests, checks the direct core
dependent, and retains one semantic Arc plus the accepted Arc-backed epoch.
Independent ownership and cleanup review accepts its exact/callerless boundary.

The subsequent root-module frontier audit found that the accepted lower
observed Host-file and package-lookup APIs are sufficient, but the legacy root
module closure has no finite recurrence terminal. `HostRootModuleFileKey`
repeatedly replaces its horizon with every newly inspected include request.
A valid include file that includes itself, or an indirect recurrence, therefore
repopulates the horizon forever and never reaches evaluation, an event batch,
or a complete DICE value.

The older `RootModuleEvaluationKey` skips a previously seen raw include label.
That is not reusable evidence: the Host path deliberately validates and
evaluates repeated acyclic occurrences, including aliases that resolve to the
same logical path, and its focused tests assert repeated events. Neither
raw-label nor canonical-path suppression may be adopted without deciding
recurrence ancestry, alias identity, diagnostic order, and Bazel 9.2 behavior.
No current `HostRootModuleFileError` variant owns this terminal. The root-module
frontier packet therefore `REPLAN`s before Rust activation.

## Required source decision

Establish one bounded contract or select one focused oracle prerequisite:

1. Locate and record the exact Bazel 9.2 implementation and tests for root
   `MODULE.bazel` `include()` traversal, direct/indirect cycles, repeated
   occurrences, label aliases, diagnostics, and source locations. If source
   does not discriminate a public result, name the smallest one-fixture Bazel
   oracle needed; do not infer parity from Slug or donor behavior.
2. Classify the identity that defines ancestry: raw spelling, apparent label,
   canonical package/target, selected logical Host path, or another
   source-backed identity. Distinguish a recurrence on one active ancestry path
   from repeated acyclic sibling occurrences.
3. Freeze traversal and terminal order for root validation, horizon label
   parsing, package preflight, grouped include-file observation, include
   validation, recurrence, and evaluation. Preserve the existing all-Need
   union and source-order semantic error selection after each concurrent batch.
4. Define the typed legacy error and exact location/message class, or classify
   recurrence as unsupported with a finite fail-closed terminal. No hang,
   silent dedupe, arbitrary depth cutoff, panic, or string-matched error is
   admissible.
5. Name the natural progress owner and lifetime. Prefer command-local
   per-occurrence ancestry/progress scratch owned by
   `HostRootModuleFileKey`; retain no global visited set, DICE side store,
   evaluator heap, transaction, event batch, or second source cache. Preserve
   DICE equality and invalidation on the completed semantic result.
6. Preserve missing-root bootstrap Need, Need/cancellation release, repeated
   acyclic occurrence validation/events, and warm and A/B/A behavior.
   Need/cancellation/nonterminal recurrence publishes no parent root-module
   frontier carrier or parent completed event batch; completed child DICE
   observations remain ordinary dependency-owned cache state.
7. State whether the behavior correction and its proof fit one independently
   bounded legacy packet before resuming
   `WP-2A-m1-root-module-frontier-design`. Freeze exact Rust/fixture allowlists,
   production/test/total caps, physical ceilings, direct dependent validation,
   and the mandatory cohesion decision for `host_module.rs` above 2,000 lines.

Existing admitted serial acyclic root MODULE/include behavior, diagnostics,
event order, and repeated occurrences remain exact regression/non-widening
invariants. Recurrence behavior remains unclassified until this packet obtains
pinned Bazel evidence; any finite Slug-only progress safeguard must be labeled
Slug-native or unsupported rather than exact. Frontier aggregation and dynamic
sealing identity remain Slug-native. Lockfile/registry, package source,
BUILD/.bzl/glob, loading/core/public activation, routed/materialized
repositories, overlap/final validation, and exact Bazel identity bytes remain
unsupported/deferred.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `slug-v2-subplans/current-packet.md`; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only the active packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, local Bzlmod
`src/{host_module,host_include,module_eval,host_file,host_package,root_bootstrap,lib}.rs`,
loading `src/bzl_module.rs`, their manifests and directly referenced focused
tests, the pinned Bazel 9.2 bzlmod implementation directory under
`src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`, its matching
`src/test/java/com/google/devtools/build/lib/bazel/bzlmod/` and
`src/test/py/bazel/bzlmod/` tests, and the existing
`module-file-directives` oracle manifest/fixture. A bounded symbol search may
identify exact Bazel files inside only those directories before reading them.
If the pinned tree is unavailable, official Bazel v9.2.0 source pages for those
same files are the only network substitute.

This docs-only packet is capped at 40 net canonical lines, 300 current-packet
lines, 260 Stage 2 lines, and 600 total net ledger lines. It authorizes no Rust,
Cargo, oracle, fixture, generated-file, or archive write.

## STOP / REPLAN

STOP on every code or oracle write; root frontier implementation; new DICE key,
store, graph, cache, interner, evaluator, public API/output, Cargo/dependency,
loading/core activation, lockfile/registry/package-source/BUILD/.bzl/glob work,
routed/materialized repositories, watcher, JVM, or combining another behavior
family.

REPLAN to exactly one focused Bazel oracle packet if pinned source/tests do not
discriminate recurrence identity, terminal, message class, or ordering. REPLAN
to a smaller docs prerequisite if recurrence cannot be separated from accepted
acyclic occurrence behavior or requires ownership outside the root-module
producer. Do not invent a terminal merely to unblock frontier aggregation.

## Immediate successor

On acceptance, activate exactly one bounded include-progress implementation or
one focused discriminating Bazel oracle packet. After that prerequisite is
accepted, resume docs-only `WP-2A-m1-root-module-frontier-design`; do not combine
the frontier implementation.
