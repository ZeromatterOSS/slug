# Current Slug V2 Work Packet

Packet: WP-7-70-m7a-cpp-command-expansion-r1
Status: accepted

## Outcome and compatibility

Expand a genuinely published CppCompile recipe into complete ordered argv and
effective environment with explicit client-environment inputs. Pure BuildAPI
projection only, not compiler execution, aquery or REAPI admission. Predecessor
WP769 at338aa339f accepted construction/publication with70 tests and Core/CLI
checks; reuse its54 unchanged upstream bodies and selected-registry caller.
Full predecessor contract/receipt is pinned in Git.

Demand: authenticated rules_rust allocator BUILD21–28 declares allocator_library.cc;
unchanged compile.bzl1691–1715 produces the admitted recipe. Commands are an
execution prerequisite. Solib inventories omit compile argv; inert constructor
fixtures do not establish production allocator flags/toolchains or execution.
No additional policy breadth is inferred from absent observations.

Exact admitted algorithms follow pinned Bazel9.2
8220c6198837d5c13d53fea211cf3282aa12408a:
CompileCommandLine.java52–58/76–160, CppCompileAction.java979–1001/1245–1264,
CcToolchainVariables.java lookup/adaptation and Builder801–820,
ActionEnvironment.java123–131. CompileCommandLineTest126–196 and
CcToolchainFeaturesTest257–467/1975–1988 supply regression themes; reuse accepted
Variables/feature tests. Filtering/paramfile/module tests stay outside this slice.
Exact argv order and environment key/value resolution use NOOP path mapping,
no execution metadata, no paramfile and always-pass copts filtering.
Slug-native: structural identity/path tokens, Rust valid-Unicode and diagnostics.
Existing canonical environment maps do not promise LinkedHashMap iteration order.

## Semantic owner and design checkpoint

CppCompileSpec remains the immutable recipe in existing configured-node DICE
results. Add typed expand_command_line() and effective_environment(client_env:
&CanonicalStringMap) methods with feature-expansion errors. Reuse the feature
engine through a BuildAPI-private retained CcVariableAccess over AnalysisValue/
CcToolchainVariables. No Starlark heap round-trip or arbitrary stringification.
Support published counterparts of Loading adaptation, direct/longest-prefix
lookup, local None shadowing, sequence/scoped iteration, bool/File/structure
conversion and lazy errors. Preserve native structure restrictions: not every
retained provider/value is automatically a native structure.

Crucial: getOverwrittenVariables returns fresh empty Variables even without
modules; CompileCommandLine wraps the original as parent of a fresh empty scope.
Argv uses that fresh dotted cache over original direct bindings, not original
dotted seeds. Environment uses original Variables/cache seeds. Model the
distinction in per-call scratch, without new retained occurrence/global cache
or published mutation. Repeated calls/errors preserve recipe/provider equality.

Argv is selected tool then ordered action-config/feature flags. No implicit -c,
-o, dependency flags, shell quoting or joining: those come from features.
Environment resolves fixed entries then only declared inherited names from the
explicit client map (missing names omitted), PWD=/proc/self/cwd unless execution
requirements contain requires-darwin (presence, not value/Host), then feature
entries overwrite. Duplicate feature keys remain errors before merging.
Never read ambient environment. This is not Starlark Action.env. No client env
enters analysis identity; future execution must obtain it through existing
immutable request/environment ownership.

Expansion buffers/caches are call-local scratch released on success/error;
returned vectors/maps belong to callers. Shared recipe fields stay Arc/Dupe/
Allocative; use existing SmallMap/CompactString/immutable handles. Stage9 WP760/
WP769 reuse disposition applies. No donor, new DICE key, filesystem observation,
interner, lock across compute, or performance/memory-reduction claim.

## Exclusions

Keep generic ActionSpec.render_argv (legacy/Spawn preview), CppCompile aquery,
ActionKey, REAPI command/input tree and Core prerequisite/staging/execution
guards closed. Typed expansion is not a fixed-input Spawn or cached execution.
Discovery, modules/overwrites, compiler/Args paramfiles, copts regex, tree/path
mapping, full toolchain producers and production allocator argv remain deferred.
Preserve WP769 construction guards; no constructor-time expansion.

## Scope, routing and proof

Base338aa339f; candidate codex/wp770-cpp-command-expansion-review. Independent
design ACCEPT on2026-09-22 precedes implementation; final actual-diff/evidence ACCEPT precedes
atomic integration. Root owns scheduling, small action projection, serial
validation and closure. Bounded workers own retained adapter, action/integration
tests, and independent source/design/final review. Concurrent edits are disjoint.

