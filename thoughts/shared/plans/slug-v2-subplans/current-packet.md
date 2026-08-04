# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-occurrence-closure-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only occurrence-preserving direct-local include-closure design
Evidence: accepted direct-local source/inspection in `e5e2c55d` and
`8aae11d6`; accepted route package horizon in `1d5edc7c`; existing root closure
acquisition shape; and pinned Bazel 9.2 nonroot include traversal. Add no oracle
unless the design proves one exact discriminator is absent.

Do not edit or format Rust. Design only the private DICE-owned acquisition of a
complete direct-local MODULE include closure. Start from the accepted
`DirectLocalModuleInspectionKey` and
`DirectLocalIncludePackageHorizonKey`; do not reconstruct their route, source,
inspection, package policy, or package lookup. Decide the smallest key/helper
boundary, exact implementation file allowlist, and measured production/test/
total caps before authorizing code.

The closure must advance breadth-first, one source horizon at a time. For each
ordered horizon, finish the accepted package preflight before requesting any
included fragment source. Only after package success derive the normalized
repository-relative fragment path from that occurrence's canonical package and
target and consume the accepted routed source owner. Deduplicate only identical
fragment dependency requests within that horizon in deterministic first-seen
order; preserve every occurrence, including duplicates, in the closure's
ordered execution sequence and diagnostic provenance.

Freeze exact source-result ordering. Request the complete first-seen fragment
group for one horizon, union every `SourcePreparationNeeds`, and then restore
raw label/`LogicalSpan` source order when choosing a missing/wrong-kind/source,
source-compute, UTF-8/parser, or unresolved result. Establish from pinned Bazel
source whether mixed terminal/Need selection mirrors package-horizon source
order or has a distinct rule. Do not infer it from async completion order.
Inspect every successfully acquired fragment once per unique dependency, but
replay its occurrence carrier wherever the raw include appeared.

The design must freeze the complete retained representation: route; root MODULE
logical identity/source/inspection as needed by the later evaluator; every
reachable fragment's route-derived logical identity, shared bytes, inspection,
raw label, and span; breadth-first horizons; and repeated execution
occurrences. Use existing `Arc<[u8]>`, `Arc<[T]>`, `CompactString`, compact
collections, `Dupe`, and `Allocative`. Do not retain package lookup results,
path-resolution internals, event batches, mutable evaluation state, or public
activation data.

Resolve repeated-include and cycle semantics from pinned Bazel 9.2 source.
Dependency deduplication may not become a visited-set truncation, and no new
finite cycle diagnostic is allowed without oracle/source authority. If exact
Bazel behavior requires unbounded recursion/nontermination that cannot be
represented by a bounded safe Rust owner, record the narrow unsupported
boundary or `REPLAN`; do not invent success, a cycle error, or occurrence
elision.

Specify exact key/value/error identity, display, source chains, complete-only
equality, and transient Need behavior. The closure owns no local event batch and
must not copy the package-policy child's routed-REPO batch. Fragment compilation
and module execution remain later owners; this packet may parse/inspect acquired
fragment bytes only to discover the next horizon, never execute directives.

Freeze lifecycle evidence for duplicate fragment dependencies versus repeated
ordered occurrences; breadth-first rather than depth-first discovery; package
success before fragment demand; both mixed terminal/Need directions; exact
multi-kind Need union; raw label/span errors; fragment add/edit/delete/recreate;
nested include add/remove/reorder; route A-to-B-to-A; root MODULE
absence/recreate; warm reuse and downstream pruning; and captured/uncaptured
child policy events with no closure-local data. Require structural stops proving
no evaluator, declarations, contextual mapping, registry/MVS/JVM transport,
public caller/export/activation, direct IO, lock across DICE, fixture, or oracle
enters the implementation packet.

Stops: no Rust, recursive module evaluation, empty-key declaration defaults or
validation, print/event ownership changes, contextual repository mappings,
registry resolution or transport, root-horizon semantic changes, public API,
fixture/oracle, direct filesystem IO, or speculative cap. Finish by recording
an accepted bounded implementation packet, a narrow unsupported boundary, or
`REPLAN` in the owner/canonical/manifest/routing records. Do not run Cargo or
Bazel.
