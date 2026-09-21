# Current Slug V2 Work Packet

Packet: WP-7-63-m7a-complete-cc-toolchain-publication-r1
Status: accepted

## Observable outcome and compatibility

Run the complete unchanged rules_cc 0.2.17 CcToolchainInfo initializer, publish
every provider field (all51 required inputs, extra_cpp_configuration=None),
and consume that same provider through unchanged
configure_features and rules_rust get_linker_and_args. Real Cpp configuration,
native features/Variables, Files/depsets and CcInfo/compilation contexts replace
the preceding authored toolchain envelope. This is required before an honest
toolchain-dependency CppCompile route in the finite bootstrap closure.
WP762 at9e753ebe3 accepted full HeaderInfo/compilation-context publication with
33 focused tests, Core/CLI checks and independent review; receipt is in Git.

Exact: unchanged constructor field relationships and three callable methods;
four authenticated recipes (global SupportsPic, fresh AlwaysPic lambda,
fresh static-runtime closure, fresh dynamic-runtime closure); function alias/
occurrence equality, hash consistency, keyword-only invocation and captured
depset behavior. Slug-native: existing artifact/configuration/path domains, Rust
Host observations and Cpp fragment occurrence identity over full structural
configuration (not JVM FragmentFactory cache identity). Unsupported/deferred:
generic callable serialization, unpinned/forged callable recipes, runtime captures
other than None or admitted depsets, full get_cc_toolchain_provider/rule/FDO
production, compile/link actions, scanning and execution. Full constructor is
not full toolchain rule analysis or bootstrap acceptance. M7A partial, M8 unproved.

## Source and architecture

Bazel9.2 revision8220c6198837d5c13d53fea211cf3282aa12408a:
StarlarkFunction.java:251–274 and SymbolGenerator.java:139–160 establish function
occurrence identity/hash/immutability. Fragment.java:38–52 establishes immutable
hashable fragments; FragmentFactory's required-option cache/object identity is
explicitly not reproduced. Pinned unchanged cc_toolchain_info.bzl:19–40,97–103,
143–153 defines callable bodies/captures; complete initializer has51 required
arguments and optional extra_cpp_configuration. Corpus SHA256:
f19589572147b7dc8f1b16ab96791b7651923c36821aed70868a74bbfce963f5.
Existing54-body fixture and sources.json pins remain unchanged.

Authenticate the initialized-provider definition at provider() using canonical
module identity, observed SHA256, actual evaluated full source digest and exact
definition callsite. The pinned site proves the init binding and returned
function/capture relationships. Preserve a private authentication tag through
freeze; only successful genuine initialized invocation registers recipes.
Raw construction, matching provider IDs/names/repr, or runtime identity tokens
never confer recipe authority. Verify genuine Cpp input and capture shapes.
Keep original function Values untouched during evaluation.

Store actual function Value → typed descriptor registrations in a GC-traced
module-root owner using Module::extra_value, alongside (not overwriting) the
existing four empty-depset cache values. Dict keys/descriptor captures must be
traced and frozen. This is phase-local provenance scratch, released with module;
Evaluator.extra/AnalysisEvaluationContext cannot root moving heap Values.
Seed the normal AnalysisValueLowerer from this owner before recursive provider/
action publication so extracted fields and aliases work regardless of field order.
No process registry, filename fallback, heap pointer in retained data, or generic
function bytecode/capture serialization.

Retained starlark runtime gains a generic lazy function identity capability:
one OnceLock<Arc<()>> per Def instance, requested explicitly by trusted host code.
Ordinary provide/equality only read existing identity and do not allocate.
Def/native callable equality must be symmetric. Freeze moves the cell; GC moves
the payload. Distinct instances sharing DefInfo remain distinct. Keep Def's
existing function-name hash permanently; native recipes write the same cached
STRING-hash u32 stream, not the finished function hash. A token is occurrence,
not source authority. Runtime has no Slug/Bazel names or recipes. BuildAPI retains
only Arc token and primitive hash, adding no starlark-runtime dependency.

BuildAPI owns complete typed callable recipes and full structural Cpp projection.
Fresh callable occurrences use shared publication bijection with captures lowered
through the ordinary bridge (preserve depset owners/order/alias topology).
Global SupportsPic publication additionally requires the same opaque token:
do not alpha-rename an externally comparable module-global function after reload.
Cpp publication checks complete configuration plus local fragment alias topology.
Native fragments authorize restricted methods using CURRENT evaluator provenance,
never the producer's caller table. No copied derived flags or independent config.

SupportsPic calls feature_configuration.is_enabled("supports_pic") and returns
its result; AlwaysPic returns True. Runtime recipes use truthiness of
is_enabled("static_link_cpp_runtimes"); disabled returns the normal empty depset,
enabled returns the exact captured depset, enabled None emits original error.
Preserve keyword-only arity and generic method invocation semantics rather than
silently requiring only one feature facade. Unsupported captures fail closed.

DICE semantic ownership remains configured-node/provider publication and tracked
loading/configuration/dependency inputs. No new key/lock/cache/source-loading
route. Existing validity, cutoff, cancellation/retry and eviction apply; follow
docs/developers/dice.md. Typed recipe Arc/Dupe, compact ordered scratch, immutable
slices and Allocative reuse Stage9 WP726/758/760 rows. No donor import or interner.
Account for lazy slot size and one identity allocation only when issued; no
throughput, memory reduction or exact numeric hash claim.

## Scope and coordination

