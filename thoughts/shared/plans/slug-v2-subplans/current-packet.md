# Current Slug V2 Packet

Packet: `WP-6-7A-generic-args-spawn-symlink-category-architecture-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Architecture `ACCEPT`. This packet remains docs-only; it authorizes
only the bounded configured-action-environment implementation prerequisite.

Base: `7b0db03e1`, which terminally accepts the dense retained depset and direct
typed action-input import gate. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Goal and category boundary

Freeze one reusable architecture for the complete **non-callback action
argument/spawn/artifact-symlink category**, then implement it through bounded
successors without C++-specific branches. The architecture must cover:

- evaluator-local `ctx.actions.args()` and ordered `Args.add`, `add_all`, and
  `add_joined` recipes;
- parameter-file policy and Args-backed `actions.write` as members of the same
  retained command-line model;
- one common `run`/`run_shell` spawn envelope for typed executable, arguments,
  inputs, tools, outputs, environment, execution requirements, progress,
  exec-group and toolchain selection;
- regular-file, directory, and unresolved-symlink output declarations plus
  artifact-to-artifact, unresolved-target-path, and authenticated absolute-path
  symlink actions; and
- File path projections required by generic Starlark consumers, beginning with
  `path`, `dirname`, and `basename`.

Authentic rules_cc 0.2.17 FDO is the first discriminator, not the architecture
owner. Its Starlark calls must pass through the same generic surfaces available
to any BCR ruleset. `cc_common` and `cc_internal` remain BCR/private Starlark
consumers and bridges; Slug does not implement C++ rules, parse their source
specially, or encode an FDO action path.

## Authority and audited evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. The audited source identities are:

- `Args.java`: SHA-256
  `ac704917bb3d6814fdb6f642c42d9300d9cac1d6fc624d769d3d41e42225ef1b`;
- `StarlarkActionFactory.java`: SHA-256
  `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`;
- `CommandLineArgsApi.java`: SHA-256
  `18e3825616f147cdcd83b60444dfc8b961c971a9aec8f7aff4aed74226e1cdf6`;
- `StarlarkActionFactoryApi.java`: SHA-256
  `0545fcc9cccd67eef47f0a2dab01388635ba472fc1c484e37b64e03c11276668`;
- `ActionEnvironment.java`: SHA-256
  `8bca177613e8ee21181728e81b8ae04455631ab8ae91abb05b648828cb555ef5`;
- `BazelRuleClassProvider.java`: SHA-256
  `a7de1ba5a700468ead269865f2563378ea0851d3430844ee6491591e52fd3d91`;
- `CoreOptions.java`: SHA-256
  `89835ed74107b21f7c51b4723e16be8b96b3c1bf43855fc63220b1dd21f5c67a`;
- `FragmentOptions.java`: SHA-256
  `b796aff8846c477982775743833b64a5da2817333e8a992f7f222cdd38f423d4`;
- `builtin_exec_platforms.bzl`: SHA-256
  `b61da947cdbd18f1d12411a057c3b88b26fff399e80d6f903e8d88eb4215956a`;
- `TargetUtils.java`: SHA-256
  `6cbb31aa7a1215f56585760cfcdf76c77776093ee97c424529d86e542535dcce`;
- `PathFragment.java` and `UnixOsPathPolicy.java`: SHA-256
  `f380d5245e989630cefbd3ca55663a2ed62483497a124659a49277808a6d1029`
  and `2dbaf1578f1f4cba085b156383cbbfa3205497f3f5cab438ea7b50506412f1db`;
- `OptionsUtils.java` and `ShellConfiguration.java`: SHA-256
  `4fbee7881d8c9b8fdc746c196b85957ec21e293724a82842c4cc512fb04817b4`
  and `65906f2f625bf4c8136c60dce95fa0386454153f365b11ae2bc654a38349779e`;
- `SymlinkAction.java`: SHA-256
  `3579cdfa2b2eb7c96b040e34f4c2774e3e684725f940e7c7facb940862a0f7ce`;
  and
- `CcStarlarkInternal.java`: SHA-256
  `143e7e4f63deac9f65ca4e85e2e4d84f3fedf6560428e1dc6f975b2255424f53`.

Those files fix binding and validation order, Args mutation/snapshot behavior,
scalar and vector formatting, parameter-file policy, command-line segmentation,
typed inputs/tools, output nonemptiness, executable forms, spawn metadata,
configuration/default action-environment construction, target-to-Exec
`host_action_env` rewriting, map canonicalization, host-sensitive
`PathFragment` normalization, symlink variants and the private absolute-path
action.
`StarlarkRuleImplementationFunctionsTest` Args tests, `StarlarkRuleContextTest`
spawn tests, `StarlarkSubruleTest` tool/depset tests and `bazel_symlink_test.sh`
are pinned-source regression authorities. Add a fresh Bazel oracle only where
those sources and existing fixtures do not discriminate a claimed observable.

The authenticated rules_cc 0.2.17 inputs are:

- `cc/private/rules_impl/fdo/fdo_context.bzl`: SHA-256
  `91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`;
  and
- `cc/private/cc_common.bzl`: SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`.

