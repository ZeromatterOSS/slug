# Current Slug V2 Packet

Packet: `WP-5-m1-source-aware-command-event-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted opaque terminal/event ownership plus the Bazel 9.2 terminal
event oracle's source-aware stderr shape
Validation tier: design/source/API feasibility audit

Design files:

- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`

Terminal scheduling updates may also change this manifest, the owner plan, and
canonical Live Status.

Result: cite the exact Bazel 9.2 `DEBUG: <path>:<line>:<column>: <text>`
Starlark print shape and audit the live Starlark `PrintHandler` call path,
event representation, root-MODULE/loading/analysis producers, and opaque
publication seam. Freeze the smallest representation/API change that captures
real source spans rather than reconstructing them from fixture text.

Specify exact future files, producer ownership, path/line/column identity,
multi-line formatting, event equality/clone cost, warm nonreplay,
success/eligible-error ordering, consuming publication of
`CommandOutput<T>`, tests, validation, and stop gates. Preserve query-first
activation as the next vertical route.

Add no Rust, production caller, CLI/server behavior, activation, execution,
REAPI, JVM, Java-bytecode, or Bazel delegation. Obtain one terminal design
review.
