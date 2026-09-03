# Current Slug V2 Packet

Packet: WP-4-7A-rule-initializer-declaration-retention-implementation-r1

Milestone: M7A bootstrap-critical loading/ruleset closure. Admit Bazel 9.2's
generic optional `rule(initializer = ...)` declaration and frozen/imported
callable lifetime while keeping every initializer-bearing target invocation
fail closed before package mutation.

Status: the docs-only audit and independent packet review return `ACCEPT`.
Implementation is active and authorized only within this manifest's frozen
allowlist, caps and stops.

## Accepted predecessor and authenticated replay

Commit `10aaed332` terminally accepts
`WP-4-6-7A-apple-common-declaration-provider-fail-closed-implementation-r1`
at 141 production and 180 proof gross Rust additions, 321 total. Focused
loading/provider/analysis proofs pass. Serial validation passes:

- `slug_loading_v2 --lib`: 533 passed, 1 ignored;
- loading integration targets: 51/29/8/6/2/1/5/1, all passed;
- `slug_analysis_v2 --lib`: 19/19;
- `slug_query_v2 --lib`: 55/55;
- `slug_cli_v2` build, formatting, diff, archive and daemon-hygiene gates.

The rebuilt bounded-PATH replay

```text
env PATH=/usr/bin:/usr/local/bin /home/wgray/slug/target/debug/slug cquery \
  //pkg:probe --@rules_rust//rust/toolchain/channel=nightly \
  --lockfile_mode=off
```

clears the former `apple_common` stop. At toolchain-registration row 14 it
loads `@@rules_java+//toolchains/BUILD` -> selected rules_cc 0.2.4
`cc_binary.bzl` -> `attrs.bzl` -> `cc_shared_library.bzl`, then fails before
any configured Apple operation with
`Found initializer extra named parameter(s) for call to rule`. The rendered
trace calls the rule at line 857 and shows `initializer` at line 859; the
durable release source has the same expression at lines 863-865.

## Bazel 9.2 authority

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes the category:

- `src/main/starlark/builtins_bzl/common/cc/cc_shared_library.bzl` is the
  bundled analogue of the selected first consumer;
- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleFunctionsApi.java`
  (SHA-256
  `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`,
  lines 698-723) defines named optional `initializer`, default `None`;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleClassFunctions.java`
  (SHA-256
  `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`,
  lines 1765-1881) proves invocation is a distinct later category: copy/lift
  explicit public Starlark attributes plus `name`, omit `None`, isolate
  load-time state, invoke child-to-parent, accept `None`/string-keyed dict,
  merge returned values, preserve `name`, and guard private/base attributes;
- the same file lines 842-855 and installed
  `tools/allowlists/initializer_allowlist/BUILD.tools` (SHA-256
  `79e67fd466f4a6b5be2ffdf925f784374de51dd9584e45f09355c1933c9d4bcb`,
  145 bytes/7 lines) establish configured allowlist behavior; and
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleClassFunctionsTest.java`
  (SHA-256
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`,
  lines 3703-4700) discriminates allowlisting, exact argument selection,
  defaults/`None`, selectors, label dictionaries, return types, unchanged
  name, and private/base-attribute rejection.

Those runtime behaviors are evidence for the stop, not implementation scope.

## Exact selected rules_cc closure

The durable BCR descriptor
`https://bcr.bazel.build/modules/rules_cc/0.2.4/source.json`, SHA-256
`2bd87ef9b41d4753eadf65175745737135cba0e70b479bdc204ef0c67404d0c4`,
selects
`https://github.com/bazelbuild/rules_cc/releases/download/0.2.4/rules_cc-0.2.4.tar.gz`,
a 276,390-byte release archive with SHA-256
`8dcd63392f0bb48adf74f413a9f39ba0fedcb8f99bf085a3b450f06d171dbb6d`
and integrity `sha256-jc1jOS8LtIrfdPQTqfOboP7cuPmb8IWjtFDwbRcdu20=`.
An exact scan of all 400 archive entries finds only three initializer-bearing
rules, all ordinary `0644` trailing-LF files:

| Source-relative path | SHA-256 | Bytes/lines | Declaration |
|---|---|---:|---|
| `cc/private/rules_impl/cc_shared_library.bzl` | `b188922d966110b8f7bd68385f896652488acb7bd669275ef3de0dc6757ca1c7` | 52,876/1,150 | initializer definition 841-861; rule 863-865 |
| `cc/private/rules_impl/cc_binary.bzl` | `d9d0f68e028ee64ef9beb73a2b51f308be5b60545b79ce27daa532b430fbc69f` | 41,488/854 | rule 818-820; imports shared initializer |
| `cc/private/rules_impl/cc_test.bzl` | `6787e5a152ce2e0ec7744a885086ad9977a0ede1da4bb3abd7f69331947ee28f` | 6,206/165 | initializer 99-113; rule 115-117 |

All three operands are ordinary Starlark functions. Their bodies are lazy at
module declaration. The first replay stop occurs while freezing the shared
library declaration, so no initializer call, target mutation or configured C++
behavior is required to clear this boundary.

## Audit verdict and compatibility classification

Audit result: `ACCEPT` for one complete **declaration-retention** category.

