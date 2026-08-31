# Current Slug V2 Packet

Packet: `WP-6-7A-noncallback-vector-args-paramfiles-implementation-r2`

Milestone: M7A generic Starlark/ruleset closure; Stage 6 action declaration.

Status: Terminal `ACCEPT`. R1 returned `REPLAN` for two bounded integration
misses: Bazel's typed binding validates a supplied vector source before the
unsupported callback boundary, and the existing no-op action-sink test adapter
was outside the proof allowlist. Focused R2 correction review accepted the
shared validation-order fix and frozen one-line adapter; terminal rereview then
accepted the complete implementation and unchanged retained architecture.

Base: `2bf929bd1`, the R1 design commit atop `78b94789c`, which terminally
accepts typed `DefaultInfo.files`, scalar Args, generic Spawn/artifact-symlink/
absolute-symlink declaration, configured caller authentication, and action
publication cutoff. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result and category boundary

An ordinary rule or subrule can add sequence or depset values to the existing
evaluator-local `Args` through non-callback `add_all` and `add_joined`, retain
the resulting recipe in the already-accepted command-line owner, attach exact
parameter-file policy, and pass the same Args to `actions.run` or
`actions.write`. The admitted vector values are strings, integers, and regular
Files. Sequence membership is snapshotted at the Args call; depset topology is
lowered once at action registration and retained without flattening.

This fills the vector and param-file variants reserved by the accepted generic
action architecture. It adds no second Args, command-line, depset, File, spawn,
or write owner. It is not an execution or C++ packet: Bazel 9 BCR Starlark,
including `cc_internal`, remains an ordinary consumer. Buck2 starlark-rust
continues to own parsing, binding, evaluation, heap lifetime, method dispatch,
and `set` semantics.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority:

- `Args.java` SHA-256
  `ac704917bb3d6814fdb6f642c42d9300d9cac1d6fc624d769d3d41e42225ef1b`
  and `CommandLineArgsApi.java` SHA-256
  `18e3825616f147cdcd83b60444dfc8b961c971a9aec8f7aff4aed74226e1cdf6`
  fix the one/two positional forms, mutation checks, sequence/depset sources,
  validation order, formatting, stable-first uniquification, omission,
  before/terminate placement, joining, directory-expansion default, and
  param-file mutators.
- `StarlarkActionFactory.java` SHA-256
  `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`
  fixes action-time Args snapshotting and the distinct Args-backed
  `ParameterFileWriteAction` path.
- `ParamFileInfo.java` SHA-256
  `a144542b382892258c4043390387e1133db51ba6db187436205db4ce105f697f`
  fixes flag format, always-use, file type, and flags-only structural policy.
- `ParameterFile.java` SHA-256
  `f188a72a4ed5cbc97142c8e3bcf447e4774599b0b1fa7e94ee3d7aa2c48be7ee`
  fixes newline framing and shell-quoted versus unquoted content; flag-per-line
  groups flag names and values before unquoted newline framing.
- `StarlarkRuleImplementationFunctionsTest.java` SHA-256
  `89e6caf0c6d234be610ccb597a015610568c27f8071d572e55a7378a106597d8`
  pins add-all/joined order, empty behavior, uniquification, mixed segments,
  parameter-file policy, invalid formats, and Args-backed write.
- `ArgsParamFileTest.java` SHA-256
  `06f3e840ec2e4ffcf0173d51eaf23bc1cff3d9ff86ed3e729c702f2d945e32c4`
  pins shell, multiline, flag-per-line and set-format-once behavior.

Reuse those source regressions; no fresh oracle is needed unless implementation
exposes an observable they do not discriminate. Java object identity,
interner behavior, implementation class names, and private builder layout are
not compatibility surfaces.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains concept-
only peer guidance for evaluator-owned mutation, action-time finalization,
typed segments, and compact immutable recipes. Copy no Zig code, names,
layout, diagnostics, fingerprints, or tests. Buck2-derived utility guidance
selects compact `Arc` slices, `CompactString`, `Dupe`, `Allocative`, and the
already-retained dense depset. Import no Buck2 command line, action, interner,
cache, parser, or `transitive_set` owner.

