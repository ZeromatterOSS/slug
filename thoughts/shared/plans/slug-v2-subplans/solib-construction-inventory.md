# Native Solib construction inventory

WP-7-54, source baseline `57856a758`, updated by WP755's production observation
and WP756's bounded construction result below. Baseline gap descriptions are
historical; the final section records the implemented boundary.
This inventory selects prerequisites for native construction; it admits no runtime behavior. WP752 regular-file
alias execution and WP753 Cargo Args remain accepted at their original boundaries.

## Demand and provenance

The predecessor records two SolibSymlink actions in WP714's CLI-root query,
`target/wp714/actions.json`, SHA256
`ffc50a44648f85b464988eb5dd6c6dd152269f342121a006839d3e8c7b4c0be8`.
That temporary file is absent from this checkout and the inspected sibling
checkouts/target roots. The historical receipt in Stage 10 proves its earlier
existence; it does not reconstruct its two owners, configurations, inputs,
outputs or downstream consumers. WP755 recovered current producer attribution below;
it does not reconstruct the old rows or their ambient options.

Pinned Bazel source is Git object
`8220c6198837d5c13d53fea211cf3282aa12408a` in the local
`/run/media/system/Colossus/dev/bazel-9.2.0` checkout (clean at audit).
Below, Java source paths are relative to
`src/main/java/com/google/devtools/build/lib/`; upstream test paths are relative
to `src/test/java/com/google/devtools/build/lib/rules/cpp/`.
Read immutable Git content when reproducing the audit.

Authenticated rules_cc 0.2.17 and rules_rust 0.73.0 expose several possible
routes. A source branch's existence does not prove that the CLI graph takes it:

| Producer route | Source condition and native call | Demand conclusion |
|---|---|---|
| C++ runtime libraries | rules_cc `cc/private/rules_impl/cc_toolchain_provider_helper.bzl:213` iterates `dynamic_runtime_lib[DefaultInfo].files`, filters shared-library names, and calls `cc_common.solib_symlink_action` | Candidate runtime-basename route; selected files/configurations unknown |
| Rust standard dylib | rules_rust `rust/private/toolchain.bzl:123` selects non-exec + experimental standard-dylib linking + available std_dylib; `rust/private/rust_allocator_libraries.bzl:186` calls public `create_library_to_link(dynamic_library=...)` | Conditional; cannot attribute the missing rows to stdlib |
| Rust cdylib | rules_rust `rust/private/rustc.bzl:2350` passes dynamic/interface libraries to public `create_library_to_link` without custom symlink paths; bin/proc-macro are excluded and lib/rlib use static input | Conditional default-mangled route on non-Windows; row attribution unknown |
| Generated cc_library shared library | rules_cc `cc/private/link/cc_linking_helper.bzl:497` uses `dynamic_library_symlink(..., False, False)` for NODEPS_DYNAMIC_LIBRARY unless neverlink/copy-dynamic-libraries suppress it | WP755 observes this route for rules_rust's C++ allocator, in target and exec configurations |

The runtime helper computes toolchain solib directory from target CPU and
runtime directory from `runtime_solib_dir_base` (`:196`).
`cc/private/rules_impl/cc_toolchain.bzl:87` supplies
`"_solib__" + cc_common.escape_label(ctx.label)`.
These are producer inputs, not strings to infer from an output path.
Runtime helper SHA256:
`92e52421866fb6fcba103159d17575480439aa0567d362937aca03aebb7372e3`;
toolchain SHA256:
`6a460affdf52e39bcc2ab1d4f7f5f6c135eaae24912a1d1a92f2b5b285321168`.
Both compare byte-for-byte with
`tests/v2_oracle/fixtures/configured-cli-authentic/mirror/github.com/bazelbuild/rules_cc/releases/download/0.2.17/rules_cc-0.2.17.tar.gz`.
The same archive's `cc/private/link/create_library_to_link.bzl:194` dispatches
non-Windows dynamic/interface libraries to the default mangled or explicit-path
constructor. Its SHA256 is
`5f57423312f24392f106aeb5959485c4f30c54ee2d8e926a45934de51a2455d1`.
Rust toolchain/allocator source hashes match the complete-source pins in
`app/slug_loading_v2/src/host_package_load_tests.rs:31013,30551`:
`c4b613cee96540a94fbdf4fbdca7b8dc4ef6d3082024c4d3636afc2e9c4d468e`,
`ae4acb50ac6a1b922254a07346d97b4649810d33836f2be4824fd0b7a81e536e`.
Rustc SHA256 `a7712508f50e5952f3f51e33c98acbe8ba39554e9c6f6edc26f16820d7c2a9e4`
is pinned in `app/slug_analysis_v2/tests/fixtures/rustc-map-each/sources.json`.
The existing rustc-map-each fixture authenticates Rustc source but uses focused
authored inputs; its success does not establish native toolchain execution.

## Bazel construction contract

`rules/cpp/CcStarlarkInternal.java:186,204,224,239` exposes runtime basename,
mangled dynamic, explicit-path dynamic, and pure soname helpers respectively.
`rules/cpp/SolibSymlinkAction.java` supplies the semantic owner:

