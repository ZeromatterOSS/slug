# Current Slug V2 Packet

Packet: `WP-4-5-7A-bazel-bzl-global-capability-category-architecture-r3`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Base: terminally accepted repository-context attribute implementation
`c83e70f0f`, accepted module-loaded native context `1f9433600`, accepted exact
process-stable Bazel universe `cb71a302d`, and the existing loading/analysis
provider and exported-rule owners. All unrelated dirty analysis, loading, core,
and REAPI work remains parked and read-only.

## Observable result

Freeze, without Rust changes, the category-wide architecture for the six
Bazel 9.2 `.bzl` global capabilities selected by the authentic
`bazel_features` generated repository:

- `macro`;
- `PackageSpecificationInfo`;
- `RunEnvironmentInfo`;
- `set`;
- `subrule`; and
- `DefaultInfo`.

This packet must assign every name to its natural runtime, loading, or analysis
owner; classify its existing and target compatibility; and schedule bounded
implementation packets that share exported-symbol and provider identity without
sharing incompatible lifecycle state. It must not add a placeholder callable
merely to advance the replay.

The first successor must make two fresh rules_rust replays pass the current
`@@bazel_features++version_extension+bazel_features_globals//:globals.bzl:7`
`macro` reference and stop at the next authentic unsupported boundary or
succeed. The generated globals repository and the requested rules_cc
compatibility route are integration discriminators only. `cc_common`,
`cc_internal`, C++ rules, and ruleset-specific parsing remain outside this
category.

## Learned facts and semantic authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority.

### Global placement and inventory

- `StarlarkGlobalsImpl.getFixedBzlToplevels` composes utility globals,
  `BazelBuildApiGlobals`, and `StarlarkRuleClassFunctions`, then installs
  `attr`, `struct`, `OutputGroupInfo`, `DefaultInfo`, `RunEnvironmentInfo`, and
  other fixed `.bzl` values. `macro` and `subrule` therefore come from the same
  rule-function collection but not from the process universe.
- `BazelRuleClassProvider.PACKAGING_RULES` registers
  `PackageSpecificationInfo` as a `.bzl` top-level. It is merged with the fixed
  `.bzl` globals by `BazelStarlarkEnvironment.createBzlToplevelsWithoutNative`.
- `BazelStarlarkEnvironment` shares that `.bzl` top-level map between BUILD-
  and MODULE-loaded `.bzl` evaluation while keeping their `native` namespaces
  distinct. None of the three missing names belongs in `native` or the BUILD
  file top level. Of the six selected names, only process-universe `set` is a
  BUILD-file global. Slug currently leaks `DefaultInfo` into
  `build_file_loading_globals`; the first successor must remove that leak and
  prove all three environments rather than treating current placement as exact.
- `set` is a process-universe builtin. Bazel's generated `globals.bzl` selects
  the exact six active fallbacks at lines 7-16. The other generated fields
  intentionally fall back to `None` in this Bazel 9.2 environment.
- `BuildLanguageOptions.experimental_enable_first_class_macros` defaults to
  true in Bazel 9.2. Slug's pinned default compatibility must therefore admit
  the normal non-finalizer macro path without requiring a new flag.

The current replay executes the authenticated generated file and fails before
freezing it with:

```text
Variable macro not found
@@bazel_features++version_extension+bazel_features_globals//:globals.bzl:7:71
```

The same generated file references `PackageSpecificationInfo` at line 8 and
`subrule` at line 15. Publishing only `macro` would guarantee immediate churn.

### Symbolic macro lifecycle

- `StarlarkRuleClassFunctions.macro` accepts an implementation, attribute map,
  `inherit_attrs`, `finalizer`, and documentation. `name` and `visibility` are
  reserved automatic attributes. Declared attributes may delete inherited
  entries with `None`; computed and late-bound defaults are rejected.
- The returned `MacroFunction` is unusable until its defining `.bzl` module
  exports it. Export binds the defining label and exported name and freezes a
  `MacroClass`; an imported alias retains the producer identity.
