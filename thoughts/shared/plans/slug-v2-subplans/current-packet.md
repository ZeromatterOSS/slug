# Current Slug V2 Work Packet

Packet: WP-7-127-m7a-native-kwargs-none-boolean-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Clear the authenticated protobuf v33.4 `native.filegroup(testonly = 1)`
Loading stop at `@@protobuf+//:protobuf.bzl:729`. Exact selected Bazel 9.2
Loading slice: for a validated callable native RuleClass kwarg in Slug's
`coerce_native_overrides` path, direct `None` leaves the attribute unset so
its existing default and default provenance apply; a Boolean-typed kwarg
accepts `False`/`True` and integer `0`/`1` as the same typed Boolean value,
with concrete inputs explicitly sourced. The oracle directly covers
`filegroup.testonly`, `alias.testonly`, and filegroup `tags`/`data` omission;
the generic branch follows Bazel's common `AttributeProvider` and `Type`
source, not a claim of all native rule behavior. Slug-native: existing
diagnostic wording and unsupported native configured capabilities.
Unsupported/deferred: direct native builtin parameters outside this kwargs
owner, selectors for nonconfigurable `testonly`, configured filegroup
`testonly=True`, test-policy propagation, and unprobed native rule semantics.
Unknown/implicit/forced attribute names must not become admissible via None.

## Source, owner and decision

The exact BCR protobuf 33.4 release artifact's root `BUILD.bazel:763-765`
passes integer `testonly = 1` through `protobuf.bzl:754-769,656-665` to
`native.filegroup` at `:729-735`; the observed value is not None. Bazel 9.2
`Type.java:379-399` and `TypeTest.java:102-134` accept only Boolean and
integer 0/1, rejecting strings and other integers. `RuleFactory.java:283-303`
passes native kwargs to `AttributeProvider.java:189-223,231-269`, which
validates names, ignores direct None, then converts supplied values; missing
values take defaults at `:281-295,376-411`. `BaseRuleClasses.java:62-69,
245-247,297-300` owns package-default `testonly` for native rules, including
`FilegroupRule.java:69-75`. `PackageFactoryTest.java:867-887` proves native
filegroup omitted/testonly=0 package-default behavior. A minimal ignored-RC,
no-fetch Bazel 9.2 oracle at `/tmp/slug-wp827-filegroup-XgygHU` proves
direct and loaded-macro filegroup/alias 0/1/None/omission under false/true
package defaults, non-Boolean filegroup tags/data None versus explicit [],
and invalid 2/string/selector negatives. Authenticated protobuf query
reports effective true and no unconfigured path from `//:probe` or protoc
to the PHP target; this is another loaded-package breadth obstacle.

`app/slug_loading_v2/src/package.rs::coerce_native_overrides` is the
producer-owned seam used by bare and loaded-macro native calls. Resolve the
canonical slot and callable policy before skipping None. For Boolean kind,
coerce bool or integer 0/1 to `CoercedAttributeValue::Boolean`; reject all
other values without Rust truthiness. Keep other kind coercion and order
normalization unchanged. Existing `NativeTargetAttributes` stores the typed
value and explicit/default provenance in package structural equality; its
late package default already supplies omitted `testonly`. No new DICE key,
lock, side registry, retained representation, schema entry or Query graph
edge. Same-DICE A/B/A must demonstrate source and normalized value
invalidation/restoration. The large `package.rs` remains the cohesive native
invocation/target-publication owner; this edits only its existing coercer.

## Evidence and validation

First compile and exact-preflight a source-shaped loaded-macro
`native.filegroup(testonly = 1)` Loading regression on clean base; it must
fail at the current `native attribute testonly must be a bool` guard.
Then cover direct and loaded-macro filegroup and alias 0/1/bool/None/omitted
under both package defaults, including late `package()` placement; assert
typed values, explicit/default provenance and `None` versus explicit false.
Cover filegroup `tags`/`data` None versus explicit empty, unknown kwarg None,
invalid integer/string/selector, and a same-DICE 1/0/1 restoration. Query
must prove selected effective `testonly` and unchanged ordinary target
projection without inventing configured support. Reuse the existing native
filegroup Analysis rejection test for `testonly=True`. No Stage 1 fixture is
needed; pinned source and accepted local oracle are discriminating.

Resolve the pinned nightly toolchain. Compile selected Loading and Query
binaries with Cargo JSON `--no-run`, exact nonignored preflight and run
focused/protected tests serially. Direct Loading/Query/Analysis/Core/CLI
compiles and rebuilt CLI precede one normalized-PATH authentic `//:probe`
smoke. That replay proves only that the filegroup Loading stop cleared;
it cannot prove configured filegroup test-policy behavior. Check/clean
`slugd` before/after. Run pinned rustfmt, `git diff --check`, plan/archive
checks and independent DESIGN/FINAL review of the shared coercion boundary.

## Scope and stops

Base clean main `de50a3a9b`. Allowlist: this manifest, canonical Current,
compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs`; focused tests in
`app/slug_loading_v2/tests/build_file_loading.rs` and
`app/slug_query_v2/tests/loading_query.rs` (or a colocated existing test
only when needed). Rough growth 45 production/180 proof lines is a review
trigger, not a license for unrelated native-rule work. REPLAN if the
authentic value is not integer 1, direct None differs from omission in the
shared native kwargs owner, package equality misses source/value changes,
or passing protobuf's stop requires selected configured filegroup work.
A later authentic failure cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviews ACCEPT. A protobuf-shaped loaded-macro
Loading test compiled with Cargo JSON `--no-run`, exact-preflight selected
one nonignored test and failed on clean base at the prior native Boolean
guard; the new Query test independently failed at the same Loading guard.
After the change, JSON `--no-run` compiled Loading, Query and Analysis test
binaries. Exact preflight selected 9/2/2 nonignored tests and all 13 passed:
the Loading set includes five new value/default/invalid/invalidation gates
and four protected native/genrule/alias gates; Query covers effective
values/provenance and ordinary edges; Analysis preserves configured native
filegroup rejection. Direct Loading/Query/Analysis/Core/CLI checks and
`cargo build -p slug_cli_v2` passed. Pinned rustfmt, diff, plan and archive
checks passed; no stale `slugd` existed before or after the smoke. The
normalized-PATH authentic `//:probe` replay cleared protobuf.bzl:729 and
exited 2 later with `no such package '@@bazel_tools//tools/jdk': BUILD file
not found`. That distinct source-verbatim content boundary is not part of
WP827 and cannot prove configured native test policy. The 282 proof lines
exceeded the rough 180 review trigger; FINAL review found them proportionate
to five distinct Loading gates and one Query gate. The recorded packet window
was at least 14:45-14:53 UTC; research began before the first timestamp and
individual compile/test/review wall times were not separately instrumented.
M7A remains partial.

## Immediate predecessor

WP826 accepted at `de50a3a9b`: exact Loading/ordinary Query retains one
nonempty aspect-bearing dependency while selected configured Analysis fails
closed. Seven focused tests and rebuilt authentic smoke cleared protobuf
BUILD:682; the next stop is this native filegroup `testonly=1` call. M7A
remains partial.
