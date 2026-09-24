# Current Slug V2 Work Packet

Packet: WP-7-113-m7a-native-alias-none-visibility-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit explicit `visibility = None` on direct `alias(...)` and loaded `.bzl`
`native.alias(...)` calls as omitted visibility. Authenticated bazel_skylib
1.8.2 `lib/selects.bzl:177-184` passes `visibility=None` through
`selects.config_setting_group` while rules_rust registration row 56 loads.
The rebuilt authentic `slug build //:probe` must clear the current alias
argument-type stop and may reach a separately typed later stop. This is a
Loading-only packet, not configured alias resolution or M7A close.

Exact Bazel 9.2 slice: for the known alias `visibility` attribute, explicit
`None` and omission both use the surrounding package/legacy-macro default;
`None` is not an explicitly declared private or empty visibility. An explicit
empty list is private; valid list/tuple visibility remains accepted, and a
scalar string remains invalid. Unknown attributes, including `bogus=None`,
remain errors. Slug-native: diagnostics, canonical visibility/label identity
and output path bytes. Unsupported/deferred: other native rule classes or
attributes receiving `None`, symbolic macro visibility, arbitrary iterable
coercion, cross-repository policy beyond accepted mapping, configured target
selection, actions and full rules_rust/Swift bootstrap.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`AttributeProvider.java:205-241` validates a known attribute name then skips
an explicit Starlark `None` before conversion; `RuleFactory.java:303-324`
uses that path for native rules. `BaseRuleClasses.java:228-237` gives alias
its nonconfigurable visibility attribute, and `RuleVisibility.java:175-203`
distinguishes an explicit empty list from inherited defaults. Bazel's
`Target.java:166-176`/`Rule.java:674-687` own normal package-default
inheritance. The authenticated bazel_skylib source hash is
`df3e9b6a0eaf081103b8d11dee55abcb98db8532cd552d6f32205c3a3d3f54c0`.

In Slug, both `alias` wrappers in
`app/slug_loading_v2/src/package.rs:9410-9420,10154-10165` currently bind
`visibility: Option<UnpackVisibility>`, whose list/tuple-only parser rejects
`None` before `alias_global` runs. Normalize explicit `None` to omitted at
these two call boundaries only, then reuse the existing `UnpackVisibility`
parser for non-None values. `alias_global`, `PackageRecorder::alias`, and
`visibility_source(..., PackageDefault)` already retain the correct fact;
do not change them or the shared parser used by other rules. The package
source/default and existing source-manifest/repository observations continue
to own DICE equality and A/B/A invalidation; no new semantic input, key,
cache, lock or retained value is added. Parsing is evaluator scratch and
returns the existing borrowed/owned visibility arguments; package/DICE
retention and release boundaries are unchanged.

## Evidence and validation

Offline Bazel 9.2 scratch `/tmp/slug-wp813-alias-visibility` used ignored RCs
and disabled repository downloads. With a public package default, XML query
accepts omitted and explicit-None aliases without a declared visibility
element; explicit list/tuple public are public and `[]` is private.
Cross-package builds for omitted/None/list/tuple public exit 0; the empty-list
alias is rejected by visibility (exit 1). A scalar-string visibility query
exits 7. A two-setting source-shaped bazel_skylib macro query exits 0 and
selects the expected aliases under `dbg` and `opt` cquery. This discriminates
None-as-omission from None-as-empty/private. Reuse this scratch and pinned
source; no Stage 1 fixture unless a demonstrated gap requires it.

Add a focused Loading regression beside existing alias tests. Through both
direct BUILD and a loaded `.bzl` legacy macro, compare omitted, explicit
None, explicit `[]`, and explicit public list/tuple under a public package
default; inspect retained visibility source/effective visibility and
cross-package query or Loading visibility where available. Keep bad scalar,
bad element, unknown-kwarg-None and malformed `actual` fail-closed controls.
Use exact nonignored test preflight/serial execution after pinned-nightly Cargo
JSON `--no-run`, plus one named direct Query/Analysis dependent compile if
the retained visibility contract changes. Rebuild `slug_cli_v2` before the
authentic normalized-PATH smoke; check/clean `slugd` before and after. Run
Rust format, diff, archive and plan-status checks. Independent DESIGN review
must precede implementation, with FINAL review of the diff/evidence. No broad
suite or remote-service gate is selected; leave BuildBuddy auth untouched.

## Scope and stops

Base clean main `c2a31c4bb`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 owner update after acceptance; only
`app/slug_loading_v2/src/package.rs` production and focused tests in
`app/slug_loading_v2/tests/build_file_loading.rs` (or the adjacent existing
Loading alias test owner if that is required for source-shaped macro proof).
Approximate production/proof growth 25/110 lines is a review trigger. The
large `package.rs` already owns both call wrappers and the shared recorder;
two local signature conversions belong there, without splitting or modifying
its many unrelated rule entrypoints. REPLAN if Bazel treats this `None` as an
explicit visibility, if Slug's wrapper cannot preserve existing list/tuple
and invalid-value validation without global parser widening, or if clearing
the authentic stop needs configured/native-rule-family semantics. Do not
paper over a new independent stop.

## Acceptance receipt

Three independent DESIGN and FINAL reviews returned ACCEPT. Pinned Bazel
9.2 source and offline scratch query/build/cquery distinguish known-alias
explicit `None`/omitted package default from `[]` explicit private, accepted
public list/tuple, rejected scalar, and the selected two-setting Skylib
path. The two Slug wrappers alone now normalize explicit `None`; non-None
values still pass the unchanged `UnpackVisibility` parser, then the existing
recorder owns `PackageDefault` versus declared visibility. No new retained
value, DICE dependency, cache, lock or imported utility was added.

Pinned-nightly Cargo JSON `--no-run` compiled the Loading integration binary
with zero compiler errors after a bounded error-conversion compile correction.
Exact nonignored preflight selected three tests; the new direct-BUILD and
loaded-macro visibility matrix and two protected alias tests passed 3/3
serially. The matrix covers bad scalar/element, unknown-kwarg-None and
malformed-actual negatives. `cargo build -p slug_cli_v2` exits 0 before the
normalized-PATH authentic one-shot smoke. That smoke clears the prior
`native.alias(visibility = None)` failure and stops separately at
`constraint setting defaults are unsupported: @@rules_rust+//rust/platform:wasi_version`
(exit 2). No configured alias/action or remote-service test ran. Pre/post
smoke `slugd` checks find no process; Rust format, diff, archive and plan
status checks pass. Packet selection through final review took roughly
thirteen minutes; the CLI build took about ten seconds and smoke about
forty-eight seconds.

## Immediate predecessor

WP812 accepted at `c2a31c4bb`: the typed Apple
`configuration_field(xcode_config_label)` declaration clears the preceding
rules_swift stop. Seven focused tests, offline Bazel oracle and independent
reviews support its finite boundary. Two extra older whole-rule DICE tests
failed diagnostically and remain unresolved, not passing evidence.
