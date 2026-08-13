# Current Slug V2 Packet

Packet: `WP-4-5-host-module-extension-repository-rule-instantiation-owner-design`
Milestone: M7 repository-rule instantiation projection design
Owners: `slug-v2-subplans/04-starlark-loading-and-build-packages.md` and
`slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: design the smallest loading-owned heap-free composition of accepted raw
repository-rule calls and selected extension namespaces into semantic RepoSpecs,
without executing repository implementations or performing repository I/O.

## Active docs-only design contract

Run only
`WP-4-5-host-module-extension-repository-rule-instantiation-owner-design`
in canonical/current/Stage 4/Stage 5. The exact docs allowlist and caps are 45
canonical, 260 current, 240 Stage 4, 220 Stage 5, and 765 total changed lines.
Authorize no Rust, Cargo, fixture, implementation activation, Bzlmod mutation,
repository implementation/context, I/O, materializer, lockfile, consumer,
public API, or JVM work before independent design acceptance.

Accepted `b7c70a1b` owns ordered raw calls, exact definition/schema identity,
generated apparent names, kwargs, and caller provenance in the sole loading
invocation receipt. Accepted `c7c55b17` exposes, through the same exact hidden
load request, the selected unique prefix, root pre-substitution mapping, final
mapping, and ordered override/inject metadata. This closes the namespace
prerequisite but owns no call-name set, RepoSpec, schema application, or
generated existence verdict.

Pinned Bazel 9.2 `ModuleExtensionEvalStarlarkThreadContext.createRepos`,
`RepoRule.instantiate`, `AttributeUtils.typeCheckAttrValues`, and
`SingleExtensionFunction` freeze one loading-owned callerless DICE projection
that computes the complete raw invocation owner first, joins each receipt to
the exact selected request, constructs the full generated-repository mapping
for every call before schema work, applies ordered root substitutions only
after base entries and generated names, then instantiates admitted scalar
attributes atomically in extension/call encounter order. The semantic result
must be heap-independent and retain complete predecessors, exact request and
definition identity, generated apparent/canonical names, caller provenance,
and the existing Bzlmod `RepoRuleId`/`RepoSpec` algebra without running the
repository implementation.

The exact namespace algorithm is request/receipt encounter order; start from
the request's root base mapping; add every call name as
`unique_prefix + "+" + generated_name`; apply the request's ordered override/
inject substitutions last with keep-last semantics; and only then instantiate
calls in encounter order. The exact request object embedded in each invocation
receipt must equal the same-index prepared input request; count/order or full
equality mismatch is terminal. No label/export-only join is permitted. The
request's final mapping remains structural predecessor identity and may be used
as an integrity discriminator, but namespace assembly never derives from it.

Pinned `RepoRule.instantiate` removes `name`, `tags`, `deprecation`, and
`visibility` before type checking and never stores them. In supplied raw
kwargs order, ignore those four and `None`; otherwise fail first unknown
attribute, scalar conversion, or Label conversion/allowed-value error. Then in
definition declaration order fail the first missing mandatory attribute,
select declared or intrinsic defaults for validation, and visibility-check the
final value. Defaults are not re-resolved and omitted/defaulted values are not
stored. The resulting `RepoSpec.attributes` contains only explicitly supplied
non-None, nonlegacy user attributes in raw kwargs order after conversion; no
built-in field is present. `name` is retained separately as the generated
apparent name. `RepoRuleId` is the definition's canonical bzl label plus
exported rule name.

For Label attrs, a supplied String resolves relative to the defining bzl
label's canonical package using the complete generated-then-substituted
mapping. A captured canonical Label is not rebound. A declared canonical Label
default is not reparsed or rebound, but must be visible from the defining
context or among the complete mapping's canonical values. Admit only the
already captured root-main ordinary nonisolated empty-factor String/Bool/i32/
Label slice. Every container/output/big-int/function/context/tag/cycle,
configurable/transition, allowed-values, file/provider/executable restriction,
repository implementation factor, or unmodeled descriptor fails closed.

The private owner is
`HostInstantiatedModuleExtensionRepositoriesKey { workspace }` returning a
complete-only
`HostInstantiatedModuleExtensionRepositories` or typed
`HostInstantiatedModuleExtensionRepositoriesError` through the existing
`SourcePreparationOutcome`. Success retains the complete raw invocation
predecessor plus extension/call-ordered immutable rows containing exact request,
raw call/provenance, generated apparent/canonical name, and `RepoSpec`.
Namespace/join failures precede all schema work. A failing call publishes no
partial row; its terminal retains the complete predecessor, all completed
extensions, the current extension's completed rows, and the exact failing raw
call/request/definition/mapping context. An upstream invocation Need remains
invalid and an upstream completed error yields no instantiation rows.

Pinned `createRepos` applies override/inject substitutions without checking
`must_exist`. Pinned `SingleExtensionFunction` validates override-missing and
inject-collision only after the eval-only generated RepoSpecs exist. Therefore
this packet retains ordered `must_exist` structurally but performs no existence
verdict; that later validator and final generated routes/mappings remain
deferred.

The future implementation is exactly existing
`app/slug_loading_v2/src/module_extension_repository_rule.rs`, one new private
`module_extension_repository_instantiation.rs`, and `lib.rs` solely for its
private declaration. Caps are 480 production, 700 tests, and 1,180 total
formatted net Rust lines against the accepted design commit; no fourth Rust
file or Bzlmod production edit. Require pure empty/one/multiple-call namespace,
collision-prefix, cross-call visibility, substitution/`must_exist`, exact
stored field/order, two-phase coercion/default/visibility, and atomic-prefix
tables; real-key predecessor Need/error,
mapping/schema/default/name/value/order/provenance A/B/A, cold/warm reuse,
zero events and zero I/O; full loading/Bzlmod dependents; and structural
absence of Heap/Value/FrozenValue/callable/context from retained state.

Exact compatibility is limited to the admitted pinned `createRepos` namespace
assembly and `RepoRule.instantiate` semantic projection. Private Rust
representation, diagnostics, and DICE scheduling are Slug-native. Repository
implementation/`repository_ctx`, environment/OS/facts, filesystem/network/
watch/download/execute, materialization, final generated routes, override/
inject existence validation/publication, lockfile, nonroot/MVO/isolation/
innate breadth, commands, public APIs, and JVM remain deferred. `REPLAN` on
Bzlmod mutation or
reverse dependency, reconstructing selected namespace state, a second loader,
retained Starlark lifetime, order-insensitive attributes, guessed built-ins,
repository execution/I/O, generated existence/final-route claims, a fourth
future Rust path, or cap excess.

## Accepted docs-only design contract

This section and everything below are historical accepted design context only,
grant no independent file, action, cap, or schedule authority, and are
interpreted solely through the active docs-only design contract above.

The definition-owner audit below found no truthful standalone definition DICE
leaf. Pinned Bazel 9.2 `repository_rule()` creates an immutable exported
Starlark callable, and its first semantic consumer is
`ModuleExtensionEvalStarlarkThreadContext.lazilyCreateRepo`; separating a
definition projection would duplicate the sole loader or retain a callable
without its natural invocation-local owner. Run only
`WP-4-5-host-module-extension-repository-rule-call-protocol-design` in
canonical/current/Stage 4/Stage 5 under 45/260/240/180/725 documentation lines.
Authorize no Rust, Cargo, fixture, generated output, or activation before
independent acceptance.

Freeze one root-main, ordinary, nonisolated, empty-factor protocol that reuses
the shared `.bzl` globals, `HostBzlModuleEvalKey`, and accepted pure-invocation
preflight. Audit the exact global parameters and construction order; export
binding and defining-label identity; lifetime-only implementation callable;
an ephemeral per-invocation sink/token; exact positional/context/export/name/
duplicate/deep-clone first-error order; and source-order capture of
heap-independent raw call records. Determine a complete first attr/option
surface rather than guessing breadth. Schema type/default/visibility
application belongs to later `RepoRule.instantiate` and must not be moved into
the capture phase.

The design must freeze an implementation allowlist no broader than
`app/slug_loading_v2/src/package.rs`, existing private
`module_extension.rs`, one new private
`module_extension_repository_rule.rs`, and `lib.rs` solely for its private
declaration, with explicit production/test/total caps. Require definition,
export/private, context ownership, one/two-call order, duplicate name,
call-before-throw prefix, all-request Need/error zero-call, definition/source/
mapping/name/raw-attr A/B/A, complete-only heap-free equality, event ownership,
cold/warm reuse, full loading/Bzlmod dependents, and structural absence of a
second loader, retained Starlark lifetime value, RepoSpec, repository
implementation execution, or I/O.

Compatibility is exact only for the admitted Bazel 9.2 definition/export and
capture surface. Private Rust representation, compact call records,
diagnostic framing, and DICE scheduling are Slug-native. `RepoRule.instantiate`,
`repository_ctx`, schema coercion/default insertion, environment/filesystem/
network/watch/download/execute, generated canonical names/RepoSpecs,
materialization, override/inject existence, final mappings, lockfile,
nonroot/MVO/isolation/innate breadth, commands, public API, and JVM are
unsupported/deferred. `REPLAN` on any RepoSpec/existence inference, repository
implementation execution, second loader/evaluator/graph, retained heap or
callable, public/wire surface, Bzlmod mutation/reverse dependency, repository
I/O, fifth Rust path, unresolved pinned order, unbounded attr/options, or cap
excess.

### Completed owner audit and frozen implementation successor

The smallest cohesive owner is the existing callerless
`HostPureModuleExtensionInvocationsKey`, extended in place. It already computes
prepared inputs, reacquires every exact frozen Host module, validates all
requests before invoking any callable, owns invocation events, and publishes a
heap-free receipt. Add no DICE key. During each extension invocation, install
one evaluation-local `RepositoryRuleInvocationState` in `Evaluator.extra`.
The state itself is the ephemeral invocation capability and owns a mutable
scratch vector but no lock and no DICE computation; after the callable returns
or throws, project it
once into ordered immutable records carried by that extension's receipt or
terminal prefix. Every preflight Need/error still performs zero invocation and
zero capture.

The shared `repository_rule` global is available only while the sole Host bzl
loader supplies `BzlEvaluationContext`. Its exact admitted signature is
`implementation` (positional or named callable) plus optional `attrs=None`;
`local` and `configure` must remain false, `environ` empty, and `doc` None.
Pinned-default semantics do not expose flag-gated `remotable`. Callable arity
is not checked at definition time. `attrs` may contain source-ordered public
ASCII identifier names and only `attr.string`, `attr.bool`, `attr.int`, or
`attr.label` descriptors with no explicit configurable policy, transition,
or file restriction and with kind-correct scalar/None defaults. Reject the
legacy built-in names `name`, `tags`, `deprecation`, and `visibility` in
source order. Private attrs, containers, computed/late defaults, values/
providers/executable/cfg/aspects/file restrictions, every other kind, nonempty
environment, true local/configure, non-None doc, and remote-exec semantics are
deferred. These rejections are fail-closed Slug terminals, not Bazel parity
claims.

`RepositoryRuleDefinitionGen<V>` retains the lifetime callable, canonical
defining bzl label, source-ordered admitted schema, and optional exported name.
Assignment uses `export_as`; anonymous values may freeze but fail only when
called. A leading-underscore top-level rule remains internally exported and
callable by its extension; the pinned `use_repo_rule` private-lookup rejection
is external lookup behavior and remains deferred. Definition/export location
is not retained because Bazel's `RepoRule` does not use it. The existing
prepared predecessor already structurally owns the selected request/mapping,
complete transitive `BzlLoadManifest`, source bytes, and extension definition,
so the call record retains the compact repository-rule projection rather than
duplicating a digest or frozen module.

For an admitted call, preserve this order: Starlark argument formation;
unexpected positional rejection; invocation-context lookup; exported-name
check; ordered kwargs extraction; `name` default-to-None/type check; exact
user-provided repository-name validation
`[A-Za-z0-9][A-Za-z0-9_.-]*`; duplicate-name check within that extension;
caller location and call-stack projection with the repository-rule native
frame removed; then recursive-value projection and atomic append. The first
slice admits `None`, bool, signed i32, valid-Unicode string, and the accepted
canonical `InvocationLabel`; list, tuple, dict, big integer, function, context,
tag, cycle, and every other value fail before append. Unknown or undeclared
scalar kwargs are captured rather than schema-checked. Retain `name` both as
the lookup key and in the ordered kwargs because later instantiation consumes
the original map.

Each `RepositoryRuleCallRecord` retains the projected definition, generated
name, ordered `Arc<[(CompactString, RepositoryRuleCallValue)]>`, one
heap-independent caller span, and ordered compact stack frames/spans.
`Arc<[RepositoryRuleCallRecord]>` preserves source/call order. Do not use
`SmallMap` equality for retained kwargs: it intentionally ignores insertion
order, while Bazel's later supplied-attribute first-error order observes it.
Use `CompactString`, typed `CanonicalLabel`, immutable `Arc` slices, cheap
clones, and `Allocative`; scratch `Vec`/linear duplicate lookup is bounded to
one invocation and is not retained. No interner, hash cache, global registry,
or strong/content digest is warranted.

An extension success still must return strict `None`; its receipt gains its
ordered call slice. An invocation error retains the complete prepared
predecessor, exact request, all earlier-extension receipts, and the current
extension's captured prefix before its typed failure. Calls emit no events;
existing loader batches remain loader-owned and prints/throws remain fresh
invocation evaluation data outside equality. Complete structural values/errors
participate in DICE equality and cutoff; Need remains invalid/non-self-equal.
Warm reuse does not rerun callables or republish batches.

After independent design acceptance, freeze
`WP-4-5-host-module-extension-repository-rule-call-protocol-implementation`
against the accepted design commit. It may edit exactly:

- `app/slug_loading_v2/src/package.rs` for narrow descriptor projection and
  shared-global registration only;
- `app/slug_loading_v2/src/module_extension.rs` for preexisting invocation
  state/receipt/error integration and a read-only canonical Label projection;
- one new private
  `app/slug_loading_v2/src/module_extension_repository_rule.rs` for the
  definition value, schema, context/sink, raw projection, records, and focused
  tests;
- `app/slug_loading_v2/src/lib.rs` solely for the private module declaration;
  and canonical/current/Stage 4/Stage 5 bookkeeping.

Cap Rust at 650 production, 850 tests, and 1,500 total formatted net lines.
Require pure definition rows for positional/named callable, wrong callable,
anonymous/exported/private rules, exact option defaults/rejections, schema
order/defaults, legacy collisions, and every deferred descriptor family.
Require invocation rows for foreign context, positional/export/name/name
syntax, one/two call order, duplicate before unsupported value, ordered kwargs,
every scalar and deferred value, caller/stack provenance, internal private
rule calls, empty calls, strict result, call-prefix-before-throw, and separate
extension namespaces. Through the real key prove prepared/loader Need and
terminal zero-call, definition/source/manifest/mapping/schema/export/name/
value/order/location A/B/A, complete error-context A/B/A, cold/warm reuse,
fresh print/error batches, and full loading/Bzlmod direct dependents. Run
`cargo fmt --all -- --check`, archive/diff checks, and a structural scan for
retained `Heap`, `Value`, `FrozenValue`, `FrozenModule`, token, event,
`RepoSpec`, repository context, I/O, and second-key state.

The exact slice is the admitted Bazel 9.2 global construction/export, internal
callability, scalar capture, repository-name/duplicate validation, call and
kwargs order, and provenance semantics. Private type names, compact layout,
unsupported diagnostics, and DICE scheduling are Slug-native. Every deferred
surface above remains unsupported. `REPLAN` on a fifth Rust path; a new DICE
key or lock; callable/heap/event retention; schema checks during capture;
repository implementation/context, RepoSpec, generated canonical name/
existence/mapping, override/inject, I/O/materialization/lockfile/consumer/API/
JVM work; loss of ordered identity; production over 650, tests over 850, or
total over 1,500.

## Completed repository-rule definition owner audit

This section and everything below are historical context only, grant no file,
action, cap, or schedule authority, and are interpreted solely through the
active docs-only design contract above.

The audit uses Bazel 9.2 tag commit `8220c619`:
`RepositoryModuleApi.repository_rule`,
`StarlarkRepositoryModule.repositoryRule`/`StarlarkRepoRule`,
`RepoRule`, `ModuleExtensionEvalStarlarkThreadContext`, and focused
`ModuleExtensionResolutionTest` export rows. The global accepts required
callable `implementation`, `attrs=None`, `local=False`, `environ=[]`,
`configure=False`, flag-gated `remotable=False`, and `doc=None`.
Construction retains ordered descriptors, defining label, transitive bzl
digest, repository-mapping entries, options, and lifetime-only callable;
`export_as` supplies the rule name. A call rejects positional arguments,
foreign context, and unexported rules before validating `name`, duplicate
names, call location, and recursively cloned raw kwargs.

Only later `createRepos` builds the full generated-repository mapping and
invokes `RepoRule.instantiate` in insertion order to type/default attributes
and produce RepoSpecs. Slug already owns the defining source label in
`BzlEvaluationContext`, the sole frozen module and transitive manifest in
`HostBzlModuleEvalKey`, and an ephemeral invocation-context lifetime. It lacks
only the shared global/value and invocation-local capture sink. Therefore a
separate definition key would be a second owner with no semantic consumer;
the smallest truthful next design composes definition/export and capture while
stopping before instantiation.

## Accepted pure-invocation implementation evidence

This section and everything below are historical context only, grant no file,
action, cap, or schedule authority, and are interpreted solely through the
active docs-only design contract above.

Independent review accepts the event correction in `f36ec593`. Implement only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r4` in the
same four app Rust paths plus canonical/current/Stage 4/Stage 5, under
730/850/1,580 against `40def0e7`. Rename the overclaiming focused test to
publication plus semantic reuse; fresh evaluated activations publish exactly
one prefix, reused activations carry no duplicate batch, event content stays
out of equality, and command-output lineage remains deferred to its real
consumer. Preserve every other accepted semantic, proof, compatibility claim,
cleanup, and stop. No fifth Rust path or behavior expansion.

