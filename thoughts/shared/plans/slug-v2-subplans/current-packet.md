# WP1111 exact Python archive preparation

Packet: WP-7-323-m7a-python-archive-preparation
Status: accepted — FINAL source/evidence ACCEPT; next Python environment capability
Baseline: 30e1e0089 (accepted WP1110)

Outcome: verify and prepare the exact rules_python1.7.0 archive and version patch
that blocked the original requested-root probe, then admit only that complete
original immutable request through the existing scoped offline materializer.
Slug-native private probe policy; production archive transformation semantics and
all accepted exact surfaces remain unchanged. Linux only; Windows/JVM deferred.

Basis: wp1110-qualified-native-readiness-window-2026-10-11.json and
 target/wp1110/next-owner-research.md (87e00b746621d4a5281608fdf5aa1f33fddb532b3e7be584581f5c46f4e86e04).
Actual291sources/14.960s/phase229.672MiB under256; no deadline/quota.
Selected Spec producer selected_repo_spec.rs:906-944 owns original9attributes.
Normal repository_archive.rs:85-225 and repository_archive_realize.rs:68-98
already capture verified ordered patches, apply strip1, then replace MODULE;
selected_bcr_source_association owns full transform identity. Never skip the patch.
Pinned Bazel9.2 IndexRegistry/RepoSpecFunction anchors in existing fixture suffice
for unchanged ordering; existing patch/file-session controls are discriminators.

Prerequisite authority explicitly extends WP1110's source-only/body-read policy:
root may acquire only the exact public release archive and exact BCR versionpatch
named by the captured request, verify full SHA256 and file identity, and inspect
bounded tar/patch/MODULE bytes for feasibility and fixture provenance. Archive URL
https://github.com/bazel-contrib/rules_python/releases/download/1.7.0/rules_python-1.7.0.tar.gz
SHA256 f609f341d6e9090b981b3f45324d05a819fd7a5a56434f849c761971ce2c47da;
patch URL https://bcr.bazel.build/modules/rules_python/1.7.0/patches/module_dot_bazel_version.patch
SHA256 a9ffb62ba1d8e9ebad5225bd2cde6e8ff20688060fff8d14133ca9bb237fb48b.
Existing manifest-listed fixture bodies may be read by the normal verifier after
preparation; no unrelated corpus/envvalue read. Fetch capped8MiB archive/64KiB patch, streaming
64KiB scratch; no extracted filesystem payload except original prepared patch.
Inspect gzip stream with total decoded cap11,478,463 (remaining64MiB), bounded
inventory; conservative write bound14,255,765 including patch and preparedMODULE.
Inspect only archived MODULE and LICENSE bodies (each<=64KiB) for patch proof and
license attribution. Existing preparedMODULE13673B/d01f995ecd137abf30238ad9ce97f8fc3ac57289c8b24bd0bf53324d937a14f8
and selected source.json369B/028a084b65dcf8f4dc4f82f8778dbe65df133f234b316828a82e060d81bdce32
may be bracket-hashed/read to verify exact Spec/source provenance. No runtime Spec
rewrites, public URL replacement or wildcard admission. Feasibility failure stops
activation under unchanged caps; preserve exact evidence and select bounded replan.

After DESIGN and feasible authority: fixed Linux-test third constructor-owned
optional Arc backing (production/nonLinux unchanged), borrowed URL+digest lookup,
all pairwise duplicate-URL rejection before publication. Bodies owns optional
Python backing/MODULE/patch paths; exact borrowed matcher requires canonical,
workspace, unmapped http_archive label,9attrs, singleton URL/SRI/strip_prefix,
exact String-key/String-value singletonpatch/SRI, Int1, exact preparedMODULE URL/SRI,
empty overlay maps and binding. Optional absent preserves prior policy; malformed
present bindings never qualify. Scope owns handles until lastDrop; no postpublication
setter, new key/lock/cache/interner, payload copy or policy String allocation.
Existing normal execution plan retains its owned transformation strings unchanged.

