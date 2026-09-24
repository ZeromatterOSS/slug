# Current Slug V2 Work Packet

Packet: WP-7-116-m7a-python-internal-guarded-global-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Admit Bazel 9.2's source-guarded `py_internal` predeclared global for the
authentic rules_python Loading closure. The rebuilt authentic `slug build
//:probe` in `/tmp/slug-wp797-authentic-2YQmSt` must clear the undefined
`py_internal` at `py_internal_renamed.bzl:30`; a later independent stop is not
covered. The defining `.bzl` source, not a loading caller or apparent repo name,
selects availability. A guarded source may export the frozen value to a caller
whose own direct global access is denied.

Exact Bazel 9.2 slice: source-label availability and denial; `type(py_internal)
== "struct"`; the source-defined 26 top-level attribute names, with
`PackageSpecificationInfo` bound to the existing provider key, `cc_helper` a
nested struct with its one callable name, and the other 24 attributes
function-typed. `getattr`/`hasattr` for those names must not silently take a
missing-member fallback. The facade is shared/frozen across Bzl evaluations.
Slug-native: diagnostics, callable representation and identity/hash beyond
the frozen reexport, and the existing structural Bzl source identity.
Unsupported/deferred: invocation of the 25 Python-helper functions (including
the nested `cc_helper` predicate), their analysis/context behavior, arbitrary
Python rules and exact Bazel configuration/output identity. Unsupported
invocations must error explicitly, never return a dummy result. This packet
does not claim all `py_internal` semantics from namespace availability.

## Source, owner and decision

Pinned Bazel 9.2 HEAD `8220c6198837d5c13d53fea211cf3282aa12408a`:
`BazelRuleClassProvider.java:293-310` registers `py_internal` for `_builtins`,
`bazel_tools`, `rules_python`, and root `//tools/build_defs/python`;
`ContextGuardedValue.java:39-79` checks the defining canonical `.bzl` label,
with Java word-boundary repository-name matching and package-fragment prefix;
`BzlCompileFunction.java:151-175` applies the guard during source resolution.
`builtins_bzl/common/python/py_internal.bzl:19-31,133-163` defines the exact
26-member struct. Authenticated rules_python 1.7.0
`tools/build_defs/python/private/py_internal_renamed.bzl:14-30` binds the
predeclared global without a load and the generated internal repository
reexports it. Bazel 9.2 offline discriminator
`/tmp/slug-wp816-guard-nKcNAt` accepts an export from a local module named
`rules_python`, including use by a root caller, as well as a generated repo
whose canonical name contains a bounded `rules_python` segment. It rejects
direct use from root, near-name `rules_python_helper`, and a root sibling
package. Its positive checks cover all 26 names and nested member; RCs and
downloads were disabled. The authenticated generated-repo mapping confirms
`rules_python++config+rules_python_internal` is the defining source for the
observed stop, not a root global or mapping error.

Slug's `BzlModuleContext` in `app/slug_loading_v2/src/bzl_module.rs` currently
selects globals only by Build/Bzlmod. Pass the canonical defining label into
globals selection at all three root/external/direct-host module-evaluation
sites; do not expose the symbol in BUILD globals or arbitrary `.bzl` globals.
Match the pinned word-boundary repository guard for valid canonical repo
names and the segment-aware root package prefix. Build a source-derived
frozen `AllocStruct` namespace in a small Loading owner using a
`GlobalsStatic` singleton; reuse `BuiltinProviderKey("PackageSpecificationInfo")`
and function-typed explicit-unsupported callables for all 25 deferred methods.
The 26-name table is exact source data, not an open-ended fallback. An absent
field would be unsound: vendored Starlark's `get_attr` returns `Option`, and
`getattr(x, name, default)` would silently take the default.

No new DICE key, lock, side cache or repository probe. Existing root/external
Bzl module keys, canonical source labels, observed source certificates,
repository mapping and frozen loaded-module closure own source selection,
Need/error, invalidation, equality cutoff and A/B/A restoration. The singleton
value is service-retained immutable; each published frozen module retains only
its chosen binding/reexport, not evaluator scratch. Source edits and mapping
changes retain their accepted owner; no historical mutable-host read is added.

## Evidence and validation