### Final implementation evidence

The formatted four-path delta measures approximately 724 production, 846
tests, and 1,570 total lines against `40def0e7`, within 730/850/1,580. The full
`slug_loading_v2 --all-targets` suite passes 92 owner tests plus every loading
integration, and `slug_bzlmod_v2 --all-targets` passes 349 owner tests plus all
integrations. The renamed event-lineage test passes and proves one evaluated
batch followed by semantic reuse with no duplicate batch. Formatting and diff
checks pass. Cleanup removed two unrelated visibility widenings; structural
review finds `FrozenModule`/`FrozenValue` only in ephemeral preflight rows and
none in the retained receipt. Independent implementation reviews accept the
architecture, ABI, errors, events, scope, caps, and all stops.

## Accepted docs-only event-contract correction

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active
implementation contract above.

Final implementation review accepts the invocation architecture but rejects
the phrase that the invocation key itself replays its batch on warm reuse.
Existing DICE ownership intentionally attaches evaluation data only to the
fresh `Evaluated` activation; a `Reused` activation has no batch. The existing
`CommandEffectOwner` later selects reachable earlier evaluated batches within
one command/retry lineage, while a fresh owner does not replay them. This
callerless invocation packet has no command consumer and must not duplicate
events in its heap-free semantic receipt or fabricate per-activation replay.

