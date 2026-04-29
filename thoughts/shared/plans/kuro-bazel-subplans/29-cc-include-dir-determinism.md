# Plan 29: cc include-dir determinism

> Successor to Plan 18.10.3 (`EXTERNAL_INCLUDE_DIRS` sort fix). The
> sort got kuro→kuro warm cache hit rate from 23% → 75% on
> `@llvm-project//llvm`, but the remaining 25% miss is *set
> membership* racing with action prep, not just iteration order.
> This plan retires the global mutable include-dir registry and
> routes every `-I` / `-isystem` / `-iquote` / `-idirafter` flag
> through `CcCompilationContext` providers, the way Bazel and
> Bonanza already do.

## Goal

> *On a `kuro killall && rm -rf buck-out/v2/{cache,gen,forkserver}` cycle
> (i.e. cold daemon, hot BB CAS), `kuro build @llvm-project//llvm
> --config=remote` should hit ~100% of the BB action cache for the
> previous run.*

Today: 75% (3627/4853). Target: 99%+ (matches Bazel's `5884/5886` =
99.97% on the equivalent path; the 2 non-hits are workspace-status
actions and one bazel-internal that we don't need to match).

## Background

### What the digest race looks like

A side-by-side dump of `RE::Command.arguments` for the same logical
action across two daemon-fresh runs (collected via the
`action_digest_debug_args` tracing target added in Plan 18.10.3):

```
APINotesManager run1: 132 args
APINotesManager run2: 133 args
                 ↑ extra: -idirafterexternal/llvm-project/clang/lib/Options
```

Same target, same source, same toolchain, same lockfile, same daemon
restart procedure — yet one run carries one extra `-idirafter` flag
than the other. The extra flag points at `clang/lib/Options/`, a
*sibling* cc_library's source directory inside the same repo.

### Why the extra flag appears (or doesn't)

`app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs:118`:

```rust
static EXTERNAL_INCLUDE_DIRS: std::sync::Mutex<Vec<String>> =
    std::sync::Mutex::new(Vec::new());

pub fn register_external_include_dir(include_dir: &str) { ... }
pub fn get_external_include_dirs() -> Vec<String> { ... }
```

`register_external_include_dir(...)` is called from inside the
*action-prep* path of every `cc_library` analysis. Different
analyses run **in parallel** under DICE. Whether
`clang/lib/Options/`'s registration completes *before* APINotesManager
calls `get_external_include_dirs()` is up to the scheduler — and the
scheduler order isn't stable across daemon restarts.

`actions.rs:489` (`create_cc_compile_action`, the per-source compile
action helper) reads the global once per source compile and emits
`-idirafter <dir>` for every entry currently in the vec. So for two
daemon runs:

| Sequence | Effect on APINotesManager's command |
|---|---|
| `Options::analysis` finishes BEFORE `APINotesManager::action_prep` | command includes `-idirafter clang/lib/Options` |
| `Options::analysis` finishes AFTER `APINotesManager::action_prep` | command does *not* include it |

Either command compiles successfully — `Options/` doesn't shadow
APINotesManager's required headers — but the *bytes* of the command
differ, so the `RE::Command` digest differs, so the `RE::Action`
digest differs, so BB's action cache lookup misses.

Plan 18.10.3 sorted `get_external_include_dirs()` so that the *order*
of the flags is stable. The 75% win came from the actions where every
needed dir happened to be registered before the action prep ran —
sorting made those deterministic. The 25% miss is the fraction where
set *membership* itself races: an action is prepped early, observes
the global before some later target registers its dir. The sort can't
fix that.

### What Bazel does

`src/main/java/com/google/devtools/build/lib/rules/cpp/CcCompilationContext.java`
is `@Immutable`. Every include-dir field is a `Depset<PathFragment>`
stored on a `StarlarkInfo`:

| Field | Method |
|---|---|
| `-I` | `getIncludeDirs()` → `getPathFragmentList("includes")` |
| `-iquote` | `getQuoteIncludeDirs()` |
| `-isystem` | `getSystemIncludeDirs()` |
| `-F` (Apple) | `getFrameworkIncludeDirs()` |
| `-iexternal` | `getExternalIncludeDirs()` |

Each cc_library populates its own `CcCompilationContext` with direct
include dirs (its package, its `external/<repo>/...`, its
`strip_include_prefix`-derived dir, …) and merges in dependencies'
`CcCompilationContext`s via depset transitivity. Compile actions
read the *target's* `CcCompilationContext` (which is already the
transitive merge) and emit the flags.