- Invocation is keyword-only, first validates and instantiates one package-
  owned `MacroInstance`, then evaluates the implementation in a fresh Starlark
  thread/evaluator that shares the package-construction context. Default
  non-finalizer macros expand synchronously so their declared targets are
  visible to later package-construction operations.
- The implementation receives effective schema values including automatic
  `name` and `visibility`, may declare nested rules and symbolic macros, must
  return `None`, and marks every resulting target as created in a symbolic
  macro. Target/submacro naming, visibility, recursion, and forbidden package
  operations are package-construction semantics, not parser semantics.
- Bazel retains macro instances, target creator/origin, defining package, and
  namespace violations in the package. An invalid target namespace can leave a
  loadable package and fail only when that target is requested/configured;
  ordinary target-name collisions still fail eagerly. Macro origin therefore
  affects target visibility and package semantic identity and cannot live only
  in evaluator scratch.
- Finalizer macros defer expansion until all non-finalizers and expose an
  existing-rule snapshot. That is a separate ordering/effect category and is
  not required by the current consumer.

### PackageSpecificationInfo identity

- `BazelRuleClassProvider` installs
  `PackageSpecificationProvider.PROVIDER`, whose public name is
  `PackageSpecificationInfo`.
- The provider instance is created only by analysis of `package_group`; its
  constructor is private. The `.bzl` global is therefore a nonconstructible
  builtin provider key, not a generic Starlark provider constructor.
- Instances expose `contains(Label|string)`. Constructed instances and
  package-group analysis remain a later configured-analysis category; global
  identity and provider-key lookup must not fabricate an empty instance.

### Subrule lifecycle

- `subrule` returns an exported Starlark callable with implementation, private
  label/label-list attributes, at most one toolchain, fragments, and declared
  child subrules. It is unusable before defining-module export.
- Export binds `(defining label, exported name)` and transforms each private
  attribute into Bazel's hidden rule attribute name. For `_foo`, the retained
  name is derived from `$<canonical .bzl label>%<subrule>%_foo` through the
  attribute value-source conversion.
- `rule(..., subrules=[...])` and `aspect(..., subrules=[...])` transitively
  discover and deduplicate subrules, lift their hidden dependency attributes,
  toolchains, and fragments into the consumer declaration, and retain the
  declared-subrule identities for invocation authorization.
- During configured analysis, a direct or nested subrule call verifies that
  the active rule/aspect or parent subrule declared it, resolves hidden
  dependencies, locks the ordinary rule context, and invokes the implementation
  with a restricted `subrule_ctx`. The context is invalidated after return.
- This is configured-analysis execution over already-loaded metadata. It must
  not execute during `.bzl` loading or add a second configured-target graph.
- `subrule_ctx` exposes `label`, `actions`, `toolchains`, and `fragments`, but
  toolchain access requires automatic execution groups. Slug currently rejects
  nonempty rule fragments and has no admitted automatic-exec-group owner. The
  first exact subrule slice must therefore be dependency-only with empty
  toolchains/fragments; those context fields require a separately reviewed
  prerequisite before admission.

## Current capability matrix

| Capability | Current owner/status | Compatibility now | Category target |
|---|---|---|---|
| `set` | `slug_starlark_v2::populate_universe`; real starlark-rust `SetType` accepted in `cb71a302d` | **exact** for the accepted Bazel 9.2 default set slice | preserve unchanged; no parser or set implementation |
| `DefaultInfo` | loading `AnalysisBuiltinCallable`; configured-analysis lowering and structural provider owner; currently also leaked into BUILD globals | **exact** for admitted identity, optional `files`/`executable`, empty synthesis, and accepted projections; BUILD placement is not exact | preserve the provider key, remove BUILD placement, and extend fields only under a separately authenticated provider packet |
| `RunEnvironmentInfo` | loading provider token plus build-api/configured projection | **exact** identity/placement and admitted structural projection; Starlark construction unsupported | preserve identity; constructor completeness remains separate configured-provider breadth |
| `PackageSpecificationInfo` | absent | unsupported | exact nonconstructible builtin provider key and later exact package-group instance/`contains` semantics |
| `macro` | absent; legacy function macros already execute as ordinary frozen Starlark functions | unsupported | exact default non-finalizer symbolic declaration/export/package expansion; finalizers separately deferred |
| `subrule` | absent; rule declarations and configured analysis already have adjacent schema/context owners | unsupported | exact dependency-only direct/nested rule declaration, hidden dependency lift, authorization, `label`/`actions` context, and action ownership; toolchains, fragments, and aspects deferred pending their owners |

