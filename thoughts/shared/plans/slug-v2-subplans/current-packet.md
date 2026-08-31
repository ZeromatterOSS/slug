# Current Slug V2 Packet

Packet: `WP-4-5-7A-subrule-direct-call-and-value-materialization-r3`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Status: terminal correction review `ACCEPT`; implementation and proof complete.

Base: `2bf86bfa8`, which accepts the source-ordered configured hidden Target/Exec
dependency producer on top of the generic selected-toolchain context. The
unrelated dirty registration-expansion proof remains parked. Stage and validate
only this packet's exact hunks.

## Observable result

After the accepted configured dependency batch succeeds, invoke an attached
subrule from its owning rule implementation. Materialize every admitted hidden
`label` or `label_list` argument as Bazel's call boundary requires: configured
target, ordered configured-target list, one Artifact for `allow_single_file`, or
one `FilesToRunProvider` for `executable`. Materialize ordinary admitted
`configuration_field` defaults through the same dependency view so `ctx.attr`
does not retain a second late-bound path.

Complete the direct-call category rather than recognizing only
`create_fdo_context`: preserve caller positional and named arguments, reject a
hidden-name override, enforce direct rule/subrule declarations for nested
calls, inject a restricted call-scoped context, restore its caller after success
or failure, lock the enclosing rule context while a subrule is active, and
reject escaped context/action access. Successful rule analysis
publishes the already-validated hidden rows as ordered implicit configured
edges and marks exactly Exec-transition rows as tools. Existing ordinary rule
evaluation, provider lowering and action ownership remain the sole completion
path.

The restricted context exposes the rule label and the existing generic
`declare_file`, `write`, and `run_shell` action namespace. `fragments` and
`toolchains` are present as explicit deferred capabilities, so a consumer fails
at the first missing category instead of at subrule invocation. This packet
does not claim complete FDO or rules_cc analysis.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the
sole semantic authority:

- `StarlarkSubrule.java:132-216` checks rule-versus-caller declaration,
  rejects hidden overrides, materializes executable/list/single-file/ordinary
  label values, prepends `SubruleContext`, locks the outer rule context, invokes
  the implementation, and restores state in `finally`;
- `StarlarkSubrule.java:310-446` defines the restricted label/actions/
  toolchains/fragments surface, active-context check, executable provenance,
  automatic exec-group action selection, and post-call invalidation;
- `StarlarkSubruleTest.java:90-423,451-723,801-1219,1235-1449` proves export and
  declaration checks, nested authorization, positional context injection,
  label/actions exposure, escaped-context rejection, hidden invisibility and
  override rejection, all four admitted hidden value shapes, and action/
  toolchain boundaries; and
- `Attribute.java:2113-2127` plus `ExecutionTransitionFactory.isTool()` define
  the implicit/tool classification published only after successful analysis.

The accepted predecessor already owns configuration-field projection,
Target/selected-Exec child identity, validation order, source/generated file
cardinality, provider predicates, executable availability, alias normalization,
cycle handling and revision behavior. This packet consumes those facts; it
does not recompute or revalidate them in an evaluator callback.

Buck2/starlark-rust supplies the unchanged parser, function binder, heap and
evaluation stack. Use its `Arguments`/`invoke_pos_kwargs` path rather than
parsing or rebinding Starlark calls. Compact immutable `Arc<[T]>`,
`CompactString`, `SmallMap` and `Dupe` remain preferred; add no new interner,
cache or retained copy.

Zabel is concept/optimization guidance only. Its clean direct-subrule design
supports four useful decisions: authenticate producer-owned declarations before
materialization, keep the dispatcher request/evaluator-local rather than adding
a subrule key, use sparse definition spans, and make the subrule context borrow
the enclosing action owner while its lifetime token is active. Copy no Zig
code, names, layouts, errors or behavioral claims. In particular, Bazel 9.2—not
Zabel—decides declaration, argument, lifetime and provider semantics.

## Compatibility boundary

**Exact:** direct calls only from a declaring rule or a currently active
declaring parent subrule; enclosing rule-context locking and nested caller
restoration; context as the first
implementation positional; preservation of caller positional/named arguments;
hidden arguments appended in descriptor order; hidden-name override rejection;
ordinary target, ordered target-list, single Artifact and executable
`FilesToRunProvider` argument shapes; `None`/empty-list absence; rule label;
hidden invisibility from `ctx.attr`; successful ordered implicit edges; and
`tool = true` exactly for Exec-transition hidden rows.

