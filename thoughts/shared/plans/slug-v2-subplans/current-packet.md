# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-public-unsupported-cycle-approval-stop`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: explicit-user-approval stop; no active implementation or design authority
Evidence: accepted private support-gated preparation in `f2b626f2`; accepted
trusted nonregistry evaluator adapter in `c683c239`; and accepted private
preparation-consuming DICE/event owner in `3cf0e441`.

Both accepted private serial packets are complete. They remain callerless and
publish no product-visible unsupported-cycle result. The private support gate,
`Unsupported` capability, semantic values, and event ownership do not authorize
a build, query, one-shot, daemon, CLI, server, or other public consumer.

Stop pending explicit user approval of the product-visible unsupported-cycle
boundary. Do not infer approval from the private implementation, this manifest,
the canonical plan, prior oracle evidence, or ordinary `/goal resume` wording.
Without that approval, do not design, implement, export, activate, format, test,
or publish a public consumer or diagnostic, and do not select a representation,
message, exit status, command surface, retry behavior, or event-publication
contract for it.

If the user explicitly approves the limitation, the next action is a separate
read-only public-boundary design packet. That future design must enumerate every
build/query/one-shot/daemon activation path, preserve the distinction between a
Slug unsupported capability and a Bazel diagnostic, and freeze exact public
status, rendering, event ordering, retry, lifecycle, and parity scope before any
Rust or fixture authority. This paragraph is sequencing guidance only and does
not grant that approval or packet authority.

Stops: no Rust, Cargo, Bazel, fixture/oracle, public export/caller/activation,
`Unsupported` publication, diagnostic text, status/exit-code selection, command
or server change, event ownership change, contextual mapping, registry/MVS/JVM
transport, or speculative cap. The terminal condition is explicit user approval
or an explicit user-directed pivot to another bounded packet.
