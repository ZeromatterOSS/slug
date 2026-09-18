# Current Slug V2 Work Packet

Packet: WP-7-30-m7a-source-input-digests-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and demand

Bind canonical source artifact identity to its existing repository-resolved path,
fixed SHA-256/size fact and complete path-observation frontier in a Core DICE
producer. This joins WP-7-28 source routing with WP-7-27 streamed observations for
the ordinary source/compiler/tool input transfer row in bootstrap-readiness and
the production //app/slug_cli_v2:slug closure. WP-7-29 supplies the subsequent
bounded upload transport. The fact alone never authorizes upload, Execute or
successful build publication; validated action closure, transfer verification
and final request-certificate validation remain execution gates.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
ConfiguredTargetFactory.java InputFile and RepositoryName.java execution paths
(WP-7-28 evidence), remote/merkletree/MerkleTreeComputer.java file metadata,
FileArtifactValue.java regular file digest/size and symlink-following input
metadata. Exact SHA-256/content and repository-aware execution paths use existing
owners; canonical repository names/configuration identity stay Slug-native.
Generated outputs/tree inputs, built-in catalog source adapter, symlink-preserving
REAPI inputs and Spawn activation remain deferred.

## Owners and invariants

Analysis dice/source_file.rs remains the single canonical-label-to-source-path
routing function, shared by configured source admission and the new Core consumer.
Retain its observed route/path frontier instead of discarding it: expose a hidden
observed resolver function and read-only observed-path result. Do not add a second
path key, parse rendered execution paths, read the host in DICE or alter legacy
routing. Configured analysis projects the same semantic Result<ResolvedPath,
AnalysisError>; source content does not become a configured-analysis dependency.
Observed route diagnostics retain their existing invalid AnalysisError mapping;
path/union frontier errors remain outer. Success and semantic errors carry all
available prefix observations. Need propagates unchanged and has no cached result.

Core SourceArtifactInputObservationKey accepts an AnalysisArtifact in its public
constructor and rejects Derived file/tree artifacts; there is no public label-only
constructor. Internally it is keyed by workspace plus canonical Source label (not
a rendered path). This fact is not declaration/visibility or closure authority;
execution consumers must obtain the retained source from a validated closure. It calls the shared observed resolver, then
PathFileDigestObservationKey using its namespace and requested path, never the
main Host namespace guessed for an immutable repository. Union route/path/digest
frontiers with PathObservationEpoch::from_shared, retaining existing Arc values
and rejecting conflicts. Result includes canonical label, namespace, requested/physical path and the
40-byte FileContentDigest; ResolvedPath is scratch here and remains owned by the
existing path producer. Route/metadata provenance is retained in the complete
epoch. Read-only accessors, no byte buffer or execution-path copy. Observed result and complete epoch are immutable Arc-owned DICE semantic
state with Allocative and structural equality. Need, infrastructure/frontier and
semantic errors are invalid/not equal for caching; successful full facts are
compared structurally. No additional semantic digest key/cache/interner/lock.
Use existing cheap Arc/Dupe carriers; new retained state is fixed metadata plus
existing shared path/frontier carriers, released with the DICE version. No donor
import, detached work, async resource or performance claim.

Missing files cannot yield an input fact; directories/special files and resolution/
digest errors fail closed. A content edit changes digest even at fixed label/size;
restoration restores digest/path semantic value, not necessarily the observed
epoch or key value (which includes mtime/node metadata). Tests distinguish both.
Repository root/generation changes stay tracked
dependencies even when label/execution path/content agree. No parallel repository
registry or stale path fallback. Public key only describes source artifacts: its constructor rejects derived/tree
artifacts before any source lookup. It does not re-run target analysis.
Consumers must retain source identity, validate provenance at final acceptance,
and verify transfer bytes; digest-only equality cannot bless a new source route.

## Scope and proof

Allowlist: analysis dice/source_file.rs, dice.rs dispatch/reexports and lib.rs;
Core new runtime/source_input.rs and source_input/tests.rs plus module/reexports in runtime/mod.rs; new
Core runtime/tests/source_input_tests.rs with a bounded include in runtime/dice.rs
for access to existing request-driver test helpers; canonical/manifest and Stage
6/7/9 architecture summaries. No dependency, action/closure activation, repository
materializer, request driver or uploader semantic changes.

