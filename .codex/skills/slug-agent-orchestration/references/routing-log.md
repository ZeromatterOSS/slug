# Recent Slug Packet Rollups

Read this only when a recent analogous packet may affect routing. The normal
summary is [routing-guide.md](./routing-guide.md).

Keep at most 20 terminal packet rows or 250 lines, whichever comes first.
Archive older rows by month as `routing-history-YYYY-MM.md`. Record one row
only when a packet reaches `ACCEPT`, `REPLAN`, or a genuine stop; fold audit,
implementation, review rounds, and corrections into that row.

| Date | Packet | Route | Wall time | Evidence | Review/rework | Result and next-use note |
|------|--------|-------|-----------|----------|---------------|--------------------------|
| 2026-07-23 | M3 Java `Pattern` substrate qualification | Terra medium audit/fixture, Sol low adjudication, root verification | not exposed | Pinned published `java_regex` 0.1.0 checksum/commit/license/MSRV/deps; `5e78abc1` adds a two-row Bazel/OpenJDK UTF-16 oracle that passed remote JDK 25 and embedded 25.0.2; candidate instead matched NUL, allocated 7/14 times, and silently maps fixed engine limits to non-match | Sol accepted the first boolean mismatch as the explicit terminal hard stop; root limited the retained artifact to the Java oracle and committed no candidate code/dependency | `REJECT` 0.1.0; reuse the surrogate/NUL gate for every future engine and rank non-regex `tests` then `visible` before proposing a V2-owned UTF-16 engine |
| 2026-07-23 | Orchestration hot-path and log-bound consolidation | Root + Sol-low independent review | not exposed | ~333 LOC normal read path; references resolve; diff check passes | Initial `REVISE`, one consolidation pass, final `ACCEPT` | Keep the skill as sole authority; cap the recent log and archive monthly |
