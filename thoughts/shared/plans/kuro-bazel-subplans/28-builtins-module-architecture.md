# Plan 28: Bazel Builtins Module Architecture

> **Main Plan**:
> [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Related**:
> - [Plan 04: Prelude Architecture](./04-prelude-architecture.md)
> - [Plan 05: Builtins Compatibility](./05-builtins-compatibility.md)
> - [Plan 15: Bazel 9 Parity](./15-bazel-9-parity.md)
> - [Plan 27: Native Language Rule Removal](./27-native-language-rule-removal.md)

## Scope

Add a first-class, Bazel-9-compatible builtins module layer for kuro.
The layer lets selected builtins be authored in bundled Starlark and
exported into:

- BUILD-file globals;
- the BUILD-file `native` struct;
- `.bzl` top-level globals where Bazel 9 exposes them;
- Starlark rule implementation wrappers.

This is inspired by Bonanza's `builtins_core/exports.bzl` and
`wrappers.bzl`, but kuro must not copy Bonanza's semantics wholesale.
The public behavior must be sourced from Bazel 9 and upstream
`rules_*`.

## Why This Exists

Kuro currently has two mechanisms:

1. **Rust base globals** registered through
   `app/kuro_interpreter_for_build/src/interpreter/globals.rs`.
2. **Prelude BUILD-global injection** through `prelude/native.bzl` and
   `__kuro_builtins__`.

Plan 04 correctly observed that external `.bzl` files do not receive
prelude injection; they only see base globals. That forced many
Bazel-compatibility shims to stay in Rust.

Plan 28 adds the missing middle layer: bundled Starlark builtins that
are loaded before user `.bzl` evaluation and then merged into the
environments that need them. Once that exists, some compatibility logic
can move from Rust into Starlark without losing visibility in external
rulesets.

## Bonanza Reference Points

Useful Bonanza patterns:

- `starlark/builtins_core/exports.bzl`: exports Starlark-defined
  providers, rules, and native-like globals.
- `pkg/model/analysis/compiled_bzl_file.go`: merges exported rules into
  `native` and BUILD globals.
- `starlark/builtins_core/wrappers.bzl`: wraps rule implementations so
  `ctx` compatibility can live in Starlark.

Do **not** adopt:

- Bonanza-only `PlatformInfo` fields such as `exec_pkix_public_key` or
  `repository_os_*`.
- Bonanza's custom remote-evaluation scheduler or object model.
- Bonanza's Bazel 8-oriented builtins content.

## Desired End State

- Kuro has a bundled builtins Starlark package, loaded deterministically
  before user BUILD and `.bzl` files.
- The builtins package exposes explicit export dictionaries, not
  implicit "everything in this file" globals.
- BUILD files receive the same native/global symbol set as Bazel 9.
- External `.bzl` files can see Starlark-defined builtins when Bazel 9
  would expose them at top level.
- Rule implementations can be invoked through a Starlark wrapper layer.
- The remaining Buck2 prelude machinery is either integrated into the
  builtins module, moved to a Kuro-specific extension surface such as
  BXL, or removed. Evaluating `prelude/prelude.bzl` must not be the
  source of BUILD globals.
- Rust retains action-creating primitives and modules that are still
  better implemented natively.

## What We're NOT Doing

1. **No full Bazel builtins zip clone in the first phase.** Start with
   the loader/export mechanism and a small set of low-risk exports.
2. **No provider-global rollback.** Plan 15 removes provider globals
   that Bazel 9 removed. The builtins module must not re-export
   `CcInfo`, `PyInfo`, `ProtoInfo`, etc. as top-level globals unless
   the Bazel 9 source says they are top-level.
3. **No Starlark `cc_common` action engine.** `cc_common.compile()`,
   `cc_common.link()`, artifact declaration, and action registration
   remain Rust-backed.
4. **No language rules in kuro builtins.** `cc_library`, `py_library`,
   `proto_library`, etc. come from external `rules_*` repos, not from
   kuro's builtins package.
5. **No Buck2 prelude compatibility promise.** `@prelude//...` is an
   implementation detail unless a Kuro-specific command such as BXL
   still owns a narrow part of it. User BUILD files should not depend on
   Buck2 prelude APIs.

## Export Contract

The bundled builtins package should have one stable entry point, for
example:

```starlark
# prelude/bazel_builtins/exports.bzl

exported_toplevels = {
    # Symbols visible in .bzl files if Bazel 9 exposes them there.
}

exported_build_globals = {
    # Symbols visible directly in BUILD files.
}

exported_native_members = {
    # Symbols visible as native.<name> in BUILD/.bzl contexts where
    # Bazel exposes native.<name>.
}

rule_implementation_wrapper = _invoke_rule
aspect_implementation_wrapper = _invoke_aspect
subrule_implementation_wrapper = _invoke_subrule
```

Rules for the contract:

- Export dictionaries must be plain string-keyed dicts.
- Every exported symbol needs a parity citation comment.
- Exported names must be audited against Bazel 9 before they are made
  visible in external `.bzl` files.
- User `load()` bindings must be able to shadow BUILD globals the same
  way they do in Bazel.

## Phase 28.1: Feasibility Spike and Environment Matrix  [DONE 2026-04-30]

### Status

Spike landed. Findings + insertion-point analysis + Phase 28.2 design
recorded in
[`thoughts/shared/research/2026-04-30-plan-28-1-builtins-loader-spike.md`](../../research/2026-04-30-plan-28-1-builtins-loader-spike.md).

Key empirical result: external `.bzl` files in Bazel-mode workspaces
(no prelude registered, e.g. `tests/core/analysis/test_native_rules_data/`)
do not see prelude-injected symbols. `prelude_import()` returns `None`
when no prelude is configured, so `import_public_symbols` is never
invoked. The only existing Starlark-symbol injection is
`rules_cc_autoload`, which fires only for `BuildFile` paths.

Insertion point identified:
`app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`
— extend the `rules_cc_autoload` pattern with a new
`bazel_builtins_autoload` field that fires unconditionally for both
BUILD and `.bzl` paths. Representation: a frozen Starlark module
loaded via the normal load resolver, with public symbols copied via
`import_public_symbols` (no new starlark-rust hooks).

### Goal

Prove that Starlark-defined builtins can be made visible in every
required context without relying on the prelude-only injection path.

### Work

1. Document the current environment matrix:

   | Context | Current inputs | Plan 28 requirement |
   |---------|----------------|---------------------|
   | Root BUILD | Rust globals + prelude native extraction | Rust globals + builtins build globals + native members |
   | External BUILD | Rust globals + prelude native extraction | same as root BUILD |
   | Root `.bzl` | Rust base globals | Rust base globals + allowed builtins toplevels |
   | External `.bzl` | Rust base globals only | Rust base globals + allowed builtins toplevels |
   | Module extension `.bzl` | Rust bzlmod globals | Rust bzlmod globals + allowed builtins toplevels |

2. Find the right insertion point in the interpreter:
   - `app/kuro_interpreter_for_build/src/interpreter/globals.rs`
   - the file-loader environment construction path
   - module-extension environment construction
3. Decide whether the merged builtins are represented as:
   - dynamic values inserted into each `Module` before eval;
   - a frozen builtins module queried by the loader;
   - Rust wrapper callables that forward to frozen Starlark functions.
4. Build a no-op prototype export such as
   `_kuro_builtins_probe = "ok"` that is visible in a dedicated test
   external `.bzl` file, then remove the probe before landing.

### Acceptance

- A test can load an external repo `.bzl` file that references a
  Starlark-defined builtin from the bundled builtins package.
- The symbol is not visible in contexts where the export contract does
  not request it.
- The prototype does not require workspace files or network access.

## Phase 28.2: Bundled Builtins Loader  [DONE 2026-04-30]

### Status

Loader landed end-to-end:

- New top-level `kuro_builtins/` directory ships an `exports.bzl`
  entry-point (kept minimal — single probe symbol) plus an empty
  `BUILD.bazel`.
- `app/kuro_external_cells_bundled/build.rs` and `lib.rs` register the
  contents as bundled cell `KURO_BUILTINS`. `get_bundled_data()`
  returns it alongside `prelude` / `bazel_tools` /
  `local_config_platform` / `local_config_python`.
- `app/kuro_common/src/legacy_configs/cells.rs` auto-registers
  `@kuro_builtins` for every bzlmod project (mirrors the Phase 17
  `@local_config_platform` registration). Legacy non-bzlmod workspaces
  can opt in via `[external_cells] kuro_builtins = bundled`.
- `app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`:
  - new `bazel_builtins_autoload: Option<OwnedStarlarkModulePath>`
    field resolved unconditionally at `Self::new()` time;
  - appended to `implicit_imports` for both BUILD and `.bzl` paths in
    `parse()` (skipped inside the `kuro_builtins` cell itself);
  - public symbols imported via `import_public_symbols(builtins_env)`
    in `create_env()`, regardless of whether a prelude is registered.
- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_2_kuro_builtins_visible_in_external_bzl`
  (Bazel-mode fixture, no prelude) references `kuro_builtins_probe`
  without a `load()` and gets `"kuro-28-2-loader-ok"` written to a
  build artifact.
- `@llvm-project//llvm:Demangle` builds clean post-Plan 28.2.

### Remaining for Phase 28.2

- The Plan 28.2 acceptance bullet about "rejecting loads outside the
  builtins package" is satisfied for the autoload path (the
  `same-cell` skip in `parse()` and `create_env()` prevents the
  exports module from importing itself), but a *user* `load()` from
  an external file into another file inside `@kuro_builtins` is not
  yet sandboxed. Track as a follow-up — low priority while the
  exports.bzl is the only file in the cell.
