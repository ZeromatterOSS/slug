# Current Slug V2 Work Packet

Packet: WP-7-161-m7a-immutable-link-sequence-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance unchanged authentic Rust-toolchain cquery past cc_internal.intern_seq
at rules_cc cc/private/link/finalize_link_action.bzl:161. Admit one required
positional-only genuine sequence (list/tuple/range), returning a tuple of its
ordered elements only when every element satisfies the existing recursive
structural immutability capability. Preserve original element types/values and tuple order/length,
including duplicates; reject mutable elements explicitly unsupported/deferred.
The outer input list may be mutable; the resulting tuple captures its current
contents. No additional caller/context gate beyond existing private accessor.

Pinned Bazel9.2 8220c619 CcStarlarkInternal.java:397-405 uses a process-global
BlazeInterners weak interner per element then Tuple.copyOf. Param.java:78/94
pins required positional binding; Sequence includes RangeList.java:54.
StarlarkList.java:244-261 Java equals/hash admits mutable equal lists, so
intern_seq([[], []]) returns aliased elements upstream: changing out[0] changes
out[1]. Slug refuses that lane. No direct Java intern_seq tests found; pinned
source discriminates tuple contents and mutable aliasing. Authentic inputs
are _NamedLibraryInfo providers with string/bool fields from finalize_link_action
create_libraries_to_link_values.bzl add_object_files_to_link:67-74.
ObjectFileGroup list fields are deferred.

This is a Slug-native immutable-sequence adapter: required binding, outer tuple
type, length and slot order are source-exact; preservation of original element
types/values is Slug-native. Upstream immutable representatives can change
observable type/repr: StarlarkFloat.java:94-119/StarlarkInt.java:215-223 equate
1 and 1.0 with equal hashes, and signed zero likewise permits substitution.
Such representative substitution (including numeric/signed-zero cases), global
canonical sharing, mutable-element interning and allocation/performance parity
remain unsupported/deferred, as
do full linking actions. Existing structural provider/config/path identity
and valid Unicode contracts remain Slug-native. Linux bootstrap priority;
no Windows implementation or execution work is selected.

## Ownership and scope

Existing Value sequence implementations/AllocTuple and
starlark_structural_is_immutable own this phase-local operation. Provider
capability is already implemented at provider.rs:1067-1071; no new provider
shape or freeze rule. Check genuine list/tuple/range first, then immutable
elements before allocating the tuple. Never execute user-defined iterable or
add Java/runtime semantic work. Reuse existing heap iteration/allocation;
no fresh generic interner, registry, cache, lock or retained collection. This
bounded pure operation has no filesystem/config read or action side effect.
It must keep duplicate entries and shared provider identity/equality semantics.
Returned tuple uses the existing evaluator heap and existing freeze/publication
owners. No untraced long-lived heap Value, async transfer, DICE key or state
repair; docs/developers/dice.md and existing tuple/provider publication apply.

Reuse skill inspected: unchanged existing allocator/structural capability
requires no donor import or retained-representation ledger workflow. No new
hasher/identity encoding or performance claim. Existing large cc_common.rs
receives only registration/module link; helper lives in bounded child
cc_common/sequence.rs with colocated tests. Production estimate below45lines;
new tests below150lines plus small existing Analysis fixture extension.

Allowlist: Loading src/cc_common.rs plus new src/cc_common/sequence.rs and
sequence/tests.rs; Analysis tests/subrule/link_actions.rs only existing fixture
extension; this manifest/canonical Current and compact existing Stage6 link
facade note. Baseline146bc1789. No action/helper activation outside intern_seq,
mutable freezing, generic retained interner, private-gate bypass, new public
oracle fixture or Windows work. Independent DESIGN before code, FINAL before
acceptance. No fallback: mutable/canonical-sharing behavior stays refused or
unclaimed until a dedicated tracing/lifetime-owner design admits it.

