# Current Slug V2 Work Packet

Packet: WP-7-78-m7a-build-info-transform-registration-r1
Status: accepted

## Outcome and compatibility

Construct the unchanged pinned bazel_cc_build_info rule with genuine status
inputs, native typed TranslateBuildInfo actions and its bare OutputGroupInfo.
Prove volatile-transform, stable-transform, redacted-Symlink registration order,
non_redacted=[stable,volatile] and redacted=[redacted] provider order. Feed that
real OutputGroupInfo into the existing authentic toolchain-helper/public compile
proof; no empty-provider or nostamp shortcut.

Exact admitted slice: both private three-required-named-argument binders, current
caller allowlist and live actions guard, genuine template File and package/root
output ownership, ordered status/template inputs, singleton output, mnemonic,
stable ordinary versus volatile constant metadata, unconditional-volatile policy,
genuine bare Info provider returns through existing lowering/publication.
Only the two unchanged capture-free C++ callbacks are admitted, authenticated
from complete pinned source and native callsite. No callback runs in analysis.
Slug-native: full structural configuration/root/action identity and diagnostics.
Deferred: arbitrary callbacks, Java build-info semantics, actual status bytes/
producer/command/host/time/env, rendering/execution/cache policy, ActionKey/aquery,
installed module tools, full cc_toolchain and bootstrap. Execution must fail
before source acquisition, writes or RPC. M7A partial; M8 unproved.

Immediate predecessor WP777 at788c153ce owns workspace-global immutable status
descriptors under a pure workspace-only DICE key;87 tests/CLI/design+final review
passed. Those descriptors confer no production/execution authority.

## Evidence and decisions

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
StarlarkActionFactoryApi.java1135–1211; StarlarkActionFactory.java475–529;
BuildInfoFileWriteAction.java72–94,118–220; BuiltinRestriction.java34–44,170–180;
StarlarkRuleContextTest.java4773–4855; BuildInfoFileWriteActionTest.java199–358.
Authentic tools/build_defs/build_info/bazel_cc_build_info.bzl17–79 SHA256
72cc98ddc39e4c7c66d23541c7b18e10b70fde6008e2883144554b7487e97514.
Its imports are empty and both callbacks have no captures. Calls36/37 directly
select their own immutable module-global defs. rules_cc cc_toolchain.bzl119,
339–342 demands its OutputGroupInfo. Rule return classification follows
StarlarkRuleConfiguredTargetUtil.java247–278; None/tuple sequence breadth may
remain deferred, but genuine bare native/user Info is not an arbitrary struct.

Reuse Loading cc_toolchain_auth's canonical identity + observed source digest +
evaluated complete source digest + exact source callsite authentication.
Normal caller allowlist remains separate. Require genuine Starlark function type,
but never authenticate by name/repr/hash alone. The unchanged complete source
fixes the actual callback expression, module globals, zero captures and applicable
fixed evaluator semantics. Only @@bazel_tools//tools/build_defs/build_info:
bazel_cc_build_info.bzl calls36/37 are admitted. Modified/copied source, other
callsites and arbitrary functions fail closed. Retain finite Info/Version recipe
plus authenticated canonical source/digest; do not serialize callback values,
retain frozen heaps, mint callable identities or evaluate substitutions early.
This is a bounded exact source adapter, not a fallback implementation.

Integration exposed an existing missing owner dispatch for source File targets
in builtin repositories. Design delta independently accepted: analysis-only
source classification selects Builtin from the tracked repository route and
computes the existing BuiltinBazelToolsSourceFileKey with its snapshot and exact
relative path. Propagate catalog/kind/integrity errors; preserve declaration and
visibility checks plus canonical Source labels. Reuse route dependencies; no
new DICE key or fabricated ResolvedPath/host metadata. Public runtime observed
path resolution and materialization remain unchanged and fail closed.

Direct root/external instantiation also exposed the existing root-origin builtin
module route's empty mapping for zero-import files. Design delta2 independently
accepted: promote only the genuine builtin C++ build-info source with zero imports
through the existing canonical route owner. Preserve the existing single-import
promotion and observation/Need/error flow. No self-only map injection or blanket
zero-import promotion; unrelated, Java and copied source identities stay excluded.