- Digest-based DICE invalidation across daemon restarts: the loader
  goes through the standard load resolver, so the bundled file's
  contents are part of the normal incremental key; the explicit
  per-builtins-file digest mentioned in the plan is not strictly
  needed today.

### Goal

Make builtins loading deterministic, cached, and independent of the
user workspace.

### Work

1. Add a bundled builtins directory. Candidate location:
   `prelude/bazel_builtins/`.
2. Add an entry-point file:
   `prelude/bazel_builtins/exports.bzl`.
3. Add a loader that:
   - resolves only bundled builtins paths;
   - rejects loads outside the builtins package unless explicitly
     allowlisted;
   - freezes the builtins module once per interpreter/DICE context;
   - computes a digest of all loaded builtins files so stale daemon
     state is invalidated when builtins change.
4. Thread the frozen export set into environment construction.
5. Keep `__kuro_builtins__` as the Rust primitive namespace for now,
   but make `native` construction merge:
   - Rust native primitives;
   - Starlark exported native members;
   - removed-rule stubs from Plan 27.

### Acceptance

- Builtins load is deterministic and independent of the root cell.
- Syntax or evaluation errors in builtins report the bundled builtins
  path, not an arbitrary workspace path.
- Existing BUILD files behave identically when the export dicts are
  empty.

