# Current Slug V2 Work Packet

Packet: WP-7-73-m7a-cc-toolchain-provider-r1
Status: accepted

## Outcome and demand

Invoke the unchanged rules_cc get_cc_toolchain_provider using genuine rule ctx,
the authentic FDO subrule, configuration provider and File inputs; publish its
complete CcToolchainInfo into a separate public compile consumer. Bundle the
three missing configuration projections and their command/exec ownership.
This replaces authored constructor arguments at the producer boundary, not the
full cc_toolchain rule envelope or real compiler execution.
WP772 at2a097c1b1 accepts pure compile input preparation with54 tests and Core/CLI
checks; its full receipt/contract remains in Git. M7A remains partial, M8 unproved.

## Compatibility and source

Exact named slice: minimum_os_version nullable string method, public callable
is_tool_configuration from semantic exec role, restricted stamp_binaries method,
command conversion/defaults, exec resets, and unchanged default no-profile/
no-runtime/no-module-map provider helper including opt/default exec configurations.
Slug-native: structural configuration/artifact/path identity, valid Unicode and
diagnostics. Full rule attribute extraction/allowlist/TemplateVariableInfo,
runtime Solib, module maps, profile actions, %package include paths and execution
remain unsupported/deferred, not silently substituted.

Pinned Bazel9.2 8220c6198837d5c13d53fea211cf3282aa12408a:
CppOptions141–148, CppConfiguration345–353,
BuildConfigurationValue668–676/747–753, CoreOptions349–356,
FieldOptionDefinition327–340, Converters69–74,
builtin_exec_platforms221–227/291–353.
rules_cc0.2.17 release archive SHA256
283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a:
cc_toolchain_provider_helper170–305 (SHA256
92e52421866fb6fcba103159d17575480439aa0567d362937aca03aebb7372e3);
fdo/fdo_context23–268/430–473 (SHA256
91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7).
Native source regressions plus authenticated unchanged producer/consumer proof
suffice; no new Bazel/compiler/network invocation selected.

## Semantic ownership

Existing full SlugConfiguration already retains minimum_os_version=Absent and
stamp=false. Add typed command enum admissions only; all supplied values flow
through existing immutable request/configuration/DICE equality and hashing.
Minimum OS is plain text: empty, arbitrary and literal "null" remain strings,
not parsed versions or absence. New projection uses existing cpp_text, not the
profile-only populated-value rejection helper. Validate malformed retained types.
Stamp uses existing CoreOptions bool conversion, including no-form semantics.
Both exec conversion paths reset minimum_os_version to Absent and stamp to false.
is_tool_configuration projects exactly SlugConfigurationKind::Exec; the legacy
raw native "is exec configuration" descriptor is not the active role owner.

Loading facades borrow/share existing immutable configuration. minimum_os_version
and is_tool_configuration are unrestricted callable methods; stamp_binaries checks
the current caller's default allowlist using existing source-provenance machinery.
No forged struct, trusted-looking receiver or helper handle bypasses that gate.
No new retained value, DICE key/lock, interner, side cache, filesystem observation
or independent configuration identity. No hashing/layout or performance change.

## Producer proof and provenance

Use the existing selected-registry compile_context workspace. Add only the11
required complete rules_cc load bodies: common/cc_helper, common/visibility,
common/objc_info, rules_impl/cc_toolchain_provider_helper, rules_impl/objc_common,
rules_impl/objc_compilation_context_info and fdo/{fdo_context,fdo_prefetch_hints,
fdo_profile,memprof_profile,propeller_optimize}. All are verbatim authenticated
release members with sources.json/fixture.toml provenance. Review full import
closure and growth; no copied mutations, stripped body or profile-subrule shim.

Authored caller lives in cc/private/rules_impl to respect private visibility,
declares fragments=[cpp] and subrules=[create_fdo_context], supplies an explicit
attributes struct with genuine config info, File depsets and OutputGroupInfo.
All seven required tool names are supplied, not the old three-tool fixture.
No-profile fastbuild and opt/default-exec call authentic FDO; no FDO actions.
Default exec proof uses the production platform-aware transition and asserts opt
mode explicitly; the role-only to_exec conversion does not imply opt. Exact PIC
action order is [PIC] in fastbuild and [PIC, non-PIC] in opt for the PIC consumer;
other consumers emit [non-PIC]. Every action receives complete downstream checks.
No runtime/module-map inputs. Assert tool/include/sysroot/build-variable producer
semantics and complete downstream provider/recipe identity. Actual published
public compile must expand argv, prepare inputs and preserve execution refusals.

## Scope and routing

Base2a097c1b1; branch codex/wp773-cc-toolchain-provider-review.
Independent design ACCEPT2026-09-22 before implementation and final actual-diff/evidence
review before atomic integration. Config worker owns:
app/slug_configuration_v2/src/command.rs, native/configuration.rs,
native/cpp_fragment.rs and focused native/configuration/toolchain_provider_tests.rs
plus module hook. Root owns Loading analysis_fragments.rs and focused child
toolchain_provider_tests.rs; cc_compile_outputs/configuration.rs and child tests;
Commands common.rs test table rows only. No production Analysis/Core/REAPI edits.
Fixture worker owns Analysis tests/rustc_map_each/compile_context/provider.rs
and provider.bzl, parent hook, sources.rs, the11 listed fixture bodies and
sources.json/fixture.toml. Existing test factory visibility changes only if needed
for sibling reuse. Root docs Stage6/bootstrap/canonical/current; other owner docs
only if the implemented gate changes their architecture.

