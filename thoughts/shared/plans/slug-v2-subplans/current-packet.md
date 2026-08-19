# Current Slug V2 Packet

Packet: `WP-6-7A-root-generated-repository-observation-frontier-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and evidence base: `b7390392`
Accepted rules_rust evidence: `b7390392`
Result: formal REPLAN from the external rules_rust analysis owner to the
uniquely smaller root generated-repository observation frontier.

## Why this prerequisite is required

The accepted Bazel 9.2/rules_rust 0.73 fixture proves the exact generated
registration membership, selected canonical toolchain implementation,
configured provider edges and Rustc/runfiles owner relationships. It does not
make those extension-generated repositories loadable through Slug's observed
families.

Live ownership shows that root apparent repository mapping still crosses
`HostSelectedExtensionMappingsKey`, `HostSelectedModuleRoutesKey`,
`HostSelectedModuleGraphKey` and `RootModuleFilesKey` without a complete
`PathObservationEpoch`. The accepted `RepositoryPackageLoadObservationKey`
starts only from a `RootRepositoryRoute`, whose current observed route owner
covers direct-local and builtin repositories, not an extension-generated
apparent repository such as `@rust_toolchains` or its canonical
`@@rules_rust++rust+rust_toolchains` identity. The private core generated-
repository definition/apparent-route keys sit above analysis and cannot be
called from the lower bzlmod/loading graph without dependency inversion.

A mapping-only sibling would therefore be incomplete: analysis would still
cross carrierless canonical definition, route/source and generated package
load edges. Duplicating those projections in analysis would create a second
repository owner and retain parallel route state. The uniquely smaller packet
is a cohesive lower frontier that carries the already-owned mapping through
the generated repository package boundary; the external rules_rust analysis
owner resumes only after that frontier is accepted.

## Design authority and caps

This packet is docs-only. Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Docs caps are <=40 canonical, <=220 current, <=220 Stage 6 and <=30 routing
net lines, <=510 aggregate. Rust, tests, fixtures, oracle generation,
Cargo/BUILD metadata and Stage 10 are read-only while the design is active.

The read-only design audit must inspect exactly this candidate owner set:

1. `app/slug_bzlmod_v2/src/module_eval.rs`;
2. `app/slug_bzlmod_v2/src/selected_graph.rs`;
3. `app/slug_bzlmod_v2/src/registry_dice.rs`;
4. `app/slug_bzlmod_v2/src/selected_repo_spec.rs`;
5. `app/slug_bzlmod_v2/src/lib.rs`;
6. `app/slug_loading_v2/src/module_extension_repository_validation.rs`;
7. `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`;
8. `app/slug_loading_v2/src/module_extension.rs`;
9. `app/slug_loading_v2/src/bzl_module.rs`;
10. `app/slug_loading_v2/src/lib.rs`; and
11. new `app/slug_loading_v2/tests/root_generated_repository_observation.rs`.

The ten existing files total 26,768 physical lines. The design must either
freeze the exact necessary subset/seams with measured per-file caps or record
formal REPLAN if another natural owner/file is required. The provisional whole-
candidate envelope is <=1,200 production, <=900 tests and <=2,100 aggregate
semantic lines, with combined physical size <=29,200. These are design bounds,
not implementation authority; no Rust becomes writable until independent
design ACCEPT and a separate implementation packet.

## Frozen design questions and owner boundary

Trace the exact root apparent extension-generated repository path through the
selected module graph, prepared extension inputs, pure extension invocation and
local event batch, repository instantiation/validation, generated canonical
repository definition, apparent route, source/materialization and package load.
Freeze the smallest structural observed sibling set that makes the complete path
reusable by lower loading and later analysis without moving private core command
ownership downward or duplicating extension evaluation/event ownership.

The design must establish one matching Legacy/Observed driver at every shared
projection boundary. Legacy callers select only existing keys and preserve
their exact values, errors, order and events. Observed callers select only
observed siblings. A structural key/carrier may be doc-hidden and re-exported
only where the later analysis crate must consume it; no public command or
Stage 10 caller is activated here.

Each observed DICE sibling retains exactly one natural local semantic
`Arc<Result<...>>` plus one compact `PathObservationEpoch`, with `Dupe` and
`Allocative`. The top generated-package carrier may compose lower carriers but
must not retain any child carrier Arc, outcome map, selected graph, route map,
frontier, scratch vector, event buffer or prefix snapshot. Existing compact
`SmallMap`/Arc-backed repository storage remains the sole retained semantic
storage; do not add a cache, store, interner, lock, task, direct Host read,
revision or certificate.

Freeze exact construction/access surfaces for:

- the selected root apparent-to-canonical repository mapping and generated
  canonical definition;
- the corresponding generated route/source projection, including registry and
  module-extension repositories without weakening direct-local/builtin paths;
- the generated repository package load consumed by later rules_rust analysis;
  and
- any narrowly necessary cross-crate carrier/accessor export.

Do not require `ConfiguredNodeAnalysisObservationKey` or another analysis key
to reconstruct repository mapping, route or package semantics. Do not move the
private core root-routing keys into bzlmod merely for reuse; freeze a V2-owned
lower projection that the core adapter can later consume without circular
dependencies.

## Epoch, terminal and event algebra

For every observed projection, compute children in the exact existing semantic
order. Merge each Complete child epoch left-first before semantic inspection.
Equal duplicate demands retain the first exact Result Arc; conflicting values
or operation mismatches remain typed `ObservedPathFrontierError`. DICE compute
failures keep their existing semantic error class and the prefix reached before
the unavailable child.

Sequential parents return the first Need or typed outer immediately with no
carrier and no later activation. Joined selected-graph/definition batches must
scan the complete deterministic input order and decide first typed outer or
epoch-union error, then combined compatible Need, then first semantic terminal,
then ordered success. REPLAN rather than invent a new error if existing Need
kinds cannot union. Semantic terminals retain exactly the decisive merged
prefix; success retains the full reached epoch.

Need is invalid and self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus the complete
epoch. Stable equality must preserve the original shared Result Arcs.

The frontier adds no event owner. Root-module, registry download,
materialization, extension evaluation, repository source and package-load
children remain sole owners of their matching local batches. Parent composition
is eventless; Need, outer and cancellation publish nothing; warm reuse suppresses
replay. No analysis, toolchain, action, aquery, REAPI or public batch is activated.

## Required proof and compatibility

The frozen implementation proof must discriminate:

- exact rules_rust apparent-to-canonical mapping and the selected generated
  `rust_toolchain` implementation from `b7390392`;
- root/registry/module-extension definition, route, source and generated BUILD
  package values and semantic errors with exact legacy parity;
- Need, typed outer and semantic terminals at every child position, later-child
  suppression, joined full-batch precedence and compatible Need union;
- exact epoch membership/order and per-demand `Arc::ptr_eq`, stable equal-
  duplicate first Arc, conflict and operation mismatch;
- observed-to-zero-legacy and legacy-to-zero-observed family activation,
  including concurrent roots without mixed edges;
- exact child-owned event order/text, parent eventlessness, warm suppression,
  real poll/drop cancellation and same-DICE successor recovery;
- root mapping, registry source and extension-generated repository/package
  create/edit/delete/recreate and A/B/A restoration; and
- compact retained state and cleanup scans with no extra collection, cache,
  store, interner, lock, task or direct Host read.

Exact: accepted Bazel 9.2 mapping/package values and errors, rules_rust 0.73
registration/selected implementation, legacy repository semantics and child
event order. Slug-native: structural observed siblings, carrier/typed-outer
algebra, compact epochs and collision-safe identities. Unsupported/deferred:
full rules_rust Starlark/provider analysis, sysroot input closure, Rust action
execution/REAPI/materialization, public named groups, M7B run/test/BEP breadth,
M8 bootstrap and exact Bazel identity bytes.

## STOP, result and successor

STOP on Rust/tests/fixtures/oracles, core dependency inversion, a mapping-only
carrier, duplicate route/package ownership, new retained state, direct Host
reads, event-family drift, analysis/toolchain/action activation, public API
change, any unaudited Rust owner/file, Stage 10, M7A closure, M8/M7B/M9, cap
excess or a second successor.

Return exactly one implementation-ready lower-frontier design or formal
REPLAN. After independent design ACCEPT, schedule exactly one bounded
`WP-6-7A-root-generated-repository-observation-frontier-implementation`.
After implementation ACCEPT, return immediately to the docs-only
`WP-6-7A-external-rules-rust-toolchain-owner-design`; do not activate it in
the prerequisite packet.
