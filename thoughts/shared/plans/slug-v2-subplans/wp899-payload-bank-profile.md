# WP899 bounded actual diagnostic payload bank

DESIGN checkpoint on clean8187e837d. Full WP899 remains unaccepted.
Compatibility: Slug-native Linux cfgtest diagnostic profile, no Bazel parity claim.
Inherit the accepted first-buffer component, single-payload transfer and
[actual producer receipt](wp899-payload-credit-frontier-2026-10-04.json).
Root owns all edits, serialized compilation/tests and preservation.

The actual first SelectedPayload reservation requested19,234,400 bytes (payload
19,234,336 plus64 header) with4,211,303 live. Its23,445,703 total exceeds the
16,777,216 bank before Arc allocation; the existing64KiB scratch would require
23,511,239 bytes. This is a policy mismatch for this admitted payload, not a new
copying defect. Native pairing and selected-leaf identity remain unproved.

Select a bounded40MiB bank only for the explicit Linux cfgtest actual_root profile:
the existing selected payload cap is32MiB, with8MiB additional headroom for the
charged4MiB association slab/headers and existing64KiB transfer scratch. This is a
finite diagnostic admission choice, not a measured whole-source upper bound or
promise that all later operations fit. The constructor's maximum live-buffer
allowance becomes40MiB only under the same Linux-test cfg; zero and40MiB+1 reject.
Default remains16MiB, and non-test/non-Linux validation remains capped at16MiB.
Explicit smaller limits remain valid. Registry profile256/Default128 and every
other limit, cancellation, first-poison, allocator, credit lifetime and semantic
source/DICE owner remain unchanged. The4GiB hard supervisor/user envelope and all
supervisor time/disk/log/swap/accounting controls remain unchanged. No actual
observation is selected by this component.

No new storage, allocator, interner, cache, donor import or payload copy. Reuse the
existing limits value, Scope bank and one safely initialized Arc. Operation-local
credit still retires at transfer return; returned/retained download bytes are not
fully accounted by this bank. Native Arc allocation remains infallible. No heap/RSS,
source-fit, total-runtime or historical40GB inference.

Allowlist: source_service/scope.rs for cfg-specific finite maximum, actual_root
value and validation only; source_service/tests.rs for the existing profile
constructor/limit discriminator; runtime/tests/repository_archive_http_tests.rs for
one normal32MiB file capture under the explicit profile and an explicit16MiB pin
in the existing10MiB overlap fixture. Preserve its old one-copy versus two-copy
assertion; do not weaken it by running it under the larger profile. Existing
buffer provenance producers/proof logic and normal transfer implementation stay
byte-identical. Contract/receipt, existing Stage9 accounting row and scheduling
status only. Estimate10–25 implementation and70–120 test lines; no adjacent cleanup.

Evidence: existing profile test asserts full limits equality except named registry
and bank fields, Default16/explicit40, exact bank equality then+1 sticky refusal,
credit release, constructor zero/maximum+1 and unchanged other ceilings. New real
file capture streams a SHA-pinned32MiB file through the normal selected capture and
transfer callees (the same scoped functions used by the public helper), with
an armed actual256 recorder. It verifies exactly one returned payload, SHA/content,
creation peak=recorder+payload+64+64KiB, live returns to recorder baseline, no poison,
clone pointer identity and actual owner/recorder lastDrop plus capture deletion.
The established10MiB fixture retains16MiB and its old duplication discriminator.
Reuse62 protected selectors on a fresh artifact plus the one new selector. All
fixtures serialized, including ancestor-sensitive normal DICE cases. No actual
source/network probe in this component; no need to rerun unchanged15s withholding
TLS control. Default Core+CLI all-target check proves production cfg remains valid.

Pinned nightly2025-09-14 tools; test compile/no-run and default check each60s
TERM/KILL3, one exact preflight then focused63 selectors30s TERM/KILL3. Use normal
approval for three loopback controls. Format changed Rust, diff/archive/plan checks.
Independent DESIGN before code and FINAL after evidence. REPLAN only if supporting
this profile needs new source/identity/lifecycle ownership, changes semantic payload
bounds or relaxes a user/supervisor ceiling. A later actual observation requires its
own reviewed contract/freeze with the explicit40MiB diagnostic bank recorded; all
previous spent attempts retain their original16MiB evidence.
