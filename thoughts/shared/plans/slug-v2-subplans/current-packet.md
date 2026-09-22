# Current Slug V2 Work Packet

Packet: WP-7-69-m7a-cpp-compile-registration-r1
Status: accepted

## Outcome and compatibility

Unchanged public cc_common.compile registers immutable typed CppCompile actions
for regular C/C++ sources with ordinary object and default dotd outputs.
Construction only: command expansion/aquery and discovery-aware execution remain
explicitly unsupported. No fixed-input Spawn substitution or compiler invocation.
Predecessor WP768 at d5c4364ec accepted genuine context composition with37 focused
tests and Core/CLI checks; full receipt is in Git. Reuse its54 unchanged bodies.

Exact admitted construction follows pinned Bazel9.2 source
8220c6198837d5c13d53fea211cf3282aa12408a:
CcStarlarkInternal.java579–758/893–947,
CppCompileActionBuilder.java83–98/192–228/252–386/536–564,
CppCompileAction.java241–340/390–425/512–518/979–992,
CcCompilationContext.java436–450. Preserve configured-action checks, source-defined
input roles/order, selected tool requirements, PIC and include-validation policy.
Slug-native: structural configuration/artifact/action identity, output roots,
Rust Host and diagnostics. No exact ActionKey or REAPI projection is admitted.
Existing Solib evidence omits compiler actions/arguments. Authenticated allocator
cc_library and compile.bzl1691–1715 establish constructor demand, not production
allocator toolchain/features/argv or execution success.

## Decisions and ownership

Retain a command recipe, not eagerly expanded arguments/environment. Expansion
would change failure timing and live Variables cache state. Complete feature
configuration and Variables payload—including unused metadata—must survive.
Configured environment retains inherited names, not ambient process values.
Future expansion must apply configured-env, then PWD unless requires-darwin,
then feature-env; retain the inputs needed for that recipe.

BuildAPI CppCompileSpec uses Arc/Dupe, Allocative, compact immutable fields and
existing ArtifactInputs/depset DAGs. Preserve distinct mandatory-spawn, cache-only,
declared-header/invalidation, builtin and additional-prunable roles; retain full
compilation context, selected tool and complete feature/Variables recipe. Object
and dotd roles reference one output collection. One PublicationEqState compares
all Variables/context/depset groups, with complete feature payload, never only an
occurrence token or ordinary Variables Eq/Hash. Analysis validates full supplied
configuration against active output root; ConfiguredActionOwnerContext owns action
configuration/platform/toolchain selection. Preserve distinct toolchain Cpp facts.

Constructor validates configured action and selects tool requirements eagerly.
Use both structural scanning flags, requested cc_include_scanning and explicit
override; effective scanning true is unsupported. Project use_specific_tool_files
rather than hardcoding compiler_files. Preserve all selected-tool requirements,
plus legal propagated target tags; nonempty deferred modify_execution_info fails
closed. Native default/automatic owner selection includes default fallback when
the automatic toolchain group is absent. Do not infer a platform from Solib.

Admit ordinary source/generated C/C++ Files, object/dotd, PIC/non-PIC, builtin
headers, non-code/additional mandatory inputs, cache-only inputs and declared
headers. Reject unsupported source kinds/tree expansion, modules/showIncludes,
nonempty copts regex, compiler paramfiles, shareable actions, auxiliary output
families and nonempty scanning roots. Validate all native binder arguments and
unsupported non-default branches before recording a pending action.

### Publication checkpoint

Bazel retains Variables through rule evaluation; snapshotting at registration
would lose later nested-list/cache mutation. Preserve Variables and the full
compilation-context Value in a traced evaluator-local pending record until normal
end-of-analysis publication (CcCompilationContext.of retains its StarlarkInfo).
Consumed toolchain fields and construction-time input lists are eager snapshots,
not delayed reads from mutable toolchain fields. Lower
with the same AnalysisValueLowerer as returned providers. Do not store raw Values
inside an untraced host map: capture the original immutable additional/tool/non-code/
declared/cache depset Values in the pending root, validate them eagerly, then lower
and compose final owned input roles with the shared publication lowerer. Eager
temporary validation must not split final aliases from context/Variables/providers.
Do not store raw Values
inside the Send+Sync sink or BuildAPI/DICE state, or hold a lock across compute.
Use a dedicated slot in Loading's existing GC-traced Module.extra_value root tuple;
the root-ctx guard clears only its own slot. Each pending record retains its
normal-action insertion index and ordered occurrence. At finalization, after
provider lowering and before runfiles completion, lower pending recipes using the
same lowerer, merge with normal actions in original order, validate the combined
batch into a fresh registry and replace the phase-local registry atomically.
Conflicts fail analysis before any graph publication; no placeholder action or
unfinished recipe enters BuildAPI/DICE. A failed finalization publishes nothing.
Independent design review ACCEPT on2026-09-22 confirms this seam. Pinned
RuleContext564/CachingAnalysisEnvironment312–314 append actions; conflict and
ordered index assignment follow in RuleConfiguredTargetBuilder277–291/Actions232–286.
Final review still requires executed GC/mutation/order/conflict evidence.