**Slug-native:** evaluator bridge type names and diagnostic wrapping;
tagged structural Null/configured target identity; source/generated artifact identity where Bazel
configuration/output bytes are outside the accepted compatibility surface; the
already-admitted `declare_file`/`write`/`run_shell` action subset; and internal
edge representation.

**Unsupported/deferred:** subrules attached to aspects; macro/finalizer calls;
subrule fragments and toolchain lookup; automatic exec-group action selection;
`ctx.actions.args`, `run`, `symlink`, `declare_symlink`, tree/template actions,
and `cc_common.absolute_symlink`; complete Artifact/runfiles membership and
manifest projection outside the four admitted hidden argument shapes; XML;
broader `configuration_field` fragments; exact Bazel configuration/output
bytes; and rules_cc/C++ semantics. `cc_common` and `cc_internal` remain BCR
Starlark consumers and discriminators, never Rust parser or C++ special cases.

## Category architecture and successors

Keep one analysis-evaluation capability installed in `Evaluator.extra`. It
authorizes the existing analysis-only provider constructors and contains an
optional phase-scratch subrule dispatcher. Loading-time evaluators install no
such capability, so frozen rule/subrule values continue to fail closed outside
configured analysis. Do not add a process-global builtin registry or replace
starlark-rust's parser, binder, `set`, provider, depset or collection semantics.

Because the frozen callable's `StarlarkValue::invoke` necessarily lives in
`slug_loading_v2`, put the smallest shared evaluator ABI leaf in the new
`subrule_invocation.rs`: lifetime-aware analysis capability, call state/tokens,
restricted context, and the common Starlark Artifact/action facades. Move the
existing Artifact/action facade implementation out of `slug_analysis_v2`
instead of duplicating it. The leaf may depend only on existing build-API and
identity types; it owns no graph lookup, configured result, DICE handle or
retained provider data. Analysis constructs it from prepared values and the
one enclosing `CtxActions` owner. This avoids an upward crate callback, raw
pointer, new crate, or second action implementation.

The dispatcher is the permanent direct-call seam. It consumes producer-owned
subrule identities/callables plus evaluator-local prepared values and exposes
typed context fields. Later category packets extend only those fields or the
shared action namespace:

1. fragment projection supplies declared typed fragment values to rule and
   subrule contexts from the structural configuration owner;
2. the action family supplies `args`, `run`, `symlink` and related artifact/
   input provenance through `CtxActions`; and
3. `cc_common.absolute_symlink` becomes one ordinary BCR-loaded builtin call
   into the admitted symlink action capability, not a parser or rule-class
   branch.

Those successors must reuse the same call state, action owner, artifact values,
provider materializer and result lowering. They may not create a second
evaluator, re-run the rule implementation, retain evaluator values in DICE, or
introduce a C++-specific configured dependency path. This packet leaves a
deterministic missing-fragment or missing-action-family terminal for the first
authentic rules_cc consumer.

## Natural owners and call flow

`StarlarkRuleImplementation` remains the loaded, DICE-retained owner of direct
and transitive subrule identities, sparse hidden spans, callables, typed
dependency descriptors and the original rule implementation. Keep its current
semantic equality independent of frozen pointer addresses. A frozen rule
implementation wrapper remains the fail-closed entry: it delegates to the
original implementation only when the configured-analysis capability is
installed after all predecessor preparation and validation.

The moved Artifact/action facades remain evaluator ABI, not loading semantics:
their only mutable reference is the existing `Arc<Mutex<CtxActions>>`; their
artifact owner is the existing `AnalysisConfiguredTargetKey`; and their call
token is phase scratch. Both the outer rule context and restricted subrule
contexts allocate these same facade types with different active tokens. No
method body, artifact identity or action registry is duplicated.

`finish_analysis` converts validated child results into two phase-scratch
views in one source pass. Ordinary rows enter the existing `PreparedDependency`
view and update the corresponding resolved late-bound attribute to the selected
label/list shape. Hidden rows are grouped by their retained definition span and
materialized according to descriptor policy. Preserve requested descriptor
order while using the child's normalized/actual provider result. Do not perform
graph lookup, provider validation or artifact discovery inside Starlark
invocation.

`slug_build_api_v2::ConfiguredTargetValue` uses one compact
`AnalysisTargetIdentity` enum: `Configured(AnalysisConfiguredTargetKey)` or
`Null(Arc<CanonicalLabel>)`. Existing configured callers convert without
behavior change; direct source files use the Null branch. Both payloads are
pointer-sized and cheap to `Dupe`; the enum must remain at most two pointer
words. The discriminant participates
in ordinary equality, hashing, publication equality and nested
`AnalysisValue` identity, so `Null(Arc(label))` can never collide with
`Configured(key)` even if a configured byte slice is empty. Both branches expose
their canonical label; only the configured branch exposes configuration bytes.
Do not synthesize bytes, erase the tag during lowering, or infer the branch from
provider contents. The enum is the natural retained identity for every
configured-target-like analysis value; it is not subrule metadata.

