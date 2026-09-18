# Current Slug V2 Work Packet

Packet: WP-7-28-m7a-repository-source-paths-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and evidence

Ordinary source-input staging exposed a prerequisite: AnalysisArtifact::Source
currently renders package/target for every repository. External compiler roots,
rlibs, native libraries and tools therefore lose their repository prefix. Fix the
shared artifact path owner so File/Args/Spawn consumers all render repository-aware
paths. Independent review found that analysis also rejects external SourceFile
nodes and assumes main-workspace paths. Admit materialized external source files
through the existing canonical repository/materialization path owner in this
cohesive checkpoint; preserve the authentic external-root acceptance gate. Preserve distinct execution and short paths, including rules_rust's
ambiguous-library lookup which currently conflates them.

Demanded by the production external crate/toolchain closure in bootstrap-readiness
(rows rules_rust evaluation, Spawn and ordinary input transfer). Pinned Bazel 9.2
8220c6198837d5c13d53fea211cf3282aa12408a: RepositoryName.java getExecPath(false)
and getRunfilesPath; Artifact.java getDirname (384-395), getExecPathString
(526-528), getRunfilesPath (636-666). Existing pinned rules_rust 0.73.0
rust/private/rustc.bzl portable_link_flags (2654-2657) looks up ambiguous libraries
by artifact.short_path; alwayslink arguments use execution path.
ConfiguredTargetFactory.java InputFile branch (316-335) supplies the input file
execution path and owning package source root to the source artifact factory.

Admit default nonsibling source layout: main source path/short_path is
package/target; external execution path is external/<canonical repo>/package/target
and short_path is ../<canonical repo>/package/target. Empty package handling and
dirname are exact for supplied canonical repository identities. Canonical repo
naming remains the existing Slug-native owner. Generated artifact output paths
and existing short-path projection remain Slug-native, not exact root extraction.
Sibling layout, path mapping and derived-root short-path parity remain deferred.
Do not infer a source's host repository root from its rendered execution path.

## Ownership and invariants

AnalysisArtifact's existing canonical Source label remains identity; paths are
scratch projections, never new retained identity/cache/DICE state. Implement
execution and short-path projection there, reuse from AnalysisArtifactValue and
native-link recipe. Preserve main and derived behavior and File hashing/equality,
retained input identity, argument ordering/deduplication, forced parameter bytes.
Do not weaken provider identity or imported-callback authentication. Native-link
lookup uses short_path, but directories and alwayslink flags use execution paths.
No new root registry, string parsing of host roots, dependency, lock or storage.
Keep scratch String/Cow allocation and existing Arc/Allocative carriers.

## External source routing owner

Expose the existing Bzlmod HostRepositoryPathKey/ObservationKey and read-only
result accessors for configured analysis. Their existing driver owns validated
repository-relative paths, materialization request/Need, local Host versus
immutable Materialization namespace, resolution and complete observation frontier.
Do not add another path or materialization key. Analysis's new cohesive
source_file module obtains HostCanonicalRepositoryLoadRouteKey/ObservationKey
from workspace plus canonical repository and passes its retained source input
to the existing path owner. Main sources reuse their existing resolver.

Widen external configured-target admission only to Null ExportedFile and the
Null absent-target case that is still guarded by package_declares_source_label.
Unrelated external target shapes stay closed. Only regular files become source
nodes; missing/nonregular files keep existing diagnostics. Built-in catalog
source files have no materialized host path and remain explicitly unsupported
by this path owner, pending their immutable-source adapter; no fabricated root.
This covers materialized local/registry/generated repositories supported by the
existing route owner, without claiming generated action outputs are source files.

Observed analysis must compute observed route/path producers and preserve Need.
Match compute_configured_package_input: canonical-load-route outer errors become
diagnostic AnalysisError (invalid analysis); the path producer outer
ObservedPathFrontierError stays outer. Neither error becomes success. Analysis
returns source semantics only, so both observed producers remain DICE dependencies
for transitive capture; no combined observed result is exposed or invented. Core's existing
transitive DICE observation capture/certificate mechanism remains the request
owner; no detached reads or historical-host snapshot assumptions. Reuse existing
path/materialization DICE tests and docs/developers/dice.md. Public visibility
changes add no new retained graph representation, lock or lifecycle. Canonical
route/materialization changes remain tracked dependencies even when the source
artifact's rendered path is unchanged. Final request validation remains required.

## Scope and validation

Allowlist: Bzlmod source_preparation.rs/lib.rs for existing path-key visibility
and read-only accessors only; analysis dice.rs (source dispatch/guard) and new
dice/source_file.rs, existing root/source lifecycle tests and Bzlmod source_preparation_observation_tests.rs
only for a public-wrapper discriminator if needed, plus fixture request
helper for observed mode/missing-path injection; Build API analysis_value.rs,
actions/rust_native_link_args.rs; loading
subrule_invocation.rs File dispatch; focused API source-path tests (new module
under existing actions tests and native_link_args test module); analysis
rustc_map_each/{mod.rs,subject.bzl,external_sources.rs} and existing fixture.toml; canonical/manifest
and Stage 6/7 summaries. Large source files receive bounded methods/dispatch;
new proof is a cohesive test module where practical. No copied implementation
or expanded top-level load closure. Same 54 pinned source files.