Run only `WP-4-5-host-pure-module-extension-invocation-event-contract-r4-design`
in canonical/current/Stage 4/Stage 5 under 30/140/100/80/350 docs lines. It
authorizes no Rust or commit before acceptance and explicit r4 activation.
Freeze the same four Rust paths, 730/850/1,580 caps against `40def0e7`, all
semantics/proofs/stops, and one bounded test-name/assertion correction: fresh
complete success/failure publishes exactly one invocation-owned print prefix;
warm semantic reuse yields `ActivationKind::Reused` with no duplicate batch;
command-lineage selection/replay remains owned and already proved by
`CommandEffectOwner`, and invocation command-output integration is deferred
until a real consumer exists. Rename the overclaiming `replays_prints` test to
describe publication plus semantic reuse. `REPLAN` on events in semantic
equality, duplicate warm batches, a command consumer, fifth Rust path, cap
excess, or any behavior expansion.

## Superseded r3 implementation activation

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted solely through the active docs-only
event-contract correction above.

Independent review accepts the final cap correction in `86f478c0`. Implement
only `WP-4-5-host-pure-module-extension-invocation-owner-implementation-r3` in
`bzl_module.rs`, `package.rs`, private `module_extension.rs`, and `lib.rs`
solely for its private declaration, plus canonical/current/Stage 4/Stage 5
bookkeeping. Caps are 730 production, 850 tests, and 1,580 total against
`40def0e7`. Preserve the complete r2 semantics, proof, compatibility boundary,
and stops; cleanup may not reintroduce the two unrelated visibility widenings.
No fifth Rust path, public API, second evaluator, repository/global/output/I/O/
consumer/JVM breadth, or cap excess.

