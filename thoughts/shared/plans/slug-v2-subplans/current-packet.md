# WP1127 bounded Capture conversion traffic

Packet: WP-7-339-m7a-capture-copy-traffic
Status: ready

Outcome: add the independently selected fixed Capture conversion-traffic diagnostic,
prove its real error/drop isolation, then observe ONE distinct native diagnostic prefix
under unchanged caps. Slug-native private Linux-test instrumentation; production semantics
and exact source identity unchanged; Windows deferred. Retained bytes and M7A remain open.

Basis: [accepted WP1126 source readiness](./wp1126-retained-allocation-readiness-2026-10-11.json)
rejects current Allocative shared traversal as unbounded, selects the original Capture
producer seam and no Dice count report. Reuse target/wp1126 allocation-design0f6a6a7e29e77b6629185ca477d8b859c329f4c5dabecfb15c3b04666a87545c,
lifecycle9592672f59c0328aa234e0c196ad8059e55cf351b616500e2069d526a91d4e34 and
independent reviewcde443630f254d6e34738b7a08612b3f6f327405775a5fdd10e98322755bbf7f.
WP1125 is the direct runtime/evidence donor: fresh45/production, protected33/fixture31,
ONE20.802s/source390/phase254.15625MiB,360brackets/226pins/18env stable; next exact
rust/BUILD.bazel Lstat refused. Different prefix from WP1124 proves no causal memory delta.
Remaining1.84375MiB is no future-fit guarantee; metadata/source expansion stays queued.

Decision/natural owner: original Capture::finish checks scope, reserves len+64 overlap,
materializes Arc<[u8]> from Vec slice, checks scope again, drops Capture and returns Arc.
Add cfg(all(test,target_os="linux")) CopyCounts child state to existing ScopeState mutex;
explicit arming, default disabled. Five u64 fields created/returned/len_sum/max_len/
max_capture_capacity and armed/overflow flags, state and copied snapshot<=64B. No payload,
path, label, pointer, Arc or collection retained. Record created/len/capacity immediately
AFTER materialization and BEFORE secondcheck; returned AFTER secondcheck success. Failed
firstcheck/reservation reports zero. Cumulative len is copied slice length, capacity is
source Vec capacity, never Arc allocator/RSS/retained bytes. Checked sticky saturation
cannot poison scope, modify errors/charge/credit/publication/admission or enable a retry.
Brief existing mutex updates; none spans allocation/IO/compute/await/formatting.

Expose no public production API. If deterministic post-materialization cancellation
proof needs a hook, one private zero-state generic finish helper may accept an empty
production closure and an injected test closure at that exact point; no callback storage,
registry/new owner or alternate conversion/check path. Review actual helper before use.
Only cfg(test/Linux) owner/hooks/report alter storage; production finish behavior remains
same original checks/reservation/Arc/drop. Independent returned-source/AST lifetimes,
DICE keys/equality/invalidation/Need/request snapshots/source certificate remain unchanged.

At the existing outer synchronous native after_command point, copy snapshot then unlock,
stream one fixed ASCII WP1127_CAPTURE_COPY record<=512B via format_args/writeln into
existing writer; no owned report String or per-object rows. Explicit traffic_only and
retained_bytes=unavailable/armed/overflow qualifications. Snapshot mutex linearization
is separate from RSS. Arm original source scope before work; never reset counters after
work, clear poison or run extra cleanup. Pending cancellation drain does not imply whole
heap disposal. SourceWeak unseen expiry remains vacuous. No Allocative/DICE actor pause,
introspection, count report, interner/label/CodeMap rewrite, global pool or semantic side store.

Rust allowlist only app/slug_core_v2/src/runtime/source_service/scope.rs,
source_service/regular_file.rs, new source_service/copy_counts.rs declared as a cohesive
scope child via #[path], and runtime/tests/platforms_module_preparation_tests.rs. Scope
1035L/Capture502L remain cohesive owner-local; large private native parent gains only
arming/report glue, no copied harness. Estimate diagnostic20–80/proof20–80 plus small
hooks/report; root reviews actual growth. Existing SourceServiceScope test layout488
must change and be measured/rebound, production storage unchanged; Bodies304 stays.
Current donor52source paths retain49 unchanged/3changed plus sole new child=53; historical
Qthree-private exceptions remain distinct and strict, oldCoreexecutable no gate credit.

Proofs: default/unarmed/zero; real success exact copied lengths versus spare capacity;
initial poison/reservation refusal zero; materialized-but-cancelled created1/returned0
with exact existing error and credit release; ordinary cancellation/no buffer retention;
concurrent updates/no lost count/maxima; synthetic u64 overflow sticky/nonsemantic;
Copy state/snapshot<=64B and maximum numeric report<=512; returned Arc survives Capture
and scope Drop, lastArc Drop unchanged, separate scopes, cumulative after lastDrop. Reuse
existing lifecycle/source controls, add only demonstrated missing scalar/real Capture
selectors (expected1–2 extra) to saved45 in one preflight/body batch. Actual selectedcount
must match saved order plus exact declared suffix, never silently lose projection/time
controls. Fresh Core no-run/list/preflight/body+production/singleton; protected33/fixture31
strict qualified WP1119 reuse, fixture35/177/32440662 unchanged. No new fixture assets.

Root resolves accepted WP1126 HEAD before writers. Independent DESIGN on THIS contract
before Rust; disjoint source implementation and target-only quartet/finalizer preparation,
root full source deltas/checkpoints before factory/main, stopped writers/freeze, serial
gates, actual PRELAUNCH before one native, actual FINAL before accepted receipt/commit.
Reuse donorWP1125 quartet/decoders with exact finite source/layout/selector/donor deltas,
no repeated archive inspector/qualifier execution or source-body acquisition: same360
vector and exact runtime profile. Finite current/protected maps; no recursive history.
Fresh pinned nightly2025-09-14 resolved cargo/rustc/rustdoc/rustfmt; offline locked jobs1
CargoJSON no-run, separate fresh invocation every compile/artifact/list/preflight/runtime,
64KiB streamed executable/source hashes. Target/wp1127 helpers/notes/config/receipt allowed;
root owns canonical/current/Stage9/accepted receipt and local commit, predecessors immutable.

Unchanged caps qualified26work/29cleanup/32outer/default5/8/ordinary10/constructor27tail3,
RSS256MiB/AS512/live16/source512/census513/transfer32MiB, SEPARATE cumulative decoded128MiB
and written128MiB, ordinaryFSIZE16/native32/exact Kotlin FDexception, six backings/eight
callers/original18env. Same360held original sources, no new authority/body capability.
One infrequent changed diagnostic native run is necessary for representative conversion
traffic; tiny fixtures cannot establish that traffic or workload fit. Preserve a genuine
memory/time/refusal boundary; no identical replay, capraise, speed or retained-byte cause
claim. No memory reduction is predicted. Needed task time authorized; recurring>180s
examined for necessity/refactoring, reuse accepted source/API proof instead of another
wide audit. Different source/production memory/ownership/interner or unbounded report
needs technical DESIGN/REPLAN. No user permission stop inferred from these packet gates.
