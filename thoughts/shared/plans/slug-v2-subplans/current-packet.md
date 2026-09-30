# Current Slug V2 Work Packet

Packet: WP-7-159-m7a-preserved-library-soname-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Advance the unchanged authentic Rust-toolchain cquery past cc_internal.
dynamic_library_soname at rules_cc cc/private/link/cpp_link_action.bzl:193.
Admit dynamic_library_soname(actions, path, preserve_name), three required
positional-only arguments, for genuine private WrappedLinkActions and
preserve_name=True. Return the Linux lexically normalized path basename,
including empty/root, repeated/trailing separators and dot/parent segments.
Do not restrict suffixes: authentic PIC static archives also call this helper.
False is explicit unsupported/deferred because its Bazel ST- mnemonic bytes
remain unowned; never substitute Slug configuration/path identity for them.

Exact binding/type and True-mode string algorithm: pinned Bazel9.2 8220c619
CcStarlarkInternal.java:239-253, SolibSymlinkAction.java:306-318,
PathFragment.java:125-143, UnixOsPathPolicy.java:65-101 and
OsPathPolicy.java:94-126. PathFragmentTest.java:310-319 pins basename cases;
normalization tests and source cover dot/parent reduction. No direct SONAME
test found. CcLibraryConfiguredTargetTest.java:357 concerns deferred False
linking. rules_cc cpp_link_action.bzl:193-196 uses True for PIC static
archive (target_types.bzl PIC row). Backslash is an ordinary Linux character.
Valid Unicode strings follow existing Slug-native string contract. Non-Linux
invocation explicitly refuses unsupported platform path policy. Windows
policy and execution remain deferred; user requests minimal Windows work.

## Ownership and scope

Existing private wrapper owns unchanged AnalysisActions handle. Require that
private wrapper specifically (base actions are excluded by upstream Java type),
then reuse actions_to_context current evaluator/live frame/root registration
validation. Suspended live owners remain readable; expired/missing/foreign
owners fail. These stronger existing evaluator guards are Slug-native;
upstream helper has no own caller or mutable check. String result is phase
scratch allocated on evaluator heap and may publish as an ordinary string;
no wrapper handle publication, new configuration/action owner, DICE key,
retained memory, registry, lock or action mutation. Existing DICE configured
publication/dependencies remain unchanged; docs/developers/dice.md applies.
True ignores mnemonic mathematically; upstream False reads context config,
not alternate wrapper construction config. Full link actions remain unproved.

Use a bounded local Linux basename reduction in link_actions child module;
do not introduce a shared path utility or file-system observation. A reverse
segment scan can cancel pending parent components and return a borrowed final
segment; unresolved relative parents yield '..', absolute parents stop at root.
No retained collection/clone or performance claim. Existing cc_common.rs is
large; add only narrow registration delegating to this cohesive child owner.
Production estimate below55lines; tests below200 newlines.

Allowlist: Loading src/cc_common.rs and
src/subrule_invocation/link_actions.rs, its tests.rs; Analysis
tests/subrule/link_actions.rs extending existing private-accessor/materialization
fixture; this manifest/canonical Current plus compact Stage6 acceptance note.
Baseline328755f4f. No full linker, false-mode naming, solib/action activation,
generic spawn, donor code, public oracle fixture or Windows implementation.
Independent DESIGN before code, FINAL before acceptance.

## Evidence and validation

Loading tests: pinned basename and normalization vectors, external '../'
short paths, static archive names, Linux backslash/Unicode, required positional
binding and types, genuine wrapper-only refusal, deferred False, missing,
foreign and expired owner validation. Suspended root/subrule read success and
captured helper invocation after success/error must preserve owner checks.
Recording sink remains unchanged (pure helper); protect existing wrapper and
solib tests. Reuse accepted lifecycle guards rather than duplicate unrelated
action testing. Real Analysis fixture uses wrapped root/subrule File short_path,
publishes exact strings beside existing FileWrite outputs; source/config A/B/A
on same DICE in Legacy/Observed routes protects reuse. Existing conflicts,
scope and publication guards remain unchanged. No new public fixture growth.

Pinned nightly serial Cargo JSON --no-run for Loading/Analysis, each60s
preparation cap; batch exact nonignored preflight then runtime expected under
one second. Core direct-dependent check and CLI rebuild. Unchanged original
three-hash cquery from target/wp856/authentic-workspace, supervised180s
operation cap justified by fresh public source preparation observed81s in
WP858; no smaller fixture proves the real finite closure frontier. Supervisor
cleanup/process checks in actual environment before/after. Rustfmt/diff/archive/
plan status checks. Root integrates/commits. These are operation bounds, not
user deadlines. REPLAN only if True requires new semantic ownership or path
normalization contradicts pinned Linux source; routine corrections stay here.

## Immediate predecessor

WP858328755f4f accepts default wrapper over original sink/token/scope. DESIGN/
FINAL and17focused tests pass; rebuilt unchanged authentic cquery81.433s first
reaches dynamic_library_soname on PIC static-library path. Three original
fixture hashes unchanged; no remaining processes. Evidence target/wp858.
M7A/M8/full toolchain and link closure remain unproved.

## Acceptance receipt (2026-09-30)

Independent DESIGN/FINAL ACCEPT. Eighteen exact
nonignored focused tests pass (Loading12, Analysis6), total runtime0.346s.
Pinned serial preparation finalLoading4.71s, Integration8.85s, Corecheck3.22s,
CLIbuild8.58s. Production51lines, tests111Loading plus net10Analysis; no new
fixture or semantic owner. Tests prove Linux normalized basename including
static/external/Unicode/backslash paths, strict positional/wrapper binding,
False refusal, current/missing/foreign/expired ownership, suspended reads and
captured helper expiry after success/error. Published root/subrule strings
change/restore under source A/B/A and preserve results under config A/B/A in
both existing DICE routes, with unchanged FileWrite ownership/conflict guards.

The rebuilt unchanged authentic cquery exits2 in81.071s, clears soname and
first reaches cpp.start_end_lib at finalize_link_action.bzl:135 via
cpp_link_action.bzl:204. Original three hashes unchanged. Supervisor cleanup
and actual-environment process checks pass; Rustfmt/diff/archive/status pass.
Evidence target/wp859; measured evidence window07:58:57UTC to after08:09:57UTC
includes design/review, preparation and replay. No performance speedup claim.
False ST-mnemonic naming, non-Linux policy, full link/toolchain closure and
M7A/M8 remain unproved.
