# Current Slug V2 Packet

Packet: `WP-6-7A-complete-noncallback-spawn-envelope-implementation-r3`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Terminal implementation `ACCEPT`. The first terminal implementation
review returned `REPLAN` because the
evaluator method declarations did not follow Bazel's public parameter order and
raw outer shapes allowed a later typed binding error to preempt an earlier
invalid argument. The correction uses the exact Bazel signature order and one
pre-method outer-shape binding pass, with dual-invalid precedence proof for
both `run` and `run_shell`; the focused correction rereview returned `ACCEPT`.
The same review otherwise accepted the common
Spawn representation, publication equality, scoped provenance, and tool-depset
branching. R1 returned
`REPLAN`: it mistook the execution-time result of `resource_set` for an
analysis-time dictionary parameter and claimed exact File executable/tool
behavior without detecting Bazel's executable-attribute-to-FilesToRun
association. R2 corrected those owners but returned `REPLAN` for treating all
depset tools alike: Bazel checks every File in a top-level `tools=depset(...)`
against that association, while a depset nested in a tools sequence bypasses
per-leaf lookup. R3 preserves that distinction without flattening retained
depset topology. It accepts only omitted/`None` resource callbacks and only
File forms admitted by their exact container-specific association rule. This
remains one bounded generic Spawn-envelope packet over the accepted action
owner, not a C++/FDO rule packet
or permission to implement adjacent provider, callback, execution, or
named-exec-group categories.

Base: `a01a23fe7`, which terminally accepts one evaluator-owned Args recipe,
scalar and non-callback vector transforms, param-file policy/write, generic
typed `run`, artifact/absolute symlinks, configured action environments, dense
depset inputs, and atomic action publication. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and category boundary

Ordinary rules and subrules can declare `actions.run` and `actions.run_shell`
through the same typed `SpawnSpec` and the same common non-callback envelope.
The packet completes all fields whose semantic inputs are already owned by the
default configured action context: arbitrary nonempty output sequences,
sequence/depset inputs and admitted tools, mixed string/Args arguments,
string/proven-unassociated File executables, string shell commands,
`unused_inputs_list` for
`run`, mnemonic, raw progress message, configured/default plus explicit
environment, filtered execution requirements with default Bazel 9 target-tag
propagation, ignored `input_manifests`, omitted/`None` `resource_set`, and
explicit `None`/omitted default-context selection.

“Complete” is deliberately category-qualified. Non-`None` `shadowed_action`,
named `exec_group`, automatic-exec-group/toolchain selection, FilesToRun and
runfiles expansion, Files associated with an executable attribute, callable
`resource_set`, deprecated list-valued shell
commands, and execution-time helper scripts are separate semantic categories
and fail closed here. Their parameter positions are admitted with exact
`None`/default or rejection behavior so a later packet extends the one envelope
instead of adding another Spawn representation.

Bazel 9 BCR Starlark owns rule control flow, including `cc_internal`.
`cc_common` and rules_cc action construction are demanding consumers of this
generic API, never Rust C++ rule bodies or parser branches. Buck2 starlark-rust
continues to own parsing, binding, evaluation, dispatch, and heap lifetime.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority:

- `StarlarkActionFactoryApi.java` SHA-256
  `0e9173ce523ff5b6a52b09065e0c4e113ae3433d968a6c1bc5d5e0d48ede8a25`
  fixes the complete `run`/`run_shell` signatures, default values, and public
  parameter types.
- `StarlarkActionFactory.java` SHA-256
  `f3e2201e7d8c712318c967b652e685ff3b17b8ba7167dd838b4ea2b96ad71681`
  fixes mixed command-line construction, shell `$0` padding, common-envelope
  validation order, tool/input handling, environment choice, execution-info
  filtering, exec-group selection, unused-input discovery, and resource-set
  parsing.
- `TargetUtils.java` SHA-256
  `d28d06e0803c4442aef9315a29019f4d1a3653bcc6ef9c488535a7036b97290d`
  fixes legal execution-info keys, explicit-value precedence, target-tag
  propagation, duplicate collapse, and sorted output.