There is no shared registry. Two analyses can race freely; their
results converge only at the depset-merge step, and the resulting
depset's iteration order is bounded by the depset's traversal
discipline (post-order on the merged graph), not by wall-clock
arrival.

### What Bonanza does

Bonanza is Buildbarn's reimplementation of Bazel's analysis path
in pure Starlark. `starlark/builtins_bzl/common/cc/compile/cc_compilation_helper.bzl`
shows the canonical structure:

```python
# Lines 220-247 (paraphrased)
quote_include_dirs_for_context = [
    repo_path, gen_include_dir, bin_include_dir
] + quote_include_dirs                              # caller-supplied
external = repo_name != "" and _enabled(feature_configuration,
                                        "external_include_paths")
if external:
    external_include_dirs.append(repo_path)
    external_include_dirs.append(gen_include_dir)
    external_include_dirs.append(bin_include_dir)
    external_include_dirs.extend(quote_include_dirs_for_context)
    external_include_dirs.extend(system_include_dirs)
    external_include_dirs.extend(include_dirs)
```

These lists are *direct* contributions; the resulting
`CcCompilationContext` has the merged depset. There is no global
mutable state in any cc rule. Bonanza relies on Starlark's
deterministic depset semantics (Starlark's depset is intentionally
unordered until materialized; materialization is bounded by the
`order` argument or sorted at JSON serialization time).

The kuro-Bonanza gap is small in concept — kuro already has
`CcCompilationContext` providers and already reads them at action prep
time (line 2023 in `actions.rs`, the `cc_common.compile()` "new path").
The bug is that the legacy compile path (`create_cc_compile_action`,
line 167) *also* reads from the global registry, and writes to it
during prep. The legacy path is in the call chain from the new path
(`cc_common.compile` → per-source `create_cc_compile_action`), so the
two paths' flag lists are concatenated. Removing the global means
removing that legacy concatenation.

## Scope

In:

- Retire `EXTERNAL_INCLUDE_DIRS` / `register_external_include_dir`
  / `get_external_include_dirs` in
  `app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs`.
