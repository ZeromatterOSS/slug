# Current Slug V2 Work Packet

Packet: WP-7-20-m7a-rustc-callback-proof-r1
Status: accepted; integrating the reviewed checkpoint on main

## Outcome and ownership

Complete the preserved WP-7-11 regular-crate-root callback candidate with a
portable production-loader proof: call the verbatim pinned rules_rust 0.73
`construct_arguments`, publish its Args as one configured Spawn, and prove
loaded source A/B/A invalidation in one DICE service. The candidate contract
and conditional design review are preserved at `ea5fcc6fd` in the manifest.
This successor retains its finite pinned-output compatibility and Slug-native
error timing; it does not claim full Rust rule, CLI, generic callback or M7A
admission. WP-7-19 at `18ef4288b` remains the accepted predecessor.

The native evaluator source span, loaded-module canonical identity and source
SHA-256 authenticate the callback; ordinary Bzl/package/configured DICE edges
own invalidation. The action retains the typed artifact/root-path recipe and
vector options, never a Starlark callable, evaluator pointer or heap. Existing
structural artifact equality and atomic configured-result publication remain
required. No new DICE key, semantic cache, host read or fallback is introduced.
DICE principles: docs/developers/dice.md; existing source invalidation tests in
app/slug_analysis_v2/tests/starlark_rule.rs remain the ownership reference.

## Proof and scope

The scoped test request helper supplies registry request inputs, observation
shards and local-module materializations, retaining the real builtin catalog.
The canonical external target selects the production cross-repository loader.
Use one local
fixture with real transitive Bzl imports, a test-authored MODULE/BUILD/custom
rule and minimal argument-builder inputs. Optional standard-library and crate
depsets are empty, so their callbacks need no invocation. Preserve callable,
allow_closure and format validation before an empty-source omission; source-pin
restrictions apply to every callback. Empty omission is limited to the exact
loaded native callee spans at lines 1275, 2573 and 2575; all nonempty
inputs at those sites and all generic empty callbacks stay unsupported. Do not admit generic eager
callback execution: its purity/lifetime contract has not been established.

The load closure has 55 Bzl files / 856,578 bytes. Reuse the existing built-in
`action_names.bzl` (5,400 bytes); the new fixture owns the other 54 files /
851,178 bytes, including the
rules_cc generated compatibility proxy and its real imports. Imported bodies are
verbatim from the current pinned Bazel repositories; proxy symbols are generated
byte-for-byte by the recorded upstream writer. Record per-file hashes,
upstream versions, licenses and the proxy writer. Test MODULE/BUILD declarations
are isolated harness inputs, not substitutions for imported implementation.
This source-growth checkpoint requires final review of the load closure; prune
no source demanded by an actual load. No full Rust toolchain, compiler action,
F3 fixture or network acquisition is selected.

Allowlist: the five WP-7-11 Rust implementation/test files; a separate focused test module and
source inventory and fixtures/rustc-map-each directory (including its exact-byte
whitespace attribute) under
app/slug_analysis_v2/tests; that crate's BUILD.bazel for compile data; this
manifest and canonical Live Status; Stage 6/7/bootstrap readiness only if a gate
is accepted. Existing large analysis/loading files retain their established
Args owner; the new fixture stays separate. New memory consists of evaluator
scratch and one immutable typed retained recipe variant, released with its
configured action. No hot-path optimization is claimed.

## Validation and decisions

Independent design review accepts this bounded proof direction conditionally.
Pinned Bazel 9.2 Args.java validateMapEach/addVectorArg and
StarlarkCustomCommandLine.applyMapEach own validation and empty callback
behavior; rustc.bzl SHA-256 is
`a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`.
Prove the real line-1169 positive, source mutation rejection/no configured
publication, exact recovery in the same Arc<Dice>, retained multiline @%s
policy, forged-callsite negatives, and same-path distinct-producer inequality.
Reuse earlier structural owner tests and compile named direct dependents.

