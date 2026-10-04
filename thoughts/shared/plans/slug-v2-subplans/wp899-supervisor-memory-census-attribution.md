# WP899 bounded supervisor memory/census attribution

Base d162e854b; Slug-native Linux diagnostic only. Global13 proves zero Slug
records, allocator failure text and SIGABRT with a separate disk_census_bound.
Actual source state, causal order and allocation owner/cause remain unknown.
Observable outcome: subsequent owned-process receipts distinguish sampled root
virtual-address size, observed Linux VmPeak and sampled root thread count, and
identify the precise selected census deadline/entry-limit producer. No actual
run is selected by this implementation. All old spent evidence remains immutable.

Root exact code allowlist scripts/v2_source_admission_supervisor.py and its
existing tests.py; scheduling/compact contract+receipt only. No Rust, CLI,
allocator, source owner, DICE, caller input/environment, cap, retry, census policy,
STOP/CONT/pidfd/join/cleanup or exception-priority change. Every old29 gate stays;
existing detail-absence assertion for newly tagged deadline is narrowly updated.
Approximate growth <=150 implementation/150tests; review actual cohesive owners.

Memory owner: existing identity-pinned memory_sample reads /proc/status and
/limits once as before. Private frozen MemorySample stores rss/swap plus vm_size,
vm_peak and threads, and explicit observed boolean. Pure strict parser requires
exactly one of five fields, kB byte units for Vm*, nonnegative decimal bytes,
positive decimal Threads. No cross-field ordering/ratio validation (kernel fields
are independently read), no status strings retained/printed. Existing before/
after starttime and existing sampler at-or-below AS-ceiling check preserved;
initial child handshake still verifies exact equality. Zombie/confirmed5ms exit paths preserved;
exit yields unobserved allzero, missing/invalid fields on liveprocess refuse.
RSS/swap existing aggregation/refusal remains. Extra root-only scalar maxima and
observed count update only on validated actual root sample, before later census
failure can suppress them: sampled_root_vm_size_peak_bytes, observed_root_vm_peak_bytes,
sampled_root_thread_peak, root_memory_samples. VmPeak is Linux reported process
lifetime highwater; VmSize/threads are sampled lowerbounds. Independent maxima
are not simultaneous/allocatedphysical/RSS or allocator-failure-time measurements.
No extra process/status reads, payload/path/environment/maps/stack/cert copies.

Census owner: inline frozen CensusFailure{site,origin} with finite enum sites for
cycle initial, pre-stop, stop wait, discovery return, post-scan, resume check,
disk account-clock, per-FD traversal clock and metadata entry ceiling; origin Deadline or EntryLimit.
No inferred phase cause; site is the exact check that created the selected error.
Existing active() calls receive explicit site codes, raise same existing reason
and detail. Disk metadata account-clock, separate per-FD traversal-clock and million-entry
ceiling each tag their own error. Initial and pre-stop checks have distinct codes.
A named internal MAX_CENSUS_ENTRIES constant preserves1,000,000 exactly and permits
tight deterministic fixture patching without any CLI cap control. Shared AccountingError and first-refusal slot admit only this fixed
POD alongside existing DiskFailure/ChildFailure. Receipt emits two numeric fields,
zero if absent. Existing primary error, cause redaction, resume finally, absolute
local/outer deadlines, same source+supervisor budget and all caps unchanged.
No partial census accepted and no entry/time bound relaxed; no retrospective
classification of global13 from duration or the new codes.

Discriminators: all29 protected controls; pure parser units/missing/duplicate/
negative/zero-threads and exit-vs-live; actual small anonymous mmap under tightened
96MiB AS retains64MiB without touching it and catches a further48MiB allocation
refusal, reports verified AS and lowRSS vs higherVmSize/VmPeak/thread stats with
actual join/cleanup. This demonstrates metrics distinctness, not actual global13
cause. Existing stopped-cycle negatives strengthened for exact producer details;
controlled deadline crossings before stop/stop wait/postscan/resume and leafclock,
entry-ceiling tagged explicitly; native primary error and first-refusal precedence
protected. Deterministic mocked clock/entry count used where useful, no slower
actual source/network workload. Root serial one focused Python batch under30s
TERM/KILL3 after compile syntax; exceptions remain finite. Independent DESIGN then
FINAL review before acceptance. Plan/archive/diff checks and compact preservation
commit. No new Cargo required: zero Rust changes, current802 artifact remains
accepted only for unchanged Rust and oldsupervisor evidence is not fresh validation.

Accepted component validation is recorded in wp899-supervisor-memory-census-attribution-2026-10-04.json. The controlled35-method batch proves the named diagnostics only; global13 remains causally unclassified.

Concurrent one-test strengthening was reconciled after c37742f9e: only the first actual validated memory sample is exposed, cleanup samples are unobserved, and exact count1 plus VM/thread equality is required. Three correction FINALs and fresh1/1 in0.0585s pass. Current35 coverage is34 unchanged preserved+1 fresh, not a new35 batch. The original35 batch remains preserved in the receipt.
