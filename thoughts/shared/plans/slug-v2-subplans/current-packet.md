# Current Slug V2 Packet

Packet: `WP-4-5-7A-generic-fragment-projection-r4`

Milestone: M7A bootstrap-critical generic Starlark/ruleset closure.

Status: R1-R3 architecture reviews `REPLAN`; R4 architecture review `ACCEPT`;
implementation/proof complete; terminal implementation review `ACCEPT`.

Base: `da6865a3b`, which terminally accepts generic direct subrule invocation
and value materialization. The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Observable result

Supply cached `ctx.fragments` collections to ordinary rule implementations and
direct/nested subrule implementations from the already-selected structural
`SlugConfiguration`. Implement the complete first authentic C++ fragment
consumer category used by rules_cc 0.2.17 `create_fdo_context`: one shared
typed `cpp` fragment value with
`compilation_mode`, `propeller_optimize_absolute_cc_profile`,
`propeller_optimize_absolute_ld_profile`, `fdo_path`, `cs_fdo_path`, and
`proto_profile` methods.

This is generic configured-analysis plumbing, not a `cc_common`, `cc_internal`,
rules_cc, parser, or native C++ rule special case. The authentic BCR subrule is
the discriminator. A target-configured default rule returns under `fastbuild`;
the selected `cc_toolchain` implementation is Exec-configured, where Bazel
copies default `host_compilation_mode=opt` into `compilation_mode` and reaches
the deferred action families. Typed long-form target and host compilation-mode
producers plus that bounded Target-to-Exec projection keep those routes
distinct and leave the next missing capability deterministic.

## Learned facts and authority

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the
sole semantic authority:

- `FragmentCollection.java:24-54` creates one immutable rule collection,
  exposes every configured active fragment name through `dir`, and delegates
  access checks to `RuleContext.getStarlarkFragment`;
- `RuleContext.java:512-533` returns no field for an unknown fragment, rejects
  a known fragment not declared by the rule/aspect, and returns the configured
  fragment only after that declaration check;
- `StarlarkSubrule.java:562-592` owns a separate token-scoped subrule
  collection: unknown names are absent, known-but-undeclared names fail, and
  `dir` is exactly the subrule's own declared set;
- `StarlarkSubruleTest.java:1450-1630` distinguishes unknown fragments,
  rule/subrule authorization isolation, declared access, and subrule `dir`;
- `StarlarkRuleContext.java` constructs and retains one root fragment
  collection rather than allocating it on each field access;
- `CppConfiguration.java:166-306,326-330,617-689,958-968` derives the six admitted
  method results from `CoreOptions`/`CppOptions`, validates FDO path forms, and
  applies the private Starlarkification allowlist; and
- `builtin_exec_platforms.bzl:221-280` propagates `host_compilation_mode` and
  replaces `compilation_mode` with it while forming the Exec configuration; and
- `BuiltinRestriction.java:35-225` defines the default restricted-API caller
  allowlist from the innermost executing `.bzl` definition and repository
  mapping.

The authenticated BCR source is the rules_cc 0.2.17 file already realized at
`target/v2o/ob/registry-yanked-lockfile-mode/bazel/external/rules_cc+/cc/private/rules_impl/fdo/fdo_context.bzl`,
SHA-256
`91b7b46c515b4773d5a241e699027212f679ab93160cc79218bd687eac51d5b7`.
Its `_create_fdo_context` reads `ctx.fragments.cpp`, exits unless
`compilation_mode() == "opt"`, then calls the other five methods before reaching
`ctx.actions.args`, `run`, `symlink`, or `cc_common.absolute_symlink`.

Buck2/starlark-rust remains the unchanged parser, binder, evaluator, heap and
method-dispatch implementation. Its `StarlarkValue::{get_attr,has_attr,dir_attr}`
contract means field presence and `dir` must be implemented together; `dir_attr`
cannot report an evaluation error. Do not add a parser, builtin registry,
custom `set`, or alternate call binder.