Existing ConfiguredNodeAnalysisKey/observed package/configuration dependencies own
final publication, invalidation and cutoff. No new DICE key, semantic side cache,
interner, filesystem discovery or retained evaluator heap. Read docs/developers/dice.md.
DICE-retained actions release with graph values; construction/lowering maps and
pending Values are phase-local and release on success/error/cancellation.
Stage9 disposition: reuse existing Arc/Dupe, CompactString, boxed slices,
ArtifactInputs/depsets/PublicationEqState and Allocative; no donor extraction,
memory-reduction or performance claim. Discovery results are not introduced.

## Proof and exclusions

Reuse selected-registry fixture adapter and all54 pinned unchanged source bodies.
Positive authored caller uses unchanged public configure_features; its old direct
configure_features([]) contains no configured action and becomes a negative.
Toolchain fixture has inert files/tool paths: valid construction evidence only.
Pinned-source regressions adapt builder/input semantics. DependencySet/HeaderDiscovery
and remote in-memory-dotd tests remain deferred with parsing/resolution/execution.
Whole compilation-output provider's generic temps closure stays unadmitted.
Immediate ctx.created_actions inspection also stays unsupported; pending records
are never presented as already-published actions.

Required discriminators:
- Public regular-source compile publishes actions and object/dotd Files in genuine
  main/external target/exec configurations. Verify source/tool/builtin/non-code/
  additional input order, cache-only exclusion from spawn, declared-header separation,
  PIC, validation and configured owner.
- Full unused feature/Variables/configuration payload discrimination and aliases;
  same-DICE A/B/A input metadata/configuration restoration. Post-registration nested
  Variables mutation and real GC survive into final recipe without early expansion.
- Genuine/forged/inactive ctx, configuration, Files, toolchain/features/Variables;
  missing action configuration, foreign output/configuration, unsupported branches,
  malformed unused expansion metadata, conflict atomicity and interleaved action order.
  Toolchain builtin-list mutation after the call must not alter eager input snapshots.
- Native default/automatic owner policy, unfiltered tool requirement plus legal tags,
  deferred modifier rejection. No inherited environment resolved from ambient state.
- Core prerequisite/source-execution, REAPI and existing FileWrite-only aquery view
  refuse CppCompile explicitly; no fabricated empty command/ActionKey success.
- Protect WP768 contexts, output roots, Solib/Spawn and publication boundaries.

## Scope, routing and gates

Base d5c4364ec; candidate codex/wp769-cpp-compile-registration-review.
Design ACCEPT precedes production edits; final actual-diff/evidence ACCEPT precedes
atomic integration. Root owns scheduling/Stage6/Stage9/bootstrap closure, integration,
serial validation and Core/REAPI rejection guards.
Root also owns Configuration native/configuration.rs child-module hook and
native/configuration/cpp_compile.rs checked scanning/specific-tool/modifier projections
over existing retained options, with focused source-derived tests. No converter,
CLI admission or new option state is added.

BuildAPI worker allowlist: actions/cpp_compile.rs and children,
actions/{spec,mod,ctx_actions,registry,reapi_projection}.rs, src/lib.rs, narrow
publication-equality visibility hooks in cc_variables.rs/analysis_value.rs.
Existing cc_features/metadata.rs POSIX normalization may be re-exported through
cc_features.rs for builtin-directory eager snapshots; no new path algorithm.
Existing tests/actions.rs may adapt the two fallible render_argv callers; the
public ActionSpec projection now returns an explicit unsupported error for CppCompile.
Native worker: Loading cc_common.rs thin binder, cc_compile_action.rs and children,
cc_context.rs, cc_compile_outputs/configuration.rs visibility, lib.rs and existing
subrule_invocation capability/tracing declarations and evaluator_roots.rs;
Analysis starlark_rule.rs thin
hooks and starlark_rule/{cc_compile_action,cc_context}.rs children.
Existing starlark_rule/context_lifecycle_tests.rs may add direct genuine compile
capability checks for foreign configuration and expired contexts before mutation.
Protected Variables-cache replay may remove the duplicate shard injection in
tests/cc_configure_features/mod.rs: WP766 centralized that injection in the common
fixture helper. No fixture routing or semantic assertions change.
The publication regression exposed existing depset memo replacement across roots.
Independent correction design ACCEPT permits the narrow Analysis analysis_value.rs
memo-reuse guard and child regression: completed immutable nodes stay shared and
new parents retain them through existing External edges. No new identity owner.
Root integration tests: tests/rustc_map_each/compile_context.{bzl,rs} and children,
mod.rs, authored cc_toolchain.bzl; focused Core prerequisite/source staging and
REAPI test children with narrow explicit production rejection hooks.
Shared genuine-action test construction may live in BuildAPI tests/cpp_compile_support/mod.rs
and be included by Core/REAPI test children; no test-only production export. Core's
existing source-family guard may be extracted to a borrowed source_spawn helper for
direct pre-observation refusal tests. Legacy payloads labeled CppCompile must also
fail registry/command/REAPI projections rather than bypass typed guards.
Affected crate BUILD files may declare these test sources and cross-package
compile_data; no new runtime or test family is admitted by those declarations.
Finishing workers may take named disjoint tests after coordination.

