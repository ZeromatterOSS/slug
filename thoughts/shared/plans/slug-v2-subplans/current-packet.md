# Current Slug V2 Work Packet

Packet: WP-7-157-m7a-disabled-coverage-instrumentation-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance the byte-identical authentic Rust-toolchain cquery past missing
analysis_ctx.coverage_instrumented at rules_cc cc/common/cc_helper.bzl:304.
Expose the public ctx.coverage_instrumented(target=None) method for structurally
coverage-disabled configurations. Accept omitted/explicit None and positional
or named genuine Target (including source-file Targets); return primitive
False only after type and active-context validation. Coverage-enabled calls
fail explicitly as unsupported/deferred rather than guessing provider/filter
results. No coverage fragment declaration is required.

This method shape and disabled short circuit are exact, pinned to Bazel 9.2
8220c6198837d5c13d53fea211cf3282aa12408a StarlarkRuleContext.java:761-773,
StarlarkRuleContextApi.java:223-248 and StarlarkRuleContextTest.java:3401-3455
plus the frozen-context matrix at 3470. CoreOptions.java:566 owns False default.
InstrumentedFilesCollector.java:153 owns enabled filter/test-target inclusion;
those semantics, InstrumentedFilesInfo, coverage actions and full C++/toolchain
closure remain deferred. Configuration/DICE/path/diagnostic identity remains
Slug-native. Linux remains the priority; no Windows implementation or execution
claim is added.

## Ownership and scope

AnalysisContextGen already retains CcContextProjection with immutable structural
SlugConfiguration, produced from the sole configured target key. Add a small
coverage-disabled projection over its existing configuration.coverage_enabled()
getter. Missing/malformed configuration fails closed. Do not store a Boolean
copy or read an explicit dependency's configuration: pinned Bazel reads the
caller ctx. Analysis method-table admission validates Target/None using existing
AnalysisConfiguredTargetValue and invokes AnalysisCallToken.require_active at
call time, including extracted bound methods. Default binding uses the existing
Starlark method parameter mechanism. No source-label heuristics, new DICE key,
cache, lock, provider side store, action, command flag or fallback. Existing
configuration structurally owns equality/invalidation; publication/lifetimes
follow docs/developers/dice.md and existing rule-context token contracts.
Both existing exec projections reset collect_code_coverage False; preserve
that owner unchanged and prove coverage-on target refusal versus exec False.
The result is a primitive and retains no context/evaluator memory.

Allowlist: Analysis src/starlark_rule.rs only context method/import/test
registration, src/starlark_rule/cc_context.rs only projection helper,
src/starlark_rule/cc_context/tests.rs only focused owner regression,
src/starlark_rule/context_lifecycle_tests.rs only bound-method lifecycle
regression, tests/subrule.rs only module registration andnew
tests/subrule/coverage_instrumented.rs; this manifest/canonical Current and
compact Stage6 acceptance note. Existing 2768-line starlark_rule.rs remains
cohesive for method registration; no new fixture/function family enters it.
Integration tests live separately. Production growth estimate under35lines;
no new representation or performance claim. Independent DESIGN precedes
implementation, FINAL gates acceptance. Baseline a16478dd5.

## Evidence and validation

Reuse pinned disabled-coverage and context-lifetime source regressions. Real
configured rule tests discriminate default/explicit disabled, omitted and
explicit None, positional/named rule Target and source-file Target, primitive
bool result, invalid scalar/list/struct/File/Label inputs, wrong arity/named
parameter and coverage-enabled explicit refusal. Configuration-off/on/off and
source A/B/A must use one DICE for both Legacy/Observed routes. Both exec
projections of an enabled target return False and target remains refused.
No fragment declaration or provider lookup is needed. Existing dependency
materialization protects Target identity. Owner tests cover missing structural
state and False/enabled handling. Existing malformed coverage Boolean validation
is inherited from Configuration's typed getter; add only if its evidence is
missing. Capture a bound method while active; invoking it after success/error
must fail via token, never return False. Keep zero actions and existing
root/subrule authorization controls. Subrule coverage_instrumented API is not
admitted by this packet (SubruleContextApi has a different surface).
Enabled upstream filter/provider assertions are deferred because this slice
refuses that phase. Reuse existing rules_cc bytes; no copied public fixture.

Resolve pinned nightly-2025-09-14; serialize Cargo. Compile Analysis unit and
subrule integration executables with JSON --no-run in separate60s preparation
bounds, batch exact nonignored preflight, then run focused selected tests.
Runtime expected under a few seconds. Compile named Core direct dependent and
rebuild slug_cli_v2 before authentic one-shot replay from durable
target/wp856/authentic-workspace, same cquery //:probe --output=label and same
three receipt hashes. Use supervised180s cap (operation cap, not user deadline)
justified by fresh public source preparation; smaller tests do not prove the
actual generated-toolchain closure advanced. Check slugd before/after in the
actual execution environment, supervisor cleans process group. Record first
new stop/success, fixture hashes and timing; full closure remains unclaimed.
Rustfmt/diff/archive/status checks; root integrates/commits accepted work.

REPLAN if the current configured context cannot own this Boolean/lifecycle
without new provenance or semantic state. Routine invocation/compile/test
corrections stay within this contract.

## Immediate predecessor

WP856 a16478dd5 accepts experimental_cc_implementation_deps getter/typed option
and exact textual Boolean null rejection; independent DESIGN/FINAL and26focused
tests pass. The byte-identical recovered authentic cquery clears the getter in
83.944s and next reaches ctx.coverage_instrumented at rules_cc cc_helper.bzl:304.
Raw evidence target/wp856. M7A and M8 remain unproved; Windows is deferred.

## Acceptance receipt (2026-09-30)

Independent DESIGN/FINAL ACCEPT. Twelve exact nonignored focused checks pass:
five Analysis unit/lifecycle and seven integration/protection tests, 0.496s
total runtime. The integration discriminator proves an enabled caller refuses
an explicit exec-disabled Target, rather than using dependency configuration.
Default/None/Target shapes, invalid binding/type, real same-DICE restoration,
both exec resets, zero actions and extracted-method expiry pass. Pinned serial
preparation: unit11.55s, integration5.56s, Corecheck33.67s, CLIbuild6.09s.
Rustfmt, diff, archive and scheduling checks pass. Production grows27lines,
with no new retained state, semantic owner, key or lock.

The rebuilt authentic `cquery //:probe --output=label` exits2 in82.989s,
clearing coverage_instrumented and reaching the separate
cc_internal.wrap_link_actions at rules_cc
cc/private/link/create_linking_context_from_compilation_outputs.bzl:105 via
cc_library.bzl:152. Original BUILD/MODULE/lock hashes remain unchanged; durable
WP856 recovered fixture is reused. Supervisor cleanup and actual-environment
process checks pass. Evidence is in target/wp857. The packet evidence window
starts07:02:20UTC and closes around07:14UTC (includes review/preparation/replay).
Full coverage filters/providers/actions, link actions/toolchain closure and
M7A/M8 remain unproved; no Windows execution claim is added.
