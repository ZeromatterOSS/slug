# Current Slug V2 Work Packet

Packet: WP-7-23-m7a-forced-virtual-paramfiles-r1
Status: accepted; independently reviewed checkpoint ready for integration

## Outcome, demand and compatibility

Expand typed Spawn Args with `use_param_file(..., use_always=True)` into one
immutable result containing replacement argv and virtual parameter-file bytes,
then merge those virtual inputs into an existing REAPI input tree atomically.
Demand: pinned rules_rust rustc.bzl:1168 selects multiline `@%s` with use_always
when the process wrapper is present; the authentic configured Spawn at
`36cb5fa3a` proves that retained policy. Its real-source proof will now assert
expanded argv and bytes. Complete this generic forced-spill behavior for the
three already-admitted Args formats (shell/multiline/flag_per_line), including
flag-only partitioning, empty files, numbering and interleaved literal chunks.

Filename suffix, substitution, partition order and content use pinned Bazel
9.2 algorithms. The base output/artifact paths retain Slug-native spelling;
parameter files do not establish exact Bazel output identity. Exact CAS digests
and protobuf topology apply to the actual produced bytes. Conservative collision
and invalid-path rejection are Slug-native validation. Conditional spill
(use_always=False) stays unsupported until its command-line-limit policy has an
authoritative owner; do not guess host limits. Output-path stripping is deferred.

This admits expansion and input-tree composition, not typed Spawn execution,
source/generated-file resolution, inherited env resolution, aquery activation,
transport or M7A. Existing typed-action rejection and resolved-closure execution
gates remain. The new result is a required component for that later handoff.

## Source and ownership

Bazel source commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
CommandLines.expand (74–153), ParameterFile.derivePath/writeParameterFile
(73–99), Args.getParamFileInfo/setParamFileFormat, and SpawnAction.getSpawn
(358–379). Base path is the first declared primary output; forced files are
numbered in spilled-chunk order as `<output>-0.params`, `-1.params`, etc.
FlagPerLine groups already belong to RetainedArgsRecipe; virtual files contain
only `--`-prefixed rendered flags, with other rendered arguments kept after the
replacement argument. Shell quoting and newline rendering reuse the existing
ArgsWrite recipe owner; an empty recipe produces zero bytes, still one file.

Pinned CommandLinesTest cases expand_paramFileUseAlways,
expand_mixOfCommandLinesAndParamFiles and expand_flagsOnly supply focused test
themes. Conditional-limit and path-mapper tests are deferred with those surfaces.
No new copied oracle fixture; extend the accepted source regression's provenance.

Build API owns a new actions/spawn_command_line.rs module containing
ExpandedSpawnCommandLine and VirtualParamFile with private immutable fields,
plus an expansion error. SpawnSpec::expand_forced_param_files derives primary
path from its retained outputs and invocation/segments from the same spec.
Rendering helpers stay in spec.rs, shared with ArgsWrite. No new retained action
identity: the expansion is action/RPC scratch derived from the retained recipe,
with no callable, heap, host read or semantic cache. Output/template strings
already validated by analysis remain that owner's contract; expansion rejects
missing/invalid primary paths and unsupported conditional policy. Each generated
virtual path must reject equality or file/directory prefix conflicts with every
declared output, including secondary outputs; cover equality and both prefix
directions in focused API negatives.

REAPI owns ReapiInputTree::with_spawn_param_files(&ExpandedSpawnCommandLine).
Build the merged path map, blobs and Merkle directories in scratch; publish only
on success, leaving the input tree unchanged on failure. Reject any virtual-file
path equal to an existing entry, even with equal bytes; reject file/directory
prefix conflicts in either direction. The existing Merkle builder must check a
file before descending into the same-named directory. Preserve ordinary entries
and inline bytes. No public mutable virtual-input bag or command-side repair.

DICE ownership is unchanged (docs/developers/dice.md): source/provider/recipe
changes invalidate configured actions before expansion, and accepted output
conflict validation must still precede execution. The aggregate keeps argv and
its virtual files together; no independent semantic side store is introduced.
Scratch vectors/blobs live with their expansion/tree and are released normally.
No performance claim is made.

## Scope and gates

Allowlist: app/slug_build_api_v2/src/actions/{spawn_command_line.rs,mod.rs,spec.rs},
src/lib.rs and tests/actions.rs; app/slug_reapi_v2/src/input_tree.rs and
tests/reapi.rs; app/slug_analysis_v2/tests/rustc_map_each/mod.rs; fixture.toml
provenance; manifest/canonical and Stage 6/7/bootstrap summaries at acceptance.
The new responsibility gets its own module; existing large files receive bounded
dispatch/rendering changes. New fixture bodies and loader code are unnecessary.

API tests cover forced replacement/order/numbering, all three formats, flags-only
positionals, empty content, escaped templates, primary-output ownership, and
conditional/missing/invalid-path negatives. The authentic Rustc proof checks its
actual retained policy/recipe against expanded argv and bytes. REAPI tests check
exact blob SHA, decoded Directory path/mode/topology, retained input bytes,
collisions (equal and different bytes, both prefix directions), deterministic
replay and changed-content digest. Existing typed-action projection/transport
rejection remains a protected gate; no transport invocation is selected.

Compile separately with pinned nightly under 60-second preparation operations;
preflight exact selectors and execute small API/analysis/REAPI batches (expected
subsecond). Compile direct query/reapi dependents; run rustfmt/diff/plan checks.
Tests over a few seconds run infrequently; over roughly 30 seconds require strict
necessity scrutiny. No daemon, full CLI, Bazel build or compiler action. Independent
design and final review are required for the new public/ownership boundary.
If expansion needs unowned limits/path mapping or runtime activation requires
bypassing resolved closure/inputs, resolve the prerequisite before extending scope.

## Acceptance receipt

Independent design and final reviews ACCEPT. Review added all-output path
collision checks and reused registration's primary-path validator, including
backslash negatives. No REPLAN or semantic scope expansion was needed.

Pinned nightly direct binaries were used after the rustup snap launcher failed
before compilation. Separate `cargo test --no-run --message-format=json`
preparation: API 3.67s then 1.85s after validation correction; REAPI 37.99s then
7.88s; analysis 24.51s. Every preparation operation exited 0 within its 60s cap.
Exact-selector preflight and execution passed:

- API: forced formats/order/numbering, invalid policy/path/output collisions,
  and protected ArgsWrite formatting: 3/3, 0.00s (wrapper 0.001s).
- REAPI: exact virtual bytes/Merkle topology, atomic input collision rejection,
  both typed execution/projection rejection tests and legacy paramfiles: 5/5,
  0.00s (wrapper 0.002s).
- Analysis: authentic Rustc source-edit/restoration with expanded argv/bytes,
  generic Args/param policy and Spawn/symlink snapshots: 3/3, 0.35s
  (wrapper 0.362s).

Direct `cargo check -p slug_query_v2 -p slug_reapi_v2` exited 0 in 11.60s;
only three existing deprecated REAPI proto-field warnings. Rustfmt check,
`git diff --check` and plan status passed. Local receipts: `target/wp723/`.
Review and total packet wall time were not separately measured. No broad suite,
daemon, Bazel/compiler action or live transport ran; no fixture bodies grew.

The observable advance is forced virtual argv/bytes plus collision-safe REAPI
input-tree composition, including the authentic Rustc builder. M7A remains open:
conditional spill, other callbacks, source/generated input resolution, inherited
environment and typed Spawn execution/aquery still need their own admission.
