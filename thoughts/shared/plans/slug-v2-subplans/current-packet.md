# WP1088 external repository glob ownership

Packet: WP-7-300-m7a-external-glob-source-owner-r1
Status: accepted — next producer work frontier pending selection
Baseline: d9b9fcc98aac0a67f65ffe5c599f0c227cba6f06
Owner: M7A normal retained compiler producer; Stage4 glob segment/traversal.
Compatibility: Slug-native Linux operational repository namespace ownership;
admitted Bazel9.2 raw-name/matcher/traversal semantics protected; Windows deferred.
Receipt: [wp1088](wp1088-external-glob-source-owner-2026-10-10.json).

Implemented the independently proved ownership loss. External boundary scope
shares the original route once. Segment keys share that route and an OS-native
relative directory built from inherited Latin1 package/raw child bytes. Literal,
symlink and wildcard-base resolution use the existing tracked original repository
path owner; wildcard listing uses its completed namespace/requested path and unions
exact resolution/listing observation Arcs. Full materialization/path Needs and
complete-error-before-Need behavior propagate. Root Host and catalog remain distinct.
Semantic source-byte/error cutoff is unchanged; no namespace added to byte carriers.

The existing Bzlmod path owner has a private explicit Root/Relative target and root
constructors. Invalid empty/dot/parent/absolute Relative inputs remain rejected;
Root is structurally distinct. Current original materialization request/instance/
generation root remains the operational owner. No marker/content probe or IO bypass.

Exact accepted source allowlist:
- app/slug_bzlmod_v2/src/source_preparation.rs (path target/driver and focused tests)
- app/slug_loading_v2/src/bzl_module.rs (one shared External constructor)
- app/slug_loading_v2/src/host_glob/mod.rs
- app/slug_loading_v2/src/host_glob/traversal.rs
- app/slug_loading_v2/src/host_glob/tests.rs
- app/slug_loading_v2/src/host_glob/traversal_tests.rs
- app/slug_loading_v2/src/canonical_repository_load_route_tests.rs (one mechanical External Arc caller)
Root updated matching Stage9 owner facts, receipt and scheduling.

Allocation: share original route/raw directory rather than new routing Strings;
borrow existing traversal PackagePath. Removed ignored absolute child allocation
and restored streaming raw-path diagnostics. Existing necessary by-value path/route/
materialization-Need request clones remain real costs; original Need contract is
full structural request equality, while observation and route Arcs retain strict
sharing checks. No new global interner/cache/retained map; existing scoped interners
remain. Compiled segment key24→40B, path key280B unchanged, source descriptor16B,
boundary scope264→256B, alignment8. Inline layout is separate from heap/Arc storage.

Validation: independent DESIGN/root-target delta before source; root freeze/static;
independent actual-source/allocation/prelaunch/FINAL ACCEPT. Fresh pinned nightly,
serial locked/offline no-run/current executable/exact preflight: Bzlmod5/5 and
Loading53/53 PASS including root/invalid-relative, full-request identity, raw0xE9,
symlink/missing/error, A/B/A current namespace/generation and exact observations.
Direct production Core/CLI check PASS. Focused Loading opt0 avoids its test codegen
preparation deadline; Core/native/direct dependent retain original Loadingopt1.
Compiler/fixture corrections and failed60s preparations have0 complete-gate credit.
An ineffective build/test graph split and rejected profile0 recovery premise were
corrected by reusing actually completed exact dependencies; no native profile change.

ONE native continuation:7.269902s,212184KiB=207.2109375MiB/256,63 stable pin brackets,
350 noninherited held-FD sources with exact before/after hashes, physical13861148B/
bracket27722296B, non-atomic collection. Internal WorkDeadline at source243,
decode55518938/write52795418; no denied path frame. This stops before prior245,
so actual/tmp frontier advancement is unproved. Only the observed prefix fits
memory; producer/time/bootstrap/actions/certificates/wholeworkload fit remain open.

All original caps preserved: native10s/RSS256MiB/AS512MiB/live+transfer16MiB/
decode+write64MiB/source256/registry128/object1MiB/admitted16MiB;
directory128entries/name1KiB/total16KiB; selectedwork7/default5/lifetime8/
cleanup3/FSIZE16MiB/log1/report4MiB. Preparation60s/RSSuncapped, no workload-fit credit.
User12s guideline is not a cap. No cap rise or duplicate native run. Source/helper
writers stopped during gates; format, diff and archive checks pass.

Observable advancement: repository glob ownership contract is implemented/proved;
natural producer frontier advancement remains unproved because of the earlier work
refusal. Select the next concrete measured producer work/allocation owner, preserving
these accepted semantics. Full goal remains ACTIVE.
