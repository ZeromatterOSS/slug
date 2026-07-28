# Current Slug V2 Packet

Packet: `WP-5-m1-build-activation`
Milestone: M1, one semantic build spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted preactivation Host closure gate, opaque consuming
publication boundary, source-aware command events, dormant typed build root,
and accepted query-first production activation
Validation tier: public production activation boundary

Implementation areas:

- dormant `BuildCommandRootKey` and the now-active shared
  retry/accept/publication seam in `slug_core_v2`;
- one-shot build adapter in `slug_cli_v2`;
- daemon build adapter and its existing filesystem observation metric in
  `slug_server_v2`; and
- the narrow existing build/core/CLI/server evidence needed to freeze an
  atomic non-executing activation.

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Implement the accepted `### Typed build atomic activation design` as one
atomic packet that:

1. makes the dormant typed build root the only semantic loading/analysis owner
   for both one-shot and
   daemon paths;
2. reuses the sole accepted retry/accept/publication owner and opaque envelope;
3. preserves daemon `invalidated_files` as metric-only and removes legacy
   snapshot values from the future semantic transaction; and
4. stops the typed DICE transaction at loading/analysis while preserving the
   already-authorized native REAPI projector outside DICE.

Add the real-driver regression first, then convert core, one-shot CLI, daemon,
and the existing REAPI projectors serially without an intermediate activation
state. Run only the focused tests and quiet checks frozen by the accepted
design plus one terminal independent review.

Add no Cargo change, execution owner, action semantics, REAPI behavior, JVM,
Java-bytecode, or Bazel delegation. The live adapters already execute through
native REAPI when requested; preserve that downstream path without moving it
into DICE or the retry driver. Stop if the slice would mix legacy snapshots
into the typed transaction, expose raw events/terminal internals, publish
retry output, change metric ownership, or imply that loading/analysis success
is execution success.