Core focused tests use the real native demand/revision driver via a test-only
root adapter over the production source key, storing an empty root event batch.
Use local MODULE/local_path_override source routing, including main versus external
same basename, main-shadow missing external, content same-size A/B/A, delete/
recreate and wrong-kind rejection. Negative source outcomes remain invalid; the
test adapter rejects them before terminal publication, verifies accepted semantic
state is unchanged and proves same-runtime recovery. It does not claim accepted
negative terminals or production execution diagnostics. Check selected epoch contains real FileDigest
and route/resolution observations with retained Arc values; no FileBytes demand
for source content. Same-runtime warm/revision transitions, symlink retarget and
request final validation of the returned complete frontier. Existing Bzlmod
immutable materialization/path namespace and Workspace digest frontier tests are
reused, plus a focused immutable source-key discriminator if their combination
does not exercise the new namespace choice. Synthetic observation tests prove
Need/invalidity, metadata-only observed inequality with stable content projection,
digest-error handling and Derived file/tree constructor rejection without real I/O. Preserve WP-7-28
external configured root and main dirname/authentic Rustc tests after resolver
refactor. No broad suite, daemon, Bazel/compiler action or live transport.

Independent design and final review for public shared resolver and DICE ownership.
Pinned nightly compile-only JSON preparation <=60s per operation; exact test
preflight. Focused tests expected <few seconds; tests >few seconds infrequent and
>~30s require concrete strict necessity. Direct analysis/Core and REAPI compile
coverage, format/archive/plan/diff. Do not waive namespace/frontier/request-owner
proof to fit a test budget; split preparation from small runtime gates.

Predecessor WP-7-29 accepted/pushed at 4948d95f8: bounded verified reader uploads,
seven cache gates and .34s real NativeLink wire proof, final ACCEPT. M7A remains
partial and M8 unproved.


## Acceptance receipt

Independent corrected design and final review ACCEPT. Gate advanced: retained
source artifact identity now joins repository-aware path and streamed content
metadata in Core DICE with the complete request-validation frontier. This is a
source fact producer, not action admission or execution activation.

Baseline 4948d95f8; review/wp730-source-input-digests. Direct pinned
nightly-2025-09-14 Cargo/rustc/rustdoc; JSON compile-only preparation capped at
60 seconds per operation. Raw local receipts: target/wp730 (not committed).

- Core `cargo test -p slug_core_v2 --lib --no-run`: two test-code compiler
  corrections (moved Option and missing Dupe derive), exits 101 in 44.889s and
  7.046s; successful preparations 21.400s, 6.525s, 4.202s and 7.366s.
  The later preparations changed only test scaffolding. No cap was reached.
- Four focused Core selectors under runtime::source_input::tests:
  source_fact_tracks_metadata_separately_and_preserves_selected_arcs,
  source_fact_needs_and_failures_are_never_cacheable,
  source_digest_join_keeps_immutable_namespace,
  source_input_constructor_rejects_derived_files_and_trees.
  All passed across exact-preflighted batches; preserved unaffected results.
  Synthetic injection now supplies observation shards and compares retained Arcs
  with DICE's selected representatives, allowing legitimate equality cutoff.
- Three native selectors under runtime::dice::tests::source_input_tests:
  native_source_inputs_route_digest_restore_and_reject_missing_main_shadow,
  native_source_digest_is_revalidated_before_acceptance,
  native_source_symlink_retargets_even_when_content_matches.
  Final exact preflight selected 3; execution exit 0, 3/3 passed in 0.565s.
  Local MODULE fixtures close the unchanged built-in dependency declarations;
  registry input is local-only. Temporary roots are canonicalized. Missing and
  wrong-kind inputs return captured semantic errors and abort the test request
  before terminal publication; accepted inputs, repository results, environment
  frontier, path observations and selected demands remain equal. Recreating the
  source and relocating/restoring its repository succeed in the same runtime.
- Earlier Core batches: 4/7 passed in 0.124s, 5/6 in 0.133s, 2/3 in 0.365s.
  Corrections addressed synthetic injection/Arc expectations, temporary path
  normalization, incomplete offline MODULE fixtures and the test adapter trying
  to accept an invalid source outcome. Production key validity and native driver
  semantics were preserved; no failed gate was waived.
- Analysis `cargo test -p slug_analysis_v2 --test starlark_rule --no-run`
  preparation exit 0 in 21.428s. Exact preflight 2, execution 2/2 in 0.632s:
  rustc_map_each::external_sources::external_source_roots_route_render_and_restore
  and rustc_map_each::pinned_rustc_file_dirnames_preserve_order_and_generated_root.
  Resolver production inputs have not changed since these passes.
- Direct dependent `cargo check -p slug_reapi_v2` exit 0 in 12.377s, including
  analysis/Core. Changed Rust formatting, diff whitespace, plan status and V1
  archive checks pass. No Cargo/BUILD dependency or copied upstream fixture grew.

Nine unique focused gates proved. Compilation operations totaled 125.233s;
all test batches together took 1.819s. Continuous packet/review wall time was
not recorded. Immutable namespace coverage exercises the production digest join
with synthetic materialization observations and reuses the existing Bzlmod path
owner proof; it is not a new native archive end-to-end test. Symlink coverage ran
on Unix. Built-in catalog input adaptation, closure admission/staging, verified
source opening/upload, generated/tree transfer, toolchains, typed Spawn execution
and broader bootstrap remain open. M7A is partial and M8 remains unproved.