Runtime worker: starlark-rust/starlark/src/eval/compiler/def.rs;
values/types/function.rs (public generic capability); tests/def.rs or cohesive
child tests and required module export. Then disjoint Loading native callable
facade/tests and Cpp projection/facade only after root supplies model API.
Loading paths: new cc_toolchain_callable.rs + tests, analysis_fragments.rs and
analysis_fragments/cc_feature_tests.rs. Configuration native/cpp_fragment.rs
only exposes complete already-owned projection, no new command options.

Root: BuildAPI new cc_toolchain_callable.rs and cpp_fragment.rs plus lib.rs,
analysis_value.rs and child tests; Loading provider.rs initialized authentication
dispatch and empty-depset root delegation, new cc_toolchain_auth.rs and
evaluator_roots.rs with tests/lib.rs exports; Analysis analysis_value.rs and
cohesive callable/fragment bridge tests, starlark_rule.rs normal lowering seed.
Large provider/analysis modules get bounded hooks, not embedded implementations.
Loading/Analysis BUILD declarations export the existing pinned source as test
compile_data; no new corpus copy or module graph edge.

Integration worker: tests/rustc_map_each/cc_toolchain.rs and sibling .bzl;
mod.rs registration/helper only. Construct all real provider fields via complete
unchanged corpus, actual Files/native projections and real CcInfo; retain full
provider, not a reconstructed struct. Authored inert fields are explicit inputs,
not claims of full FDO/toolchain producer support.

Reviewer owns independent design/final audit. Root serializes Cargo, integration,
docs and atomic commit. Docs manifest/canonical, bootstrap-readiness, Stage6 and
matching Stage9 rows. Estimate800–1400 production/1200–2000 proof lines across
these cohesive owners; review growth. No actions/config-option/remote changes.

## Acceptance and validation

Runtime: ordinary defs remain unissued; two-way Def/native equality and dict
lookup/replacement before/after issuance; same-name closures distinct; issue
before and after freeze; GC modes; tuple hash consistency; measured slot layout.
Loading: authenticated exact source versus changed digest/foreign source/raw
constructor forgery; rooted registry coexists with empty depsets through GC/freeze;
closure captures and field extraction; Cpp current-caller allowlist behavior.
BuildAPI/Analysis: every recipe/capture/config input participates in publication;
fresh alias bijection, global-token discrimination, retained depset alias with
provider fields, heap disposal and restored invocation. No raw Value retained.

Integration: genuine complete producer/consumer and actual configure/Rust linker
use; mixed restored/global-vs-new genuine constructor equality and dictionary
lookups both ways; forcePIC lambda distinction; static/dynamic None/empty/nonempty,
enabled/disabled and error cases; full provider unused payload; configuration
and capture changes/restoration in same DICE. Source forgery restoration requires
successful reauthentication, not alpha-equality of recreated global functions.
Protect WP762 HeaderInfo and preceding8 Rustc/linker integrations.

Resolve pinned nightly via rustup. Serialized offline/locked Cargo JSON no-run
for retained starlark runtime focused tests, BuildAPI lib, Loading lib, Analysis
lib/starlark_rule integration; Core/CLI checks. Exact selector preflight once per
unchanged executable.60s preparation cap/operation; tests expected seconds, not
a user deadline. No Slug/Bazel process or action execution needed. Format/diff,
plan-status/archive and independent final evidence review before acceptance.
REPLAN only for a new semantic owner/compatibility decision outside this contract;
ordinary compile, selector and implementation corrections stay in packet.

Independent design ACCEPT covers the rooted authentication registry, lazy runtime
identity protocol, strict global-token publication and current-caller Cpp transport.

## Terminal validation receipt

Baseline9e753ebe3; pinned nightly-2025-09-14 resolved through rustup. Serialized
`cargo test --offline --locked -p <owner> --lib --no-run --message-format=json`
passed for starlark, slug_build_api_v2, slug_loading_v2 and slug_analysis_v2;
Analysis also passed with `--test starlark_rule`. `cargo check --offline --locked`
passed for slug_core_v2 and slug_cli_v2, with the latter rechecking current Core
dependencies. Exact selector preflight preceded each unchanged executable batch.

39 selected/pass, zero remaining failures: runtime4 (lazy issuance, equality/hash,
GC/freeze/layout); BuildAPI7 (recipes/captures/global-token/config/alias);
Loading12 (auth3, callable3, Cpp5, protected empty-depset1); Analysis2
(callable producer-heap disposal plus protected HeaderInfo disposal); integration14
(complete toolchain4, HeaderInfo2, protected Rustc/linker8). Latest test runtime
1.810s aggregate, max0.202s. Preparation/check aggregate262.897s including failed
attempts, maximum53.592s; no60s preparation cap exceeded. Local detailed receipts
are under ignored target/wp763. Function identity slot16bytes, live/frozen Def304
bytes on this platform; no memory or throughput improvement claim.

Corrections stayed in contract: macro return spelling/iterator comparison/module
lifetime and API compiler fixes; authored cross-repo callback injection instead
of an unsupported canonical load; actual compiler lparen call span(164,164);
test dialect/private-caller setup, supported statement-safe GC and exact flattened
capture assertions instead of opaque depset repr. Old failures remain in receipts;
no assertion was weakened. Existing54-body corpus is unchanged; the existing
toolchain source is declared in Bazel test compile_data. No Bazel/Slug process,
daemon or build action ran. Rustfmt, diff, plan-status and archive checks pass.
Elapsed approximately34minutes from prior acceptance, including design/review;
separate reviewer wall time is unavailable. No REPLAN was needed. The observable
gate advanced is complete constructor publication and genuine downstream queries,
not full toolchain/FDO rule production or CppCompile. M7A remains partial.
Independent final ACCEPT confirmed the actual implementation,39 passing tests,
GC/capture/current-caller ownership and corrected Bazel test source declarations.