## Phase 28.3: Initial Low-Risk Starlark Exports  [partial — export contract + 28.4 wrapper hook landed, 2026-04-30]

### Status

The export contract structure from this plan's "Export Contract"
section is now real:

- `kuro_builtins/exports.bzl` defines an explicit
  `exported_toplevels` dict. Only entries listed there reach the
  consuming env; private helpers (leading `_`) and other top-level
  names (e.g. `rule_implementation_wrapper`) are intentionally
  invisible to user `.bzl`/BUILD files.
- `app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs::create_env`
  now reads `exported_toplevels` from the bundled module and copies
  each `(name, value)` into the consuming env's bindings. Replaces
  the Phase 28.2 `import_public_symbols` autoload, moving
  visibility-control logic out of the interpreter and into the
  bundled exports.bzl — anybody adding a name now writes it
  explicitly in the dict.
- Phase 28.4 hook in place: `rule_implementation_wrapper = _invoke_rule`
  is defined at the top of `exports.bzl` (an identity wrapper),
  intentionally NOT in `exported_toplevels` so user files cannot
  reference it. Phase 28.4 Stage 2 will wire
  `kuro_analysis::run_analysis` to call this wrapper and start
  migrating ctx-method bodies.
- New tests:
  - `test_28_3_export_contract_hides_unlisted_symbols` —
    references `rule_implementation_wrapper` from a fixture's
    `defs.bzl` and asserts the load fails with
    "Variable rule_implementation_wrapper not found".
  - `test_28_2_kuro_builtins_visible_in_external_bzl` continues to
    pass via the new dict-based path.

### Remaining for Phase 28.3

The plan's larger candidates (`runfiles` constructor body,
`ctx.target_platform_has_constraint()`, `ctx.runfiles()`,
`ctx.var` / `expand_make_variables`) all touch `ctx`-method
dispatch and are blocked on Phase 28.4 Stage 2 wiring the rule
wrapper through `kuro_analysis::run_analysis`. Once the wrapper
fires, the per-method migration is a one-method-at-a-time exercise.

### Goal

Move a small set of compatibility logic into Starlark to prove the
mechanism before touching action-heavy APIs.

### Candidate Exports

Start with items that are pure value construction or thin delegation:

| Candidate | Why low risk | Notes |
|-----------|--------------|-------|
| small helper globals used only by kuro-owned builtins tests | proves export plumbing | Remove probes after tests or keep under `_kuro_*`. |
| `runfiles` construction helper | mostly data shaping | Must preserve `DefaultInfo`/`FilesToRunProvider` interoperability. |
| `ctx.runfiles()` wrapper body | data shaping around existing runfiles provider | Best done after Phase 28.4 no-op wrapper exists. |
| `ctx.target_platform_has_constraint()` wrapper body | query over existing platform providers/config data | Replaces current Rust host-OS shortcut. |
| `ctx.var` / `expand_make_variables` merge policy | Starlark can express `TemplateVariableInfo` precedence clearly | Native code may still supply raw configuration/toolchain data. |

Do not start with:

- `cc_common.compile()` or `cc_common.link()`;
- artifact declaration;
- action registration;
- provider classes whose identity must match existing Rust provider
  IDs, unless a dedicated provider-identity design is complete.

### Acceptance

- At least one real compatibility behavior moves from Rust to Starlark.
- Existing tests pass without duplicating behavior in both Rust and
  Starlark.
