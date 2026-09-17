# Current Slug V2 Work Packet

Packet: WP-7-22-m7a-rustc-crate-dependencies-r1
Status: accepted; reviewed checkpoint ready for integration

## Outcome and compatibility

Admit the two demanded nonempty provider-depset sites in pinned rules_rust
0.73.0 `add_crate_link_flags`: direct `--extern` arguments (ordinary and metadata
variants, including alias names) and transitive `-Ldependency` directories.
Reuse the real builder/load closure accepted at `ff14d4e33` and dirname
checkpoint `73ba48a41`; add no upstream source files. The result is configured
Spawn publication with these arguments and retained provider/artifact identity.
M7A, generic callbacks, native linking, compiler execution, virtual param-file
staging, tree expansion and path stripping remain open.

Algorithm/order/metadata selection are exact for the admitted provider shapes;
artifact path bytes and eager rejection timing remain Slug-native. Only actual
CrateInfo and AliasableDepInfo from their canonical pinned rules_rust module
labels are admitted, with names as strings, selected output as regular File, metadata as
None/regular File, and a boolean pipelining flag when metadata exists. Fields
not read by the selected callback need no new restriction. No duck-typed custom
struct, foreign provider, arbitrary callable or tree artifact is admitted.

## Source, authentication and ownership

Bazel 9.2 source commit `8220c6198837d5c13d53fea211cf3282aa12408a` Args.java
validateMapEach/addVectorArg and StarlarkCustomCommandLine.applyMapEach retain
the existing validation/order contract. rules_rust rustc.bzl lines 2554–2635
own both sites (one-based native callee lines 2573 and 2575), alias unwrapping,
metadata fallback and directory rendering. Source SHA stays
`a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`.
Provider declarations are rustc.bzl AliasableDepInfo and providers.bzl CrateInfo.

Loading first authenticates the existing canonical loaded identity/SHA/span.
At line 2573 the pinned source selects one of two immutable module functions.
Distinguish only those two exact function repr strings, including the same
source filename, after authentication; unknown forms fail closed. The local
starlark-rust DefGen::collect_repr supplies this documented discriminator; it
is not a standalone identity or a replacement for source authentication. No
callable or evaluator pointer crosses publication. Line 2575 has one fixed
mapper. Existing positional/callable/format validation stays before omission.

Analysis lowers the whole provider depset through the same AnalysisValueLowerer
as other action values. A dedicated actions/rust_crate_args.rs owns a validated
RetainedRustCrateArgs (private depset and mapper fields), RustCrateArgMapper
(Extern, ExternMetadata, DependencyDir), and a typed error. Constructor validation
checks actual provider identities and only callback-demanded fields. Metadata
None falls back to output without reading the pipelining flag; metadata File
uses it only when the boolean flag is true. Directory mapping reads output.
Alias names come from AliasableDepInfo, while its dep is an actual CrateInfo.

Retain the original AnalysisDepset, all provider fields, artifact owners and
mapper enum. No flattened string/path side store, provider rewrite or parallel
cache. Rendering visits the retained graph and uses existing path/dirname
projections before generic formatting/uniquification. Publication equality
uses one shared PublicationEqState, preserving provider/depset aliases and
structural shape across action values. Existing Bzl/package/configured DICE
edges own source/provider edits and A/B/A (docs/developers/dice.md); no new
key, lock or host read. Configured-action retained memory follows the existing
Arc lifetime, with rendering scratch released per use. No performance claim.

## Scope, evidence and validation

Allowlist: app/slug_build_api_v2/src/actions/{rust_crate_args.rs,mod.rs,spec.rs},
src/lib.rs and tests/actions.rs; app/slug_loading_v2/src/subrule_invocation.rs;
app/slug_analysis_v2/src/starlark_rule.rs and tests/rustc_map_each/{mod.rs,subject.bzl};
fixture.toml provenance only; canonical/manifest and Stage 6/7/bootstrap gate
summaries at acceptance. New provider mapping lives outside the large spec owner;
existing lowering/Args owners retain small dispatch changes. Upstream fixture
bytes/inventory stay unchanged and reuse the accepted growth review.

The real builder test loads real CrateInfo/AliasableDepInfo, covers ordinary vs
metadata selection/fallback, alias names, force-all-direct, transitive dirname
dedup/order, argv and multiline bytes, and dependency-input edits/restoration
in one DICE service. Preserve loaded-source A/B/A, generic callback negatives,
root/dirname proofs; replace the former nonempty-provider rejection with an
invalid-provider discriminator. API tests cover malformed/foreign providers,
non-Files/directories, alias/mapper/producer distinctions with identical rendered
bytes and retained depset shape/alias identity.

Pinned nightly preparation stays separate under 60 seconds. Exact-selector
preflight and small focused owner/protected test batches are expected subsecond;
query/reapi direct dependents compile once. Run rustfmt, diff and plan checks;
independent design and final review precede integration. Tests over a few seconds
run infrequently and anything over roughly 30 seconds needs strict necessity
scrutiny. No full CLI, daemon, Bazel build, compiler action or network test.
If the function discriminator is ambiguous or provider lowering needs new
semantic ownership, resolve the concrete prerequisite before extending scope.

## Validation receipt

Base `73ba48a41`; pinned nightly-2025-09-14 direct binaries after the unavailable
snap rustup launcher. Cargo `--no-run --message-format=json` produced the named
executables; all selectors passed exact nonignored preflight. Local receipts
are `target/wp722/{api,analysis}-tests.log` and `dependent-check.log`.

- API `actions`: 6/6 in 0.00s (0.003s wrapper), compile-only preparation 4.05s.
  New selectors `rust_crate_args_validate_provider_fields_and_retain_identity`
  and `rust_crate_args_preserve_depset_alias_and_retained_shape` pass, alongside
  the four WP-7-21 API selectors. They discriminate mapper, alias/provider and
  producer identity with equal rendering, shared/split depsets and retained
  graph shape, foreign/malformed providers, metadata selection/fallback and
  regular-File validation.
- Analysis `starlark_rule`: 6/6 in 0.47s (0.481s wrapper), preparation 35.24s.
  The new `rustc_map_each::pinned_rustc_dependencies_select_aliases_metadata_and_restore`
  passes both actual callable repr branches, metadata None short-circuit,
  disabled-pipelining fallback, aliases, force-all-direct, directory ordering/
  deduplication, argv/multiline bytes, and provider-input edit/restoration on
  one DICE service. An unused callable field is rejected by full value lowering.
  Both preceding rustc_map_each selectors and the three protected Args selectors
  from WP-7-21 pass unchanged apart from the now-invalid-provider negative.
- `cargo check -p slug_query_v2 -p slug_reapi_v2`: exit 0 in 15.01s; existing
  REAPI deprecated-field warnings remain. Formatting, diff and plan checks pass.
  Upstream fixture Bzl files/inventory are unchanged; prior provenance and growth
  review are reused. No test exceeded one second; compiler preparation is
  separate. No daemon, Bazel invocation, compiler action or network test ran.

Independent final review returned ACCEPT for authenticated selection, full
provider retention, structural equality and the recorded discriminators. The observable gate advanced is configured
publication of the two demanded nonempty provider callbacks; full M7A is open.
