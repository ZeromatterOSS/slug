# Current Slug V2 Work Packet

Packet: WP-7-126-m7a-lazy-attached-aspect-target-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Load and query the authenticated protobuf v33.4 `cc_proto_library` target
at `@@protobuf+//:BUILD.bazel:682` without applying its nonempty attached
`cc_proto_aspect`. This target is present in the protobuf package but
ignored-RC/offline Bazel 9.2 query finds no path from `//:probe` or
`@@protobuf+//:protoc` to it. Exact selected Loading/Query slice: one
exported, frozen, parameterless-aspect `attr.label_list` with a final
literal nonempty list retains its attached aspect and canonical dependency
labels; ordinary unconfigured `deps`/attribute projections expose the same
base-target edges as Bazel query. A selected configured target carrying a
nonempty attachment must fail before ordinary dependency/provider analysis.
Slug-native: sparse structural aspect identity and the diagnostic for that
unsupported configured boundary. Unsupported/deferred: applying or
propagating the aspect, provider overlays, aspect-generated actions,
configured `cquery`/`aquery`, configurable/concatenated attached values,
aspects with declared attrs/required parameters/required aspects, multiple
attached aspects and broader rulesets. No inert aspect or plain-dependency
substitution is permitted.

## Source, owner and decision

Pinned protobuf v33.4 `BUILD.bazel:682-689`, `bazel/cc_proto_library.bzl:3-10`
and `bazel/private/bazel_cc_proto_library.bzl:49-197` define the fallback
Starlark rule. Its `deps` has one `cc_proto_aspect`, requires `ProtoInfo`
and disallows files; the aspect propagates through `deps`, requires
`ProtoInfo`, advertises `CcInfo`, requests C++/proto fragments and toolchains,
and registers GenProto/C++ actions whose generated providers the wrapper
consumes. Under Bazel 9.2 modern toolchain resolution the aspect declares
no `attrs`, required parameters or required aspects; Slug's pinned
`native_proto_common.INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION` is true.
Bazel 9.2 `AspectResolutionHelpers.java:43-84`,
`DependencyMapProducer.java:261-305` and `AspectFunction.java:310-317,976-985`
show actual configured application. Authenticated ignored-RC/offline XML
query reports one canonical `deps` label and rule-input at BUILD:682;
`query --output=build` names the Starlark definition; `cquery` shows one
aspect application; `aquery` identifies the aspect's `.pb.cc/.pb.h`
GenProto action. The proto3 sibling yields seven applications across six
direct proto deps. Ignored-RC/no-fetch `somepath(//:probe, target)`,
`somepath(@@protobuf+//:protoc, target)` and
`kind(cc_proto_library,deps(@@protobuf+//:protoc))` are empty. These prove
this stop is package Loading breadth, not selected configured demand.

Slug's frozen rule attribute retains `attached_aspect`, but current
`FrozenRuleDefinition::reject_deferred_attribute_invocation` in
`app/slug_loading_v2/src/package.rs` rejects every nonempty attachment.
The published `AttributeSchema` in `attrs.rs` drops aspect metadata;
Analysis currently keys target+configuration and would otherwise pass
ordinary providers into `ctx.attr`, silently losing the aspect. Introduce
one sparse immutable attachment slice on `StarlarkRuleImplementation`,
aligned by schema index. Each row retains the exported aspect's defining
canonical label and first exported name as structural identity and its
`FrozenValue` as lifetime-only callable metadata. Construct it only from
the frozen declaration after final typed value coercion; reject missing/
unexported/wrong-kind attachment identities and mismatched schema indexes.
Compare row identities in the target's `PartialEq`; do not compare evaluator
addresses. The package's existing full Bzl load fingerprint and target's
observed source provenance own implementation-source invalidation; prove
that imported aspect-source edits change package equality and restore on
same-DICE A/B/A. PackageEvaluation's retained Bzl module closure owns the
frozen heaps; the slice owns only compact identity and pointer metadata.
No new DICE key, lock, cache, interner, raw-pointer registry or second
attribute schema. The Stage 9 adopted compact/Arc/Allocative patterns
remain sufficient; avoid an optional pointer on every AttributeSchema.