BuildAPI owns immutable Arc/Dupe/Allocative BuildInfoTransformSpec: status,
template, rooted output, finite callback source/role, volatility. Equality
compares every structural input through existing ActionSpec publication; status
root retains workspace and output root full configuration. Add orthogonal
ActionOutputMetadata::{Ordinary,Constant}; regular constructor remains Ordinary,
constant constructor only regular File. Full Eq/Hash/Ord include metadata.
Declaration registry retains metadata with kind, refuses incompatible
redeclarations, and does not reconstruct ordinary policy from path/kind.
Registry permits constant output only in typed matching BuildInfo transforms;
legacy marker-only actions and other families cannot claim it. No weakening of
existing root/declaration/conflict checks. No exact key claim from source digest.

Analysis uses existing per-evaluation sink/registry to register immutable specs
in source order. The sink shares the real WP777 descriptor pair. No new DICE key,
global cache, interner, evaluator borrow, pending callable buffer or lock across
compute. Existing configured-node dependencies track modules/templates/config;
DICE publication/cancellation/lifetime and Arc owners bound retention. Short path
construction, authentication hashing and return normalization are phase scratch.
Retained utility policy inherits Stage9 structural artifact and finite callable
rows; reuse existing SmallMap, Arc/Dupe/Allocative, no dependency/donor import or
performance claim. Read docs/developers/dice.md before ownership edits.

Port installed build_info package and templates verbatim from upstream
BUILD.tools (as BUILD), both .bzl files and all six template files. The Java .bzl
is required by unchanged package loading but its transform callsites remain
unsupported. Extend builtin catalog/provenance through its normal owner; no
authored replacement BUILD or duplicate oracle corpus. Focused tests use those
same checked-in bytes. No JVM/Java semantic runtime or probe.

## Scope and routing

Base788c153ce, codex/wp778-build-info-transforms-review. Independent design before
production edits and actual-diff/evidence final review. Large parent files get
closed variant/wiring hooks only; new semantics and tests in child modules.
Estimate500–950 production,500–1000 tests, plus verbatim upstream files.
Any additional file or ownership choice requires root reassessment.

Worker1 (Loading):
- src/build_info_transform.rs + child tests; lib.rs.
- src/bzl_module.rs finite existing-route promotion hook for the pinned source.
- src/subrule_invocation.rs trait/default + two thin method hooks only.
- existing builtin_restriction/source identity APIs reused unchanged.
- Root adds Loading BUILD.bazel test compile_data for the single shared upstream
  C++ callback source; no duplicate source fixture.
Worker2 (BuildAPI + Analysis):
- BuildAPI src/actions/build_info_transform.rs + child tests;
  actions/{mod,spec,registry,ctx_actions,reapi_projection}.rs plus existing
  projection tests; lib.rs.
- Analysis src/starlark_rule/build_info_transform.rs + child tests;
  starlark_rule.rs sink field/declaration metadata/bare-provider normalization;
  context_lifecycle_tests.rs/cargo_runfiles_tests.rs fixture initialization;
  solib_tests.rs fixture field initialization only;
  result.rs typed accessor and Ordinary-only FileWrite view guard.
- Analysis src/dice/source_file.rs existing route helper reuse and private
  analysis classification; dice.rs thin source-target dispatch. Focused source
  route/catalog negatives in a child test module or existing integration child.
Root:
- Bzlmod builtin/bazel_tools/tools/build_defs/build_info files named above,
  src/builtin_repository.rs, tests/builtin_bazel_tools.rs, builtin provenance
  metadata; BUILD.bazel compile_data and the two normal catalog BUILD.bazel
  sidecars (excluded from installed bytes).
- Analysis tests/rustc_map_each/compile_context/provider/build_info_transform.rs included by existing provider proof
  module to reuse its genuine selected-registry harness without duplicating it;
  tests/rustc_map_each/compile_context/provider.{rs,bzl} genuine dependency proof.
- Core runtime/action_prerequisites.rs plus focused child test; relevant resolved
  action/formatter narrow refusal branch only if exhaustive match requires it.
- Core runtime/configured_action_closure.rs Ordinary-only scalar FileWrite guard.
- REAPI input_tree.rs/command.rs/executor.rs plus focused tests; generic command refuses
  typed transform. CLI dependent check covers every new enum consumer.
  Public raw projection/input-tree and configured-output materialization APIs
  independently reject constant metadata; they cannot rely on registry checks.
- Stage6/9/bootstrap and scheduling records.

## Gates and validation