## Compatibility classification

**Exact:** active-context/receiver and evaluator mutability checks; same-object
chaining; `add_all` and `add_joined` one/two positional forms over lists,
tuples, and depsets of admitted strings, integers, or regular Files;
`format_each`, `before_each`, `omit_if_empty`, `uniquify`, `terminate_with`,
`join_with`, and `format_joined` order; the single-`%s`/`%%` formatter;
sequence membership snapshot, action-time snapshot, and post-registration
isolation; `expand_directories=True` or false when no directory File occurs;
directory File rejection before publication; `use_param_file` flag format and
always-use state; shell, multiline, and flag-per-line format selection and
set-once validation; param-file newline/quoting/grouping bytes for Args-backed
write; string-versus-Args write dispatch; output ownership, executable bit,
default FileWrite mnemonic, and atomic publication; and publication equality
for separately allocated dense depset recipes including alias topology.

**Slug-native:** evaluator-to-analysis borrowed snapshot mechanics; Rust valid-
Unicode strings; compact recipe storage; structural action identity; generated
File path bytes and any rendered vector/param-file bytes containing them; and
the typed non-executed Args-write action representation. The transformation
algorithm and typed artifact relationships are exact; only configured output
spelling remains Slug-native.

**Unsupported/deferred:** `map_each`, `allow_closure`, `DirectoryExpander`, and
directory/tree artifact expansion; Labels and arbitrary Starlark values in
vector sources; runtime platform command-length selection when
`use_always=False`; materialization of spawn param files; aquery, execution,
ActionKey, or REAPI projection for typed Spawn/Symlink/ArgsWrite actions;
explicit Args-write mnemonic/execution requirements; callbacks; Args-backed
template expansion; FilesToRun/runfiles; and remaining spawn-envelope or
symlink forms. A deferred value or callback fails during Args lowering or
action registration and publishes no action.

## Frozen ownership and implementation

### Evaluator-local recipe and synchronous lowering

Replace the scalar-only vector inside `StarlarkArgs` with one evaluator-local
ordered call recipe. Scalar calls keep their accepted owned retained values.
Vector calls retain an evaluator-local source plus owned options:

```text
EvaluatorArgCall = Scalar(RetainedScalarArg)
                 | Vector {
                     kind: AddAll | AddJoined,
                     source: Sequence([Value]) | Depset(Value),
                     arg_name, format_each, before_each,
                     join_with, format_joined,
                     omit_if_empty, uniquify, terminate_with,
                   }
EvaluatorParamState = { file_type, flag_format?, always, flags_only }
```

The Starlark value traces every stored `Value`; no unsafe trace omission may
hide a vector source. A list/tuple copies its element occurrences when
`add_all`/`add_joined` is called, so later container mutation is invisible. A
depset retains only its evaluator occurrence until a consuming action call.
The Args object still refuses freezing.

The starlark-rust method signature uses `Value` for Bazel's sequence-or-depset
union. The shared positional helper therefore validates and captures that
source after the two-position arg-name check but before callback rejection.
For either positional form an invalid source wins over a valid callback, while
an invalid two-position arg name wins over both. This reproduces Bazel's typed
binder plus `Args` method ordering without changing the parser or retaining a
callback.

At `run` or Args-backed `write`, clone only a request-local snapshot of the
evaluator recipe and synchronously hand it to the existing analysis-owned sink.
The analysis lowerer converts each admitted scalar and the depset root into
owned build-API values before return. No `Value`, heap, evaluator, mutable
container, callable, repository mapping, or call token enters `ActionSpec`.
Lowering and rendering occur before the short `CtxActions` registry lock.

### One retained command-line and dense-depset owner

Extend the existing retained command-line variants, rather than adding a
parallel vector:

