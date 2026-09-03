# Current Slug V2 Packet

Packet: WP-4-7A-rule-label-computed-default-declaration-retention-implementation-r1

Milestone: M7A bootstrap-critical loading/ruleset closure. Admit the complete
selected Bazel 9.2 `attr.label(default = <Starlark function>)` declaration and
frozen/imported callable lifetime while keeping every target invocation that
could consume such a default fail closed before package mutation.

Status: the docs-only audit and independent packet/retained-representation
review return `ACCEPT`. Implementation is active and authorized only within
this manifest's exact allowlist, caps, proofs and stops.

## Accepted predecessor and authenticated replay

Commit `db9e693e7` terminally accepts
`WP-4-7A-rule-initializer-declaration-retention-implementation-r1` at 21
production and 131 proof gross Rust additions, 152 total. The terminal reviewer
accepted the pinned-source-shaped three-declaration proof. Serial validation
passes:

- `slug_loading_v2 --lib`: 535 passed, 1 ignored;
- loading integration targets: 51/29/8/6/2/1/5/1, all passed;
- `slug_query_v2 --lib`: 55/55;
- direct pinned-nightly `slug_cli_v2` rebuild, formatting, diff, archive and
  daemon-hygiene gates.

The rebuilt bounded-PATH replay

```text
env PATH=/usr/bin:/usr/local/bin /home/wgray/slug/target/debug/slug cquery \
  //pkg:probe --@rules_rust//rust/toolchain/channel=nightly \
  --lockfile_mode=off
```

clears all selected initializer declarations, then stops while declaring
selected rules_cc 0.2.4 `cc_shared_library` with
`rule attribute _def_parser uses a default form deferred outside this packet`.
The rendered call is line 857 and `_def_parser` line 1081; the durable release
source has the same expressions at lines 863 and 1087.

## Exact selected rules_cc closure

The durable BCR descriptor
`https://bcr.bazel.build/modules/rules_cc/0.2.4/source.json`, SHA-256
`2bd87ef9b41d4753eadf65175745737135cba0e70b479bdc204ef0c67404d0c4`,
selects
`https://github.com/bazelbuild/rules_cc/releases/download/0.2.4/rules_cc-0.2.4.tar.gz`,
a 276,390-byte, 400-entry release archive with SHA-256
`8dcd63392f0bb48adf74f413a9f39ba0fedcb8f99bf085a3b450f06d171dbb6d`
and integrity `sha256-jc1jOS8LtIrfdPQTqfOboP7cuPmb8IWjtFDwbRcdu20=`.
An exact full-archive scan finds one Starlark computed-default callback, three
descriptor construction sites and four rule consumers:

| Source-relative path | SHA-256 | Bytes/lines; mode | Selected role |
|---|---|---:|---|
| `cc/common/semantics.bzl` | `6eb89858e52eb3c50dcd1575f734585083752dd4121dcf09f709ed395dee0f4a` | 7,003/216; 0664 | `_def_parser_computed_default(name, tags)` returns `None` or the Bazel-tools label; `_get_def_parser()` constructs `attr.label(default = callback, allow_single_file = True, cfg = "exec")` |
| `cc/private/rules_impl/attrs.bzl` | `c368203a345cb0d74d461c77e53d1b468ecd23e07fb0fae083e64b34d65eda42` | 18,749/417; 0664 | `_def_parser` in `cc_binary_attrs` |
| `cc/private/rules_impl/cc_binary.bzl` | `d9d0f68e028ee64ef9beb73a2b51f308be5b60545b79ce27daa532b430fbc69f` | 41,488/854; 0664 | consumes `cc_binary_attrs` |
| `cc/private/rules_impl/cc_test.bzl` | `6787e5a152ce2e0ec7744a885086ad9977a0ede1da4bb3abd7f69331947ee28f` | 6,206/165; 0664 | clones `cc_binary_attrs` |
| `cc/private/rules_impl/cc_library.bzl` | `79af1daa5d12f07b3dd6a489e781bfa2c973b520e883b9ab8c024ee6d0c1925b` | 38,773/962; 0775 | direct `_def_parser` descriptor |
| `cc/private/rules_impl/cc_shared_library.bzl` | `b188922d966110b8f7bd68385f896652488acb7bd669275ef3de0dc6757ca1c7` | 52,876/1,150; 0664 | direct `_def_parser` descriptor and first replay stop |

All files have a trailing LF. No other archive expression passes a Starlark
function as an attribute default. Other defaults are literals or already-owned
label/configuration categories. Thus this one generic constructor category
closes all three selected construction sites and all four declarations without
special-casing `_def_parser`, rules_cc or C++.

