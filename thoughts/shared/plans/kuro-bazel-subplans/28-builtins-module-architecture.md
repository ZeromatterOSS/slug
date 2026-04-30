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

## Phase 28.1: Feasibility Spike and Environment Matrix

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

## Phase 28.2: Bundled Builtins Loader

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

## Phase 28.3: Initial Low-Risk Starlark Exports

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

## Phase 28.4: Rule Implementation Wrapper

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
