# Current Slug V2 Work Packet

Packet: WP-7-144-m7a-cc-suite-empty-filegroup-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Advance the authentic Rust-toolchain `cquery //:probe --output=label`
past generated rules_cc `local_config_cc//:BUILD:84`'s
`native.cc_toolchain_suite` call, then report the first new stop.
Implement only Bazel 9.2's default builtins-injected `.bzl` native
binding: require `name`, evaluate but discard all extra keyword values,
and declare an otherwise empty `filegroup(name = name)` with inherited
package defaults. No C++ suite mapping, selection or provider is admitted.

The default Bazel 9.2 injected declaration's filegroup kind, name-only
arguments, empty `srcs`, ignored `toolchains`/`visibility` kwargs,
inherited package visibility and absent suite dependency edges are
**exact**. Slug's existing structural target/configuration identity,
source observation and valid-Unicode Host behavior remain **Slug-native**.
Direct BUILD-global `cc_toolchain_suite`, builtins-injection overrides,
the un-injected Java tombstone rule, legacy `--crosstool_top` resolution,
real `cc_toolchain`/`CcToolchainInfo` semantics and full toolchain/root
Build closure are **unsupported/deferred**. Preserve all WP839–843 exact
and native slices without widening them.

## Source basis and next boundary

Pinned Bazel 9.2 `src/main/starlark/builtins_bzl/bazel/exports.bzl:53-56`
exports `cc_toolchain_suite` as
`lambda name, **kwargs: _builtins.toplevel.native.filegroup(name = name)`.
`BazelStarlarkEnvironment.java:450-499` injects exported rules into
`.bzl` `native`; lines 506-537 also describe BUILD-global injection,
which this packet does not implement. `CcRules.java:56-57` registers only
an `EmptyRule` placeholder, whose configured factory fails; it is not
the default active suite behavior. Pinned rules_cc 0.2.17
`cc/toolchains/cc_toolchain_suite.bzl:17-18` forwards `**kwargs` to
`native.cc_toolchain_suite`. Its generated `BUILD.tpl:81-89` passes
`name="toolchain"` and a four-entry `toolchains` map. The wrapper call
thus creates an empty filegroup, not toolchain selection. The next
source-ordered generated `BUILD.tpl:91-109` call is `cc_toolchain`,
loaded through `@cc_compatibility_proxy`; this packet does not pre-admit
that distinct Starlark rule or its configured semantics.

Slug's `package.rs:10362-10752` shares a base `native_methods` set
between BUILD, `.bzl` and Bzlmod modules; this packet must add a
separate `.bzl`-only method set to `BzlNativeModule` and
`BzlmodNativeModule`, not to BUILD's `NativeModule`.
`filegroup_global` at lines 3441-3456 already
records a native filegroup, sets its generator, and inherits package
visibility when no explicit value is supplied. The recorder at lines
1628-1682 retains empty `srcs`; existing configured filegroup analysis
owns that target. The new binding must not route the discarded kwargs
through `set_native_overrides` or coerce/resolve their labels. A changed
ignored map is a changed source observation but not a configuration-
affecting target input; no DICE key, lock, rule class, retained variant,
interner, hash identity, action or cache owner is introduced.

## Evidence and implementation boundary

Add one `native.cc_toolchain_suite` method in a `.bzl`-only native-method
set, accepting required `name` and arbitrary extra kwargs but calling
`filegroup_global(name, None, None, eval)` without reading or retaining
the map. Keep ordinary `native.filegroup` and target-name conflict
behavior. BUILD's `native` object must not gain this method; loaded `.bzl`
calls outside package evaluation still fail without a package recorder.
Do not add top-level BUILD global `cc_toolchain_suite`, a native suite schema/variant, legacy
selection, or special analysis of the resulting filegroup.

Add a small independent offline fixture with a `.bzl` wrapper
`def cc_toolchain_suite(**kwargs): native.cc_toolchain_suite(**kwargs)`.
Its BUILD has public package default, invokes the wrapper with
`name="toolchain"`, a `toolchains` value naming nonexistent `:missing_cc`,
and explicit private visibility. Bazel 9.2 queries must establish that
the target has filegroup kind, `deps(//:toolchain)` contains only itself,
and `visible(//observer:observer, //:toolchain)` includes it because the
private kwarg was discarded. A configured `cquery //:toolchain` must
succeed as an empty filegroup. The fixture checks the injected default,
not real suite resolution or generator metadata. Generate and
independently verify expected Bazel results in fresh ignored-RC roots,
then replay through rebuilt Slug. No BuildBuddy auth file is read.

