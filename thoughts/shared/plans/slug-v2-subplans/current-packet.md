# Current Slug V2 Work Packet

Packet: WP-7-68-m7a-compile-context-composition-r1
Status: accepted

## Outcome and demand

Run the unchanged compilation-context initializer with genuine ctx/configuration,
Files and feature/dependency contexts, then compose the public cc_common.compile
entrypoint up to the real compiler-action boundary. The allocator regular-source
path from the accepted Solib inventory requires this initializer before action
creation: compile.bzl284–314, helper404, then compile1237/1243 and1691. No new
upstream fixture body or oracle is needed; reuse the existing54-body corpus.
Predecessor WP767 at7016c3c29 admits configured compile-output declarations;
31 focused tests, Core/CLI and independent final ACCEPT. Its receipt is in Git.

## Compatibility and ownership

Exact admitted surface: current-caller restricted configuration method
is_sibling_repository_layout() for the supported nonsibling layout; genuine File
is_tree_artifact classification; unchanged initializer include ordering/header/
define/dependency relationships. Slug-native: fixed nonsibling layout policy,
structural configuration/root/artifact identity, tokens, Rust Host and diagnostics.
Sibling layout remains unsupported, not a missing CoreOptions default: Bazel's
flag belongs to Starlark semantics. Expose false for Slug's existing fixed layout,
with rejection evidence for the unadmitted flag; do not add a guessed native option
or an untracked true mode. Preserve the source-sensitive default allowlist.

ConfigurationValue keeps the complete immutable SlugConfiguration handle from
WP767; add no retained field or identity. File classification reads existing
AnalysisArtifact kind (Directory true, source/regular false), not a path suffix
or duck-typed attribute. No new artifact/configuration facade, cache, interner,
key, lock, filesystem read or semantic side store. Existing observed package/
configuration dependencies and ConfiguredNodeAnalysisKey own initializer results;
existing provider/HeaderInfo publication, equality cutoff and DICE lifetimes own
retention. Read docs/developers/dice.md. All new collection work is test scratch;
production representation and Stage9 WP766/WP767 dispositions are unchanged.

Deferred: sibling/Windows layout, virtual-header rewriting, module-map generation,
tree templates, DWO/LTO side outputs, whole compilation-output provider publication
(its generic temps closure is not admitted), full production toolchain, CppCompile
action/discovery/execution, Run and bootstrap. No no-op compiler action, fake source,
test-only production export or rewritten upstream body. A missing-action diagnostic
proves only the reached frontier; it does not prove compilation or action arguments.

## Pinned source and proof

Bazel9.2 source8220c6198837d5c13d53fea211cf3282aa12408a:
BuildLanguageOptions.java373–393 and skyframe/config/BuildConfigurationFunction.java77–82
own layout semantics; BuildConfigurationValue.java345–353/530–537 owns equality and
restricted getter. CcStarlarkInternal.java545–552 and Artifact.java589–591/872–874
own tree classification. StarlarkCcCommonTest.java5488–5534/5707–5715 provides include/
dependency-define outcomes;6451–6481 covers tree-input themes, not template admission.
The unchanged helper394–430/591–650 establishes ordinary/requested-system/external
include branches and public versus implementation dependencies. Public wrapper
cc/private/cc_common.bzl451–579 normalizes variables_extension; use it for full calls.
CcStarlarkInternal.java702–758 and CppCompileAction.java512–516 explain why a real
action/discovery owner, not generic Spawn, is the subsequent gate.

Required discriminators:
- Loading native tests: source/derived regular/Directory types; positional-only
  artifact binder (Param.java defaults named=false); named/forged/missing/extra
  arguments rejected. Restricted layout method
  has allowed, denied and missing-source contexts; preserve immutable coverage,
  genuine receiver and unsupported configuration publication.
- Genuine main/external target/exec configured initializer with nonempty public,
  private and textual headers, normal and implementation dependencies; verify
  include ordering/dedup, defines/local defines and dependency visibility. Exercise
  ordinary, system-requested and external feature paths without virtual prefixes.
- Publish full returned contexts/CcInfo to a configured consumer; same-DICE A/B/A
  for source-defined metadata/dependency and configuration changes, including an
  unused config input. Preserve hidden HeaderInfo structure, not just a string view.
- Public header-only compile succeeds with genuine toolchain/headers; inspect
  returned output fields locally and publish context/observations, not its closure.
  The same public call with a regular .cc fails at missing create_cc_compile_action.
  This is explicit prerequisite/frontier evidence, not allocator compile success.
