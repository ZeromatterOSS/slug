# Current Slug V2 Packet

Packet: `WP-4-5-host-pure-module-extension-invocation-owner-design`
Milestone: M7 bounded module-extension invocation ownership design
Owners: `slug-v2-subplans/04-starlark-loading-and-build-packages.md` and
`slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: freeze the smallest Rust-native invocation leaf or `REPLAN` at its
first missing prerequisite.

## Active design contract

Perform a read-only ownership audit for one callerless loading-owned DICE leaf
that computes `HostPreparedModuleExtensionInputsKey`, reacquires each exact
request through the sole `HostBzlModuleEvalKey`, verifies manifest/export/
definition identity, creates ephemeral read-only module/tag/context Starlark
values, invokes the lifetime-owned callable, and accepts only a `None` result.
The retained result must be heap-independent and include the complete prepared
predecessor, exact request/manifest/definition factor identity, invocation
outcome, and complete typed success/error context.
Never retain a `FrozenValue`, heap, callable, or runtime context in DICE.

Admit only the root-main singleton ordinary nonisolated input already prepared
by the accepted scalar owner, definitions with `environ = []`,
`os_dependent = false`, `arch_dependent = false`, and `facts_version = 0`, a
read-only `ctx.modules`, no repository-rule calls, and an implementation that
returns exactly `None`. Freeze preparation-before-load, request-order
reacquisition, module/tag/attribute/dev/location visibility, callable error and
print event ownership, strict result validation, and complete-only equality/
validity. A prepared terminal or Need must perform zero reacquisition work.

Pinned Bazel 9.2 commit `8220c619` anchors the admitted ABI in
`ModuleExtensionContext`, `StarlarkBazelModule`, and `TypeCheckedTag`. The
ephemeral `module_ctx` exposes exactly `modules`, `is_dev_dependency(tag)`, and
`tag_sort_key(tag)`. `modules` is an immutable one-element root-BFS list.
`is_dev_dependency` reads the prepared tag bit; `tag_sort_key` returns an
immutable opaque value ordered by `(module_index, tag_index)`. `facts`,
`is_isolated`, `root_module_has_non_dev_dependency`, `extension_metadata`, and
all inherited external-context members are unsupported in this slice. In
particular `wait`, `download`, `download_and_extract`, `extract`, `file`,
`getenv`, `path`, `read`, `watch`, `report_progress`, `os`, `execute`,
`load_wasm`, `execute_wasm`, and `which` are absent and access fails before any
side effect. The shared `.bzl` globals continue to omit `repository_rule` and
repository-rule callables; require a negative probe for every forbidden
context/global name, including a callable captured through a load.

The immutable root `bazel_module` exposes exactly `name: string`, normalized
`version: string` (including the empty sentinel), `is_root: bool = true`, and
`tags`. `tags` has one field for every declared tag class, including an empty
immutable list when unused; each list preserves source order. A tag is an
immutable structure with exactly the declaration-order schema fields and the
prepared String/Bool/i32/Label values. Dev-dependency is not a tag field and is
visible only through `ctx.is_dev_dependency`; logical location is not a field
and participates only in tag debug/error rendering. Cross-class source order
is visible only through `ctx.tag_sort_key`. Unknown tag-class and attribute
accesses fail with their typed class/attribute distinction; mutation of the
context, module, tags container, tag lists, or tag values fails closed.

An admitted Label is an immutable canonical-label Starlark value: equality,
hashing, `str` as the unambiguous canonical label, `repr` as
`Label("@@repo//pkg:target")`, and the pure `name`, `package`, `repo_name`,
deprecated `workspace_name`, and `same_package_label(target_name)` surface are
exact. `workspace_root`, deprecated mapping-sensitive `relative`, construction
through a global `Label`, and every filesystem/target lookup are unsupported;
probe each. The main repository uses `@@//...` and an empty repo name. No label
operation may observe a package, target, route, or filesystem.

Invocation owns a fresh local event capture. Loader events remain solely the
existing Host-bzl key's evaluation data. Successful or failed invocation
publishes its complete print prefix (including print-before-throw ordering) as
the invocation key's evaluation data and replays that batch on warm reuse. The
heap-independent semantic receipt retains only complete structural inputs and
the typed invocation outcome; event content is not semantic equality and no
event identity is stored in that receipt. Preparation and reacquisition
terminals publish no invocation events.