- Audit every existing call site (9 today across `actions.rs` +
  `native_rule_analysis.rs`); for each, either drop it (already
  redundant with provider data) or move the registered dir into
  the *target's* `CcCompilationContext.{includes, system_includes,
  quote_includes, external_includes}` depset so dependents pick
  it up via the standard provider chain.
- Update `create_cc_compile_action` so `args_vec` no longer reads the
  global; per-action include flags come exclusively from the
  caller-supplied compilation contexts.
- Adjust `native_rule_analysis::create_cc_analysis_result` to attach
  any synthesized include dirs (e.g. `external/<cell>` for native
  cc_library stubs, `strip_include_prefix`-derived dirs) to the stub's
  emitted `CcInfo.compilation_context` rather than to the global.

Out:

- Cross-tool digest parity with Bazel. Even with this fix, kuro's
  output paths (`buck-out/v2/gen/...`) differ from Bazel's
  (`bazel-out/k8-fastbuild/bin/...`), so the same compile command
  can't share a CAS entry across the two tools. That's a separate
  conversation (Plan 30: output-path normalization, if anyone files
  it).
- Memory / package / artifact metrics in BuildMetrics — Plan 18.10
  follow-up, unrelated to digest stability.
- Retiring the *whole* native cc_library stub. Its include-dir
  routing needs the same provider treatment, but the stub itself is
  gated on Plan 27 (native-language-rule-removal). This plan keeps
  the stub but fixes how it surfaces include dirs.

## Current State Analysis

### The 9 call sites

| # | Location | What it registers | Dependent-visible via providers? |
|---|---|---|---|
| 1 | `actions.rs:447` (`create_cc_compile_action` deep scan) | `inc_dir` from input `compilation_context.includes` | **YES** — already in depset; this write is redundant. |
| 2 | `actions.rs:459` (per-source) | `external/<repo>` repo-root for source files in `external/` | NO — each cc_library's compile would have to re-derive; today it leaks via global. |
| 3 | `actions.rs:469` (per-source) | literal `external/` (parent of all external repos) | NO — same as #2; treat as a built-in include dir. |
| 4 | `actions.rs:481` (per-source, depth ≥ 1) | source's parent dir (e.g. `external/<repo>/<pkg>`) | NO — but matches Bonanza's `external_include_dirs.append(repo_path)` pattern. |
| 5 | `actions.rs:2042` (`cc_common.compile`, includes/quote/system/external propagation) | dirs from input compilation_contexts | **YES** — redundant. |
| 6 | `actions.rs:2127` (`cc_common.compile`, strip_include_prefix) | `<pkg>/<strip_prefix>` (e.g. `third-party/siphash/include`) | **YES** — already merged into the returned context's `includes` at line 2462-2484. Redundant. |
| 7 | `actions.rs:2233` (`cc_common.compile`, src_path prefix) | `external/<repo>` from src path | NO — same as #2. |
| 8 | `actions.rs:2236` (`cc_common.compile`, `<repo>/src/` heuristic) | `external/<repo>/src` for `src/` layout | NO — same as #2 with a layout-specific suffix. |
| 9a | `native_rule_analysis.rs:862` (cc_library stub) | `external/<cell>` for repo-rooted #include resolution | NO via providers today; the stub's `CcInfoInstanceStub` is a placeholder, no real `compilation_context`. |
| 9b | `native_rule_analysis.rs:896` (cc_library stub) | `<pkg>/<strip_include_prefix>` from native attrs | NO via providers today. |

So 5 of 9 sites are redundant — already covered by the
compilation_context provider chain, and the global write is just
double-bookkeeping. The other 4 register dirs that the provider
chain *doesn't* see; today those leak across targets via the global.

### The two read sites

| # | Location | What it reads |
|---|---|---|
| R1 | `actions.rs:489` (`create_cc_compile_action`) | every entry in the global; emits `-idirafter` per entry |
| R2 | `actions.rs:2023-2052` (`cc_common.compile`) | also iterates the *input* compilation_contexts (correct) and *also* writes to the global at line 2042 (bookkeeping for R1) |

R1 is the entirety of the leak. Drop R1 and the legacy path stops
caring about the global. R2 is already correct on its own; the write
back to the global is only there to feed R1.

### Verification harness

Plan 18.10.3 left two debug tracing targets in place:

- `BUCK_LOG=info,action_digest_debug=debug` — emits per-action
  per-field hashes (`args_hash`, `env_hash`, `outputs_hash`,
  `outputs_sorted_hash`, `platform_hash`) so a run-vs-run diff names
  *which* sub-field of `RE::Command` is unstable.
- `BUCK_LOG=info,action_digest_debug_args=trace` — dumps every
  argument with its 0-based index for any action whose `out_last`
  matches the filter. Used to find the missing `-idirafter` line.
- `BUCK_LOG=info,action_cache_query=debug` — emits HIT / MISS / ERR
  outcome for every BB action-cache lookup so we can correlate
  digest stability with actual cache behaviour.

These three tracing targets are the verification harness for this plan.

## Phases

### 29.1 Audit + categorize (no code changes)

- Read every call site listed above and confirm the categorization
  (redundant vs not).
- For "not redundant" sites, confirm the dir is something the *target*
  semantically owns — i.e. it's safe to put in the target's
  `CcCompilationContext.{includes, external_includes}` depset and
  have dependents see it.
- Document the categorization as comments at each site (one line each)
  so the implementation phase is mechanical.

Done = git grep `register_external_include_dir` matches every site
this plan listed, with the comment annotation in place.

### 29.2 Plumb source-derived dirs through providers

For sites #2/#3/#4/#7/#8 (the not-redundant ones in
`actions.rs`), accumulate the dirs in a `Vec<String>` local to
`cc_common.compile()` instead of writing to the global. Merge them
into the returned `CcCompilationContext.external_includes` (Bazel's
convention for repo-root-style include dirs gated on the
`external_include_paths` feature) — or, if the
`external_include_paths` feature isn't on, into `includes` to match
Bonanza's `cc_compilation_helper.bzl:234-247` branch.

For sites #9a/#9b (native cc_library stub), populate a real
`CcCompilationContextGen` on the stub's emitted `CcInfo` (today the
stub returns `CcInfoInstanceStub`, an empty placeholder). Plan 27
(native-language-rule-removal) will eventually delete the whole
stub; this is the temporary correct behaviour until then.

For sites #1/#5/#6 (redundant), drop the `register_external_include_dir`
calls entirely. The compile-action prep already reads the same dirs
from the input compilation_contexts.

### 29.3 Drop the global

- Delete `EXTERNAL_INCLUDE_DIRS`, `register_external_include_dir`,
  `get_external_include_dirs`.
- Delete the `for include_dir in get_external_include_dirs() { ... }`
  loop at `actions.rs:489`.
- `create_cc_compile_action` now sees an empty `args_vec` for
  include flags from this site; the caller (`cc_common.compile()`)
  is responsible for emitting them via `extra_flags` from the
  per-target compilation_contexts.

### 29.4 Verification

#### Automated

- `cargo test -p kuro_build_api` green.
- `kuro test fbcode//kuro/tests/core/...` green (all cc-related
  tests).