Zabel is peer concept and optimization guidance only. Its evaluator-local
typed `CppFragmentOptions`, separate active/declared fragment sets, one cached
collection, and rule/subrule authorization split support the ownership choice.
Copy no Zig code, names, layout, errors, tests, or behavior. In particular,
Bazel 9.2 decides values, caller restrictions and error precedence.

## Compatibility boundary

**Exact:** successful `cpp` access only after the applicable rule or subrule
declaration; unknown and undeclared access both reject; separate rule and
subrule declaration authorization; root `dir(ctx.fragments)` over active
configured fragment names; subrule `dir(ctx.fragments)` over exactly its
declarations; one cached collection per context; one evaluator-local `cpp`
value shared by authorized contexts; all six named zero-argument methods; default
target-default `fastbuild` and Exec-default `opt` plus
`None`/`None`/`None`/`None`/`True` results; typed `fastbuild`/`dbg`/`opt` target
and host compilation-mode projection; copying host mode into compilation mode
at the admitted Target-to-Exec transition; context and nested-call lifetime
behavior; the complete Bazel 9.2 default private Starlark API allowlist for
every admitted method; and structural configuration equality/invalidation for
both new command producers and their Exec result.

**Slug-native:** Rust enum/optional/string representation; root diagnostic
wrapping after the exact declaration fact; evaluator bridge type names; and the
already-admitted collision-safe structural configuration identity.

**Unsupported/deferred:** short `-c`; every Exec-transition rewrite except the
already-owned target-platform/Starlark projection and this packet's exact
host-to-compilation-mode copy; absolute-path FDO/CS-FDO/Propeller command
producers and Bazel `PathFragment` normalization; non-default `proto_profile`;
other C++ fragment methods; every non-`cpp` configured fragment value; aspects;
toolchain lookup; `ctx.actions.args`, `run`, `symlink`, `declare_symlink`, tree
and template actions; `cc_common.absolute_symlink`; exact Bazel configuration
checksum/output paths; and complete rules_cc analysis. Existing accepted label-
valued `--fdo_optimize=//...` remains structurally valid but projects `fdo_path`
as `None`, matching Bazel's distinction between label-backed FDO state and the
absolute path getter. An absolute-path input continues to fail closed rather
than being normalized from an unowned host path model.

The exact claim deliberately excludes the wording/error-class distinction
between an unknown subrule fragment and a known-but-undeclared subrule fragment,
plus `hasattr(ctx.fragments, name)` for fragment collections. starlark-rust's
dynamic `StarlarkValue::get_attr` returns `Option<Value>` and cannot raise the
Bazel-specific undeclared error. Both accesses still reject and `dir` remains
exact; the diagnostic/`hasattr` distinction is unsupported rather than silently
claimed. A known but unimplemented fragment value is likewise outside the
admitted value surface even though its name remains in the exact `dir` inventory.

No claim is made that the six-method surface completes `CppConfiguration`.
This packet completes the one coherent FDO fragment-method category consumed
before the authentic subrule's first action call. Later fragment categories
extend the same projection/collection architecture without changing context
ownership.

## Architecture

### Structural producer and typed projection

Add `CompilationMode` and `HostCompilationMode` to the closed
`NativeCommandOption` table and classify both as `CoreOptions`. Accept only long
forms and only the three Bazel 9 enum spellings. Extend
`SlugConfiguration::to_exec_for_platform` to replace `compilation_mode` with
the structural `host_compilation_mode` value in the same options-vector pass
that installs the selected execution platform. Do not rewrite or silently
claim any other Bazel Exec-transition field. The existing command parser,
scoped configuration construction and DICE request key perform all mutation/
invalidation; add no environment reader, fallback, graph key, cache, lock,
registry or process-global state.

Create a small `slug_configuration_v2::native::cpp_fragment` leaf rather than
growing the already-large `configuration.rs`. `CppFragmentProjection` holds a
cheap clone of `SlugConfiguration`, reads the sole canonical option vector on
demand, and exposes typed Rust results for the six admitted methods. It is
phase scratch, not a parallel retained C++ options store. Expose only the
minimal private option/enum accessors required by this sibling module.