## Accepted docs-only cap-correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active
implementation contract above.

The frozen 720/800/1,520 stop fired after the complete required proof was
formatted: the exact four-path diff measures approximately 724 production, 846
tests, and 1,570 total lines against `40def0e7`. The excess is the already
required immutable ABI, all-request preflight/lifetime boundary, and frozen
Need/error/drift/factor/result/event/A-B-A tests; cleanup removed two unrelated
callable-visibility widenings, and no safe 50-line mechanical reduction remains
without weakening the accepted discriminators or auditability. Retain the
fully passing Rust diff unaccepted and run only
`WP-4-5-host-pure-module-extension-invocation-owner-r3-cap-design` in
canonical/current/Stage 4/Stage 5. This correction may edit only those four
plans under 30/120/100/80/330 docs lines and authorizes no Rust or commit before
independent acceptance and explicit r3 activation.

Freeze the same four Rust paths, semantics, proof, and stops at 730 production,
850 tests, and 1,580 total lines against `40def0e7`, leaving only 6/4/10 lines
of measured contingency and no margin for a field, ABI member, owner, consumer,
or behavior family. `REPLAN` on a fifth Rust path, any semantic expansion,
production above 730, tests above 850, total above 1,580, or inability to retain
the passing complete proof.

## Superseded r2 implementation activation