## Bazel 9.2 authority

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes the category:

- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkAttrModuleApi.java`
  (SHA-256
  `af70c851882fa049034184dbb6f6580731cfa738d79dfb8abcf61af176257670`,
  lines 315-348) admits a Starlark function as `attr.label`'s default;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkAttrModule.java`
  (SHA-256
  `388421c44c623c1c6625fd9f2b059d2a7d1e13b8d45e7c96173f24866a917967`,
  lines 315-370) distinguishes a `StarlarkFunction`, retains a callback helper
  and records its parameter names in a computed-default template;
- `src/main/java/com/google/devtools/build/lib/packages/Attribute.java`
  (SHA-256
  `fbe208c37ad4ed88030f874fa6cd8bd5cf2f4aac63f9a01a4ff24ca499c9a6a4`,
  lines 1367-1515) owns dependency-name ordering, callback invocation,
  `None` fallback and result type validation;
- `src/main/java/com/google/devtools/build/lib/packages/AttributeProvider.java`
  (SHA-256
  `995e75ea72f98dfe3e69f3a2b4a95808f064dd66273f6ff15a49a9391c00b046`,
  lines 330-435) evaluates ordinary defaults before computed defaults and
  precomputes configurable combinations; and
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkIntegrationTest.java`
  (SHA-256
  `ced8fc27cbe35bf30174678800d29b73012f800bff00bcdff6a5cf8c78fef836`,
  lines 1450-1510) discriminates label callback inputs and outputs.
- `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleClassFunctionsTest.java`
  (SHA-256
  `e09c93616e096d639ec69b6b0c6a397a8a36bc8a95fa21b986cb5fc7f8f010aa`,
  lines 7169-7190) proves callback result validation for the separately
  deferred `label_list` category; and
- `src/test/java/com/google/devtools/build/lib/analysis/ConfigurableAttributesTest.java`
  (SHA-256
  `d0d7186241fdedd054a133146830a9f49fc943d2a8eb45b683400d8b8a92abe6`,
  around line 1502) proves the separately deferred configurable-dependency
  behavior.

The analogous callback categories on `label_list`,
`string_keyed_label_dict` and `label_keyed_string_dict`, configurable
precomputation, and callback execution are evidence for the explicit stop, not
implementation scope.

## Audit verdict and compatibility classification

Audit result: `ACCEPT` for one complete generic **`attr.label` function-default
declaration-retention** category.

Classify as **exact**: recognize only an ordinary Starlark function/lambda in
`attr.label(default = ...)`; retain that callable and its exact parameter
metadata through descriptor and rule freeze, import and re-export; admit the
selected `name, tags` shape and all three construction/four consumer sites;
keep declaration lazy; and preserve literal, `None`, late-bound label defaults
and rules without computed defaults unchanged. Existing aspect, subrule,
symbolic-macro, repository-rule and tag-class rejection remains unchanged.

Classify as **Slug-native**: the compact sparse retained representation below
and this stable target-invocation diagnostic:

`target invocation for computed-default attribute '<name>' is unsupported`

Reject a valid named target invocation before `PackageRecorder`, unknown-
attribute checks, coercion, output or target publication. Explicitly supplying
the attribute does not bypass the rejection. If a rule also has an initializer,
the already-accepted initializer diagnostic retains precedence.

Keep **unsupported/deferred**: callback invocation and label context; parameter
dependency semantics and ordering; `name`/`tags` argument values; `None` to
intrinsic-default conversion; result coercion/type checks; selectors and
Cartesian precomputation/limits; explicit-value bypass; dependency loading,
query and configured semantics; every other callback-supporting attribute
constructor; output computed defaults/materializers; and C++ behavior. Never
silently substitute `None`/empty or discard the callback.

## Ownership, representation and incremental safety

`app/slug_loading_v2/src/package.rs` is the sole production owner. Change
`AttributeDefinitionGen<V>::computed_default: bool` to `Option<V>` so the
descriptor owns one existing-heap callable pointer. Preserve existing
`.is_some()` rejection in all unadmitted consumers.

Live anchors are `RuleDefinitionGen`/`FrozenRuleDefinition` at lines 3697/3730,
`RuleAttributeSchemaGen` at 4775, `AttributeDefinitionGen` and its conversion/
freeze at 5419-5616, default discrimination at 6289, frozen invocation at 7263,
and the current typed rule-level rejection at 8768. starlark-rust
`Value::parameters_spec()` is deliberately present only for `def`/lambda and
the frozen equivalents, and the retained function itself owns those parameter
names; no parallel metadata collection is required.

Add a private sparse `ComputedDefaultRuleAttributeGen<V>` containing only a
`u32` schema index and callback `V`. The transient rule definition owns a
schema-ordered `Vec` and the frozen rule definition owns an immutable `Arc`
slice. Adjust indexes by the existing builtin count as late-bound attributes
already do. Do not add a pointer to every `RuleAttributeSchemaGen`: sparse
storage avoids permanent per-attribute overhead and preserves association.

Reuse starlark-rust `Value`/`FrozenValue`, `Vec` construction scratch, the
existing frozen-module lifetime closure, `Arc<[T]>`, `u32` and `Allocative`.
Freeze/import/re-export clone pointers or the Arc in constant time. Add no map,
set, interner, string, raw pointer, cache, registry or new heap owner. The
callback never enters `StarlarkRuleImplementation`, final `PackageEvaluation`,
analysis, provider/action state or semantic equality because target invocation
stops before lowering.

Existing source digest, recursive load-manifest fingerprint and DICE equality
own add/remove/change invalidation. There is no new key, input, observation,
lock, await, task, retry or fallback. Overlapping requests share only existing
immutable frozen modules. The large `package.rs` remains cohesive here because
it already exclusively owns descriptor creation/freezing, rule freezing and
the pre-recorder invocation boundary; splitting that lifecycle would add a
second owner without reducing retained complexity.

No benchmark is required for this cold declaration boundary, but proof must
report exact sizes and allocation accounting. Stage 9 records this retained-
representation choice. Independent retained-representation review returns
`ACCEPT`; implementation may use only this frozen sparse design.

Deletion condition: a separately reviewed complete runtime packet replaces the
guard and declaration-only carrier only after admitting the constructor's
callback arguments/results, label context, configurable combinations, package
mutation, query/configured identity and invalidation. It must not move the
callable into configured state without a new retained-state review.

## Required proof

Adjacent tests must prove:

- ordinary def and lambda defaults are accepted only by `attr.label`; native
  callables/nonfunctions retain their current noncomputed/invalid behavior;
- transient and frozen descriptors retain callback identity and exact parameter
  names across freeze/import/re-export;
- all three selected construction patterns and four rule declarations coexist
  lazily without callback execution;
- sparse entries are schema ordered with correct `u32` index/name, including
  multiple distinct computed attributes and builtin-index adjustment;
- `Option<Value>`/`Option<FrozenValue>` are pointer-sized; the sparse entry is
  at most two machine words; exact final owner sizes are reported; and
  `Allocative` accounts for the immutable sparse slice;
- non-label function defaults retain the current rule-level deferred error and
  unadmitted macro/aspect/subrule/repository/tag consumers remain fail closed;
- computed-bearing target invocation returns the exact diagnostic before
  unknown-attribute/coercion/recorder/output/target effects, including with an
  explicit value; initializer plus computed default reports initializer first;
- a failed invocation followed by a clean evaluation on the same DICE has no
  leaked state, while no-computed rule loading/equality is unchanged; and
- source A/B/A plus callback add/remove/change proves existing recursive-
  manifest invalidation and marker restoration without new state.

The authenticated replay must clear all four selected declarations and stop at
the next independent typed boundary without invoking the callback.

## Allowlist, caps, validation and stops

Only these files may change during implementation:

- `app/slug_loading_v2/src/package.rs`, sole production owner and adjacent unit
  proof; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, proof only for frozen
  ordinary/Bzlmod import/re-export and package loading.

Caps: 80 production Rust, 180 proof Rust and 260 aggregate gross additions;
within them, `package.rs` is capped at 80 production plus 100 proof and the host
test at 80 proof. No docs, fixture, asset, Cargo manifest or other Rust file may
change during implementation.

Run serial focused declaration/freeze/import/rejection/invalidation tests,
then the full loading library/integrations, query library, CLI rebuild and exact
bounded-PATH replay above; finish with stale-`slugd`, formatting, diff, archive,
allowlist and cap gates.

Return `REPLAN` if clearing replay requires callback execution; another
constructor/runtime behavior enters scope; the callable outlives the existing
frozen-module closure; the marker enters final package/analysis/configured
state; package mutation precedes rejection; a per-schema pointer, new key,
cache, map, interner, fixture or consumer branch is proposed; another production
owner is required; or the allowlist/caps fail.
