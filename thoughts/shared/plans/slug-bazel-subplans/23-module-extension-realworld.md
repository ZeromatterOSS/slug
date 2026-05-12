# Plan 23: Real-world bzlmod module extension parity (`buildbuddy` pattern)

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Follow-up from Plan 22 (`--config=remote` now parses); discovered while
> wiring Plan 18 (BEP) end-to-end against
> `@llvm-project//llvm:Demangle --config=remote`.

## Scope

Make Slug's module-extension executor handle the extension shape used by
`toolchains_buildbuddy` (and the wider `rules_python` / `crate` /
`apple_support` families that follow the same pattern): an `_ext_impl(mctx)`
function that iterates `mctx.modules`, picks the root module, reads its
tags, and calls `repository_rule(...)` through a wrapping macro.

Today Slug silently captures zero repo specs from this extension and
falls back to a stub repo. The downstream consumer (`--config=remote`'s
`--platforms=@toolchains_buildbuddy//platforms:linux_x86_64`) then errors:

```
Error running analysis for `toolchains_buildbuddy//platforms:linux_x86_64 (<unbound>)`
  ...
  Unknown target `platform_linux_x86_64` from package `buildbuddy_toolchain//`.
  Did you mean one of the 0 targets in buildbuddy_toolchain//:BUILD.bazel?
```

This plan covers diagnosis, fix, and minimal real-world verification. It
does NOT cover remote execution itself (the RE client wiring, platform
constraint resolution, content-addressable store uploads to BuildBuddy
workers) — that becomes Plan 24 once this one lands.

## Current State Analysis

Reverse-engineered path:

- `app/slug_external_cells/src/extension_repo.rs:502` logs
  `Extension '...' did not generate repo '...' (available: []). Creating stub repo.`
  The stub has a single comment line `# Stub repo (...)` — which is what
  we see in `bazel-external/toolchains_buildbuddy+buildbuddy+buildbuddy_toolchain/BUILD.bazel`.
- Bazel's own cache (`/home/wgray/.cache/bazel/.../external/toolchains_buildbuddy++buildbuddy+buildbuddy_toolchain/BUILD`)
  has the real 374-line output with `platform_linux_x86_64`,
  `platform_linux`, `ubuntu_cc_toolchain`, `default_java_toolchain`, etc.
- Slug's execution path:
  `slug_external_cells::extension_repo::...` →
  `ModuleExtensionExecutionKey::compute()` (slug_bzlmod) →
  `MODULE_EXTENSION_EXECUTOR_IMPL` late-binding (set by
  `slug_interpreter_for_build`) →
  `ConcreteModuleExtensionExecutor::execute_extension`
  (`app/slug_interpreter_for_build/src/module_extension_executor_impl.rs:324`)
  → `try_execute_starlark` (line 185).
- Repo rules invoked during the extension body are captured via
  `with_repo_spec_registry` (`slug_bzlmod::repo_spec::RepoSpecRegistry`).
- The fallback on line 384–397 is the bug magnifier:

  ```rust
  let specs = match self.try_execute_starlark(ctx, aggregated, module_ctx).await {
      Ok(specs) => specs,
      Err(e) => {
          tracing::warn!(
              "Could not execute extension '{}' Starlark implementation, \
               falling back to empty specs: {:?}", …);
          fxhash::FxHashMap::default()
      }
  };
  ```

  Any error inside the extension (load failure, Starlark fail(),
  `repository_rule()` registry miss, `native.bazel_version` unsupported,
  attr-access shape mismatch) turns into an empty-specs fallback, which
  is then silently stubbed at the downstream repo-materialization step.
  Root-causing without surfacing that error is guesswork.

Extension source (`bazel-external/toolchains_buildbuddy+0.0.4/extensions.bzl`):

```starlark
def _ext_impl(mctx):
    for m in mctx.modules:
        if m.is_root:
            root_module = m
            found_root_module = True
            break
    ...
    tags = root_module.tags
    platform_tag = tags.platform[0] if tags.platform else None
    ...
    bb_macro(name = "buildbuddy_toolchain", **macro_args)
```

Three plausible root causes (pick once 23.1 surfaces the real error):