The live FDO source calls chained `Args.add` with strings and Files, reads File
`path`/`dirname`/`basename`, registers `run` with File executables, Args
arguments, list inputs and `tools=[all_files]` where `all_files` is a depset,
sets `use_default_shell_env=True` in both LLVM profile actions, registers
artifact-to-artifact symlinks, and reaches `cc_common.absolute_symlink` for an
authenticated absolute profile path.

Bazel's retained action environment is a fixed key/value map plus a set of
client-inherited names. `use_default_shell_env=True` composes the configured
environment with the action `env` dict after removing overridden inherited
names; false uses only the action dict. `--action_env` is last-operation-wins
per name, and the Exec transition replaces target `action_env` with
`host_action_env`. Environment equality is map/set semantic. Execution
requirements are filtered and copied into an `ImmutableSortedMap`. A string
executable is normalized through `PathFragment.create`; the private absolute
symlink route does the same and `SymlinkAction.toAbsolutePath` rejects a
non-absolute result.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
architecture and optimization guidance only. Its evaluator-owned Args recipe,
action-time snapshot, typed command-line segments, distinct direct/transitive
tool inputs, symlink variants and explicit invocation-finalization boundary are
useful design lessons. Copy no Zig code, names, layout, errors, tests,
fingerprints or behavior. Where Zabel and Bazel differ, Bazel 9.2 wins.

starlark-rust from Buck2 remains Slug's parser, binder, evaluator, heap, `set`
implementation and method-dispatch substrate. Add no parser, binder, custom
`set`, Buck2 `transitive_set`, or C++-specific evaluation path. Buck2-derived
utility guidance governs compact immutable slices, `Arc`/`Dupe`, interning and
`Allocative`; it does not authorize a second semantic representation.

## Compatibility classification

**Exact in the configured-action-environment prerequisite:** native option
conversion and last-operation-wins `action_env`/`host_action_env` semantics;
the strict option including its old name; default runfiles/shell-option state;
strict/default shell environment construction for the authenticated Host;
target-to-Exec environment rewriting; fixed-map/inherited-name composition
with an action `env` dict; explicit override precedence; and canonical
map/set-semantic equality. Rust Host OS, server-environment and client-
environment observations retain their approved Slug-native observation
boundary even when the resulting Bazel environment algorithm is exact.

**Exact in the first action implementation successor:** active-context and
receiver checks; `ctx.actions.args()` identity and chaining; `Args.add` with
one value or arg-name/value, strings, integers and Files; one-`%s` formatting;
rejection of vectors and directory Files; action-time snapshot isolation;
`dirname`/`basename` transformation; `actions.run` outputs, list/depset inputs,
list/depset/nested-depset tools, PathFragment-normalized string and typed File
executables, mixed strings/Args argument order, mnemonic, progress message and
effective `use_default_shell_env=True`; the default exec group already selected
by retained analysis; artifact-to-artifact symlink; normalized authenticated
`cc_internal.absolute_symlink`; and typed action-input/artifact/argv projection
order required by authentic FDO.

**Exact through later successors under this same architecture:** non-callback
`Args.add_all` and `add_joined` over sequences and depsets, formatting,
`before_each`, `terminate_with`, `omit_if_empty`, `uniquify`, and
`expand_directories=False`; `use_param_file`, all three parameter-file formats,
Args-backed `actions.write`; complete non-callback `run` and `run_shell`
environment/execution-requirement/toolchain/exec-group forms; direct
FilesToRun/runfiles tools; multiple outputs; regular directory declarations;
explicit `enable_runfiles` and `shell_executable` command states; and unresolved
target-path symlinks when the admitted configuration option is owned. Each
successor must remain fail-closed until its proof lands.

