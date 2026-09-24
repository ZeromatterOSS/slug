# Current Slug V2 Work Packet

Packet: WP-7-125-m7a-empty-aspect-dependency-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the zero-propagation attached-aspect attribute in the authenticated
protobuf/rules_cc `cc_binary` Loading call. `@@rules_cc+//cc:cc_binary.bzl:44`
currently stops at an unconditional target-invocation guard for private
`_deps_analyzed_by_graph_structure_aspect`. Exact Bazel 9.2 slice: an
aspect-bearing `attr.label_list` whose final typed value is literally empty
has no dependency edge on which to propagate its aspect, provided the
attached aspect declares no attributes, required parameters or required
aspects. The rule target and empty value may be recorded. Slug-native
structural package identity and diagnostics remain. Unsupported/deferred:
any nonempty, selected or concatenated aspect-bearing dependency value,
aspects with parameters/required aspects, actual aspect application,
configured providers/actions and configured C++. Do not treat an empty
branch of an unresolved selector as an empty final value. A Loading advance
does not claim protobuf compilation.

## Source, owner and decision

Pinned rules_cc 0.2.17 `cc/private/rules_impl/cc_shared_library.bzl:827-834,
1087-1115,1129-1134` declares this private attr as
`attr.label_list(providers=[CcInfo], aspects=[graph_structure_aspect])`
without an explicit default. The graph aspect has no attributes, required
parameters or required aspects. `dynamic_deps_initializer` returns kwargs
unchanged without nonempty `dynamic_deps`; protobuf v33.4's `cc_binary`
call has none. rules_cc `cc_binary.bzl:851-853` invokes that initializer.
Pinned Bazel 9.2 `StarlarkRuleClassFunctions.java:1776,1806-1824,
1844-1858,1900-1903` validates attached-aspect parameters at rule
invocation even when the dependency value is empty, omits private attrs
from initializer inputs and allows rules_cc's private return only in the
dynamic-deps case. `AspectsList.java:87-116` checks public parameters and
required rule attrs. An ignored-RC/offline Bazel query of authentic
`@@protobuf+//:protoc` with XML default values explicitly reports empty
`dynamic_deps` and `$deps_analyzed_by_graph_structure_aspect` lists.
Offline fixture `/tmp/slug-wp825-aspect-zero-32VFsR` demonstrates omitted
and explicit empty values produce no aspect application, whereas nonempty
produces one; this fixture uses a public attr for initializer variants
because arbitrary `.bzl` files cannot return private initializer keys.

Slug's `FrozenRuleDefinition::reject_deferred_attribute_invocation` in
`app/slug_loading_v2/src/package.rs:4494-4505` unconditionally rejects
any declaration with `attached_aspect`; it runs after the initializer but
before the final typed value is assembled. The frozen declaration and
`FrozenAspectDefinition` retain the aspect and its parameter schema, but
the published `AttributeSchema` does not. Move the guard to the bounded
final-value seam: accept only a final literal
`CoercedAttributeValue::LabelList([])` on an attached-aspect `LabelList`
whose aspect has no attributes, required parameters or required aspects;
reject every other aspect-bearing value before target publication. Keep
existing coercion, defaults, initializer execution, provenance and schema
publication unchanged. No aspect is executed or silently discarded for a
nonempty edge. The observed BUILD/loaded-source fingerprints and structural
package equality already own DICE invalidation and A/B/A restoration; no
new key, lock, cache, retained representation, sidecar or semantic fallback.
This cold Loading guard has no performance benchmark.

## Evidence and validation

First add a source-shaped private aspect-bearing empty label-list invocation
test that compiles on the clean base and fails at the selected guard. Assert
typed empty value, implicit provenance and no retained dependency labels;
protect the existing Query edge projection test.
Keep nonempty default, parameterized aspect and unresolved selector
negative, with no partial package publication; change the existing
provider/aspect guard regression to use a genuinely nonempty dependency
rather than expecting an empty zero-edge value to fail. Prove same-DICE
empty/nonempty/empty source restoration. Reuse the pinned/offline Bazel
oracle; no Stage 1 fixture.

Resolve pinned-nightly Cargo. Compile selected Loading/Query/Analysis test
binaries with JSON `--no-run`; exact nonignored preflight and run focused/
protected tests serially. Direct Loading/Query/Analysis/Core/CLI compile
and CLI rebuild precede one normalized-PATH authentic smoke. That replay
is required to prove this exact transitive closure advance. Check/clean
`slugd` before/after. Rust format, diff, plan and archive checks, and
independent DESIGN and FINAL review of the zero-edge boundary are required.

## Scope and stops

Base clean main `5ac86ad7e`. Allowlist: this manifest, canonical Current,
compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs`; selected tests in
`app/slug_loading_v2/tests/build_file_loading.rs` and the existing
`app/slug_loading_v2/src/host_package_load_tests.rs` negative regression.
Rough growth 40 production/150 proof lines is a review trigger, not
permission to implement aspect analysis. REPLAN if the final authentic
value is nonempty/selected, if the empty case has a Bazel-visible aspect
effect, or if accepting it requires dropping a nonempty semantic input.
A later authentic error cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the exact zero-edge Loading
boundary. The source-shaped private-aspect test compiled on the clean base,
passed exact preflight 1, then failed (exit 101) at the selected unconditional
guard. Final pinned-nightly JSON `--no-run` compiled Loading integration/lib,
Query lib and Analysis `starlark_rule` binaries. Exact preflights selected
4, 2, 1 and 1 nonignored tests; all eight passed serially. Direct Loading/
Query/Analysis/Core/CLI `cargo check` and required CLI rebuild passed. The
final-source normalized-PATH authentic smoke (exit 2, about 53 seconds)
clears rules_cc `cc_binary`'s private empty aspect attr and next fails
separately at `@@protobuf+//:BUILD.bazel:682`, through `cc_proto_library`:
target invocation for nonempty aspect-bearing `deps` is unsupported. Pre/
post checks found no `slugd`; Rust format, diff, plan and archive checks
pass. No remote transport or BuildBuddy credentials were used. The 41
production and 192 proof lines modestly exceed rough 40/150-line review
triggers to cover the final-value guard, parameter-validation exclusion,
source-shaped empty/explicit controls, negatives and same-DICE restoration;
FINAL review accepted proportionality. Aspect application, configured C++
and protobuf compilation remain open.

## Immediate predecessor

WP824 accepted at `5ac86ad7e`: unchanged configurable StringList selector
pass-through clears protobuf's rules_cc initializer `copts` stop. Eight
selected Loading/Query/Analysis tests, direct checks, rebuilt CLI and
authentic smoke pass their bounded gates; the next separate stop is this
aspect-bearing private attribute. M7A remains partial.