Compile separately under a 60-second preparation cap; preflight exact selectors
before running focused tests. Start the new proof with a 10-second operation
cap. This is a diagnostic bound, not a user deadline. Tests should be as small
as possible; >few-second tests run infrequently and >roughly 30-second tests
require strict necessity scrutiny. No broad suite is selected. Run rustfmt,
plan-status and diff checks, then independent final review before integration.
Preserve substantial unfinished work on the review branch; do not merge a
partial semantic activation. If imports need invented semantics, a nonempty
unadmitted callback is reached, or source invalidation needs a new owner,
record the concrete prerequisite and revise the contract before extending it.

## Demanded File property prerequisite

The real loader proof reaches `rustc.bzl:1086`, which requires `File.is_source`
before the callback. Independent design review accepts exposing this exact
boolean through the existing AnalysisArtifactValue wrapper and dir_attr, derived
only from Source versus Derived identity. Pinned Bazel 9.2 FileApi.is_source /
Artifact.isSourceArtifact supply the source contract. The proof checks true for
a root SourceFile and false for a declared output; neither ctx.file nor external
SourceFile configured targets is admitted. The custom rule uses the established
source-attribute File representation and targets the canonical rules_rust repo.

The next real source access at `rustc.bzl:1089` requires
`Label.workspace_root`. Independent design review accepts the existing
StarlarkLabel owner computing the exact default-layout repository prefix:
empty for root, `external/<canonical repository>` otherwise, without package
or target. Add `app/slug_loading_v2/src/starlark_label.rs` to the allowlist.
Pinned Bazel 9.2 Label.java:379–394, RepositoryName.java:333–342 and
BuildLanguageOptions.java:287–288 own this default. Slug's existing BazelLayout
uses the same external directory. The authentic proof checks root/external
labels, discoverability and CARGO_MANIFEST_DIR. Sibling layout is unsupported;
this does not claim source materialization or output-path identity parity.

## Final validation receipt

Base: main `00d26ee7b`; candidate: review/wp-7-20-rustc-callback-proof.
Pinned nightly-2025-09-14 direct binaries were used after the rustup launcher
failed. All test executables came from successful Cargo `--no-run` JSON and
exact nonignored selector preflights. Logs are local `target/wp720/` artifacts.

- `slug_analysis_v2 --test starlark_rule`: the exact
  `rustc_map_each::pinned_rustc_arguments_publish_and_restore_after_source_edit`
  selector passed 1/1 in 0.35s after 7.263s final preparation. It checks the real
  pinned builder, configured Spawn, retained multiline parameter policy,
  source A/B/A on one DICE service, non-string root-path rejection, nonempty
  optional callback rejection, source/derived File properties and root/external
  nested Label properties.
- The unchanged executable passed the three protected selectors
  `scalar_args_reject_vector_and_invalid_format_before_action_publication`,
  `vector_args_param_policy_and_args_write_use_one_generic_recipe`, and
  `args_run_and_symlink_snapshot_through_the_generic_action_sink`: 3/3 in 0.10s.
- `slug_build_api_v2 --test actions`: exact
  `pinned_regular_crate_root_retains_artifact_and_root_path_identity` and
  `vector_depsets_share_publication_alias_state_with_spawn_inputs` passed 2/2
  in 0.00s (0.003s wrapper), after 0.786s preparation. Distinct owners/root-path
  values retain unequal identity even when rendered paths match.
- `cargo check -p slug_query_v2 -p slug_reapi_v2` passed in 15.92s; existing
  REAPI deprecated-field warnings remain. Analysis test preparation also
  compiles the loading and action API owners.
- Source inspection verified all 55 closure file lengths/hashes and absence
  of missing or unused loaded bodies; generated proxy bytes exactly match the
  recorded upstream writer. One file-specific whitespace attribute preserves
  that writer's trailing spaces and final blank line without changing bytes.

No compiler action, daemon, network or full CLI test ran. Individual tests
remain subsecond. Elapsed packet wall time is not reliably available across
continuations; final preparation/test timings above are measured. Independent
final review returned ACCEPT for the pinned callback, demanded File/Label
properties and source-growth checkpoint. Formatting, staged diff and plan
status checks pass. The observed gate advanced is authentic configured Spawn
publication plus source invalidation/restoration; M7A remains open.