Existing accepted exact slices remain exact. A global's presence never implies
that an explicitly deferred constructor field or execution lane is supported.
Every deferred call must fail at its typed boundary before retained semantic
state or actions are published.

## Frozen architecture

### One global-composition owner, typed capability modules

`complete_loading_globals(bool_config, bzlmod_native)` remains the sole owner
of Slug's loading global composition. The `bool_config` branch is the existing
`.bzl`/BUILD distinction and must install the three missing `.bzl` names plus
the existing `DefaultInfo` and `RunEnvironmentInfo` for both BUILD-loaded and
Bzlmod-loaded `.bzl` modules. Move `DefaultInfo` out of the unconditional
branch. Do not install these five names in the `native` namespace, BUILD-file
globals, or process universe.

Keep `populate_universe` as the sole `set` owner. Keep the specialized
`DefaultInfo` and `RunEnvironmentInfo` values because they already own admitted
construction/projection behavior. Add no runtime enum that erases these value
types into one generic callable.

Add one focused inventory proof over `loading_globals`,
`bzlmod_loading_globals`, and `build_file_loading_globals`. Both `.bzl`
environments contain all six names. BUILD globals contain only `set` among the
six and must reject the other five. Establish value classes without asserting
that deferred calls are exact. The proof is the shared anti-churn ledger;
runtime registration remains direct typed composition rather than a callback
registry.

### Shared builtin-provider key lane

Add a key-only builtin provider representation in `provider.rs` as one
loading-owned nonconstructible `BuiltinProviderKey` carrying a static name. It
provides structural `ProviderIdentity::builtin(name)`, exact display/type
behavior, hashing/equality, and frozen rematerialization through the existing
provider bridge. It carries no callability enum and no configured instance
fields.

Use it for `PackageSpecificationInfo`. Converge the existing nonconstructible
TestingBootstrap provider tokens onto the same key lane only if a focused proof
shows their exact type, repr, hash/equality, and call error stay unchanged;
otherwise leave them alone. Keep callable-but-deferred TestingBootstrap
constructors and the specialized
`DefaultInfo`, `RunEnvironmentInfo`, and `OutputGroupInfo` values distinct.

This is provider-key convergence, not a universal builtin-value abstraction.
Provider instance construction, membership, `contains`, and configured
package-group dependencies remain with analysis.

### Symbolic macro declaration and frozen identity

Add loading-private transient/frozen symbolic macro definition values adjacent
to `RuleDefinitionGen` in `package.rs`. The transient definition owns:

- the implementation value;
- the defining `BzlModuleIdentity`;
- one declaration-order schema over automatic and declared/inherited
  attributes;
- the finalizer bit and documentation; and
- one export-time `OnceCell<CompactString>`.

The frozen definition owns the frozen implementation, defining identity,
exported name, immutable schema, and finalizer bit. `export_as` is the sole
identity transition. Freeze rejects unexported definitions. Imported aliases
retain the original frozen value and therefore cannot acquire importer
identity. Do not add a side export table or rescan frozen module globals.

Normalize explicit and inherited attributes into the existing attribute
descriptor/schema representation once at declaration. Preserve declaration
order, deletion by `None`, public-only inheritance, mandatory/default/
configurable policy, and automatic `name`/`visibility`. Do not copy a rule's
complete builtin schema into every macro: retain only the inherited public
attribute projection needed by Bazel's macro contract.

### Package-owned non-finalizer expansion and retained origin

Frozen non-finalizer macro invocation is keyword-only and requires the existing
`PackageRecorder` capability. It must:

1. validate export state, invocation shape, name, known/mandatory attributes,
   defaults, and the new macro-instance name before recording that instance;
