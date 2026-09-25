# Current Slug V2 Work Packet

Packet: WP-7-146-m7a-filegroup-package-license-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain alias `cquery //:probe --output=label`
past the post-WP845 `native filegroup nondefault licenses is unsupported`
guard, then report the first new stop. Admit only the source-authored
package-default `licenses(["notice"])` on configured native filegroups.
Keep its loaded/query value and provenance; treat it as inert for the
filegroup's providers, source/runfiles edges and actions under Bazel 9.2
default license-checking options. Do not add generic license enforcement.

Bazel 9.2's inherited notice value, successful filegroup configuration,
existing source/DefaultInfo forwarding and absence of filegroup-owned
actions are `exact` for this bounded case. Slug's structural configured
target and package/DICE equality, route and display identity remain
`Slug-native`. Explicit per-target licenses, other package license values,
`--check_licenses` and other license-related option effects,
`output_licenses`, other nonneutral filegroup attrs, full C++ toolchain
providers, exact Bazel config/output identity and M7A closure remain
`unsupported/deferred`. Preserve WP837-845 accepted slices.

## Pinned source, oracle and semantic owner

Pinned Bazel 9.2 `BuildGlobals.java:100-124` stores BUILD-global
`licenses()` in package arguments. `BaseRuleClasses.java:315-326` and
`AttributeProvider.java:330-383` install a nonconfigurable native-rule
license attr, distinguishing an explicit target value from the inherited
package value; `Rule.java:733-747` resolves explicit before package
license. `FilegroupRule.java:31-82` separately defines
`output_licenses`. `Filegroup.java:44-118` constructs filegroup
files/runfiles/providers from source/data/output-group inputs without
reading the license or registering an action. The default
`--check_licenses=false` is source-defined in `CoreOptions.java:706-711`;
this packet does not admit the opt-in path.

WP845's exact built-in whitelist `BUILD:1` and pinned rules_cc 0.2.17
`cc/private/toolchain/BUILD.tpl:21-23,44-68` both declare package
`licenses(["notice"])` above native filegroups. Their target-specific
visibility remains a separate owner. Fresh ignored-RC Bazel 9.2 in a
temporary root workspace configured a filegroup inheriting notice:
`cquery //:inherited --output=label` exited 0 with one configured label,
and `aquery //:inherited --output=text` exited 0 with no actions. An
explicit-notice variant also configured in Bazel but is deliberately
outside Slug's admitted slice. Existing Stage 4 package-license query
evidence already proves inherited versus explicit loading projections;
do not duplicate it in a new Loading fixture.

Slug Loading records package licenses at `package.rs:1608-1614` and
projects them into native attrs with `AttributeProvenance::Default` at
`package.rs:2725-2732`; explicit rule overrides retain distinct
provenance. The loaded package and native attrs remain the DICE-owned
semantic input. Analysis `dice/filegroup.rs:91-111` currently rejects
every nonempty license as an unadmitted attr. Change only that guard to
accept a resolved `StringList` exactly `["notice"]` when provenance is
`Default`. Keep explicit notice, other inherited values and all other
nonneutral attributes on the existing fail-closed path. Do not erase or
normalize the loaded license. Existing target/configuration keys,
package equality, source observation and restoration own invalidation;
no new DICE key, lock, retained representation, hash, interner, Host
read or memory-accounting form is allowed.

The touched `filegroup.rs` module is below the plan's large-file trigger,
though `prepare` is long. This is one local guard in its existing
attribute-admission responsibility; splitting the asynchronous source
resolution flow would widen risk without clarifying this single rule.
No donor runtime or fallback is used; the default license metadata
continues to live in Loading, not a command-side repair.

## Evidence and implementation boundary

Add a small offline fixture with only a MODULE, a source file, and a root
BUILD declaring package `licenses(["notice"])` and
`filegroup(name="inherited", srcs=["payload.txt"])`. One configured
`cquery //:inherited --output=label` row is the discriminator for the
current Slug guard. Generate and independently verify expected Bazel 9.2
results in fresh ignored-RC roots, then replay through a rebuilt Slug
CLI. Add an `aquery` zero-action row only if the harness gives a stable
cross-tool comparison; source plus the temp oracle already cover that
non-effect. No BuildBuddy auth file is read.

