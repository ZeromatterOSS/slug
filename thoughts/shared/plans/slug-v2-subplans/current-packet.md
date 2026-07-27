# Current Slug V2 Packet

Packet: `WP-5-m1-private-opaque-terminal-envelope`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted corrected private opaque terminal-envelope design over the
dormant shared retry driver
Validation tier: private cross-module Rust ownership boundary

Implementation files:

- `app/slug_core_v2/src/runtime/events.rs`
- `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_core_v2/src/runtime/mod.rs`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: add public `#[must_use]` opaque `AcceptedCommand<T>`,
`TerminalOutput`, and `CommandOutput<T>` with private construction/storage.
Refactor dormant acceptance to return only the envelope. Its sole public
projection borrows `&T`, retains the original terminal and unrendered selected
event buffer, and exposes no parts.

Do not render events: current Starlark print capture lacks Bazel source
locations. Prove terminal identity, success/error/empty projection, exact
single projection, retry-only exclusion, retained terminal events, and no
envelope on every failure seam. Validate focused/full core, direct
query/loading/analysis, GNU-Windows, formatting, diff, exact three-file scope,
no Cargo/caller, and unchanged six activation-blocker matches.

Add no production caller, raw event/buffer API, renderer, Cargo/dependency
change, CLI/server behavior, activation, snapshot retirement, execution,
REAPI, JVM, Java-bytecode, or Bazel delegation. Obtain one terminal
implementation review.