2. construct one evaluation-lifetime effective argument vector in schema order;
3. append one compact package-owned macro-instance record containing stable
   identity, parent, producer/defining-package identity, name/depth, visibility,
   and call metadata, then push a scratch frame referencing that record;
4. invoke the frozen implementation with named arguments in a fresh evaluator
   over the same package recorder/construction context, matching Bazel's fresh
   thread rather than recursively reusing the caller evaluator;
5. require a `None` result; and
6. restore the previous frame on success or error.

Rule and native-rule recording consult the active macro frame to mark targets,
derive exact macro default/actual visibility, retain compact creator/origin and
defining-package identity on each target, and reject BUILD-only package/
environment/glob/subpackages operations at Bazel's corresponding boundary.
Nested calls create retained child instances and fresh evaluators over the same
recorder. Recursive macro-class identity and actual target-name conflicts fail
eagerly. Macro namespace violations are retained on the package/target so the
package may load and the later requested-target/configuration boundary rejects
them as Bazel does; do not turn that diagnostic into eager package-load failure.

Only the current frame and effective argument vector are evaluation scratch.
Frozen definitions, compact macro instances, target origin/visibility, and
namespace-violation state participate structurally in package equality and
existing package-load DICE invalidation. Do not add a macro cache or key.
Errors restore the active frame; do not promise transaction rollback of targets
already emitted before a later implementation error without pinned evidence.
Finalizers `REPLAN` around an ordered package-owned deferred queue and exact
existing-rule snapshot rather than reusing the synchronous frame.

### Dependency-only subrule declaration metadata and rule lift

Add transient/frozen `SubruleDefinition` values adjacent to rule definitions.
They own frozen implementation, defining/export identity, declaration-order
private dependency schema, and direct child-subrule identities. Export computes
hidden rule attribute names once. Imported aliases retain producer identity.

Extend the rule declaration's retained metadata with a compact ordered set of
declared subrule identities and a sparse ordered array of lifted hidden
attributes. Transitive discovery is stable-first and deduplicates a shared
child identity. Lifted attributes enter the existing `AttributeSchema` and
dependency pipeline; do not create a parallel dependency resolver. Hidden
subrule attributes are absent from ordinary `ctx.attr` publication.

The first exact slice admits only Bazel's private label and label-list schemas,
target/exec configuration, noncomputed defaults, empty `toolchains`, empty
`fragments`, and exported child subrules. Nonempty toolchains/fragments,
attached aspect attribute shapes, and `aspect(..., subrules=...)` fail at their
typed declaration boundaries rather than being discarded. They remain
unsupported until separately reviewed automatic-exec-group, configuration-
fragment, and configured-aspect owners exist.

### Evaluator-local subrule analysis state

Extend `StarlarkRuleImplementation` with the immutable declared-subrule
metadata. It participates structurally in equality and DICE invalidation while
the frozen implementation pointer remains lifetime-only as today.

During `evaluate_starlark_rule`, install one evaluator-local
`AnalysisInvocationState` beside the existing toolchain/materialization state.
It borrows the current `AnalysisContext`, declared subrules, resolved hidden
dependencies, actions owner, and an active-subrule stack. A frozen subrule call:

1. authenticates direct or parent-subrule declaration;
2. creates a restricted `SubruleContext` exposing exact `label` and `actions`
   over the same rule/action owner;
3. projects only its own hidden dependency arguments;
4. locks ordinary context access and pushes the active identity;
5. calls its implementation; and
6. invalidates the wrapper and restores the parent stack on every exit.

No evaluator value escapes the analysis call. Actions registered through a
subrule remain actions of the configured target's existing structural action
owner. No new DICE key, provider graph, action cache, or nested evaluator is
introduced.

Direct rule subrules are the first exact execution slice. Nested subrules are
included because their authorization determines the retained representation.
`subrule_ctx.toolchains` and `.fragments`, nonempty declaration requirements,
and configured-aspect invocation remain deferred until Slug has admitted
owners; declarations must reject them rather than silently discard them.

## Request, revision, memory, and concurrency

