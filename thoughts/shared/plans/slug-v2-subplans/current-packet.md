# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-query-routing-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted Host source-preparation/materialization owners, repository
mapping foundations, typed query/build production activation, opaque
publication, and source-aware command events
Validation tier: design-only external repository command boundary

Design areas:

- Bazel 9 apparent-to-canonical repository mapping and root-module graph
  ownership in `slug_bzlmod_v2`;
- external repository materialization and Host loading boundaries;
- the active typed query root and retained retry/publication path; and
- the smallest oracle/core/CLI/server evidence for one external query label.

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Design the smallest observable external-repository query slice that:

1. resolves one apparent external label through the command's Bazel 9
   repository mapping;
2. prepares/materializes the repository through accepted native owners before
   loading it;
3. keeps the typed root as the sole semantic query owner in one-shot and
   daemon modes; and
4. publishes exact output/events without eager workspace snapshots or
   delegation.

This packet is design-only. Inspect only the live mapping, materialization,
typed-query, and relevant oracle seams. Freeze exact files, API/identity,
retry/error/event behavior, focused evidence, and stop gates, then obtain one
independent terminal review. Do not edit Rust or fixtures.

Add no Cargo change, build execution, new materializer, REAPI behavior, JVM,
Java bytecode, or Bazel delegation. Stop if the slice would bypass repository
mapping, fabricate `@bazel_tools`, mix legacy snapshots into the transaction,
or require general discovery breadth beyond the first external query.