- The moved behavior has a Bazel 9 parity citation.

## Phase 28.4: Rule Implementation Wrapper  [Stage 8 done 2026-05-01]

### Status

Stage 3 lands the first Rust→Starlark `ctx`-method migration. The
wrapper is no longer an identity: `_invoke_rule` in
`@kuro_builtins//:exports.bzl` now installs a Starlark `struct`
facade around `raw_ctx` that mirrors every public attribute and
binds bound-method values for non-migrated methods. The facade
serves `ctx.target_platform_has_constraint(...)` from a Starlark
helper (`_kuro_target_platform_has_constraint`); the Rust impls in
`app/kuro_build_api/src/interpreter/rule_defs/context.rs` and
`app/kuro_build_api/src/interpreter/rule_defs/aspect/context.rs`
were deleted as part of this stage (single-owner discipline,
Plan 28.7).

Host OS/CPU constraint labels are baked into the bundled cell at
kuro build time:
`app/kuro_external_cells_bundled/build.rs` stages the
`kuro_builtins/` source into `OUT_DIR/kuro_builtins_staged/` and
emits `_host_constants.bzl` (a list `HOST_CONSTRAINT_LABELS`
matching the table previously in the Rust impl). `exports.bzl`
loads that file at module evaluation; the facade closes over it.

The Stage 2 design (Stage 3 builds on it):

- `kuro_analysis::analysis::calculation::get_kuro_builtins_module`
  loads `@kuro_builtins//:exports.bzl` via DICE. Returns `None` for
  workspaces where the alias isn't registered.
- `Impl::lookup_rule_implementation_wrapper` reads
  `rule_implementation_wrapper` from the bundled module.
- `Impl::invoke` branches: wrapper present →
  `eval.eval_function(wrapper, &[rule_impl, ctx], &[])`; absent →
  direct invocation.

Why static field enumeration in `_invoke_rule` instead of
`dir(raw_ctx)` + `**`-spread: the first attempt hit a Starlark
`struct(**dict)` field-loss issue captured in
[`thoughts/shared/research/2026-04-30-plan-28-4-stage3-facade-blocker.md`](../../research/2026-04-30-plan-28-4-stage3-facade-blocker.md).
Static enumeration sidesteps the issue and gives a parse-time
failure when a new ctx field lands without a corresponding facade
line.

### Acceptance

- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_4_stage3_facade_in_call_path`
  builds a Starlark rule that asserts `ctx.kuro_facade_active ==
  True` (proves the facade is in the call path) and exercises
  `ctx.target_platform_has_constraint` against a host-matching label
  (positive case) and a non-host label (negative case).
- Stage 2's `test_28_4_stage2_wrapper_passes_through` continues to
  pass — the no-op-equivalence guarantee survives the migration for
  every code path that doesn't touch `target_platform_has_constraint`.
- `@llvm-project//llvm:Demangle` (8 actions, ~4.4 s total, analyze
  ≈ 240 ms) and `@llvm-project//llvm:Support` (183 actions, ~14.7 s
  total, analyze ≈ 194 ms) build clean through the facade. Stage 2
  baseline for Support analyze was 190 ms — facade overhead is ~4 ms
  across 183 rules (~22 µs per rule), well under 1% of analyze time.

### Stage 4 (aspects)

Stage 4 lands the aspect-side counterpart to Stage 3:

- `kuro_analysis::analysis::aspect_calculation::execute_aspect`
  fetches `@kuro_builtins` via the now-`pub(crate)`
  `super::calculation::get_kuro_builtins_module` and looks up
  `aspect_implementation_wrapper`. Wrapper present →
  `eval.eval_function(wrapper, &[impl, target, ctx], &[])`;
  absent → original `impl(target, ctx)`.
- `_invoke_aspect(implementation, target, raw_ctx)` in
  `exports.bzl` mirrors `_invoke_rule` but for `AspectContext`
  (smaller field set: no `attrs`, `outputs`, `executable`, etc.).
  Reuses the same `_kuro_target_platform_has_constraint` shim, so
  aspects now answer the question meaningfully where the previous
  Rust stub returned `False` unconditionally.
- `AspectContext.attr` accessor switched from raise-on-None to
  `NoneOr` so the facade can mirror it eagerly (Starlark has no
  try/except, so a raise here would crash for every attr-less
  aspect — the common case). Aspects with no `attrs` declared see
  `ctx.attr == None` instead of the previous error.
- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_4_stage4_aspect_facade_in_call_path`
  defines an aspect that stuffs facade observations into a
  provider; the collector rule reads that provider and asserts
  `ctx.kuro_facade_active == True`, `ctx.kuro_facade_kind ==
  "aspect"`, plus positive/negative cases for
  `target_platform_has_constraint` from inside the aspect.
- `@llvm-project//llvm:Support` cold build with the aspect facade
  installed: analyze ≈ 205 ms (Stage 3 was 194 ms; +~11 ms over
  the rule-only facade — under 2% of analyze time, well within
  noise for cold-cache analysis).

