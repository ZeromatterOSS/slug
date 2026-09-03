# Current Slug V2 Work Packet

Packet: WP-4-6-7A-exec-group-declaration-closure-fail-closed-implementation-r1

Status: independent architecture and retained-state review `ACCEPT`; active
implementation authorized only within this frozen packet.

## Predecessor checkpoint and selected stop

Commit `64a0f29f6` terminally accepts generic `attr.label` computed-default
declaration retention at 59 production, 178 proof and 237 total gross Rust
additions. Loading passes 537 unit tests with one ignored and integration
targets 51/29/8/6/2/1/5/1; query passes 55/55. The direct pinned-nightly CLI
rebuild, formatting, diff and daemon-hygiene gates pass; the archive checker
reports only its three longstanding thought paths.

From `/tmp/slug-rules-rust-replay-D4VrVd`, the authenticated replay was:

```text
env PATH=/usr/bin:/usr/local/bin /home/wgray/slug/target/debug/slug cquery \
  //pkg:probe --@rules_rust//rust/toolchain/channel=nightly \
  --lockfile_mode=off
```

It clears every selected computed-default declaration and stops during Bzl
evaluation at toolchain-registration row 14, before callback execution or
target invocation:

```text
error: Variable `exec_group` not found
@@rules_cc+//cc/private/rules_impl:cc_binary.bzl:905:21
    "cpp_link": exec_group(toolchains = use_cc_toolchain()),
```

The renderer's line 905 corresponds to the byte-authenticated release
expression at line 848.

## Exact selected rules_cc closure

The durable BCR descriptor
`https://bcr.bazel.build/modules/rules_cc/0.2.4/source.json`, SHA-256
`2bd87ef9b41d4753eadf65175745737135cba0e70b479bdc204ef0c67404d0c4`,
selects
`https://github.com/bazelbuild/rules_cc/releases/download/0.2.4/rules_cc-0.2.4.tar.gz`,
a 276,390-byte, 400-entry archive with SHA-256
`8dcd63392f0bb48adf74f413a9f39ba0fedcb8f99bf085a3b450f06d171dbb6d`
and integrity `sha256-jc1jOS8LtIrfdPQTqfOboP7cuPmb8IWjtFDwbRcdu20=`.
An exact full-archive scan finds four constructor calls in exactly three rule
declarations and one configured consumer:

| Source-relative path | SHA-256 | Bytes/lines; mode | Complete selected role |
|---|---|---:|---|
| `cc/find_cc_toolchain.bzl` | `5784eeb7ce1e597380b4393bd28d0822f51c23a5d7d7313c65732a2cf38ad979` | 4,808/131; 0664 | `use_cc_toolchain()` returns existing `config_common.toolchain_type` declarations |
| `cc/private/rules_impl/cc_binary.bzl` | `d9d0f68e028ee64ef9beb73a2b51f308be5b60545b79ce27daa532b430fbc69f` | 41,488/854; 0664 | `cpp_link` at 847-849 |
| `cc/private/rules_impl/cc_test.bzl` | `6787e5a152ce2e0ec7744a885086ad9977a0ede1da4bb3abd7f69331947ee28f` | 6,206/165; 0664 | configured `ctx.exec_groups["test"]` at 59; `cpp_link` and optional `test` declarations at 155-159 |
| `cc/private/rules_impl/cc_library.bzl` | `79af1daa5d12f07b3dd6a489e781bfa2c973b520e883b9ab8c024ee6d0c1925b` | 38,773/962; 0775 | `cpp_link` at 959-961 |
| `cc/common/semantics.bzl` | `6eb89858e52eb3c50dcd1575f734585083752dd4121dcf09f709ed395dee0f4a` | 7,003/216; 0664 | two `config.exec(exec_group = "test")` attribute declarations at 75/80; `extra_exec_groups = {}` at 202 |

Every file has a trailing LF. There are no other `exec_group(...)`, nonempty
`exec_groups`, `ctx.exec_groups`, or named `config.exec` expressions in the
selected 400-entry archive. The three rule dictionaries preserve source order;
all four constructors use only the admitted toolchain-list shape and omit
`exec_compatible_with`.

## Bazel 9.2 authority

Pinned Bazel commit `8220c6198837d5c13d53fea211cf3282aa12408a`
establishes that declaration is coupled to configured semantics:

- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/StarlarkRuleFunctionsApi.java`
  (SHA-256 `be73dbda0b5a3e8285a05bb732a0a01441f99e8d20dc29b83759ef972c0392ea`,
  lines 1057-1084) exposes `exec_group(toolchains=[],
  exec_compatible_with=[])`, and lines 684-699 expose `rule(exec_groups=...)`;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkRuleClassFunctions.java`
  (SHA-256 `a1f706cfbbc67aa3cd2521df2091dd5ed9af96eb4568049f8eee966d06c622f7`,
  lines 1058-1080 and 2144-2159) validates names, resolves labels and retains
  `DeclaredExecGroup` values;
