# Current Slug V2 Work Packet

Packet: WP-7-176-m7a-rust-stdlib-solib-construction-r1
Status: accepted; independent DESIGN/FINAL accepted; M7A remains partial

## Outcome and evidence

Baseline `8c427a879` (WP875). Remove a concrete full-callback prerequisite:
actual allocator_libraries.bzl dynamic-stdlib creation invokes rules_cc
create_library_to_link.bzl:220 -> cc_internal.dynamic_library_symlink with
preserve_name=True, prefix_consumer=True, and Rust stdlib source Files. Current
Loading rejects either flag; BuildAPI rejects all source inputs. Admit the four
flag combinations and source/generated regular Files in the existing native
Solib construction family. Preserve complete artifact and consumer configuration
identity. No callback publication-only substitute: WP875's complete finite helper
and source/private-authority implementation remains the next objective.

Pinned clean Bazel9.2 `8220c619`: CcStarlarkInternal.java:205–221 passes all five
positional arguments; SolibSymlinkAction.java:158–172 uses consumer owner label,
configuration mnemonic and input root-relative path. :228–245 validates shared
library filetypes and refuses already-Solib inputs without requiring a generated
artifact. :249–258 hashes oversized components using UTF16 length/UTF8 SHA256 and
preserves final extension; :273–319 covers all flag combinations. True preserves
basename beneath escaped parent directory; prefix adds escaped consumer label.
False/False existing WP756 proofs remain authoritative. Public source/derived
eligibility, flag binding and branch structure are source-exact; Slug structural
configuration, canonical label spelling, artifact/root paths and resulting names
remain Slug-native (no Bazel ST mnemonic/output byte parity). Solib execution,
REAPI projection, dynamic_library_symlink2 and full Rust bootstrap remain deferred.

## Owners and invariants

BuildAPI SolibSymlinkSpec remains the existing two-field immutable owned
input/output model. Constructor accepts consumer CanonicalLabel and both flags;
for source inputs use the existing artifact source path projection, for derived
inputs use root_relative_path. Validate regular File (not directory/symlink or
WorkspaceStatus), canonical relative paths, shared-library names and Solib-input
refusal before constructing output. Source label and derived configured owner,
output root/kind/metadata remain structural; naming never reconstructs identity.
Preserved-name directory hashes only escaped directory/consumer component and
keeps library basename unchanged. False branches keep current soname policy.
Consumer prefix uses the current action sink's owner label, never producer label.
No new retained flags/strings, cache, DICE key, interner, registry, heap borrow or
code carrier. Naming buffers stay phase construction scratch. Existing Stage9
WP756/766 AnalysisArtifact/root/Allocative and registry reuse disposition applies;
no donor code or new retained representation is selected.

Loading forwards genuine booleans through existing AnalysisActions and
AnalysisActionSink signatures. Active/suspended/expired action tokens remain
mandatory before delegation. Analysis uses its live consumer owner/root and the
same declare/register path; repeated identical typed actions preserve existing
index and collisions reject, including complete source/derived identity and
nondefault generic execution envelopes. Both analysis routes share this sink.
No locks cross DICE await, new observations, source scans or invocation capability.
Windows remains minimal/deferred; no platform execution change.

## Scope and validation

Allowlist: app/slug_build_api_v2/src/actions/solib_symlink.rs;
app/slug_loading_v2/src/cc_common.rs;
app/slug_loading_v2/src/cc_common/solib_tests.rs;
app/slug_loading_v2/src/subrule_invocation.rs;
app/slug_analysis_v2/src/starlark_rule.rs;
app/slug_analysis_v2/src/starlark_rule/solib_tests.rs;
app/slug_core_v2/src/runtime/action_prerequisites/solib_tests.rs (two existing
constructor callsites only, discovered by live search);
manifest/canonical and Stage6 contract. Update other direct signature callsites
only if live search demonstrates them, recording the bounded delta. Production
estimate under100 lines, proofs under200; review triggers, not user ceilings.
Touched large files receive local signature/delegation edits only; naming and
focused tests remain in their existing cohesive Solib modules.

Independent DESIGN before edits and FINAL on actual implementation/proofs.
Reuse pinned-source WP756 false/false filetype/Unicode/hash/conflict and token
proofs; add all flag branches on root/external source/generated inputs, consumer
prefix separation, preserved-name long-directory/hash behavior, complete typed
registry snapshots on identity/type/envelope conflicts, source/configuration
A/B/A and restoration. Loading binder accepts/forwards all four flag pairs and
still rejects invalid types/binding; production sink proves current consumer
root/owner/input and active-token behavior. Full rules_cc/helper dispatch tests
will belong to the callback packet; no new copied source/archive here.
Reuse the existing unchanged create_library_to_link.bzl asset for one focused
True/True source-branch-to-binder discriminator; unused imports must fail on call.

Resolve pinned toolchain; serialize no-run Cargo JSON prep bounded60s, preflight
exact nonignored tests, run focused BuildAPI/Loading/Analysis selectors bounded20s
per executable. Compile direct Core and rebuilt CLI dependents. No authentic
replay required: this prerequisite cannot advance the still-unimplemented callback
publication refusal, and unchanged WP874 replay already identifies that boundary.
Diff/fmt/plan/archive checks; actual pre/post slug/slugd hygiene for any runtime.
No performance claim; source eligibility/naming are not proof of execution.

Independent DESIGN/FINAL accepted. Seventeen exact nonignored focused tests pass:
BuildAPI6/6 in0.114s, Loading4/4 in0.032s, Analysis4/4 in0.007s and Core3/3 in0.016s
(total0.169s). Serial no-run prep2.821/12.687/11.635/54.868s; corrected Analysis
fixture prep2.070s, rebuilt CLI10.640s pass. Initial Analysis fixture omitted its
retained execution Host OS; adding the Linux fact fixes target/exec restoration,
with failure receipt preserved. Source-artifact A/B/A distinguishes same-path
canonical inputs; configuration A/B/A and another consumer label are separate
proofs. All four flag pairs, preserved-name Unicode/hash, complete registry state,
actual source-builder True/True and expired/suspended-token gates pass. Proof
growth exceeds the200-line estimate because those distinct owner/branch/lifecycle
gates are required; production remains bounded, no new copied assets or file growth.
Pinned fmt/diff/plan/archive and actual pre/post process checks pass. No authentic
replay or runtime callback advancement is claimed. Receipts target/wp876; elapsed
about18 minutes (03:59–04:17 UTC), no workflow speedup claim.

REPLAN for new Solib lifetime/action family, generic Spawn/REAPI/execution work,
Bazel checksum/path byte reproduction or weakened token/input/consumer identity.
Remain within the full captured-helper objective after this prerequisite.

## Predecessor

WP875 `8c427a879` independent DESIGN/FINAL accepted finite four-capture semantic
recipe and complete-helper requirement; source/doc checks pass, no runtime gate.
DICE-owned frozen helper phase bindings are feasible, but complete semantic load
manifest, producer preparation, collision-safe provenance and invocation lifetime
require reviewed implementation. Dynamic True/True source solib is the first
concrete prerequisite; self-contained pipeline prerequisites stay under research.