```text
RetainedArgsRecipe = {
  calls: Arc<[RetainedArgCall]>,
  write_format: RetainedParamFileFormat,
}
RetainedSpawnArgsSnapshot = {
  recipe: RetainedArgsRecipe,
  param_file: Option<RetainedSpawnParamFilePolicy>,
}
RetainedArgCall = Scalar(RetainedScalarArg)
                | AddAll(RetainedVectorArg)
                | AddJoined(RetainedVectorArg)
RetainedVectorSource = Sequence(Arc<[RetainedScalarValue]>)
                     | Depset(RetainedArgsDepset)
RetainedParamFileFormat = Shell | Multiline | FlagPerLine
```

`RetainedArgsDepset` directly wraps the accepted `AnalysisDepset`, validates
empty/string/integer/artifact element types, and iteratively visits it only for
scratch rendering. It never flattens the semantic ABI. Rendering performs, in
order: scalar projection, `format_each`, stable-first uniquification,
`before_each`, omission decision, arg-name/terminator insertion, or join then
`format_joined`. Empty strings remain values. `add_joined(...,
omit_if_empty=False)` emits the empty joined argument.

Command-line publication equality becomes manual. One shared
`PublicationEqState` compares every vector-depset source together with Spawn
inputs and tools, preserving dense rows, order, values, roots, and alias
partitions. Sequence and depset sources remain structurally distinct. Ordinary
`AnalysisDepset::Eq` remains occurrence identity.

`use_param_file` snapshots a structurally distinct policy on each consuming
Spawn only; a later Args mutation affects only later Spawn actions.
`set_param_file_format` sets the common recipe encoding at most once whether or
not a Spawn policy is present. Args-backed write consumes the common calls and
format but deliberately drops `param_file_arg` and `use_always`, matching
`StarlarkActionFactory.write`. Do not import Bazel's weak interner or retain
both policy and pre-rendered bytes.

### Typed Args-backed write

Add one `ArgsWriteSpec` payload to the closed `ActionPayload` union. It owns one
authenticated output, one `RetainedArgsRecipe` without Spawn-only param policy,
the executable bit, default FileWrite mnemonic, and an empty canonical
execution-requirement map. String content continues through the existing
FileWrite action unchanged. Args content uses only `ArgsWriteSpec`; it does not
populate legacy content, argv, input, or param-file vectors. Two writes whose
Args differ only in `use_param_file` flag format or `use_always` compare equal;
the corresponding Spawn snapshots compare unequal.

The build API may expose a non-digest rendered-content helper for tests and
future aquery work. It emits exact selected format bytes, but REAPI Command,
InputTree, action planning, execution, and digest construction reject
`ArgsWriteSpec` before producing bytes. Spawn param-file policy remains
declarative; runtime spill choice and derived param-file paths are not guessed.

## Request, revision, and memory behavior

No new DICE key, observed input, environment read, filesystem read, async task,
cache, interner, or global registry is added. Args objects and evaluator source
occurrences are phase scratch and die with evaluation. Lowering/rendering
vectors and stable-first seen sets are action-call or consumer scratch.
Retained recipes, scalar values, dense depsets, policy, and ArgsWriteSpec are
immutable configured-analysis state and participate in result equality/cutoff.
Cancellation before registry publication leaves no partial action. Concurrent
requests have independent evaluators and sinks; they share only immutable
already-published configuration/depset values. Release follows the existing
analysis-result/action owners, with no separate eviction or shutdown work.

## Allowlist, caps, and stops

Production files:

- `app/slug_loading_v2/src/subrule_invocation.rs`;
- `app/slug_analysis_v2/src/starlark_rule.rs`;
- `app/slug_build_api_v2/src/actions/{spec.rs,ctx_actions.rs,registry.rs,reapi_projection.rs,mod.rs}`
  and `app/slug_build_api_v2/src/lib.rs`;
- `app/slug_reapi_v2/src/{command.rs,input_tree.rs}` only for explicit
  fail-closed handling of the new payload if the existing typed gate is not
  already exhaustive.

Proof files:

- `app/slug_build_api_v2/tests/actions.rs`;
- `app/slug_analysis_v2/tests/starlark_rule.rs` and `tests/subrule.rs`;
- `app/slug_loading_v2/src/builtin_restriction_tests.rs` only to change the
  unused no-op sink's `content` parameter from `&str` to `Value`; its frozen
  base SHA-256 is
  `959a3cc8a243cb7efcf1b44e479d74168beb22d389997b2d262f3b5791126a60`;