Estimate80–180 production and350–600 authored test lines, excluding necessary
verbatim import closure. Central files get only cohesive methods/enum/reset hooks;
focused tests live in children. Growth is a review trigger, not an automatic stop.
Existing shared configuration/AnalysisValue/Variables/provider owners suffice.
No donor utility or representation change; inherited Stage9 WP763/764 applies.

## Validation and stops

Required discriminators:
- defaults, empty/literal-null/arbitrary minimum OS, malformed retained types,
  stamp boolean aliases/no-form, full structural distinction/restoration;
  both exec paths erase target minimum OS/stamp, explicit Exec role only.
- public methods callable in ordinary provenance, stamp denied there but allowed
  from authenticated rules_cc source; genuine values, no untracked constants.
- actual helper producer in main/external target/exec, fastbuild/opt with no-profile
  FDO; complete output consumed by public compile and immutable projections.
- same-DICE producer File/metadata/configuration A/B/A, missing/error/restoration,
  source tool-path rejection and no partial provider/actions; unused full
  configuration remains structural. No request-global repair or direct filesystem
  resolver in production.
- protected existing constructor/public compile/command/preparation/dotd/cache
  and restriction proofs. Reuse unaffected evidence; do not claim full cc_toolchain.

Pinned nightly serial offline/locked Cargo JSON no-run for Configuration,
Loading and Analysis; exact batched preflight/tests; Commands focused capture,
Core/CLI checks; source hashes/closure, rustfmt, diff, archive and plan-status.
Preparation60s/operation; small runtime selectors. These are operation estimates/
caps, not a reinstated user hard12/15s ceiling. No daemon/Bazel/compiler action.
REPLAN for unowned state, contradictory source, helper requiring a new semantic
boundary beyond these projections, unauthenticated replacement or execution
activation. Ordinary scoped corrections stay in this packet.

## Review and validation receipt

Production change is45 net lines plus7 test-hook lines in existing
Configuration/Loading owners;
focused tests remain in children, not the large configuration module. Authored
Config/Loading/provider tests total943 lines, above350–600 estimate: nullable
option/exec conversion, genuine caller authorization and full producer/consumer
state restoration are distinct discriminators. The11 mandatory verbatim load
bodies add2470 lines/100288 bytes, growing corpus54→65; all65 source hashes and
lengths pass, and each new body is byte-identical to its release archive member.
Independent review confirmed the complete Objc/FDO load closure is required,
without admitting unused functions. No new repository or copied mutation.

Validation uses pinned nightly2025-09-14, offline/locked serial Cargo JSON no-run,
batched exact preflight and serial runtime selectors. Local detailed receipts:
target/wp773/*.receipt.json and test logs. Corrections were limited to test
setup: CanonicalLabel parse error conversion, genuine .bzl function provenance
for bound method calls, DefaultInfo.files instead of unadmitted ctx.files,
source-defined PIC action multiplicity, and platform-aware default exec mode.
No production design contradiction or relaxed assertion; no REPLAN needed.

Final executed gates (all exit0, selected/pass counts identical):

| Gate | Exact tests | Runtime | Receipt suffix |
| --- | ---: | ---: | --- |
| Configuration new6 + protected compile5 | 11 | 0.046s | tests-1790043798015596478 |
| Loading new3 + protected fragments/outputs13 | 16 | 0.105s | tests-1790044195660172807 |
| Commands raw minimum/stamp capture tables | 2 | 0.002s | tests-1790044221234214806 |
| Analysis new producer3 + protected22 + constructor4 | 29 | 4.675s | tests-1790044541154357075 |

Total58 unique tests; new provider matrix covers6 producers/18 consumers/22
compile recipes. Analysis command is cargo test -p slug_analysis_v2 --test
starlark_rule --no-run; other owners use --lib, all add --offline --locked and
--message-format=json. Runtime is exact binary --exact SELECTOR --nocapture,
following v2_test_preflight.py; full selectors and commands are in receipts.
Final compile times Configuration1.117s, Loading4.121s, Commands3.572s,
Analysis6.376s. Core/CLI cargo check exit0 in7.626s/8.028s.
Loading preparation attempts14.237s(exit101),31.672s,4.121s,4.121s; Analysis
17.204s,1.467s,6.426s,6.376s. All preparation operations stayed below60s.
Earlier runtime failures are retained alongside the passing receipts; they
establish test-setup defects, not admitted semantic behavior.

Pinned CppConfigurationStarlarkTest31–45 nullable/arbitrary text and
CompileBuildVariablesTest405+ minimum-OS propagation are exercised through the
Rust configuration/projection and unchanged helper proof, without launching Java.
Rustfmt, git diff --check, v2_archive_status.sh and v2_plan_status.py pass.
No daemon, Bazel, compiler action, external runtime or remote service invoked.

Independent design and final ACCEPT2026-09-22; final review inspected actual
source and corrected evidence. Packet elapsed about18 minutes from02:18:43 UTC
contract activation to02:36:41 acceptance record, plus prior bounded research.
All compile attempts90.313s, checks15.654s, all recorded runtimes8.958s; final
unique runtime4.828s. Review ran concurrently; separate review CPU time unavailable.
Observable gate advanced: authentic no-profile toolchain provider construction
and complete publication into public compile, not merely new getters. M7A stays
partial and M8 unproved; full rule construction and discovery/execution remain open.
