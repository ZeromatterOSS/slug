# Current Slug V2 Work Packet

Packet: WP-7-171-m7a-filegroup-data-implicit-source-r1
Status: accepted; independent DESIGN/FINAL accepted; M7A remains partial

## Outcome and compatibility

Baseline `50a71f761` (WP870). Admit implicit source labels referenced only by
native filegroup data through the existing package declaration predicate and
tracked source-resolution owner. Advance the unchanged authentic Rust-toolchain
cquery beyond the missing host-tools
`lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld` target if that physical
source is present; its archive membership is currently unverified. Do not claim
that it exists or change archive/symlink policy from a target-not-found error.
Exact bounded same-package data-label eligibility and source File/runfiles
semantics; artifact paths and configuration identity remain Slug-native. Existing
macro, visibility, unsupported filesystem-kind, coverage/output-group and other
native-attribute boundaries remain deferred or explicitly refused. Windows stays
minimal; full toolchain/query/bootstrap and M8 remain unproved.

Pinned Bazel 9.2 clean `8220c619` Package.java:873–965 creates assumed InputFile
for same-package referenced label-typed attributes, excluding existing target/
macro and symbolic-macro-only declarations; it is not limited to srcs.
TargetRecorder.java:230–231/429 collects Rule.getLabels; PackageFactoryTest.java:
301–338 testCreationOfInputFiles and VisibilityTest.java:785–806 protect implicit
file and export policy. FilegroupRule.java:60–62 admits data LABEL_LIST Files.
Reuse WP870's exact data runfiles split, source/actual-File identity and declared
selected edges. Slug dice.rs:2740's package_declares_source_label recognizes only
Filegroup.srcs. Driver:5355–5413 rejects data-only implicit Null labels BEFORE
physical observation. Source-admitted preparation:793–877 and source_file.rs:
203–205/246–306 already own nested label paths and canonical repository tracked
resolution. Generated archive extractor:161–164 explicitly rejects symlink/
hardlink/GNU-longlink rather than dropping them; current receipts do not justify
archive changes. The real host-tools template/archive is not retained after
one-shot temporary-root cleanup, so the generic declaration gap and replay must
be distinguished from an asserted physical file.

## Owner, invariants and scope

Extend only the existing Filegroup arm of package_declares_source_label: preserve
its srcs candidates. For new data candidates, exclude targets with symbolic macro
origin and labels equal to existing symbolic macro instance names using retained
macro_origin/macro_instances metadata. Consult genuine retained native data values through
package.native_attributes, using the existing CoercedAttributeValue.labels walk.
Selector values and concatenations supply declaration candidates; condition keys
must not become source candidates. Candidate presence is eligibility only;
filegroup's existing configured join still observes only selected labels.
Keep package inventory, immutable attributes/source certificates, macro/source
admission, Null identity and SourceFileState/Need/error semantics with their
existing owners. Every actual existence check stays in DICE-owned source_file;
no filesystem scan, fabricated artifact, fallback, export or hardcoded ld.lld.

No retained representation/key/lock/cache/action/heap/async owner is added.
Candidate vectors are phase scratch using existing cheap label handles; retained
native attributes, source observations and configured provider publication retain
their existing equality, validity and release boundaries. DICE ownership doc and
Stage 9 native-filegroup utility disposition apply; no extraction/donor or
performance claim. Independent DESIGN/FINAL for the shared source-admission
boundary. The very large dice.rs module remains its existing native dispatch/
package declaration predicate owner; this small branch does not justify adjacent
refactoring or general implicit-source activation.

Allowlist: Analysis src/dice.rs only the declaration predicate; existing
Analysis tests/native_filegroup/{mod.rs,BUILD.fixture,data.rs,fixture.toml};
manifest/canonical and Stage 6 current native-filegroup paragraph. No Loading,
source_file, archive, configuration, CLI or dependency changes. Estimate
production <20/proof <120 net Rust lines. Reuse authored fixture/Workspace and
both existing analysis routes; nested data Files must NOT be exports_files or
srcs references, otherwise the proof bypasses this gap. Existing implicit srcs,
explicit exports, data policies/guards and selector metadata remain protected.

## Proof, validation and decision boundaries

