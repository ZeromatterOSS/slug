# Current Slug V2 Packet

Packet: `WP-6-7A-host-instantiated-module-extension-repositories-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `c1c8e1d8`

## Goal and design authority

Freeze the smallest crate-internal surface that lets the validation sibling
name the accepted observed instantiation Key Value and borrowed carrier. Decide
the opaque wrapper projection required by Rust effective visibility. Do not
edit Rust, activate validation or expose private instantiation terminals.

Design write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, net <=40;
- this manifest, net <=180;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  net <=220; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`, net <=30.

Aggregate net growth is <=470. Every Rust file, test, fixture, oracle,
Cargo/BUILD target, public API/export and caller is read-only. Schedule exactly
one implementation successor on ACCEPT or one narrower prerequisite/REPLAN.

## Live visibility and ownership facts

`c1c8e1d8` accepts a callerless private observed instantiation owner. In
`module_extension_repository_instantiation.rs`, the observed key and constructor
are private at lines 157-169, the carrier and its accessors are private at
202-216, `result()` exposes the private `InstantiatedRepositoriesResult` alias,
and the private outer at 219 directly exposes its Pure child variant.

Validation at `module_extension_repository_validation.rs:208` is the sole
production consumer of the legacy instantiation key and the next semantic
owner. It alone validates receipt/request joins, imports before overrides,
override-backed imports, must-exist and injection polarity, and retains the
public flattened generated-spec certificate. It stores no event batch.

The public validation key has exactly one production consumer,
`HostGeneratedRepositoryDefinitionKey` at
`generated_repository_definition.rs:168`. Canonical publication checks the
parallel selected definition first at 428 and the generated definition at 467;
root apparent mapping separately consumes root mapping at 678. Selected
definition and root mapping derive from accepted routes and extension mappings,
not validation. These later/parallel owners cannot replace carrier visibility.

## Decision to freeze

Decide exactly one minimal crate-internal surface:

1. the existing observed instantiation key and `new` constructor;
2. the existing carrier with borrowed concrete
   `Arc<Result<HostInstantiatedModuleExtensionRepositories,
   HostInstantiatedModuleExtensionRepositoriesError>>` and
   `PathObservationEpoch` accessors; and
3. one opaque nominal outer whose fields and terminal variants remain private.

The design must determine whether to rename the current private outer to an
inner enum and project one field-private `pub(crate)` wrapper only at the
observed Key boundary, following the accepted prepared and pure precedents.
Keep `InstantiatedRepositoriesResult` private. Preserve all key/carrier derives,
workspace identity, `observed-{legacy Display}`, Complete-only equality and
validity, Result Arc identity and epoch contents. Add no alias, variant/field
accessor, conversion trait, second key/carrier/adapter, public/lib reexport or
caller.

Freeze one sibling compile-only proof in the existing validation test module.
It may construct the key only to prove constructor and unchanged Display, then
type-check the exact associated Value, carrier, opaque outer and concrete
borrowed accessors through one nonexecuted local function and explicit function
pointer. It must not construct a carrier/outer, inspect the outer, compute any
key, add a driver, observe dependencies/events or activate validation.

## Prospective implementation boundary

Prospective Rust authority is exactly:

- production
  `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  baseline 2,049 physical lines with tests at 641; and
- test-only
  `app/slug_loading_v2/src/module_extension_repository_validation.rs`, baseline
  1,156 physical lines with tests at 332.

Caps are <=60 production, <=50 proof, <=110 aggregate semantic and physical
<=2,110/1,210. Every changed helper/test stays below 100. The large
instantiation file remains cohesive because it owns the private driver and
representation; the validation sibling is the sole future consumer and natural
visibility witness. No split or hot-path measurement is warranted for this
visibility-only step.

Reuse the accepted instantiation identity, family, terminal, event, lifecycle,
cancellation and nonactivation proof; add no oracle. The design must freeze
serial validation of the named sibling smoke, focused `observed_instantiation_`,
protected validation `real_validation_`, full `cargo test -p slug_loading_v2`,
direct `cargo check -p slug_core_v2`, formatting and `git diff --check`.

## Compatibility and terminal

Accepted instantiation/validation values, errors, order, import/override
polarity, `RepoSpec` iteration, DICE equality and pure-owned events remain exact
Bazel 9 compatibility. The crate-internal carrier handoff is Slug-native.
Validation observation, generated/public/root-mapping/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

ACCEPT schedules exactly one carrier-visibility implementation, then returns
to a docs-only `HostValidatedModuleExtensionRepositoriesKey` observation-owner
design. STOP a public/lib reexport, exposed alias/field/variant/inspector,
second key/carrier/adapter, caller or compute activation, validation semantic
change, event/equality/retention drift, third file, fixture/oracle work, cap
waiver, upper/parallel activation, milestone closure, M8/M7B or exact identity
work. REPLAN before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

`c1c8e1d8` proves the accepted observed instantiation carrier is the only
missing input to validation. Generated publication is later, while selected
definition and root mapping are parallel.
