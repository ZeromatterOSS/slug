# Current Slug V2 Packet

Packet: `WP-6-7A-fdo-basic-args-run-symlink-implementation-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Implementation `ACCEPT`. R1 terminal review returned `REPLAN`, the
independent R2 architecture review accepted the correction contract, and the
independent R2 terminal review accepted the completed implementation.

Base implementation: `71d34affa`; accepted R1 design commit: `265581695`. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

Immediate predecessor: commit `71d34affa` terminally accepted the structural
configured-action-environment owner. Commit `94fd24e9f` accepted the complete
non-callback Args/spawn/symlink category and selected this bounded successor.

The first pre-review accepted the single build-API-to-configuration dependency
and the synchronous borrowed action sink, but returned `REPLAN` for four
bounded contract misses: Args was tied to a rule/subrule call token rather than
the evaluator heap; Windows short-path candidates lacked Bazel's filesystem
observation; action equality would have inherited occurrence-pointer depset
equality; and the authentic rules_cc absolute-symlink route first calls the
missing generic `_cc_internal.check_private_api`. This correction changes only
those four contracts.

R1 then implemented that accepted contract and passed its focused and full
serial configuration, build-API, loading, analysis and REAPI suites, CLI build,
formatting, source ledger and cap checks. Its authentic rules_cc discriminator
proved the generic action architecture, but also proved that the wrapper used
to reach the action body had bypassed two older generic defects. Terminal
review therefore returned `REPLAN`, not a focused implementation correction:

- file targets synthesize `DefaultInfo.files` as `Depset<String>` and analysis
  rematerializes those leaves as Starlark strings, so unchanged authentic
  `rules_cc` fails at `artifact.short_path`;
- configured rule execution installs `AnalysisEvaluationContext`, which does
  not retain the loaded definition's recursive source-identity manifest, so
  configured `check_private_api` cannot authenticate its caller;
- the action A/B/A proof compared values but did not count the parent DICE
  recomputations needed to prove publication cutoff; and
- `host_package_load_tests.rs` was missing from the proof allowlist while the
  implementation had already consumed 1,498 of 1,500 production lines.

R2 corrects those prerequisites at their general owners. Independent review
accepts the provider recursion, phase-neutral source context, parent-key cutoff
proof, allowlists and revised caps. It does not introduce
an FDO adapter, rules_cc branch, source-file-only wrapper, second provider
representation, or relaxed proof. Correct only the frozen R1 action candidate
within the provider, context, identity and cap contract below.

## Observable result

An ordinary rule or subrule can construct scalar `ctx.actions.args()` values,
register a typed `ctx.actions.run()` with every output plus list/depset inputs
and list/depset/nested-depset tools, and register artifact symlinks. The existing
authenticated rules_cc private bridge can register a normalized absolute-path
symlink through the same generic action sink. File `path`, `dirname`, and
`basename` support the real rules_cc FDO call shapes.

Every configured target's `DefaultInfo.files` retains the existing dense
`AnalysisDepset` topology with typed `AnalysisArtifact` leaves. This includes
source files, generated-file aliases and files declared by Starlark rules; a
target-shaped dependency therefore rematerializes the same Starlark `File`
category as an `allow_single_file` dependency. Configured evaluation also
receives the loaded implementation's immutable recursive source manifest, so
all source-sensitive builtins can resolve loading and configured callers
through one phase-neutral accessor.

The retained result has one immutable command-line recipe, one typed spawn
owner, one typed symlink owner, and the already-accepted effective action
environment. The real rules_cc 0.2.17 FDO route is the end-to-end discriminator;
it is not a C++ implementation path. Buck2 starlark-rust continues to own the
parser, binder, evaluator, heap, method dispatch, and `set` builtin.

This packet does not execute the new actions and does not claim their Bazel
ActionKey, REAPI Command/Action digest, or generated output-path bytes.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Reuse the accepted source audit from `94fd24e9f`:

- `Args.java` SHA-256
  `ac704917bb3d6814fdb6f642c42d9300d9cac1d6fc624d769d3d41e42225ef1b`
  and `CommandLineArgsApi.java` SHA-256
  `18e3825616f147cdcd83b60444dfc8b961c971a9aec8f7aff4aed74226e1cdf6`
  fix scalar binding, validation, mutation, formatting, and snapshot behavior.
  `Args.newArgs` receives evaluator `Mutability`; `MutableArgs` implements its
  freezable contract and every mutation calls `Starlark.checkMutable`. It is
  not tied to a rule/subrule call frame.
- `StarlarkActionFactory.java` SHA-256
  `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`
  and `StarlarkActionFactoryApi.java` SHA-256
  `0545fcc9cccd67eef47f0a2dab01388635ba472fc1c484e37b64e03c11276668`
  fix argument segmentation, typed executable/input/tool/output handling,
  default mnemonic/progress/environment behavior, and symlink validation order.
- `PathFragment.java` SHA-256
  `f380d5245e989630cefbd3ca55663a2ed62483497a124659a49277808a6d1029`,
  `UnixOsPathPolicy.java` SHA-256
  `2dbaf1578f1f4cba085b156383cbbfa3205497f3f5cab438ea7b50506412f1db`,
  and `WindowsOsPathPolicy.java` SHA-256
  `0c6d2354f741fd0fcc166d71c055bd7b3e1b97f12f269d27d8135220e65d93a0`
  fix valid-Unicode path normalization and absolute/relative classification.
- `SymlinkAction.java` SHA-256
  `3579cdfa2b2eb7c96b040e34f4c2774e3e684725f940e7c7facb940862a0f7ce`
  and `CcStarlarkInternal.java` SHA-256
  `143e7e4f63deac9f65ca4e85e2e4d84f3fedf6560428e1dc6f975b2255424f53`
  fix artifact and private absolute symlink forms. `CcStarlarkInternal` also
  fixes custom `(apparent_repo, package_prefix)` allowlist coercion, default
  caller depth one, nonnegative Starlark-function depth selection, and the
  no-enclosing-function success branch.
- `ActionEnvironment.java` SHA-256
  `8bca177613e8ee21181728e81b8ae04455631ab8ae91abb05b648828cb555ef5`
  and the accepted predecessor proof fix the retained fixed/inherited
  environment and per-action composition.
- Pinned `FileConfiguredTarget.java` SHA-256
  `36082a2bbd0c6f7595080c75c85b637683b12df5dcc2171224bf75f5aec4e61d`,
  `DefaultInfo.java` SHA-256
  `749a01fa226ffe32990bbafeb00aee470b9196a80ba06e1cbec6b82f0fa7833e`,
  and `FileProvider.java` SHA-256
  `8456938f29ec193fbd25e3de5375e6ab920c098e5069648d72cb3d590aaeeda2`
  establish that a file configured target retains an `Artifact` and exposes a
  `DefaultInfo.files` depset of Artifacts. Source, generated and rule-produced
  files therefore share one typed provider surface; path strings are not an
  admitted substitute.

Pinned `StarlarkRuleImplementationFunctionsTest` Args tests,
`StarlarkRuleContextTest` spawn tests, `StarlarkSubruleTest` depset-tool tests,
and `bazel_symlink_test.sh` remain the regression authorities. Add a fresh
Bazel oracle only if implementation exposes an observable not discriminated by
those sources and tests.

The authenticated consumer is rules_cc 0.2.17:

- `cc/private/rules_impl/fdo/fdo_context.bzl` SHA-256
  `91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`;
- `cc/private/cc_common.bzl` SHA-256
  `5e6ab737945b487759c9f039c77a066dc65bbe15cf590b566fe86029cc610762`;
- `cc/common/cc_helper_internal.bzl` SHA-256
  `793ab429f8e397df9c486f4c3c7b5c57fae81c8432ba6d08189d65d75676dae1`.

It uses chained scalar `Args.add` with strings and Files; File `path`,
`dirname`, and `basename`; File executables; mixed Args/literal arguments;
list inputs; `tools=[all_files]` where `all_files` is a transitive depset;
multiple declared outputs across action shapes; `use_default_shell_env=True`;
artifact symlinks; and `cc_common.absolute_symlink` for an absolute profile.
The public wrapper invokes `_cc_internal.check_private_api` with the custom
rules_cc allowlist and omitted `depth`, hence Bazel's default depth one, before
it invokes `_cc_internal.absolute_symlink`.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is concept-only peer
guidance for evaluator-owned Args mutation, action-time finalization, typed
segments, direct/transitive tool separation, and distinct symlink variants.
Copy no Zig code, layout, names, diagnostics, fingerprints, or tests. Bazel 9.2
wins wherever Zabel differs.

Buck2-derived guidance selects compact `Arc` slices, `CompactString`, `Dupe`,
and `Allocative`. The accepted dense depset is reused directly. Import no
Buck2 action owner, command line, interner, cache, `transitive_set`, or parser.
The live build API already owns iterative `AnalysisDepset` publication equality
with a shared bidirectional alias map while ordinary `Eq` intentionally uses
occurrence identity; this packet exposes that existing comparator only within
the crate instead of inventing another graph walk.

## Preflight correction to the accepted architecture

The accepted environment owner is public from `slug_configuration_v2`, while
`ActionSpec` is owned by `slug_build_api_v2`; the latter currently has no
configuration dependency and the former has no build-API dependency. Copying
the environment or Host-flavored path type into the build API would create the
parallel semantic representation that `94fd24e9f` forbids.

This packet therefore admits exactly one new acyclic workspace edge:
`slug_build_api_v2 -> slug_configuration_v2`. It may update only the matching
package dependency rows in `Cargo.toml` and `Cargo.lock`. The build API retains
the existing `RetainedActionEnvironment` directly.

The predecessor explicitly reserved promotion of its private Windows
normalizer into a later public `NormalizedBazelPath`. Promote that code into a
configuration-owned, Host-flavored `NormalizedBazelPath` and
`NormalizedAbsoluteBazelPath`; make configured environment construction reuse
the same normalizer rather than cloning the algorithm. A path value retains its
flavor, normalized compact spelling, and absolute/relative class. The absolute
wrapper rejects a normalized relative result. Bazel's Windows normalizer may
resolve any 8.3 short-path candidate with `WindowsPathOperations.getLongPath`,
which is an unavailable filesystem observation at this pure semantic boundary.
Both generic path constructors and configured environment construction
therefore fail closed on every such candidate; they never retain the unresolved
short spelling. Non-short Windows paths remain exact.

`cargo metadata` must prove the edge is acyclic. Any second crate dependency,
second normalizer, copied environment type, or reverse build-API/configuration
edge is `REPLAN`.

## Compatibility classification

**Exact:** evaluator-heap Args lifetime and active-context/receiver checks;
same-object Args chaining;
`Args.add(value)` and `Args.add(arg_name, value, format=...)` for strings,
integers, and regular Files; the single-`%s`/`%%` formatter; vector and
directory-File rejection; action-time snapshot isolation; File basename and
dirname algorithms; run output nonemptiness and owner authentication; sequence
or depset inputs; sequence/depset and nested-depset File tools; typed File or
PathFragment-normalized string executable; literal/Args segment order; default
or supplied alphanumeric mnemonic; progress message; effective false/true
default-shell environment over the accepted configuration owner; artifact
symlink kind/owner checks; authenticated normalized absolute symlink; and the
bounded generic `_cc_internal.check_private_api` custom tuple allowlist,
default/explicit nonnegative Starlark-function depth, caller authorization, and
no-enclosing-function success branch. `DefaultInfo.files` is one retained
dense depset of typed Files for source, generated and declared outputs, with
source/generated configured-target projection and configured caller-source
authentication matching the pinned Bazel category.

**Slug-native:** evaluator-to-analysis callback mechanics; Rust valid-Unicode
path/mnemonic edges; compact storage and allocation accounting; structural
action identity; and generated File `path`/`dirname`/`basename` plus rendered
argv bytes because exact Bazel configuration/output-directory spelling is M9.
The path-property algorithm, source/generated/declared artifact relationship,
and depset topology are exact; only the configured output spelling is
Slug-native.

**Unsupported/deferred:** `Args.add_all`, `add_joined`, callbacks,
DirectoryExpander, directory Files in scalar Args, param files, Args-backed
write, FilesToRun providers, explicit execution requirements, explicit
exec-group/toolchain selection, shadowed actions, resource callbacks,
unused-input lists, full run-shell migration, unresolved target-path symlinks,
client-inherited environment value resolution, execution/materialization of
the new actions, exact ActionKey and REAPI/CAS digests, and exact generated
output bytes. Windows paths containing an 8.3 short-path candidate are also
deferred because exact normalization requires a filesystem observation.
Typed migration of Runfiles files/symlinks/empty filenames,
`OutputGroupInfo`, `DefaultInfo.executable`, and FilesToRun manifest fields is
a separately reviewed standard-provider-category successor. R2 introduces no
new string-backed File field and no adapter between typed and raw
`DefaultInfo.files`; any use that requires a deferred provider File fails
closed. Supplying any other deferred form fails before action publication.

## Frozen implementation ownership

### Typed `DefaultInfo.files` and phase-neutral source context

`DefaultInfo.files` changes from `Depset<String>` to the already accepted
`AnalysisDepset`. Every leaf is validated as `AnalysisValue::Artifact` before
provider publication. Rule implementations retain the exact lowered depset
instead of flattening and rebuilding it. Synthetic file-target providers use
`configured_dependency_artifact`: source targets retain
`AnalysisArtifact::Source`, generated targets retain the producing configured
owner and declared output, and unsupported or ambiguous generated ownership
fails closed. `allow_single_file` consumes that same typed provider rather than
reconstructing an artifact from a path.

`DefaultInfo::from_files` becomes a checked constructor accepting only an
empty or `AnalysisValueType::Artifact` depset. `from_executable` additionally
accepts the typed executable artifact used when it must synthesize the implicit
singleton default-file depset; the admitted legacy executable/path fields do
not become the source of that File. Synthetic target-provider construction is
fallible and propagates missing/ambiguous generated ownership. One shared
phase-scratch artifact path projection serves extension checks, action argv and
the existing bounded run view without retaining a second path beside the
artifact.

The public provider ABI has one representation. It adds no source-only
`DefaultInfo`, string sidecar, flattened cache, new depset graph, or new DICE
key. `DefaultInfo` has an explicit publication comparator that compares its
files through the existing shared `PublicationEqState` and its still-admitted
legacy fields structurally. Ordinary `AnalysisDepset::Eq` remains occurrence
identity. `ProviderValue` routes `DefaultInfo` through that comparator, so a
separately allocated publication-equal topology cuts off while changed values,
order, rows, roots or alias partitions invalidate.

`AnalysisValueMaterializer` rematerializes `DefaultInfo.files` through its
existing iterative typed depset path; it never allocates a Starlark string for
those leaves. The older string-depset materializer remains only for the named
deferred Runfiles and OutputGroup surfaces and is not callable for
`DefaultInfo.files`.

`AnalysisEvaluationContext` gains the loaded rule implementation's immutable
`Arc<[(CompactString, BzlModuleIdentity)]>` recursive source manifest. A single
phase-neutral helper resolves this manifest from either loading
`BzlEvaluationContext` or configured `AnalysisEvaluationContext`; source-aware
builtins use that helper and do not branch on FDO, C++, repository names, or
individual callers. The manifest is immutable invocation context, not a DICE
key, global registry, parser annotation, evaluator-retained semantic value, or
second source owner.

### Evaluator-local Args

`slug_loading_v2` owns a mutable `StarlarkArgs` value containing an ordered
scalar recipe. Each call stores only an optional compact arg name, a compact
string/integer projection or typed `AnalysisArtifact`, and an optional validated
format string. It is a complex mutable value allocated on the active
starlark-rust evaluator heap and uses evaluator-local interior mutability; it
does not retain an `AnalysisCallToken`. Every successful add returns the same
object, and any rule or nested subrule executing in that evaluator may mutate a
passed Args value before an action snapshots it. Freezing rejects. Action
facades remain independently call-token-gated.

No Starlark `Value`, `FrozenValue`, heap, evaluator, callable, call token,
mutable vector, or repository-mapping object may enter `ActionSpec`, a
configured result, DICE, or execution state.

### One synchronous finalization sink

`slug_loading_v2` declares the borrowed request structs and object-safe
`AnalysisActionSink` contract because it owns the Starlark bindings.
`slug_analysis_v2` installs one invocation-owned implementation in
`AnalysisEvaluationContext`. Rule and subrule action facades retain only that
sink, their call token, and context name.

The sink owns package path, configured owner, Host path flavor, configured
action environment, and the shared `CtxActions`. It authenticates every File,
uses `AnalysisValueLowerer` for depsets, snapshots Args, constructs the complete
owned request, and only then takes the short `CtxActions` mutex to register.
Existing declare/write/run-shell calls also cross this sink, although run-shell
keeps its accepted legacy representation until its named successor. No lock is
held during lowering or any DICE computation.

`CcInternalModule` exposes one generic `check_private_api` method. It validates
the supplied sequence of two-string tuples, accepts an integer `depth` defaulted
to one, rejects negative depth, identifies the `depth`-th innermost Starlark
function through starlark-rust evaluator frames (including compiler-inlined
frames), resolves that function's source in the recursive Bzl manifest, and
checks the supplied apparent-repository/package-prefix entries with the
existing repository-mapping rules. Loading and configured calls receive that
manifest only through the phase-neutral context helper above. No enclosing Starlark function succeeds as
in Bazel's execution-callback branch; an unresolved or ambiguous present caller
fails closed. Add the smallest general evaluator accessor needed for this
depth-aware Starlark-function lookup; it is not a parser or rules_cc hook.

The private absolute-symlink method obtains the generic action facade from its
passed rule/subrule `ctx` and submits the same sink request. It relies on the
preceding ordinary Starlark wrapper call to `check_private_api`; it adds no C++
sink, action branch, FDO branch, or duplicated allowlist.

### Retained command line and action owners

`slug_build_api_v2` owns immutable, `Allocative` retained values:

```text
RetainedCommandLine = Arc<[RetainedCommandLineSegment]>
RetainedCommandLineSegment = LiteralRun(Arc<[CompactString]>)
                           | ArgsSnapshot(Arc<[RetainedScalarArg]>)
