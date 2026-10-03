# WP899 scoped selected-download transfer amendment

Baseline: b9e0f528c. Status: component validated; independent DESIGN/FINAL accepted.
Parent WP-7-199 remains unaccepted. Independent source,
input/lifecycle and accounting audits accept this bounded design direction.

Global8 proves first Scope BufferLimit/outer DownloadCapture but no inner creator.
Source audit independently establishes another concrete implementation defect:
transfer_selected_capture holds Vec(size),64KiB scratch, then Arc(size+64), and
Arc::from(Vec) copies the payload. With the actual-root recorder, a real10MiB
file requires25248231 credit bytes, exceeding16MiB; one Arc plus scratch requires
14762471. These are deterministic source-derived credits, not global8 attribution
or RSS. No extra trace/replay is needed to discriminate this defect locally.

Choose direct initialization of one Arc payload for the scoped diagnostic path.
The existing unscoped transfer stays byte-for-byte unchanged except the early
scoped dispatch. Precharge size+64 header allowance and unchanged64KiB scratch
before allocation, in the same bank. Initialize only bounded slots via safe
MaybeUninit::write; expose no byte slice before every byte is initialized. A
single documented assume_init follows full EOF, exact-length, digest and final
Scope/active validation. Error/partial paths drop the uninitialized Arc safely.
Original growth, checksum, shrink, cancellation and Scope precedence remain.

Pinned Rust1.91 alloc/src/sync.rs provides stable Arc::new_uninit_slice (1.82)
and no fallible slice constructor. Existing final Arc::from is infallible;
eliminating Vec removes its separate recoverable try_reserve_exact failure.
Record this allocation-failure change explicitly; do not add allocator machinery
or promise fallible OOM. Resource precharge still refuses before payload allocation.

Compatibility: Slug-native diagnostic allocation policy; normal source bytes,
SHA identity, returned Arc<[u8]>, SelectedExtensionDownloadVerified, DICE keys,
dependencies, request/current-certificate authority and unscoped semantics stay
unchanged. This is operation-local transfer scratch: credits live until helper
locals drop, then ownership transfers to the existing verified-download/DICE
owners. No retained semantic accounting redesign, cache, interner, new owner,
donor or dependency. Reuse std Arc; Stage9 diagnostic-accounting row records it.

Allowlist: repository_archive_http.rs; repository_archive_http/scoped.rs;
existing tests/repository_archive_http_tests.rs; this contract, compact receipt,
matching Stage9 row, manifest/canonical status. Root owns edits/serial validation;
independent reviewers read only. Estimate net120-200 Rust lines; actual267 consists of198 discriminating test
lines,66-line cohesive scoped helper and3-line dispatch. Independent FINAL accepts
this bounded growth; no existing production owner or representation moved.

First compile/test the new10MiB actual-root fixture against old production code:
normal scoped file transport, actual filled chunks/SHA, armed recorder, expected
candidate success. Preserve its exact baseline BufferLimit failure. After repair,
require exact returned bytes/hash, no poison, peak within16MiB, operation-local
credit returned to recorder baseline and clone/lastDrop retirement. Add tiny
direct-transfer required-credit and required-1 controls; refusal precedes transfer
body read/allocation, not preceding transport. Cover empty/partial/shrunk/growing
capture, bad SHA, cancellation after partial/final read and sticky scope failure.
Reuse existing unscoped file/SHA/cap, redirect, redaction/cancellation and declared
oversize cases plus changed/protected source lifecycle controls; no new network
fixture or slow TLS repetition. Source-derived one-payload credit reduction is
the correctness discriminator; no timing/RSS performance claim or benchmark.

Pinned compile/no-run <=60s, exact batch preflight per rebuilt artifact, focused
execution <=30s, default Core+CLI all-target compile; changed Rust format, archive,
diff and plan checks. Independent FINAL covers actual diff and evidence before
preservation. All registry/byte/bank/HTTP/deadline/supervisor limits remain unchanged.
No actual observation in this amendment; any later run needs a new reviewed freeze.
REPLAN for payload identity/retained-owner changes, unsafe read of uninitialized
storage, bank/cap growth, source bypass or command/process activation.

Global8's exact producer remains unproved, even if the local defect is repaired.
Full actual source/profile admission and coupled WP899 FINAL remain required.
Preserve only on the local review branch; no main activation or push.
