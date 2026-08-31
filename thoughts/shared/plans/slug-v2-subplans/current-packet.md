# Current Slug V2 Packet

Packet: `WP-6-7A-fdo-basic-args-run-symlink-implementation-r1`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Design `ACCEPT`. The first independent review returned `REPLAN`; the
focused correction rereview returned `ACCEPT`. Rust implementation is now
authorized only within this contract.

Base: `71d34affa`. The unrelated dirty
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

## Observable result

An ordinary rule or subrule can construct scalar `ctx.actions.args()` values,
register a typed `ctx.actions.run()` with every output plus list/depset inputs
and list/depset/nested-depset tools, and register artifact symlinks. The existing
authenticated rules_cc private bridge can register a normalized absolute-path
symlink through the same generic action sink. File `path`, `dirname`, and
`basename` support the real rules_cc FDO call shapes.

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
no-enclosing-function success branch.

**Slug-native:** evaluator-to-analysis callback mechanics; Rust valid-Unicode
path/mnemonic edges; compact storage and allocation accounting; structural
action identity; and generated File `path`/`dirname`/`basename` plus rendered
argv bytes because exact Bazel configuration/output-directory spelling is M9.
The path-property algorithm and typed artifact relationship are exact.

**Unsupported/deferred:** `Args.add_all`, `add_joined`, callbacks,
DirectoryExpander, directory Files in scalar Args, param files, Args-backed
write, FilesToRun providers, explicit execution requirements, explicit
exec-group/toolchain selection, shadowed actions, resource callbacks,
unused-input lists, full run-shell migration, unresolved target-path symlinks,
client-inherited environment value resolution, execution/materialization of
the new actions, exact ActionKey and REAPI/CAS digests, and exact generated
output bytes. Windows paths containing an 8.3 short-path candidate are also
deferred because exact normalization requires a filesystem observation.
Supplying a deferred form fails before action publication.

## Frozen implementation ownership

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
existing repository-mapping rules. No enclosing Starlark function succeeds as
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
change invalidates. Scratch used for normalization, validation, formatting,
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
- `app/slug_build_api_v2/src/analysis_value.rs` only for the crate-owned
  publication-equality helper;
- `app/slug_build_api_v2/src/actions/{spec.rs,ctx_actions.rs,registry.rs,reapi_projection.rs,mod.rs}`
  and `app/slug_build_api_v2/src/lib.rs`;
- `starlark-rust/starlark/src/eval/runtime/{evaluator.rs,cheap_call_stack.rs,inlined_frame.rs}`
  only for the general depth-aware Starlark-function caller accessor;
- `app/slug_loading_v2/src/builtin_restriction.rs`;
- `app/slug_loading_v2/src/subrule_invocation.rs` and
  `app/slug_loading_v2/src/cc_common.rs`;
- `app/slug_analysis_v2/src/analysis_value.rs` and
  `app/slug_analysis_v2/src/starlark_rule.rs`;
- `app/slug_analysis_v2/src/result.rs` only for a typed non-digest projection;
- `app/slug_reapi_v2/src/{command.rs,input_tree.rs}` only for explicit
  fail-closed handling of the new payloads.

Proof files:

- `app/slug_configuration_v2/src/native/tests.rs` or colocated path tests;
- `app/slug_build_api_v2/tests/actions.rs`;
- focused starlark-rust evaluator tests colocated with the caller accessor;
- `app/slug_loading_v2/src/builtin_restriction_tests.rs`;
- `app/slug_analysis_v2/tests/{starlark_rule.rs,subrule.rs}`;
- `app/slug_reapi_v2/tests/reapi.rs`;
- at most one new oracle fixture containing at most 6 regular files and 220
  newline-counted text lines, only if the accepted source evidence leaves a
  demonstrated gap.

Plans may touch this manifest, the canonical Live Status, Stage 6, and Stage 9.
Do not touch the parked loading proof.

Caps are at most 1,500 added production Rust lines, 1,000 added proof Rust
lines, and 2,500 total added Rust lines. Cargo and plan lines are counted
separately. No touched production file may cross 2,000 lines. `evaluate_loaded_rule`
receives wiring only; new lowering logic belongs in the focused sink owner.
`REPLAN` before exceeding a cap or adding another production file.

Add no dependency beyond the one internal build-API-to-configuration edge; no
new DICE key, cache, process-global interner, parser, executor support, C++ rule
branch, production fallback, JVM, Java helper, or copied donor code.

## Evidence contract

Implementation must prove all of the following:

1. Args same-object chaining, call order, one/two positional forms, integer and
   File conversion, valid/invalid `%s`/`%%`, vector and directory rejection,
   action-time snapshot, and post-registration mutation isolation.
2. File path/basename/dirname including root-parent behavior, with generated
   bytes explicitly asserted as Slug-native.
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
   evaluator/action owners drop. One same-DICE cold/warm plus A/B/A proof must
   show separately allocated publication-equal action depsets cut off, a
   topology/value mutation invalidates, and restoration cuts off again.
7. File and normalized string executable identity; default/custom mnemonic;
   progress; false/true default environment; canonical environment equality;
   and structural inequality for every changed field.
8. Artifact and absolute symlink kind, owner, authentication, normalized target,
   progress, input topology, and variant inequality. Generic
   `check_private_api` proofs cover tuple coercion, default depth one, explicit
   depth zero/two, negative rejection, an allowed wrapper/caller pair, a denied
   caller selected above an allowed wrapper, and the no-enclosing-function
   branch.
9. No retained evaluator values, no lock across lowering/DICE, `Allocative` and
   cheap-clone coverage, and no populated parallel raw spawn representation.
10. REAPI Command/InputTree/execution reject the new payloads before producing
    a digest or action plan; existing FileWrite projections remain unchanged.
11. One authenticated rules_cc FDO discriminator crosses File properties,
    chained Args, typed executable, list inputs, the transitive `all_files`
    depset tool, default environment, run, artifact symlink, and absolute
    symlink, including the preceding custom-allowlist check, without any parser,
    FDO, `cc_common`, or C++ action special case. A parent-created Args is passed
    to and mutated by a nested subrule before snapshot.

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
  successor without a second owner; and
- the focused correction rereview must confirm evaluator-owned Args lifetime,
  fail-closed short paths, action publication equality, and the generic custom-
  allowlist/depth bridge, then return `ACCEPT`. A second material contract miss
  is `REPLAN`.

After implementation, run serially:

- focused configuration path/environment tests;
- focused build-API action/projection tests;
- focused loading restriction and analysis Args/action/symlink tests;
- focused REAPI fail-closed/FileWrite regression tests;
- `cargo test -p slug_build_api_v2`, `cargo test -p slug_loading_v2`,
  `cargo test -p slug_analysis_v2`, and the named direct
  `slug_configuration_v2`/`slug_reapi_v2` dependents;
- `cargo build -p slug_cli_v2` before any `SLUG_V2_BIN` oracle;
- the bounded FDO oracle/smoke if selected, with stale `slugd` cleaned before
  and after;
- `cargo fmt --all -- --check`, `scripts/v2_archive_status.sh`, and
  `git diff --check`.

Independent terminal review is mandatory for this public cross-crate retained
identity. One focused correction is allowed. A second material contract or
implementation correction is `REPLAN`.

Terminal `ACCEPT` updates the canonical M7 row, this manifest, and the Stage 9
generic Args/spawn/symlink row, then commits the complete packet without the
parked proof. The next packet remains
`WP-6-7A-noncallback-vector-args-paramfiles-implementation-r1`.