- Unadmitted sibling flag is rejected by actual option owners. Existing roots,
  compile-inputs, toolchain, declarations and restricted-native tests stay protected.

## Scope, routing and validation

Independent design review ACCEPT on 2026-09-21 freezes this contract; final
acceptance requires the actual implementation and all named proofs below.

Base7016c3c29; preserve candidate on codex/wp768-compile-context-review.
Implementation worker: Loading cc_common.rs, cc_compile_outputs/configuration.rs,
cc_compile_outputs/tests.rs (or its child tests); native option-rejection assertion
may live in those tests using existing public constructors. Thin methods only:
existing large cc_common dispatch retains ownership, no new central algorithm.
Integration worker: new tests/rustc_map_each/compile_context.{bzl,rs}, mod.rs;
existing authored cc_toolchain fixture may gain narrowly necessary feature inputs.
Finishing Loading worker: compile_context/registry.rs fixture adapter, reusing
existing selected-registry metadata/materialization observations for unchanged
source bodies. Main-origin local-override cross-package loading remains deferred;
the existing selected-registry route supplies the genuine main ctx proof instead.
No production loader change, network call, new upstream body or fake ctx.
Registry metadata and materialization-success inputs are explicitly synthetic
test transport inputs (including sha256-test); this is not archive-download or
integrity verification evidence. Corpus bytes still use existing pinned sources.
Root: app/slug_commands_v2/tests/sibling_layout.rs command-capture rejection proof,
scheduling and Stage6/bootstrap closure, serial validation; reviewer:
independent design and final actual-diff/evidence review, no writes. Production
estimate40–90 lines plus400–650 test lines; growth is review trigger, not partial landing.

Resolve pinned nightly tools; serial offline/locked Cargo JSON no-run for Loading
and Analysis integration, plus the Commands rejection target; batched exact
preflight and direct selected tests.
Core/CLI dependent checks, format/diff/archive/status checks and independent final
ACCEPT precede atomic integration. Preparation bounded60s/operation; runtime
seconds estimates are not new user deadlines. No remote/daemon/Bazel execution gate.
REPLAN for unmodeled layout identity, publication/ownership change, or need to
activate a deferred action branch; correct ordinary fixture/compiler failures
inside this contract. M7A remains partial and M8 unproved.

## Closure receipt (2026-09-22; accepted)

Base7016c3c29; candidate on codex/wp768-compile-context-review. Independent design
ACCEPT and independent final ACCEPT confirm the context-composition contract,
actual publication/restoration and precise action-frontier evidence.
Production adds39 lines over unchanged retained representations. Test growth
exceeds the initial estimate because the selected-registry adapter is251 lines;
independent review accepted that bounded fixture ownership rather than a new loader.
All54 upstream bodies remain unchanged. Only authored toolchain features were added.

37/37 exact-selected tests pass: Loading16 (three new boundary tests plus13
protected), configured20 (four new composition tests plus16 protected), Commands1
(actual build flag capture rejection). Receipts under target/wp768:
tests-1790035320535074564 (Loading), tests-1790035843044137272 (configured),
tests-1790035222373931946 (Commands), all exit0/selected=pass. The configured
frontier assertion requires the exact missing cc_internal.create_cc_compile_action
attribute diagnostic, not just an incidental function name in a traceback.

Pinned nightly2025-09-14 offline/locked Cargo JSON no-run and Core/CLI checks
pass. Preparation/check48.673s total, max12.788s per operation; latest successful
tests2.148s. Packet elapsed
approximately19 minutes including research/design/review. One compile correction
removed a duplicate Arc wrapper; one preserved runtime failure identified the
missing pinned builtin rules_cc0.2.17 metadata request. Only that explicit registry
version was added; selected1.0 corpus bytes did not change. No assertion waiver,
new loader surface, network, daemon, Bazel, remote or compiler action invocation.

Genuine main/external target/exec initializer contexts and full CcInfo/HeaderInfo
publication now pass, including ordinary/implementation visibility, include order,
dependency edit/restoration and unused-configuration-payload restoration. Root
BUILD changes only; selected materialized fixture corpora remain immutable.
Header-only public compile succeeds; regular .cc reaches the real missing action
boundary. Synthetic transport observations do not prove archive verification.
Formatting, diff, archive and matched packet-status checks pass. M7A is still
partial; CppCompile/discovery and M8 remain unproved.
