# Current Slug V2 Packet

Packet: `WP-4-5-7A-configured-toolchain-selection-architecture`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `e1d0723ea`.

Result: freeze the complete category-4 boundary for provider-independent
configured toolchain eligibility and selection. The design must cover target-
platform ownership, configured aliases, target settings, target and execution
constraints, `use_target_platform_constraints`, requested/post-alias type
identity, mandatory and optional requirements, selection order, derived exec
configuration, DICE/lifetime behavior and the bounded marker bridge. It adds no
Rust or runtime behavior.

## Accepted predecessor and decision pressure

Commit `e1d0723ea` completes category 3. Typed command registrations now expand
through the sole loading target-pattern/package walker, preserve signed Bazel
ordering, and merge before unchanged MODULE registrations in the sole
configured consumer. Category 4 may consume only those canonical registration
results and the already-retained native declarations and condition facts.

The current marker selector is not the target architecture. It supports one
mandatory root toolchain type, rejects registered aliases and target
compatibility, compares execution constraints directly, and immediately
analyzes the implementation under the owner's target configuration. The
category-4 design must replace those assumptions without inventing the
provider graph reserved for categories 5 and 6.

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority. Clean
`../zabel` commit `0795445f…` is peer architecture/optimization guidance only.
Buck2-derived Rust owns generic Starlark syntax and evaluation; authenticated
BCR Starlark owns every rule definition and control path including
`cc_internal`. `cc_common` is only a demanding later client of the same generic
host/provider ABI, never a Rust C++ rule engine.

## Research and learned-fact contract

Read and reconcile the following before choosing a representation or packet
split:

- Bazel `RegisteredToolchainsFunction` and
  `RegisteredExecutionPlatformsFunction`: registered labels are configured in
  target configuration, aliases resolve through the configured target, target
  settings filter before selection, and surviving source order is retained.
- Bazel `ToolchainTypeLookupUtil`, `ToolchainTypeRequirement` and
  `ToolchainResolutionFunction`: preserve requested and post-alias type
  identities; multiple requested aliases may name one actual type; missing
  optional types do not fail; all mandatory types must resolve; the selected
  platform maximizes resolved optional types with stable candidate-order ties.
- Bazel `SingleToolchainResolutionFunction`, `DeclaredToolchainInfo`,
  `Toolchain` and `ConstraintCollection`: the earliest compatible declaration
  per type/platform wins; explicit target constraints match the configured
  target platform; execution constraints match each candidate; and
  `use_target_platform_constraints` rejects simultaneous explicit target/exec
  constraints then matches the target platform's constraints against the
  execution platform.
- Tests `RegisteredToolchainsFunctionTest#testRegisteredToolchains_targetSetting`,
  `RegisteredExecutionPlatformsFunctionTest#testRegisteredExecutionPlatforms_aliased`,
  `ToolchainTypeLookupUtilTest#testToolchainTypeLookup_toolchainAlias`, and the
  optional/mandatory/platform-order cases in `ToolchainResolutionFunctionTest`.
  Reuse accepted Slug selector, registration, first-platform, canonical-package
  and lifecycle evidence; identify a fixture gap before adding one.
- Current Slug `SlugConfiguration`, configured-condition key, native
  `toolchain`/platform/constraint facts, alias analysis, prepared registration
  consumer, action context and topology result. Inspect every existing
  fail-closed marker test before deleting a boundary.
- Zabel's producer-owned target-platform/configuration facts and separation of
  requested versus post-alias toolchain identity, including unresolved
  optional types. Treat these as ownership/lifetime lessons only: copy no Zig
  layout, packed key, store, scheduler, diagnostic, or semantic claim.

Resolve explicitly whether Bazel behavior needs additional source evidence for
target-platform defaulting, registered alias chains, constraint-setting
defaults, multiple optional tie-breaking, duplicate requested/post-alias types,
or target-setting error precedence. Record unsupported gaps rather than
assuming them.

## Required architecture result

### Natural producers and configured identity

Name one natural producer/key/value for each of:

1. the target platform selected by the structural target configuration;
2. configured registered execution-platform aliases and platform facts;
3. configured registered toolchain aliases and declaration eligibility;
4. requested-to-post-alias toolchain-type identity; and
5. the provider-independent selected platform plus declaration set and derived
   exec configuration.

The final design must state which facts already exist and which require a new
public projection, DICE key or retained value. Every configuration, mapping,
alias target, condition result, platform constraint, declaration constraint,
requirement label/mandatory bit and registration position that can change
selection must participate structurally in equality and invalidation. Do not
use display labels, checksums, raw registration text, provider markers or
analysis-side package discovery as semantic identity.

If native `platforms`/`host_platform` label values need a cross-crate
projection or final repository mapping, freeze that conversion and its error
state here. Do not expose the private native option vector or create a parallel
platform field. If the exact Bazel default target platform is not bounded by
already-authenticated `@bazel_tools` content and configuration ownership,
declare a prerequisite packet instead of substituting an execution platform.

### Eligibility and selection algebra

Freeze one pure provider-independent algorithm that:

- resolves alias chains without losing each requested registration/type
  identity or the canonical actual identity;
- validates all selected `target_settings` through the sole configured-
  condition owner before choosing a terminal result;