All three missing definitions are determined solely by source, transitive load
closure, repository mapping, and the pinned Bazel 9.2 semantics already present
in `.bzl`/package/configured-analysis keys. There is no command overlay or
ambient Host input in this category.

Frozen macro/subrule definitions are owned by the defining `FrozenBzlModule`
and released with its existing DICE value. Macro effective arguments and the
active frame are evaluator scratch; compact macro instances and target origin,
visibility, and namespace-violation state are package-owned semantics. Retained
subrule identities, hidden schemas, and resolved values are owned by the
package and configured rule implementation and participate in their existing
equality/invalidation. Subrule contexts are evaluator scratch and are invalid
after the call.

Use `Arc<[T]>`, `CompactString`, `SmallMap`, and the existing provider/schema
identities where values are retained. Use a small stack/vector for active macro
or subrule frames. Do not intern request-local instance names, clone frozen
function graphs, retain evaluator `Value`s outside their owning frozen heap, or
hold a lock across DICE computation. No task, cancellation, shutdown, daemon,
or eviction policy changes.

## Zabel peer guidance

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
concept/optimization guidance only.

- Its symbolic macro host uses a request-local call scope so rule/native
  capture can observe macro creation without putting the frame into retained
  graph identity.
- Its subrule loader binds defining-module/export identity once and retains
  sparse `declared_subrules` plus a grouped hidden-attribute array rather than
  duplicating full rule schemas per subrule.
- Its configured subrule paths reuse the ordinary configured dependency,
  toolchain, action, and provider owners.

These ideas corroborate the ownership and compact-retention choices above.
Copy no Zig code, error wording, evaluator representation, allowlist,
unsupported boundary, test result, or compatibility claim. Bazel 9.2 source
and accepted oracle evidence alone define behavior.

## Compatibility and non-decisions

**Exact:** the corrected six-name `.bzl`/BUILD inventory and placement; real starlark-rust
`set`; existing admitted `DefaultInfo` and `RunEnvironmentInfo` identities;
nonconstructible `PackageSpecificationInfo` provider key; defining-module
macro/subrule export identity; default non-finalizer macro declaration,
attribute projection, synchronous nested package expansion, visibility and
name constraints including retained late namespace violations; dependency-only
direct/nested rule subrule declaration, hidden dependency lift, authorization,
restricted `label`/`actions` context, and action ownership.

**Slug-native:** Rust type/module names; compact collection selection;
structural DICE identity; allocation details; diagnostics where Bazel's exact
punctuation is not discriminating; evaluator-local frame representation.

**Unsupported/deferred:** symbolic macro finalizers and their existing-rule
snapshot; symbolic macro laziness beyond Bazel 9.2's eager default path;
subrule toolchains/automatic execution groups, fragments, attached-aspect
attribute shapes, and configured-aspect subrule declaration/invocation;
`PackageSpecificationInfo` instances and `contains` until package-group
analysis is admitted; unadmitted `DefaultInfo` fields and
`RunEnvironmentInfo` construction; `_builtins` injection; `.scl`; runtime
selectable Bazel versions; parser/set changes; `cc_common`, `cc_internal`, C++
rules/actions, and any ruleset shortcut.

## Successor schedule

The category is implemented in this fixed order so shared identity lands once
and every later packet consumes it:

1. `WP-4-5-7A-symbolic-macro-and-bzl-provider-key-implementation`: add the
   key-only provider lane, exact `PackageSpecificationInfo` global, corrected
   BUILD placement, exact default non-finalizer macro declaration/export/
   retained package expansion, the loading/package portion of the frozen proof
   matrix below, and two fresh replays. This successor retains late namespace
   violations in package identity but does not yet claim their configured-
   target enforcement. Allow only `app/slug_loading_v2/src/package.rs`,
   `app/slug_loading_v2/src/provider.rs`,
   `app/slug_loading_v2/src/testing_bootstrap.rs`,
   `app/slug_loading_v2/src/host_package_load_tests.rs`, and
   `app/slug_loading_v2/src/bzl_invalidation_tests.rs`; cap 2,100 production,
   2,200 proof, 4,300 aggregate. `testing_bootstrap.rs` is optional and may be
   touched only for proven nonconstructible-key convergence.
