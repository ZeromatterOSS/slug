# Current Slug V2 Packet

Packet: `WP-5-m1-build-activation-design`
Milestone: M1, one semantic build spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted preactivation Host closure gate, opaque consuming
publication boundary, source-aware command events, dormant typed build root,
and accepted query-first production activation
Validation tier: design-only public production activation boundary

Design areas:

- dormant `BuildCommandRootKey` and the now-active shared
  retry/accept/publication seam in `slug_core_v2`;
- one-shot build adapter in `slug_cli_v2`;
- daemon build adapter and its existing filesystem observation metric in
  `slug_server_v2`; and
- the narrow existing build/core/CLI/server evidence needed to freeze an
  atomic non-executing activation.

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Design one atomic typed-build activation that:

1. makes the dormant typed build root the only semantic loading/analysis owner
   for both one-shot and
   daemon paths;
2. reuses the sole accepted retry/accept/publication owner and opaque envelope;
3. preserves daemon `invalidated_files` as metric-only and removes legacy
   snapshot values from the future semantic transaction; and
4. stops at loading/analysis result publication without action execution.

This packet is design-only. Read the live build adapters and dormant root,
freeze exact files, API shape, output/error equivalence, tests, scans, and stop
gates, then obtain one independent terminal review. Do not edit Rust.

Add no Cargo change, execution, action materialization, REAPI, JVM,
Java-bytecode, or Bazel delegation. Stop if the slice would mix legacy
snapshots into the typed transaction, expose raw events/terminal internals,
publish retry output, change metric ownership, or imply that loading/analysis
success is execution success.