### Stage 5 (subrules)

Stage 5 closes the per-context-wrapper trio:

- A second TLS slot in
  `kuro_build_api::interpreter::rule_ctx_storage` carries the
  bundled `subrule_implementation_wrapper` `Value` for the duration
  of the enclosing rule's eval. `RuleSpec::Impl::invoke` sets it
  alongside `CURRENT_RULE_CTX`; the same `Drop` guard clears both.
- `FrozenStarlarkSubruleCallable::invoke` reads
  `get_current_subrule_wrapper()` and routes through
  `wrapper(impl, ctx, **kwargs)` when present; absent (legacy / no
  bzlmod) → original `impl(ctx, **kwargs)` direct call.
- `_invoke_subrule(implementation, raw_ctx, **kwargs)` in
  `exports.bzl` builds the same `_make_rule_facade` struct rule
  contexts get (subrules share `AnalysisContext`), tagging
  `kuro_facade_kind = "subrule"` so acceptance tests can prove
  which dispatch path produced the facade. Kwargs forward verbatim
  via `**kwargs` spread to preserve the existing
  named-arg-injection semantics.
- `_invoke_rule` / `_invoke_aspect` / `_invoke_subrule` now share
  `_make_rule_facade(raw_ctx, kind)`; `_make_rule_facade` is the
  single point of truth for the AnalysisContext field set, so
  adding a new ctx field is a one-line edit instead of a
  three-place edit.
- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_4_stage5_subrule_facade_in_call_path`
  verifies the marker, the kind tag, the migrated method (Starlark
  shim works inside subrules), and that a sentinel kwarg
  round-trips through the wrapper.
- LLVM Support cold analyze ≈ 207 ms with all three wrappers
  installed (Stage 2 baseline: 190 ms; Stage 5 overhead: ~17 ms
  across 183 rules ≈ 90 µs/rule, still well under 1% of analyze
  time).

### Stage 6 (`ctx.package_relative_label`)

Stage 6 is the first Stage 4-style migration of a method that
*depends* on facade attributes (rather than only on host info baked
at build time). The Rust impl in `context.rs` was deleted.

- `_kuro_package_relative_label(raw_ctx, label_str)` in
  `exports.bzl` reads `raw_ctx.label.cell` / `.package` and
  constructs the resolved label string. `Label(...)` performs
  canonicalisation via `BazelLabel::parse`. Same input/output
  contract as the Rust impl, including root-cell elision in
  `BazelLabel`'s canonical form.
- Bound via closure inside `_make_rule_facade`:
  `_package_relative_label_bound(label_str)` captures `raw_ctx`
  and forwards to the helper. Subsequent migrations of methods
  that depend on `raw_ctx` follow this pattern (one closure per
  facade construction, ≤ 1 µs per close).
- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_4_stage6_package_relative_label_starlark`
  exercises every branch (`bare_target`, `:target`,
  `//pkg:target`, `@cell//pkg:target`) and pins the canonical
  string by round-tripping through `Label()` itself — robust to
  workspace-name canonicalisation rules.
- LLVM Support cold analyze ≈ 156 ms (Stage 5 was 207 ms; Stage 2
  baseline 190 ms). Within run-to-run noise — closure construction
  and call cost are both negligible.

### Stage 7 (`ctx.tokenize`)

Stage 7 migrates a *pure-function* method — no facade attrs, no
host info, no globals. The Rust impl plus its 60-line
`shell_tokenize` helper in `context.rs` were deleted; the
top-level `_kuro_tokenize` in `exports.bzl` mirrors the algorithm
byte-for-byte for ASCII input. Bound directly into the facade
without a closure (same pattern as
`_kuro_target_platform_has_constraint`).

Translation notes:

- Starlark has no `while` loops, so the iteration uses two
  for-loops bounded by `range(n + 1)` with explicit `i`
  advancement and `break` on `i >= n`. Each step consumes ≥ 1
  input character so the bound is safe.
- Whitespace set matches Rust's `char::is_ascii_whitespace`:
  space, `\t`, `\n`, `\f` (`\x0c`), `\r`. Vertical tab `\v` is
  NOT whitespace per Rust's definition.
- Non-escapable backslash inside double quotes preserves the
  literal `\\` and does not consume the next char (Rust quirk
  that we preserve on purpose).
- Trailing `\\` at end of input (inside or outside quotes) drops
  silently, matching Rust.

Acceptance: pre-existing `test_tokenize` (basic shapes, single/
double-quoted, empty, multi-whitespace) keeps passing through the
Starlark impl. New `test_28_4_stage7_tokenize_starlark` pins the
edge cases the basic test missed: backslash escapes inside and
outside quotes, all four double-quote escapable chars (`"`, `\`,
`$`, `` ` ``), non-escapable backslash quirk, trailing backslash
drop, all five ASCII whitespace separators.