- `SpawnAction.java`, `StarlarkAction.java`, and `AbstractAction.java` SHA-256
  values `2f71947da1863b6e6264cfb0480b47b39b9e527135b10cfefacfb4ba3700b5fa`,
  `75a798c4e6225078ed803c87a427a052debf856e0ad47d3e44330a5457e4d7b7`, and
  `02e6f567792d285a7139f956238ea990118160ce333d1ac8d88f7a43efbb188a` fix retained Spawn fields, shell and progress-message
  ownership, input discovery, and the later presentation substitution seam.
- `StarlarkRuleImplementationFunctionsTest.java` SHA-256
  `c87d218a50c8380178cae400876c906155e3a4fbe84b731537c085adcbe36260`
  pins shell padding, environment, execution-info filtering, mnemonic,
  progress-message templates, mixed arguments, depset inputs, and default
  Bazel 9 rejection of list-valued shell commands.
- `StarlarkRuleContextTest.java` SHA-256
  `5d3973895db273a1d5d705489d820910ed0353db87a2d33f9e847ede87f61510`
  pins unused-input discovery, callable resource-set results, and
  FilesToRun-backed executable behavior. The resource dictionary is the
  callback's execution-time return value, never the public parameter.
- `StarlarkRuleContext.java` and `StarlarkAttributesCollection.java` SHA-256
  values `5200266852f65ca66a958a3adaf82a29f9b5cbbd1a604a4e91d7815476985072`
  and `9b3b300d7e9c25dceafc8a9450dd2511f9b0b83088e11421b6dc3b5086cc7442`
  fix the producer-owned executable-Artifact to FilesToRun association used by
  both executable and direct-list tool lowering.
- `StarlarkSubrule.java` SHA-256
  `9d2115fdf86f1807abaf0405d3a5b36fbb3d9f8abd87aa82440f72e6e46657b6`
  fixes the stricter subrule boundary: a File corresponding to dependency
  runfiles is rejected rather than recovered and must be passed as
  FilesToRunProvider.
- `AutoExecGroupsTest.java` SHA-256
  `c45c938a358c46bfdcc71becabf3a0332bf80441b24b32b71eed1c38e3a7cc4e`
  proves that named/automatic exec-group compatibility is a real configured
  topology boundary and therefore is not guessed by this default-context
  packet.

No fresh Bazel oracle is required unless implementation exposes an observable
not discriminated by those sources/tests. Java weak interning, object layout,
builder classes, helper-script counters, and implementation class names are
not compatibility surfaces.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only
peer guidance. Its `ARCHITECTURE.md` and
`src/analysis/{logical_actions,starlark_action_registration}.zig` demonstrate
one producer-owned Spawn row, explicit-over-tag execution-info merging,
configured inherited-environment policy, compact retained flags, and atomic
registration. Copy no Zig code, layout, fingerprints, diagnostics, caches, or
tests. Buck2 commit `088c75c7e36805df99c3de29062baa95db700b8b` supplies only
retained-utility guidance: reuse Slug's existing `Arc` slices,
`CompactString`, `CanonicalStringMap`, `Dupe`, `Allocative`, and dense depset;
import no Buck action, command-line, registry, scheduler, or user semantics.

## Compatibility classification

**Exact:** active-context and receiver checks; named parameter/default binding;
nonempty ordered output sequences; admitted File/depset input and tool order,
where File executables, direct sequence tools, and every leaf of a top-level
tools depset are proven absent from the configured executable-attribute
association before publication, while nested sequence depsets are retained
without that lookup;
mixed string/Args segment order and Args snapshotting; run executable and
run-shell command validation order; `$0` padding based on a nonempty top-level
arguments sequence even when an Args expands empty; default `Action` mnemonic
and alphanumeric validation; raw progress-message retention; explicit and
default action-environment composition; string/string dict validation;
execution-requirement legal-key filtering; default-true legal target-tag
propagation with explicit values winning and canonical key order;
`unused_inputs_list` File/`None`; ignored sequence/`None` `input_manifests`;
omitted/explicit `None` resource callback equivalence and public rejection of
non-callable resource values before method entry; `None` or
omitted default exec context; atomic output conflict behavior; and publication
equality/cutoff for every retained field.