Exact compatibility is limited to that Bazel 9.2 slice: preparation/load/
invocation order, root module and tag identity/order, admitted scalar values,
dev/location state, callable failures, and strict `None`. Private Rust wrapper
layout, diagnostic wording, event carrier, and nonobservable internal
scheduling are Slug-native. Deferred are nonroot/MVO/isolation/innate inputs,
environment/OS/arch/facts observation, extension metadata, repository-rule
proxies/calls, generated names/RepoSpecs/existence, override/inject final
validation, lockfile replay/write, filesystem/network/download/execute work,
materialization, commands/consumers, and exact JVM identity bytes.

This docs-only packet may edit exactly canonical, this manifest, Stage 4, and
Stage 5. Cap growth at 45 canonical, 260 manifest, 240 Stage 4, 220 Stage 5,
and 765 total lines. Require pinned Bazel 9.2 source/test anchors; live owner,
visibility, callable-lifetime, event, and representation audits; an explicit
future allowlist/caps/proof/stops; and independent design review.

A credible future implementation may use only
`app/slug_loading_v2/src/bzl_module.rs`,
`app/slug_loading_v2/src/package.rs`, one new private
`app/slug_loading_v2/src/module_extension.rs`, and
`app/slug_loading_v2/src/lib.rs` solely for `mod module_extension;`, initially
capped at 520 production, 800 tests, and 1,320 total lines. Require prepared
Need/error with zero bzl activation; exact ordered reacquisition; missing,
private, wrong-kind, manifest, export, and definition drift; unsupported factor
preflight; callable-visible module/tag/attribute/dev/location order; empty and
multiple tags; `None`, wrong-result, throw, and print rows; contextual error
A/B/A; source/callable/manifest/prepared-tag A/B/A; cold/warm reuse; Need
invalidity; structural absence of retained heaps/callables and repository-rule
globals; field-by-field ABI positives plus every forbidden-name probe; cold/
warm print replay and throw-with-prior-print order; full loading/Bzlmod direct-
dependent suites; cleanup and independent review.

`REPLAN` on any environment/OS/facts observation, repository-rule global or
call, generated output or metadata, I/O, retained Starlark heap/value/callable,
second loader/evaluator, Bzlmod mutation or reverse dependency, public generic
API, need for broader attribute containers, result other than strict `None`, a
fourth Rust file beyond the three semantic files plus private `lib.rs`
declaration, cap excess, or inability to make the invocation receipt fully
heap-independent. No Rust or fixture is authorized before independent design
acceptance and explicit implementation activation.

## Accepted composition implementation evidence

This section is historical evidence and grants no file, action, cap, or
schedule authority. Independent review accepts the implementation at 414
production, 529 test, and 943 total formatted net lines against `aee502ff`.
The callerless owner computes raw inputs first, borrows the sole definition
loader, performs the exact supplied-map then schema-order scalar coercion and
label visibility checks, retains every predecessor/error context, publishes no
events itself, and keeps callables, contexts, execution, I/O, and generated
repositories absent. Focused and full loading tests, full prior Bzlmod tests,
format/diff/scope/cleanup checks, and two independent reviews pass.

## Predecessor design record

This section is historical context only and grants no files, actions, caps, or
scheduling authority.

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

The completed docs-only packet edited exactly canonical, this manifest,
`04-starlark-loading-and-build-packages.md`, and
`05-bzlmod-and-repository-graph.md`. Cap net growth at 45 canonical, 240
manifest, 220 Stage 4, 220 Stage 5, and 725 total lines. Require pinned Bazel
9.2 source/test anchors, live visibility and error-order audit, exact versus
Slug-native/deferred classification, compact-utility review, explicit future
allowlist/caps/proof/stops, and independent design review.

The active successor is limited to
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

No other Rust, Cargo/BUILD, fixture, schema widening, callable/heap handle,
`module_ctx`, execution, I/O, generated-repository, lockfile, materializer,
consumer, JVM/Java, or source-owner change is authorized. `REPLAN` on a
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