LLVM Demangle smoke clean (8 actions, 5.1 s, analyze 214 ms).

### Stage 8 (`ctx.coverage_instrumented` — global-state hook pattern)

Stage 8 introduces the third migration pattern (after host-info-at-
build-time in Stage 3 and facade-attr-via-closure in Stage 6):
**runtime global-state access via a kuro-internal Starlark builtin**.
The Rust impl in `context.rs` was deleted.

- New module
  `app/kuro_interpreter_for_build/src/interpreter/functions/kuro_runtime.rs`
  registers `kuro_collect_code_coverage()` as an analysis-time
  Starlark global. The function reads
  `kuro_build_api::interpreter::rule_defs::build_config::get_collect_code_coverage()`,
  the per-build `--collect_code_coverage` flag.
- Wired via `register_kuro_runtime` in `register_analysis_natives`,
  so the global is reachable at module-eval time of
  `@kuro_builtins//:exports.bzl` and at every analysis call.
- Naming: every global in this module is `kuro_*`-prefixed. End-user
  code can technically call them (Starlark globals are flat) but the
  contract is "internal to `@kuro_builtins`" — treat as private.
  Future kuro-runtime hooks (e.g. `kuro_compilation_mode()` for the
  upcoming `var` migration) follow the same naming.
- `_kuro_coverage_instrumented(dep = None)` in `exports.bzl` reads
  the flag and returns it, ignoring `dep` (matches the Rust impl,
  which also ignored `dep`). When kuro grows per-target
  instrumentation lists, the per-dep branch lands here.
- Acceptance test
  `tests/core/analysis/test_native_rules.py::test_28_4_stage8_coverage_instrumented_starlark`
  verifies both call shapes (`()` and `(None)`) return the flag's
  default `False` for a build without `--collect_code_coverage`.

LLVM Demangle smoke clean (8 actions, 5.4 s, analyze 221 ms).

### Stack-trace fidelity (inspected 2026-04-30)

End-user error messages with the facade installed, observed against
synthetic typo / wrong-arg fixtures:

- **Source location is preserved.** Errors still pinpoint the exact
  user-code line and column (`defs.bzl:5:9`, with caret-underlining
  the offending expression). No regression from pre-Stage-3
  behaviour.
- **The wrapper frame is visible in tracebacks.** Every error
  caused inside a user impl shows one extra frame, e.g.
  `kuro_builtins/exports.bzl:152, in _invoke_rule` (or
  `:_invoke_aspect`, `_invoke_subrule`). Mild noise for the user
  but informative — points at the dispatch site if they need to
  understand the call shape.
- **Type-name regression on missing-attribute errors.** Before the
  facade, a `ctx.typo` error said `Object of type
  'AnalysisContext'` (or `'aspect_ctx'`). Through the facade it
  says `Object of type 'struct'`. Source location and field name
  are unchanged; only the type label is generic. Worth fixing
  later by switching the facade from `struct()` to a kuro-internal
  Starlark value type with a friendlier `TYPE` constant.
- **Migrated-method errors expose internal names.** A wrong-arg
  `ctx.target_platform_has_constraint()` call reports `Missing
  parameter constraint_value for call to
  kuro_builtins/exports.bzl._kuro_target_platform_has_constraint`.
  Acceptable — the leading underscore tags it as internal and the
  module path tells the user where to look. No fix needed.

No critical regressions. The type-name issue is the only candidate
for a follow-up; deferred until enough ctx fields/methods have
migrated that the cumulative typo cost justifies a custom Starlark
type for the facade.

### Remaining for Phase 28.4

- (Optional) Custom Starlark type for the rule facade so missing-
  attribute errors say `Object of type 'AnalysisContext'` instead
  of `'struct'`. Same shape as `struct()` but with a typed wrapper
  Rust value carrying a `SmallMap<StringValue, Value>`. Low
  priority — see "Stack-trace fidelity" above.

### Goal

Route Starlark rule implementation calls through a bundled Starlark
wrapper so `ctx` compatibility behavior can move out of Rust
incrementally.

### Work

1. Add a wrapper export:

   ```starlark
   def _invoke_rule(implementation, raw_ctx):
       return implementation(raw_ctx)

   rule_implementation_wrapper = _invoke_rule
   ```

2. Change the analysis invocation path so Starlark rules call the
   wrapper instead of directly calling `implementation(ctx)`.
   Candidate code areas:
   - `app/kuro_analysis/src/analysis/env.rs`
   - `app/kuro_analysis/src/analysis/native_rule_analysis.rs`
   - `app/kuro_interpreter_for_build/src/rule.rs`
3. Land the wrapper as a no-op first.
4. Add hooks for wrapping:
   - rule implementations;
   - aspect implementations;
   - subrule implementations.