**Slug-native:** Rust valid-Unicode strings; configured generated paths and
shell executable spelling; compact immutable envelope layout; structural
action identity; symbolic retention of a shell command rather than Bazel's
analysis-time helper-script Artifact; and progress-message output/input bytes
when generated path spelling participates. The raw template and substitution
rules remain exact; presentation is not moved into semantic identity.

**Unsupported/deferred:** FilesToRunProvider executables/tools and runfiles;
File executables/direct-list tools authenticated by the executable-attribute
association (detected and rejected rather than silently stripped);
named `exec_group`, automatic exec groups, non-`None` toolchain-driven group
selection, and nondefault Starlark-semantics flags; non-`None`
`shadowed_action` and action-introspection providers; callable `resource_set`
and its execution-time resource dictionary; list-valued
`run_shell(command=...)` when Bazel 9's default
incompatible flag is disabled; directory/tree inputs and expansion;
execution-time shell helper scripts and unused-input pruning; spawn param-file
materialization; aquery, ActionKey, REAPI, or execution projection of typed
Spawn; and C++-specific action families. Every non-`None` deferred value fails
before publication; existing REAPI/InputTree/execution gates remain closed.

## Frozen ownership and implementation

### One typed invocation and one common envelope

Replace `SpawnSpec.executable` with one closed launcher union:

```text
RetainedSpawnInvocation = Executable(SpawnExecutable)
                        | Shell { command: CompactString, pad_dollar_zero: bool }

SpawnSpec = {
  invocation,
  command_line: RetainedCommandLine,
  inputs: ArtifactInputs,
  tools: ArtifactInputs,
  outputs: Arc<[ActionOutput]>,
  unused_inputs_list: Option<AnalysisArtifact>,
  environment: RetainedActionEnvironment,
  execution_requirements: CanonicalStringMap,
  mnemonic: CompactString,
  progress_message: Option<CompactString>,
}
```

There is no retained resource field in this non-callback packet. The public
parameter is bound as `StarlarkCallable | None`: omitted and explicit `None`
share Bazel's fixed default, a direct dictionary/non-callable fails in
starlark-rust binding, and an admitted callable value is rejected as the first
unsupported callback-owned step before any publication. A later execution
packet owns callback retention, `(os_name, input_count)` invocation, dictionary
validation, and resource values together.

Both evaluator methods build an evaluator-borrowed request and call the same
analysis sink. The sink lowers all values, filters/merges execution
requirements, validates deferred fields, constructs the complete `SpawnSpec`,
then acquires the short action-registry lock exactly once. `run_shell` no
longer publishes the legacy `ActionKind::RunShell` payload. Keep legacy helpers
only where unrelated scaffolding still consumes them; no evaluator path may
publish a second Spawn owner.

Follow Bazel's admitted method-body ordering after starlark-rust binding:
command-line values first; run executable or run-shell command next; then
inputs, outputs, unused list, tools, mnemonic, progress message, environment,
execution requirements, default-context selectors, shadow/resource policy,
and finally publication. Validation failure leaves no declared action.

### Producer-owned executable provenance, tags, and default action context

Before evaluator entry, derive immutable executable-Artifact sets from the
already-prepared configured dependencies whose attribute declaration has
`executable=True`: one root-rule set and one set per subrule identity for its
hidden dependencies. This is the bounded Slug equivalent of Bazel's
context-local `StarlarkAttributesCollection` association and is
configuration-result state, not a filesystem lookup or global registry. A
root dependency must not affect a subrule set, and one subrule's hidden
dependency must not affect the root or a sibling. Add only the executable
Artifact identity; do not copy or synthesize FilesToRun or runfiles payloads.

