# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-include-cycle-boundary-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only direct-local include-cycle support-boundary design
Evidence: accepted package horizon in `1d5edc7c`; pinned Bazel 9.2
`ModuleFileFunction.advanceHorizon`; accepted repeated-include and nested-order
oracles; the existing unvisited root-closure loop; and
`docs/developers/dice.md`. Add no oracle unless the boundary analysis proves an
exact discriminator is absent.

Do not edit or format Rust. The unrestricted occurrence-closure packet reached
`REPLAN`: a reachable self-cycle or multi-file include cycle keeps Bazel's
breadth-first horizon permanently nonempty, recompiling repeated occurrences
without a visited set and producing no finite cycle diagnostic. A complete
finite Rust closure value, invented Need, visited-set truncation, depth limit,
cycle error, or recursive DICE dependency would all violate accepted semantics
or DICE ownership.

Design only the narrow support/activation boundary. Decide whether Slug may
explicitly classify direct-local MODULE include cycles as unsupported under the
project's exact-Rust boundary rule, and if so where that classification lives
without pretending it is a Bazel diagnostic. Distinguish a planning/capability
boundary from a semantic closure-key result. Do not authorize an acyclic-only
closure key until the design proves how every public activation path establishes
or reports the supported domain without hanging DICE or silently changing
observable Bazel behavior.

Audit existing typed unsupported-feature owners and command publication paths,
but do not activate or modify them. Freeze the exact future owner of cycle
detection, whether detection is allowed before fragment dependency acquisition,
the retained provenance needed to identify a repeated active occurrence, and
the public diagnostic/status contract if the architecture permits one. A
supported boundary may not deduplicate ordinary repeated includes, reject a
finite duplicate DAG, alter breadth-first order, or add cycle identity to the
equality of acyclic closure values.

Also freeze the prerequisite ownership correction for any later acyclic closure
packet. The accepted `DirectLocalIncludePackageHorizonKey` is rooted in
`DirectLocalModuleInspectionKey` and cannot preflight nested fragment requests.
Any successor must extract one same-file private
`preflight_direct_local_include_package_horizon(ctx, route, requests)` helper
and make both the accepted key and closure owner consume it; it may not copy the
package parse/dedupe/Need/order logic or add a second route/policy/lookup graph.

If a bounded unsupported-cycle policy is architecturally acceptable, design the
smallest serial packets and exact file allowlists/caps for (1) the support
boundary and shared package-preflight refactor, then (2) acyclic breadth-first
fragment acquisition. Retain pinned acyclic behavior: package success before
fragment demand; horizon-local first-seen dependency dedupe; source-order mixed
terminal/Need selection; repeated occurrence compilation and execution;
next-horizon order; complete reachable compilation before execution; exact
multi-kind Need union; route/raw-label/span provenance; complete-only equality;
and no closure-local event batch.

If no non-deceptive bounded support boundary exists without explicit product
approval, record terminal unsupported status for direct-local include closure
activation and pivot the canonical plan to another bounded M1 gap. Do not
silently assume approval.

Stops: no Rust, Cargo, Bazel, fixture/oracle, finite cycle result, intentional
hang, DICE recursion, visited set, occurrence elision, fragment acquisition,
evaluator/default/validation/print change, contextual mappings, registry/MVS/
JVM transport, public activation, or speculative implementation cap. Finish by
recording an accepted bounded design sequence, terminal unsupported status, or
`REPLAN` in the owner/canonical/manifest/routing records.