**Slug-native:** Rust storage layout, compact indexes, builder scratch,
invocation-finalization mechanics, structural action identity, allocation and
memory counters, Rust Host observations, and the collision-safe separation of
semantic action identity from display paths, Bazel ActionKey and REAPI digests.
Generated-artifact `File.path` bytes, their `dirname`/`basename` results and
their rendered argv bytes are Slug-native because exact Bazel configuration and
output-directory spelling remains M9. The path-property algorithm and typed
artifact relationship are exact; source-artifact paths remain exact where the
accepted source graph already owns their spelling.

**Unsupported/deferred:** `map_each`, `allow_closure`, DirectoryExpander and
execution-time tree-artifact expansion; `resource_set` and `shadowed_action`;
unused-input-list execution behavior; exact Bazel ActionKey; exact Bazel output
configuration/path bytes; inherited client-environment value resolution and
execution invalidation until the spawn execution successor; REAPI/CAS digests
for newly admitted action shapes; runtime paramfile spilling decisions;
unresolved-symlink execution on hosts or remote executors; native C++ action
constructors; and other action kinds.
Supplying a deferred callback or expansion form fails at its documented public
or action-registration boundary and never leaves evaluator state retained.

## Frozen ownership architecture

### Evaluator-local Args builder

`StarlarkArgs` is a mutable starlark-rust value owned only by the active
configured-analysis evaluator. It contains an ordered recipe of scalar,
add-all and add-joined calls plus parameter-file policy. A scalar recipe keeps
an optional unformatted arg name, one typed scalar and an optional validated
format. A vector recipe keeps its source kind, optional arg name, transform
options and no callback in admitted non-callback slices.

Sequence membership is snapshotted when `add_all`/`add_joined` is called;
later list mutation does not change the Args recipe. A depset source retains
the evaluator-local Starlark depset occurrence without flattening until action
registration. Empty-source omission happens after Bazel's binder/type/format
validation in the pinned order. Every mutator returns the same Args occurrence.
Freezing or using the value outside its active implementation rejects exactly
at the Starlark boundary.

No `Value`, `FrozenValue`, heap, evaluator, callable, call token, mutable vector
or repository-mapping object may enter `ActionSpec`, a configured result, a
DICE key or execution state.

The crate boundary is explicit. `slug_loading_v2` owns the starlark-rust
methods and their Bazel-ordered binding checks, but it cannot lower retained
analysis values without inverting dependencies. `slug_analysis_v2` therefore
installs one invocation-owned `AnalysisActionSink` in
`AnalysisEvaluationContext`. The loading binding passes a synchronously
borrowed request containing Args/File/depset values to that sink; the analysis
implementation authenticates the call token and configured owner, completes
all lowering, and returns an owned registration request. The trait/callback
may be higher-ranked over the evaluator lifetime but the installed sink itself
is owned and contains no evaluator reference. `slug_build_api_v2::CtxActions`
accepts only the owned request.

This is the sole Starlark-to-action finalization boundary for ordinary rule and
subrule contexts and for the private `cc_internal.absolute_symlink` bridge.
Do not teach the loading crate an analysis-value clone, expose raw store indexes
through the callback, or add a second C++ action sink.

### Configured and per-action environment ownership

The existing structural `SlugConfiguration` remains the sole owner of native
option values. Its Host conversion input is extended with one compact,
immutable `ActionEnvironmentHost` observation containing the Bazel Host OS and
only the server-environment facts required by Bazel's shell-environment
algorithm. That observation participates in configuration canonical bytes and
equality. Missing Host facts fail closed; no action-registration code reads
ambient `std::env`, guesses an OS from a path, or consults a second option
store.

One configuration projection consumes `action_env`, `host_action_env`,
`incompatible_strict_action_env`, `enable_runfiles` and `shell_executable` from
that same option vector. The existing target-to-Exec transform must also copy
`host_action_env` to `action_env` before this projection can be exact. It
produces:

```text
RetainedActionEnvironment {
  fixed: CanonicalStringMap,
  inherited: CanonicalStringSet,
}

CanonicalStringMap = Arc<[(CompactString, CompactString)]> // key-sorted unique
CanonicalStringSet = Arc<[CompactString]>                  // sorted unique
```