Large central files get only hooks; child modules own construction/publication.
Estimate600–1000 production and500–800 test lines; growth is responsibility-review
trigger, not partial landing. No unrelated loader, converter, formatter or
execution architecture changes. Pending publication must preserve exact lifetime
semantics; it is not permission to snapshot early or waive GC/order/conflict gates.

Resolve pinned nightly tools; serial offline/locked Cargo JSON no-run for BuildAPI,
Loading, Analysis and relevant Core/REAPI targets. Exact batched preflight then
focused execution, Core/CLI direct-dependent checks, changed-Rust formatting,
diff/archive/status checks. Preparation bounded60s/operation; seconds-scale runtime
estimates are not new user deadlines. No Bazel/daemon/remote execution gate.
REPLAN for a new semantic owner beyond this checkpoint, contradictory pinned
evidence or required discovery activation. Ordinary fixture/compiler corrections
stay in contract. M7A remains partial; M8 unproved.

## Closure receipt (2026-09-22; accepted)

Base d5c4364ec; candidate codex/wp769-cpp-compile-registration-review. Independent
design ACCEPT covers traced publication; correction design ACCEPT covers completed
depset memo reuse. Independent final ACCEPT confirms the coupled construction/
publication change against the actual diff and70 passing selectors.
Growth exceeds the initial estimate: approximately1,200 production lines and2,000
test/fixture lines, including thin central hooks. Responsibilities remain split
between owned BuildAPI recipes, Loading binder/roots, Analysis construction/finalization
and separate policy/lifecycle/integration/refusal tests. No donor, generic serializer,
discovery cache or new DICE key; all54 upstream fixture bodies remain unchanged.

70 unique exact-selected tests pass: Configuration2, BuildAPI7 plus actions8,
Loading11, Analysis library14 plus configured19, Core4 and REAPI5. Receipts are
under target/wp769 (each row records command, exit, selected/pass count and elapsed):

- Configuration: tests-1790037365053162275; BuildAPI: tests-1790037864655659109
  and tests-1790037893789837186.
- Loading: tests-1790038962431454336 and protected tests-1790038837879333917.
- Analysis library: tests-1790039123783388212 plus unaffected protected rows in
  tests-1790038646082441905 (duplicate selectors counted once).
- Configured: first7 rows of tests-1790039087695094368, protected9 in
  tests-1790038632625144776, and corrected cache3 in tests-1790039188521913336.
- Core: tests-1790038726218676515; REAPI: tests-1790038765408523234 and
  tests-1790038773422826150.

The constructor matrix proves native malformed/foreign/schema/lifecycle guards,
explicit scanning overrides, eager compiler-field fallback validation, C/C++ and
main/external target/exec publication. Public metadata and full unused configuration
restore A/B/A in one DICE. Native interleaving/conflicts and post-registration
Variables/context mutation pass; a separate pending-root test proves real safe-point
GC before/after context cleanup. Existing dotted-cache publication tests separately
protect stale cache seeds. All semantic assertions remain intact.

Pinned nightly2025-09-14 offline/locked Cargo JSON preparation and Core/CLI checks
pass. Across23 preparation/check attempts, elapsed290.114s, max55.719s; latest
unique test executions total2.351s. Packet elapsed approximately55 minutes;
review time was not separately instrumented. Preserved failures drove explicit-None
binder correction, shared depset memo reuse, a test-only safe GC scheduling fix,
and removal of duplicate observation-shard injection from an older protected
fixture (WP766 moved that injection into its shared helper). No failure was waived.
Formatting, diff, archive and matched packet-status checks pass. No Bazel, daemon,
remote or compiler action ran. Command/aquery/ActionKey and discovery-aware execution
remain unsupported; inert toolchain construction is not allocator execution evidence.
M7A remains partial; M8 unproved.
