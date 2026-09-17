# Current Slug V2 Work Packet

Packet: WP-7-25-m7a-rustc-native-link-args-r1
Status: accepted; independent final ACCEPT; ready to integrate

## Outcome and compatibility

Admit the pinned rules_rust native-library directory and default-platform link
flag callbacks into immutable retained Args recipes. The authentic Linux Rustc
`construct_arguments` proof will produce native search paths, static/dynamic
flags, direct/indirect whole-archive flags, ambiguity substitutions and user link
flags, including their forced parameter bytes and same-DICE source restoration.
This advances the bootstrap Rustc/linker argument gap. Exact mapping/order and
ASCII numeric-version stripping follow pinned source; File paths/short_path
retain Slug-native spelling. Conservative input-shape rejection is Slug-native.

No Cc constructors, Cc toolchain discovery, compiler execution, Windows/Darwin
flag families, C++ runtime library callbacks, tree artifacts or M7A completion.
The proof supplies explicit toolchain/linker-input structs to the unchanged
upstream builder, as existing proofs do. It selects the real Linux fallback Rust
linker path with cc_toolchain=None; no fake native API or copied function body.
Actual native Cc object production/lowering remains a prerequisite to full CLI.

## Source and ownership

Pinned rules_rust 0.73.0 rustc.bzl SHA
`a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`:
`_libraries_dirnames` at add_all line 2881, `_make_link_flags_default_direct` /
`_make_link_flags_default_indirect` at line 2896, and `_get_dirname` at line 2890
for ambiguous-library search paths. Imported utils.bzl SHA
`8aa49b9312d4ae5c4aed033aba65392a039a681b3ee21ca83da0f05acac28ace`:
`get_preferred_artifact` and `get_lib_name_default`. Bazel 9.2
8220c6198837d5c13d53fea211cf3282aa12408a LinkerInputApi supplies sequence fields;
StringModule.isDigit specifies nonempty ASCII [0-9]. Existing Args mapping and
forced-file source anchors/contracts remain inherited from WP-7-20/23.

Loading authenticates caller source/manifest/span as before; the two new mapping
sites also require exactly one canonical utils provenance row with the pinned
observed SHA. Pinned rustc relative loads and immutable bindings fix the helper
identities. The link callback repr distinguishes only the two fixed default
branches after authentication; unknown branches fail closed. No host reads,
callable invocation or evaluator retention. Ambiguous directory uses the existing
regular-File recipe after caller authentication.

New Build API `rust_native_link_args.rs` owns RetainedRustNativeLinkArgs with
immutable Arc<[AnalysisValue]> input rows and a closed mapper enum. Analysis
lowers complete `(linker_input, use_pic, ambiguous_libs, include_link_flags)`
tuples with the existing shared AnalysisValueLowerer. Struct/provider fields,
sequence shape, unused values and alias/depset sharing remain structurally
retained; publication uses the same PublicationEqState as other Spawn inputs.
Constructor validates regular File/None library slots, bool selectors/alwayslink,
string user flags and string-to-regular-File ambiguity maps. Libraries and user
flags are ordered sequences. No flattened string cache or parallel identity.

The recipe implements preferred PIC/static/interface/dynamic selection,
per-row directory dedup before generic uniquify, whole-archive ordering,
include_link_flags suppression (alwayslink/user flags survive), static library
sandwich flags, standard libstd/libtest special case, versioned dynamic names,
and ambiguity remapping only where the pinned portable flags apply. Full input
values remain the owner even if a selected branch does not render a field.
This is DICE-retained semantic data using existing Arc/Allocative/SmallSet
patterns (Stage 9); rendering is action scratch, released with its consumer.
No new DICE key, lock or global cache; source/action invalidation stays owned by
existing loading/configured dependencies.

## Scope and gates

Allowlist: Build API actions/{rust_native_link_args.rs,mod.rs,spec.rs}, lib.rs,
and tests/actions.rs plus a focused tests/native_link_args/mod.rs included
module and BUILD.bazel src declaration if needed; loading subrule_invocation.rs;
analysis starlark_rule.rs and tests/rustc_map_each/{mod.rs,subject.bzl}, existing
fixture.toml provenance; canonical/manifest and Stage 6/7/9/bootstrap summaries.
New recipe and focused API tests get separate cohesive files; existing large
files receive bounded dispatch. No new copied Bzl bodies or fixture closure.

API proof covers selection priority/fallback, regular-file rejection, both
mapper branches, name edge cases, ambiguity, bool/type negatives, order,
include flags, retained unused-field/shape/alias identity. Real-source proof
covers nonempty Linux native rows, direct/indirect selection, include false,
input/source A/B/A, changed imported utility rejection, generated param bytes
and a retained unused-callable negative. Preserve prior root/provider callback
proofs and generic Args. Compile direct query/REAPI consumers. Exact preflight
and small subsecond test batches; separate pinned-nightly preparation operations
capped 60s, rustfmt/diff/archive/plan checks. Tests above a few seconds run rarely;
above roughly 30s require strict necessity scrutiny. No broad/daemon/transport
or Bazel/compiler action. Independent design and final review required.

If source provenance cannot authenticate a transitive helper, or real builder
reaches unowned native behavior, resolve that dependency before widening the
recipe. Predecessor WP-7-24 at 3a72e4d7a supplies observed per-module SHA transport;
M7A and typed Spawn execution remain open.

## Acceptance receipt

The retained recipe implements the pinned native-link projections with full
tuple ownership and shared publication equality. Authentic Linux builder tests
prove direct/indirect selection, include-flags suppression, ambiguity, user
flags, forced parameter bytes and imported-source rejection/restoration.
Cross-Spawn depset alias tests and unused-callable rejection protect ownership.
No copied upstream bodies or fixture files were added.

Pinned nightly direct binaries compiled the final API tests in 1.71s and
analysis tests in 34.43s, separately from execution and within the 60s preparation
cap. Exact-selector preflight and execution passed:

- API 5/5 in 0.00s (0.002s wrapper): three native-link tests plus existing
  ArgsWrite formatting and Rust crate retained-shape/alias checks.
- Analysis 4/4 in 0.45s (0.461s wrapper): native-link source authentication and
  restoration, prior Rustc root/dependency callbacks, and generic Args policy.

`cargo check -p slug_query_v2 -p slug_reapi_v2` exited 0 in 14.47s, with three
existing REAPI deprecation warnings. Rustfmt, diff, archive and plan checks pass.
Receipts are in `target/wp725/`. Packet/review wall time was not separately
measured. No broad suite, daemon, compiler action or live transport ran.

Independent final review ACCEPT found no material algorithm, provenance,
retained-value or scope blocker. The observable advance is native-link Args
publication and forced parameter bytes through the unchanged Rustc builder.
Native Cc construction/lowering, toolchain discovery and resolved Spawn execution
remain required; this checkpoint does not close M7A or full CLI buildability.
