# WP1137 Host final-buffer allocation correction

Frozen detailed contract: target/wp1137/packet-contract.md.

Packet: WP-7-349-m7a-host-final-buffer-correction
Status: accepted — independent FINAL; current memory gate fits unchanged256MiB; selecting exact rustfmt metadata successor
Baseline: f850b8478 preservation commit over accepted WP1135
33699afa7a762fa6556eda5f139d5557594a5db5. The pending WP1136 Rust2 component
metadata/routing candidate remains applied, not accepted. Frozen WP1136 contract
4f7789c82d37f11d5b91aa8228823b821519d2c208abb2a9d91bc688e4c9545c
and complete inherited contracts remain authority except explicit changes below.
Last ACCEPT native/receipt stays WP1135, not the rejected WP1136 native.

Evidence: WP1136 resource failure receipt
thoughts/shared/plans/slug-v2-subplans/wp1136-resource-failure-2026-10-11.json
SHA775884efff5555b0c32ddf552320e040399d462fafad0d5a9456e25274fe3eeb.
Actual frozen native phase262180KiB exceeded262144KiB by36KiB. Immutable FIRST
once exited1; no candidate/FINAL acceptance. Preserve every old source/config/
checkpoint/operation/stream/review. Never replay the unchanged cohort or rerun
its FIRST. Resource review57b8039988332058be408d90d4a21d62a845f332cc00d66a81e24957fadf87d3
and Core readiness9498155ac90205384bd052ba924bb71a6f068dced6892b2110d8976588a0b6b0
are read-only source basis. Loading advisory06203a556fb822c2a9bdf1ef410a913cc76a626e8e30560a50bee5579a0f568e
is an unmeasured alternative; no Loading implementation selected here.

Outcome: scoped regular Host/KnownShas reads initialize only their final,
uniquely owned pinned-size Arc allocation, eliminating their intermediate Vec
and its final payload copy. Registry keeps its original fixed-capacity Vec,
full64KiB read, overflow/charge ordering and conversion behavior. The returned
Arc remains independent of request/scope scratch. No additional source/policy
admission beyond preserved pending WP1136 rust/rust_analyzer Lstat. No new
interner/cache/global pool, DICE key/lock, allocator/API/dependency or Windows work.
Compatibility: Slug-native bounded allocation implementation; exact existing
Host file bytes/inode/name/EOF/error/epoch semantics stay. System-OOM handling
is explicitly narrowed below, not claimed to be exact Bazel behavior.

Natural owner: regular_file.rs/read_regular_with, SourceServiceScope buffer bank,
normal path_observation FileBytes/current source epoch, independently returned
Arc bytes. Retained DICE semantic payload shape/equality/history remain identical.
Host capture storage is unpublished command/read scratch until full validation;
returned payload history remains bounded by existing cumulative DecodedBytes,
not a per-Arc live bank lease. Arc/Weak/last-Drop and scope cleanup remain separate.

Decision for DESIGN: reuse repository_archive_http/scoped.rs transfer_selected_capture
unpublished Arc::new_uninit_slice/initialized-prefix pattern (SHAc349008bb6af9a5c04fea351a032746bfaf37f36628aa0f0dae5fab20c7e6bec).
Use the existing64KiB initialized read scratch; charge before writing new slots;
never expose partial storage. Initialize all0..pinned_len exactly, verify full
EOF/size/inode/current name and scope before assume_init/publication. Empty capture
has a valid fully initialized empty range. Partial IO/growth/shrink/cancel failures
drop MaybeUninit<u8> safely; no unsafe initialized read or public uninit interface.
Keep interrupted Host reads bounded by actual scope checks and IO postcheck priority.

Explicit allocation-failure/sequencing decision: budget/overflow/scope refusal
must precede final Arc allocation. Reserve pinned_len+64 conservative header/owner
credit and64KiB scratch under unchanged bank/caps. Arc::new_uninit_slice is infallible,
as was existing final Arc::from. Removing the Vec removes its try_reserve_exact
system-allocator Allocation result; a physical system-OOM can abort earlier,
rather than after the old typed Vec allocation/refund. This narrow Slug-native
sequencing change is selected explicitly; do not claim preservation of a fallible
final-Arc allocator. Bound length/credit; preserve existing explicit poisoned
Allocation/cancel/acquisition and other typed refusal precedence. No allocator_api,
custom allocator/manual Arc layout or catch/retry. If independent DESIGN rejects
this explicit change, correct the contract before implementation, not permission-stop.

Implementation allowlist: regular_file.rs and existing regular_file_tests.rs;
shared evidence/helper controls solely for exact current-memory/source transition;
mechanically adapted fixed target/wp1137 factory/FIRST/artifacts; Stage9 matching
Core source row and root compact receipts/scheduling. Pending Rust2 policy/proof
files and all other production/source owners remain byte-exact WP1136. Reader
currently537L/one~190L validation loop: keep one cohesive identity/IO loop and one
private final-buffer helper/enum, avoiding duplicate Host reader or new framework.
Estimate <=120 net production lines and <=80 net test lines; responsibility triggers,
not quotas. No changes to scope state/limits, copy counter storage, production CLI
semantics, request selector, archive capture, source grants or normal Host observation.

