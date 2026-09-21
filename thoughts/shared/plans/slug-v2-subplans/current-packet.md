# Current Slug V2 Work Packet

Packet: WP-7-60-m7a-cc-variables-and-expansion-r1
Status: accepted

## Outcome and compatibility

Run unchanged rules_cc 0.2.17 create_compile_variables on real analyzed inputs,
then select tools and expand command lines, environments and execution requirements
from the accepted native feature configuration. These variable builders are the
same producers used by compile.bzl before create_cc_compile_action. This advances
the demanded compiler prerequisite, not an isolated actions2ctx shim.
Predecessor WP759 at 065830a89 accepted public feature configuration with 30 focused
tests, Core/CLI checks and independent final ACCEPT; its receipt is in Git.

Exact: named native variable constructors/composition and feature query/expansion
semantics, including definition order, predicates, explicit iteration, dotted
lookup and None shadowing, first matching tool and POSIX path origin. Exact
Starlark observable variable Map/Single distinctions and parent reference equality
must not be replaced by WP758's occurrence-only wrapper equality. Diagnostics are
source-derived message-shape regressions. Slug-native: existing structural
configuration/artifact/publication identity, Rust valid-Unicode strings and hash
bits. Unsupported/deferred: Windows execution OS, execution-time path mapping/tree
artifact expansion, foreign/internal VariableValue implementations, cyclic raw
variable payloads, builtin provider structures other than ToolchainInfo,
publication of raw set/range bindings (the bridge has no such retained types),
cross-call diagnostic-cache message history after freeze,
full toolchain/FDO provider construction, link-variable Cpp
projections, actions2ctx/compile actions and execution. Existing loading/mapping
boundaries remain. M7A partial; M8 unproved.

## Source, ownership and lifecycle

Pinned Bazel 9.2 revision 8220c6198837d5c13d53fea211cf3282aa12408a:
rules/cpp/CcStarlarkInternal.java:133–183 constructors/composition/string sequence;
CcModule.java:62–138 and :202 caller restrictions; CcToolchainFeatures.java:
Flag/EnvEntry, FlagGroup:200–298, predicates/FlagSet/EnvSet:300–386,
Tool/ActionConfig:523–724, FeatureConfiguration:904–1004;
CcToolchainVariables.java:323–461 lookup/cache, :475–708 typed access,
:795–836 composition/conversion, :846–1056 structure and Map/Single semantics.
CcToolchainFeaturesTest:231–441,528–1098,1379–1524,1792–1909,1975–1989 and
StarlarkCcCommonTest:347–391,500–610 provide upstream observable themes.
Skip Java implementation-only interner assertions.
Public-global routing is the composed injected facade: builtins_bzl/bazel/exports.bzl
:17,46–50 and StarlarkBuiltinsFunction:271–287 inject cc_common_bazel.bzl.
Its six tool/requirements/action-enabled/argv/env/empty wrappers are unrestricted;
the inner Java CcModule restriction is not exposed as the global API. Preserve
that distinction instead of inventing a rules_cc-to-builtins source-label mapping.
The separately restricted get_tool_requirement_for_action alias is not selected.
Existing rules_cc archive SHA256 is
283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a.

Follow docs/developers/dice.md. Loading owns evaluator-live variables; Analysis
owns normal graph publication; BuildAPI owns retained semantic variables and pure
expansion algorithms. No new DICE key, filesystem observation, cache service,
global interner, lock across compute, dependency or digest domain.

Use a narrow generic variable-access adapter for BuildAPI expansion. Loading
adapts actual values lazily, avoiding eager expansion of unused variables and
avoiding a Loading-to-Analysis dependency. Copy the constructor's top-level
bindings in Java string-sorted key order but preserve nested live value references.
Combine retains the supplied parent, copies only local bindings of added variables,
rejects overlapping added local keys, permits local shadowing of parent keys,
and chooses Single iff one added local key. None shadows a parent's direct key.
Lookup tries the full direct name first, then longest dotted prefix; scoped
iteration shadows exactly the named variable while leaving other parent bindings.
Accept native admitted strings, bools, Files, sequences/dict keys/depsets and
struct/user-provider/ToolchainInfo structures; preserve value types, order, aliasing
and deferred type errors. Other builtin providers lack a common live/frozen field
owner in the current bridge and are not needed by these compile-variable producers;
reject them on access rather than adding an unrelated provider capability.
Raw set/range sequence reads may execute live; normal publication rejects those
raw types instead of flattening them to lists and losing type/equality identity.
intern_string_sequence converts an admitted empty range to an immutable list.
Tool lookup uses all declared actions, even disabled ones. Only enabled action
configs supply action flags; enabled feature flags can match arbitrary actions.
Environment conflicts fail, never silently last-wins.