1. **`native.bazel_version` unsupported** — referenced in `rules.bzl:46`
   inside the toolchain's repo-rule impl. Slug may not provide it; the
   expression `native.bazel_version and native.bazel_version < "6.0.0"`
   would raise.
2. **`ModuleContext.modules[i].tags.<class>`** shape mismatch — slug's
   `module_ctx` may not expose `tags.<tag_class>` as an attribute-style
   accessor returning a list. `tags.platform` with zero invocations
   should return `[]`, not error; a mismatch here would fail silently
   because `fallback to empty specs` eats the error.
3. **`repository_rule()` invocation through a macro doesn't register**
   with `RepoSpecRegistry` — the rule is called inside `buildbuddy()`
   which is called from `_ext_impl`. If the registry-active thread-local
   isn't propagated through the macro call, nothing registers.

## Phases

### 23.1 Surface extension-execution errors (DONE 2026-04-24)

`app/slug_interpreter_for_build/src/module_extension_executor_impl.rs:384`
fallback now prints a user-visible stderr line
(`warning: module extension '...' failed; any repo it was supposed to
generate will be stubbed out...`) including the Starlark trace that
triggered the fallback. Optional-extension tolerance preserved: the
subscriber still returns empty specs so non-critical extensions don't
fail the build.

Immediately surfaced the real root cause (`fail("buildbuddy must be used
in root module")`), which turned the ambiguity of 23.2 into a one-line
investigation.

Make `execute_extension`'s fallback loud by default. Optional-extension
tolerance must remain, but silent corruption of build analysis is worse
than a noisy error.

Changes in `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`:

- Reclassify the fallback from `tracing::warn!` to a user-visible
  stderr line (e.g. via the existing `console` infrastructure or plain
  `eprintln!`), including the Starlark stack trace.
- Keep returning empty specs so non-blocking optional extensions
  (e.g. telemetry) still don't hard-fail; but print loudly so blockers
  are diagnosable.
- Add a `#[cfg(test)]` fixture extension that `fail()`s in its
  implementation; assert the stderr output includes the failure message.

**Success criteria.** `slug build @llvm-project//llvm:Demangle --config=remote`
prints an actionable error including the extension id and the Starlark
trace that triggered the fallback. Choose the next phase based on that
output.

**Est. effort.** 1-2 hours.

---

### 23.2 Root cause (DONE 2026-04-24)

Two compounding bugs:

1. **Non-canonical extension id as map key.** `aggregate_extensions_with_root`
   and `pre_compute_extension_repo_cells` both used `usage.extension_id()`,
   which returns the path *as written in MODULE.bazel* — `//:extensions.bzl%buildbuddy`
   from `toolchains_buildbuddy`'s self-use vs.
   `@toolchains_buildbuddy//:extensions.bzl%buildbuddy` from llvm-project's
   consumer use. Two entries in the aggregation map, never merged. The
   executor looked up the self-use entry (no `llvm-project-overlay` in
   `tags_by_module`), so `mctx.modules` had zero root modules and the
   extension `fail()`ed at the explicit "must be used in root module"
   check.

2. **`rctx.template(path, Label, subs)` and `rctx.symlink(Label, path)`
   mishandled `Label` values.** Both fell through to `value.to_repr()`,
   writing `"@@cell//templates:foo.tpl"` as the literal file content or
   symlink target. Even with fix #1 producing a correct RepoSpec, the
   materialized repo was garbage.

Read the trace from 23.1 and confirm which of the three hypotheses
applies. Narrow to a single targeted fix before touching semantic code.

Deliverable: a one-paragraph append to this plan capturing the exact
Starlark line that failed and why.

**Est. effort.** 30 min – 1 hour.

---

### 23.3 Canonical extension id (DONE 2026-04-24)

New helper `slug_bzlmod::extensions::canonical_extension_id(bzl_file,
name, declaring_module)` produces a single canonical id for a given
extension regardless of which shape the `use_extension()` call used:

- `//:ext.bzl` declared inside module `X` → `@X//:ext.bzl%name`.
- `@X//:ext.bzl` anywhere else → `@X//:ext.bzl%name`.
- `:ext.bzl` (rare) → same as relative, normalized.

