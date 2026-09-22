# Current Slug V2 Work Packet

Packet: WP-7-76-m7a-template-variable-provider-r1
Status: accepted

## Outcome and compatibility

Admit genuine platform_common.TemplateVariableInfo construction, Target lookup,
direct/nested publication and consumption of authentic rules_cc make-variable
helper outputs. This removes one full cc_toolchain prerequisite; full rule
construction, build-info status transforms, embedded helpers and execution remain
deferred. Predecessor WP775 at833968c36 accepted PackageSpecificationInfo with73
tests and Core/CLI/Query checks; its complete receipt remains in Git. M7A partial,
M8 unproved.

Exact: one required vars dict argument, positional or named; strict string keys
and values, construction snapshot preserving input order; immutable provider,
occurrence equality/hash, type TemplateVariableInfo, sole field variables,
repr struct(variables = {...}). Each variables read returns a fresh mutable dict
copy, including from frozen providers. Constructor is the native provider key.
Aliases stay equal and distinct constructors stay unequal even for equal maps.
Slug-native: structural publication/configuration identity and diagnostic text;
raw Java identity-hash bytes and source-location diagnostic parity not claimed.
Deferred: make-variable expansion/merging into ctx.var, full toolchain envelope,
unrelated provider surfaces and execution. No placeholder provider or bypass.

## Source and ownership

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
TemplateVariableInfo.java43–47,63–75; TemplateVariableInfoApi.java43–69;
NativeInfo.java58–59,125–137; PlatformCommon.java43–45, BuiltinProvider.java94–96;
Printer.java239–258 native repr escaping (the generic retained Rust formatter
escapes Latin-1 differently, so this facade owns its bounded exact quoting).
Mutable-copy chain Eval.java655–661, MethodDescriptor.java240–246/315,
Starlark.java216–217, Dict.java456–490. Adapt TemplateVariableInfoTest.java34–149
producer/forwarding/field tests; its make-variable expansion is a deferred phase.
rules_cc0.2.17 cc_toolchain.bzl124–146 requires constructor136 after unchanged
cc_helper make-variable477–515/CC_FLAGS791–812 helpers. Reuse authenticated WP773
corpus; no new upstream bodies or Java probes.

BuildAPI owns TemplateVariableInfo as one Arc-owned immutable ordered SmallMap
of CompactString pairs. Arc allocation is native occurrence identity: new always
allocates, clone/Dupe shares. Native Eq/hash use occurrence only. Publication
compares complete ordered entries plus a two-way occurrence mapping in existing
PublicationEqState; same payload with different alias topology is not equivalent.
Typed ProviderValue and AnalysisValue variants share the same handles through
direct/nested roots; no generic structural-provider substitution.
Loading owns constructor/facade and strict binder, fresh evaluator-local mutable
dict copies. A zero-sized native constructor token replaces only the old generic
TemplateVariableInfo token: type Provider, native function repr and stable key
Eq/hash across repeated platform_common accesses. Other constructors are unchanged.
Analysis only lowers/materializes typed handles and decodes returned
providers. Existing configured-node/Bzl DICE dependencies own caching/invalidation,
Need/errors and restoration. No DICE key, lock, counter, registry, interner, cache,
filesystem discovery or evaluator borrow in retained values.

Memory is DICE-retained immutable data; construction/dict copies/bijection maps
are evaluation/comparison scratch. Existing publication cutoff, graph invalidation,
cancellation and ownership release apply. Reuse Stage9 Arc/Dupe/CompactString/
SmallMap/Allocative and occurrence-bijection dispositions (WP758/762/763/765),
not donor semantic code. No performance or memory-reduction claim.

## Scope and routing

Base833968c36; codex/wp776-template-variable-review. Independent design ACCEPT
before production edits; actual-diff/evidence final review before integration.
Worker1: app/slug_build_api_v2/src/template_variable_info.rs and child tests;
lib.rs exports, analysis_value.rs closed variants/bijection, providers/mod.rs
typed payload; app/slug_loading_v2/src/provider/template_variable_info.rs and
child tests, provider.rs constructor/key dispatch, lib.rs exports. package.rs
only if existing platform_common token wiring requires it.
Worker2: app/slug_analysis_v2/src/analysis_value.rs typed lower/materialize;
analysis_value/template_variable_tests.rs lifecycle tests; starlark_rule.rs
returned-provider bridge; tests/rustc_map_each/compile_context/provider.bzl and
provider.rs authentic helpers/native provider proof.
Root: tests/template_variable.rs and tests/starlark_rule.rs module hook; Stage4/
Stage6/bootstrap/Stage9 owner and scheduling documents. Parent production files
are large: only closed dispatch/wiring there, semantics in cohesive child modules.
Estimate250–450 production and350–650 test lines. Additional files or growth need
root reassessment; no dependencies or copied fixture bodies planned.

