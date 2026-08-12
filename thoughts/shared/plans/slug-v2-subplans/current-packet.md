# Current Slug V2 Packet

Packet: `WP-4-5-host-module-extension-evaluation-input-composition-design`
Milestone: M7 module-extension pre-execution input composition
Owners: `slug-v2-subplans/04-starlark-loading-and-build-packages.md` and
`slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: audit and freeze the smallest loading-owned schema/type-check boundary.

## Active design contract

Perform a read-only ownership audit for one callerless loading-owned DICE key
that composes the accepted heap-free definition aggregate with the accepted
heap-free selected evaluation-input aggregate. It must prepare typed root
module/tag views but must not reacquire or publish a callable, construct
`module_ctx`, or execute an extension.

Pinned Bazel 9.2 `SingleExtensionEvalFunction` obtains the selected usage value
before `RegularRunnableExtension.load`; `StarlarkBazelModule.create` then walks
modules/tags, looks up each tag class, and calls `TypeCheckedTag.create` with a
label converter for that module's repository mapping. Audit and freeze:

- raw selected-input computation before definition loading, including completed
  raw-input error/Need precedence and zero Host-bzl observation on that terminal;
- exactly one join by the complete accepted load request, rejecting absent,
  duplicate, reordered, or extra definition/input rows rather than matching
  only label or exported name;
- root-module encounter order, source-order tags, tag-class declaration order,
  module/tag sort indices, dev-dependency and logical-location retention;
- tag-class lookup, unknown/missing attribute, mandatory/default, raw-value
  type checking, and exact first-error order for the admitted schema;
- label/default conversion through the exact request context repository and
  immutable selected mapping, with every semantic input retained structurally;
- a heap-independent prepared value retaining both complete predecessors,
  exact load request, manifest/schema identity, module identity, typed/defaulted
  attributes, dev flag, location, and ordering identity;
- complete-only DICE equality/validity and contextual typed terminals. Need is
  invalid and non-self-equal.

The first successor admits exactly this matrix. `None` in the supplied map is
omission for every admitted kind; a mandatory omitted value fails. Declared
defaults are the already-coerced definition-owned values below; absent optional
defaults use the listed intrinsic value.

| kind | supplied raw shape | declared/intrinsic default | conversion owner |
|---|---|---|---|
| `String` | `String` only | `String` / `""` | scalar projection |
| `Boolean` | `Bool` only | `Boolean` / `false` | scalar projection |
| `Integer` | `Int::Small(i32)` only | `Integer` / `0` | scalar projection |
| `Label` | `String` or retained `Label` token | canonical `Label` or `None` / `None` | supplied values use the module context repository plus the request's immutable selected mapping; declared defaults remain the definition-load owner's already-canonical value and are not re-resolved |

Reject a mismatched declared-default variant. Defer `LabelList`,
`StringKeyedLabelDict`, `LabelKeyedStringDict`, `LabelListDict`, `Output`,
`OutputList`, `StringList`, `StringListDict`, and `StringDict`; raw list and
tuple stay distinct but both fail closed, as do every dictionary shape,
big-decimal integer, float token, builtin-print token, extension proxy, and
self-list. `allow_single_file` remains structural schema identity but causes no
file observation or target validation in this pre-execution owner. Definition
loading already rejects nondefault unprojected restrictions including allowed
values, so the admitted phase has no allowed-value predicate to run.

Freeze Bazel's two-phase per-tag algorithm exactly. First walk the retained
supplied `SmallMap` order, skip `None`, and fail at the first unknown name or
raw type/label-conversion error. Then walk declaration-order schema slots, fail
on the first missing mandatory value, insert the declared or intrinsic default,
and fail on the first non-visible label. Publish only after all source-order
tags complete. Duplicate raw names are impossible in the retained `SmallMap`
and are rejected by the existing MODULE evaluator/syntax owner; composition
adds no fabricated duplicate check. Reuse the existing loading schema and
compact/Buck2-derived containers; do not add a second raw-value or schema owner.

## Compatibility boundary

Exact only for the admitted root-main, ordinary, nonisolated, singleton-module
Bazel 9.2 slice: usage-before-load ordering, tag-class/type/default semantics,
module and source tag order, label resolution, dev identity, and structural
invalidation. Slug-native: private key/type names, compact layout, diagnostic
wording, and internal scheduling where Bazel exposes no user-visible order.
Deferred: nonroot/MVO/isolation/innate inputs, callable reacquisition,
`module_ctx`, facts/environment/OS inputs, implementation execution/events,
repository rules, generated names/RepoSpecs/existence, override/inject final
validation, lockfile replay/write, materialization, commands/consumers, and
exact JVM identity bytes.

## Scope, proof, successor, and stops

This docs-only packet may edit exactly canonical, this manifest,
`04-starlark-loading-and-build-packages.md`, and
`05-bzlmod-and-repository-graph.md`. Cap net growth at 45 canonical, 240
manifest, 220 Stage 4, 220 Stage 5, and 725 total lines. Require pinned Bazel
9.2 source/test anchors, live visibility and error-order audit, exact versus
Slug-native/deferred classification, compact-utility review, explicit future
allowlist/caps/proof/stops, and independent design review.

If the audit proves one bounded seam, freeze a successor limited to
`app/slug_loading_v2/src/bzl_module.rs` and
`app/slug_loading_v2/src/package.rs`, with colocated tests and initial caps of
420 production, 700 tests, and 1,120 total lines. Require paired positive and
negative pure rows for every matrix/default/error/order branch and every named
fail-closed family; real
DICE raw-error/Need-before-load, definition error/Need, absence/multiple order,
label-mapping/default/tag/order/dev/location A/B/A, retained-error-context
A/B/A, cold/warm reuse, and events. A raw terminal performs zero Host-bzl
observation; definition loading after successful raw input may publish only its
accepted loader events; composition publishes none. Require full Bzlmod/loading
suites, format/diff/scope/forbidden-edge/cleanup audits, and independent review.

No Rust, Cargo/BUILD, fixture, schema widening, callable/heap handle,
`module_ctx`, execution, I/O, generated-repository, lockfile, materializer,
consumer, JVM/Java, or source-owner change is authorized now. `REPLAN` on a
second loader/evaluator, Bzlmod mutation, public generic API, a third future
Rust file, unbounded attribute coercion, unresolved error order, or cap excess.

## Accepted predecessor evidence

The loading definition owner accepted in `bf2c36e9` retains complete request,
manifest, schema, factor declaration, and error identity while the callable
remains frozen-lifetime-only. The selected raw-input r2 implementation is
accepted at 263 production, 304 test, and 567 total lines against `a31cf3d9`;
it retains complete request/error context, exact root identity and source-order
raw tags, excludes unrelated graph/files/lockfile state, passes the full
Bzlmod/loading suites, and has independent `ACCEPT` review.