Exact source path/short_path/dirname/basename cases for main/two external repos,
empty/nested packages and slash-containing targets. Retained Args/Spawn executable
and forced param bytes distinguish identical basenames across repositories.
Native-library ambiguity keys prove short-path selection and reject execution-path
aliasing; alwayslink/search directories retain execution paths. Authentic unchanged
Rustc builder runs with external root File, checks File properties and rendered
root argument, then source repo A/B/A restores the configured result in one DICE.
Exercise observed and legacy analysis, external deletion/recreation despite a
same-named main source, wrong-kind rejection and restored equality. Reuse owner
tests for materialization namespace and source-path frontiers; add a direct
public-wrapper gate if existing coverage does not prove the exposed boundary.
Preserve existing root/dirname and native-link authentic proofs. A source label
change must remain a semantic input even if contents/basenames agree.

Independent design and final review required for shared public path behavior.
Use pinned-nightly compile-only preparation <=60s each, exact-selector preflight,
subsecond focused tests. Tests >few seconds infrequent and >roughly30s require
strict necessity; no such test planned. Direct API/loading/analysis plus Core/
query/REAPI compile coverage as required by this shared projection. No full suite,
Bazel build, compiler action, daemon or live transport. Format/archive/plan/diff.
Resolve any caller relying on package-relative artifact.path before activation;
repository routing/input upload and resolved Spawn execution remain required.

Predecessor WP-7-27 accepted/pushed at 994abb76c: streamed digest/size observation,
complete provenance and semantic equality cutoff; 13 focused gates and independent
final ACCEPT. Its Windows native validation remains explicitly unverified.

## Acceptance receipt

Independent revised design and final review ACCEPT. Baseline 994abb76c; candidate
on review/wp728-repository-source-paths. Gate advanced: configured materialized
external SourceFile admission and repository-aware File/Args/Spawn source path
projection, including authentic pinned Rustc root rendering and observed routing.
No retained representation, path/materialization key or imported source was added.

Validation used direct nightly-2025-09-14 binaries because the rustup snap launcher
cannot run for this user. Cargo test preparation used --no-run --message-format=json
and a 60-second operation cap; selected executables were preflighted exactly.
Commands and local raw receipts are in target/wp728 (not committed):

- cargo test -p slug_build_api_v2 --test actions --no-run: 4.096s, then 1.246s
  for a corrected expected user-link flag spelling. Two source_paths selectors,
  external_libraries_use_short_keys_and_execution_paths, existing
  file_dirname_recipes_preserve_identity_and_map_before_uniquify and
  preferred_library_flags_and_directory_order_follow_pinned_source: 5/5 proved
  across the initial four passing selectors and corrected single selector.
  Runtime batches 0.004s and 0.001s.
- cargo test -p slug_analysis_v2 --test starlark_rule --no-run: 51.334s, then
  7.485s after supplying the empty-directory Lstat omitted by the fixture's
  snapshot helper. Existing pinned_rustc_file_dirnames_preserve_order_and_generated_root
  and pinned_rustc_native_link_flags_authenticate_imports_and_restore passed in
  the initial 0.515s batch. Corrected
  rustc_map_each::external_sources::external_source_roots_route_render_and_restore
  passed in 0.590s. Together 3/3 proved, including legacy/observed A/B/A,
  missing external despite main shadow, nonregular rejection and restoration.
- cargo test -p slug_bzlmod_v2 --lib --no-run: 52.520s. Existing
  source_preparation::tests::observation_tests selectors
  observed_host_source_preserves_exact_symlink_epoch_and_isolates_families,
  observed_host_source_preserves_immutable_and_file_error_prefixes and
  observed_host_source_covers_path_need_route_error_and_reverse_isolation:
  3/3 passed in 0.011s; public analysis use is also proved by the authentic gate.
- cargo check -p slug_core_v2 -p slug_query_v2 -p slug_reapi_v2: exit 0 in
  25.156s, including API/loading/analysis dependency compilation.
- Changed Rust rustfmt --check, git diff --check, v2_plan_status.py and
  v2_archive_status.sh passed. All 54 pinned source sizes/SHA-256 hashes match;
  no copied fixture growth or load-closure change.

Compilation totaled 141.837s across six separate preparations/checks, each below
60s. Test batches totaled about 1.12s including the two corrected fixture
assertions; no long-running test, daemon, compiler action, Bazel or transport ran.
Packet/review wall time was not continuously measured across the user pause.
The initial path-only design was expanded after review found external SourceFile
admission missing; the authentic external-root requirement was preserved.
Builtin catalog source files, generated-root short-path parity, input digest
staging/upload, toolchain discovery and resolved Spawn execution remain open.