The evaluator-local dispatcher maps retained subrule identity to its loaded
callable and prepared hidden rows. On a call it:

1. checks root-direct or active-parent-direct authorization before creating a
   context;
2. expands caller arguments through starlark-rust, rejects any prepared hidden
   name already present, and appends hidden named values in descriptor order;
3. pushes a fresh monotonically unique call token, allocates one restricted
   context borrowing the enclosing label/action owner, and invokes the retained
   implementation with that context followed by caller positionals; and
4. invalidates the token and restores the previous active caller with an RAII
   guard on success, Starlark error or panic unwind.

The outer rule context, restricted subrule context, and every attr/output/
toolchain/action facade derived from either carry the applicable fresh token and
compare it with the dispatcher's current token on access. The rule context is
inactive during any subrule; a parent subrule is inactive while its child runs;
both become active again at the correct return boundary. An escaped context or
bound facade/method never reactivates during a later call of the same subrule.
Use one evaluator-local `Arc<Mutex<_>>` call stack/token state because the
shared Artifact/action facades are ordinary starlark-rust heap values and must
satisfy that heap's `Send + Sync` storage contract. The Arc is never shared
across evaluators or retained in DICE: every lock is a short stack inspection
or push/pop, is released before invoking Starlark, and is never held while
acquiring the existing action-registry mutex. No lock is held across DICE
computation.

## Value and edge materialization

Refactor predecessor validation to return or record the already-proved value
projection without changing its error order. A regular `label`/`label_list`
row receives the same configured-target wrapper/provider collection used by
ordinary `ctx.attr`; a direct source node uses the retained Null identity and a
generated node keeps its configured identity. Both gain only the bounded
file-target provider view required by that wrapper. `allow_single_file` receives the
one validated source or owner-derived output Artifact. `executable` receives a
`FilesToRunProvider` whose `executable` field is that real evaluator Artifact,
not the current display-path string. Keep the retained build-API provider
representation unchanged: improve the evaluator projection with the child
identity/artifact already available at this boundary rather than widening
execution, REAPI or provider storage.

The generic analysis lowerer must recognize evaluator `BuiltinProviderView`
values and lower their existing provider identity plus recursively lowered
fields into the existing `AnalysisValue::Provider` representation. This is
required when a subrule result such as `FilesToRunProvider` is nested inside a
user provider; it is not a new retained provider variant or a subrule-specific
serialization path. Rematerialization uses the same generic provider branch.

Absence materializes as `None` for `label` and an empty list for `label_list`.
Each label-list member remains distinct and ordered. Alias provider access uses
the actual configured child while the configured edge preserves the accepted
normalized graph target. No evaluator value survives lowering of the rule's
returned providers/actions.

Add `ConfiguredEdgeKind::ImplicitAttribute { attribute, index, tool }`.
`implicit()` is true and `tool()` returns the stored bit. Emit one edge for
every hidden descriptor child only after `evaluate_loaded_rule` succeeds, in
the same source/element order used by preparation. Failed invocation publishes
no configured result, edge, action or provider. Loading-query implicit edges
remain unchanged.

## DICE, memory and revision behavior

`ConfiguredNodeAnalysisKey` and its Observed counterpart remain the sole
retained computations. This packet adds no compute, key, cycle edge, cache,
registry, task, watcher or request carrier. All child preparation stays under
the predecessor's single aggregate cycle guard. The synchronous evaluator runs
only after those futures finish and holds no DICE handle or lock.

Loaded definitions and structural configurations remain DICE-retained semantic
inputs. The tagged analysis-target identity is retained only when an
`AnalysisValue`/provider result actually contains that target, and participates
in the existing result equality/publication cutoff. Creating a retained Null
Target admits one Arc allocation and one canonical-label clone at the
source-to-analysis-value boundary; every later clone is an Arc bump. Do not
allocate that identity for validation-only rows or values that never become a
Target. Prepared value rows,
dispatcher maps, call stack/tokens, context/action
facades and Starlark values are phase/evaluator scratch. Successful providers,
actions and configured edges are lowered into the existing immutable
`ConfiguredNodeResult`; no retained value borrows the evaluator heap. Error and
cancellation drop all scratch before publication. Same-DICE source/command
A/B/A must invalidate and restore call results and errors with no stale token,
edge, action or provider.