Implement the optional `initializer` slot generically in the existing transient
and frozen rule definitions. Omitted and explicit `None` mean no initializer;
a present value must pass the existing Starlark-function validation used by
callable definition APIs. Freeze it with the rule and retain
that one frozen pointer through ordinary and Bzlmod module load, import,
re-export and package BUILD evaluation. Declaration and freeze must not
execute the callable.

For a valid named target call on a rule with an initializer, return the stable
Slug-native diagnostic

`target invocation for rule initializer is unsupported`

before `PackageRecorder` access, attribute coercion, target/output insertion or
other package mutation. Existing positional/name call-shape errors may retain
their current precedence. A rule with omitted/`None` initializer remains on the
unchanged package-lowering path.

Classify as **exact** for the selected consumer: the named optional declaration
shape, `None` default, acceptance of the three selected Starlark functions,
lazy declaration, freeze/export/import/re-export availability, and unchanged
rules without an initializer.

Classify as **Slug-native**: one frozen-pointer representation and the explicit
invocation rejection while runtime semantics are unadmitted.

Keep **unsupported/deferred**: initializer execution; argument copy/lift and
label context; omission/default/selector semantics; return-dict validation and
merge; mutation isolation; `native.package_relative_label`; name/private/base
attribute rules; experimental/configured allowlisting; parent rules and
child-to-ancestor chaining; configured rule behavior, C++ semantics and any
consumer-specific bypass. Never silently ignore an initializer.

## Ownership, retained memory and incremental safety

`app/slug_loading_v2/src/package.rs` already solely owns `rule()`, transient
`RuleDefinitionGen<Value>`, `Freeze`, `FrozenRuleDefinition::invoke`, package
recording and adjacent unit tests. It is the only production owner.
`app/slug_loading_v2/src/host_package_load_tests.rs` is the existing proof-only
owner for recursive ordinary/Bzlmod imported-module package loading.

Retain only `Option<Value>` transiently and `Option<FrozenValue>` after freeze,
using starlark-rust's existing pointer-sized/niche representation and
`Allocative` treatment. The pointer belongs to the same frozen module heap as
the rule. Existing `FrozenBzlModule` and `FrozenBzlLifetimeEntry` ownership
keeps that heap alive through imports and BUILD evaluation. Add no owned heap,
raw pointer, map, set, vector, string, interner, registry, cache or copied
callable.

The initializer never reaches `StarlarkRuleImplementation`, configured target
state or `PackageEvaluation` semantic equality because every initializer-
bearing target call stops before lowering. Existing module source digest,
recursive manifest fingerprint and DICE key/value equality own source change
and invalidation. Identical failed calls publish no package. No request input,
DICE key, observation, lock, await, task, retry or fallback changes; overlapping
requests share only existing immutable frozen modules and cannot publish
partial initializer state.

Buck2/V1 review selects no extraction. Reuse starlark-rust `Value`/
`FrozenValue`, Slug's existing frozen-module lifetime closure and `Allocative`.
The pointer is constant-time to freeze/clone. No benchmark is required.

Deletion condition: a separately reviewed packet may remove the invocation
guard only when it admits the complete Bazel runtime category above, proves
heap-safe copied inputs and returned values, models exact label contexts and
allowlisting, and preserves structural package equality/invalidation. It must
not carry the initializer pointer into configured state unless a new reviewed
semantic need proves that necessary.

## Required proof

Adjacent tests must prove:

- omitted and explicit `None` preserve current rule declaration/invocation;
- a function initializer declares, exports and freezes without executing;
- `Option<Value>` and `Option<FrozenValue>` remain pointer-sized and the frozen
  field participates in the existing `Allocative` rule-definition owner;
- all three selected rules_cc declaration shapes coexist and remain lazy;
- imported/re-exported frozen rules retain the initializer marker under
  ordinary and Bzlmod loading;
- non-function initializer values, including other callable value kinds, fail
  at declaration and publish no module;
- valid target invocation returns the exact diagnostic before attribute
  coercion or package/target/output publication;
- failed initializer-bearing invocation followed by a clean evaluation has no
  leaked package state; and
- rules without an initializer retain existing package values, equality and
  loading behavior.

The authenticated replay must clear all three selected declarations and stop
at the next independent typed boundary without executing an initializer body.

## Allowlist, caps, validation and stops

Only these files may change:

- `app/slug_loading_v2/src/package.rs`, for the sole production change and
  adjacent unit proof; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, proof only for frozen
  import/re-export and authenticated package loading.

Caps: 32 production Rust, 160 proof Rust and 192 aggregate gross additions.
No documentation, fixture, asset, Cargo manifest or other Rust file may change
during implementation.

Run serially with the pinned direct nightly toolchain:

- focused rule-initializer declaration/retention/rejection tests;
- `cargo test -p slug_loading_v2 --lib --quiet` and loading integrations;
- `cargo test -p slug_query_v2 --lib --quiet`;
- `cargo build -p slug_cli_v2 --quiet`, stale-`slugd` cleanup, and the exact
  bounded-PATH replay above; and
- `cargo fmt --all --check`, `git diff --check`, archive hygiene, exact
  allowlist and cap checks.

Return `REPLAN` if clearing the selected declaration requires executing an
initializer; a callable outlives the existing frozen-module lifetime closure;
the marker must enter `StarlarkRuleImplementation` or another retained graph;
package mutation occurs before rejection; a new key/lock/cache/context/fixture
or consumer branch is proposed; an additional production owner is required;
or the allowlist/caps fail.