Rust allowlist: platforms_module_preparation_tests.rs and child
immutable_archive_preparation.rs; source_service/offline_archive.rs and scope.rs;
tests/repository_archive_file_session_tests.rs (all under app/slug_core_v2/src/runtime/).
Fixture allowlist: configured-cli-authentic/fixture.toml, exact Python mirrorarchive,
registry/modules/rules_python/1.7.0/patches/module_dot_bazel_version.patch and
licenses/rules_python-LICENSE. Reuse registry_bundle MODULE, no duplicate metadata.
Target-only prepared patch at original target/wp954 URI, helpers/evidence/scheduling.
Fixture test owner tests/v2_oracle/test_configured_cli_fixture.py updates only
exact inventory pins and prepared Python path checks; no fixture tool/parser/realizer changes. Estimated<120wiring/policylines and cohesive
existing controls; no copied lifecycle harness. Cohesive large testowners remain
existing family owners. Fixture growth3objects; review provenance/duplicates before
publication. Existing fixture verifier/patch check proves patched originalMODULE
matches selected preparedMODULE. No donor imports; existing Arc/path sharing and
interners retained (Stage9 existing source owner disposition).

Controls: third optional slot preserves two-slot successes; all duplicate pairs,
borrowed original paths/Arc counts/fullhash, missing/wrong source/SRI/patch/strip,
extra/wrongtypedpatch and wrongMODULE/role negatives; original firstoffender/zero
progress and namespace/current epoch remain. Reuse normal file-session patchthenMODULE,
A/B/A integrity, quota/cancel/warmreuse/stale generation and Weak retirement tests.
Fresh current ordinary63+new controls, direct productioncompile, CargoJSON no-run/
list/batchedpreflight and actual bodies. Freeze prior37Rust+metadata plus changed
file-session owner and fixture objects; caller8 IDs/paths unchanged, changed Scope
pin explicit. Source350 predecessor order unchanged plus exactarchive/patch bindings;
18env unchanged; original heldFDowner remains sole native sourcehashreader.

Inherit WP1110 RSS256MiB/AS512MiB/qualifiedsource512/census513/decodedwritten64MiB,
all transfer/live/object/registry/directory/log limits and time20/23/26 unchanged;
compilerRSS separate, prepnull/ordinary10/grace3. Fresh pinned toolchain/offline/
locked/jobs1, no concurrentCargo; allwriters stopped before ONE distinct freshshell
native after source/helper/config PRELAUNCH. FINAL actualdiff/proofs/typedterminal/
limits; plan/archive/diff checks and acceptedcommit. No identical native replay,
smallerroot, automaticcapgrowth or fullproducer/M7A completion claim. Recurring
>180s is necessity/refactor signal; reuse frozen scaffolding and review only changed
invariants. DESIGN checkpoint precedes acquisition/source; FINAL independent.

Predecessor: accepted WP1110 30e1e0089;63proofs, productioncompile, bounded original
native prefix. Full compiler producer/certificate/analysis/actions/Rustc/M7A open.

Accepted receipt: wp1111-python-archive-preparation-2026-10-11.json. Current74
Rustproofs/productioncompile and7relevantfixturetests pass;3unchangedportable-driver
rows environment-unproved (missingstrace). ONE15.674s/source309/phase232.289MiB
under256 crossed the Pythonarchive; nextoriginalNeed RULES_PYTHON_BZLMOD_DEBUG.
Decoded61853085/written57985242 under64MiB; noquota/deadline;154strictstablepins/
352orderednoninheritedheldFDbrackets. Corrected prereq writebound5132143 includes
omitted finalMODULE13673B and remains feasible; frozen originalartifacts preserved,
no replay/cap/source change. IndependentFINAL accepted this correction. Source-only
next-owner-research.md ready; no value/producer authority inferred. M7A remains open.
