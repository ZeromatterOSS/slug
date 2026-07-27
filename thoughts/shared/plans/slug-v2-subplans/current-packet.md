# Current Slug V2 Packet

Packet: `WP-5-m1-source-aware-command-events`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted source-aware command-event design, opaque terminal/event
ownership, and Bazel 9.2 source plus oracle stderr shape
Validation tier: public cross-crate representation and publication

Implementation areas:

- retained Starlark codemap, parser, compiler/bytecode, print API, and exports;
- `slug_events_v2` plus MODULE, REPO, loading, and analysis producers; and
- the opaque `slug_core_v2` command publication boundary.

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Implement the exact accepted `### Source-aware command event design` contract
in the owner plan, serially in two locally reviewable phases with one terminal
independent review:

1. retain the actual call `(` token, expose Bazel-shaped UTF-16
   `PrintLocation`, share apparent filenames per codemap with `Arc<str>`, and
   convert every captured print producer without changing uncaptured REPO
   output; then
2. consume `CommandOutput<T>` into primitive streams with exact DEBUG,
   diagnostic, order, multiline, and platform-line-separator behavior.

Add the narrow retained-Starlark regression first. Run only the focused owner
and producer tests, quiet named direct compile dependents, GNU-Windows no-run,
format/diff/archive/scope/no-Cargo guards, and unchanged activation-blocker
scans frozen by the accepted design.

Stop on source reconstruction, scalar/byte columns, repeated filename
allocation, event clones, a second output owner, DICE/acceptance semantic
change, or any production caller/activation. Add no CLI/server behavior,
execution, REAPI, JVM, Java-bytecode, or Bazel delegation.

After acceptance, design the query-first atomic vertical activation; do not
interpose build execution work.