Command occurrences retain Bazel's parse/validation order, including unset and
last-operation-wins behavior, but the published map/set identity is independent
of insertion order. The wrappers are V2-owned, `Allocative`, cheap to clone and
constructed with bounded scratch; do not retain `BTreeMap`, a Starlark `Dict`,
or a process-global interner. This applies equally to the action `env` dict and
filtered execution requirements. An eventual exact Bazel ActionKey projection
is a distinct domain and may not turn registration order into semantic action
identity.

The prerequisite admits command mutation only for `action_env`,
`host_action_env` and `incompatible_strict_action_env`/its old name. It consumes
the already-retained default `enable_runfiles=auto` and absent
`shell_executable`; explicit mutations of those two options stay fail-closed
until the complete spawn-envelope successor owns the Host-contextual path
converter and no-form/tri-state command surface. This is an admitted-state
boundary, not a guessed value.

At action registration, `use_default_shell_env=True` composes the configured
environment with the validated action dict: action keys are removed from the
inherited set and action values replace fixed values. False produces only the
action dict. `SpawnSpec` retains the resulting fixed map and inherited-name set,
not the input boolean and dict as competing semantic state.

Inherited *values* are not configuration state. A later spawn-execution packet
must resolve the retained names from one immutable command-ingress client-
environment snapshot, reuse the same captured process snapshot already needed
by repository commands rather than rereading `std::env`, and make resolved
values participate in the REAPI Command digest/action-cache invalidation. Until
that execution owner lands, a spawn containing inherited names may be queried
but fails closed before execution. This does not block authentic default-strict
Linux FDO, whose default action environment has a fixed PATH and no inherited
names.

### One action-time finalization seam

When `run`, `run_shell`, or `write` consumes an Args occurrence, one
`RetainedCommandLineBuilder` snapshots the recipe as it exists at that call.
It lowers Files to `AnalysisArtifact`, labels/primitive scalars to their exact
string projection for the active repository mapping, sequence members to one
compact immutable slice, and depset sources through the accepted
`AnalysisValueLowerer` dense retained owner. A later mutation of the same Args
object affects only later action registrations.

The retained form is an ordered sequence of segments, not one flat argv:

```text
RetainedCommandLine
  = LiteralRun([CompactString])
  | ArgsSnapshot {
      calls: Box<[RetainedArgCall]>,
      parameter_file: Option<ParameterFilePolicy>,
    }
```

`RetainedArgCall` distinguishes scalar, add-all and add-joined recipes.
`RetainedArgSource` distinguishes compact sequence membership from one dense
retained `AnalysisDepset`. Typed File scalars remain artifacts until a consumer
requests execution/display bytes. Literal strings never become artifact
inputs merely because their bytes resemble a path. Adjacent literal arguments
may share one compact segment; Args boundaries and parameter-file policy may
not be merged away.

Expansion is iterative. It preserves call/segment order, Bazel formatting and
stable-first uniquification. A direct argv/aquery consumer may expand without
storing a second retained vector. An execution consumer may choose a paramfile
only after that successor owns the platform limit, while structural action
identity retains the recipe and policy independently of the decision.

### Typed executable, inputs, tools and outputs

Replace raw string path ownership at the Starlark registration seam with typed
retained forms:

```text
SpawnExecutable = Path(NormalizedBazelPath)
                | Artifact(AnalysisArtifact)
                | FilesToRun(retained provider projection)

ArtifactInputs = Box<[ArtifactInputSource]>
ArtifactInputSource = Direct(AnalysisArtifact)
                    | Depset(RetainedArtifactInputs)
                    | FilesToRun(retained provider projection) // tools only
```

`NormalizedBazelPath` is a V2-owned valid-Unicode reproduction of Bazel 9.2
`PathFragment.create` under the retained Host path flavor. It stores only the
normalized compact spelling and absolute/relative classification. It is not
`std::path::PathBuf`, `slug_workspace_v2::NormalizedAbsolutePath`, an artifact
identity or a configured-output token. Repeated separators, `.`/`..` segments
and host separator rules are normalized before publication; normalization
aliases compare equal. String executables may be relative or absolute, so their
wrapper is deliberately distinct from the absolute-only symlink target type.