This section is historical context only, grants no file, action, cap, or
schedule authority, and is interpreted solely through the active docs-only
cap-correction contract above.

The shared string prerequisite is accepted in `40def0e7`. Resume only
`WP-4-5-host-pure-module-extension-invocation-owner-implementation-r2` in
`app/slug_loading_v2/src/bzl_module.rs`, `package.rs`, private
`module_extension.rs`, and `lib.rs` solely for its private declaration, plus
canonical/current/Stage 4/Stage 5 bookkeeping. Measure 720 production, 800
tests, and 1,520 total against `40def0e7`; no fifth Rust path or cap excess.
Preserve the accepted prepared-first all-request preflight, optional Label
None, context-owned tags, immutable exact-list ABI including negative indexes,
canonical Label str versus repr, strict None, complete-only heap-free receipt,
and invocation-owned event ordering/replay. Require the full frozen ABI,
forbidden-member/callable, Need/terminal zero-invocation, drift/factor/result/
throw, A/B/A, cold/warm, formatting, full loading/Bzlmod, and structural
heap/callable-absence proof. All repository rules, generated outputs,
environment/OS/facts, I/O, consumers, public API, second loader/evaluator, JVM,
and earlier stops remain forbidden.

## Accepted string-protocol prerequisite

This section is historical accepted evidence only, grants no independent file,
action, cap, or schedule authority, and is interpreted through the active
docs-only cap-correction contract above.