No request option, host observation or filesystem input changes. Concurrent
requests receive separate evaluators and dispatch state while sharing only the
already-immutable loaded package and configured child results through DICE.

## Proof contract

Reuse the pinned Bazel tests above; add no Java helper or copied fixture. Prove
both Legacy and Observed routes where the graph family matters:

- direct success, undeclared root call, undeclared sibling/parent/child calls,
  declared nested success, repeated calls and a high-count nonrecursive
  declaration set;
- positional/named passthrough, context-first binding, hidden override
  rejection before implementation side effects, and descriptor-order hidden
  kwargs;
- context label/action access, enclosing rule-context inactivity, parent
  inactivity during a child call, caller restoration after child failure,
  escaped contexts and saved bound-facade/action rejection, and a later call of
  the same subrule not reactivating an escaped value;
- absent/single/list configured targets, source and generated single Artifacts,
  executable source/rule `FilesToRunProvider` with Artifact executable, aliases,
  nesting each admitted value in a returned user provider, and ordinary
  late-bound `ctx.attr` values through the shared materializer;
- tagged Arc-backed Null/configured identity collision resistance,
  two-pointer-word identity size, cheap clone after the single admitted
  source-boundary allocation/label clone, configured-call
  compatibility, exact label projection, equality/hash/publication inequality,
  and nested lower/rematerialize preservation for a Null source Target;
- successful implicit edge order/target/implicit/tool projection, including
  interleaved Target/Exec rows and list indices; no edge/action/result on
  invocation failure;
- one successful existing action from a subrule retaining the enclosing owner,
  plus explicit fragment/toolchain/missing-action-family terminals; and
- same-DICE source A/B/A success/error restoration with no stale call state,
  providers, actions or configured edges.

Run the full existing subrule predecessor suite unchanged so dependency error
precedence, cycle termination/repair and target-only outer-frontier order remain
proved. Existing analysis-value/provider tests cover generic configured target
and provider lowering; extend them only for Artifact-backed executable
projection. No benchmark is required because this adds no retained cache and
does not alter the dependency hot path; retained-size and clone audits are
required.

## Frozen implementation envelope

Allowed production paths at `2bf86bfa8`:

- `app/slug_build_api_v2/src/analysis_value.rs`, only the tagged target identity
  and `ConfiguredTargetValue` integration,
  `26e2f0cec569af165e88d79d55583f19ec65d9d3`, plus the export hunk in
  `src/lib.rs`, `cb299989b487bb3f0150bd7f6f7ecc2c244b70f2`;
- new `app/slug_loading_v2/src/subrule_invocation.rs` plus module/export hunks
  in `src/lib.rs`, `4add32e3499539a6e0246a7c54e290393c1059ed`;
- `app/slug_loading_v2/src/subrule.rs`,
  `326fa12b214f384577ccce979b6e4590000cb8bb`; `src/package.rs`, only the
  frozen rule-wrapper/accessor hunk,
  `7831cacf526d2c3d87e28cf6ce51b4985b9758ef`; and `src/provider.rs`, only
  analysis-capability recognition,
  `b8913126566d8e4ff4448720c134b6db136dba0c`;
- `app/slug_analysis_v2/src/subrule.rs`,
  `1e86fe17b31de983a048da8d3a94d7f72740db0b`; `src/starlark_rule.rs`,
  `17fb9818dca737e0a67bac23533e79a7f9066030`; `src/analysis_value.rs`, only
  generic configured-target/Artifact-backed executable projection,
  `ac78d30616431f65411be9d5b430dc858fcfc884`; `src/dice.rs`, only
  finish/value/edge orchestration hunks,
  `917add337f30586b13a20bf94acd14a052d37d52`; and
  `src/configured_target.rs`,
  `76aef751ae0f786958213aca3038e7c6db3f0e16`.

Allowed proof paths are `app/slug_build_api_v2/tests/analysis_value.rs`,
`3cc11fafdc101c06b08f5d8403933b66e0e0d83b`,
`app/slug_analysis_v2/tests/subrule.rs`,
`d25cc60d45df2de4f08fa8b54c5830752b2ce833`, the single existing
`retained_daemon_subrule_dependency_error_precedence_restores_a_b_a` assertion
hunk in `app/slug_server_v2/src/tests.rs`,
`b8dcd3af0feb612ca67a201da5a2e68047e50b00`, and focused existing unit-test
modules in the listed production files. The daemon correction must only replace
the obsolete initial/restored deferred-boundary expectation with rule-body
execution, move the shared implementation-body rejection to selected B only,
and retain the selected dependency-validation precedence and initial/restored
A/B/A equality proof. Every other dirty hunk/file is excluded. Initial caps:
1,350 production additions, 1,400 proof additions, 2,750 aggregate additions;
no new production function above 140 lines and no new retained
descriptor/provider copy. `package.rs` and `dice.rs` exceed 2,000 lines, so keep
only adapter/orchestration hunks there. Put the evaluator bridge in the new
loading module and cohesive materialization in the existing analysis
subrule/value owners.