RetainedScalarArg = { arg_name, value: String|Artifact, format }

SpawnExecutable = Path(NormalizedBazelPath) | Artifact(AnalysisArtifact)
ArtifactInputSource = Direct(AnalysisArtifact) | Depset(RetainedArtifactInputs)
ArtifactInputs = Arc<[ArtifactInputSource]>

SpawnSpec = {
  executable,
  command_lines,
  inputs,
  tools,
  outputs,
  environment,
  execution_requirements,
  mnemonic,
  progress_message,
}

SymlinkTarget = Artifact {
  input,
  require_executable,
  use_exec_root_for_source,
} | AbsolutePath { target: NormalizedAbsoluteBazelPath }
SymlinkSpec = { output, target, progress_message }
```

An `ActionSpec` payload is a closed tagged union of legacy, Spawn, and Symlink
state. Spawn and Symlink contain no populated legacy argv/input/tool/env vectors;
their expansion methods derive display/aquery projections from the typed owner.
Existing FileWrite and the temporarily legacy run-shell path keep their current
payload. No action contains two populated semantic representations.

Typed File scalar arguments remain artifacts until projection. A string that
looks like a path never becomes an input. Args boundaries remain visible; only
adjacent literal strings may share a segment. Inputs and tools remain separate
ordered domains. Dense depsets are retained, never flattened into the action
ABI; stable-first flattening is consumer scratch.

Artifact symlinks accept a regular File input and regular File output in this
packet, with the later directory form remaining deferred until generic
directory declaration is admitted. Absolute symlinks accept an authenticated
regular output and no artifact input. The variants cannot compare equal merely
because their rendered target strings match.

`AnalysisDepset::Eq` remains occurrence identity for evaluator materialization
and hash-map lookup. The existing publication comparator becomes a crate-owned
helper available to action values without changing that ordinary `Eq` contract.
`SpawnSpec` has manual publication equality: it compares all ordinary fields
structurally and compares the ordered input/tool depset roots through one shared
publication state, preserving order, node rows, values, and alias partitions.
`ActionSpec` has manual equality that dispatches the closed payload and uses
Spawn publication equality; legacy and Symlink equality remain structural.
Each action is an independent semantic publication unit, so sharing a depset
allocation across two otherwise independent actions is memory topology rather
than action identity.

REAPI projection detects Spawn/Symlink payloads and returns a typed unsupported
error before producing bytes. FileWrite remains unchanged. Aquery/display may
use a non-digest retained projection, but this packet does not widen the
accepted M5 FileWrite formatter surface.

## Request, revision, and memory behavior

Args mutation and borrowed action requests are evaluator/phase scratch and die
with the invocation. Finalization publishes atomically: any binder, owner,
path, depset, environment, or registry failure leaves no partial action.
Post-registration mutation changes only later snapshots.

Command-line segments, artifacts, dense depsets, paths, environment, SpawnSpec,
SymlinkSpec, and ActionSpec are immutable configured-analysis-retained state.
They participate structurally in configured-result equality and DICE cutoff
through the existing result owner. Separately allocated but publication-equal
action depsets cut off; a row, value, order, root order, or alias-partition
change invalidates. The proof injects separately allocated A1/A2, B, and A3/A4
results below an instrumented parent DICE key: A2 and A4 leave the parent count
unchanged, B increments it, and restoration A3 increments it exactly once.
Scratch used for normalization, validation, formatting,
deduplication, equality, and projection is released after the call. There is no
service cache, global interner, async transfer, callback, eviction policy,
ambient environment read, filesystem read, or new DICE key.

Concurrent requests retain independent evaluator-local Args and sinks; they may
share only already-immutable configuration/depset values. Cancellation before
registration publishes nothing; ordinary configured-result cancellation and
release semantics remain unchanged.

## Allowlist and caps

Production files:

- `Cargo.lock`;
- `app/slug_configuration_v2/Cargo.toml` only if metadata requires no change
  beyond formatting (normally untouched);
- `app/slug_configuration_v2/src/native/action_environment.rs`;
- `app/slug_configuration_v2/src/native/path.rs` (new),
  `app/slug_configuration_v2/src/native/mod.rs`, and
  `app/slug_configuration_v2/src/lib.rs`;
- `app/slug_build_api_v2/Cargo.toml`;
- `app/slug_build_api_v2/src/analysis_value.rs` for the crate-owned
  publication-equality helper and one phase-scratch artifact-path projection;
- `app/slug_build_api_v2/src/providers/mod.rs` for the sole typed
  `DefaultInfo.files` owner and its publication comparator;
- `app/slug_build_api_v2/src/actions/{spec.rs,ctx_actions.rs,registry.rs,reapi_projection.rs,mod.rs}`
  and `app/slug_build_api_v2/src/lib.rs`;
- `starlark-rust/starlark/src/eval/runtime/{evaluator.rs,cheap_call_stack.rs,inlined_frame.rs}`
  only for the general depth-aware Starlark-function caller accessor;
- `app/slug_loading_v2/src/builtin_restriction.rs`;
- `app/slug_loading_v2/src/subrule_invocation.rs` and
  `app/slug_loading_v2/src/cc_common.rs`;
- `app/slug_analysis_v2/src/analysis_value.rs` and
  `app/slug_analysis_v2/src/starlark_rule.rs`;
- `app/slug_analysis_v2/src/dice.rs` and `app/slug_analysis_v2/src/subrule.rs`
  only for generic source/generated provider construction and typed-file
  validation/consumption;
- `app/slug_analysis_v2/src/result.rs` only for a typed non-digest projection;
- `app/slug_core_v2/src/runtime/dice.rs` only for mechanical phase-scratch
  inspection of the now-typed default file in the existing bounded run view;
- `app/slug_reapi_v2/src/{command.rs,input_tree.rs}` only for explicit
  fail-closed handling of the new payloads.

Proof files:

- `app/slug_configuration_v2/src/native/tests.rs` or colocated path tests;
- `app/slug_build_api_v2/tests/{actions.rs,providers.rs}`;
- focused starlark-rust evaluator tests colocated with the caller accessor;
- `app/slug_loading_v2/src/{builtin_restriction_tests.rs,host_package_load_tests.rs}`;
- `app/slug_analysis_v2/tests/{configured_target.rs,starlark_rule.rs,subrule.rs}`;
- `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` only if an
  existing typed-run assertion requires a mechanical correction;
- `app/slug_reapi_v2/tests/reapi.rs`;
- at most one new oracle fixture containing at most 6 regular files and 220
  newline-counted text lines, only if the accepted source evidence leaves a
  demonstrated gap.

Plans may touch this manifest, the canonical Live Status, Stage 6, and Stage 9.
Do not touch the parked loading proof.

R1 measured 1,498 production and 735 proof lines before this correction. R2
caps the complete candidate at 1,850 added production Rust lines, 1,150 added
proof Rust lines, and 3,000 total added Rust lines. Cargo and plan lines are
counted separately. No otherwise-small touched production file may cross 2,000
lines. The pre-existing large canonical owners
`app/slug_analysis_v2/src/dice.rs` and
`app/slug_core_v2/src/runtime/dice.rs` may add at most 80 and 20 net lines
respectively; do not relocate their logic merely to evade those caps.
`evaluate_loaded_rule` receives typed-provider and manifest wiring only; action
lowering remains in the focused sink owner. `REPLAN` before exceeding a cap or
adding another production file.

Add no dependency beyond the one internal build-API-to-configuration edge; no
new DICE key, cache, process-global interner, parser, executor support, C++ rule
branch, production fallback, JVM, Java helper, or copied donor code.

## Evidence contract

Implementation must prove all of the following:

1. Args same-object chaining, call order, one/two positional forms, integer and
   File conversion, valid/invalid `%s`/`%%`, vector and directory rejection,
   action-time snapshot, and post-registration mutation isolation.
2. File path/basename/dirname including root-parent behavior, with generated
   bytes explicitly asserted as Slug-native. Source, generated and declared
   `DefaultInfo.files` materialize as typed Files, preserve dense topology and
   never expose strings; invalid non-Artifact leaves fail before publication.
3. Unix and non-short Windows path normalization aliases,
   flavor/absolute distinction, above-root and relative absolute-symlink
   rejection, exact rejection of every Windows 8.3 candidate without a
   filesystem read, and unchanged configured Windows action-environment cases
   through the promoted normalizer.
4. Mixed literal/Args segment order, typed artifact argv projection, and no
   path-looking literal promoted to an input.
5. Every output and its order; empty/cross-owner outputs reject atomically.
6. List versus depset inputs, separate tools, a nested tools depset, dense
   topology sharing, cold/warm identical projection, and release after all
   evaluator/action owners drop. One same-DICE parent-compute counter must show
   A1 computes once, separately allocated publication-equal A2 does not
   recompute, B topology/value mutation recomputes once, restored A3 recomputes
   once, and separately allocated equal A4 does not recompute. Exercise both a
   direct action depset and `DefaultInfo.files` publication through the shared
   comparator state.
7. File and normalized string executable identity; default/custom mnemonic;
   progress; false/true default environment; canonical environment equality;
   and structural inequality for every changed field.
8. Artifact and absolute symlink kind, owner, authentication, normalized target,
   progress, input topology, and variant inequality. Generic
   `check_private_api` proofs cover tuple coercion, default depth one, explicit
   depth zero/two, negative rejection, an allowed wrapper/caller pair, a denied
   caller selected above an allowed wrapper, and the no-enclosing-function
   branch. Repeat allowed and denied caller selection inside configured rule
   evaluation, proving that the recursive manifest is handed off rather than
   recovered from paths or repository names.
9. No retained evaluator values, no lock across lowering/DICE, `Allocative` and
   cheap-clone coverage, and no populated parallel raw spawn representation.
10. REAPI Command/InputTree/execution reject the new payloads before producing
    a digest or action plan; existing FileWrite projections remain unchanged.
11. One authenticated rules_cc FDO discriminator crosses File properties,
    chained Args, typed executable, list inputs, the transitive `all_files`
    depset tool, default environment, run, artifact symlink, and absolute
    symlink, including the preceding custom-allowlist check, without any parser,
    FDO, `cc_common`, or C++ action special case. Its `_fdo_optimize` input is
    the direct source file target consumed through
    `target[DefaultInfo].files.to_list()[0]`; no wrapper provider may inject a
    typed File. A parent-created Args is passed to and mutated by a nested
    subrule before snapshot. If the existing external configured-target-shape
    boundary still prevents the public BCR label, the proof may relocate the
    authenticated source without changing the hashed function body, and must
    record that route boundary rather than claiming complete BCR execution.

Record exact commands, exit status, test counts, and any skipped upstream case.
An upstream case may be skipped only for a named deferred form, Java
implementation-detail assertion, obsolete behavior, or stronger existing
coverage.

## Validation and review gates

Before implementation:

- independent architecture review must answer whether the single dependency
  and promoted normalizer preserve ownership/layering;
- it must confirm object safety and evaluator lifetime isolation of the sink;
- it must confirm the tagged payload can host every named later non-callback
  successor without a second owner;
- it must confirm that embedding `AnalysisDepset` in `DefaultInfo` introduces
  no unbounded value recursion, parallel File representation or crate cycle,
  and that manual provider publication equality preserves alias topology;
- it must confirm the source manifest is immutable invocation context available
  to both loading and configured evaluation without a second loader or parser
  annotation; and
- it must confirm evaluator-owned Args lifetime,
  fail-closed short paths, action publication equality, and the generic custom-
  allowlist/depth bridge, then return `ACCEPT`. Independent R2 review returned
  `ACCEPT`; implementation may now proceed within this contract.

After implementation, run serially:

- focused configuration path/environment tests;
- focused build-API provider/action/projection tests;
- focused loading restriction and analysis typed-provider/Args/action/symlink
  tests, including the parent DICE recomputation counter;
- focused REAPI fail-closed/FileWrite regression tests;
- `cargo test -p slug_build_api_v2`, `cargo test -p slug_loading_v2`,
  `cargo test -p slug_analysis_v2`, `cargo test -p slug_core_v2`, and the named
  direct `slug_configuration_v2`/`slug_reapi_v2` dependents;
- `cargo build -p slug_cli_v2` before any `SLUG_V2_BIN` oracle;
- the bounded FDO oracle/smoke if selected, with stale `slugd` cleaned before
  and after;
- `cargo fmt --all -- --check`, `scripts/v2_archive_status.sh`, and
  `git diff --check`.

Independent terminal review is mandatory for this public cross-crate retained
identity. One focused correction is allowed. A second material contract or
implementation correction is `REPLAN`.

Terminal review returned `ACCEPT` with no material findings. The complete
candidate measured 1,849 added production Rust lines, 1,050 added proof Rust
lines, and 2,899 total added Rust lines; the two giant-file deltas were +19 and
+3. Full build-API, loading, serial analysis, configuration, and REAPI suites
passed. Four core failures were reproduced unchanged at the frozen base. The
authenticated rules_cc source ledger passed, and the relocated unchanged FDO
body consumed a direct source target through typed `DefaultInfo.files` and
completed configured analysis. Its `aquery` stopped at the declared unsupported
typed-action projection boundary. The archive checker reported only three
unchanged pre-existing non-V2 thought paths. Formatting, metadata, diff, parked-
file integrity, CLI build, and slugd cleanup checks passed.

This terminal acceptance updates the canonical M7 row and the Stage 6/9
records, then commits the complete packet without the parked proof. The next
packet remains `WP-6-7A-noncallback-vector-args-paramfiles-implementation-r1`;
author its complete packet contract before implementation.