Independent review accepts the scope correction recorded in `6215fe03`.
Implement only `WP-4-starlark-custom-string-protocol-implementation-r2` in the
six vendored starlark-rust files frozen below plus canonical/current/Stage 4/
Stage 5 bookkeeping. Caps remain 90 production, 220 tests, and 310 total
formatted net lines against `73b22cec`. Require the complete synthetic global
str/repr, percent, format, print, nesting, and cycle matrix. Preserve all
default-to-repr, string-fast-path, repr/hash/equality/type, single-protocol,
derive, public-API, and behavior stops. No app Rust, InvocationLabel, loading,
or DICE proof is authorized; those resume only with the invocation packet.

### Implementation evidence

The isolated six-file delta against `73b22cec` is approximately 31 production
and 76 test lines, within 90/220/310. The synthetic protocol matrix passes;
focused `starlark` string, interpolation, and format suites pass 73/73, 8/8,
and 9/9. Full `slug_loading_v2 --all-targets` (92 owner tests plus all
integrations) and `slug_bzlmod_v2 --all-targets` pass. Full vendored
`starlark --all-targets` passes 808 tests and retains 29 unrelated existing
profiler/bytecode golden failures; no string, interpolation, format, value, or
protocol test fails. Formatting and diff checks pass. Independent review
accepts the isolated six-file implementation and confirms the dirty app paths
remain unaccepted and unstaged.

## Accepted docs-only scope-correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted solely through the active
implementation contract above.

Independent implementation review found the six shared runtime files sound but
rejected the frozen eight-file Git boundary. `module_extension.rs` is a wholly
new retained invocation owner and `bzl_module.rs` carries its retained lifecycle
tests, so landing the authorized Label override/tests would also land production
that this packet explicitly keeps unaccepted. Record only the bounded correction
`WP-4-starlark-custom-string-protocol-implementation-r2-scope-design` in
canonical/current/Stage 4/Stage 5. It may edit exactly those four plans under
30/120/100/80/330 documentation lines and authorizes no Rust, Cargo, fixture, or
commit before independent acceptance and explicit r2 activation.

Freeze the r2 implementation to the first six vendored starlark-rust files in
the successor list below, keeping 90 production/220 test/310 total caps against
`73b22cec`. Require the synthetic custom value to prove default and overridden
global `str`/`repr`, `%s`/`%r`, optimized/default and general format, print,
nested-container repr, and recursive-cycle fallback. Defer InvocationLabel and
all loading/DICE proof until the invocation packet legitimately lands. Preserve
every existing protocol semantic and stop. `REPLAN` on any app Rust file,
seventh Rust file, derive edit, reduced shared-consumer proof, public API, second
protocol, changed default behavior, or cap excess.

