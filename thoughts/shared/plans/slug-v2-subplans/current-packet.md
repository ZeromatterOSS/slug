# Current Slug V2 Work Packet

Packet: WP-7-21-m7a-rustc-file-dirnames-r1
Status: accepted; reviewed checkpoint ready for integration

## Outcome and compatibility

Admit the regular-File directory mappers demanded by pinned rules_rust 0.73.0
`construct_arguments`: generated-root CARGO_MANIFEST_DIR, output directory,
nonempty standard-library search paths, and generated sysroot. Reuse the
verbatim fixture and production-loader proof accepted at `ff14d4e33` (WP-7-20).
The observable result is a configured Spawn with those flags and exact retained
artifact relationships, with no Starlark callable retained beyond evaluation.

This remains finite pinned-output compatibility. Callback behavior/order and
relative File dirname operations are exact; actual generated path bytes remain
Slug-native as before, as do eager rejection timing and configuration identity.
Generic callbacks, directory expansion, tree outputs, dependency-provider/link
mappers, path stripping, sibling layout and execution remain unsupported.
This does not close M7A or claim complete Rustc/CLI behavior.

## Source and owners

Pinned rustc.bzl SHA-256 stays
`a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`.
Authenticate the loaded canonical label, SHA and native callee line as in
WP-7-20. New one-based sites: 1099, 1228, 1275 and 1425. `_get_dirname`
returns File.dirname; `_get_out_dir_path` does likewise for admitted regular
files. Other existing callback/empty restrictions remain intact. Callable,
allow_closure, format and positional validation stay before empty omission.

Bazel 9.2 source commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
Args.java validateMapEach/addVectorArg and StarlarkCustomCommandLine.applyMapEach
own mapping, formatting and uniquification order. Artifact.java:384–395 and
PathFragment.java:721–723 require `.` for a relative basename's dirname.
Correct the existing empty-string case through one AnalysisArtifact dirname
projection shared by File and retained rendering; no separate identity is
created. Root and nested source/derived paths discriminate this correction.

Loading owns callsite classification; analysis validates only actual regular
Files and lowers sequence/depset inputs through its existing value lowerer.
Use one typed RetainedVectorSource::RegularFileDirnames over ArtifactInputs,
reusing Direct/Depset ownership and publication equality with the same
PublicationEqState as Spawn inputs. Preserve depset shape, order, alias topology
and artifact producer identity; never retain only dirname strings or flatten
depsets at publication. The renderer maps each artifact before existing
format/uniquify/before_each processing. The admission constructor validates every retained artifact as a regular File
and rejects FilesToRun before retention; analysis must use that constructor.
External source path spelling remains deferred with existing path projection.

Existing Bzl/package/configured DICE edges own source and graph changes;
see docs/developers/dice.md. No new key, cache, host observation, lock or fallback.
New memory is configured-action-retained Arc data plus phase-local rendering
scratch and is released with existing owners. No performance claim is made.

## Scope and validation

Allowlist: app/slug_build_api_v2/src/actions/spec.rs and src/analysis_value.rs,
its tests/actions.rs; app/slug_loading_v2/src/subrule_invocation.rs;
app/slug_analysis_v2/src/starlark_rule.rs and tests/rustc_map_each/{mod.rs,subject.bzl};
fixture.toml provenance only; this manifest, canonical status and Stage 6/7/
bootstrap gate summaries at acceptance. Existing large owners remain cohesive;
no duplicated evaluator/renderer machinery or new upstream files are needed.

Extend the real builder proof with nonempty nested standard-library depsets,
same-directory uniquification, regular output/sysroot and generated-root cases.
Assert argv and multiline param-file bytes and reject non-Files/directories.
Preserve original source A/B/A and generic forged callback negatives; move the
prior nonempty-negative to an unadmitted dependency-provider mapper. API tests
prove dirname formatting/order, root dirname, distinct producers with identical
rendering, and shared-versus-split depset identity across argv and Spawn inputs.
Reuse provenance and source-growth acceptance at ff14d4e33 unchanged.

Compile separately with pinned nightly direct tools under the skill's
60-second preparation cap. Preflight exact selectors, then run focused new and
protected tests in small batches (expected subsecond). Run named direct
compile dependents slug_query_v2 and slug_reapi_v2, rustfmt, diff and plan checks.
Tests over a few seconds run infrequently; over roughly 30 seconds require
strict necessity scrutiny. No broad suite, Bazel build, daemon or CLI selected.
Independent design and final review are required for retained semantic identity.
If demanded behavior needs generic callable retention, directory expansion or
new DICE ownership, resolve that decision before widening this packet.

## Validation receipt

Base `ff14d4e33`; pinned nightly-2025-09-14 direct toolchain (rustup's snap
launcher is unavailable). Cargo `--no-run --message-format=json` supplied each
executable and all selectors passed the exact nonignored preflight.
Local logs are in `target/wp721/`.

- Analysis `starlark_rule` executable: 5/5 in 0.34s. Selectors are the two
  `rustc_map_each::pinned_rustc_*` tests plus
  `scalar_args_reject_vector_and_invalid_format_before_action_publication`,
  `vector_args_param_policy_and_args_write_use_one_generic_recipe`, and
  `args_run_and_symlink_snapshot_through_the_generic_action_sink`.
  The real builder covers all four native spans, nested stdlib traversal,
  same-directory uniquification, generated-root A/B/A, argv/multiline bytes,
  non-File sequence/depset rejection, source A/B/A and protected generic negatives.
- Build API `actions`: 4/4 in 0.00s (0.003s wrapper):
  `file_dirname_recipes_preserve_identity_and_map_before_uniquify`,
  `file_dirname_depsets_share_publication_alias_state_with_spawn_inputs`,
  `pinned_regular_crate_root_retains_artifact_and_root_path_identity`, and
  `vector_depsets_share_publication_alias_state_with_spawn_inputs`.
  Direct/depset directories, nonregular outputs and FilesToRun reject;
  identical rendering preserves producer, artifact and depset alias/shape
  distinctions. Two initial graph-shape assertions used ordinary constructor
  inputs that canonicalize to the same retained shape. The corrected test uses
  the existing explicit retained-graph constructor; no production fix was needed.
- Initial analysis preparation took 24.59s. Final API and analysis preparation
  took 1.139s and 7.513s after test-only corrections.
- `cargo check -p slug_query_v2 -p slug_reapi_v2`: exit 0 in 7.77s;
  existing REAPI deprecated-field warnings remain.
- Changed Rust formatting, diff and plan-status checks pass. Imported fixture
  files and their inventory are unchanged, so prior source/provenance acceptance
  is reused. No daemon, compiler action, Bazel invocation or network test ran.

Independent final review returned ACCEPT for the four pinned sites, shared
dirname projection, retained identity and discriminating evidence. No claimed workflow speedup or complete
M7A admission; the advanced gate is the four demanded regular-File directory
mappings through authentic configured analysis.
