# Current Slug V2 Work Packet

Packet: WP-4-6-7A-rule-execution-group-runtime-design-r1

Status: docs/source design selected by the named-only runtime audit's REPLAN.
No Rust implementation or runtime admission is authorized by this packet.

## Immediate predecessor and learned facts

Imported native genrule loading is accepted and pushed as `de62232ad`.
Its authenticated root-alias replay cleared all six rules_java helper calls,
then stopped at toolchains/BUILD:138 -> rules_cc cc/cc_library.bzl:19 with
`target invocation for named execution-group semantics is unsupported`.
No package, genrule analysis or Java runtime analysis published.

The named-execution-group audit authenticates rules_cc 0.2.17's generated
proxy and complete private cc_library declaration. It declares named
`cpp_link` plus `_use_auto_exec_groups=True`, with the same mandatory C++
toolchain requirement on the default declaration and named group. There are
no named `config.exec` edges in this library's schema: `_def_parser` uses
ordinary `cfg="exec"`; the two named test transitions belong to another rule.
There is no initializer; its computed-default runtime remains independently
unsupported. Stage 4 records exact member hashes and source anchors.

Named-only activation cannot erase the selected automatic policy. Stage 6's
"Rule execution-group runtime prerequisite: named-only REPLAN" records the
pinned behavior matrix, live gaps, shared ownership obligations and tests.
Reuse it; do not repeat archive downloads, authentic replay or the entire audit.

## Observable result and reserved decision

Produce one independently reviewed, implementation-ready contract for the
shared generic rule execution-group runtime needed by this owner: default,
named and automatic group normalization/resolution and their dependency,
provider/property/action consumers. Explicitly classify test-runner and rule
inheritance behavior; configured aspects remain deferred. This is one shared
cross-stage abstraction, not a rules_cc/C++ branch or a loading-only bridge.

Close the remaining design questions: automatic flag/attribute precedence,
default versus per-toolchain requirements, generated-name identity and alias
handling, toolchain-to-action inference including omitted versus explicit None,
named+toolchain compatibility, and common group constraint/property ownership.
Extend the already traced named behavior; do not run a second general audit.
Freeze exact files, compact fields/keys, caps and executable proof commands.
If the category cannot be bounded, identify a concrete prerequisite owner and
its observable contract; do not authorize a partial guard removal.

## Authority and implementation-readiness gates

Use Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
from local git objects, not the different `/home/wgray/bazel` working HEAD.
Read the Stage 6 audit's relevant anchors and remaining automatic-policy
consumers/tests: DeclaredExecGroup, UnloadedToolchainContextsProducer,
ToolchainCollection, RuleContext, StarlarkActionFactory, StarlarkAttributesCollection,
StarlarkRuleClassFunctions and AutoExecGroupsTest. Establish precise public
behavior/failure order from source and discriminating tests before claiming
exactness. Internal Java identity/state-machine assertions are not Slug APIs.

Read the plan-authoring guide and docs/developers/dice.md. Apply the Buck2
utility skill only for proposed retained representation, using the matching
Stage 9 rows, not a broad donor archaeology pass. Obtain one independent
architecture/retained-state review before authorizing implementation.

Required contract:

- Loading-owned normalized declarations/named transitions through final target
  equality; immutable request/configuration inputs and tracked source/repository
  observations, with no command-side semantic reconstruction.
- Constraint-bearing resolution inputs, including zero-toolchain groups; one
  configured-target-owned immutable group collection supplying selected Exec
  configurations, toolchain provider views, properties and action routing.
- Distinct target/Exec and default/named/automatic identity; complete structural
  equality/invalidation with unchanged configuration/path/ActionKey/REAPI domains.
- Source/configuration/platform A/B/A; unchanged-input cutoff; overlapping
  requests; cancellation/Need/error, atomic parent publication and final source
  validation. Never fabricate historical Host snapshots.
- Retained/scratch/view/async lifetimes, compact sharing and Allocative;
  no second global registry, side cache or lock across DICE awaits.
- Pinned-source regressions/focused oracles for actual evidence gaps, default
  controls and named downstream consumers. Reuse existing scaffolding and
  record fixture provenance; do not add copied real-workspace breadth.
- Exact file allowlist and production/proof/aggregate growth caps, with a
  bounded split or concrete cohesion decision for large package.rs/dice.rs.
  No placeholder caps or unspecified public/schema consumers at handoff.

Exact describes only established admitted Bazel surfaces. Host/DICE integrity
and structural identities are Slug-native. This design admits nothing yet;
later computed defaults, C++/Java providers/actions, configured aspects,
broader action/execution families and exact configuration bytes stay deferred.

## Scope and stops

Allowed edits: this manifest, canonical Live Status, and relevant Stage 4/6
owner sections. No Rust, fixtures, vendored sources, harness, dependencies,
runtime or repository changes. Design additions are capped at 300 text lines
excluding manifest replacement. Keep reusable decisions compact, with Git
history retaining completed chronology.

Never move the named invocation guard just to publish loading targets, erase
group names, substitute the default platform or wire the unused
toolchains/exec_groups.rs prototype into live analysis. A new semantic owner,
request projection, async boundary or default behavior change needs explicit
contract/review; a second material design correction is REPLAN.

Validation here is source/structure checks and `git diff --check`; no build or
network replay is needed. The stopped checkout-wide query must not restart.
Tests above one minute require investigation; fifteen minutes is the absolute
maximum. Preserve the separately demonstrated path-epoch fanout concern, with
no longer-timeout, disabled-provenance or fresh-graph workaround.