## Superseded initial implementation activation

This section is historical only and grants no file, action, cap, or schedule
authority; it is interpreted solely through the active docs-only correction.

Independent design review accepts the protocol owner frozen in `73b22cec`.
Implement only `WP-4-starlark-custom-string-protocol-implementation` in the
exact eight Rust files named below plus canonical/current/Stage 4/Stage 5
bookkeeping. Caps are 90 production, 220 tests, and 310 total formatted net
lines against `73b22cec`. Preserve the single default-to-repr vtable protocol,
all shared consumer proofs, the exact InvocationLabel str/repr boundary, and
every stop. A ninth Rust file, derive-crate change, public Slug API, behavior
expansion, or cap excess requires `REPLAN`.

## Accepted docs-only REPLAN contract

This section is historical design context only, grants no file, action, cap,
or schedule authority, and is interpreted only through the active docs-only
scope-correction contract above.

The r2 implementation stop fired while completing the exact Label ABI proof.
The accepted slice requires `str(label)` and `%s` to render the canonical label
while `repr(label)` and `%r` render `Label("@@repo//pkg:target")`. Live
starlark-rust hardwires non-string `str` to `collect_repr` in
`values/layout/value.rs`, the standard `str` global in
`values/types/string/globals.rs`, and percent-string interpolation in
`values/types/string/interpolation.rs`. A loading-only global override would
not affect `%s` or the other shared formatting paths, while allocating Labels
as strings would destroy type, repr, equality, and attribute semantics. The
frozen exact surface is therefore not implementable in the active four Rust
paths or 720 production lines.

Retain the current four-path Rust diff as compiling but unaccepted evidence;
it grants no implementation authority. Run only the docs/evidence packet
`WP-4-starlark-custom-string-protocol-design`. Audit one backward-compatible
starlark-rust protocol in which every value has a distinct custom `str`
projection defaulting exactly to its existing `repr`, strings preserve their
unquoted fast path, and custom values may override `str` without changing
`repr`. Enumerate every standard formatting consumer that must use the same
protocol, at minimum global `str`, `Value::to_str`, `ValueLike::collect_str`,
`%s`, `str.format` default/`!s`, and print formatting; keep `%r`, global
`repr`, debug/error rendering, hashing, equality, and type identity unchanged.
Audit the generated StarlarkValue vtable/derive path rather than adding a
Slug-only downcast or a second formatter. Require existing starlark-rust
string/format suites plus a synthetic custom value proving distinct str/repr,
and a later InvocationLabel row proving global str/repr, `%s`/`%r`, format,
print, equality/hash, and warm DICE behavior.

Compatibility is exact for the admitted formatting operations and unchanged
existing values; the private hook name, vtable layout, and Rust diagnostics are
Slug-native. Java UTF-16 edge behavior and all module-extension surfaces
already deferred by the predecessor remain unsupported/deferred. This packet
may edit exactly canonical, current, Stage 4, and Stage 5 under 45/220/180/100
and 545 total documentation lines. It authorizes no Rust, Cargo, fixture,
loading, Bzlmod, evaluator, or consumer edit before independent design
acceptance and explicit successor activation. Freeze a future Rust allowlist,
caps, tests, and stop conditions; `REPLAN` if exact semantics require a
type-specific downcast, a second string formatter, public Slug API, non-Rust
runtime, or unrelated Starlark behavior changes.

### Completed owner audit and frozen successor

The smallest implementation seam is one `StarlarkValue::collect_str` method
whose default calls that value's existing `collect_repr`. The existing
`starlark_value_vtable` derive automatically owns the new function pointer; no
derive-crate edit or parallel registry is required. Add the erased dispatch in
`values/layout/vtable.rs`, override `ValueLike::collect_str` in
`values/layout/value.rs` to preserve the string fast path and repr-stack cycle
guard while dispatching non-strings, and make `Value::to_str` use that shared
operation. The standard `str` global, percent `%s`, and the optimized/default
`str.format` paths must call the same operation. Existing `%r`, `repr`,
`collect_repr_cycle`, type errors, `fail` framing, hashes, equality, and string
allocation/interning remain unchanged. `print` already composes through
`Value::to_str`; the general format `!s` path already composes through
`ValueLike::collect_str` and receives only regression coverage.

After independent acceptance, activate only
`WP-4-starlark-custom-string-protocol-implementation` in exactly:

