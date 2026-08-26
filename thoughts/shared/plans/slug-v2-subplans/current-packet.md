# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-label-global-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: shared loading Label value, recursive Bzl provenance and fixed aspect adapter
Base: `650075d8`

Result: add the smallest exact `.bzl` `Label(...)` construction and canonical
stringification vertical needed to load the complete fixed
`rust_analyzer_aspect` declaration. Resolve imported-function calls against
their defining `.bzl`, never merely the outer evaluator. Stop before later
rules_rust declarations, general repository mapping or aspect application.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkRuleFunctionsApi.Label` installs a `.bzl` constructor with one
  positional string-or-Label input; Label input is returned unchanged.
- `BazelModuleContext.ofInnermostBzlOrFail` and
  `StarlarkRuleClassFunctions.label` select the innermost executing Starlark
  function's module, not the builtin exporter or outer evaluator.
- `cmdline.Label` owns repository/package/target identity and canonical `str`;
  focused class-function tests authenticate construction, idempotence and the
  already-admitted narrow value properties.
- `StarlarkIntegrationTest.testLabelConstructorFailsInBuildFile` proves that a
  loaded alias still rejects in BUILD.

The accepted rules_rust archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Its first remaining source-order expression is
`str(Label("//rust:toolchain_type"))` at
`rust/private/rust_analyzer.bzl:210` inside the already-admitted fixed aspect.

Slug's vendored Starlark `DefInfo` already retains the definition `CodeMap`.
Expose one typed native-caller definition-filename accessor; never parse
diagnostic stack text. `BzlLoadManifest.reachable` already pairs each retained
recursive module's exact logical path with its canonical label. The top-level
manifest root handles direct calls/aliases; an imported function must map its
definition filename through that closure or fail closed. BUILD has no
`BzlEvaluationContext` and therefore rejects before frame resolution.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is concept/test guidance only.
Its generic Label layer retains canonical identity in the value while a shared
builtin consults executing-definition module context. Do not copy its parser,
repository mapping, observer, runtime, scheduler or storage. Bazel 9.2 remains
authoritative.

The Buck2 utility-reuse audit selects the existing `CanonicalLabel`, compact
strings, Starlark simple/frozen value and Arc-backed manifest closure. Move and
rename the existing `InvocationLabel` into one small shared loading module;
update both consumers. Add no second wrapper, interner, hash domain, cache,
registry or Stage 9 ledger entry.

## Decision and non-decisions

Add `Label` only to complete `.bzl` globals. Admit:

- one positional string in `//package:target` or `:target` form;
- an existing shared Starlark Label input, returned as the same value;
- defining-repository/package resolution from the top-level manifest root or
  typed caller-definition filename;
- the already-accepted Label value's canonical str/repr/hash/equality,
  `name`, `package`, repository-name aliases and `same_package_label`; and
- the fixed aspect adapter's acceptance of a canonical string only when its
  repository equals the aspect's defining repository.

The `:target` form is included to discriminate imported-function ownership.
Reject bare strings, explicit `@`/`@@` repositories, malformed labels, missing
frame-to-manifest mappings and BUILD calls. Do not add repository mapping,
non-visible repositories, special-package behavior, new Label properties or
methods, attribute conversion/defaults, rule aspect parameters, propagation,
selection, analysis, actions or later rules_rust call shapes.

## Ownership, revision and lifetime

The calling Bzl module/function is the producer of resolution context. The
existing manifest root/reachable identities are the sole path-to-canonical
source; `BzlEvaluationContext` holds only an evaluation-scratch projection.
The resulting shared Label value owns one `CanonicalLabel`, whose repository,
package and target drive equality/hash. Display is a projection, never a
second identity.

Existing recursive Bzl DICE keys observe every source and retain the manifest
and frozen module closure. Source or load edits invalidate the context before
evaluation; no new key, observation, request overlay, filesystem inference or
command repair is added. Concurrent requests keep their existing transaction
isolation.

Context maps and native-caller filename strings are evaluator scratch. A Label
assigned into a module is frozen/DICE-retained with that module and released
with it. No service cache, command retention, async transfer, cancellation,
task, eviction or shutdown duty is added.

