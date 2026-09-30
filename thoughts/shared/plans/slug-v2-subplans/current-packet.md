# Current Slug V2 Work Packet

Packet: WP-7-166-m7a-native-mangled-soname-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Baseline659273f8c (WP865). Advance unchanged authentic Linux Rust-toolchain
cquery beyond confirmed dynamic_library_soname preserve_name=False refusal at
rules_cc cpp_link_action.bzl:193 on dynamic link creation. Reuse existing Solib
Slug-native naming and existing Linux path normalization; preserve accepted
True basename exact slice and all wrapper/current/live/token guards. Windows
path policy stays refused/minimal; no Windows implementation or execution.

Pinned Bazel9.2 8220c619 CcStarlarkInternal:247-253 passes PathFragment.create
(path) and constructing configuration mnemonic. SolibSymlinkAction:306-320:
False returns lib + optional mnemonic suffix from first ST- through end + '_'
+ Actions.escapedPath(normalized full supplied path). Actions:45-52 escapes
underscore/slash/backslash/colon/@ to _U/_S/_B/_C/_A. No extension/relative-path
checks, 255 limit or hashing belong to soname; hashing belongs separate solib
output construction249-258. CcLibraryConfiguredTargetTest:2482 covers ST names.

Decision: False naming is explicitly Slug-native, omitting Bazel ST mnemonic
bytes exactly as accepted SolibSymlinkSpec naming. Linux normalization and five
escape substitutions over supplied Rust Unicode strings match pinned rules;
Bazel transition mnemonic bytes, exact output/path/config identity and other
platform paths remain unsupported/deferred. Never invent prefix from Slug
configuration token/hash or infer transition origin. Existing full structural
configured owner identity remains authoritative; primitive names do not alias
semantic configurations. No claim that external short_path equals a solib
input's root-relative path. Each producer supplies its own original path.

## Owners, scope and proof

BuildAPI SolibSymlinkSpec already formats lib+escape(root_relative_path) before
shortening. Extract a bounded associated slug_native_soname(normalized_path:&str)
->String using the unchanged escaper; its ctor reuses then retains existing
shorten/filetype/relative guards. Loading False calls existing
NormalizedBazelPath::new(HostPathFlavor::Unix,path), then shared unshortened
formatter; returns primitive string on current heap. True keeps borrowed
linux_basename unchanged. All states are existing phase scratch/evaluator
values, not new retained fields/containers/actions/identity/cache/key/lock.
Existing configured/source DICE owners capture resulting scalar provider/Args
values; read docs/developers/dice.md. No utility import/performance claim.

Allowlist: BuildAPI src/actions/solib_symlink.rs bounded formatter+existingtests;
Loading src/subrule_invocation/link_actions.rs and childtests.rs;
Analysis tests/subrule/link_actions.rs existing helper/Proof only;
manifest/canonical and compact Stage6 existing SONAME/Solib owner notes. No
configuration projection/transition owner, public builtin binder shape changes,
new fixture, action execution or filename-shortening change. Estimate production
below35/proof below90 netRust lines. Independent DESIGN then FINAL because
shared formatter/public native naming boundary; no new architecture owner.

Loading existing True vectors remain; add False literal vectors empty/root,
dot/repeated slash/parents/root clamping, relative external short spelling,
all five escapes plus Unicode/NUL and long>255 unshortened. Wrong types/wrapper/
missing/foreign/expired/suspended guards protected for both flags. BuildAPI
existing naming/equality/long filename tests must protect unchanged native
Solib output bytes/shortening/owner identity and shared formatter difference.
Existing Analysis root/subrule helper adds False primitive provider fields
without replacing True or existing file actions/library sequence proof;
assert exact native names, both-route config/source restoration including exec
where existing harness supports. Generic resource registration and existing
LinkArgs gates remain protected through direct compile dependencies/replay.

Root serial pinned Cargo JSON no-run60s BuildAPIlib/Loadinglib/Analysissubrule;
exact nonignored preflight and focused runtime; Corecheck/CLI rebuild before
unchanged three-hash authentic supervised180s replay (76s prerequisitecost).
Actual process cleanup, scopedrustfmt/diff/archive/status. Caps not userlimits.
REPLAN if existing formatter cannot preserve ctor bytes, True exactness or
fullpath normalization, or new mnemonic/transition semantic owner is required.
A Slug-native False result is not a fallback/parity claim; matching Bazel ST
bytes stays a separate future configuration/output-identity milestone.

## Predecessor

WP865659273f8c independent DESIGN/FINAL;57focused1.163s/Core/CLI pass. Unchanged
cquery76.818s exit2 clears static resource registration and next refuses False
SONAME193 on dynamic path; three hashes/cleanup/sourcefixture unchanged.
target/wp865. Resource execution, full linking/query/bootstrap/M8 deferred.

## Validation receipt

Fresh pinned offline Cargo: BuildAPI4/4 (0.064s, prep3.57s), Loading28/28
(0.064s, prep10.11s), Analysissubrule6/6 (0.314s, prep9.36s); all exact ordinary
nonignored selectors preflighted on rebuilt executables. Corecheck3.19s and
CLIbuild8.71s pass. Production net11/proof91 lines; paired authority/lifetime
guards and distinct vectors justify one line over proof estimate, no new
fixtures/helpers/retained representation. Scoped formatting/diff/archive/status
pass. Unchanged authentic query76.423s exit2 clears False SONAME193 and reaches
cpp.incompatible_remove_legacy_whole_archive missing at finalize_link_action436.
Three original input hashes match before/after; actual process checks empty.
Receipts target/wp866. Query/link/bootstrap/M8 remain unproved.

Independent FINAL ACCEPT: shared formatter preserves original ctor bytes and
True exact slice, bounded False normalization and both-flag guards; proof growth
is justified. Packet window10:58–11:20UTC (2026-09-30); no speedup claim.
