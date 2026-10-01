# Current Slug V2 Work Packet

Packet: WP-7-183-m7a-generated-owner-cycle-contract-r1
Status: accepted design; live-context API prerequisite selected; M7A remains partial

## Outcome

At clean accepted baseline09aaf477f, resolve an implementable Rust-native DICE
cycle/owner contract for module_ctx.path of the proved generated host-tools Label
@@rules_rust++rust_host_tools+rust_host_tools//:rust_host_tools. Deliver a reviewed
choice of graph coverage, notification/error integration, attempt lifecycle and
production transaction sharing, with exact allowlist and discriminating gates
for the implementation successor. This is a design packet because those choices
are unresolved, not an automatic split for a boundary category.

## Ownership and compatibility

Reuse public DICE UserCycleDetector/Guard start, immediate add_edge, cycle_guard
and lifecycle; no DICE engine fork, global owner registry, stack-only recursion
check or lock across compute. Read docs/developers/dice.md and the local utility
reuse skill. Structural full keys/mode must explain graph node equality; no
canonical-name-only identity. New graph is request/runtime plumbing, never a
semantic cache or proof that content is unchanged. Memory/notification ownership
must retire exact computation attempts on cancellation/Need/finish and release
request tasks/graphs; preserve compact SmallMap/SmallSet/Arc utility use.

Existing isolated .bzl cycle handling, observed-input epochs, event publication,
package-marker validation and full-request materialization remain protected.
A consumer must observe completed producer certificates and real immutable
roots, never partial extension results or discovered physical roots. Same-owner
path remains fail-closed before evaluation; unequal owners alone admit nothing.
Bazel9.2 pinned at8220c6198837d5c13d53fea211cf3282aa12408a is the source authority:
ModuleExtensionResolutionTest1474–1549/1603–1634 covers independent reads,
reciprocal/self cycles; RepoDefinition/SingleExtension/RegularRunnableExtension
publish only completed extension results. Label paths require actual package
lookup/repository directories. Production stays Rust-native; Bazel external only.

This packet admits no behavior. Current exact source/package slices remain
exact; graph/error integration is Slug-native architecture for later exact
cycle/success behavior gates. Generated path admission, complete configuration,
bootstrap and deeper graph memory remain unsupported/deferred until implemented.

## Decisions to resolve

1. Enumerate the authentic pure-owner→route→definition→certificate→producer
chain and every intermediate generated-source/.bzl key that can close a mixed
cycle. Immediate DICE edges do not cross unguarded bridges. Choose a bounded
complete detector graph/typed-guard composition, preserving existing .bzl
projection diagnostics without mixed-family panic. Do not admit a closure with
an untracked return path or silently restrict it to get an easy passing test.
2. Define how guarded waits return source-owned cycle terminals with current
observations/diagnostic prefix and no published partial effects. Include warm
checking_deps edges before Key::compute. Decide exact-attempt RAII retirement
versus callbacks and notification ownership; no sticky cycle reuse across Need.
3. Inspect actual Core/daemon request serialization and DICE user-data sharing.
Prove supported concurrent-worker and cross-transaction lifetime rather than
assuming per-request detectors observe each other's shared computations. Resolve
any conflict with existing supported runtime behavior in the same design.

## Scope and evidence

Root owns this manifest/canonical scheduling and commits. Three independent
read-only roles: complete key/producer closure; DICE lifecycle/runtime sharing;
Bazel/native-cycle review and accepted-decision gate. Optional compact private
receipts target/wp883. No production/test/owner-plan edit until a reviewed
implementation packet is ready. No Cargo/Slug/Bazel command, archive extraction,
download, RC inspection or unchanged full-root replay. Inspect current source,
retained evidence and exact upstream tests only. Do not copy donor runtimes.

Before acceptance give exact function/type anchors, chosen module/API/data
lifetime, exclusions justified by coverage and bounded implementation allowlist.
Name focused gates: authentic independent cold Need/replay and immutable root,
same-owner rejection, two/three-owner and mixed .bzl cycles, warm introduce/remove
cycle, shared producer concurrency, cancellation/Need/retry, mode/identity and
no-partial publication; existing .bzl cycle regressions and direct Core/CLI
compile dependents. Compile60s/runtime20s gates are successor operation ceilings;
no runtime in this design. Memory growth requires a stated scoped bound and
lifecycle proof, with supervision for any later production replay. Run plan-status
and diff checks. Independent FINAL review accepts only an actionable complete
contract; if public DICE cannot supply required lifecycle/edges, name smallest
owner/API prerequisite instead of an unsafe bypass. Completion permits immediate
semantic implementation under existing goal authorization with DESIGN checkpoint.

## Immediate predecessor

WP882 accepted at09aaf477f proves the exact host-tools Label at metadata preflight;
three focused gates/default CLI build pass, rebuilt root exit2 at10.257s,
301.76MiB high-water, zero sampled swap, unchanged inputs/CLI and owned cleanup.
WP880 metadata metrics memory fix remains accepted. Task-local stacks and
preflight-only wait graphs are rejected: independently shared workers and warm
historical-dependency checks respectively bypass them. Helper line877 source
provenance remains inference; actual Label/site is direct runtime evidence.