Focused Loading tests must cover `.bzl` wrapper name-only conversion,
discarded missing `toolchains` label, ignored private visibility, empty
srcs/inherited visibility, unavailable direct BUILD-native method,
existing direct `native.filegroup`, and
duplicate name rejection. Reuse existing package source A→B→A DICE
coverage unless a demonstrated gap appears; the ignored map must not
become a retained configured dependency. Run JSON `--no-run`, exact test
preflight, focused Loading tests, CLI build after the production edit,
fresh offline Bazel/Slug fixture, then authentic cquery. The smoke must
clear the `native.cc_toolchain_suite` absence and reach a source-attributed
first new stop; an earlier error or resource failure is diagnostic only.
Serialize Cargo, check/clean stale `slugd` around smokes, and run format,
diff, plan-status, archive and fixture hygiene.

Independent DESIGN review must accept the injected-vs-tombstone and
discarded-kwargs contract before production edits. Independent FINAL
review checks actual diff and evidence; compact Stage 4/9 owner notes
follow acceptance.

Allowlist: this packet and canonical Current;
`app/slug_loading_v2/src/package.rs` and
`app/slug_loading_v2/src/host_package_load_tests.rs`, with no new
owner file unless a demonstrated gap requires one;
`tests/v2_oracle/fixtures/native-cc-toolchain-suite-bzl-proxy/` small
source/manifest/expected files; compact Stage 4/9 notes only after
acceptance. No Core, DICE, Analysis, native attr schema, repository IO,
remote cache, action or other fixture edits.

`REPLAN` if the selected wrapper does not actually receive default
builtins injection, if ignored kwargs change package/configured output
in the pinned oracle, if the suite target is not an empty filegroup,
if source-authentic replay requires suite selection before the next
stop, or if implementing it needs a new retained owner or legacy
resolution. A later `cc_toolchain`/provider boundary is a separate
packet, not permission to widen this one.

## Acceptance receipt

Independent DESIGN review ACCEPT required the `.bzl`-only method set so
BUILD's `NativeModule` does not gain this binding. Pinned nightly toolchain
resolved. Loading JSON `--no-run` passed (13.6 s); exact preflight found
both focused tests, and both passed. `cargo build -p slug_cli_v2` passed
after the Loading production edit (8.7 s). Fresh ignored-RC Bazel 9.2
generation and independent verification passed all four fixture rows
(5.25/5.56 s after correcting a Slug-unsupported cquery progress flag),
with filegroup kind, no missing-label dependency edge, inherited public
visibility and configured success. The rebuilt Slug CLI passed all four
rows (12.4 s). The first Slug fixture run's cquery parse failure was
caused solely by `--noshow_progress` in the new manifest; no production
change was needed, and the corrected manifest was re-oracled fresh.

The authentic Rust-toolchain alias workspace `cquery //:probe
--output=label` exited 2 after about 64 s and cleared the prior absent
`native.cc_toolchain_suite` method. Its new first stop is a missing
`@@bazel_tools//tools/build_defs/cc/whitelists/parse_headers_and_layering_check`
package. Pinned rules_cc
`cc/private/rules_impl/cc_toolchain.bzl:335-336` authors this implicit
`cc_toolchain` label default through `semantics.get_repo()`; no suite
selection or configured C++ provider was claimed. `@bazel_tools` content
must be ported verbatim in a later packet, not fabricated here. Host had
about 29 GiB available RAM before replay; no `slugd` remained afterward.
Format, diff, plan-status, archive and fixture hygiene passed. Independent
FINAL review ACCEPT verified the `.bzl`-only route, no legacy suite
selection, focused tests, fresh oracle and authentic next stop. Compact
Stage 4/9 owner and memory notes were updated after acceptance. Packet
elapsed was roughly 20 minutes from WP843 acceptance through FINAL review.

## Immediate predecessor

WP843 at `8a86218b3` accepted the bounded seventh GCC include-path
file with fresh three-row Bazel/Slug evidence, 20/20 Core process tests
and independent FINAL review. Authentic cquery cleared the GCC prefix
and subsequent probes and reached generated `BUILD`'s missing
`native.cc_toolchain_suite`; the rejected call-14 argv was not
separately captured. The full WP843 receipt remains in that commit's
`current-packet.md`. M7A remains partial.
