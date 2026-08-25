# Current Slug V2 Packet

Packet: `WP-6-7A-generated-package-selected-extension-demand-owner-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design base: retained bridge/input candidate over `4d83a829`, accepted Bazel
9.2 fixture evidence `6fd78a21`, and terminal aggregation-order REPLAN

Result: design the smallest producer-authenticated demand for one selected
module extension before any definition load, evaluation, instantiation or
validation, so an unrelated unsupported extension cannot poison an admitted
generated repository.

## Read-only authority and retained state

This is docs-only. Write only the canonical plan, this manifest, the Stage 6
owner and the routing log. Rust, tests, fixtures, oracles, Cargo/BUILD and
`../zabel` are read-only.

Retain the nine-file candidate exactly. Its current discriminators are:

- the facade proof and all 533 Bzlmod unit tests plus integrations/docs pass;
- the protected external-build lifecycle and generated-route/bridge focused
  proofs pass;
- core is at its exact accepted 278/279 query baseline and runtime is at its
  exact accepted 12/13 `PathObservationEpochKey` baseline;
- cumulative accounting is +566 production/+508 proof/+1,074 aggregate from
  `4d83a829`; and
- the rebuilt `module-extension-use-repo` fixture reaches the generated route
  but fails because unrelated selected `@bazel_tools` xcode extension usage is
  rejected before the demanded root extension loads.

Freeze current SHA-256 values:

- `host_module.rs` `185ec7685abd51851c570762e393df1d59892596854cf6c826603d00a2703c39`;
- `registry_dice.rs` `c736f60743709040ca1f7b327bd02f4ac956c5576b02db3375a106df6c9f8933`;
- `selected_repo_spec.rs` `25a0d0855ed83bc58942b02ec7daa1fcc78b50e604695a60b0e148b1edf24cad`;
- `dice.rs` `c10651ec7a5777dbed5db78df57a6d50b5c50f098191a44fe177379a48e8f914`;
- `generated_repository_definition.rs` `8166e0c83a0f86e50d251d25b649be18cfd37020434f163a1e06dde723ba27ad`;
- `runtime/mod.rs` `204fd7510b216b9794b6ce646c29ab30dcf2b453bb42c2b402a76da6f41ac651`;
- `generated_package_route.rs` `27e6ee70e2b95c3b1e48bb6fcca8795fd2ba763cb6b0867ffd7fc9ba87f90818`;
- `root_apparent_repository_definition.rs` `a1cf060405c4a5d7be26acc4b23dda542c7c0fad20325fd6fa4b7369f8dc1f3a`;
  and
- `build_command_tests.rs` `cf96c012f4de303b9b0b0d94d345ecfbc395dc1a81427ea32399503474a067f1`.

The private Host registry owner remains frozen at
`a253dba09c0c10e51525c268402cb237961130a867e808d0a768c5b7b15feac7`.
The accepted fixture workspace and evidence remain byte-identical.

## Required design audit

Trace and design one selected-extension demand chain:

1. In Bzlmod, derive a producer-owned demand seed from the requested canonical
   repository and the recorded selected extension usage/import/unique-name
   facts. Match typed identities; never parse a `+`-delimited canonical name.
2. Preserve the authenticated selected owner in DICE identity/equality and
   produce exactly one extension definition-load request. A directly demanded
   unsupported owner remains a typed terminal, while an unrelated unsupported
   owner is not evaluated.
3. Reuse or extract single-request finishers through Bzl loading, pure
   invocation, repository instantiation and validation. Evaluate every usage,
   tag, import, override and generated output belonging to that selected owner,
   not merely the named repository output.
4. Carry observed Need/outer/epoch semantics only along that selected chain,
   preserve child event ownership and exact left-first epoch composition, then
   select the requested generated repository from the owner-authenticated
   validated result.
5. Keep the existing workspace-wide load/invocation/instantiation/validation
   keys semantically complete. Do not turn them into filtered or best-effort
   carriers and do not suppress xcode at a downstream validator.

The design must name exact Rust authority, visibility-only exports, key/value
shapes, error algebra, event/epoch ownership, retained state, proof matrix,
line/physical caps, hashes and serial validation for one implementation packet.
Prefer one per-selected-extension semantic spine over adapter keys or copied
aggregate logic. Read `docs/developers/dice.md` before specifying ownership or
locking.

## Architectural and oracle basis

Bazel 9.2 is exact authority:

- `RepoDefinitionFunction` first maps a canonical repository prefix to one
  `ModuleExtensionId`, then requests `SingleExtensionValue.key(id)`;
- `SingleExtensionUsagesFunction` projects graph facts for only that id;
- `SingleExtensionFunction` evaluates and validates only that id's complete
  usages/imports/overrides; and
- `BazelDepGraphValue#getRepositoryMapping` derives imported canonical names
  from typed extension identities and unique names.

Use `../zabel` as architectural guidance, after respecting its `AGENTS.md`:

- `session_selected_extension_graph_demand_seeds.zig` derives canonical-repo
  demand seeds and owner indexes before evaluation;
- `session_selected_graph_extensions_root_direct_routes.zig` keeps repository
  access demandable without evaluating every selected extension; and
- `session_selected_extension_source_execution.zig` executes one selected
  owner index.

Adapt the ownership lessons to Rust DICE; copy no Zig code or representation
and add no Zabel dependency.

## Compatibility, proof and stops

Requested imported generated-repository behavior is exact Bazel 9. The demand
seed/key representation and private bridge carriers are Slug-native. Execution
of xcode and other unadmitted non-root extensions remains unsupported/deferred;
their mere presence must not poison an unrelated exact demand. Configuration/
output bytes, query/public publication, explicit module mirrors and other
platforms remain deferred.

The eventual proof must include at least: requested root extension plus an
unrelated unsupported non-root extension; two root extensions where the
unrequested one fails; direct demand of an unsupported owner; exact generated
repo selection/missing/duplicate behavior; Legacy/Observed parity, epochs,
events, Need/outer/cancellation/warm A/B/A; protected unknown fallback; full
Bzlmod/core baselines; and rebuilt accepted fixture success.

Documentation net caps are <=50 canonical, <=220 current, <=240 owner and <=30
routing, <=540 aggregate. Reach one independently reviewed implementation
design or formal REPLAN. STOP Rust edits, downstream filtering, canonical-name
string parsing, global-carrier weakening, duplicated semantic pipelines,
fixture/test weakening, Java/JVM work, milestone closure, M8/M7B or exact
identity bytes. M7 remains partial and M7A -> M8 -> M7B remains.