Inputs and tools remain separate ordered domains. Input accepts either one
sequence or one depset. Tools accepts a sequence or depset and may contain
direct Files, FilesToRun providers, or nested depsets exactly as Bazel admits.
The direct dense depset adapter streams into consumers without a public flat
list. Stable-first deduplication is consumer scratch; no flattened input list
or visited set is retained in the action graph.

All output Files are authenticated against the current configured owner before
registration. Preserve every output and its declared kind in argument order;
the existing one-output convenience API is not the retained ABI. An empty
output sequence rejects before action publication. `AnalysisArtifact` remains
the artifact identity owner; `ActionInput` path/digest is only an execution or
wire projection and must not be the semantic registration representation.

### One common SpawnSpec

`run` and `run_shell` lower through one `SpawnSpec` embedded in `ActionSpec`:

```text
SpawnSpec {
  launcher: Executable | ShellCommand,
  command_lines: Box<[RetainedCommandLine]>,
  inputs: ArtifactInputs,
  tools: ArtifactInputs,
  outputs: Box<[ActionOutput]>,
  environment: RetainedActionEnvironment,
  execution_requirements: CanonicalStringMap,
  mnemonic,
  progress_message,
  exec_group,
  toolchain_selection,
}
```

Shell padding is derived only by the shell launcher and remains the already
accepted Bazel behavior. Environment maps/sets and execution requirements have
canonical map/set identity, independent of Starlark insertion order.
Environment, execution requirements, selected platform/exec group and
toolchain identity participate structurally in configured-result/action
equality. No action compute or callback occurs while holding the shared
`CtxActions` mutex; evaluator values are finalized before the short registry
mutation.

The existing `argv: Vec<String>`, `inputs: Vec<ActionInput>` and
`tools: Vec<ActionInput>` become projections of this owner, not parallel
semantic fields. Do not preserve both representations as competing sources of
truth. REAPI and aquery must consume the typed owner; a missing newly required
projection fails closed.

### Artifact and symlink family

File path properties derive from the typed `AnalysisArtifact` and its declared
output kind; they do not consult the filesystem. The declaration owner
canonicalizes package-relative output paths and rejects cross-package or
conflicting kind declarations before returning a File. Source paths preserve
their accepted exact spelling. Generated paths use Slug's structural
configuration/output token, so the returned `path` and derived
`dirname`/`basename` bytes are explicitly Slug-native until M9; no plan or test
may label those full bytes exact.

Use one tagged retained symlink target:

```text
SymlinkTarget = Artifact {
  input: AnalysisArtifact,
  require_executable: bool,
  use_exec_root_for_source: bool,
} | UnresolvedPath {
  target: CompactString,
  expected_kind: Unspecified | File | Directory,
} | AbsolutePath {
  target: NormalizedAbsoluteBazelPath,
}
```

Artifact targets require matching file/directory kinds and a regular
file/directory output. Unresolved targets require a declared symlink output,
the admitted configuration flag and exact target bytes. Absolute targets are
normalized with the same Host-flavored Bazel path algorithm and reject unless
the normalized result is absolute. They are private, require the authenticated
rules_cc/`cc_internal` allowlist route and a regular-file output, and share only
the structural action owner—not the public unresolved-symlink semantics.
`UnresolvedPath` intentionally retains the public API's target bytes without
converting them into an absolute-path identity. Progress text participates in
the action description but never changes artifact identity.

## Bounded successor sequence

The category is designed once and implemented in this order:

1. `WP-6-7A-configured-action-environment-owner-implementation-r1`: extend
   the existing structural configuration/Host observation with exact
   `action_env`/`host_action_env`, strict environment, default runfiles/shell-
   option state and required BAZEL_SH/PATH/SYSTEMROOT inputs; complete the Exec
   rewrite; and publish the canonical fixed-map/inherited-name projection plus
   per-action dict composition. Explicit `enable_runfiles`/`shell_executable`
   remain unadmitted. Client inherited value resolution remains execution-
   deferred and fails closed there.
2. `WP-6-7A-fdo-basic-args-run-symlink-implementation-r1`: File
   `path`/`dirname`/`basename`, Args scalar calls and snapshot isolation,
   retained command-line segments, typed File/string executable, multiple
   outputs, list/depset inputs, nested depset tools, the accepted effective
   default action environment, artifact symlink and authenticated absolute
   symlink. The real FDO action slice is its discriminator.