Applied at both call sites that build the aggregation / pending-repo-cell
maps. Result: `tags_by_module` now contains both the owner module and the
consumer module (root), so `is_root` resolution works.

Verification via the end-to-end build: `keys=["llvm-project-overlay",
"toolchains_buildbuddy"]` after the fix (was `keys=["toolchains_buildbuddy"]`
before). Lockfile cache lookup also starts hitting because the lookup key
now matches the shape Bazel wrote into `MODULE.bazel.lock`.

Candidate fixes, in decreasing likelihood based on slug memory notes:

**3a. `repository_rule(...)` invocation through a wrapping macro doesn't
propagate the registry.** The `bb_macro(name=...)` call sits inside
`_ext_impl`; it calls `_buildbuddy_toolchain(...)` (the
`repository_rule`). If slug's Starlark invocation of a repository rule
doesn't walk back up to the active `RepoSpecRegistry`, macro-wrapped
registrations go nowhere.

- Fix location: `slug_bzlmod::repo_spec` or the `repository_rule`
  starlark callable implementation.
- Unit test: synthesize a tiny extension that wraps a `repository_rule`
  call in a user-defined macro and assert the spec is captured.

**3b. `ModuleContext.modules[i].tags.<class>`** missing attribute-style
dispatch. Slug's `ModuleInfo` has `tags: HashMap<String, Vec<ExtensionTag>>`
(see `app/slug_bzlmod/src/extensions.rs:236`). The Starlark-facing view
needs `mod.tags.platform` to return the list of `platform` tag
invocations (empty list when absent), not raise `AttributeError`.

- Fix location: the Starlark value wrapping `ModuleInfo.tags`.
- Unit test: `assert module_ctx.modules[0].tags.nonexistent == []` or
  `tag_class` discoverability matching Bazel semantics.

**3c. `native.bazel_version`** — set to a plausible string
(`"9.0.0-slug"` or the live slug version) during repo-rule evaluation,
or stub to `None` consistently so the `and` short-circuits.

- Fix location: wherever `native` is constructed for repository-rule
  evaluation.
- Precedent: Bazel returns the running Bazel version string here; some
  rules parse it with string comparison.

Each candidate is ~half a day. Pick ONE from 23.2 output; defer the
others to their own issues.

**Acceptance criteria for 23.3.**

1. `slug build @llvm-project//llvm:Demangle --config=remote` no longer
   logs "did not generate repo 'buildbuddy_toolchain'".
2. `bazel-external/toolchains_buildbuddy+buildbuddy+buildbuddy_toolchain/`
   has a real `BUILD` file (not the stub comment).
3. Target node `buildbuddy_toolchain//:platform_linux_x86_64` resolves.
4. The new unit test added in 23.3 passes.

---

### 23.4 rctx materialization completeness (DONE 2026-04-24)

Two small fixes in `app/slug_interpreter_for_build/src/repository_ctx.rs`:

- `template()` now accepts `Label` values: resolves via
  `resolve_label_to_path`, reads the file, applies substitutions. Without
  this the generated `BUILD` file contained `@@toolchains_buildbuddy//templates:BUILD.tpl`
  as its literal body.
- `symlink()` now accepts `Label` as the symlink target, resolving to an
  absolute filesystem path so the resulting symlink actually points at
  the template file. Previously `cc_toolchain_config.bzl` was a dangling
  symlink to the label string.

Together these produce the real 12KB `BUILD` file + working symlinks
under `bazel-external/toolchains_buildbuddy+buildbuddy+buildbuddy_toolchain/`.

Even with 23.3 producing a correct RepoSpec, `_buildbuddy_toolchain_impl(rctx)`
must run end-to-end against slug's repository-context surface:

- `rctx.os.name`, `rctx.os.arch` (used for default-platform selection)
- `rctx.template(path, Label, substitutions)` for `BUILD`, `gcc_config.bzl`,
  `msvc_config.bzl`, `bin/cc_wrapper.sh`
- `rctx.symlink(Label, path)` for `cc_toolchain_config.bzl` and
  `windows_cc_toolchain_config.bzl`