Allowlist: BuildAPI cc_variables.rs thin hook and cc_variables/owned_access.rs
with child tests; actions/cpp_compile.rs thin hook and cpp_compile/expansion.rs
with child tests. Existing cpp_compile/tests.rs may expose its data constructor
only to sibling tests and update unchanged generic-refusal diagnostic assertions;
no test-only production API. Analysis
tests/rustc_map_each/compile_context.rs thin child hook,
compile_context/command_expansion.rs and authored lifecycle.bzl/registration.rs
only for discriminating published-command proof; authored cc_toolchain.bzl may
add default-preserving feature/env test inputs. Relevant BUILD source declarations
only if recursive globs do not cover children. Root docs: Stage6/9, bootstrap,
canonical and manifest. No copied corpus or unrelated transport/production edits.
actions/spec.rs may clarify the unchanged generic-preview refusal diagnostic;
typed expansion does not activate that legacy/Spawn preview surface.
Estimate350–500 production and300–500 test lines; growth triggers responsibility
review. Central files get hooks; owned adapter is a bounded Loading counterpart,
not another feature engine. Shared algorithm extraction needs design reassessment.

Required discriminators:

- Complete argv from unchanged public C/C++ compile, PIC/non-PIC and main/external
  target/exec roots; tool first, action/feature ordering, duplicates/empty strings.
- Stale original dotted sequence versus fresh argv overlay, scoped iteration,
  longest prefix, None shadowing, lazy unused invalid metadata versus used errors.
- Fixed/inherited/missing env, undeclared client values ignored, Darwin-key
  presence, PWD/feature precedence, duplicate feature errors.
- Repeat expansion leaves recipe/cache/provider identity unchanged; errors occur
  after successful construction. Same-DICE A/B/A yields changed/restored argv.
- Preserve generic command/REAPI/Core refusals and native construction/publication/
  order/conflict/GC, feature expansion and Variables-cache proofs.

Pinned nightly tools; serial offline/locked Cargo JSON no-run BuildAPI/Analysis,
exact batched preflight/execution, Core/CLI checks, Rust formatting and diff/archive/
status checks. Reuse unaffected WP769 evidence; run affected projection/refusal
assertions. Preparation capped60s/operation; seconds-scale test estimates are not
user deadlines. No Bazel/daemon/remote/compiler action invocation selected.
REPLAN for unowned execution inputs, contradictory source, required discovery or
new semantic owner. Ordinary implementation/test corrections stay in contract.
M7A remains partial; M8 unproved.

## Acceptance evidence — 2026-09-22

Observable gate: published regular CppCompile recipes now expose complete typed
argv and effective environment without changing construction, structural identity
or execution admission. Pinned-source regressions implement the source contract
above; no new Bazel observation, upstream corpus body or external compiler run.

42 exact nonignored selectors pass: BuildAPI26 (6 retained-adapter,5 typed
projection,7 protected construction/refusal,8 feature-engine) and Analysis16
(3 new complete-command/lifecycle,10 protected compile/context,3 Variables-cache).
The new command matrix covers12 public recipe cases, plus nonvacuous native
three-action publication. Fresh argv/stale original caches are distinguished at
adapter, typed projection and real publication boundaries. Existing WP769 GC,
Core/REAPI refusal and unrelated transport evidence is reused; those owners are
unchanged. Core/CLI direct-dependent checks pass.

Receipts: target/wp770/tests-1790040597251394662.receipt.json (BuildAPI26),
tests-1790040559632654034.receipt.json (Analysis16), compile-slug_build_api_v2-3,
compile-slug_analysis_v2-2, check-slug_core_v2-1 and check-slug_cli_v2-1 receipts.
Pinned nightly/offline/locked Cargo JSON preparation:7 attempts,52.595s total,
16.652s maximum; successful selected tests2.381s total,0.270s maximum. Ordinary
corrections: test imported CanonicalStringMap from the wrong crate; saved-error
assertion compared prefixed Display rather than raw payload. Neither changed the
contract. Rebuilt/repreflighted affected executables; no unaccounted failure.
Rust format, diff, archive and plan-status checks pass.

Growth review:362 lines in two focused production modules, with thin owner hooks;
test modules exceed the initial estimate (861 lines) because adapter semantics,
typed composition and actual configured publication require distinct proofs.
No copied feature engine or retained representation. Independent review covers
this responsibility split. Independent final ACCEPT follows actual-diff and
terminal-evidence review. Recorded implementation/validation interval starts
01:15:46 UTC; acceptance at01:31:23 UTC (~16 minutes), excluding prior
research/design work. Review time was concurrent and not separately measured.
All exclusions and M7A/M8 status remain unchanged. Next selection must resolve
the demanded CppCompile discovery/execution prerequisites before any activation.