Add unexported nested data-only authored regular sources. Both routes/root and
external/target and exec must preserve srcs-only output files/executable,
unconditional actual File data contribution even under include-files=False,
Null source edges and zero own actions. Selected data excludes an unselected
missing candidate; selecting it refuses. Same-DICE declaration/source A/B/A,
missing/delete/recreate and directory-to-file restoration must use existing
observed epoch helpers and recover the exact original provider result. Physical-file negatives protect symbolic-macro-only data references and macro-name
collisions; an ordinary nonmacro data reference is eligible. Unknown
unmentioned labels and selector condition keys remain ineligible. Preserve
existing implicit srcs and protected native data/visibility/testonly/coverage
proofs. No new fixture assets beyond tiny authored files in the existing setup;
add precise Package/Test source anchors to current provenance.

Root serial pinned Cargo JSON Analysis starlark_rule no-run 60s, exact ordinary
nonignored preflight and focused 20s runtime, Core check and CLI rebuild before
unchanged three-hash authentic supervised 180s replay. Its observed 77s setup
proves the real bootstrap boundary; smaller authored source proofs cannot verify
archive membership or actual query progress. Actual process cleanup and scoped
format/diff/archive/status gates. These are operation caps, not user limits.
REPLAN if eligibility needs a new source owner, visibility/macro bypass, physical
path policy, archive/link admission or missing file fabrication. A later genuine
physical absence or unsupported kind is a truthful successor boundary, not query
success. M7A/bootstrap closure remains partial.

## Predecessor

WP870 `50a71f761` accepts configured data and required typed Loading declarations.
Independent DESIGN/Loading delta/FINAL, 29 focused tests in 1.006s, Core/CLI pass.
Unchanged query 77.271s exit 2 clears data refusal and reaches the missing host-
tools ld.lld target; three hashes and actual cleanup pass. Receipts: target/wp870.

## Validation receipt

Independent DESIGN accepted both metadata-based symbolic-macro exclusions.
Production is 20 net Rust lines; proof/setup slightly exceeds the 120-line estimate
because both macro guards and declaration/delete/directory restoration are distinct
requirements. Actual proof/setup is 175 net Rust lines, still one existing fixture
and one focused selector; no copied assets or authentic workspace mutations.
Initial Core compile corrected the existing native_attributes.get
schema/value tuple destructuring. Core final passes in 4.33s; CLI rebuilt in 11.85s.
Initial 15-test group passed 14 protected cases and found an incorrect proof
expectation for unknown external observed direct-root labels: the existing
route-shape guard
refuses before source eligibility. Direct-root unknown/condition-key negatives
protect their existing route refusal, not a dynamically exercised predicate path;
the unchanged CoercedAttributeValue.labels selector-value-only walk provides the
condition-key exclusion. Macro data child negatives exercise the new predicate.
Failure receipts are preserved. A second proof failure revealed that the generic
fixture epoch omitted empty-directory Lstat observations and absent source paths.
The existing root_epoch_with_missing helper now records genuine absent known
fixture paths; a tiny child file makes the directory visible to its existing
snapshot collector. This is proof preparation, not a production observation bypass;
selected/deleted sources must now fail target-not-found, and directories must report
unsupported Directory kind. The accepted source owner/design remains unchanged.

Unchanged authentic replay: 83.533s, exit 2, original three hashes unchanged, actual
pre/post process checks empty. It clears the host-tools source target and reaches
rules_rust private/utils.bzl:516 actions.declare_file(..., sibling=rlib_file), which
rejects the unsupported named argument. Physical host-tools admission was sufficient
for this replay; archive/symlink policy was not changed. Query/bootstrap remain
partial. Receipts: target/wp871.

Final Analysis compile 7.03s; exact preflight selects 15 ordinary nonignored cases,
all pass in 1.165s. Fourteen protected cases and the new case cover both routes,
root/external target/exec, selected missing-path refusal, macro-only and macro-name
exclusions, declaration A/B/A, delete/recreate and Directory-to-file restoration.
Recorded proof corrections include exact observed-versus-prepared direct-root
refusal expectations; failed groups are not accepted gates. Scoped rustfmt,
diff, archive and plan status pass. No Windows implementation was added.

Independent FINAL accepted source ownership, both macro guards and proof growth.
Packet window approximately 13:13–13:45 UTC (32 minutes, including proof preparation
corrections and independent review); final compile/runtime measurements above are
operation receipts, not a workflow-speed claim. Observable gate: authentic Rust
host-tools data sources now resolve; next frontier is File sibling declaration.