2. `WP-4-5-7A-symbolic-macro-late-namespace-enforcement`: consume the retained
   target violation at the existing configured-target admission lookup and
   prove that the package loads while direct configured lookup fails. Freeze a
   one-production-hunk/two-file analysis allowlist and exact dirty-file hashes
   at activation; `app/slug_analysis_v2/src/dice.rs` and
   `app/slug_analysis_v2/tests/configured_target.rs` are currently overlapping
   dirty files and are not authorized by successor 1. Cap 40 production, 120
   proof, 160 aggregate. No new DICE key, target resolver, or loading mutation
   is allowed.
3. `WP-4-5-7A-subrule-declaration-and-hidden-schema-implementation`: add exact
   dependency-only subrule export identity, declaration validation, transitive
   stable-first metadata, rule `subrules` input, hidden schema/dependency lift,
   explicit rejection of toolchains/fragments/aspect shapes, and loading
   proofs. Allow only `app/slug_loading_v2/src/package.rs` and
   `app/slug_loading_v2/src/host_package_load_tests.rs`; cap 1,200 production,
   1,300 proof, 2,500 aggregate.
4. `WP-4-5-7A-direct-subrule-analysis-invocation-implementation`: carry the
   retained metadata into `StarlarkRuleImplementation`, add evaluator-local
   direct/nested authorization and restricted `label`/`actions` context, reuse
   dependency/action owners, and prove configured A/B/A. Freeze its exact file
   allowlist against the live carrier state during packet activation; cap 1,200
   production, 1,500 proof, 2,700 aggregate.
5. `WP-4-5-7A-subrule-toolchain-fragment-prerequisite-architecture`: activate
   only when a real consumer reaches this boundary. Authenticate automatic exec
   groups, fragment ownership, attached-aspect shapes, and context publication
   before admitting any nonempty requirement; this is not part of the initial
   exact dependency-only slice.
6. `WP-4-5-7A-bzl-global-capability-category-closure`: zero-new-semantics audit
   of all six names, fresh rules_rust replays, exact next-boundary selection,
   and explicit retained/deferred matrix. Any newly demanded provider field,
   finalizer, aspect, or ruleset primitive becomes a separately reviewed
   category packet.

Caps are addition/deletion net lines from each packet base. Moving existing
code counts at the destination. Any successor that needs a new DICE key,
parallel dependency/action owner, generic builtin-callable erasure, or a file
outside its allowlist must stop and `REPLAN` before editing.

### Frozen symbolic-macro/provider proof matrix

Successor 1 owns every loading/package discriminator below. Successor 2 owns
only the explicitly separated configured-target rejection row; together they
terminally admit the default non-finalizer symbolic-macro slice.

- inventory: both BUILD-loaded and MODULE-loaded `.bzl` environments contain
  all six names; BUILD globals contain `set` and reject the other five;
- provider key: `type(PackageSpecificationInfo) == "Provider"`, exact
  `<function PackageSpecificationInfo>` repr, pinned noncallability and
  call-error class, hashing/equality/provider identity, freeze/rematerialize,
  and proof that no provider instance can be fabricated;
- declaration/export: `.bzl`-only `macro`, factory/callable value class,
  unexported freeze/use rejection, defining-label/exported-name identity, and
  imported-alias preservation;
- schema: automatic `name`/`visibility`; every already-admitted relevant attr
  kind; mandatory/default/explicit `None`; private-name rejection; unknown
  attributes; inheritance from rules, exported macros, and `"common"`; `None`
  deletion; declaration order; configurable-value promotion;
- invocation: keyword-only calls, effective schema order, fresh evaluator over
  the same package recorder, `None` return requirement, direct nesting,
  recursive macro-class rejection, and success/error frame restoration;
- visibility: top-level package defaults plus call-site package, nested private
  default plus definition/call-site packages, explicit forwarding, and proof
  that BUILD package defaults do not leak into nested macro declarations;
