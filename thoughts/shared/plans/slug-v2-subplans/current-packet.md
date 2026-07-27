# Current Slug V2 Packet

Packet: `WP-5-m1-query-first-activation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted preactivation Host closure gate, opaque consuming
publication boundary, source-aware command events, and dormant typed query root
Validation tier: design-only production activation boundary

Design areas:

- dormant typed query root and retry/accept/publication seams in
  `slug_core_v2`;
- one-shot query adapter in `slug_cli_v2`;
- daemon query adapter and metric-only filesystem observation in
  `slug_server_v2`; and
- the narrow existing query/core/CLI/server tests needed to prove equivalence.

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Audit the live typed query and both activated legacy query call chains. Freeze
one atomic implementation packet that:

1. makes the typed root the only semantic query owner for both one-shot and
   daemon paths;
2. publishes its accepted envelope exactly once into exit code/stdout/stderr;
3. removes both activated legacy query adapter matches atomically while
   preserving daemon `invalidated_files` observation as metric-only; and
4. proves one-shot/daemon output and error equivalence for the smallest useful
   query surface before expanding function or format breadth.

This packet is design-only. Add no Rust, Cargo change, build activation,
execution, REAPI, JVM, Java-bytecode, or Bazel delegation. Stop if the slice
would mix legacy snapshots into the typed transaction, expose raw
events/terminal internals, publish retry output, change metric ownership, or
require query-language breadth unrelated to the first vertical operation.