## Files and caps

Only these Rust files may change, against the listed base SHA-256:

| File | Base SHA-256 | Final line cap |
|---|---|---:|
| `starlark-rust/starlark/src/eval/runtime/evaluator.rs` | `67b701eb2ec7af89a58843d41d11a176a33b6ae7ce66fbff924e591b1f6c9378` | 1,225 |
| `app/slug_loading_v2/src/lib.rs` | `af17ce9306a10779e6faffd49fb4951e8d9485fafe907165707ddbede289f918` | 120 |
| `app/slug_loading_v2/src/starlark_label.rs` | new | 155 |
| `app/slug_loading_v2/src/module_extension.rs` | `3b823dd2f971332955162d2b74bf6ad97a205eead5ada0573d899ef7ab83abcb` | 2,430 |
| `app/slug_loading_v2/src/module_extension_repository_rule.rs` | `4ac465f184b5c2df37e1dae1c4493cb6d75eefef1b379d3220ad13023b08a35e` | 655 |
| `app/slug_loading_v2/src/provider.rs` | `7b625396c3b841f3b498532993100b765996bef99f819aa49ea2e1bbf57f689d` | 575 |
| `app/slug_loading_v2/src/bzl_module.rs` | `10accf93f7a960834c118812f83f6abc7d805a260e07d9c6e056ed39362abc8a` | 9,660 |
| `app/slug_loading_v2/src/package.rs` | `a8c407a6320b4cba288510b458c9dcaeb7415a9488f87ca2ec625206e46e9e1c` | 5,425 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `79a635c5c25f991daf870a762d040670abe3195c5d52b10e6b22396b28813b51` | 4,375 |

Cap production additions at 300, proof additions at 140 and total additions at
440. The oversized loading files remain cohesive owners: `bzl_module.rs`
constructs manifests/evaluators, `package.rs` assembles the global set/fixed
aspect adapter, and the test file owns recursive loading proof. The Label value
itself is split from the oversized module-extension file because it now has a
second consumer. No touched function may exceed 150 lines. `REPLAN` before a
tenth Rust file or any cap breach.

## Proof and validation

Prove:

- the typed runtime accessor reports a Starlark caller's definition filename
  and reports no function for a direct module-scope native call;
- top-level live construction/stringification completes the fixed aspect with
  canonical toolchain identity;
- a direct re-exported builtin alias uses the calling top-level module, while
  an imported function with `Label(":owned")` uses its defining package;
- the same alias rejects in BUILD and an absent frame-manifest entry fails;
- Label input is idempotent; malformed/bare/explicit-repository inputs reject;
  and existing module-extension Label ABI tests remain unchanged.

Run serially: `cargo fmt --all -- --check`, focused Label/aspect/runtime tests,
full `cargo test -p slug_loading_v2`, focused vendored Starlark tests,
`cargo check -p slug_core_v2 --locked`, `cargo build -p slug_cli_v2 --locked`,
`git diff --check` and `scripts/v2_archive_status.sh`. Clean stale `slugd`
before/after any smoke. Pinned source and local focused tests suffice; broader
upstream Label tests are skipped because repository mapping, special cases and
wider methods are unsupported. Do not add an oracle fixture or run Bazel.

## Compatibility and STOP

- **Exact:** the admitted `.bzl` placement/input forms, typed defining-function
  context, Label idempotence/value surface, canonical str and fixed-aspect
  same-repository toolchain projection, plus BUILD alias rejection.
- **Slug-native:** Rust representation, no cross-call object interning,
  valid-Unicode strings and nonrequired diagnostics.
- **Unsupported/deferred:** every excluded Label spelling/mapping/API and all
  aspect attachment/application/analysis, later rules, bool/list targets,
  M8/M7B and exact output bytes.

STOP on dirty overlap, wrong/exporter/outer-evaluator context, stack-text or
path inference, duplicate Label identity, guessed mapping, BUILD visibility,
aspect execution/application, Zabel behavior/code, Java/JVM, fixture/network/
dependency drift, public-success claims or cap breach. `REPLAN` before widening.
