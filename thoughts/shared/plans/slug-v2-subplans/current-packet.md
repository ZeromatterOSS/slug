# Current Slug V2 Packet

Packet: `WP-6-7A-host-pure-module-extension-invocations-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `9bab80b3`

## Goal and decision boundary

Design only the smallest crate-internal surface that lets the sibling
instantiation module name the accepted observed pure-invocation child. Freeze
the exact key/constructor, carrier accessors, opaque outer projection,
two-file compile proof and implementation caps. Do not implement or activate
instantiation, validation or any upper/public owner.

Docs write authority is exactly the canonical plan, this manifest, Stage 6 and
the routing log. Net caps are <=40/<=180/<=220/<=30 respectively and <=470
aggregate. Rust, tests, fixtures, oracles, Cargo/BUILD, APIs, exports and
callers are read-only. This design may authorize exactly one implementation
successor.

## Learned frontier and ownership

Accepted commit `9bab80b3` adds the private observed pure owner at +301/-171
production and +600/-127 proof, 2,232 physical lines. Its production types are
at `module_extension.rs:108-145`: private
`HostPureModuleExtensionInvocationsObservationKey`, private
`ObservedHostPureModuleExtensionInvocations`, private
`HostPureModuleExtensionInvocationsObservationError`, and a carrier accessor
returning private alias `PureInvocationsResult`.

The legacy pure key has exactly one production consumer:
`HostInstantiatedModuleExtensionRepositoriesKey` computes it at
`module_extension_repository_instantiation.rs:191`. The observed sibling has
zero production consumers. Instantiation cannot name the private observed Key
Value across the sibling-module boundary, so it cannot begin a lawful observed
driver. No semantic/evidence prerequisite is missing beneath pure.

Instantiation is the next natural semantic owner. It alone transforms ordered
pure repository-rule receipts into generated repositories by preserving:

1. prepared/invoked count and exact request-order joins;
2. base namespace followed by generated-call entries and declared overrides;
3. generated canonical names and ordered repository-rule calls;
4. schema lookup, supplied raw attribute order, legacy/None omission,
   mandatory/default validation and exact two-phase prefixes; and
5. label spelling, repository visibility and ordered `RepoSpec` projection.

Existing `pure_instantiation_*`, `real_key_builds_*` and
`real_key_need_*` tests already cover these legacy semantics, errors,
eventlessness, Need, reuse and A -> B -> A identity. Reuse the accepted pinned
Bazel 9.2 extension evidence; visibility adds no observable gap and no oracle.

## Rejected false prerequisites

`HostInstantiatedModuleExtensionRepositoriesKey` has exactly one production
consumer, validation at
`module_extension_repository_validation.rs:208`. Validation is then consumed
by generated repository definition at
`app/slug_core_v2/src/runtime/generated_repository_definition.rs:168` and is
already doc-hidden at loading's public crate boundary. These are serially
later consumers, not prerequisites to observing instantiation.

Root repository mapping is parallel: it directly computes
`HostSelectedExtensionMappingsKey` at
`selected_repo_spec.rs:4466-4469`. Canonical selected module definition is also
parallel, directly computing `HostSelectedModuleRoutesKey` at 2365-2368.
Generated/public/root mapping/bootstrap work cannot supply or substitute for
the missing pure carrier visibility.

## Required visibility design

Freeze exactly one minimal crate-internal surface:

- the existing observed pure key and constructor;
- the existing carrier with borrowed accessors spelled using concrete
  `Arc<Result<HostPureModuleExtensionInvocations,
  HostPureModuleExtensionInvocationsError>>` and `PathObservationEpoch`; and
- one field-private nominal outer usable in the associated Key Value without
  exposing Prepared/HostBzl/Merge variants or the private result alias.

Determine the exact wrapper/projection boundary required by Rust effective
visibility. The private driver and terminal enum must remain private; wrapping
may occur only at the observed key's associated Value boundary. Do not expose
fields, variants, aliases, prepared context, indexes, frontier errors or an
outer inspector. Add no public/lib reexport.

The sole prospective implementation files are:

- `app/slug_loading_v2/src/module_extension.rs`, baseline 2,232 physical with
  first `#[cfg(test)]` at 895, for visibility/wrapper production; and
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  baseline 1,363 physical with tests at 532, for test-only sibling proof.

Caps are <=60 production, <=50 proof and <=110 aggregate semantic; physical
caps are 2,290 and 1,415. Every changed helper/test stays below 100 lines. The
sibling proof may only name the key constructor, exact associated Key Value,
opaque outer and borrowed carrier accessors through function pointers or type
checks. It must not construct the carrier/outer, compute a key, add a driver,
or activate instantiation.

The file pair remains cohesive: `module_extension.rs` owns the private pure
driver and representation, while the instantiation sibling is the sole future
consumer and natural compile-visibility witness. A third file or lib export
would widen the surface without proving a new boundary.

## Compatibility, validation and terminal

Exact Bazel 9 compatibility remains all existing pure and instantiation values,
errors, ordering, namespace/attribute/label semantics, DICE equality and event
behavior. The crate-internal key/carrier/opaque outer and Result-Arc/epoch
handoff are Slug-native. Instantiation observation, validation,
generated/public/root-mapping/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

The design must require focused visibility/compile proof, protected
`observed_pure_`, `pure_instantiation_` and `real_key_` tests, full
`cargo test -p slug_loading_v2`, direct `cargo check -p slug_core_v2`,
formatting and `git diff --check`. No oracle is authorized.

Design ACCEPT schedules exactly
`WP-6-7A-host-pure-module-extension-invocations-observation-carrier-visibility-implementation`.
After implementation ACCEPT, return only to a docs-only
`HostInstantiatedModuleExtensionRepositoriesKey` observation-owner design.
STOP a public/lib export, exposed field/variant/alias, second key/carrier/
adapter, instantiation semantic/caller change, compute activation, event/
equality/retention drift, third file, oracle/fixture work, cap waiver,
validation/generated/root-mapping/public/bootstrap activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Pure observation `9bab80b3` is accepted with exact prepared and ordered
Host-Bzl dependencies, pure-owned print events, historical carrier lifecycle,
cancellation recovery and upper nonactivation. Its only blocker to
instantiation ownership is effective sibling visibility.
