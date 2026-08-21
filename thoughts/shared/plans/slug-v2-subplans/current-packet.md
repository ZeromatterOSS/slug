# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-repository-mapping-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `c96ae09d`

## Goal and authority

Design only the private observation sibling of
`HostRootRepositoryMappingKey`. This is the uniquely smallest missing lower
owner before the accepted canonical observation can feed a complete apparent-
mapping family. Do not implement or promote a carrier and do not activate
canonical apparent mapping or any upper consumer.

Write only the canonical plan, this manifest, Stage 6 and the orchestration
routing log, at net caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust,
tests, fixtures, oracles, Cargo/BUILD, APIs, exports, callers and commits are
read-only.

## Learned frontier and decision

Accepted `c96ae09d` adds the callerless private canonical observation key,
carrier and typed outer at
`generated_repository_definition.rs:493-710`. Its exact two direct semantic
consumers remain:

- same-file `HostCanonicalRepositoryApparentMappingKey`, whose nonroot branch
  computes canonical definition at lines 905-934; and
- sibling `HostRootApparentRepositoryDefinitionKey`, which computes canonical
  definition at lines 310-343 only after root apparent mapping resolves a
  non-main, non-builtin target.

The observed canonical key, constructor, carrier/accessors and outer are all
module-private. The same-file mapping owner can use them, while the sibling
root-definition module cannot. A visibility change is not the next step:
apparent mapping's total key domain also includes root context, whose first
child at generated-definition line 879 is the carrierless public Bzlmod
`HostRootRepositoryMappingKey`. An observed mapping key that covered only
nonroot context, silently used an empty epoch on root context, or split the
legacy key family would be an invalid partial adapter.

Live Bzlmod source shows root mapping at
`selected_repo_spec.rs:4607-4669` has exactly one semantic child,
`HostSelectedExtensionMappingsKey`, and exactly one production consumer, the
core apparent-mapping root branch. The accepted private
`HostSelectedExtensionMappingsObservationKey` and Result-Arc/epoch carrier
already occupy the same Bzlmod file. Therefore no visibility or semantic
prerequisite precedes a private root-mapping observation owner.

Root apparent definition remains later: it first computes apparent mapping at
line 266, handles main/builtin deferred targets, and only then computes
canonical definition. Its sole production consumer is root apparent route at
route line 303; route feeds source input at its line 186, followed by existing
source-path/observation, repository route/source/file and public command/
bootstrap boundaries. None directly consumes the canonical observed key, and
none can substitute for the missing root-mapping epoch.

## Required design decision

Freeze one private matching-family Legacy/Observed root-mapping owner in
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 12,564 physical with
tests at line 4,678. The design must decide exact nominal key/carrier/typed
outer names, shared driver and child projection over only the accepted observed
extension-mappings sibling. Preserve the existing root full-scan ordinal law:
first root, first conflict while scanning all routes, exact Missing/Duplicate/
Context terminals, existing predecessor retention and mapping order.

Need and lower observed outer must remain carrierless. DICE compute and
complete semantic terminals must retain the exact empty or child epoch dictated
by the accepted family; the single complete child epoch passes unchanged, with
no merge/rebuild/union. Preserve lower event ownership, batchless parent/warm
rows, cancellation recovery, Complete-only validity/equality and exact legacy
parity. Retain only one root-mapping Result Arc plus compact epoch; no child
carrier, extra map/order copy, scan scratch, event/evaluator state, cache,
manual lock or task may survive publication.

Freeze exactly three proof tests for identity/terminal/full-scan algebra, real
family/order/events/parity, and held lifecycle/cancellation/nonactivation.
Require exact observed-parent -> observed-extension-mappings and legacy-parent
-> legacy-extension-mappings dependency rows, no canonical/core activation,
epoch subset/metadata-only/Arc-Reused evidence and an upper denylist. Reuse the
accepted extension-mappings proof and Bazel 9.2
`BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping` and `BazelDepGraphFunctionTest`; add
no fixture or oracle.

Prospective implementation caps are <=230 production, <=680 proof, <=910
aggregate semantic and <=13,480 physical, at most six production/six test
helpers, exactly three tests, driver below 120 and every helper/test below 200.
The design must retain or tighten these caps, name exact serial focused/full
Bzlmod validation plus direct core compile check, and record the large-file
cohesion decision. No hot-path measurement is required unless source review
finds a demonstrated hot path.

## Compatibility and terminal

Root mapping values/errors/order/full-scan behavior, equality/invalidation and
lower event ownership remain exact Bazel 9 compatibility. A private observed
key/carrier/typed outer and transaction-local Result-Arc/epoch association are
Slug-native. Cross-crate carrier promotion, canonical apparent mapping, root
definition/route/source, public command/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

Design ACCEPT may schedule exactly
`WP-6-7A-host-root-repository-mapping-observation-implementation`. After its
acceptance return only to a root-mapping carrier-visibility audit, then the
canonical apparent-mapping frontier. STOP implementation, a second file/key/
owner/adapter, export/reexport/caller, canonical/core edit, partial root/nonroot
family, semantic/order/event/equality/retention drift, epoch merge, retained
scratch/task/lock, fixture/oracle, cap/proof waiver, upper activation,
milestone closure, M8/M7B or exact identity work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Canonical implementation `c96ae09d` is green and callerless. The consumer
audit rejects early canonical visibility and partial nonroot mapping because
neither closes the root branch's missing root-mapping observation epoch.