The Buck2 utility review classifies most work as call-flow/evaluator scratch and
the tagged identity as one bounded retained-representation correction. Reuse
the existing `AnalysisConfiguredTargetKey` Arc and wrap the one cloned Null
`CanonicalLabel` in an Arc at the admitted source boundary; derive
`Dupe`/`Allocative`, add no later string/configuration copy, interner, cache or
side lookup, and assert the two-word enum plus `ConfiguredTargetValue` sizes.
`REPLAN` rather than altering retained `FilesToRunProvider`, adding an
interner/cache, or introducing a broad Starlark-runtime crate move.

## Validation and stops

R1 independent review returned `REPLAN`: it required source Targets to survive
nested lowering/rematerialization but `ConfiguredTargetValue` could represent
only configured keys, while forbidding a retained correction. R2 embedded
`CanonicalLabel` directly, but correction review returned `REPLAN` because that
multi-String value inflated every identity, cloned deeply and could not derive
`Dupe`. R3 uses the Arc-backed tagged identity and accounts for its sole
boundary allocation/clone. Before implementation, obtain independent
correction review of that identity and the cross-crate
evaluator capability, call-token lifetime, ordinary/hidden shared value owner,
Artifact-backed executable projection, successful edge publication and exact
scope. Independent terminal implementation review remains mandatory.

The first terminal implementation review returned `REPLAN`: file nodes passed
validation with inherent `DefaultInfo` but their materialized Target copied the
node's intentionally empty provider collection; each frozen-subrule call cloned
the full prepared dispatcher map and target/package strings; and the proof did
not discriminate high-count calls, absence shapes, source executables, alias
actual access or hidden descriptor order. The bounded correction creates a
materialization-only singleton-file `DefaultInfo` for source/generated Target
values without altering their configured result, Arc-shares the immutable
dispatcher payload so a call clones only two Arcs, and adds all missing direct
discriminators plus nested source/generated provider rematerialization.
Terminal correction rereview returns `ACCEPT`: the bounded provider view,
Null/configured identities, Arc-only call clone, 256-call two-route proof,
absence/source-executable/alias/order discriminators, scope isolation and
1,230-production/469-proof/1,699-aggregate accounting all satisfy the packet.

Run formatting and `git diff --check`; focused loading/analysis-value/subrule/
configured-edge tests; the complete serial `slug_loading_v2` and
`slug_analysis_v2` suites; named build/cquery/query/server dependents; relevant
multi-crate checks; retained-size/cap/clone audits; forbidden-surface scans;
base/worktree-blob isolation; archive checker; and an index-only repeat with
only packet hunks. Clean stale `slugd` around daemon tests and rebuild
`slug_cli_v2` before any `SLUG_V2_BIN` replay.

`REPLAN` for a new DICE key/compute/cache/registry; evaluator values retained in
DICE; an unsafe/raw callback bridge; a lock across DICE compute; a second rule
evaluation or action owner; failure publication; loss of dependency-error
precedence; any retained provider change beyond the exact tagged target
identity; fabricated configuration bytes; loss of the Null/configured tag
during lowering/rematerialization; a parser/binder/`set`
fork; C++/rules_cc special casing; silently exposing fragments/toolchains or
unlisted action families; inability to invalidate escaped contexts on every
exit; dirty-hunk overlap; unlisted files/cap overflow; Java; or an exact claim
without Bazel 9.2 evidence.

## Immediate predecessor and successor

Commit `2bf86bfa8` accepted configured hidden Target/Exec dependencies,
validation and loading-query facts with one source-ordered stream and one
aggregate parent cycle guard. It deliberately stopped before evaluator values,
invocation and configured hidden edges; this packet is that named successor.

After acceptance, run the fragment-projection category needed by the authentic
rules_cc `create_fdo_context` call, then the generic action category
(`args`/`run`/`symlink` and artifact/input provenance), then the
`cc_common.absolute_symlink` BCR consumer. Reaudit the authentic repository
after each category and select the first new generic terminal; do not infer a
native C++ rule path merely because the consumer is rules_cc.
