# Current Slug V2 Work Packet

Packet: WP-5-178-m7a-selected-bcr-relative-file-symlink-r1
Status: accepted; independent DESIGN/FINAL accepted; return to WP877; M7A remains partial

## Outcome and evidence

Base `64f52f1c4`. Remove the selected-BCR extraction refusal for the demanded
rules_jvm_external6.7 tar's relative leaf-file symlink, preserving the genuine
archive rather than deleting an entry or mutating the authentic workspace.
WP877 callback candidate is preserved at `d318df353` on local
`review/wp877-rust-toolchain-callback`; its independent closing proofs continue
there. This prerequisite never admits that callback by itself.

Hash-verified actual archive (4,738,304 bytes, SHA256
`a1e351607f04fed296ba33c4977d3fe2a615ed50df7896676b67aac993c53c18`)
contains559 logical entries and one symlink:
`tests/integration/bzlmod_lock_files/.bazelversion` -> `../../../.bazelversion`
after strip_prefix `rules_jvm_external-6.7`, mode0777,size0. WP877 authentic
entry-result diagnostic82.986s exit2 records MaterializationError("selected BCR
tar contains an unsupported entry type"); original3workspace hashes unchanged.
Temporary diagnostic is removed. Source receipt in `../slug/target/wp877`.

Pinned Bazel9.2 `8220c619` CompressedTarFunction.java stores symlinks until
regular extraction completes, preserves relative target text and creates a
symlink rather than copying target bytes; ordinary-file chmod/mtime does not
apply to links. CompressedTarFunctionTest.testDecompressWithoutPrefix and
.testDecompressWithPrefix are the source regression anchors. Interpret this
finite admitted relative leaf-file behavior as exact; existing bounded Unicode,
mode/path/type/limit and namespace rejection remain Slug-native admission guards.
Absolute, escaping, dangling, chained, directory and hard links remain deferred.
Windows gets only the ordinary existing unsupported stub.

## Owner and invariants

Existing Core `repository_archive_realize::extract` owns bounded selected-BCR
physical realization. Existing captured archive SHA and complete selected source
association already own all entry/path/link bytes; no new DICE key, request field,
semantic cache, source fallback or filesystem observation is introduced.
Existing generation/error/session/provisional-root cleanup and final promotion
remain authoritative. Matching Stage9 reuse basis is existing Core bounded
SHA-256 archive capture, retained Arc/Dupe repository epochs and transfer scratch.
No donor production code is imported.

Parse symlink path through the existing normalized entry/prefix policy. Require
zero payload, ordinary0777 header mode, bounded valid Unicode relative target
text. Lexically resolve target from the stripped link parent; permit leading .
and .. only while resolution remains inside the extracted root, then require a
normalized ordinary tail. Interior cancellation (missing/../target or
regular_file/../target), even if its endpoint is ordinary, remains deferred.
Preserve target text; ordinary target membership ensures tail-parent directories
were created by regular extraction without host following.
Extend namespace collision checks so a symlink is never an ancestor and never
has already admitted descendants, regardless of archive order. Retain pending
links as bounded extraction-phase scratch. After complete regular extraction,
require each resolved target be an admitted ordinary extracted file and install
Unix symlink, checking session activity before each. No host path scan/following,
no link installation during ordinary-file writes, no linked-directory writes.
Reject pending links before successful return on unsupported platforms.

## Scope, validation and stops

Allowlist: Core repository_archive_realize.rs and its existing tests module;
Stage5 contract and manifest/canonical. Growth estimate under150 production/
200 proof lines; split focused lexical/install helper if needed rather than
expanding the existing >150-line parser's responsibilities. Source association
stays unchanged because existing archive/transform digests cover link bytes.
No CLI/error-renderer/other archive-family/patch engine changes or JVM execution.
Independent DESIGN before implementation and FINAL against actual delta.

Tests: target-text preservation/readlink+content, both archive orders, strip
prefix containment, exact demanded nested link, duplicate and ancestor/descendant
collisions, self/dangling/chained/directory/absolute/escape/invalid target refusals,
mode/payload bounds, cancellation and provisional-root disposal. Protect existing
streaming/metadata/type/namespace/integrity/overlay/patch/module behavior. Use
pinned no-run Cargo JSON prep60s, exact-selector20s grouped runtime, direct CLI
compile. One explicitly selected ignored local archive probe (env path/hash checked
by root) may exercise full downloaded559-entry archive without network or full
metadata capture. Runtime10s; never count a skipped or zero-selected probe.
Serialize all freshness/native queries and shared-target Cargo. Actual pre/post
Slug process checks for rebuilt authentic replay after integration with preserved
WP877 candidate. That replay >30s is justified as the only existing genuine
full producer boundary; do not repeat it until code resolves this exact stop.
Fmt/diff/plan/archive checks. No performance claim.

REPLAN for a link outside the bounded ordinary-file slice, physical parent
aliasing, required source-association change, namespace safety contradiction or
an unrelated demanded archive format. Root resolves authorized prerequisites;
no user approval is required. Preserve incomplete substantial changes locally.
After acceptance on main, return to WP877 and reconcile its candidate base/gates.

## Acceptance receipt

Independent FINAL ACCEPT against unchanged source and actual receipts. Pinned
nightly2025-09-14 Core cached-dependency compilation50.412s exit0;16/16 ordinary
exact tests0.114s; explicit ignored full-archive probe1/1 in0.022s with exact
SHA verification. CLI cached-dependency build22.918s exit0. Earlier Core/CLI
preparation60s timeouts are preserved separately, with process cleanup and no
raised operation cap; they do not count as semantic failures. Scoped fmt/diff/
plan/archive checks pass. No speedup claim; packet wall time not recorded.

WP877 candidate and its new proof sources remain preserved separately. Reconcile
this accepted prerequisite into that review branch before rebuilding its CLI and
replaying the unchanged authentic workspace. WP878 adds no oracle fixture and
does not reset or waive the inherited WP834 oracle-growth accounting.