## Evidence and validation

Loading checks exact required positional binding, typed list/tuple/range,
empty/singleton/order/duplicates, outer-list snapshot after later mutation,
immutable provider/tuple nested fields and exact metadata strings/bools,
and rejects arbitrary iterable/string/dict/set/None plus mutable list/dict/
provider-nested-mutable elements. Mixed integer/float/signed-zero inputs preserve each original type/repr in
Slug, explicitly discriminating its admitted native behavior. Refusal leaves
original values untouched;
no mutable alias/canonicalization claim. Invoke a previously obtained private
helper from an ordinary evaluator to prove no new caller gate, preserving
accessor admission separately. Use existing tuple/provider comparison/field
semantics; no pointer canonicalization assertions. Protect existing wrapper/
SONAME and provider immutable/refusal tests as relevant.

Analysis extends the existing genuine rules_cc root/subrule link fixture with
immutable metadata providers and duplicate entries passed to intern_seq,
publishing exact fields/tuple values alongside unchanged FileWrites. Source/
config A/B/A on same DICE both Legacy/Observed proves existing publication/
restoration; original action-owner/conflict/scope/publication guards remain.
No fixture growth/copied assets or materialization changes.

Pinned nightly serial Cargo JSON --no-run Loading/Analysis each60s preparation
cap; batch exact nonignored preflight then focused runtime expected under a
fewseconds. Core dependent check, CLI rebuild, unchanged originalthreehash
cquery supervised180s operationcap justified by observed82s publicsource
preparation; no smaller fixture proves the authentic finite closure frontier.
Actual-environment process checks and supervisor cleanup before/after.
Rustfmt/diff/archive/status; root integrates/commits. Bounds are operation caps,
not user ceilings. REPLAN if actual provider fields are mutable or meaningful
canonicalization requires a new owner on the admitted immutable lane. Routine
invocation/compile/test corrections remain in scope.

## Immediate predecessor

WP860146bc1789 admits requested start_end_lib from retained defaultTrue bool
with existing private caller and exec carry. DESIGN/FINAL,29focusedchecks and
Core/CLI pass. Unchanged authentic cquery82.489s first reaches intern_seq;
threehashes unchanged and cleanup pass. Evidence target/wp860. M7A/M8/full
link closure and Windows remain unproved.

## Acceptance receipt (2026-09-30)

Independent corrected DESIGN/FINAL ACCEPT. Review
identified immutable mixed-numeric/signed-zero representative substitution;
contract now explicitly classifies original element type/value preservation
as Slug-native. No implementation started before corrected DESIGN acceptance.
Twenty-three exact nonignored focused checks pass (Loading17/Analysis6), total
runtime0.346s. Five new unit cases prove sequence shape/binding, outer-list
capture, nested immutable providers, Slug-native numeric/signed-zero type/repr
preservation and mutable refusal without input mutation. Real root/subrule
metadata tuples preserve exact primitive fields/order/duplicate semantic
equality under same-DICE source/config A/B/A in both routes, alongside all
original FileWrite owner/guard assertions. Production42lines including test
module registration; tests135newLoading plus net44Analysis, no public fixture.
Existing allocator/structural capabilities reused; no cache/retained owner.

Pinned serial preparationLoading11.60s, Analysis10.84s, Core3.50s, CLI11.89s.
Unchanged rebuilt authentic cquery exits2 in83.148s, clears intern_seq with
actual immutable library providers and first reaches cpp.
incompatible_use_specific_tool_files at finalize_link_action.bzl:233. Original
three hashes unchanged; supervisor and actual-environment cleanup pass.
Format/diff/archive/status pass. Evidence target/wp861; measured evidence
window08:34:51UTC to after08:48:41UTC includes design correction/review,
preparation and replay. No canonicalization/allocation/performance claim.
Mutable/global representative substitution, full link/toolchain closure,
M7A/M8 and Windows execution remain unproved.