Every optional absolute-path method must distinguish the existing label-valued
FDO option from a future absolute-path producer. Do not reinterpret a label,
display string, workspace path, or host path as Bazel's normalized
`PathFragment`. Unsupported stored shapes fail closed while constructing the
projection, before Starlark execution.

### Generic fragment values

Add an `analysis_fragments.rs` evaluator-ABI leaf in `slug_loading_v2` with two
concrete collection facades matching Bazel's separate ownership:

- `RuleFragmentCollection` stores the root `AnalysisCallToken`, immutable rule
  declarations, and shared frozen `cpp` value. A type-static fallible `cpp`
  attribute performs the live-token and rule-declaration check; starlark-rust
  merges it into `dir` while `dir_attr` supplies the other 11 active names;
- `SubruleFragmentCollection` stores the call token, the subrule's immutable
  declaration set, and shared frozen `cpp` value. Its dynamic `get_attr` returns
  `cpp` only when declared and otherwise returns `None`; its `dir_attr` is the
  declaration set. Future admitted fragment values extend this one dynamic
  match and do not require concrete types for declaration-set combinations;
- `CppFragmentValue` stores only `CppFragmentProjection` plus the immutable
  caller-authorization inputs needed by its methods;
- root collections use one phase-scratch `SmallSet<CompactString>` created from
  the rule's retained fragment slice, while subrule collections clone the
  already-retained `Arc<SmallSet<CompactString>>`; and
- admitted field lookup checks the applicable live token and declaration
  authorization. It never checks only membership in the active configured
  fragment set.

Freeze the pinned Bazel 9.2 active Starlark fragment-name inventory separately
from implemented values: `android`, `apple`, `bazel_android`, `bazel_py`,
`coverage`, `cpp`, `j2objc`, `java`, `objc`, `platform`, `proto`, and `py`.
Root `dir_attr` plus the static `cpp` attribute returns those active names even
when the rule declared none; attempting `ctx.fragments.cpp` still enforces the
rule declaration with a fallible native attribute. Only `cpp` has an admitted
value in this packet. A declared access to another known name remains outside
the admitted fragment-value surface; an unknown field remains absent through
starlark-rust's ordinary attribute error. Subrule `dir_attr` returns its
declaration set exactly, including unknown or unimplemented names, as Bazel 9.2
does. Do not allocate error-placeholder values or claim that mere field access
to an unimplemented fragment succeeds.

The collection is allocated once into each root/subrule context. The frozen
`cpp` object is allocated once per analysis evaluator and shared by those
collections. No evaluator `Value`, heap, token or collection is retained in a
DICE key or configured result.

### Caller restriction and context ownership

Retain the flat filename-to-`BzlModuleIdentity` manifest already used by loading
macros on `StarlarkRuleImplementation`; include it in semantic equality because
it controls private API authorization. Analysis resolves the innermost
`Evaluator::native_caller_function_filename()` through that immutable manifest
and applies a V2-owned generic translation of Bazel's default
`BuiltinRestriction`. A direct native/module-scope call without an innermost
`.bzl` definition fails closed. A helper defined in a loaded module is checked
against that helper's identity, not the owning rule/subrule identity.

Freeze this exact Bazel 9.2 allowlist inventory in one typed constant table:

- the 18 main-repository package prefixes:
  `third_party/bazel_rules/rules_cc`, `tools/build_defs/cc`,
  `third_party/bazel_rules/rules_java/java`, `third_party/protobuf`,
  `third_party/bazel_rules/rules_rust/rust/private`, `third_party/crubit`,
  `tools/build_defs/go`, `tools/build_defs/build_info`,
  `bazel_internal/test_rules/cc`, `tools/build_defs/android`,
  `third_party/bazel_rules/rules_android`, `third_party/apple_crosstool`,
  `third_party/bazel_rules/rules_apple`,
  `third_party/cpptoolchains/portable_llvm/build_defs`,
  `third_party/gpus/cuda`, `tools/build_defs/packaging`, `test`, and
  `bazel_internal/test_rules`;