5. Introduce a Starlark ctx facade only when needed. The first landing
   should avoid changing ctx identity or equality semantics.
6. Move one method at a time:
   - `ctx.target_platform_has_constraint()`
   - `ctx.runfiles()`
   - `ctx.var` / `expand_make_variables`
   - selected `ctx.configuration` computed fields

### Acceptance

- A no-op wrapper produces byte-for-byte equivalent provider results
  for representative Starlark rules.
- Wrapper failures preserve user stack traces well enough to identify
  the user rule implementation.
- Aspects and subrules either use the same wrapper model or are
  explicitly documented as follow-ups.

## Phase 28.5: Native Struct and BUILD Global Integration

### Goal

Replace the current prelude-only `native` construction with a
source-of-truth export merge that can include both Rust primitives and
Starlark builtins.

### Work

1. Keep `prelude/native.bzl` small, but stop making it the only place
   where BUILD-global native members are assembled.
2. Define precedence:
   - user `load()` binding wins at the use site;
   - Starlark exported native member wins over no symbol;
   - Rust primitive wins for action-creating APIs and true native
     modules;
   - removed-rule stubs from Plan 27 win over accidentally reintroduced
     working language-rule implementations.
3. Add tests for:
   - `native.filegroup` or another true native rule remains available;
   - `native.cc_library` has the Plan 27 removed-rule behavior;
   - a Starlark exported BUILD global appears directly in BUILD files;
   - external `.bzl` files do not see BUILD-only exports.

### Acceptance

- BUILD and `.bzl` symbol visibility is tested from root and external
  cells.
- `prelude/native.bzl` remains a thin compatibility file, not a growing
  second builtins registry.

## Phase 28.6: Buck2 Prelude Machinery Disposition

### Goal

After the builtins export path exists, decide the fate of every
remaining Buck2 prelude mechanism. Each piece is either:

- integrated into the Bazel builtins module;
- moved behind a Kuro-specific extension boundary;
- retained as a tiny bootstrap shim with an owner and deletion
  condition; or
- removed.

The end state should be Bazel-shaped: BUILD globals and `native` members
come from the builtins export merge, not from evaluating Buck2's prelude
and scraping a `native` struct out of it.

### Inventory

Audit and classify at least these pieces:

| Piece | Current role | Target disposition |
|-------|--------------|--------------------|
| `prelude/prelude.bzl` | Entry point evaluated to populate BUILD globals | Remove as BUILD-global source after Phase 28.5; keep only as temporary bootstrap shim if needed. |
| `prelude/native.bzl` | Constructs `native = struct(...)` from `__kuro_builtins__` | Integrate into builtins export merge; shrink to compatibility shim, then delete if no caller needs it. |
| `prelude/rules.bzl` / `prelude/rules_impl.bzl` | Buck2-era rule declaration plumbing | Remove unless a remaining symbol is proven to be a Bazel 9 builtin and is better hosted in `bazel_builtins/`. |
| `prelude/user/` | Buck2 user customization hook | Remove from Bazel-compatible builds. Bazel has explicit `load()`/module semantics, not a prelude user overlay. |
| `prelude/decls/` | Buck2 rule declaration helpers | Remove or move only still-needed typed helpers into Rust/native attr code. |
| `prelude/paths.bzl`, `prelude/artifacts.bzl`, `prelude/utils/` | Shared helper libraries for the old prelude | Move only Bazel-sourced helpers into `prelude/bazel_builtins/`; delete Buck2-only helpers. |
| `prelude/bxl/` | Kuro-specific BXL support | Keep, but make it explicitly owned by `kuro bxl`, not by Bazel-compatible BUILD loading. |
| `prelude/toolchains/` and any remaining language/toolchain trees | Buck2 language/toolchain support | Remove from Bazel-compatible preload path; replace with Bazel `rules_*`, `@bazel_tools`, bzlmod, or Kuro toolchain internals. |
| `app/kuro_interpreter/src/prelude_path.rs` | Resolves `@prelude//:prelude.bzl` | Keep only if BXL or another Kuro extension still needs it; otherwise delete or restrict to extension mode. |
| `app/kuro_interpreter/src/file_loader.rs::get_native_symbols_from_prelude` | Scrapes `native` members from prelude evaluation | Delete once BUILD globals come from the builtins export merge. |
| `__kuro_builtins__` namespace | Rust primitive namespace exposed to prelude | Rename or restrict after migration so it is not a public compatibility surface. |

### Work

1. Add a prelude inventory document or section to this plan with one row
   per remaining file/directory and a disposition: `integrate`,
   `extension-only`, `temporary-shim`, or `remove`.
2. Change BUILD-global construction so it no longer depends on:
   - loading `@prelude//:prelude.bzl`;
   - reading a `native` symbol out of the prelude module;
   - `prelude/native.bzl` merging `__kuro_builtins__`.