- `:158`: mangling reads consumer label, configuration mnemonic, library
  root-relative path, solib directory, preserve-name and prefix-consumer flags.
- `:189`: explicit-path form checks input/output basenames, then appends the
  supplied path. PathFragment/ArtifactFactory normalization and containment
  rules also apply; do not invent an extra native validation from API prose.
- `:209`: runtime form uses input basename without parent/name mangling;
  explicit override wins over toolchain solib directory. The Java helper admits
  null override, while the exposed Starlark parameter is a required string.
- `:228`: all routes validate library filetype, reject an already-solib input
  root-relative string beginning `_solib_`, allocate a shareable derived File,
  and register a native action. There is no early-return reuse despite the comment.
- `:66,77`: one stable-order artifact input and one regular derived output;
  execution links the output file to the input file. It never links a parent
  directory despite the introductory comment.
- `:123,132,321`: native mnemonic is `SolibSymlink`, no progress message;
  action-specific key inputs are output exec path then input exec path, with
  no GUID. Execution platform is empty and execution properties are empty.
  The common `actions/ActionKeyComputer.java:37` envelope still applies.

`analysis/RuleContext.java:319,711` places shareable artifacts relative to the
configuration's bin root, bypassing package prefixing. The repository argument does
not imply a repository-specific root: `analysis/config/OutputDirectories.java:147`
uses a shared bin root under default nonsibling layout; only sibling layout uses
repository-specific roots (`:127`). This corrects WP754's earlier overstatement.
`analysis/CachingAnalysisEnvironment.java:262` preserves artifact
ownership. Shared paths never authorize reconstructing ownership from a filename.

`actions/Actions.java:45,376` escapes underscore, slash, backslash, colon and
at-sign as `_U/_S/_B/_C/_A`. Solib `:273` preserves the basename when requested,
mangles its parent and optionally prefixes the consumer label; otherwise it uses
`lib` plus escaped full root-relative path. `:306` includes a configuration
mnemonic suffix starting at `ST-` for non-preserved names. `:249` hashes an
over-255-Java-code-unit component with SHA256 UTF-8 while preserving its final
extension; the preserved library basename itself is not shortened.
Rust valid-Unicode behavior and Slug-native configuration/output projections
must remain explicitly classified before admitting this algorithm.

`rules/cpp/Link.java:39` and `CppFileTypes.java:243,247,255` admit shared,
versioned shared, interface-shared and extensionless names at the native layer.
The public rules_cc constructor validates a narrower context-dependent set.
Output path validity also comes from `actions/ArtifactFactory.java:302,333`.
Execution IO failures and logging IO failures differ (Solib `:82,102`).

Native Solib is not ordinary `analysis/actions/SymlinkAction.java`: the latter
has its own GUID/optional-path fingerprint (`:429`), target policies, and
Symlink/ExecutableSymlink mnemonic (`:447`). Reusing alias mechanics cannot
erase this distinction or confer an exact ActionKey claim.

## Slug owners and missing prerequisites

| Boundary | Existing owner | Required decision before implementation |
|---|---|---|
| Native binder | `app/slug_loading_v2/src/cc_common.rs:185`; restricted internal API access at `:288` | Add only the demanded native method; preserve caller restrictions and active-context checks |
| Action bridge | `app/slug_loading_v2/src/subrule_invocation.rs:127,1164` | Route through existing AnalysisActionSink/token lifetime, never create ownership in Loading |
| Bin-relative output | `app/slug_analysis_v2/src/starlark_rule.rs:1061` | Ordinary declaration prefixes package; introduce a narrow native declaration and preserve input repository identity in naming |
| Owned artifact/action | `app/slug_build_api_v2/src/analysis_value.rs:327`, `actions/spec.rs:956,968,1264` | Preserve configured owner and complete target artifact; add structural native family identity |
| Registration/conflicts | `app/slug_build_api_v2/src/actions/registry.rs:57,80` | Current registry rejects all duplicate outputs; specify equivalent native sharing and conflicting-input rejection |
| Execution dispatch | `app/slug_core_v2/src/runtime/action_prerequisites.rs:301`, `action_prerequisites/symlink.rs:8` | Any current symlink_spec enters generic alias dispatch; new family must be gated deliberately |

Derived artifacts already retain structural configuration and owner; `path` and
`short_path` remain Slug-native projections (`analysis_value.rs:348`).
Physical `configured_output_root` is configuration-only
(`app/slug_core_v2/src/runtime/configured_output.rs:225`).
`app/slug_core_v2/src/runtime/configured_action_closure.rs:122,154` also groups
conflicts by configuration bytes plus output path, without consumer repository.
The normal sink receives only the package path (`app/slug_analysis_v2/src/dice.rs:2444`).
The observed default Bazel Solib route also shares that bin root: repository separation
enters the mangled **input root-relative path**, not an extra consumer-repository root.
Ordinary Bazel external outputs receive `external/<repo>/<package>` through
RuleContext.java:730 and PackageIdentifier.java:188; shareable Solib bypasses this.
A Slug producer must model the input root-relative/repository projection and preserve
structural ownership. Adding a namespace is a possible Slug-native choice, not a
requirement proven by Bazel's bin-root API. Keeping repository only in artifact
identity cannot resolve an otherwise-colliding physical placement.
Relaxed output bytes never permit conflating two semantic repositories.