Variable nodes retain an occurrence token for parent-reference comparison and
publication alias topology, NOT as their sole Starlark equality. Map is hashable
even with raw list/dict payloads; Single is unhashable. Use separate raw-payload
hashing rather than accidentally applying ordinary list/dict hash barriers.
Parent, local values, File owners and depset structure participate in retained
semantic publication comparison. Cache entries are excluded from Starlark equality
and hash, but cannot be excluded from publication if they change future expansion.
empty_variables uses the source-defined immutable singleton occurrence
(CcToolchainVariables:272–288), not a fresh parent token per call. A fixed
OnceLock identity constant is not a semantic interner or mutable cache service.

Native dotted lookup caches resolved scalar values, sequence snapshots, live
structure references and missing/error results; direct-key lookups remain live.
Preserve this within evaluator lifetime and across publication. Live wrappers
own traced mutable caches. Freeze and Analysis publication retain an immutable
cache seed, including stale snapshots; frozen adapters add only per-call scratch
resolutions over that seed. This is sound for the admitted NOOP mapper/no-metadata
path because frozen raw references cannot change. A materializer yielding mutable
values must instead install a live seeded cache. Repeated frozen calls preserve
outputs and failure category; cross-call detailed diagnostic-cache message history
is explicitly deferred. No semantic state mutates a shared DICE result after
publication. Never silently deep-copy live inputs or discard stale cache seeds.
The live cache uses an evaluator-local traced mutex so the generic frozen type
remains Sync without unsafe claims. Guards only clone/insert cache entries and
never cross value conversion, evaluation or DICE compute. Freeze consumes it;
the actual frozen variant contains only an immutable boxed seed.

Retained variable nodes use shared immutable handles, CompactString, compact
ordered maps/slices and Allocative per Stage 9 utility dispositions. Mutable
evaluator-local references/cache and expansion stacks/output buffers are phase
scratch until normal snapshot publication; retained values contain only owned
AnalysisValues and immutable cache seeds. Normal memo/cycle/depset traversal,
freeze/GC, equality cutoff, invalidation and cancellation release apply. No
performance claim or new utility import.

## Scope and coordination

Expansion worker: app/slug_build_api_v2/src/cc_features.rs and metadata.rs narrow
dispatch/access, new cc_features/expansion.rs and tests; shared adapter interface
coordinated with Loading worker. Tool/action-enabled, command/environment and
requirements operations all use the existing selected model.

Loading worker: new app/slug_loading_v2/src/cc_variables.rs and children;
cc_common.rs new native variable/query dispatch, cc_features.rs access as needed,
lib.rs exports. Implement live variables, native call restrictions, freeze and
adapter. New semantic code stays in child modules, not the large cc_common owner.
provider.rs may expose its existing exact depset flattening to this adapter with
pub(crate) visibility only; do not duplicate or change the traversal.

Root: new app/slug_build_api_v2/src/cc_variables.rs and lib.rs; BuildAPI
analysis_value.rs plus Analysis analysis_value.rs narrow variant integration and
cohesive child tests; new Analysis tests/cc_feature_expansion/ and starlark_rule.rs
registration; BUILD.bazel only necessary source inclusion. Root owns retained
publication, authentic integration, Cargo, docs and commit. Reuse WP759 fixture
setup through a bounded shared test helper if needed; preserve its existing gates.
No configuration/action-sink/dependency/lockfile/source-corpus edits.

After finishing expansion, that worker is reused for the new authentic Analysis
integration module and its registration/BUILD source list. Root supplied the
bounded WP759 shared fixture setup and continues retained bridge/lifetime tests.

Estimate 1,200–2,200 production and 900–1,600 test lines. Growth triggers a
responsibility/redundancy review, not weaker admission. Independent design and
final reviews required for the public lifetime/publication boundary.

Independent design review ACCEPT: the live-cache/immutable-seed lifecycle is
sound for NOOP mapping and immutable frozen inputs. Final review must verify
parent identity, complete cache publication, raw hashability and frozen-call
regressions; deferred diagnostic history cannot widen into an exact claim.

## Evidence and validation

Reuse existing creator/legacy/configuration sources. Authentic compile-variable
proof additionally imports already-vendored compile_build_variables.bzl,
cc_helper_internal.bzl, private/paths.bzl and rules_impl/native_cc_common.bzl.
The complete public cc_common export wrapper is exercised where its existing load
closure permits; never extract or rewrite a wrapper body to fake production proof.
Explicit authored capability/FDO envelope remains a boundary input, not full
production provider construction. Use WP759 canonical-external observed root.