3. `WP-6-7A-noncallback-vector-args-paramfiles-implementation-r1`: sequence
   and depset `add_all`/`add_joined`, all non-callback transforms,
   parameter-file policy and Args-backed write. Directory expansion remains
   fail-closed.
4. `WP-6-7A-complete-noncallback-spawn-envelope-implementation-r1`: migrate
   `run_shell` to the same owner and admit remaining non-callback
   execution-requirement, FilesToRun, exec-group and toolchain forms, plus
   explicit `enable_runfiles`/`shell_executable`, with retained platform/Host
   evidence.
5. `WP-6-7A-unresolved-symlink-family-implementation-r1`: own the configuration
   option and complete declared/unresolved target-path semantics plus their
   execution/REAPI boundary.

Every successor gets its own compatibility table, caps, oracle evidence and
terminal review. A later successor may fill a frozen variant; it may not add a
second command-line, input, spawn or symlink owner. A discovered representation
miss is `REPLAN` for this architecture rather than permission for a special
case.

## Environment prerequisite allowlist and caps

Production candidates:

- `app/slug_configuration_v2/src/command.rs` and
  `app/slug_configuration_v2/src/native/{configuration.rs,host.rs,mod.rs}` for
  the three typed native command options, retained Host facts, Exec rewrite and
  canonical action-environment projection;
- `app/slug_commands_v2/src/common.rs` for the already-generic flag admission;
  and
- the existing `app/slug_core_v2/src/runtime/{process_host.rs,dice.rs}` Host
  observation/installation seam only if the retained action Host facts cannot
  be populated through the current value without a new observation. Read
  `docs/developers/dice.md` before any such edit; add no new DICE key or ambient
  read inside a compute.

Proof may touch the directly matching configuration, command, core, server and
analysis transition tests. Plans may touch the canonical plan, Stage 6, Stage 9
and this manifest. Set exact line caps after preflight, not above 600 production
and 750 proof additions. `REPLAN` if exact default environment construction
requires a second configuration store, a new DICE key, a second process-
environment read, or execution-time client values in configured identity.

## First action successor allowlist and caps

Production candidates:

- `app/slug_loading_v2/src/subrule_invocation.rs` for generic Starlark File,
  Args, action bindings and private bridge dispatch;
- `app/slug_loading_v2/src/cc_common.rs` for only the authenticated generic
  `cc_internal.absolute_symlink` method on the existing private module;
- `app/slug_analysis_v2/src/analysis_value.rs` and
  `app/slug_analysis_v2/src/starlark_rule.rs` for evaluator finalization and
  authenticated configured-owner context;
- `app/slug_build_api_v2/src/actions/{spec.rs,ctx_actions.rs,registry.rs,
  reapi_projection.rs,mod.rs}` and `app/slug_build_api_v2/src/lib.rs` for the
  typed retained action owner and projections; and
- only the already-owned generic private-builtin restriction module if the
  existing `cc_common.internal_DO_NOT_USE` source authentication cannot be
  reused unchanged.

Proof may touch existing action, analysis-rule, subrule, provider/materializer
and REAPI-projection test modules plus one bounded Bazel oracle fixture. Plans
may touch the canonical plan, Stage 6, Stage 9 and this manifest.

The first action successor must set conservative classified caps after a line-
level preflight. Architecture review must return `REPLAN` if it cannot remain
below 1,500 production and 1,000 proof additions or if a cohesive source file
would cross its existing split gate. Add no dependency, DICE key, global cache,
process-global interner, parser, executor behavior, C++ rule branch, production
fallback or JVM/Java artifact.

## Evidence contract

The architecture and its successors must prove:

1. `action_env`/`host_action_env` set/inherit/unset and repeat normalization;
   target/Exec distinction; strict/non-strict and runfiles branches; fixed/
   inherited composition; explicit action override; map insertion reorder
   equality; and Host/configuration A/B/A restoration without a second ambient
   read.
2. Args mutation order, same-object chaining, sequence snapshot, action-time
   snapshot and post-registration mutation isolation.
3. Exact scalar/vector binding and validation precedence for admitted forms,
   including empty omission only after validation.
4. Mixed literal/Args segment order; typed File argv rendering with generated
   path bytes classified Slug-native; no path-looking string becoming an input;
   and identical cold/warm expansion.