3. Move any still-needed Bazel-compatible Starlark helpers into
   `prelude/bazel_builtins/` with explicit export dictionaries and
   Bazel 9 parity citations.
4. Move Kuro-only helpers behind explicit Kuro extension boundaries:
   - BXL helpers remain reachable from `kuro bxl`;
   - non-Bazel debug/testing helpers use `_kuro_*` names or test-only
     fixtures;
   - they are not visible in ordinary BUILD or external `.bzl`
     evaluation.
5. Delete or make unreachable the old Buck2 user-overlay path:
   - `prelude/user/all.bzl`;
   - automatic user prelude composition;
   - any `rules_impl.bzl` aggregation that exists only for Buck2
     prelude extensibility.
6. Delete `get_native_symbols_from_prelude` and associated tests once
   Phase 28.5 has an equivalent builtins-export path.
7. Add guardrails:
   - a test that BUILD globals are identical with the prelude shim
     disabled;
   - a test that external `.bzl` files receive builtins exports without
     prelude evaluation;
   - an `rg`-style CI check or unit test preventing new
     Bazel-compatible BUILD globals from being added only to
     `prelude/native.bzl`.

### Acceptance

- Ordinary `kuro build` does not need to evaluate
  `prelude/prelude.bzl` to construct BUILD globals.
- `prelude/native.bzl`, if still present, is a temporary shim with a
  deletion condition and no unique symbol ownership.
- Every remaining file under `prelude/` has an owner:
  `bazel_builtins`, `bxl`, `test fixture`, or `delete`.
- `@prelude//...` loads in user BUILD/.bzl files are either unsupported
  with a clear Kuro/Bazel-compatibility error or explicitly documented
  as Kuro extension APIs.
- No Buck2 language/toolchain prelude directories are reachable from
  Bazel-compatible BUILD loading.

## Phase 28.7: Migration and Deletion Discipline

### Goal

Avoid ending up with two permanent implementations of the same builtin.

### Rules

1. Every migrated builtin has a single owner:
   - Rust primitive;
   - Starlark builtins export;
   - external ruleset.
2. Each migration PR deletes or gates the old implementation.
3. If a Rust fallback remains temporarily, add a TODO with:
   - owning plan phase;
   - parity source;
   - condition for deletion.
4. Any new long-lived string maps introduced by the builtins loader must
   follow [Plan 26](./26-string-interning.md).

### Acceptance

- `rg` can find the owning plan/TODO for every temporary duplicate.
- There is no silent fallback from a Starlark migrated builtin back to
  stale Rust behavior.

## Dependencies

- **Plan 27** can start before Plan 28, but the final native/global
  merge should use Plan 28's export model.
- **Plan 15 Phase 1** decides which provider globals must stay removed.
  Plan 28 must not undo it.
- **Plan 04** remains accurate until Phase 28.1 lands. After that, add
  a note that the builtins loader is the mechanism that makes selected
  Starlark-defined values visible to external `.bzl` files.
- **Plan 28.6** should start after Phase 28.5 has a working replacement
  for BUILD-global construction. Individual file removals can start
  earlier when the inventory proves they are already unreachable.

## Risks

- **starlark-rust environment constraints.** If globals cannot accept
  frozen Starlark values at the needed point, the loader may need Rust
  forwarding callables. The feasibility spike must resolve this before
  real migrations.
- **Provider identity mismatch.** Starlark-defined providers are not
  automatically identical to existing Rust provider IDs. Avoid provider
  migrations until identity semantics are designed.
- **Stack trace quality.** Wrappers can obscure user implementation
  frames. Add tests for error reporting before moving many methods.
- **Startup cost.** Builtins loading must be cached and digest-keyed so
  every `.bzl` load does not re-evaluate bundled files.
- **Hidden BXL/prelude coupling.** Some `prelude/` files may still be
  needed by `kuro bxl` or internal tooling even though Bazel-compatible
  BUILD loading should not see them. Classify these as `extension-only`
  before deleting.

## Verification

Minimum verification before closing this plan:

- `cargo check -p kuro`
- builtins loader unit tests
- external `.bzl` visibility tests
- BUILD global/native struct visibility tests
- prelude-disabled BUILD-global construction test
- inventory/guardrail test proving no Bazel-compatible BUILD global is
  owned only by `prelude/native.bzl`
- no-op rule wrapper equivalence tests
- one migrated ctx behavior with parity tests
- representative rules_cc/rules_python/protobuf/rules_rust builds still
  pass

## Estimated Effort

1 week for the loader and no-op wrapper if starlark-rust accepts the
planned insertion point. 2-3 additional weeks to migrate the first real
ctx behaviors and delete their Rust duplicates. Add roughly 1 week for
the prelude inventory and first removal pass; longer if BXL still
depends on broad `prelude/` loading.