Keep the ordinary dependency labels derived from typed values for Query.
At the configured Starlark-root entry in `app/slug_analysis_v2/src/dice.rs`,
reject a selected rule with a retained nonempty attachment before transition,
selector, child dependency, toolchain or rule implementation computation.
The error is a typed unsupported analysis result, never a successful plain
dependency. Existing zero-edge WP825 values remain eligible for their
previous analysis path. This is a necessary lazy phase boundary toward
actual aspect analysis, not a claim that aspect implementation is optional.
No memory escapes evaluator lifetime; package invalidation/eviction releases
the sparse row with its retained frozen-module closure. No hot-path
benchmark is selected for this cold Loading/early-analysis gate.

## Evidence and validation

First compile and exact-preflight a protobuf-shaped nonempty attached
`deps` Loading test on clean base; it must fail at the current aspect guard.
Then assert retained attachment index, defining label/exported name,
canonical typed labels, required-provider constraints, ordinary dependency
list and initializer execution where applicable. Add an imported aspect
source/attachment A/B/A on one DICE to prove identity and source-fingerprint
restoration; protect WP825 empty/parameter/selector negatives. Add a
focused Query test for ordinary `deps` and attribute projection with no
fabricated aspect node, matched to the accepted Bazel XML/label-kind query.
Add a selected Analysis test that fails before child analysis/actions or
provider consumption, while an unrelated target in the same loaded package
can be configured normally. Preserve existing zero-edge analysis behavior.
No new Stage 1 fixture; reuse the authenticated protobuf/oracle evidence.

Resolve the pinned nightly toolchain. Compile selected Loading, Query and
Analysis binaries with Cargo JSON `--no-run`; exact nonignored preflight
and run focused/protected tests serially. Direct Loading/Query/Analysis/
Core/CLI compile and CLI rebuild precede one normalized-PATH authentic
`//:probe` smoke. That roughly 55-second replay is necessary to show this
package-loading obstacle cleared; it cannot prove aspect execution. Check/
clean `slugd` before/after. Rust format, diff, plan and archive checks and
independent DESIGN/FINAL review of retained identity/analysis fail-closed
are required.

## Scope and stops

Base clean main `5153bbc11`. Allowlist: this manifest, canonical Current,
compact Stage 4/6 owner notes after acceptance; production
`app/slug_loading_v2/src/package.rs` and
`app/slug_analysis_v2/src/dice.rs`; focused tests in
`app/slug_loading_v2/tests/build_file_loading.rs`,
`app/slug_query_v2/tests/loading_query.rs` and
`app/slug_analysis_v2/tests/starlark_rule.rs` (or colocated existing tests
only when needed). Rough growth 130 production/240 proof lines is a review
trigger, not authority for aspect execution. The large `package.rs` remains
the cohesive frozen-rule invocation/target owner; add only one adjacent
sparse projection. REPLAN if source provenance does not cover imported
aspect mutations, the frozen value lacks safe package lifetime, the
authentic target requires configured aspect work to load, the target is
actually selected by the `//:probe` graph, or Analysis cannot reject before
plain dependency/provider use. A later authentic error cannot widen this
packet.

## Acceptance receipt

Independent DESIGN and FINAL reviews ACCEPT. The clean-base protobuf-shaped
Loading test compiled, exact-preflight verified one nonignored test and failed
at the prior nonempty-aspect guard. After the change, JSON `--no-run` compiled
the selected Loading, Query and Analysis binaries; exact preflight verified
5/1/1 nonignored tests and all seven passed, including retained producer
identity, imported aspect A/B/A restoration, selector/parameter/multiple
negatives, ordinary Query base edges and selected Analysis fail-closed before
child computation. Existing source-only Bzl manifest invalidation evidence
complements the imported identity A/B/A. Direct Loading/Analysis and Core/CLI
checks, pinned rustfmt, diff/plan/archive checks and rebuilt CLI passed.
There was no stale `slugd` before or after the smoke. The normalized-PATH
authentic `//:probe` replay cleared protobuf BUILD:682, then exited 2 at
`@@protobuf+//:protobuf.bzl:729` in `internal_php_proto_library` because
`native.filegroup(testonly = testonly)` received a non-boolean value. That
later native-attribute boundary is not part of this packet; configured aspect
application, provider overlays and actions remain unsupported/deferred. M7A
remains partial.

## Immediate predecessor

WP825 accepted at `5153bbc11`: the exact zero-edge, parameterless attached
aspect `LabelList([])` clears rules_cc `cc_binary`'s empty private attr.
Eight selected Loading/Query/Analysis tests, direct checks and rebuilt
authentic smoke pass their bounded gates; the next distinct stop is this
nonempty but unselected protobuf `cc_proto_library` target. M7A is partial.
