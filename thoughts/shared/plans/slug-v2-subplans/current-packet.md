# Current Slug V2 Work Packet

Packet: WP-7-27-m7a-streamed-input-digests-r1
Status: accepted; independent final ACCEPT; ready to integrate

## Outcome and compatibility

Produce observation-backed SHA-256/size facts for ordinary compiler/toolchain
files without retaining their bytes in DICE. This is the source-input prerequisite
for the production CLI Rustc/process-wrapper closure recorded in
bootstrap-readiness.md, ordinary source/generated/tree input transfer row, and Stage 7/11 bounded transfer
contracts. It does not activate Spawn execution or upload unverified host files.
Exact SHA-256 of observed bytes; Slug-native observation/request identity and
error behavior. Generated artifacts, tree expansion, uploads and execution remain
required future work. No claim of a stable historical filesystem snapshot.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
vfs/DigestUtils.java getDigestWithManualFallback/manuallyComputeDigest owns file
content digests; remote/merkletree/MerkleTreeComputer.java addFile (1032-1050)
uses content digest and always executable input nodes. This packet creates no
REAPI nodes and does not project host executable bits. Source anchors describe
content identity, not a port of Bazel's process-global digest cache.

## Ownership and contract

Add FileDigest to the existing PathObservationOperation/Result family with a
fixed-size SHA-256 plus actual byte count. Core's OS observer reads through a
fixed-size buffer and returns the digest; no host I/O occurs inside DICE.
Unix and Windows use std File/Read with their existing error-classification
policies. Reads retry Interrupted at the individual operation, count actual
bytes with checked accumulation and reject sizes above i64::MAX (the REAPI
signed-size domain). The fixed-size fact constructor enforces that bound.
Reject non-regular opened files before reading; Unix uses nonblocking open
to avoid a FIFO replacement hanging the observer. No file-sized allocation. Observation
completion closes the handle; synchronous scratch is released on return.

Workspace owns PathFileDigestObservationKey over namespace/logical path. It
uses ResolvedPathObservationKey, demands the resolved physical file digest and
retains the complete resolution-plus-digest observation frontier. Missing,
wrong-kind, symlink cycle, read failure, and resolution/read disappearance retain
existing path error semantics; reuse PathFileBytesError as the shared file-read
error payload rather than copying its error classification. A typed digest
error adds size disagreement between resolution metadata and bytes actually read.
PathFileDigestKey projects the content result from this observed producer; equal
SHA-256/size cuts off semantic dependents even when resolution metadata changes,
while the observed producer preserves certificate changes. Need stays invalid
and never equals itself. Frontier construction errors remain explicit.

Use existing immutable observation epochs, DICE keys and Allocative/Dupe facts.
DICE api Key equality/validity and existing path_resolution tests are the DICE
reference. No new cache, global registry, lock, retained file bytes or evaluator
borrow. Fixed digest facts are DICE-retained semantic memory; epoch slices share
existing Arc facts. Request owners must validate the returned frontier before
publication; later transfers must reverify bytes against the digest. This packet
cannot itself authorize a mutable path for upload. Existing request validation
must recognize digest observations and reject changed/error observations. Digest
observations count as content evidence for the existing metadata revalidation
policy; kind/size checks remain, same-content node-id/mtime changes are harmless.

## Scope and proof

Allowlist: workspace src/{path_observation,path_resolution,lib}.rs plus new
path_file_digest.rs and focused tests; Core runtime/path_observation.rs plus
new path_observation/file_digest.rs and focused tests, repository_io.rs validation
exhaustiveness; bzlmod host_file.rs, repository_ignore.rs,
source_preparation.rs and source_preparation/repository_source_observation/
registration_diagnostic.rs only new exhaustive observation branches; other
existing exhaustive matches only if required by compilation. Canonical, manifest,
Stage 7 and Stage 9 owner summaries. Large existing files get dispatch only;
new cohesive modules own digest algorithm/projection/tests. No upstream fixture
copy or new oracle fixture; pinned-source regression plus standard SHA-256 vectors.

Discriminators: empty/abc/multi-buffer digest+size, bounded reads, interrupted
and failing reads, real native observer regular/missing/directory/symlink paths;
same-DICE A/B/A digest changes, symlink retarget with equal content but different
frontier, missing/delete/recreate, wrong kind/read error/size mismatch, Need
validity and semantic equality cutoff. Protect an existing file-bytes resolver
and observer regression. Unix host execution plus compile Windows observer if
the installed target is available; otherwise platform-independent scripted error
classification and document unavailable native Windows compilation.

Compile separately using pinned nightly, each preparation bounded to 60s.
Preflight exact subsecond selectors. Checks over a few seconds run infrequently;
>roughly 30s tests need strict necessity, no such tests planned. Direct bzlmod,
loading, analysis and Core compile coverage catches enum consumers; no full
suite, daemon, Bazel build, compiler action or live transport. Format/diff/archive/
plan checks. Independent design and final review required for new DICE boundary.
Resolve observer/error/certificate contradictions before activation. Overall
M7A/bootstrap remains open after this prerequisite.

Predecessor WP-7-26 accepted and pushed at 8af36b3d8: public rules_cc static/PIC
providers, shallow immutable collections and full CcInfo/HeaderInfo retention;
focused tests and independent final ACCEPT. Receipt is in that commit's manifest.

## Acceptance receipt

Core test compilation covers production bzlmod/loading/analysis/query consumers
of the extended enums. Initial workspace preparation caught test-only misuse
of PathOutcome (it intentionally has no PartialEq); assertions now use complete_eq.
Initial Core preparation caught a test module path and a scripted enum match;
only those tests were corrected and the digest final-error branch was added to
the existing dispatch proof. Largest preparation 54.41s, below the skill's 60s
cap; final workspace preparation 3.40s and Core preparation 22.75s. The installed
pinned nightly binaries were used because rustup's snap launcher cannot run here.

Exact preflight and execution: workspace 4/4 in 0.064s including semantic
cutoff, full frontier, A/B/A, delete/recreate, errors and the existing byte read
projection. Core 8/9 passed in 0.008s; the existing native byte regression failed
at UnixListener::bind with EPERM in the sandbox. The sole affected selector
passed 1/1 in 0.01s after approved sandbox escalation. All nine Core gates are
now proved, including standard SHA-256 vectors, bounded/short/interrupted reads,
partial-read failure, native file/link/missing/directory/FIFO dispatch, digest
revalidation, existing byte error/dispatch and Windows error-classification.
Only Linux's Rust target is installed. Native Windows compilation was unavailable;
the shared streaming algorithm and portable Windows classifier tests pass, with
native Windows execution remaining unverified. No broad suite, daemon, build,
compiler action, live service or upstream fixture growth. Receipts: target/wp727/.

Format, diff, archive and plan checks pass. Packet/review wall time was not
separately measured. This advances fixed-memory ordinary input observation, not
source artifact routing, input transfer or Spawn execution. Overall goal open.

Independent final ACCEPT confirmed the actual bounded observer, signed-size
constructor, complete observed frontier, content-only equality cutoff, error
retention and existing validation policy. No material blocker; the reviewer
reused recorded evidence and did not rerun tests. Windows native validation is
the explicit platform limitation, and this checkpoint makes no execution claim.
