# WP1128 root-package origin sharing

Packet: WP-7-340-m7a-root-origin-sharing
Status: ready — independent DESIGN pending

Outcome: replace repeated identical root-package origin string construction with
one lazily created invocation-local Arc, preserving original diagnostics and source
ownership. Slug-native allocation optimization; admitted exact semantics unchanged,
Windows deferred. No new namespace/body admission or memory-cap increase.

Basis: [Accepted WP1127](./wp1127-capture-copy-traffic-2026-10-11.json)'s sole native diagnostic passed with 390 admitted sources, 481 returned
copies / 8120409 copied bytes, phase254.31640625MiB under256. Cumulative conversion
traffic is not retained memory or the cause of RSS; headroom1.68359375MiB gives no
future-fit guarantee. Reuse source-only origin-reuse-readiness.md bd880236ce0bee358
f0f85a89256bddf11ce9c84bf98602bc1a1c5d75c2a4c87 and existing Stage9 root BUILD syntax
ownership. Current bzl_module.rs6330 builds the same relative byte-to-char String
and package-prefixed Arc separately for each successfully resolved direct load.

Natural owner: root_package driver, its immutable package/source and existing
RootPackageLoadError Bzl/ExternalBzl origin Arc. One Option<Arc<str>> initialized
before the load loop remains invocation-local. A private producer helper accepts
that option plus borrowed package/path inputs, creates the original spelling with
the original byte-to-char mapping only on first use, returns a cheap Arc/Dupe clone
and ends its option borrow before any await. Call remains after successful label
resolution and before root/external dispatch, including before external route
failure. Zero-load and first invalid-label paths allocate no origin. Independent
driver invocations do not share storage. Returned errors retain their own Arc;
success/Need/cancel driver retirement drops its cache handle. One origin stays live
through the remaining driver/awaits, a small deliberate local retention tradeoff.

Keys, structural equality/content, invalidation/restoration, ordered observations,
Need/error polarity, source/AST/evaluator lifetime, child routes, visibility, source
certificates and request projections remain with existing owners. No shared public
API, global pool/interner/cache, semantic key/value field, lock or donor import.
Existing Arc/Dupe is leaf reuse; interning would add unnecessary ownership machinery
for this single immutable value. This proves reuse, not workload frequency, total
allocation/RSS savings, retained-byte attribution or whole-build fit.

Rust allowlist: app/slug_loading_v2/src/bzl_module.rs and existing
app/slug_loading_v2/src/host_package_load_tests/root_package_syntax_tests.rs only.
Estimate parent15–25/proof40–70, one new actual-helper ownership proof. Reuse existing
real driver root/external dispatch, semantics/events, zero-load/Need/A-B-A restoration,
syntax identity/retirement and nested diagnostic tests. Missing proof covers exact
empty/nonempty package, BUILD/BUILD.bazel and byte mapping, same invocation Arc identity,
different invocation separation, cache/drop/Weak behavior and real nested error lifetime.
No copied harness/fixture asset or allocation-only implementation mirror.

Independent DESIGN before implementation; root full source delta review before
freeze. Fresh pinned-toolchain offline locked jobs1 CargoJSON Loading/Core no-run,
one exact list/preflight/body batch per fresh executable, production Core/CLI check.
Reuse saved Loading26 plus missing named dispatch/diagnostic controls and one new
ownership selector (expected29); fresh Core47 original order. Qualify Bzlmod7/fixture31
only against exact unchanged semantic owners and explicit finite historical source
exceptions, never old Loading/Core executable credit. Actual selector counts determine
the recorded gate; no omission or prediction substitutes for execution.

One representative changed-origin native observation after actual PRELAUNCH retains
exact360 source vector/eightcallerpaths/18env/sixbackings and qualified26/29/32,
default5/8/ordinary10/constructor27tail3; RSS256MiB/AS512/live16/source512/census513,
transfer32/SEPARATE cumulative decoded128+written128MiB, FSIZE16/native32 and exact
Kotlin FD exception. Compilation is separate from workload memory. No identical
replay or capraise. Preserve actual failure/boundary. Actual FINAL before accepted
receipt/local commit. User authorizes necessary time; examine recurring>180s for
refactoring, reuse accepted finite source/API anchors and avoid repeated wide audits.

REPLAN only if scope/identity/lifecycle invariants cannot hold, an unavailable bounded
gate is needed, or memory pressure requires a different semantic owner; resolve
technical decisions within existing task authorization. Broader capability expansion
remains queued until actual memory readiness, not an inferred permission stop.
