# WP1143 Host bzl temporary load-key borrowing

Packet: WP-7-355-m7a-host-bzl-load-key-borrow
Status: selected — independent DESIGN accepted; bounded implementation

Detailed contract: target/wp1143/packet-contract.md
SHAa2c918464d60a1384b2752ef69b266e4a0af7b5487b748c37f2446d492cfa72a

Baseline: cc71405cc67ac786c100825d01bbe52136cb29b6, clean preservation of failed WP1142.
Last accepted semantic/native projection: WP1141, receipt
thoughts/shared/plans/slug-v2-subplans/wp1141-rust-runfiles-build-metadata-2026-10-11.json
SHA352052b2f73c84bd3e2c97117a6bb1d30e7c6d692aa6a224856b5b39c31885e6.
Preserved WP1142 Rust2 remains coupled, unaccepted; its failure receipt
wp1142-resource-failure-2026-10-11.json SHA74b80663bf10454271cd6adf09f7fb8a7f2dad494b3ae6bfeb27d6a4b498bfd4
records raw phase262304KiB/160KiB excess. No replay of that cohort or its FIRST.
Inherit target/wp1142/packet-contract.md SHA04e7094b85a682a6f50232782ea0d859944a7adf1b23a817ed0e9141f6f844e3
except the explicit allocation, evidence and review changes below.

Outcome: compute_host_bzl_module borrows raw keys from its independently owned
loads Vec<String> for temporary loaded_modules across child awaits/evaluation.
Remove only load.clone() at this Host driver, using load.as_str(); LocalBzlLoader
receives the same raw spellings in the same order. Keep retained FrozenBzlModule.loads
owned. Build independent retained_module_closure before moving loads and end/drop
temporary loader/rows first. Generalize unused tuple keys in private visibility/retained functions. Keep the
existing String-typed flattened wrapper for its empty-slice inference, extracting
one private borrowed-module iterator leaf with the original traversal body shared
by both closure wrappers; other drivers keep owned keys. No new interner, pool, cache, DICE key, lock, API, dependency or allocator.
Borrowing existing storage takes priority over interning temporary unique keys.
Compatibility: Slug-native private scratch allocation; exact existing semantic
identity, canonical labels, source digest, visibility, duplicate first-match order,
manifest reachability, errors/Need, observations/epochs, invalidation and child-module
lifetimes remain. Linux selected; Windows deferred. No extra source/metadata grant.

Natural owner: HostBzlModuleEvalKey/HostBzlModuleObservationKey normal computation,
its own retained loads and child FrozenModule values. Scratch keys cannot escape
into a frozen value or evaluator heap. Cancellation, error or Need drops scratch
with the existing future; publication owns strings and retained module Arc closure.
The infallible retained closure allocation moves before loads movement/final path
construction; system OOM may abort in that changed order, without typed error change.
docs/developers/dice.md ownership principles apply; no lock crosses a compute.

Source basis: stopped target/wp1136/loading-allocation-readiness.md
SHA06203a556fb822c2a9bdf1ef410a913cc76a626e8e30560a50bee5579a0f568e and
target/wp1142/loading-gate-readiness.md SHAff7ab97a889db455685ae197f4d3257c99af738e052fc7d0e441896fc039669a.
This is an identified duplicate, not attribution of the phase excess. Structural
removal and representative unchanged256MiB fit are acceptance criteria; no savings,
speedup, whole/future-fit or A/B claim, and no unchanged native replay for benchmarking.
Stage9 row143 remains existing manifest/scratch borrowing concept, no donor import.
Existing Starlark scoped string interners and frozen representation stay unchanged.