Traffic: existing CopyCounts still counts actual Vec-to-Arc created/returned copies;
Host final storage creates no such copy and must not increment those counts. Registry
controls/counts/overflow/arming stay unchanged. A lower native count is copy traffic,
not retained memory/RSS saving or36KiB-cause attribution. Discriminate final storage
shape using actual live-bank peak, independent Arc identity/last-Weak behavior,
known-size content/partial cancellation. No performance percent/speedup claim or
A/B replay; acceptance is structural duplicate removal plus unchanged256MiB fit.

Fresh owner tests: all six existing reader tests plus one bounded final-storage
partial/post-publication cancellation discriminator, added to unchanged Core52:
Core59+protected36=95. Reuse existing reader byte/empty/max/KnownShas/missing/nonregular/
overlimit/inode/growth/shrink/current-name/Interrupted/IO priority/cumulative history/
Registry proofs. Update only former Host two-allocation overlap expectations to
one final allocation; test exact lower bank succeeds and one-byte-under refuses
without payload publication, then live credit0 and last-Weak expiry. Final-helper
partial prefix never becomes initialized Arc; post-created cancellation retires
Arc/credit; counters remain charged only for consumed bytes. Registry CopyCounts2
remain in current Core52. No original Host payload acquisition in source tests.

Provenance correction for distinct next cohort: accepted-before54 remains WP1135
actual after54. Add exactly one known existing reader-test source (before SHA
6da02db74f164389b00cb5c296cfeb84a644843481ffc724004bcadb8b536a55) to
before55/after55; current physical design baseline54 is preserved WP1136 after54,
with exactly pending Rust2 differences from accepted-before. Final changed order
is Rust parent, Rust proof, regular_file.rs, reader tests; other51/auth6 unchanged.
No authority addition; exact known/new gate-source map is reader tests only.
Original WP1132 compile54 remains intact; compare its original keyset/other52/auth6
to accepted-before54 excluding the one added gate source, original Rust2 differences
remain distinct from new current4 allowlist. Historical source maps stay original.

Shared helper must authenticate that distinction explicitly, preserving all six
utility/qualified-source function bodies, full original pin/config+2 relations,
original Q/L/S/H/O/E/I direct103/eleven gates/seven original source exceptions,
distinct L-before/I1131 introduction and strict protected executable identities.
Only one extra named replaced-source exception regular_file.rs is allowed where
old Core compile/P5 native/L gate maps pinned it; use EACH original map's recorded
SHA, never substitute another predecessor hash or pretend old Core ran current code.
Original Q regular_file already has its own older diagnostic exception; preserve
that original pin separately. Reader-test new source stays outside old maps, explicitly
in current freeze/checkpoint/configs. No arbitrary extra helper/binary exception.
Caller metadata has exactly one changed ID2 row: same regular_file.rs path; old
P5 row SHA04413bdc878b65224556f24a8d8b56dc9fe289041bd3704f866035323394d48f/15294B
authenticated against accepted-before54/receipt, fresh row SHA=after[regular_file]
and actual current source byte count. IDs1/3–8 remain entire-row exact; cardinality8
unchanged. Preserve original prior caller metadata; no arbitrary caller exception.

Before implementation helper author must pin exact original maps and expose wrong
extra exception/order/source/count/protected-binary refusals within13 controls.

Validation/integration: independent DESIGN before owner/helper edits. Resolve actual
pinned cargo/rustc/rustdoc/rustfmt; format changed Rust only, diff/archive checks.
Stopped full SOURCE review/current freeze/checkpoint precede any new helper import/
factory main. Fresh serial compile CargoJSON/current Core executable streamed SHA,
separate artifact binding/list/batched59 preflight/body; default Core+slug production;
exact ignored singleton; all13 helper controls. Reuse strictly unchanged protected36/
fixture31 evidence only with the explicitly revised memory-source relations above.
PRE reviews actual new current/source/compile/native config; ONE distinct native,
immutable FIRST once after JOIN, FINAL before coupled acceptance/local commit.
Old failed native remains immutable evidence, not an accepted predecessor or template.

Caps unchanged: work26/cleanup29/outer32/default5/8/ordinary10/constructor27+tail3;
RSS256/AS512/live16/source512/census513/transfer32, independent decoded128/written128,
FSIZE16/native32/Kotlin-only FD exception. Current source361/caller8/env18/backings6
and all source bytes/order remain unchanged. Broader plan expansion waits for the
representative cohort within memory budget. Native identity/producer/actions/
Rustc/bootstrap/whole/future fit and savings remain unproved; no new frontier prediction.
Recurring work>180s is examined for actual need/refactoring, not a deadline. Reuse
closed Core allocation seam rather than repeat broad scans; setup/FIRST protocol
changes remain bounded explicit relations, no new validation framework. REPLAN only
for contradictory semantic owner/authority/compatibility choice; root can resolve
this declared private allocation sequencing choice under existing user authorization.