Add focused Loading tests for root/foreign/near-name denial, `_builtins`,
`bazel_tools`, `rules_python+` and generated `rules_python++config+...`
allowance, root package-prefix boundary, BUILD denial, and an allowed-source
export consumed by a denied-source caller. Assert all 26 names and kinds,
`getattr`/`hasattr`, provider-key identity, nested member presence, and an
explicit error on deferred invocation. Include a same-DICE source A/B/A or
equivalent retained-module restoration check for the export path, using
existing real Loading routes rather than a command-side inject. Compare the
selected guards to the offline Bazel discriminator and pinned source; do not
grow Stage 1 oracle fixtures absent a demonstrated gap. Reuse existing
provider-key tests as a protected control.

Before the edit, WP815's recorded rebuilt smoke supplies the authentic
undefined-global negative. Resolve pinned-nightly Cargo toolchain, JSON
`--no-run` compile for Loading's selected tests, exact nonignored selector
preflight, serial focused/protected tests, direct Core and CLI dependent
compile, Rust format and diff checks. Rebuild `slug_cli_v2` before one
normalized-`PATH` authentic smoke; its roughly 52-second execution is
necessary to prove the transitive guard clears. Check/clean `slugd` before
and after. Independent DESIGN review precedes implementation and FINAL
review checks the actual diff/evidence. No broad suite or remote transport;
do not read or alter BuildBuddy authentication.

## Scope and stops

Base clean main `7bb492885`. Allowlist: this manifest and canonical Current
paragraph; Stage 4 Loading owner note after acceptance; production
`app/slug_loading_v2/src/py_internal.rs`, `src/lib.rs`, `src/package.rs`,
`src/bzl_module.rs`; focused Loading tests in `src/host_package_load_tests.rs`
or a small dedicated test module. Approximate growth 160 production / 180
proof lines is a review trigger, not permission to weaken the 26-member table.
The >10,000-line package owner is touched only for one guarded registration;
the new module owns the static source guard and facade. REPLAN if source-keyed
globals cannot preserve frozen reexport or if selected Loading calls a deferred
helper, demanding a new semantic admission. A later stop is a successor, not
authority to widen this packet. Exact helper invocation requires separate
source/oracle proof.

## Acceptance receipt

Independent DESIGN and FINAL reviews ACCEPT the finite source guard and
26-field namespace. The pinned Bazel 9.2 source and offline query oracle
distinguish defining-source access from root/near-name/sibling denial,
generated repository word boundaries, frozen export and field shape. The
source-labelled selector now runs at all three admitted `.bzl` evaluation
sites; BUILD globals remain unchanged. One `GlobalsStatic` heap retains the
complete struct and its nested callable/provider values, while all 25 helper
invocations explicitly error. No DICE key, lock or repository side cache was
added.

Pinned-nightly JSON Loading `--lib --no-run` compiled the final test binary;
exact nonignored preflight selected five tests and all five passed serially:
source guard, all 26 fields and provider equality/hash, same-DICE real
allowed-export/root-BUILD-use A/B/A with direct-root denial, and two protected
provider/frozen-module controls. Initial integration attempts exposed only
fixture setup needs (the test transaction's hardcoded `dep+` materialization
and root module's unrelated rules_license registry request); the test helper
was parameterized for `rules_python+`, with the normal `dep+` behavior
preserved. Direct Core and CLI `cargo check` passed, and `cargo build -p
slug_cli_v2` rebuilt the binary. The normalized-`PATH` authentic one-shot
smoke clears the old undefined `py_internal` and stops separately at
`@@rules_python+//python/private:attr_builders.bzl:89`, `only one required
aspect is supported` (exit 2, roughly 41 seconds). This packet does not
admit required-aspect breadth. Pre/post checks found no `slugd`; Rust format,
diff, archive and plan-status checks pass. No remote service or BuildBuddy
credentials were used. Packet wall time from draft through review was about
20 minutes; the five final selected tests ran in under 0.1 second combined,
with Cargo preparation and the authentic smoke dominating elapsed time.

## Immediate predecessor

WP815 accepted at `7bb492885`: bare/implicit None rule return now produces
zero explicit providers and the authentic smoke reaches this separate
rules_python guarded-global stop.