For `run(executable=File)`, direct sequence File entries in `tools`, and every
File leaf of a top-level `tools=depset(...)`, consult the current evaluator
call scope's set before retaining the action. Visit top-level depset leaves as
validation scratch through the accepted dense traversal; do not flatten,
rebuild, or replace the retained depset node. An absent entry proves the
admitted no-associated-runfiles case and retains the File normally. A present
entry fails closed as the deferred FilesToRun category; it must never publish
the Artifact alone. In contrast, a depset that is itself one member of a tools
sequence remains a transitive tool depset without per-element association
lookup, matching Bazel's branch ordering. The evaluator request carries
a closed root-or-specific-subrule identity so later FilesToRun work can keep
Bazel's context-local lookup and distinct subrule rejection without replacing
the common Spawn owner.

The loaded rule's already-resolved builtin `tags` attribute is the natural
producer for tag-derived execution requirements. Before evaluator entry,
extract only legal Bazel execution tags and give the synchronous sink one
immutable `CanonicalStringMap`. At action registration, filter the explicit
dict, then add missing tag keys with empty values. Do not read the filesystem,
environment, CLI globals, or reconstruct tags from `ctx.attr` strings.

The existing `ConfiguredActionOwnerContext` remains the sole selected-platform
and toolchain owner. This packet accepts only its default group. Explicit
`exec_group=None` and `toolchain=None`/omission select that context. A Label or
string toolchain value may be validated only where the existing canonical
label/package-context owner can do so without guessing; otherwise fail closed
and leave that form with automatic-exec-group work. Never synthesize a named
context, infer one from tool paths, or add a command-side platform lookup.

`input_manifests` is type-checked and discarded exactly as Bazel documents;
it never enters equality. A non-`None` shadowed action or callable resource set
is rejected before any evaluator value could cross the sink boundary.

## Request, revision, and memory behavior

No new DICE key, observed input, environment read, filesystem read, async task,
cache, interner, or global registry is added. Rule tags, context-scoped
executable-attribute provenance, configuration action environment, selected
default action context, Files, depsets, and target owner are already inputs of
the configured-analysis computation, so changes
invalidate through existing keys and structural result equality.

Evaluator requests, dictionaries, and top-level tool-depset association walks
are phase scratch. The sink copies them before return. Retained invocation,
command-line recipe, inputs/tools, outputs, unused-input artifact, environment,
requirements, mnemonic, and progress template are immutable configured-analysis state and
participate in equality/cutoff. Rendering vectors and validation maps are
action-call scratch. No `Value`, callable, heap, mutable dict/list, mapping
closure, call token, or lock guard enters `ActionSpec`. Cancellation before
the single registry publication leaves no partial action; concurrent requests
share only immutable configured inputs and retain no transfer-owned task.

## Evidence, allowlist, caps, and stops

Adapt the named Bazel tests into existing Rust integration tests; no copied
workspace fixture is needed. Each adaptation records its source method in the
test name/comment and compares structured semantics or exact diagnostics, not
Java object identity. Callback/resource execution and FilesToRun/runfiles
expansion,
shadowed actions, named exec groups, helper-script thresholds, and disabled
incompatible flags are skipped because their owning phases/categories are
explicitly deferred, not because their behavior is assumed.

Production files:

- `app/slug_loading_v2/src/subrule_invocation.rs`;
- `app/slug_analysis_v2/src/{dice.rs,starlark_rule.rs}`; `dice.rs` may only
  forward already-resolved executable-attribute provenance into prepared
  analysis state and may add no DICE key or computation;
- `app/slug_build_api_v2/src/actions/{spec.rs,ctx_actions.rs,registry.rs,mod.rs}`
  and `app/slug_build_api_v2/src/lib.rs`;
- `app/slug_reapi_v2/src/{command.rs,input_tree.rs}` only if exhaustive
  fail-closed matching requires a compile adapter.

Proof files:

- `app/slug_build_api_v2/tests/actions.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs`, `tests/subrule.rs`, and the
  exact mechanical `SpawnSpec` constructor adapter in
  `tests/configured_target.rs`;
- focused loading tests colocated in `subrule_invocation.rs` only when binding
  behavior cannot be proved through analysis;
- `app/slug_reapi_v2/tests/reapi.rs` only for the existing typed rejection.