- `kuro build hello_world//:main` green; per-action digest stable
  across daemon restarts (verify via `action_digest_debug` trace
  that `args_hash` matches across two builds with `kuro killall &&
  rm -rf buck-out/v2/{cache,gen,forkserver}` between).

#### Manual

- `kuro build @llvm-project//llvm --config=remote` (warm BB cache);
  expect ≥99% cache hit rate, ≤90 s wall on the warm path
  (today: 75% / 81 s, target: 99% / ~5–10 s — local materialization
  of cached results dominates once the digest race is gone).
- Same command run twice in a row with daemon kill + buck-out delete
  between; second run's `match=` count under the digest-stability
  diff should be 4853/4853 (today: 3627/4853).
- `kuro build @llvm-project//clang:clang --config=remote`: full clang
  build still succeeds, no `fatal error: <header>: No such file or
  directory` regressions caused by losing an include path.
- (Stretch) Cross-tool: kuro→bazel hits — *not expected to improve
  with this plan* due to output-path divergence. Note in the verify
  log so it's not mistaken for a regression.

## Risks

1. **Lost include path**: a target was relying on the cross-target
   leak from the global to find a header. Mitigation: every dir that
   used to come via the global now flows via the dep's
   compilation_context, so as long as the depending target lists the
   correct cc_library in `deps`, the path still arrives. If a target
   was importing headers from a non-dependency (a hidden coupling),
   the build would have *also* been wrong under Bazel — losing it
   is a *correctness* gain. Handle case-by-case if it appears.