- `rctx.symlink("/usr/bin/ar", "bin/ar")` — absolute-path symlink
- `rctx.attr.<name>` for each of the 10 repo-rule attrs

These are largely already implemented per `03-rule-primitives.md` Tier
1-3 and memory notes. Audit each call during this phase; file a sub-task
only if something specific fails. Likely no code change is needed in
this phase, just verification.

**Success criteria.** Running slug through the extension produces a
BUILD file whose `diff` against bazel's output in
`/home/wgray/.cache/bazel/_bazel_wgray/*/external/toolchains_buildbuddy++buildbuddy+buildbuddy_toolchain/BUILD`
shows only expected differences (cell-name paths, absolute
template-value substitutions — documented in the phase writeup).

**Est. effort.** 1-3 hours if nothing is missing, half a day each for
missing rctx methods.

---

### 23.5 End-to-end checkpoint (DONE 2026-04-24)

Rerun: `slug build @llvm-project//llvm:Demangle --config=remote`.

The buildbuddy extension executes cleanly, generates the real repo,
target `buildbuddy_toolchain//:platform_linux_x86_64` resolves. The next
failure is:

```
Error evaluating build file: `buildbuddy_toolchain//:BUILD`
...
error: Missing named-only parameter `compatible_javacopts` for call to `java_toolchain`
  --> bazel-external/rules_java+9.3.0/java/toolchains/java_toolchain.bzl:27:5
```

This is slug eagerly analyzing Java toolchains from the buildbuddy_toolchain
BUILD file even though the target (`Demangle`) never needs Java. Root cause is
Plan 13 territory (lazy toolchain loading) — not Plan 23.

Plan 23 is done. Follow-up work is:

- **Plan 13 (lazy toolchain loading):** avoid eagerly analyzing Java
  targets when the current build has no Java in its dep graph.
- **Plan 24 (rules_java compat):** when Java IS needed, accept
  `compatible_javacopts` and the rest of Bazel 9's `java_toolchain`
  surface.
- **Plan 25 (remote execution client):** actually dispatch actions to
  BuildBuddy's executor via `--remote_executor=grpcs://...` +
  `--remote_header=x-buildbuddy-api-key=...`.

None of those are blockers for local builds + BEP upload, which is the
Plan 18 success path and already works today.

Re-run `slug build @llvm-project//llvm:Demangle --config=remote`. Expect
one of two outcomes:

- **Build succeeds locally** (slug ignores the `--remote_executor` flag
  and falls through to local execution). That confirms Phases 23.1–23.4
  plus Plan 22.1 land this part of `--config=remote` parity.
- **Build fails on something new** (RE client, constraint resolution,
  actual remote dispatch). Capture the failure in a new plan (candidate
  Plan 24: "Remote execution against BuildBuddy"). Do not chase in this
  plan.

Either way, record the invocation's BuildBuddy URL and file a snapshot
alongside this plan so the next session can see what the user-facing
stream looked like after Plan 23.

---

## Dependencies and ordering

```
Plan 22.1 (landed)
  │
  ▼
23.1 (surface errors) ──► 23.2 (diagnose) ──► 23.3 (fix one thing)
                                                  │
                                                  ▼
                                             23.4 (rctx audit)
                                                  │
                                                  ▼
                                             23.5 (checkpoint)
```

## Out of scope

- Remote execution client — Plan 24 candidate.
- `--remote_header=x-buildbuddy-api-key=…` actually plumbed through the
  RE client — tracked in Plan 22.2.
- Other module extensions that hit the same root cause — they become
  "LANDED" automatically when 23.3 fixes the underlying semantic gap.

## Success criteria (plan)

- `slug build @llvm-project//llvm:Demangle --config=remote` either
  succeeds (local fallback) or fails with a specific, actionable error
  pointing at the remote-execution subsystem (scope handoff to Plan 24).
- The `buildbuddy` extension produces a real `buildbuddy_toolchain` repo
  under slug.
- New unit test covering the identified gap lives in
  `app/slug_bzlmod/` or `app/slug_interpreter_for_build/`.
- The extension-failure path surfaces a readable error to stderr by
  default.