## Acceptance and validation

Cover constructor ABI/type negatives, empty/Unicode/order/reflection, native
occurrence equality/hash in dictionary/depset, provider immutability versus fresh
mutable dict reads, source-dict mutation snapshot, loading/frozen construction.
Freeze/GC and source-heap disposal/rematerialization preserve direct/nested aliases
and complete data, including aliases across native and user-provider roots.
Same-DICE payload/order A/B/A and alias split/merge invalidate consumers; missing
source/recreate restores through existing owners, not fresh graphs.
Authentic unchanged make-variable helpers feed native construction and downstream
Target indexing in existing main/external target/exec toolchain/compile proof.

Root serializes freshly resolved pinned nightly offline/locked Cargo JSON
--no-run BuildAPI/Loading/Analysis tests; exact batched nonignored preflight and
actual counts. Protect WP775 native allowlist, helper/compile tests and existing
native publication/lifecycle selectors; Core/CLI direct-dependent checks.
Preparation60s/operation, small runtimes expected seconds (operation caps, not
user deadlines). Rustfmt, diff/archive/plan-status checks. No daemon, Bazel/JVM,
compiler action, CLI invocation or network required. REPLAN only if bounded
ownership or pinned semantics contradict the contract; ordinary corrections stay
inside the packet.

## Acceptance evidence — 2026-09-22

Freshly resolved nightly-2025-09-14 tools; serial offline/locked Cargo JSON
--no-run builds, batched exact nonignored preflights and actual execution:
86 selected /86 passed in5.832s aggregate. BuildAPI9 (0.009s), Loading10 (0.039s),
Analysis library18 (0.114s), integration49 (5.670s). Receipts and exact commands
under target/wp776: tests-1790048502948187022,1790048893466275492,
1790048927080220404,1790048986139665072.receipt.json. Integration retains all47
WP775 selectors plus two native template producer/forwarder/invalidation tests;
the four existing helper tests now also prove genuine template construction.

Final no-run receipts: compile-slug_build_api_v2-1 exit0/3.225s;
compile-slug_loading_v2-3 exit0/7.930s; compile-slug_analysis_v2-4
exit0/13.084s (library+integration), then -5 exit0/6.327s (integration-only after
test-only missing-source expectation correction). Core/CLI check1 exit0 in
16.787s/17.441s. Nine compile attempts101.196s; both checks34.228s;
all runtime attempts12.544s. Every preparation stayed below60s. No Bazel/JVM,
daemon, compiler action, CLI invocation or network. Rustfmt, diff/archive/status
checks pass. Review branch created03:34:57Z; source research preceded that point.

Discriminators: ordered maps and direct/nested occurrence aliases, equal-map
distinct construction and native hash/depset behavior, immutable input snapshots
versus fresh mutable reads, exact type/key/field reflection, all constructor
binding negatives; freeze/source-heap disposal/GC; same-DICE value/order and alias
split/merge A/B/A, missing source Need and recreation. Genuine unchanged helper
composition checks every ordered variable, global-over-additional collision,
main/external paths and target/exec configuration, downstream lookup and metadata
restoration. No new upstream copied body or semantic owner.

Corrections preserve scope: test FreezeError formatting, Dupe import and explicit
CompactString inference; local native repr quoting fixes a demonstrated generic
runtime Latin-1 mismatch using Printer.java without altering that runtime.
Main CROSSTOOLTOP expectation is source-derived '.' rather than empty. The
same-DICE fixture uses ASCII identifiers to avoid conflating source representation
with its order/alias proof; native/retention/GC tests retain Unicode coverage.
Missing .bzl input asserts the existing harness's exact Need outcome rather than
an invented path-bearing error. All affected evidence was refreshed; no failure
waived. About320 net production lines in cohesive children/small wiring and775
test lines; test growth exceeds650 estimate for independent native-key/reflection,
cross-root/container aliases, GC and exact helper-map discriminators.
Independent final actual-diff/correction/terminal-evidence review ACCEPT at
03:51Z, about16min after review-branch creation. Initial research and interleaved
reviews were not separately timed; no workflow speedup claim or REPLAN.