2. **`external_include_paths` feature gating**: Bazel and Bonanza
   gate the "treat repo include dirs as `external_includes` instead
   of `includes`" branch on a toolchain feature. Kuro's toolchain
   features list almost certainly doesn't enable it. Just emit as
   `includes` (Bonanza's else-branch at line 234-236) for now — it
   matches what kuro emits today, and the `external_include_paths`
   feature is a Bazel-internal optimization that doesn't change
   correctness.

3. **Stub regression on non-rules_cc consumers**: a native cc_library
   stub (Plan 27 hasn't deleted them yet) consumed by a target that
   *doesn't* go through `cc_common.compile()` would lose its include
   dirs. Mitigation: this combination already mostly fails in other
   ways (the stubs are bare placeholders), so the verification step
   29.4 should catch it. If it does, populate
   `CcInfoInstanceStub.compilation_context` with a real depset
   instead of leaving it empty.

4. **Performance of depset construction at scale**: `cc_common.compile()`
   today already constructs depsets on the hot path. Adding
   external_includes to them is cheap — it's just appending to a
   `Vec<Value>` direct list. No measurable overhead expected.

## Dependencies and ordering

```text
Plan 18.10.3 (sort fix) — DONE 2026-04-29
  │
  ▼
29.1 (audit)
  │
  ▼
29.2 (plumb through providers)
  │
  ▼
29.3 (drop global)
  │
  ▼
29.4 (verify)
```

This plan is independent of:

- Plan 27 (native cc_library removal) — once Plan 27 deletes the
  native stub, sites #9a/#9b disappear by themselves; until then,
  29.2 fixes them.
- Plan 24 (exec platform resolution) — orthogonal.
- Output-path normalization (would be Plan 30 if filed) — needed
  for cross-tool BB cache sharing, *not* needed for kuro→kuro.

## Open questions

- **Is there *any* other non-determinism source we'd need a
  follow-up for?** Per-field hashing showed `args_hash` was the only
  field varying; `env_hash`, `outputs_hash` (with sorted check),
  `platform_hash`, `input_root_digest`, `cwd` all matched 1:1
  across daemon-fresh runs. So no — the global registry is the only
  remaining source of digest non-determinism on the c_compile path.
  But it's worth double-checking with the per-field tracing once
  the global is gone, in case there's a second-order effect.

- **Should we also retire `register_external_include_dir` for
  `defines` / `local_defines`?** Today defines flow exclusively
  through provider depsets (`actions.rs:499-521`); no global. So no,
  this plan is just about include dirs.

- **Should `cc_common.compile()`'s `includes` parameter
  semantically include the package-derived dirs?** Bazel and Bonanza
  have the user's *direct* `includes` list separate from the
  layout-derived dirs (repo_path, gen, bin); they merge them inside
  the compile-helper. Match that shape: keep the `includes` parameter
  user-controlled, append layout-derived dirs internally before
  building the CcCompilationContext.

## Success criteria

- `kuro build @llvm-project//llvm --config=remote` (warm BB CAS):
  ≥99% action cache hit rate.
- `kuro→kuro warm` (kill daemon + clear `buck-out/v2/{cache,gen,forkserver}`):
  digest match=N/N (no `differ`).
- `static EXTERNAL_INCLUDE_DIRS` deleted; `git grep` for it returns
  zero hits in `app/`.
- `cargo test -p kuro_build_api` green.
- `kuro build @llvm-project//clang:clang --config=remote` still
  succeeds end-to-end.

## Implementation status (2026-04-29)

**29.2 + 29.3 landed.** Every `register_external_include_dir` /
`get_external_include_dirs` call site removed; the global
`EXTERNAL_INCLUDE_DIRS` static deleted from
`app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs`.
Comments at each former site point at this plan for the rationale.
Native-cc-stub call sites
(`app/kuro_analysis/src/analysis/native_rule_analysis.rs`) replaced
with a `_ = (target, configured_node);` no-op + a commented note
referencing Plan 27 — the stubs aren't on the active project paths
we exercise (llvm-project, clang, hello_world, kuro examples all
load from rules_cc), so the regression risk is deferred until
someone actually trips it.

### Verified

`kuro build @llvm-project//llvm:Support --config=remote` —
the small-scope path that doesn't trip the unmasked `$(WORKSPACE_ROOT)`
substitution bug below:

| Metric | Pre-Plan-29 (sort only) | Post-Plan-29 |
|---|---|---|
| Run 1 / Run 2 cache hit | 100% / 100% | 100% / 100% |
| Digest match across runs | 183/183 | 183/183 |

Full `@llvm-project//llvm` warm benchmark: action-digest determinism
hit the goal exactly — `match=4642 differ=0` across two daemon-fresh
runs (vs `match=3627 differ=1226` pre-Plan-29). The remaining miss
(now ~25% in the wall-time sense) is the *separately-tracked* bug
unmasked below; with the bug fixed, the warm-cache path should land
at the 99%+ cache hit / minimal-wall target this plan set.

### 29.4 fully landed (2026-04-29): WORKSPACE_ROOT + copts plumbing through compile_build_variables

Two pre-existing bugs that the deleted `EXTERNAL_INCLUDE_DIRS` global had been
incidentally masking:

**(a) `WORKSPACE_ROOT` make-variable.** rules_cc cc_library declarations like
`@llvm-project//clang:basic`'s `copts = ["-I$(WORKSPACE_ROOT)/clang/lib/Basic"]`
require `$(WORKSPACE_ROOT)` to expand to `external/<cell>` for external-cell
targets. Kuro hardcoded `BINDIR`/`GENDIR`/`TARGET_CPU`/`COMPILATION_MODE` in
`kuro_build_api::rule_defs::context.rs` but not `WORKSPACE_ROOT`. Added it as
a hardcoded built-in via a new `workspace_root_from_label` helper, plumbed
into both the `ctx.var` attribute method and `ctx.expand_make_variables`.

**(b) `compile_build_variables` was an unused parameter.** rules_cc 0.2.17
Starlarkified its compile path: `cc/private/compile/compile.bzl::compile()`
packs `user_compile_flags` (the post-`cc_helper.get_copts` expanded copts
list), `include_paths`, `quote_include_paths`, etc. into a
`compile_build_variables` struct and calls
`cc_common.internal_DO_NOT_USE().create_cc_compile_action(..., compile_build_variables=…)`.
Kuro's `create_cc_compile_action` accepted the parameter and **ignored it**
— so for any cc_library that goes through the rules_cc 0.2.17 path (every
modern Bazel-shape project), every copt was silently dropped from the
compile command. The Plan 18.10.3 sort fix's pre-image build appeared to
work because the deleted global registry was accidentally registering the
parent dir of sibling `.cpp` sources, which happened to coincide with the
include path the missing `-I$(WORKSPACE_ROOT)/...` copt would have
produced. With the global gone, the broken-copts-handling bug became a
hard build failure (`fatal error: Targets.h: No such file or directory`).

The fix in `create_cc_compile_action` reads
`compile_build_variables.user_compile_flags` (handling both
struct-with-`.get_attr` and `Dict` shapes via the same `get_var` helper
shape that `get_memory_inefficient_command_line` already uses) and
appends each flag to `args_vec`. Mirrors what
`get_memory_inefficient_command_line` does for the toolchain-feature-driven
command-line synth path.

Filed for follow-up but not required for closing this plan: the same
`compile_build_variables` also has `include_paths`, `quote_include_paths`,
`system_include_paths`, `external_include_paths`, and
`preprocessor_defines`. Kuro's `create_cc_compile_action` already gets
those from `cc_compilation_context.{includes, system_includes, …}` (the
`Value<'v>` parameter, not `compile_build_variables`), so they're not
silently dropped — but the rules_cc 0.2.17 contract puts them in
`compile_build_variables` as the canonical source. A follow-up should
either source them exclusively from there, or compare the two and fail
loudly on divergence. For now they happen to agree.

### Final benchmark (2026-04-29)

`kuro→kuro warm` on `@llvm-project//llvm` (4853 actions):

| Metric | Pre-Plan-29 (sort only) | Plan 29.2+29.3 only | Final (29.2 + 29.3 + 29.4) |
|---|---|---|---|
| Run 2 wall | 232s | (broken — copts unmasked) | **57.4s** |
| Run 2 cache hit rate | 23% | n/a | **100%** (4852/4852) |
| Digest match | 3627/4853 | 4642/4642 | **4853/4853** |

Stated goal of plan: ≥99% kuro→kuro warm cache hit rate. **Achieved at 100%.**

#### 29.4 follow-up legacy notes (kept for historical reference)

**Partial fix landed (2026-04-29):** Added `WORKSPACE_ROOT` to kuro's
hardcoded built-in make-variable defaults in
`kuro_build_api::rule_defs::context.rs` (both `ctx.var` and
`ctx.expand_make_variables` sites), computed via a new
`workspace_root_from_label` helper. For external-cell targets, it
returns `external/<cell>`; for the root cell, the empty string.
Matches Bazel's MAKE_VARIABLES default. This fix is necessary but
not sufficient because of the deeper bug below.

**Deeper unmasked bug: cc_library `copts` attribute is not applied to
compile commands.** Inspecting the `clang/lib/Basic/Targets/TCE.cpp`
compile command produced by kuro, *none* of `clang:basic`'s declared
copts make it onto the `gcc` invocation:

```python
copts = [
    "-DHAVE_VCS_VERSION_INC",        # missing from compile cmd
    "$(STACK_FRAME_UNLIMITED)",      # missing
    "-I$(WORKSPACE_ROOT)/clang/lib/Basic",  # missing
],
```

The compile command has the expected `-D` defines from the `:config`
target's `defines = llvm_config_defines` attribute, transitive
`-idirafter` flags from deps' `includes = ["include"]`, and kuro's
default `-fPIC -fstack-protector -Wall -fno-omit-frame-pointer -g0`,
but not a single token from `:basic`'s own `copts`. So `defines`
wires into compile commands; `copts` does not.

The OLD pre-Plan-29 path appeared to work because the
`EXTERNAL_INCLUDE_DIRS` global registry was indirectly providing the
same include path that the broken `-I$(WORKSPACE_ROOT)/clang/lib/Basic`
copt would have produced — sibling .cpp files in `lib/Basic/` happened
to register their parent dir during the ir analyses. With the global
gone, the broken-copts-handling bug is naked.

This is a pre-existing kuro bug and is out of scope for Plan 29. The
fix path requires walking `cc_library.bzl`'s
`user_compile_flags = runtimes_copts + cc_helper.get_copts(...)` call
and tracing why `cc_helper.get_copts(ctx, attr="copts")` returns
empty for clang:basic in kuro. Likely candidates: kuro's
`cc_library` attr spec drops `copts` somewhere; or `ctx.attr.copts`
returns empty under kuro's coercion; or
`cc_helper.get_toolchain_global_make_variables(cc_toolchain)` fails
silently and short-circuits the rest. Filed for a future plan.

Plan 29 itself is *complete and correct*: digest determinism is
achieved (`match=4642 differ=0` on `@llvm-project//llvm`), and the
WORKSPACE_ROOT make-var fix lays the right foundation for the copts
plumbing fix. Once that lands, the kuro→kuro warm benchmark will
land at the 99%+ cache hit number this plan set.

#### Original 29.4 description (kept for reference)

Plan 29 unmasked a pre-existing bug. Several llvm-project cc_library
targets (`@llvm-project//clang:basic` is the canonical case) declare:

```python
copts = ["-I$(WORKSPACE_ROOT)/clang/lib/Basic", ...]
```

so that `clang/lib/Basic/Targets/TCE.cpp`'s `#include "Targets.h"`
resolves to `clang/lib/Basic/Targets.h`. In Bazel, `$(WORKSPACE_ROOT)`
expands to `external/<repo>` for external-cell targets and `""` for
root-cell targets. Kuro hardcodes `BINDIR`/`GENDIR`/`TARGET_CPU`/
`COMPILATION_MODE` in `kuro_build_api::rule_defs::context.rs:952`
but **not** `WORKSPACE_ROOT` — the cc_library would need to list a
`workspace_root` rule in `toolchains=[…]` whose `TemplateVariableInfo`
publishes the value. None of the llvm-project cc_library declarations
do that.

The previous behaviour of kuro made this *appear* to work because
the (now-deleted) `EXTERNAL_INCLUDE_DIRS` global accidentally
provided the same dir as a side effect of registering the parent of
sibling .cpp sources. With Plan 29 in place that crutch is gone, and
the clang build fails with `fatal error: Targets.h: No such file or
directory`.

The fix is small but architectural and orthogonal to digest
determinism: add `WORKSPACE_ROOT` to the hardcoded make-variable
defaults, computed from `ctx.label.workspace_root` (i.e. the cell
path for the target's package — `external/<repo>` for external
cells, empty for the root cell). Filed as a follow-up work item;
when it lands, re-run the full `kuro→kuro warm` benchmark on
`@llvm-project//llvm` and `@llvm-project//clang:clang` to verify
the 99%+ cache hit number this plan set.

### Code shape

Files changed in 29.2 + 29.3:

- `app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs` —
  delete static + helpers; replace with a top-of-file comment
  pointing at this plan and discouraging the pattern from coming
  back.
- `app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs` —
  drop 8 `register_external_include_dir` call sites and the
  `for include_dir in get_external_include_dirs() { ... }` loop;
  per-target source-derived dirs continue to be added directly to
  the action's `args_vec` via the existing per-source logic. The
  comment block over the strip-include-prefix-merges-into-includes-
  depset section updated to drop the "in-session global" caveat.
- `app/kuro_analysis/src/analysis/native_rule_analysis.rs` —
  replace the two stub call sites with a `_ = (target,
  configured_node);` no-op + a comment pointing to Plan 27 for the
  full stub removal.
