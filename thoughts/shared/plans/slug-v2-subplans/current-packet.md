# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-nonregistry-empty-key-evaluation-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: read-only nonregistry include-closure and evaluation ownership design
Evidence: accepted direct `local_path_override` route,
`HostRootModuleFileKey`, `HostRepositorySourceFileKey`, and the accepted
direct-local handoff design/cap correction in the owner plan.

Do not edit Rust. Design one private, callerless direct-local nonregistry
evaluation owner above the accepted `DirectLocalModuleInspectionKey`. Its exact
expected identity is `NonrootModuleKey { name: route.module_name(), version:
"" }`: Bazel rewrites nonregistry override requests to empty version before
discovery, validates the declared name, and skips declared-version equality
only for that empty key.

Freeze the DICE dependency order for acquiring the complete transitive
nonregistry include closure through the same retained route and existing Host
source owner. All reachable files must be inspected before execution, with
exact Need/error/cycle/missing/unreachable semantics and logical identities.
Then define the smallest evaluator wrapper/value/error/event boundary that can
consume the empty expected key plus complete supplied closure. Preserve the
declared name/version separately in evaluator output.

Decide whether closure acquisition and evaluation require serial packets, the
exact file allowlist and caps, event capture/replay ownership, complete-only
equality and transient Need behavior, and the lifecycle matrix. Reuse the
accepted local-override version-selection oracle; add no oracle unless an
observable include/evaluation discriminator is demonstrably absent.

Stop with **REPLAN** on a root-requested or file-declared version used as the
expected key, nonempty nonregistry selected version, root mapping used as final
nonroot context, contextual mapping construction, registry/JVM transport,
legacy `ModuleFile::parse`/`resolve_*`/`ModuleSourcePreparationKey`, direct
filesystem IO, a parallel source graph, public activation/export, or evaluation
before the full owned include closure. Do not edit/format Rust, run Cargo/Bazel,
or change an oracle in this design packet.