- `starlark-rust/starlark/src/values/traits.rs`;
- `starlark-rust/starlark/src/values/layout/vtable.rs`;
- `starlark-rust/starlark/src/values/layout/value.rs`;
- `starlark-rust/starlark/src/values/types/string/globals.rs`;
- `starlark-rust/starlark/src/values/types/string/interpolation.rs`;
- `starlark-rust/starlark/src/values/types/string/dot_format.rs`;
- `app/slug_loading_v2/src/module_extension.rs`; and
- `app/slug_loading_v2/src/bzl_module.rs` for focused invocation tests only,
  plus canonical/current/Stage 4/Stage 5 bookkeeping.

Cap the successor at 90 production, 220 tests, and 310 total formatted net
lines against the eventual accepted design commit. Require the full existing
starlark test suite and synthetic values proving default str==repr, overridden
str!=repr, nested/list/cycle behavior, global str/repr, `%s`/`%r`, optimized and
general format, and print. Require InvocationLabel canonical `str`, `%s`,
format, and print alongside unchanged `Label("...")` repr/`%r`, type,
attribute, equality, hash, and DICE success/error identity; prove cold/warm
event lineage and source A/B/A. Structural scans must show one protocol, no
Label downcast outside its override, and no retained Starlark value in DICE.
Run full loading plus direct Bzlmod dependents, formatting, diff, and archive
classification before independent implementation review.

`REPLAN` on a ninth Rust file, derive-crate change, type-specific standard
formatter branch, second protocol/registry, changed default output for any
existing value, repr/hash/equality/type-error behavior change, retained heap or
callable, public Slug API, non-Rust runtime, loading/Bzlmod semantic breadth, or
90/220/310 excess. The retained pure-invocation diff remains unaccepted and
must not resume in the same packet beyond the exact InvocationLabel override
and tests needed to prove this prerequisite.

## Accepted r2 correction contract

This section is historical correction context only, grants no file, action,
cap, or schedule authority, and is interpreted only through the active
docs-only scope-correction contract above.

The first compiling implementation is 630 production lines against `db45d182`
before tests: 597 lines in the required private invocation/ABI owner and 33 in
narrow existing-owner accessors plus the private module declaration. Independent
review finds no credible 110-line mechanical reduction without collapsing
distinct Starlark values, typed error/event orchestration, or lifetime
boundaries. Retain that unaccepted four-path Rust diff unchanged while this
docs-only packet corrects the future caps to 720 production, 800 tests, and
1,520 total lines against `db45d182`. The margin is solely for the semantic
corrections below, not another owner, field family, or behavior surface.

Freeze four corrections for r2. First, optional Label attributes prepared as
`CoercedAttributeValue::None` allocate Starlark `None`; no accepted prepared
value reaches `unreachable!`. Second, reacquire and validate every exact Host
module, factor set, manifest, export, and definition into ephemeral lifetime-
only preflight rows before invoking any callable. Any Need or terminal during
preflight performs zero user invocation and publishes zero invocation events;
only a complete preflight invokes rows in encounter order. Third,
`is_dev_dependency(tag)` and `tag_sort_key(tag)` accept only tags minted by that
exact ephemeral context. Use an invocation-local nonsemantic ownership token;
reject foreign/captured tags without retaining the token in DICE equality.
Fourth, `ctx.modules` and every `module.tags.<class>` value are immutable
Starlark lists with exact list iteration/index/length behavior; mutation such
as `.append` fails closed. The module, tags structure, tag instances, Labels,
and sort keys remain immutable as already frozen.

Require omitted optional-Label `None`; two-request first-print/second-Need
zero-invocation then restoration-order; cross-context and captured-tag
rejection; and mutation negatives for both modules and tag-class lists. Keep
all original ABI positives, every forbidden-name/captured-callable probe,
strict-None/wrong-result/throw/print ordering, lifecycle/A-B-A, event replay,
heap-absence, full-suite, cleanup, and independent-review proofs.

This correction may edit exactly canonical, current, Stage 4, and Stage 5,
under 30/150/120/120/420 docs caps. It authorizes no Rust, fixture, Cargo,
BUILD, public API, or semantic implementation until independent acceptance and
explicit r2 activation. Preserve the same future four Rust paths and every
existing stop. `REPLAN` on production above 720, a fifth Rust path, retained
heap/callable/token, a second loader/evaluator, public API, repository/global/
I/O/generated-output breadth, or inability to make all preflight terminals
side-effect-free.

## Accepted design contract

This section is historical design context only, grants no file, action, cap,
or schedule authority, and is interpreted only through the active docs-only
scope-correction contract above.

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