- naming and package ownership: allowed separator cases, eager ordinary name
  collisions, retained namespace violations whose package loads, compact
  instance parent/depth/definition origin, target generator/call metadata, and
  package result equality for every semantic field. Successor 2 separately
  proves that requested target/configuration rejects the retained violation;
- forbidden operations and errors: package/environment/glob/subpackages/
  existing-rule access at Bazel's exact admitted boundary, no post-error frame
  leakage, and pinned evidence for any partial package mutation retained after
  an implementation error;
- incrementality: source-definition and BUILD-invocation A/B/A each invalidate
  and restore through the existing frozen-module/package DICE owners, including
  origin, visibility, and namespace-violation changes; and
- integration: rebuild `slug_cli_v2`, clean stale `slugd`, run the focused and
  full loading/Bzlmod suites, then run both fresh authenticated rules_rust
  replays. They must clear `macro` and stop only at the next real unsupported
  boundary or succeed.

No generic “colocated tests” allowance exists. If one matrix row cannot be
proved within the exact file/cap envelope, stop and `REPLAN` the successor
rather than weakening the exact claim.

## Architecture proof and acceptance gates

This zero-Rust packet is accepted only if independent review confirms:

1. the six-name inventory and current status match the authentic generated
   Bazel 9.2 file and Slug's live globals;
2. `set` remains entirely owned by starlark-rust/universe composition;
3. the provider-key lane cannot fabricate `PackageSpecificationInfo` instances
   or weaken existing provider callability;
4. macro source/export/package state is separate from subrule configured-
   analysis state while both reuse defining-module identity;
5. macro retained instances/origin and subrule actions reuse the existing package and
   configured-target owners rather than a parallel graph;
6. retained macro/subrule metadata participates in the correct existing DICE
   equality and no evaluator scratch escapes;
7. finalizer, subrule toolchain/fragment/attached-aspect, provider-instance, and
   broader provider-field boundaries are explicit and fail closed; and
8. the successor order advances the real bootstrap frontier without treating
   `cc_common` or rules_cc as a semantic implementation target.

## Architecture allowlist and stop conditions

This packet may change only:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`
  only if routing-log rollover requires it.

Production and proof caps are both zero. Stop for human design input only if
independent review finds that exact Bazel 9.2 macro expansion requires a new
package/DICE ownership model, or direct subrule actions require a second
configured-target/action identity. Ordinary review corrections remain within
this architecture packet.

The live `app/slug_loading_v2/src/package.rs` baseline is intentionally dirty
from another workstream: 28 additions, zero deletions; HEAD blob
`39d35aa742d6db24989e6e5ce4a65963bf447d86`; worktree SHA-256
`623bcd93f7a8dde2fad8728ea157e9510b05dedd79a4f1cba5a4ba4a4275f047`.
Successor activation must re-audit that hunk, stage only packet-owned hunks, and
prove the staged diff excludes the pre-existing definition-source additions.

The current late-enforcement files are also dirty and require a fresh activation
certificate. `app/slug_analysis_v2/src/dice.rs` is 231 additions/368 deletions,
HEAD blob `e31aae7f06d6de497ee7a7bd9e1968d6548be540`, worktree SHA-256
`f848cc912a379791668c3c2ee01f44648668627757ba53d6c64c3ae3297b5687`.
`app/slug_analysis_v2/tests/configured_target.rs` is 297 additions/40
deletions, HEAD blob `675ba67e2f114f310aedb176ebdc91cfc1bd471a`, worktree SHA-256
`cada05ad59aab7927fd781c552e905077874fa7faca0a2ca316adc9d4da009fc`.
Successor 2 must refresh these values, restrict production to the existing
configured-target admission function, and stage only its enforcement and one
focused proof hunk.

Independent architecture review returns `ACCEPT` for R3. R1 was rejected for
incorrect BUILD placement, unowned subrule toolchain/fragment semantics, and an
under-specified successor proof. R2 corrected those points but put configured-
target enforcement inside a loading-only implementation envelope. R3 separates
retained loading/package identity from the bounded natural analysis consumer;
no semantic claim was weakened.