Discriminators: selected/unselected actions; first matching tool and no-match;
all three tool path origins; duplicate requirements/order; action flags before
feature flags; OR feature predicates and all flag-group guards; nested explicit
iteration and full-key/longest-prefix lookup; missing/None/wrong types;
relative path chunks; environment conflicts; variable composition/local-only copy;
Map/Single equality/hash/parent aliases; top-map copy versus nested live mutation
and dotted cache snapshots; repeated frozen calls; freeze/GC and provider round trip including unused
File/depset/cache identity; same-DICE input change/restoration and published reuse.
Keep execution-time tree expansion and link/compile actions explicitly unproved.

Resolve pinned nightly-2025-09-14 tools; serialize offline/locked Cargo JSON
--no-run preparation and preflight exact selectors once per unchanged binary.
Focused BuildAPI, Loading, Analysis unit/integration gates plus Core and CLI
checks. No Slug/Bazel binary, daemon, build action or broad suite required.
Preparation cap 60s/operation, expected test runtime seconds; packet estimates
are not user ceilings. Preserve failed receipts and correct ordinary implementation
or invocation errors in scope. Format, diff, plan status and archive checks.

REPLAN for a contradiction requiring a new semantic owner or unbounded foreign
runtime, not ordinary coding failures. Acceptance requires actual invariant review,
source-grounded regressions, terminal validation receipts and atomic commit.

## Terminal evidence

Pinned nightly-2025-09-14 tools resolved via rustup. Serialized commands:
`cargo test --offline --locked -p <owner> --lib --no-run --message-format=json`
for BuildAPI, Loading and Analysis; Analysis integration uses `--test starlark_rule`
instead of `--lib`. Core and CLI use `cargo check --offline --locked` with JSON.
Final preparation: BuildAPI 1.319s, Loading 9.180s, Analysis unit 6.829s,
integration 6.325s; Core check 15.085s and CLI check 15.988s, all exit 0.
All 13 preparation/check attempts total 147.757s; maximum 30.878s, under 60s cap.

One preflight per unchanged executable/selector set; exact tests then execute
with `--exact <selector> --nocapture`, exit 0 and exactly one pass each:

- BuildAPI `cc_features::expansion::tests`: 8 tests covering ordering, predicates,
  guards, nesting, path/chunks, conflicts and lazy iteration.
- Loading `cc_variables::tests`: 5 tests covering native shape/composition/hash,
  live cache semantics, deferred failures, frozen seeds, validation/cycles;
  existing Auto/Never/Always GC modes execute inside the helper.
- Analysis `analysis_value::cc_variables::tests`: 3 tests covering owned/frozen
  round trip, all cache entry kinds, alias topology, unused File owner and hash
  barriers; 2 protected `analysis_value::cc_features_tests` tests.
- Analysis integration `cc_feature_expansion`: 5 tests covering unchanged
  compile variables, configured publication, same-DICE change/restoration,
  live dotted snapshots, public binder/output boundaries and cycle/range rejection;
  all 9 protected `cc_configure_features` tests also pass.

32 selected/32 passed; latest per-selector runtime total 0.402s. Local receipts
under ignored `target/wp760`: tests-1790021175969053707 (BuildAPI),
tests-1790021428375053726 (Loading), tests-1790021683598058283 (Analysis),
tests-1790021997717773515 (integration), each `.receipt.json`, plus command logs.
No Bazel/Slug binary, daemon or build action ran; pinned-source regressions and
unchanged already-vendored helpers supply evidence without expanding the corpus.

Preserved failures: four compiler attempts exposed ordinary type/lifetime,
test-module path and private test-harness API mistakes; corrected within existing
owners, moving live publication proof to integration rather than exposing private
Loading APIs. Two test attempts expected different diagnostic text; corrected
against actual native errors. Review also caught unsupported dotted availability
being masked and eager sequence conversion; both corrected and discriminated.
No ownership/compatibility replan. Roughly 1,620 production and 1,480 proof lines
remain within estimate, concentrated in cohesive child modules. Shared utility
reuse and lifetime ownership are recorded in Stages 6/9 and bootstrap readiness.

Next demanded boundary: link-variable Cpp projections and authentic linker-args
use, then separately scoped compile/actions2ctx/full toolchain producers. This
packet does not admit action execution, full cc_library or M8 bootstrap.

Independent final review ACCEPT after the final 32-pass receipts and closure
audit. Rustfmt, diff, plan-status and archive checks pass. Packet elapsed about
40 minutes (2026-09-21 19:42–20:22 UTC); design/final review overlapped implementation
and was not timed separately. Observable gate advanced: real configured
compile-variable production and expansion through published native Variables.