Allowlist: app/slug_loading_v2/src/bzl_module.rs and existing
app/slug_loading_v2/src/bzl_source_memory_tests.rs; scripts/v2_probe_evidence.py and
scripts/v2_probe_evidence_tests.py for finite current Loading/known-test routing;
root target/wp1143 factory/FIRST/roles/checkpoint/gate/artifact/receipt records;
matching Stage9 disposition and canonical/manifest scheduling. Pending Core Rust2
is byte-identical WP1142. All other production sources/auth6, reader/scope/counters,
caller8 and original source361/Core59 JSON remain byte-identical.
Large driver remains cohesive: change one scratch owner site and two private
unused-key signatures plus one shared module traversal leaf, about28 net production lines; no adjacent split/refactor.
One owned-only test in existing memory-test file, estimate <=100 added lines;
no new copied fixture/harness. Reviewer may choose the smallest demonstrated gap:
raw-key borrowing/duplicate first-match plus independent retained closure after
scratch/driver retirement. Reuse existing visibility/order/warm A-B-A/missing/parse/
cycle/evaluation/freeze/child-lifetime regressions. No test solely mirrors tuple return.

Finite evidence frame: before56=accepted WP1141.after55 plus known memory-test old
ddc4136ed6e690045646499830bf0d3376cb95eb14bf52aaa1fbe4125e9faee4.
after56 changes ordered pending Core parent/proof, Loading driver and test only;
52unchanged. Driver old95305bdf2fafc8440c0aa5ae17f46dc98314ff6448b84f39c58886bb9a1ac0b0.
Known validation before/new gate source maps name only this existing test; current
after hash/name bound after owner STOP. Prior accepted native full55 is checked
against before56 with just test projected out, never against failed WP1142 native.
Direct103/original finite Q/L/S/H/O/E/I maps remain complete, no ancestry union.
Core59 selectors whole-byte unchanged; Loading exact old29 ordered prefix plus
one actual new nonignored discriminator30. Current fresh59+30 plus qualified Q7=96;
fixture31 separate, portable3 unproved. Old L29 operations/maps are historical only.

Helper changes must enforce the exact seams in the stopped gate-readiness note:
fixed P1137 reader introduction; P1132 original54 projection excludes known test,
retaining Rust2/oldregular04413 differences and adding named oldLoading95305 bracket;
old L original53/29 map same named95305 replacement, while Q40e48 and its existing
root-syntax exception stay distinct. No new Q exception. Old L artifact identity
joins obsolete Core as saved metadata, never physical runtime credit; fresh Loading
artifact at its possibly same path must match actual full SHA. Q Bzlmod remains
physical strict. Current protected Loading after maps use actual after, not before.
Preserve15 public helper signatures/six immutable utility bodies and all13 meaningful
controls, extending their wrong test/driver/introduction/selector/historical-map/
obsolete-versus-fresh artifact refusal coverage. Review growth for finite auth only.

Validation: independent DESIGN before any owner/helper/wrapper implementation;
independent stopped SOURCE of actual diff/helper/control/wrappers before imports.
Root freeze/checkpoint after fully read SOURCE. Fresh serial Loading/Core CargoJSON
compile with pinned toolchain, full streaming executable SHA/profile/features/binding;
separate Loading list/preflight/body30 and Core list/preflight/body59, default Core+
dependent slug production, singleton ignored list, helper13 and strict Q7/fixture31
qualification. All factory/gate/artifact/list/preflight/body/native/FIRST invocations
are distinct fresh tool cells. Root PRE and strict prototype/pins/caps/361FD guard
before ONE native per changed frozen cohort; actual JOIN then immutable FIRST once.
Independent FINAL before coupled acceptance/local commit. Failure is preserved;
no identical replay, cap lift or relaxed assertions. No new source acquisition.

Caps unchanged26/29/32/default5/8/ordinary10/constructor27+tail3, RSS256/AS512,
live16/source512/census513/transfer32/decoded128/written128/FSIZE16/native32,
Kotlin-only FD exception, source361/caller8/env18/backings6. Compile separately.
Recurring work over180s examined for necessity/refactoring, not deadline or permission
stop. New owner/helper transition requires current DESIGN/SOURCE; reduced same-table
review advisory is inapplicable. REPLAN only contradictory ownership/authority/
compatibility or a prerequisite outside this bounded contract. Invocation corrections
stay in scope. Broader metadata plan waits until representative memory fits.
Producer/actions/Rustc/full bootstrap remain open.