Bazel `actions/Actions.java:85,191,200,225` tolerates equivalent shared actions
and rejects conflicting generators. Matching mnemonic/key plus ordered mandatory
inputs/outputs, ignoring owner where shareable, matters; output path equality
alone is insufficient. A Slug implementation must preserve original artifact
owners while choosing any execution representative, include every semantic input
in equality/invalidation, and distinguish same-owner and cross-owner registration.
The WP754 baseline permits repeated declarations of the same kind but rejects repeated
registrations. Cross-owner sharing is separately restricted to scalar FileWrite
(`configured_action_closure.rs:177,221,236`); other colliding families yield
UnsupportedEquivalence. Both layers need an explicit native Solib contract.

Configured-analysis DICE values and their existing source/repository/configuration
dependencies remain the natural producer. Construction must perform no filesystem
discovery, introduce no semantic side cache or global registry, and retain no
evaluator heap or command scratch. A future representation change requires the
utility-reuse skill and Stage 9 review; WP756 records that reuse in the Stage 9 ledger.

## Evidence and selected successor

Adapt upstream `StarlarkCcCommonTest.java:1713,1757,1809` for default/custom/long
Solib names; `CcLibraryConfiguredTargetTest.java:1954,2479,2530` for duplicate
reuse, transition naming and long filename behavior; `CcBinaryThinLtoTest.java:326`
for link-action input topology. These tests were read, not run. Broad C++ compile,
LTO and platform suites cannot establish a bounded Slug constructor by themselves.

Reuse Slug's structural symlink test
`app/slug_build_api_v2/tests/actions.rs:1359`, owner rejection in
`app/slug_analysis_v2/tests/starlark_rule.rs:9469`, and regular alias prerequisite
negatives in `app/slug_core_v2/src/runtime/action_prerequisites/symlink_tests.rs:82`.
Future proof must add the selected native method, repository/bin placement,
duplicate/conflicting generator cases, source/config A/B/A restoration, and explicit
non-admission of unsupported constructors/execution. Existing alias backing tests
remain evidence for their own family only.

WP755's filtered CLI-root observation succeeds with two Solib actions, both owned by
`@@rules_rust+//ffi/cc/allocator_library:cc_allocator_library`, under k8-fastbuild and
k8-opt-exec. The [durable evidence](./solib-demand.json) retains full raw graph and
source attribution. Both inputs are the declared shared-library outputs of cc_library;
the source route calls `dynamic_library_symlink(..., False, False)`. Native mangling
of `external/rules_rust+/ffi/cc/allocator_library/libcc_allocator_library.so` explains
the exact observed `_solib_k8/libexternal_Srules_Urust+_Sffi_Scc_Sallocator_Ulibrary_Slibcc_Uallocator_Ulibrary.so`.
Neither mnemonic contains ST-. Serialized owner platform is `@@platforms//host:host`,
distinct from the native action's empty platform override used by key semantics.

WP756 implements the demanded non-preserved, non-consumer-prefixed generated-File
constructor through a distinct SolibSymlinkSpec and active typed analysis bridge.
Input naming projects main output.path directly, external inputs as
external/<input-owner-repository>/output.path, without changing input identity;
output declaration is bin-relative without consumer package/repository prefixes.
Full configured input identity governs duplicate sharing and collision rejection.
The projection omits unowned Bazel ST mnemonic bytes explicitly as Slug-native.
Pinned escaping/long-name hashing and native filetype predicates have focused
Rust proofs; the unchanged authenticated helper exercises the real private binder,
and separate production-sink tests prove artifact/action publication through the
same bridge. This is a segmented leaf proof, not full-rule analysis or DICE replay.
Full allocator cc_library compilation/linking remains separate: native feature,
compile, link-action wrapping and toolchain APIs are additional prerequisites.
The filtered graph proves registration in root analysis, not downstream selection or
execution of either Solib output. No broader query/build or F3 replay is implied.

Existing exact slices are unchanged. True flags, source/tree inputs, custom-path
and runtime constructors, ownerless alias sharing, native Solib execution,
aquery projection and exact Bazel ActionKey remain unsupported/deferred.
Requested output and Spawn-prerequisite tests protect the execution boundary.
M7A is partial and M8 remains unproved.

WP757's [consumer observation](./solib-consumption.json) selects backing libraries
and Solib outputs through one input regex. Both producer controls are present;
neither output has a direct declared/scheduling consumer under the recorded
pruned-input/aspect flags. The raw dump equals WP755 byte-for-byte. This is a
bounded negative, not absence from runfiles/default outputs or proof of selected
execution. Static/PIC preference is a source-grounded explanation, not an observed
static-input inventory. Keep native execution deferred and advance the preceding
C++ feature-model/configuration prerequisite instead.