- the 11 external module/prefix pairs: `rules_cc/`, `rules_java/java`,
  `protobuf/`, `com_google_protobuf/`, `rules_rust/rust/private`,
  `bazel_tools/tools/build_defs/build_info`, `rules_android/`,
  `build_bazel_rules_android/`, `rules_apple/`,
  `build_bazel_rules_apple/`, and `rules_shell/`; and
- the separate unconditional canonical `_builtins` repository branch.

Implement Bazel's repository matching classes explicitly: exact main
repository, an apparent external name mapped to main by the executing module's
repository mapping, exact canonical `bazel_tools`, and version-insensitive
canonical module repositories beginning `<module>+`. A bare name, filesystem
path substring, or unrelated prefix such as `rules_cc_evil+` never authorizes.

This restriction belongs to a reusable loading/analysis helper, not
`CppFragmentValue` string heuristics. Table-driven proof must exercise every
inventory row plus every repository matching class, exact-prefix negatives,
helper-defined caller selection, and repository-mapping A/B/A identity/
authorization restoration. The authentic rules_cc 0.2.17 caller must pass
through its retained canonical identity, never a filesystem substring test.

Extend `PreparedSubruleInvocation` with its existing declared fragment set.
Extend `AnalysisEvaluationContext` with the shared fragment value and the
caller-identity manifest. Root construction receives the rule declarations and
definition provenance. Subrule dispatch allocates one restricted generic
context with its own cached collection. Preserve the accepted call stack,
action owner, direct/nested authorization, context locking and RAII invalidation
unchanged.

### Natural successor boundary

A target-configured default proof must return from the authentic-shaped FDO
function under `fastbuild`. The authentic selected Exec-configured rules_cc
route must project default host `opt`, pass all six method calls, and stop at the
first deferred action family. `--compilation_mode=dbg` changes only the target
route; `--host_compilation_mode=fastbuild` changes only the Exec route and makes
it take the early return. That terminal selects the generic
`Args`/`run`/`symlink` action packet; it does not justify a rules_cc branch.

## Frozen scope

Production allowlist:

- `app/slug_configuration_v2/src/command.rs`
- `app/slug_configuration_v2/src/native/configuration.rs`
- `app/slug_configuration_v2/src/native/cpp_fragment.rs` (new)
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/value.rs`
- `app/slug_configuration_v2/src/lib.rs`
- `app/slug_loading_v2/src/analysis_fragments.rs` (new)
- `app/slug_loading_v2/src/builtin_restriction.rs` (new)
- `app/slug_loading_v2/src/lib.rs`
- `app/slug_loading_v2/src/package.rs`
- `app/slug_loading_v2/src/subrule_invocation.rs`
- `app/slug_analysis_v2/src/starlark_rule.rs`

Proof allowlist:

- `app/slug_configuration_v2/src/native/tests.rs`
- `app/slug_commands_v2/tests/commands.rs`
- `app/slug_loading_v2/src/builtin_restriction_tests.rs` (new)
- `app/slug_loading_v2/src/host_package_load_tests.rs`
- `app/slug_analysis_v2/tests/subrule.rs`
- `app/slug_server_v2/src/tests.rs`

Plan/evidence allowlist:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`
- `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md`

Production additions cap: 950 lines. Proof additions cap: 850 lines. Plans and
generated lockfiles are excluded. Any other path, a new dependency, a retained
representation wider than two pointer words, a second configuration store, a
new DICE key/cache/lock/interner, or an inability to preserve exact private
caller provenance is `REPLAN`.

## Required proof

1. Configuration unit tests prove target/host defaults, all three modes and
   invalid spellings, target/Exec separation, host-to-compilation copying,
   re-projection, all six method defaults, label-valued FDO distinction, and
   fail-closed unowned absolute paths.
2. Restricted-API table tests prove every frozen allowlist row, `_builtins`,
   exact `bazel_tools`, main prefixes, mapping-to-main, version-insensitive
   module repos, exact-prefix negatives, helper-definition selection, and
   mapping A/B/A without filesystem substring authorization.
