# Current Slug V2 Packet

Packet: `WP-4-5-7A-configured-toolchain-selection-live-allowlist-audit`

Milestone: M7A provider-independent configured eligibility and selection,
feeding ordinary M8 Stage 10.3 analysis.

Base: `c8064b106`.

Result: audit the accepted category-4 architecture against the live post-
target-platform tree and freeze the exact file/blob/line allowlist, bounded
caps, proof matrix and stop conditions for one implementation packet. This is
a zero-Rust packet.

## Accepted prerequisite and authority

Commit `c8064b106` owns the reusable configured target-platform fact, actual-
target alias projection, platform-specific exec configuration, configured
constraint matching, graph-derived complete builtin repository mapping and
exact `@bazel_tools//tools:host_platform` composition. Independent terminal
review returned `ACCEPT`.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` remains the sole behavior
authority. Authenticate and cite the registered-toolchain and registered-
execution-platform alias handling, target-setting filtering, target/execution
constraint checks, `use_target_platform_constraints`, mandatory/optional
multi-type selection, candidate ordering, no-common-platform behavior and exec-
configuration derivation already frozen in
`06-analysis-toolchains-and-actions.md`.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architectural and
optimization guidance only. It may guide producer-owned target-platform facts,
requested-versus-actual type separation and compact immutable selection rows.
No Zig representation, scheduler, identity, diagnostic or behavior is
authority or may be copied.

## Audit work

Read the live checkout from `c8064b106` and produce one implementation-ready
packet for category 4 of the frozen M7A sequence:

1. Map the existing loading-owned registered toolchain/execution-platform
   expansions, native toolchain declarations and ordered requirement slice;
   the configuration-owned target/exec platform projections and configured
   condition owner; and the analysis-owned configured node/alias and marker
   consumers.
2. Freeze one `ConfiguredToolchainResolutionKey` boundary over workspace,
   structural target configuration and ordered requirements. It must consume
   existing owners rather than source text, MODULE carriers, display labels or
   ruleset-specific state.
3. Preserve requested and post-alias toolchain-type identities, mandatory `OR`
   for aliases converging on one actual type, first-request order, one published
   row per request and explicit `None` for optional absence.
4. Freeze provider-independent filtering and selection as one complete
   category: target settings in the target configuration; target constraints
   against the configured target platform; execution constraints against each
   candidate; `use_target_platform_constraints`; declaration order within each
   candidate; greatest distinct requested-actual-type coverage; mandatory-
   absent versus no-common-platform errors; and platform-specific exec
   configuration.
5. Keep registered execution-platform alias convergence fail-closed at the
   existing duplicate-key boundary. The selected result retains only target
   platform, selected actual execution platform/exec configuration and
   requested/actual type-to-declaration identities.
6. Preserve the zero-requirement bypass and platform-only contexts. Bound the
   old singular marker as a post-selection bridge for exactly one mandatory,
   no-optional request; it cannot influence eligibility.
7. Inventory every changed file at its `c8064b106` blob and physical line
   count, classify production/proof/ledger additions, set per-file and aggregate
   caps, reuse existing fixtures, and name only discriminating evidence gaps.
8. Obtain independent Sol review of the frozen implementation packet before
   changing Rust.

The audit must inspect `docs/developers/dice.md` before freezing any DICE key or
cycle/locking contract, and must apply the Buck2 utility-reuse skill to every
retained selection row or compact collection choice.

## Compatibility

- **Exact:** the admitted Bazel 9.2 alias, setting/constraint eligibility,
  requested/actual grouping, mandatory/optional, candidate-order and selection
  behavior named above, once backed by pinned-source regressions or accepted
  oracle evidence.
- **Slug-native:** Rust type/layout choices, structural configuration and DICE
  identity bytes, compact scratch/retained collections, memory accounting and
  unproved diagnostic wording.
- **Unsupported/deferred:** implementation analysis, arbitrary
  `ToolchainInfo`/user-provider payloads, `ctx.toolchains`, provider/action
  semantics, broader exec-group behavior and exact Bazel configuration/output
  bytes. Categories 5 and 6 own those surfaces.

BCR-delivered Starlark owns every rule definition and control path, including
`cc_internal`. `cc_common` is only a demanding client of the future generic
host/provider ABI. The audit may not introduce or plan a Rust C++ rule engine,
a rules_rust-specific selector, or any builtin-specific semantic shortcut.

## Allowlist and stops

Only the canonical plan, this manifest and the relevant completion ledger may
change. No Rust, Cargo/BUILD, fixture, asset, lockfile, oracle, generated file,
CLI/server, repository materialization or public behavior change is authorized.

STOP and `REPLAN` if the live tree cannot support a bounded complete category-4
owner; if selection requires provider/evaluator values, raw source or display
labels; if a new parser, configuration store, graph or interner would be
required; if target-platform and toolchain-resolution ownership forms a DICE
cycle; if any lock would span a DICE computation; if the requested/actual or
mandatory/optional category must be narrowed; if ruleset/`cc_common`
specialization appears; if Zabel is treated as authority; or if the packet
cannot state exact baselines and caps before implementation.

## Validation

Run plan/canonical packet-name matching, `git diff --check`, allowlist and
baseline-blob audits, the archive-status script, and independent Sol review.
Do not run Cargo for this zero-Rust packet unless a read-only live-owner audit
needs an already-built test listing; no implementation proof is claimed here.