- validates constraint values by canonical constraint-setting identity,
  rejecting duplicate settings and wrong target kinds;
- applies explicit target constraints against the target platform, explicit
  exec constraints against candidates, and target-to-exec constraint policy
  exactly within the admitted no-default constraint-setting surface;
- computes every requested type independently, accumulates all outer/Need/
  semantic states before precedence, and preserves registered declaration and
  candidate-platform order;
- requires every mandatory type, permits unresolved optional types, and selects
  the first candidate among those with the maximal count of resolved optional
  types after mandatory suitability; and
- publishes only selected declaration identities, platform and derived exec
  configuration. It must not analyze an implementation or materialize a
  provider.

Specify duplicate semantics when requested aliases converge, how optional
absence is represented, and how no-requirement owners retain the existing
platform-only action context. Keep required and optional type identity ready
for the later `ctx.toolchains` value without allocating any temporary provider
shape.

### Marker bridge, lifetime and rollout

The design must name the narrow bridge that preserves currently accepted
single-marker behavior after pure selection. That bridge may consume a single
resolved mandatory selection, but it may not participate in eligibility,
change selection, become the retained category-4 value, or widen the marker
payload. It must be deleted by category 6 when the selected implementation is
analyzed under the derived exec configuration and the real provider occurrence
is exposed.

Classify new memory as DICE-retained semantic state or compute/request scratch.
Retained values use immutable `Arc` slices, canonical labels, `Dupe` and
`Allocative`; compact ordered maps/sets may be scratch. Review Buck2/V1 utility
sources before selecting a new representation. Add no global cache/interner,
standard retained hash collection, evaluator value, package copy, lock across
DICE, or provider-shaped placeholder. Complete errors/results use equality
cutoff, Need is invalid, cancellation publishes nothing, and same-graph repair
must recover.

Produce an ordered implementation sequence. Keep one packet if the resulting
public identity/key/value and proof fit bounded caps; otherwise separate only a
true target-platform/cross-crate identity prerequisite from the selection
implementation. Do not split aliases, settings and constraints into competing
selection owners.

## Compatibility classification

- **Exact design target:** admitted configured alias chains; direct no-default
  constraint settings/values; target-setting filtering; explicit target and
  execution compatibility; `use_target_platform_constraints`; registered and
  candidate order; requested/post-alias type identity; mandatory and optional
  resolution; maximal-optional platform choice with stable ties.
- **Slug-native:** Rust layout, DICE decomposition, structural configuration and
  exec identity bytes, valid-Unicode labels, observation carriers and unproved
  diagnostic wording.
- **Unsupported/deferred:** constraint-setting default values unless the audit
  proves existing ownership; platform `required_settings`, flags and allowed-
  toolchain-type policy; explicit command `--platforms`/`--host_platform` until
  their command/configuration packet; forced and host execution platforms;
  named/automatic exec groups; provider payloads, implementation analysis under
  exec configuration, `ctx.toolchains`, exact Bazel configuration/output bytes,
  and all Rust implementation of BCR rule control flow.

This docs-only packet changes no compatibility behavior.

## Proof obligations

1. A source-to-owner table covers target platform, both alias families,
   requested/actual type identity, settings, constraints, selection and exec
   derivation without a second semantic store.
2. Pseudocode fixes exact loop/order, mandatory/optional and alias-convergence
   semantics, including multi-type error/Need precedence.
3. The design gives structural key/value fields, equality/validity,
   observation frontier, cancellation/recovery and memory-lifetime contracts.
4. The target-platform/default boundary is either implementable from existing
   authenticated inputs or becomes a named bounded prerequisite; no guessed
   substitute is allowed.
5. The marker bridge has one consumer, no selection authority, no new payload,
   and an explicit category-6 deletion condition.
6. The implementation allowlist, caps, direct dependents, source regressions,
   discriminating tests, skipped upstream tests and stop conditions satisfy the
   plan-authoring checklist.
7. Independent reserved-architecture review returns `ACCEPT`; one material
   correction is allowed, a second is `REPLAN`.

## Allowlist, caps and validation

Writable files are limited to the canonical plan, Stage 6 owner, Stage 9
ledger when a concrete utility decision is made, and this manifest. No Rust,
Cargo, fixture, generated evidence, oracle execution or runtime behavior.

Caps: 260 net manifest lines, 140 net Stage 6 lines, 40 net canonical lines and
40 net Stage 9 lines. Run targeted source/owner scans,
`scripts/v2_archive_status.sh`, `git diff --check`, packet/canonical ID matching,
and independent Sol reserved-architecture review. No performance benchmark is
needed in a zero-Rust packet; the design must name measurement as residual if
it proposes a retained representation or changes the selection hot path.

## Stops

STOP and `REPLAN` for an execution platform substituted as the target platform;
unmapped/display/checksum identity; a second condition matcher, alias resolver,
constraint store, registration store or selector; implementation/provider
analysis inside category 4; one-required-type-only architecture; optional types
silently treated as mandatory or absent; provider markers deciding selection;
package/source discovery in a consumer; copied native option storage; a global
cache/interner or lock across DICE; Rust ruleset/`cc_internal` control flow;
`cc_common` specialization; Zabel as authority; or an unbounded packet whose
identity, owner and proof cannot be independently reviewed.