Prove genuine installed rule action/group contents with real status/template
artifacts, ordinary/constant output policy and publication identity; bare-provider
native/user success versus struct/nonprovider rejection; cross-role/workspace/
template/output/source/config equality, path/conflict and ordered atomic
registration. Main/external target/exec and A/B/A in one DICE; authentication
accepts unchanged authority and rejects modified/copied/missing provenance in
the Loading harness. Installed builtin source is immutable catalog content, not
a mutable DICE input. Escaped action expiry and post-evaluation/GC
publication, no eager callback. Protected WP777 status/root/lifecycle gates and
authentic helper compile/template-provider proof. Negative binders include
positional/missing/unknown args, nonfunction/template, nonallowlisted caller,
wrong source/callsite and invalid output. Execution/REAPI refuse before side
effects; existing ordinary action families and constant-policy rejection remain.

Pinned-source regressions suffice; no new Bazel corpus unless a demonstrated
gap needs an oracle. Reuse installed bytes, no copied proof bodies. Serial
freshly-resolved pinned offline/locked Cargo JSON no-run for owner crates and
Bzlmod integration; batched exact preflight + actual passes; CLI check; rustfmt,
diff/archive/status checks. Preparation60s/op, runtime seconds expected. No
daemon/Bazel/network/compiler action required. REPLAN only if source authority,
metadata/declaration integrity or complete semantic publication cannot be owned
under this finite construction boundary; ordinary implementation corrections
remain in packet. Do not claim real status production or bootstrap completion.

## Acceptance receipt

Independent design, both route-design addenda, and actual-diff final review:
ACCEPT (/root/bazel_solib). Root integrated disjoint Loading, BuildAPI/Analysis,
integration and documentation work using existing Arc/SmallMap/DICE owners.
No new retained callback owner, heap, cache, interner or execution capability.

120/120 unique exact nonignored tests selected and passed, all exit0:

| Gate | Passes | Local receipt IDs under target/wp778/tests-*.receipt.json |
| --- | ---: | --- |
| Bzlmod catalog/provenance | 11 | 1790051558332208743,1790051567980455540,1790051586523299613 |
| BuildAPI identity/registry/status | 19 | 1790051735792093978,1790051758720337038 |
| Loading binders/auth/callsite/GC/routes | 24 | 1790052512712799482 |
| Analysis publication/sink/lifecycle | 10 | 1790052553353632963 |
| Configured installed/helper/main/external/target/exec | 22 | 1790052543337867363 |
| Core prerequisites/source/closure refusals | 28 | 1790052588532392529 |
| REAPI binding/projection/materialization refusals | 6 | 1790052640757010775 |

Fresh pinned nightly-2025-09-14 tools; serial offline/locked Cargo JSON no-run,
batched exact preflight then individual execution. Final compile receipts:
slug_build_api_v2-2, slug_bzlmod_v2-1, slug_loading_v2-5,
slug_analysis_v2-6 (--lib --test starlark_rule), slug_core_v2-3,
slug_reapi_v2-2. check-slug_cli_v2-2 exit0/10.129s. Commands use
`cargo test --offline --locked -p <crate> --lib --no-run --message-format=json`
(Bzlmod also --test builtin_bazel_tools), and `cargo check --offline --locked
-p slug_cli_v2 --message-format=json`. No CLI execution or daemon smoke required.

All 10 installed files byte-match pinned upstream; 59-entry manifest digest
e45427b34b6d4cb171c4d4440282a80045f79d37b836cf6c3ddefa05f02ea8af.
Named rustfmt check, diff check, archive check and plan-status check pass.
Two invocation corrections (integration target name and selecting a fixture child
as a standalone target) and test-fixture mapping/diagnostic corrections retained
their failed receipts. Actual integration exposed the two owner-route gaps above;
both were corrected under independent design review, not bypassed. Input-set
deduplication and stronger callsite/GC/bare-return discriminators completed the
original invariants. No REPLAN or ceiling increase.

Branch creation to final acceptance about30min; 21 preparation operations total
388.983s, maximum56.779s (60s preparation cap); final selected runtime2.767s,
slowest0.484s. Review ran concurrently and was not separately timed.
Observable gate advanced: unchanged installed C++ build-info construction feeds
real output groups into the authentic toolchain-helper/public-compile proof.
M7A remains partial and M8 unproved. Next selection must address a remaining
authentic full-toolchain/bootstrap prerequisite; status producer/rendering and
execution/cache-policy admission remain separate work, not implied by this receipt.