- focused loading tests colocated in `subrule_invocation.rs` only if binding
  order cannot be proved through analysis tests;
- `app/slug_reapi_v2/tests/reapi.rs`.

Plans may touch this manifest, canonical Live Status, Stage 6, and Stage 9.
Do not touch the parked loading proof. Add no crate dependency, DICE key,
starlark-rust parser/evaluator change, FDO/C++ branch, executor behavior,
fallback, JVM, Java helper, or donor code.

Cap added Rust at 900 production, 800 proof, and 1,700 total lines. No touched
production file may cross 2,000 lines; `starlark_rule.rs` remains cohesive as
the analysis-owned synchronous lowering sink, while its large integration test
file is a pre-existing proof owner. `REPLAN` before exceeding a cap, adding a
production file, retaining any evaluator value, flattening a depset into the
semantic ABI, duplicating the command-line/write owner, or needing callback,
directory-expansion, execution, or repository-mapping semantics.

## Evidence and validation gates

Focused proof must discriminate:

1. list/tuple snapshot versus later mutation, depset dense topology, scalar
   type rejection, one/two positional forms, and same-object chaining;
2. every admitted transform and its order, empty/empty-string behavior,
   stable-first uniquification, formats with `%s`/`%%`, mixed scalar/vector/
   literal segments, and cold/warm identical rendering;
3. regular source/generated Files with generated bytes classified Slug-native,
   plus fail-closed directory, callback, closure, arbitrary value, and invalid
   source cases before action publication; both positional forms prove invalid
   source before callback, while invalid two-position arg name wins first;
4. action-time and post-registration isolation for calls and param policy;
5. shell, multiline, and flag-per-line bytes; invalid flag format, invalid
   file format, and set-format-twice errors; Spawn always true/false structural
   distinction without a runtime-spill claim; and ArgsWrite publication/render
   equality when only `param_file_arg` or `use_always` changes;
6. Args-backed write versus unchanged string FileWrite, output ownership,
   executable bit, default mnemonic, one semantic payload, and atomic conflict
   rejection;
7. publication equality/cutoff for separately allocated equal vector depsets,
   then inequality for topology/value/order/source-kind/alias/policy changes;
8. `Allocative`, cheap-clone, evaluator/action release, and no retained
   Starlark value or parallel rendered vector; and
9. REAPI Command/InputTree/execution rejection before digest/action-plan bytes,
   with existing FileWrite and typed Spawn/Symlink behavior unchanged.

Run serial focused loading/build-API/analysis/REAPI tests, then full
`slug_build_api_v2`, `slug_loading_v2`, `slug_analysis_v2`, and `slug_reapi_v2`
tests. Run `cargo check -p slug_core_v2` as the direct public dependent; rebuild
`slug_cli_v2` only if a CLI smoke is selected. Finish with `cargo fmt --all --
--check`, `cargo metadata --format-version 1 --no-deps`,
`scripts/v2_archive_status.sh`, `git diff --check`, cap accounting, and parked-
file integrity.

R1 terminal review accepted the retained recipe/snapshot split, shared
publication state, ArgsWrite policy separation, atomic lowering, REAPI
boundary, caps, and generic no-C++ architecture, but returned `REPLAN` on the
binding-order and allowlist misses above. Focused R2 correction review and the
complete terminal rereview both returned `ACCEPT`. Focused/full validation
passes 60 build-API, 541 loading with one ignored, 106 analysis, and 22 REAPI
tests with one ignored, plus the public core check, formatting, metadata,
diff-check, frozen-base archive classification, and parked-file integrity.
Added Rust is 891 production, 747 proof, and 1,638 total lines; every touched
production file remains below 2,000 lines.

Terminal `ACCEPT` updates the canonical M7 row and Stage 6/9 record, commits the
packet without the parked proof, and selects
`WP-6-7A-complete-noncallback-spawn-envelope-implementation-r1`.
