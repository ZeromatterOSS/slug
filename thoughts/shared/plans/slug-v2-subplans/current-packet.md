# Current Slug V2 Packet

Packet: `WP-5-m1-external-bzl-macro-query-oracle-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: design worker
Evidence: accepted dormant external Bzl owner `0463cb17`, accepted external
query package identity `845e89b7`, accepted ad-hoc Bazel 9.2 external
load/missing/cycle evidence, and the protected 17-row
`module-local-override` fixture.

Design only the smallest Bazel 9.2 oracle that proves a same-package external
`.bzl` macro can create one native `filegroup` and that query reports its Bzl
provenance. Read `AGENTS.md`, the orchestration skill design-worker reference,
the activation-design REPLAN in the Stage 5 owner plan, the oracle harness
schema, the current fixture and expected record, and one analogous retained or
mutation fixture before writing the contract.

Preserve all 17 existing commands and
`workspace/dep/BUILD.bazel` byte-for-byte. The protected checkpoint metric is
598 lines: expected record 460, fixture 123, dependency BUILD 15. Use only a
new `workspace/dep/macro/BUILD.bazel` and
`workspace/dep/macro/defs.bzl`; do not copy a MODULE subtree. The BUILD must
load the same-package defs file and invoke its macro, and the macro must call
`native.filegroup` so no direct declaration can satisfy the evidence.

Freeze exactly three nonduplicative queries against
`@dep//macro:macro_files`: `--output=label_kind`, `loadfiles()`, and
`buildfiles()`. Specify exact stdout/order, empty/normalized stderr policy,
fresh-root generation plus distinct-root replay, Bazel 9.2 invocation, row and
asset caps, protected-row replay, and whether daemon epoch observation adds a
discriminator or only noise. Do not add lifecycle/missing/cycle rows unless the
accepted evidence audit proves they distinguish the macro seam; those
behaviors already have accepted direct probes and belong in Rust/CLI lifecycle
tests after activation.

Do not edit Rust, Cargo, fixtures, generated records, or oracle tooling in this
design packet. Do not authorize cross-package/repository loads, globs, target
kinds beyond ExportedFile/Filegroup, or any query/loading activation. Obtain
one independent latest-text fixture/harness review. At `ACCEPT`, append the
exact oracle implementation contract and advance this manifest to that
oracle-only packet; at `REPLAN`, record the smallest missing harness or source
prerequisite.
