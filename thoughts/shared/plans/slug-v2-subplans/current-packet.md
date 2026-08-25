# Current Slug V2 Packet

Packet: `WP-4-5-selected-repository-file-effect-producer-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: `04-starlark-loading-and-build-packages.md` and
`05-bzlmod-and-repository-graph.md`
Design base: `e45dbcf8` plus the retained selected-owner/source-preflight Rust
candidate and independently accepted producer/application split

Result: implement one callerless selected-call DICE producer that reacquires
and authenticates the exact repository rule, executes the admitted string-only
`repository_ctx.file` subset, and returns a heap-free ordered effect plan.

## Frozen scope and baseline

The full fixture vertical is too broad for one packet: Starlark ABI, paired
DICE ownership, a cross-crate structural value, generated-route handoff and
secure root publication are five distinct proof boundaries. This packet closes
only the first three. No production caller is activated; a later docs design
owns route/request handoff and immutable-root application.

Write authority is exactly:

- new `app/slug_bzlmod_v2/src/generated_repository_file_effect.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`, 460 lines,
  `c697e9341eb1990c6ef47c157d0967e6a9ea1d4c94e71ad2c6415f5c9c7674ab`;
- new
  `app/slug_loading_v2/src/module_extension_repository_file_effect.rs`;
- `app/slug_loading_v2/src/module_extension_repository_rule.rs`, 633 lines,
  `b1bf1e89f23d66ecf2ffb4dfb5e0cad1ac17e375e53514662840d51c465fb380`;
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  2,090 lines,
  `9cfda25c7be7837be7911176d7ae9359523b69a02e3baa5f446a3373514f7b66`;
- `app/slug_loading_v2/src/module_extension_repository_validation.rs`, 1,943
  lines, `7265f96aeb5d2b1e856c4bef69a88e4596bbe6b8497b93fe7ad5968555be6c15`;
  and
- `app/slug_loading_v2/src/lib.rs`, 104 lines,
  `4d03dcfbbbc5a4d7a8cbcbb885219a37412543539061680f2a3d4a30d2c22b13`.

All other dirty Rust and fixture bytes are retained and non-writable. Within
the five existing files, change only module/doc-hidden reexports and these
crate-private seams: frozen rule projection/implementation accessors,
instantiated repository call accessor, and certificate repository-by-ordinal
accessor. Do not reshape any accepted selected/global value.

## Shared effect value

The new Bzlmod child defines doc-hidden nominal values only; it adds no key:

- `GeneratedRepositoryFileEffect` retains a normalized valid-Unicode
  repository-relative `CompactString`, exact `Arc<[u8]>` content and executable
  polarity;
- `GeneratedRepositoryFileEffectPlan` retains an ordered
  `Arc<[GeneratedRepositoryFileEffect]>`; and
- `GeneratedRepositoryFileEffectPlanError` distinguishes invalid path from
  repeated-path unsupported scope.

Construct each effect at the `ctx.file` call so invalid-path/repetition order is
not deferred until invocation completion. A valid path is nonempty and
relative, contains only nonempty normal `/`-separated segments, and contains no
`.`/`..`, backslash, NUL or trailing slash. Preserve call order exactly and
reject a repeated normalized path before recording the second effect. Content
is the exact UTF-8 byte sequence of Slug's valid-Unicode string; ASCII fixture
bytes are exact Bazel 9. Derive full structural Clone/Eq/Hash/Allocative over
path, bytes, executable and order. Expose borrowed accessors only.

Reuse `CompactString`, `Arc`, `SmallSet` as invocation scratch, and
`Allocative`; add no interner, cache, digest, retained map, Buck2/V1 import or
Stage 9 row.

## Loading producer

Add exactly one legacy/observed key pair:

`HostSelectedRepositoryFileEffectKey { workspace, owner, ordinal }`

and `HostSelectedRepositoryFileEffectObservationKey`. `owner` is the existing
`Arc<HostSelectedExtensionOwner>` and `ordinal` is the exact flattened ordinal
already selected by the generated-definition producer. Complete success is a
nominal value retaining the exact owner certificate, ordinal and shared plan.
The observed carrier retains that Result Arc plus `PathObservationEpoch`.

Compute in this order:

1. demand the existing selected-owner certificate legacy/observed key using
   the supplied workspace/owner;
2. select exactly `ordinal`, retaining MissingOrdinal as a typed semantic
   error; do not search by canonical name or execute another call;
3. derive a root Host `.bzl` label from the call's defining canonical label;
   nonroot defining repositories are typed unsupported in this first slice;
4. demand the existing Host-Bzl legacy/observed key and merge its observed
   epoch after the certificate epoch, left-first;
5. select the exported value by the retained exported rule name, downcast to
   `FrozenRepositoryRuleDefinition`, and compare defining label, exported name
   and ordered attribute schema with the retained call projection;
6. keep the reacquired frozen module alive only on the compute stack, allocate
   one invocation-local repository context/collector, and invoke the exact
   frozen implementation once; and
7. require a `None` return, freeze the ordered plan and drop evaluator, context,
   callable and module before constructing the retained value.

The context exposes only:

`file(path: str, /, content: str = "", executable: bool = True, legacy_utf8: bool = False)`.

`path` is mandatory positional-only. The other three parameters retain Bazel's
positional-or-named acceptance and defaults; `legacy_utf8` is accepted and
ignored exactly as Bazel 9.2. Preserve ordinary duplicate positional/named and
argument-order diagnostics. Label/path arguments, repeated/invalid paths and
every other repository-context member are outside the admitted slice and fail
closed under Path/Invocation owner errors without diagnostic-string dispatch.
Preserve the partial ordered effects only inside a heap-free semantic error
when needed to prove first-failure order.

Use a nominal error graph that distinguishes certificate semantic/compute,
ordinal, defining-label unsupported, Host-Bzl semantic/compute, projection
drift, path/repetition, invocation and non-None result. An observed Host-Bzl
outer or epoch merge error retains only the completed certificate plus ordinal,
never a failed child/module/heap. Need is carrierless. Cancellation publishes
no result or batch.

Host-Bzl load events remain at the existing child. When event capture is
enabled, the new key owns exactly one local complete invocation print batch on
success or semantic invocation terminal; pre-invocation outer/Need/cancellation
owns none. Warm equality reuse publishes no duplicate batch. Add no effect
event type or command-line publication.

## Proof, caps and compatibility

Focused proof must cover:

- exact two-file fixture-shaped order, ASCII bytes and default executable true;
- positional-or-named content/executable/legacy-UTF8 arguments, defaults,
  duplicate argument/order diagnostics, positional-only path and named-path
  rejection;
- invalid, absolute, parent, backslash, empty and repeated paths at exact call
  order; Label/path values and unknown context members rejected;
- exact owner/ordinal selection with missing ordinal and no sibling execution;
- defining-label/export/kind/schema/implementation reload authentication and
  A/B/A drift/restoration;
- Legacy/Observed parity, certificate-then-HostBzl epoch association,
  Need/outer/merge/semantic split, equality/validity and cancellation recovery;
- evaluated versus warm invocation event batches; and
- source/shape proof that no retained success/error/carrier contains
  `FrozenModule`, `FrozenValue`, Starlark heap/context/evaluator or I/O handle.

Run formatting; focused Bzlmod/loading proof; full Bzlmod then full loading
serially; dependent core check; `scripts/v2_archive_status.sh`; `git diff
--check`; exact seven-file scope/hash/accounting; forbidden-secret/stale-JVM
scans; and independent implementation review. Do not run the terminal fixture:
this callerless prerequisite intentionally has not changed its materialization
failure.

Caps are <=560 production, <=650 proof and <=1,210 aggregate added Rust lines.
Physical ceilings are <=300 for the new Bzlmod child, <=475 Bzlmod `lib.rs`,
<=1,150 for the new loading child, <=680 rule, <=2,125 instantiation, <=1,990
validation and <=120 loading `lib.rs`. Add no `rustfmt::skip`.

The fixture's distinct ASCII string-path `ctx.file` calls, defaults, bytes,
order and executable polarity are **exact Bazel 9**. Private key/error/carrier,
valid-Unicode path/content representation and epoch association are
**Slug-native**. Nonroot rule definitions, Label/path arguments, repeated paths,
all other `repository_ctx` members, route/materialization activation, generated
query breadth, other platforms and exact configuration/output identity are
**unsupported/deferred**.

Pinned Bazel 9.2 commit `8220c619…` authority is `RepoRule#instantiate`,
repository-rule definition reload/export and
`StarlarkBaseExternalContext#createFile`. Pinned `../zabel` commit `c7298478…`
is concept-only guidance for one `{origin, canonical repository, ordinal}`
producer, evaluated `.bzl` reload and heap-free effect result. Copy no Zig code,
representation, scheduler, digest, manifest/root format or output vector.

STOP a core/source-preparation/host-module/route/repository-I/O edit, caller or
fixture activation, second key family, canonical-name rescan, retained Starlark
state, unobserved Host read, direct filesystem write, new store/cache/lock/task,
global aggregate change, public API, native repository change, other context
API, cap/proof waiver, Java/JVM, milestone closure, M8/M7B or identity-byte
work. `REPLAN` before widening. After implementation ACCEPT, design only the
generated effect-plan route/request/materialization handoff; M7 remains partial
and M7A -> M8 -> M7B remains.
