# Current Slug V2 Work Packet

Packet: WP-7-124-m7a-initializer-selector-passthrough-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit the authenticated protobuf/rules_cc initializer's unchanged
configurable `StringList` selector pass-through during Loading. Protobuf
v33.4 `@@protobuf+//:BUILD.bazel:356-362` calls rules_cc `cc_binary` with
frozen imported `COPTS` and `LINKOPTS` selects. The rebuilt authentic
`slug build //:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must clear
the current `rule initializer input 'copts' cannot use select()` stop;
any later failure is separate. Exact Bazel 9.2 slice: explicit public
configurable StringList selector enters the initializer unresolved, and
the initializer's same-attribute, same-object return retains its ordered
conditions/branches/default without choosing a configuration. Slug-native:
pointer-identity pass-through gate, structural package identity and
diagnostics. Unsupported/deferred: synthesized, modified, swapped or
concatenated selector returns; other attribute kinds and nonconfigurable
selectors; definition-relative remapping of new returned selector keys;
configured C++ rule semantics, branch selection/actions and exact Bazel
configuration/output bytes. A Loading pass-through is not an executable
`cc_binary` claim.

## Source, owner and decision

Pinned Bazel 9.2 `StarlarkRuleClassFunctions.java:1793-1878` lifts only
explicit non-None public attrs into the initializer and reifies returned
dict values under the initializer defining-label converter;
`BuildType.java:274-299,321-340` copies/lifts symbolic selector branches
without selecting one. Authenticated protobuf `build_defs/cpp_opts.bzl:3-22,
25-52` defines frozen StringList `COPTS`/`LINKOPTS` selects and BUILD
356-362 forwards them. rules_cc 0.2.17 `cc/private/rules_impl/attrs.bzl:168,
330` declares these attrs as `string_list`; `cc_binary.bzl:851-853` runs
PostMark's identity initializer then `dynamic_deps_initializer`, which
returns kwargs unchanged without `dynamic_deps`. The ignored-RC/offline
Bazel 9.2 oracle `/tmp/slug-wp824-init-select-oracle` shows the initializer
receives imported `copts` as `type=select`, direct attrs as lists, and
omitted/None attrs absent. Returning kwargs unchanged preserves both
branches: fastbuild produces `-DDEFAULT`, dbg `-DDBG`; unconfigured XML
lists both branch values and the condition input. This does not prove
Slug's configured C++ path.

Slug's `invoke_rule_initializer` in
`app/slug_loading_v2/src/package.rs:7935-8055` already coerces the
frozen StringList selector to a canonical `CoercedAttributeValue::Selector`
and `allocate_macro_attribute` reifies it as one scratch immutable
`SelectorValue`; an input guard rejects it before callback and a return
guard rejects it afterward. Admit only a pure `Selector` with declared
configurable `StringList` kind. Save a scratch tuple of its attribute name,
reified `Value` and pre-coerced structural value. Execute the initializer
normally. On return, accept a selector only for that same attribute when
`Value::ptr_eq` confirms the same reified object; publish the original
pre-coerced value, never re-coerce it in the caller context. New or swapped
selectors continue to fail closed because Bazel would use the defining
`.bzl` mapping that this packet does not implement. Starlark's opaque
`SelectorValue` exposes no in-place mutation; addition makes a new object.
Do not bypass callback execution or infer selector contents from repr/path.

The existing frozen rule `AttributeValue`, selector branches and derived
`$config_dependencies` own the final fact, with package structural equality
and observed BUILD/loaded-source fingerprints providing invalidation and
same-DICE A/B/A restoration. The new name/`Value`/coerced tuple is
initializer-attempt scratch released on success, failure or cancellation;
no evaluator value enters a retained value. No new DICE key, lock, cache,
interner, retained sidecar, alternate selector representation or semantic
fallback. The large `package.rs` already owns initializer invocation and
selector reification; keep any extraction bounded to those adjacent
functions. This cold Loading seam needs no performance benchmark.

## Evidence and validation

First add a source-shaped imported frozen `COPTS`/`LINKOPTS` identity
initializer test that fails on the unmodified branch at the selected
`copts` input guard. Assert initializer execution using an additional
returned marker; assert frozen output selectors retain exact condition
labels, branch order, values, default, explicit provenance and derived
configuration dependencies. Cover direct list and omitted/None inputs.
Prove same-DICE imported source/selector branch change and A/B/A restore.
Keep synthesized/swapped returned selectors, concatenation, wrong-type
branch and Label selector negative; preserve the definition-time rejection
of `attr.string_list(configurable=False)`, which cannot reach the initializer.
No partial package publication. New returned raw selector keys must not silently
re-map through the caller. Reuse existing selector/initializer tests and
the offline Bazel oracle; no Stage 1 fixture growth.

Resolve pinned-nightly Cargo. Compile selected Loading and direct Query/
Analysis dependent test binaries with JSON `--no-run`, exact nonignored
preflight and serial focused/protected runs. Direct Loading, Query,
Analysis, Core and CLI compile and CLI rebuild precede one normalized
`PATH` authentic smoke. That roughly 40-second replay is necessary to
prove transitive closure advancement that focused tests cannot. Check/
clean `slugd` before/after. Rust format, diff, plan and archive checks
and independent DESIGN/FINAL review cover selector ownership, mapping
gate and fail-closed boundaries.

## Scope and stops

Base clean main `81341fae8`. Allowlist: this manifest, canonical Current,
compact Stage 4 owner note after acceptance; production
`app/slug_loading_v2/src/package.rs`; selected tests in
`app/slug_loading_v2/tests/build_file_loading.rs` and, only if needed for
same-DICE imported-source proof, `src/host_package_load_tests.rs`.
Rough growth 55 production/140 proof lines is a review trigger, not
authority for transformed selectors or configured C++. REPLAN if the
authentic initializer does not return the same object, if pointer identity
cannot safely preserve the original structural value, if mapping/selector
facts escape scratch, or if the selected stop requires configured C++
behavior. A later error cannot widen this packet.

## Acceptance receipt

Independent DESIGN and FINAL reviewers ACCEPT the bounded selector hand-off.
The source-shaped imported-selector test compiled on the clean base, passed
exact preflight 1, then failed (exit 101) at the selected initializer input
guard for `copts`. Final pinned-nightly JSON `--no-run` compiled Loading
integration/lib, Query lib and Analysis `starlark_rule` test binaries. Exact
preflights selected 4, 2, 1 and 1 nonignored tests; all eight passed serially.
The two Loading lib tests protect initializer reexports and failure atomicity.
Direct Loading/Query/Analysis/Core/CLI `cargo check` and CLI rebuild passed.
The normalized-PATH authentic smoke (exit 2, about 55 seconds) clears the
initializer `copts` selector stop and next fails separately at
`@@rules_cc+//cc:cc_binary.bzl:44`: target invocation for aspect-bearing
attribute `_deps_analyzed_by_graph_structure_aspect` is not supported.
Pre/post checks found no `slugd`; Rust format, diff, plan and archive checks
pass. No remote transport or BuildBuddy credentials were used. The 26
production and 255 proof lines exceed the rough 55/140-line review trigger
only on proof: source-shaped multi-selector values, negative boundaries and
same-DICE imported-source restoration require separate checks; final review
accepted proportionality. Configured C++ and action behavior remain open.

## Immediate predecessor

WP823 accepted at `81341fae8`: direct integer 0/1 Boolean Starlark-rule
call values clear rules_license's `testonly=0` stop. Six selected Loading/
Query/Analysis tests, direct checks, rebuilt CLI and authentic smoke pass
their bounded gates; the next independent stop is this initializer input.