Focused native-filegroup Analysis tests must prove inherited notice
acceptance in root and external packages, unchanged source forwarding,
explicit notice still rejected, other inherited license value rejected,
other nonneutral attrs still rejected, and same-DICE A->B->A package
license restoration. Existing `rejection.rs` explicitly rejects target
`licenses=["notice"]`; preserve that assertion. Do not alter Loading
package-global/schema code or source/DefaultInfo provider construction.
Use JSON `--no-run`, exact test-name preflight and focused tests; rebuild
`slug_cli_v2` after the Analysis production edit; run fresh Bazel/Slug
fixture and authentic cquery. The authentic error currently omits its
target label, so a changed diagnostic must not be assigned to a specific
filegroup without evidence. Serialize Cargo; check/clean stale `slugd`
before and after smokes. Run format, diff, plan-status, archive and
fixture hygiene. Independent DESIGN and FINAL reviews gate the edit and
acceptance; update compact Stage 6/9 owner notes only after acceptance.

Allowlist: this manifest and canonical Current; `app/slug_analysis_v2/src/dice/filegroup.rs`;
`app/slug_analysis_v2/tests/native_filegroup/{rejection.rs,lifecycle.rs}`
and test `mod.rs` only if a helper is needed; one new small
`tests/v2_oracle/fixtures/native-filegroup-package-license/` fixture;
compact Stage 6/9 notes after acceptance. No Loading production, Core,
repository catalog, rules_cc, remote cache/action or other fixture edits.

`REPLAN` if default provenance is unavailable in Analysis, if the
bounded notice value affects Bazel's configured providers/actions under
default options, if Slug needs to erase it from loaded/query identity,
if admitting it requires a new DICE/Host owner, if fresh Slug replay
does not discriminate the guard, or if the authentic stop persists and
evidence points to a different attribute/provenance. A later C++ rule
or opt-in license checker is a separate packet.

## Acceptance receipt

Independent DESIGN review ACCEPT preceded the production edit. The only
production change is one provenance/value gate in the existing configured
native-filegroup attr loop; Loading and DICE identity are untouched.
`cargo check -p slug_analysis_v2` and JSON `--no-run` compilation of
`slug_analysis_v2 --test starlark_rule` passed. Exact test-name preflight
found both new/changed selectors. Each passed independently, and all nine
`native_filegroup::` integration tests passed. The tests cover root and
external inherited notice forwarding/no own actions, explicit notice and
other-attribute rejection, and same-DICE notice -> restricted -> notice
restoration. `cargo build -p slug_cli_v2` passed after the Analysis edit.

The one-row offline fixture passed fresh ignored-RC Bazel 9.2 generation
at `/tmp/slug-wp846-bazel-final-gen-e29tV9`, independent fresh-root
verification at `/tmp/slug-wp846-bazel-final-verify-K4nOJH`, and rebuilt
Slug replay at `/tmp/slug-wp846-slug-final-4QP3Cs`. Two invocation-only
fixture corrections preceded that final evidence: `--noshow_progress`
was removed because Slug cquery does not parse it, and the anchored
configured-label regex was widened to accept Bazel's seven-hex token or
Slug's explicit `slugcfg-v2:` token. Neither required a production change;
the corrected fixture was regenerated and independently verified fresh.
No BuildBuddy credential file was read.

Authentic Rust-toolchain alias workspace
`/tmp/slug-wp797-authentic-2YQmSt` ran rebuilt one-shot
`cquery //:probe --output=label` and exited 2 after about 64 seconds.
The former nondefault-license guard is gone. The first new stop is
`resolving configured dependency _xcode_config:
configuration_field(fragment = "apple", name = "xcode_config_label")
configured resolution is unsupported`. Slug Loading already freezes
this Apple field identity in `subrule_loading.rs:512-560`; configured
resolution remains separate and unimplemented. The diagnostic identifies
the dependency name and field but not its owning target, so this packet
does not assign it to a particular rule or claim C++ closure. Host had
about 25 GiB available RAM and 30 GiB free `/tmp` space before replay;
no stale `slugd` was present before or after smokes. Format, diff,
plan-status, archive-status and fixture discovery passed. Independent
FINAL review ACCEPT verified the one-value/default-provenance gate,
preserved negatives, root/external and restoration tests, separate
configuration display domains in the oracle, and source-attributed
new stop without inventing its owning target. Compact Stage 6/9 owner
notes were updated after acceptance.

## Immediate predecessor

WP845 at `df7c4992b` ported the one-file Bazel 9.2 whitelist package
verbatim, with byte/mode/catalog tests and fresh four-row Bazel/Slug
query evidence. Authentic cquery cleared the missing package and reached
the nondefault-license Analysis guard without identifying its target.
Independent FINAL review accepted the slice; M7A remains partial.

Independent WP846 DESIGN review ACCEPT verified the provenance-gated
default-license boundary, existing DICE ownership and closed opt-in
command-option path before production edits.