5. `PathFragment` normalization aliases for string executables, absolute/
   relative distinction, and normalized-absolute rejection for the private
   symlink bridge.
6. One local dense store for a depset argument/tool graph, direct consumption
   by command-line/input sinks, and release after evaluator and action owners
   drop.
7. Inputs versus tools separation, nested depset tools, executable identity,
   every output, environment and default execution-context structural
   participation.
8. `run` and migrated `run_shell` share the retained SpawnSpec rather than
   duplicating parsing or action state.
9. Artifact, unresolved and absolute symlink variants reject mismatched output
   kinds and cannot compare equal merely because rendered target bytes match.
10. No retained evaluator/Starlark values, no lock held across lowering or DICE,
   `Allocative` coverage, and deterministic retained-byte/allocation/operation
   controls against the current raw-vector scaffold.
11. The authenticated rules_cc FDO call chain crosses Args, File projections,
   dense tools depset, run and both admitted symlink routes without a
   `cc_common`, `cc_internal`, FDO, parser or custom-builtin special case.

## Validation and review gate

For the architecture packet, run the archive/root checker, Markdown/diff
checks, staged-only docs allowlist and unrelated-dirty-file audit. For the
environment prerequisite, run focused configuration/command/Host/transition
tests and the named direct dependents serially. For the first action successor,
run focused Args/action/symlink/materializer tests, then serial
`slug_build_api_v2`, `slug_loading_v2`, and `slug_analysis_v2` suites, rebuild
`slug_cli_v2` before any `SLUG_V2_BIN` oracle, and clean stale `slugd` before and
after daemon-sensitive tests.

The first independent review returned `REPLAN`: it found an exact claim for
generated File path bytes despite M9, no owner for authentic FDO's
`use_default_shell_env=True`, insertion-ordered map identity, and raw string
executable/absolute-symlink paths. This correction changes only those findings:
generated path bytes are Slug-native, the configured action environment is an
explicit prerequisite and retained map/set owner, semantic maps are canonical,
and the two path roles use normalized typed wrappers. Focused rereview inspects
this correction against those blockers; any other material representation miss
is a new `REPLAN`.

Focused rereview returned `ACCEPT`: generated artifact bytes are honestly
Slug-native; the bounded prerequisite owns configured action-environment
semantics without retaining client values; and canonical map/set plus
normalized path wrappers provide collision-safe semantic identity. A final
feasibility narrowing admits command mutation only for `action_env`,
`host_action_env` and the strict option while consuming default runfiles/shell
state; explicit mutations of the latter two fail closed until their contextual
converter successor. The same reviewer confirmed that narrowing preserves
authentic default-strict FDO and the architecture `ACCEPT`.

The independent architecture reviewer must answer:

- Is evaluator-local mutation separated from retained immutable command-line
  and action ownership with Bazel's snapshot boundaries intact?
- Does the analysis-owned synchronous action sink solve the loading/analysis
  dependency direction without retaining borrowed evaluator values or adding a
  second rule-specific registration path?
- Can all named non-callback Args forms, run/run_shell, typed inputs/tools and
  three symlink variants inhabit the frozen representation without parallel
  legacy sources of truth?
- Does the environment prerequisite own every configuration/Host input needed
  by authentic `use_default_shell_env=True`, preserve map/set identity and keep
  client inherited values out of configured state?
- Are generated File bytes honestly Slug-native and are executable/absolute
  symlink strings normalized through distinct Bazel-path types?
- Does the first action successor remain a generic FDO-discriminated slice
  rather than a C++/parser special case?
- Are deferred callbacks, directory expansion, paramfile execution and new
  REAPI behavior fail-closed at honest boundaries?
- Are semantic artifact/action identity, display paths, Bazel ActionKey and
  REAPI digests distinct?
- Are ownership, lifecycle, locks, memory accounting and successor caps
  sufficient to prevent future churn or hidden evaluator retention?

Only `ACCEPT` authorizes the first Rust successor. `REPLAN` is mandatory if the
design retains evaluator state, flattens depsets into the semantic action ABI,
stores typed artifacts only as paths, duplicates spawn owners, conflates
unresolved and absolute symlinks, retains insertion order as semantic map
identity, guesses an execution-time decision, adds a C++-specific branch, or
claims parity for a deferred form.