## Resolved contract and implementation prerequisite

The selected graph uses exhaustive typed structural nodes for19 concrete key
types (seven ordinary/innate owner, certificate, definition, canonical route,
load-route and file-effect legacy/observed pairs; Host/External .bzl pairs and
simple BzlModuleEvalKey). target/wp883/key-closure.json proves the returning-key
closure;12 aggregate/root-entry types are prefix-only and package/materialization/
source observations are sinks with no Loading callback. Display is diagnostic
only, never node identity. Preserve isolated .bzl projections via embedded bridge
fields in their existing concrete typed guards; composite wrappers fail DICE's
actual guard downcast. Mixed notifications must be consumed by ExternalBzl
canonical-child/effective-route/child waits as distinct mixed errors, plus ordinary
owner definition/path, innate definition/route and file-effect certificate/
definition/Label waits. A fresh .bzl compute may be the only compute in a warm
mixed cycle; forwarding alone is insufficient. Mixed errors never use old pure
family projection constructors. Existing transient poison dependency prevents
cached cycles after raced-away child dependencies; preserve current epochs and
unpublished invocation prefixes when translating terminals.

Choose one synchronous compact graph/coordinator per actual live DICE context.
No actor backlog, process registry or async lock across compute. Guards own exact
attempt IDs/notification receivers; callbacks and idempotent token Drop retire
attempts and incident edges. Same-key replacement waits prior termination within
one live cache. Clone a receiver for each guarded wait, notify outside synchronous
lock, and retain no finished/sticky graph entries. Request-local userdata effects,
environment and trackers remain separate. Weak coordinator registry belongs to
one retained WorkspaceRuntime/DICE; prune expired weak entries. This is
nonsemantic runtime plumbing and releases with transactions/active workers.

Public VersionNumber is insufficient: last transaction removes its live cache
and reopening the SAME semantic version creates another private VersionEpoch,
while old cancellation workers can retain userdata. Root's version-only proposal
is rejected. The necessary smallest prerequisite is a public opaque
DiceTransaction live-context token derived from EXISTING engine identity,
VersionNumber and VersionEpoch, with equality/hash and no semantic/durable use.
Forward through transaction.rs/BaseComputeCtx and read existing CoreStateHandle
engine ID; no scheduler, cache, transaction, key equality or engine allocation
change. Keep fields opaque and explicitly separate from DiceEquality/version.
Include engine identity so foreign retained DICE instances cannot alias. This
read-only adapter is not a semantic engine fork.

Then install a deferred cycle-detector proxy in userdata before commit and bind
it immediately AFTER commit to the exact live-context registry entry BEFORE
compute. Commit creates contexts/injects values without semantic Key::compute or
detector start; same-context workers share one graph, revived/different contexts
remain isolated. Preserve legacy concurrent request admission. Equality-only
existing_state on a selected updater must remain unbound: it later commits new
inputs with the same userdata. Actual compute-capable constructors need fresh
proxy binding; never recommit or migrate a fixed proxy across contexts. The
retained WorkspaceRuntime owns its Tokio runtime; any reused asynchronous
coordinator must retain that owner or the new synchronous graph must avoid a
request-local task lifetime. Native synchronous attempt/abort cleanup additionally
drains pending cancellation after transaction drop before retry/accept/release,
but that drain is not the sharing identity.

Implementation sequence: first land the opaque live-context adapter with focused
DICE tests for simultaneous/clone sharing, committed version separation, engine
separation, and idle same-version revival WHILE prior cancellation remains live.
Then implement exact-context proxy/registry plumbing and the complete mixed graph/
source-owned path admission atomically with the named lifecycle/cycle gates; no
partial generated-owner activation. Prerequisite allowlist dice/dice/src/api/
transaction.rs, transaction.rs, impls/ctx.rs, impls/core/state.rs, lib.rs, a
cohesive api/live_context module/tests, and Hash derive for existing VersionEpoch.
Runtime successor allowlist cohesive Core cycle-plumbing module; runtime/dice.rs
and request_revision.rs; new cohesive Loading graph module, factory/typed bridge
fields and the audited ordinary/innate/effect/external-bzl guard sites. Direct
Loading/Analysis/Core/CLI compile dependents and existing concurrency/poison
regressions protect the public adapter and integration.

Pinned Bazel mixed discriminator ModuleExtensionResolutionTest1554–1599 includes
extension repo reads plus generated defining .bzl loads;1603–1635 self-generated
source;1474–1549 independent reads/reciprocal cycles. Warm gates must show which
fresh participant consumes a notification while others check historical deps.
Any cycle wholly from historical valid graphs would contradict acyclic-valid/
poison invariants and requires prerequisite correction, not assumed notification
success. API token is plumbing only: existing semantic equality and namespace
requests remain structural; full root/bootstrap remain unproved.

Independent FINAL design ACCEPT: source-level sharing/mixed-cycle contract only;
opaque-token lifecycle, warm notification, cancellation and immutable-root
completion remain implementation gates. No production/runtime changed.