3. Root analysis proves cached collection identity, active-name `dir`, unknown
   absence, the exact 12-name inventory, declaration rejection, explicit
   unsupported known-fragment failure, declared `cpp`, every zero-argument
   method, and wrong arity.
4. Subrule analysis proves its exact `dir`, rule/subrule declaration isolation,
   admitted access success/failure, the explicitly unsupported specialized
   error/`hasattr` distinction, nested declarations, distinct collection
   identity, shared `cpp` identity, escaped-context rejection, and extracted
   fragment behavior.
5. An authenticated-source regression checks the realized rules_cc file hash
   and the source-ordered six-method call ledger.
6. An authentic-shaped `create_fdo_context` proof distinguishes target-default
   early return, selected-Exec default action terminal,
   `--compilation_mode=dbg` target-only behavior, and
   `--host_compilation_mode=fastbuild` Exec-only early return.
7. Stable-daemon target/host C0/C1/C0 proves command and Exec-projection
   identity, invalidation and restoration with no cross-request fragment reuse.
8. `cargo test -p slug_configuration_v2`,
   `cargo test -p slug_loading_v2`,
   `cargo test -p slug_analysis_v2`, and the named server/CLI dependents pass
   serially. Rebuild `slug_cli_v2` before binary smokes and clean stale `slugd`
   before and after daemon-sensitive validation.
9. A staged-only diff audit proves both line caps, allowlists and the exclusion
   of the parked registration proof. Independent terminal review must return
   `ACCEPT` before commit.

## Implementation evidence

The candidate uses 665 production additions and 648 proof additions, excluding
plans and the parked registration proof. It adds no dependency, DICE key,
cache, lock, interner, parser, rules_cc branch, or second configuration store.
The `cpp` projection is constructed only when the root rule or an attached
subrule declares `cpp`; unused invalid C++ state therefore preserves the
previous ordinary-rule/config-setting behavior.

Serial validation passed `slug_configuration_v2` (49 tests),
`slug_loading_v2` (456 unit tests plus all integration suites, with the
realized-source proof run explicitly under `--ignored`), `slug_analysis_v2`
(100 tests), and `slug_commands_v2` (25 tests). The authentic-shaped fragment
proof, prior FDO daemon A/B/A proof, and new target/Exec fragment C0/C1/C0 proof
pass individually. A serial full `slug_server_v2` run passed 55/56 and exposed
the eager unused-projection regression; after the lazy-construction correction,
both the previously failing FDO test and new fragment daemon test pass. CLI
library tests and command-boundary tests pass. The broad CLI integration suite
remains independently red in existing selected-BCR unsupported-entry and
unavailable-DICE fixture paths; no fragment/command diagnostic appears in
those failures, so this packet does not claim to repair that baseline.

Independent terminal review returned `ACCEPT`: the staged candidate preserves
one structural configuration owner, lazy evaluator-local projection, exact
Target/Exec mode copying, separate root/subrule authorization, authenticated
caller provenance, the pinned allowlist, and fail-closed absolute-path state.
The reviewed accounting is 665 production and 648 proof additions, with only
the unrelated registration proof left unstaged.

## Review gate

The architecture reviewer must answer:

- Does the packet preserve one structural configuration owner and one
  evaluator-local projection without adding a parallel retained representation?
- Are root/subrule authorization, `dir`, caching and lifetime facts separated
  exactly as Bazel 9.2 requires?
- Does the two-facade design fit starlark-rust without declaration-set type
  explosion, and is the narrowed subrule diagnostic/`hasattr` boundary honest?
- Does the complete frozen private-caller inventory and repository-branch proof
  make the exact admitted-method claim defensible?
- Does the bounded host-to-compilation Exec projection preserve target/Exec
  separation without silently claiming the rest of Bazel's transition?
- Are all six FDO-facing methods handled as one coherent category while
  absolute-path producers remain an explicit, honest unsupported boundary?
- Can later fragments and action builtins extend these seams without a parser,
  rules_cc, `cc_common`, or C++-specific configured-analysis branch?

Only `ACCEPT` activates implementation. Any ambiguity in caller provenance,
path normalization, semantic equality, or evaluator-value retention is
`REPLAN`.