Plans may touch this manifest, canonical Live Status, Stage 6, and Stage 9.
Do not touch the parked loading proof. Add no crate dependency, DICE key,
starlark-rust parser/evaluator change, provider/runfiles/action-introspection
owner, exec-group topology, callback retention/evaluation, execution behavior,
fallback, JVM, Java helper, donor code, or C++/FDO branch.

Cap added Rust at 950 production, 800 proof, and 1,750 total lines. No touched
production file may cross 2,000 lines and no changed function may cross 150
lines; factor small validation/lowering helpers within the allowlist rather
than centralizing other builtins. `spec.rs` remains the closed typed action
schema, `starlark_rule.rs` the synchronous lowerer, and
`subrule_invocation.rs` the evaluator ABI. `REPLAN` before exceeding a cap,
adding a production file, duplicating Spawn state, retaining an evaluator
value, moving presentation into identity, bypassing configured tag/environment
owners, or requiring any deferred category.

Focused proof must discriminate:

1. run and run-shell publish the same typed payload, preserve every output and
   mixed command-line segment, and differ structurally by invocation kind;
2. shell `$0` padding follows top-level arguments-sequence emptiness, including
   an empty-rendering Args, and default/list command behavior matches Bazel 9;
3. explicit inputs/tools/depsets, executable, unused-input File,
   mnemonic, progress template, environment, and requirements each affect
   equality while ignored manifests and equivalent defaults do not;
4. legal/illegal requirement keys, legal/illegal target tags, duplicates,
   explicit-over-tag precedence, stable key order, and A/B/A configured
   analysis cutoff;
5. omitted/explicit-`None` resource values are equal, direct dictionaries and
   other non-callables fail in binding, and callable rejection occurs before
   publication;
6. locally declared/unassociated File executable and direct-list tool forms
   publish, current-context executable-attribute-associated Files and
   FilesToRun-like values fail closed without partial actions, root and sibling
   subrule provenance cannot cross scopes, an associated leaf in a top-level
   tools depset fails closed, and the same leaf in a sequence-nested depset is
   retained without per-element association inference;
7. `None`/omitted default-context fields are equal, while named group,
   shadowed action, and other deferred forms fail closed without partial
   actions;
8. post-registration list/dict/Args mutation cannot affect retained actions,
   release retains no Starlark value, and compact values implement
   `Allocative`/cheap clone where applicable; and
9. REAPI Command/InputTree/execution still reject the enriched typed Spawn
   before digest/action-plan bytes, with FileWrite/Symlink/ArgsWrite unchanged.

Run serial focused loading/build-API/analysis/REAPI tests, then full
`slug_build_api_v2`, `slug_loading_v2`, `slug_analysis_v2`, and
`slug_reapi_v2` suites. Run `cargo check -p slug_core_v2` as the direct public
dependent; rebuild `slug_cli_v2` only if a CLI smoke is selected. Finish with
`cargo fmt --all -- --check`, `cargo metadata --format-version 1 --no-deps`,
`scripts/v2_archive_status.sh`, `git diff --check`, cap/physical-size
accounting, and parked-file SHA-256 verification.

Independent corrected design review must confirm that the category boundary is
honest, the typed launcher/envelope avoids future builtin churn,
scope-separated executable provenance and tag ownership are producer-correct,
`resource_set` is not misrepresented as a dictionary parameter, validation
ordering is bounded,
deferred fields fail closed, and caps are credible before Rust edits. The R3
focused correction review returned `ACCEPT`. Independent terminal review
is mandatory for the retained public action representation. A material miss is
`REPLAN`; one focused correction may be reviewed before implementation resumes.

The first terminal implementation review returned `REPLAN` only for public
binding order/outer-shape precedence and the omitted mechanical
`configured_target.rs` proof allowlist row. The focused correction/rereview
returned `ACCEPT` under the unchanged production boundary and caps.

Terminal `ACCEPT` updates canonical M7 and Stage 6/9, commits the packet without
the parked proof, and selects the next bounded bootstrap-critical consumer or
the typed standard-provider/exec-group category from actual BCR reachability.