- `src/main/java/com/google/devtools/build/lib/packages/DeclaredExecGroup.java`
  (SHA-256 `791a3141fbfe7675dc82c490a78fc753fce94cdab9fb3368bd2c89339615efce`,
  lines 35-133) owns toolchain requirements, execution constraints, default
  copying and automatic-group processing;
- `src/main/java/com/google/devtools/build/lib/starlarkbuildapi/config/StarlarkConfigApi.java`
  and `analysis/starlark/StarlarkConfig.java` (SHA-256
  `2679bc99e2cc35dc72ed38aee1934a64b3cf2f6715b11b32a1f2b6e67db63f25`
  and `6af787d37fdc7499ce3766751a632aa045a9f8fe38c3b38baee91f1737bfbd65`,
  lines 174-190 and 68-72) retain the named execution transition;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkExecGroupCollection.java`
  (SHA-256 `4ec1835fad1899341acfd3747318a95c6133b0321f8ff5d1be427051437cc5f5`,
  lines 54-159) exposes per-group toolchains and typed missing-name behavior;
- `src/main/java/com/google/devtools/build/lib/analysis/starlark/StarlarkActionFactory.java`
  (SHA-256 `bee52fa85442fe668c8573bbd2218dd454485ac8d4451ecf3553201fba6169a2`,
  lines 861-895) chooses and validates each action's execution group; and
- `src/test/java/com/google/devtools/build/lib/analysis/StarlarkExecGroupTest.java`
  and `src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleContextTest.java`
  (SHA-256 `5f99076670edcd8570d124aeb03430a7b2f8f41f48b41b31e6d19a90b6391fe2`
  and `d195e5d49aae52a92bd3abebfc8de7942aacb252b522cea315985d41277f082d`)
  discriminate named dependency transitions, action platforms, validation and
  `ctx.exec_groups` toolchain projection.

## Audit verdict and compatibility classification

Audit result: `ACCEPT` for one generic **selected-shape execution-group
declaration closure with fail-closed invocation** design. The
closure includes both the group declarations and their named attribute
execution-transition references; accepting only the constructor would merely
move the replay to `config.exec(exec_group = "test")`. Independent architecture
and retained-state review returns `ACCEPT`; implementation is active without
scope change.

Classify as **exact**: expose `exec_group` only in ordinary and Bzlmod `.bzl`
globals; accept omitted/list `toolchains` entries already admitted by
`toolchain_requirements`; accept only omitted/empty `exec_compatible_with`;
accept `rule(exec_groups = None|{})` and nonempty string-keyed dictionaries of
those values; validate legal non-default identifiers; admit
`config.exec(exec_group = None|<string>)` as an immutable transition descriptor
and retain the selected `test` references; preserve dictionary/toolchain order;
freeze/import/re-export without execution; and preserve rules without named
groups/transitions unchanged. These shapes close all four selected
constructors, both selected named transitions and all three selected rule
declarations without a rules_cc, C++ or name special case. Direct BUILD
omission is exact for this exposure boundary. `rule()` is the sole admitted
descriptor consumer: aspect attributes, subrule attributes, symbolic-macro
explicit attributes and macro `inherit_attrs` from a rule must detect and
reject a named marker before their existing projections can discard its name
or reduce it to default exec. Repository-rule and tag-class descriptors retain
their existing rejection.

Classify as **Slug-native**: the compact detached representation below and the
stable diagnostic

`target invocation for named execution-group semantics is unsupported`

for every valid invocation of a rule bearing a declared group or named
transition. Reject before initializer/computed-default checks, recorder access,
unknown-attribute validation, coercion, output or target publication so no
group semantics can be silently lost. Declaration validation still precedes
this invocation boundary.

Keep **unsupported/deferred**: nonempty `exec_compatible_with`; duplicate
toolchain normalization beyond the already-admitted parser; parent/aspect/
subrule groups; automatic groups and `_use_auto_exec_groups`; configured
application of named `config.exec`; `exec_group_compatible_with`;
per-group platform/toolchain resolution; `ctx.exec_groups`; named action
selection and exec properties; test-runner defaults; all configured C++
behavior. Non-rule descriptor consumption remains explicit error behavior,
never empty/default substitution or marker loss.

## Ownership, representation and incremental safety

`app/slug_loading_v2/src/package.rs` is the sole production owner. Add one
detached immutable `DeclaredExecGroup` containing only
`Arc<[ToolchainTypeRequirement]>`, wrapped by a Starlark simple value, plus one
immutable `config.exec` descriptor carrying `Option<CompactString>`. Empty
constraints are validated and discarded; no permanent field represents a
category this packet does not admit. The
attribute descriptor retains only a sparse optional named group; the existing
boolean continues to represent default exec. At `rule()` construction, project
only actual named references into an immutable schema-indexed
`Arc<[(u32, CompactString)]>` and groups into source-ordered
`Arc<[(CompactString, DeclaredExecGroup)]>`. Do not add a group string to every
final attribute schema or any map. Freeze/import/re-export clones only existing
Arc/compact immutable data. `Allocative`, structural `Clone`/`Eq`, exact size
assertions and no raw pointer complete the retained-state contract.

Live anchors are `ToolchainTypeRequirement`/`toolchain_requirements` at
`package.rs:762-791`/`2784-2822`, `RuleDefinitionGen` and
`FrozenRuleDefinition` at 3697/3731, freeze at 4026, the pre-recorder invocation
boundary at 7295-7320, `ConfigModule`/attribute cfg binding at 7094-7257/
6400-6425, Bzl-only globals at 9386, and `rule()` at 8725. Existing source
digest, frozen-module closure and recursive load-manifest fingerprint own
add/remove/change invalidation. Add no DICE key, request input, observation,
lock, await, retry, cache, interner, registry, second map or fixture.

Stage 6 already has `ConfiguredExecGroup::{Default, Named}`, action-owner group
identity and context matching (`exec_group.rs`, `result.rs:434-543,985-1020`),
but `dice.rs:3679-3691` prepares only the default resolved context;
`starlark_rule.rs:1380-1390` rejects named action selection; and analysis ctx
does not expose `ctx.exec_groups`. The invocation guard keeps every new token
out of `StarlarkRuleImplementation`, package equality, configured keys, DICE,
toolchain resolution and actions. Stage 6 therefore owns proof of the existing
boundary, not a code change.

Deletion condition: a separately reviewed cross-stage packet may remove the
guard only after it owns application of named `config.exec`, per-group
platform/toolchain resolution, configured dependency identity,
`ctx.exec_groups`, action routing, exec properties, equality/invalidation and
all failure ordering. The existing
prototype `toolchains/exec_groups.rs` is not an accepted semantic owner and
must not be wired in without that review.

## Required proof

Adjacent tests must prove:

- `exec_group` and `config.exec` exist in ordinary/Bzlmod `.bzl` globals and
  remain absent from direct BUILD; `config.exec()` preserves default-exec
  behavior and a string group freezes/imports/re-exports with exact identity;
- for `exec_group`, omitted and explicit admitted toolchain lists preserve
  label, mandatory bit and order; nonempty constraints and invalid entries
  fail;
- `rule(exec_groups=None|{})` is unchanged; three selected dictionary shapes
  retain `cpp_link`, `test`, optional toolchains and union with an empty dict;
- non-dict, nonstring keys, non-`exec_group` values, invalid/default names and
  unadmitted fields fail at declaration, without fallback;
- aspect attrs, subrule attrs, symbolic-macro explicit attrs and macro
  `inherit_attrs` from a rule reject a named `config.exec` marker before
  projection; repository-rule and tag-class attrs reject it as regression
  controls; none silently becomes target/default exec or loses the name;
- transient/frozen groups and sparse named-transition rows retain structural
  equality, schema index (including builtin-count adjustment) and source order
  through ordinary/Bzlmod freeze/import/re-export; Arc clone is constant-time;
- exact sizes and `Allocative` accounting are reported for transition
  descriptor, group and both sparse slices; no map or per-rule allocation
  occurs for rules with neither groups nor named transitions;
- target invocation for a rule bearing a group or named transition returns the
  exact diagnostic before every recorder/coercion/publication effect, including
  a rule also bearing an initializer or computed default; a clean evaluation
  after failure has no leaked state;
- source A/B/A and group add/remove/change restore the existing recursive
  manifest marker without new DICE state; and
- current Stage 6 named action rejection/default action context and ordinary
  no-group target analysis remain unchanged.

The authenticated replay must clear the three selected declarations and stop
at the next independent typed boundary before any group-bearing target or
configured group behavior executes.

## Implementation allowlist, caps, validation and stops

Implementation may change only:

- `app/slug_loading_v2/src/package.rs`, sole production owner plus adjacent
  unit proof; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`, proof only for exact
  selected-source ordinary/Bzlmod freeze/import/re-export and invalidation.

Caps: 170 production Rust, 220 proof Rust and 390 aggregate gross additions;
within them, `package.rs` is capped at 170 production plus 130 proof and the
host test at 90 proof. No docs, fixture, asset, Cargo manifest or Stage 6 Rust
file may change during implementation.

Run serial focused global/constructor/rule/freeze/import/rejection/invalidation
tests, then full loading library/integrations, query library, CLI rebuild and
the exact bounded-PATH replay; finish with stale-`slugd`, formatting, diff,
archive, allowlist and cap gates.

Return `REPLAN` if any selected declaration requires nonempty constraints,
target invocation or configured behavior; any non-rule descriptor consumer
accepts, downgrades or discards a named marker; a group/transition token
enters final package/analysis/action/DICE state; the invocation guard is not
strictly before package mutation; another production owner or second map is
needed; a callable/dynamic object is retained; a new key/cache/lock/fixture is
proposed; or the allowlist/caps fail.
