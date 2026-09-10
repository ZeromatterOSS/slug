# Current Slug V2 Work Packet

Packet: WP-5-7A-selected-bcr-0640-0750-mode-implementation-r1

Status: source audit and independent activation review ACCEPTED. Implement only
the frozen two-file mode-admission contract and its bounded local gates.

## Observable result and evidence

Accept generic regular0640 and directory0750 entries through the existing
selected-BCR extractor, without changing source routing or manufacturing CLI
fixtures. Preserve all existing safety, cancellation and publication boundaries.
This is a necessary source prerequisite, not combined runtime acceptance.

Stage5's "Configured CLI source audit: regular 0640 and directory 0750" owns
the cached provenance and producer trace. Existing platforms1.0.0 descriptor:
SHA-256 f4ff1fd412e0246fd38c82328eb209130ead81d62dcd5a9e40910f867f733d96;
7,879-byte archive SHA-256
3384eb1c30762704fbe38e440204e114154086c8fc8a8c2e3e28441028c019a8
matches its SRI. Read-only complete listing has11 regular0640 files and three
directories0750; real host/constraints.bzl is present. Original failed HTTP
capture is not retained: do not claim a direct comparison to that capture.
The host source still forwards to generated @host_platform, and MODULE requires
rules_license. The spent rules_shell registry has no real shell/sh_binary.bzl.
Neither this change nor local_repository admission alone proves CLI closure.

Pinned Bazel9.2: /home/wgray/bazel git object
8220c6198837d5c13d53fea211cf3282aa12408a, never checkout HEAD.
src/main/java/com/google/devtools/build/lib/bazel/repository/decompressor/CompressedTarFunction.java:
110-112 directory creation;142-152 regular bytes, mode|0400 and mtime.
Source SHA-256:28dd9b8ace7d64b432b4bf566b1d1325cffea81df338ace428dfff7c756ae333.
Corresponding src/test/java/.../CompressedTarFunctionTest.java SHA-256:
3a2865acca41f7ebe484886a978aeef2eeb9aba2aa9d3337f0b81a6576c925c2.
Adapt without/with-prefix themes using existing inline tar-header helpers.
Those upstream tests are not a permission matrix; new pinned-source regression
covers the gap. Links/renaming are unsupported and not imported as proof.

## Decision, ownership and lifetime

Change only the two kind-specific mode predicates in repository_archive_realize.
Exact: regular0640 bytes/mtime and retained mode via existing mode|0400.
Slug-native: existing directory0755 normalization, bounded extraction, Unicode
paths, deterministic errors and Host observations. Do not claim exact Bazel
directory permissions/umask. Broader modes, special bits/types, links, local
source policy, cache ingestion, HTTP/file URL changes and source closure remain
unsupported/deferred. No platform/module-name special case or new fallback.

Keep existing Host canonical source projection/request_kind, immutable request
and SelectedBcrArchive owners. Verified archive digest already covers mode bytes;
source association includes archive/prefix/ordered transforms/MODULE. Extraction
is private scratch until AssociatedImmutable publication; existing session checks,
provisional-root lifetime and final validation/source-observation certificates
remain authoritative. Failure/cancellation drops captures/temp roots, never
publishes partial success. No new retained state, DICE key, cache, lock, async
task or request field; no equality or invalidation change. Existing
selected_bcr_active_cutoff_and_source_association_are_session_semantic and capture
cleanup tests cover the unchanged lifecycle. Buck2 is concept-only; no donor or
utility import/layout work, so no Stage9 representation row is required.

## Exact allowlist and caps

Production: app/slug_core_v2/src/runtime/repository_archive_realize.rs.
Proof: app/slug_core_v2/src/runtime/tests/repository_archive_realize_tests.rs.
Docs: current manifest, canonical Live Status and relevant Stage5/6 owner status.
Maintain /home/wgray/PROGRESS.md <=500 lines.
Gross caps:6 production/120 proof/126 total Rust additions; no new file/helper.
Extractor765 lines; extract>150 lines stays cohesive as one bounded stream and
namespace owner. Only its admission predicates change. Existing separate753-line
test module owns proof. No dependency, fixture/harness, source-input, vendored,
transport, Bzlmod, CLI/server or unrelated Rust changes.

## Proof and resource-bounded gates

After activation review, add two independent red tests under the
selected_bcr_admits_ prefix for regular0640 and directory0750. Reuse tiny inline
headers/payloads; provenance comments cite the pinned source and archive hash.
Do not read user caches in CI or copy authentic module bodies as test scaffolds.
Prove nested/root-prefix directory handling and readable child bytes; Unix
assertions retain regular0640 and normalize directory0755. Keep existing0444/
0644/0755 controls. Add discriminators rejecting regular0750, directory0640,
setuid/setgid/sticky-bit modes, and preserve existing regular0600/directory0700,
path traversal, collision, malformed-stream and size-limit rejections.

Serialized commands, each timeout60, bounded output:

- cargo test -q -p slug_core_v2 --lib selected_bcr_admits_ (red, then corrected green)
- cargo test -q -p slug_core_v2 --lib repository_archive_realize::tests (once at integration; ignored disposable archive test stays ignored)
- cargo check -q -p slug_cli_v2 (direct compile consumer only)
- cargo fmt --all; git diff --check; scripts/v2_archive_status.sh

Archive checker must retain only the three established thoughts-path failures.
Source/diff review confirms no mode admission outside those two kind/mode pairs.
No full-core suite, baseline-comparison rerun, ignored artifact test, CLI/Bazel/
oracle/daemon/replay, network acquisition or binary invocation. No cache warming
or new archive body is needed. Tests over one minute require investigation;
fifteen minutes is absolute maximum; no automatic longer retry.

## Stop and handoff

Independent terminal review required. Any new mode/type, source policy,
representation, cap overflow, publication change or second material correction
is REPLAN. Commit/push this complete prerequisite after acceptance. Then resume
the source-closure decision for real CLI proof before running any spent setup;
do not restore the combined candidate into this two-file milestone.

Complete unaccepted R2 remains at /tmp/slug-conflict-r2.XZJWwv/candidate.patch,
base97dffd5d4, SHA-256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e.
Adjacent validation.txt owns gates;757 production/2103 proof/2860 total additions.
Hash/forward applicability verified; Rust restored to baseline. Two fast core
failures reproduce identically on baseline; other full-core failures remain
unattributed. Actual one-shot/stable-daemon cquery/aquery/build/run and positive
common REAPI sharing remain unaccepted. Never partially ship or weaken R2.
