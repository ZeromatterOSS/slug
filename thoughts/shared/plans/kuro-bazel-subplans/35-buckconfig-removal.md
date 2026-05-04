# Plan 35: Remove BUCK and `.buckconfig`

> **Main Plan**:
> [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> **Related**:
> - [Plan 02: bzlmod](./02-bzlmod.md) (cells migrate to `MODULE.bazel`)
> - [Plan 04: Prelude Architecture](./04-prelude-architecture.md)
> - [Plan 09 §9d-9e: Unified Execution Architecture](./09-unified-execution-architecture.md)
>   (made `.buckconfig` cell sections *optional* when `MODULE.bazel` exists;
>   Plan 35 finishes the migration by deleting them outright and draining
>   the remaining sections)
> - [Plan 28: Bazel Builtins Module](./28-builtins-module-architecture.md) (bundled-cell loader replaces `[external_cells]`)

## Scope

Today the kuro source tree has roughly:

- **~336 `BUCK` files in active workspaces** (`./BUCK`, `app/**/BUCK`,
  `examples/**/BUCK`, `prelude/**/BUCK`, `shim/**/BUCK`, `bazel_tools/**/BUCK`,
  plus the BUCK files under `tests/` that define the *test targets themselves* —
  not the fixtures inside `*_data/` directories).
- **~28 active-workspace `.buckconfig` files** (root, `prelude/`, `shim/`,
  `bazel_tools/`, `tests/manual_test/`, and ~22 under `examples/`).
- **~290 test-fixture `.buckconfig` files** under
  `tests/{core,e2e}/**/test_*_data/` — each fixture is a self-contained
  mini-workspace used by an integration test. Most fixtures use
  `[buildfile] name = TARGETS.fixture` and do **not** rename to
  `BUILD.bazel`; the ones that do not set `[buildfile] name` use either
  `BUCK` or `BUILD.bazel` per the fixture's own choice.
- 1 `.bazelrc` (in an unrelated example).

Kuro is positioning itself as a Bazel-compatible build system but its own
bootstrap is still Buck-shaped. This plan converts the source tree to
Bazel-native conventions:

- `BUCK` → `BUILD.bazel` **in active workspaces only** (test fixtures keep
  their `TARGETS.fixture` / `BUCK` until the test that owns them migrates
  or is deleted in Phase 35.6).
- Active-workspace `.buckconfig` → some combination of `MODULE.bazel`
  (cells), `.bazelrc` (flags), `.bazelignore` (ignored paths), or
  hardcoded Rust defaults.
- Test-fixture `.buckconfig` files are handled in Phase 35.6 as a
  classify-then-batch operation: many will be **deleted along with the
  test that owns them** (because the test exercises legacy
  `.buckconfig`-only behaviour we're retiring); the rest get a scripted
  conversion to `MODULE.bazel`.
- The `legacy_configs` parser in `app/kuro_common/src/legacy_configs/`
  becomes dead code and is deleted.

**No `.kurorc`**, see "Knob disposition" below for the analysis.

### Active workspace vs test fixture (critical scoping decision)

Earlier drafts of this plan undercounted `.buckconfig` files by ~20× because
they ignored test fixtures. The fix is **not** to migrate every fixture in
the early phases — most of the fixtures whose tests exercise legacy buckconfig
parsing (`test_audit_config`, `test_select_buckconfig`,
`test_external_buckconfigs`, `test_deprecated_config`,
`test_read_root_config`, `test_target_aliases`, etc.) are scheduled to be
deleted alongside the parser. Rewriting their fixtures into `MODULE.bazel`
form would be wasted work.

Phases 35.1–35.5 therefore touch **active workspaces only**. Phase 35.6
classifies test fixtures in batch and disposes of them (delete or
auto-migrate) immediately before deleting the parser.

## Why This Exists

Three reasons the conversion has been deferred until now:

1. The `legacy_configs` parser carries Kuro-specific knobs (cell registration,
   bundled cells, parser settings) that have no off-the-shelf Bazel
   equivalent. Plan 02's bzlmod work, Plan 28's bundled cells, and the
   existing `.bazelrc` parser at `app/kuro_client_ctx/src/bazelrc.rs`
   eliminated most of those gaps; the residual knobs that don't fit anywhere
   else can become CLI flags read from `.bazelrc`.
2. Prior plans treated `BUCK` as the bootstrap convention. The build-file
   discovery code at `app/kuro_common/src/buildfiles.rs:26` already defaults
   to `["BUILD.bazel", "BUILD"]`; only an explicit `[buildfile] name = BUCK`
   opt-in keeps `BUCK` working in kuro's own tree.
3. There was no MODULE.bazel at the kuro repo root, so cell registration had
   to live in `.buckconfig`. Most other workspaces (LLVM, examples) have
   already migrated to MODULE.bazel.

## Knob Audit (current state)

### Sections used across all `.buckconfig` files in the repo

| Section | Knobs in use | Where the data should live |
|---|---|---|
| `[cells]` / `[repositories]` | cell-name → directory map | `MODULE.bazel` via `bazel_dep` + `local_path_override` |
| `[cell_aliases]` / `[repository_aliases]` | alias → cell-name | `MODULE.bazel` via `repo_name` on `bazel_dep` (Plan 02 ships this) |
| `[external_cells]` | `prelude = bundled`, etc. | drop; `kuro_external_cells_bundled` Rust crate auto-registers them |
| `[buildfile]` | `name`, `name_v2`, `extra_for_test`, `includes`, `package_includes` | drop `name`/`name_v2` (default to `BUILD.bazel`); `extra_for_test` survives as a `.kurorc`-style escape valve only if needed |
| `[parser]` | `target_platform_detector_spec` | `.bazelrc` flag (`build --platforms=...`); auto-registered `@local_config_platform//:host` covers the host case |
| `[project]` | `ignore`, `package_boundary_exceptions`, `watchman_merge_base` | `.bazelignore` for `ignore`; the others as `.bazelrc` flags |
| `[oss]` | `internal_cell`, `stripped_root_dirs` | drop (Meta-internal Buck1 vestige; `is_open_source()` already handles this in Rust) |
| `[rust]` | `default_edition` | drop (kuro's own bootstrap config; not a kuro build-system concern) |
| `[build]` | `execution_platforms`, `threads`, `lazy_cycle_detector`, RE knobs | `.bazelrc` flags |
| `[client]` | `id` | `.bazelrc` flag |
| `[log]`, `[sandbox]`, `[test]`, `[ui]`, `[http]` | runtime knobs | `.bazelrc` flags |
| `[kuro]` | ~50 daemon-tuning keys | `.bazelrc` flags with `--kuro_*` prefix where they aren't already named that way |
| `[kuro_re_client]` | RE backend address, TLS, headers | `.bazelrc` flags |
| `[kuro_resource_control]` | cgroup/memory limits | `.bazelrc` flags |
| `[kuro_health_check]`, `[kuro_system_warning]` | diagnostics thresholds | `.bazelrc` flags |
| `[build_report]` | (no consumers in audit; likely dead) | drop |

### Build files

In **active workspaces** there are ~336 `BUCK` files and ~521 `BUILD.bazel`
files (the `BUILD.bazel` count is large because vendored Cargo deps under
`shim/third-party/` already use `BUILD.bazel`). The 336 `BUCK` files only
resolve because kuro's root `.buckconfig` opts in via
`[buildfile] name = BUCK` (and `shim/.buckconfig` adds `BUCK.reindeer,BUCK`
for vendored Cargo deps). In Phase 35.2 every active-workspace directory
with a `BUCK` gets a `git mv` to `BUILD.bazel`, **except** the
`BUCK.reindeer` files (which stay — reindeer hardcodes the name) and the
directories where `BUILD.bazel` already exists (then the `BUCK` file is
either dead or the two get merged). The buildfile-discovery code already
lists `BUILD.bazel` first, so this is a no-op behavior change post-rename.

In **test fixtures**, build files are usually named `TARGETS.fixture` (set
explicitly by the fixture's own `.buckconfig`). They do **not** get
renamed in Phase 35.2; their fate is decided in Phase 35.6 alongside the
fixture's `.buckconfig`.

## Why no `.kurorc`

Considered + rejected. A parallel `.kurorc` was the obvious candidate for
Kuro-only knobs that don't map to `.bazelrc`'s flag-form shape. But:

1. **Every surviving knob is flag-shaped.** The audit found no `.buckconfig`
   key that needs structured config (nested objects, lists with semantic
   ordering). They all reduce to `key=value` pairs that fit on a `.bazelrc`
   line.
2. **`.bazelrc` already supports profiles.** `--config=foo` lets users gate
   blocks of flags per build mode. We don't need a second selector mechanism.
3. **One file, one parser, one source of truth.** A separate `.kurorc` would
   need its own parser, its own DICE invalidation, its own docs, its own
   precedence rules vs `.bazelrc`. Two files of essentially the same content
   is worse than one.
4. **The `--kuro_*` prefix is a sufficient namespace.** Bazel-compatible
   flags pass through to whatever Kuro accepts; Kuro-specific flags are
   visibly distinct.

If a future Kuro feature genuinely needs structured config (unlikely for
build-tool knobs), introduce `.kurorc` then. Today: `.bazelrc` only.

## Phases

### Phase 35.1: Audit + decision freeze  [~1 day]  ✅ COMPLETE 2026-05-01

**Output**: [thoughts/shared/research/2026-05-01-buckconfig-knob-disposition.md](../../research/2026-05-01-buckconfig-knob-disposition.md)


#### Goal

Lock down the per-knob disposition table for **active workspaces**. No code
changes. Test fixtures are explicitly deferred to Phase 35.6.

#### Scope boundary

In scope: `./.buckconfig`, `./prelude/.buckconfig`, `./shim/.buckconfig`,
`./bazel_tools/.buckconfig`, `./tests/manual_test/.buckconfig`,
`./examples/**/.buckconfig` (~28 files).

Out of scope: every `tests/{core,e2e}/**/test_*_data/.buckconfig` (~290
files). Those are inputs to integration tests; they're inventoried by
`grep` in this phase but not classified key-by-key.

#### Work

1. Walk every active-workspace `.buckconfig` (per the scope boundary
   above). Produce a per-key spreadsheet: `(file, section, key) → disposition`.
2. For each key, classify:
   - **MODULE.bazel** (cells / aliases / external cells)
   - **`.bazelrc`** (CLI flag form)
   - **`.bazelignore`** (ignored path)
   - **drop** (no consumers, or Meta-internal vestige, or already covered
     by a Rust default)
   - **defer** (needs design — flag this as a sub-question)
3. Cross-reference with the Rust `BuckconfigKeyRef` consumers in
   `app/kuro_common/src/legacy_configs/` and elsewhere. Any key with no
   Rust consumer goes straight to **drop**.
4. For knobs migrating to `.bazelrc`, decide flag names. Bazel-compatible
   names where possible (`--platforms`, `--remote_executor`,
   `--watchfs_*`); Kuro-prefixed otherwise (`--kuro_starlark_max_callstack_size`).
5. Build a **test-fixture inventory** (filename + size + first-line comment
   only — not key-by-key). This list seeds Phase 35.6's classification.
6. Document the per-knob landing in
   `thoughts/shared/research/2026-MM-DD-buckconfig-knob-disposition.md`,
   with a separate appendix for the fixture inventory.

#### Acceptance

- Every knob in every active `.buckconfig` has a documented destination.
- Every section has either a single migration target or an explicit "drop
  with rationale" entry.
- The list of `.bazelrc` flags Kuro must accept is finalized; cross-checked
  against the existing `app/kuro_client_ctx/src/bazelrc.rs` parser.
- Test-fixture inventory exists as an appendix; no per-key classification
  yet.

### Phase 35.2: `BUCK` → `BUILD.bazel` rename (active workspaces)  [~1 day]  ✅ COMPLETE 2026-05-01

**Notes**:
- Renamed 483 active-workspace `BUCK` files to `BUILD.bazel`. Deleted 2
  collision files at `examples/hello_world/{BUCK,toolchains/BUCK}` (both
  Buck1-style; the existing `BUILD.bazel` siblings are the bazel-compat
  versions).
- 0 `BUCK.reindeer` files exist in tree; `shim/.buckconfig`
  `[buildfile] name` updated `BUCK.reindeer,BUCK` → `BUCK.reindeer,BUILD.bazel`.
- Root `./.buckconfig` already had no `[buildfile]` section in the OSS
  branch; step 4 was a no-op.
- 0 `"BUCK"` string literals in `app/` outside doctest/unit-test fixture
  data. Test-fixture data strings (`"cell/pkg/BUCK"`) left alone — they're
  synthetic paths in serializer unit tests, not real-file references.
- Regression test added at `app/kuro_common/src/buildfiles.rs::tests::test_buildfiles`:
  explicit `assert!(!default.contains("BUCK"))`.


#### Goal

Drop the Buck-shaped naming opt-in from kuro's active source tree. Test
fixtures keep their fixture-specific buildfile names; they're handled in
35.6.

#### Scope boundary

Rename in: `./BUCK`, `app/**/BUCK`, `dice/**/BUCK`, `gazebo/**/BUCK`,
`shed/**/BUCK`, `starlark-rust/**/BUCK`, `superconsole/**/BUCK`,
`remote_execution/**/BUCK`, `host_sharing/**/BUCK`, `pagable/**/BUCK`,
`pagable_derive/**/BUCK`, `app_dep_graph_rules/**/BUCK`,
`prelude/**/BUCK`, `shim/**/BUCK` (excluding `BUCK.reindeer`),
`bazel_tools/**/BUCK`, `examples/**/BUCK`,
**plus the `BUCK` files under `tests/core/` and `tests/e2e/` that define
test-target rules** (not the fixture `BUCK` files inside `*_data/`
directories — those use `TARGETS.fixture` per their fixture config).

Do **not** rename:
- `BUCK.reindeer` (reindeer hardcodes the name)
- Anything inside `tests/**/test_*_data/` or `tests/**/test_*_inputs/`
  (test-fixture build files, handled in 35.6)
- Anything inside `buck-out/` (build outputs)

#### Work

1. Codemod: `git mv` every in-scope `BUCK` to a sibling `BUILD.bazel` in
   directories where `BUILD.bazel` does not already exist.
2. For directories that have *both* `BUCK` and `BUILD.bazel`: investigate;
   typically one is dead. Merge or delete as appropriate.
3. Update any explicit references in source: `grep -rn '"BUCK"' app/` for
   string literals (error messages, completion strings in
   `kuro_cmd_completion_client/src/complete/package.rs`). Test fixtures
   that reference `"BUCK"` are out of scope.
4. Drop `[buildfile] name = BUCK` from `./.buckconfig`. Leave the file
   present (other sections still in use); just the `[buildfile]` section
   goes.
5. Verify each cell still resolves: `cargo build -p kuro`,
   `python -m pytest tests/core/analysis/test_native_rules.py`, LLVM
   Demangle smoke. Build files inside cells (`shim/`, `prelude/`,
   `bazel_tools/`) inherit each cell's own `[buildfile] name` setting —
   migrate those too if they opt out of the default.
6. Add a regression test that `BUCK` is NOT in the default
   `parse_buildfile_name` output (already true in `buildfiles.rs:26`,
   make it explicit).

#### Acceptance

- `find . -name BUCK -not -name 'BUCK.reindeer' \( ! -path '*/test_*_data/*' \) \( ! -path '*/test_*_inputs/*' \) \( ! -path '*/buck-out/*' \) | wc -l` → 0.
- `cargo build -p kuro` clean. Analysis suite at the same baseline as
  pre-rename. LLVM Demangle clean.
- `./.buckconfig` no longer has a `[buildfile]` section.
- Test-fixture builds still work (fixture-internal `[buildfile] name`
  settings unaffected).

### Phase 35.3: Cells → MODULE.bazel (active workspaces)  [~1-2 days]  ⚠️ PARTIAL 2026-05-01 — root + shim + most examples deferred to 35.6a

**Done**:
- `examples/bzlmod_local_test/.buckconfig`: stripped `[cells]`,
  `[cell_aliases]` (all dead — only `@local_lib//` is actually used,
  declared via `bazel_dep` in `MODULE.bazel`). `[parser]` retained for
  35.5. Verified `kuro build //...` still passes.
- `examples/hello_world/.buckconfig`, `tests/manual_test/.buckconfig`,
  `examples/multi_package/` — already at target state (comment-only or
  no `.buckconfig`).

**Deferred to 35.6a** (see "Scope reality" subsection above):
- Root `.buckconfig` + `.buckconfig.d/common.buckconfig` (multi-aliasing
  with N→1 mapping; needs design decision).
- `shim/.buckconfig` (depends on root resolution).
- `examples/{android/demoapp,bootstrap,bxl_tutorial,no_prelude,persistent_worker,vscode,with_prelude*}` and all `examples/{remote_execution,toolchains}/*` (most non-functional today; classify per 35.6a buckets).


#### Goal

Move every `[cells]` / `[cell_aliases]` / `[external_cells]` declaration into
the corresponding active workspace's `MODULE.bazel`. Drop those sections
from every active `.buckconfig`. Test fixtures unchanged (handled in 35.6).

#### Scope reality (discovered during execution, 2026-05-01)

Two complications surfaced when executing this phase that the plan did not
anticipate:

1. **Multi-aliasing in root `.buckconfig.d/common.buckconfig`**: 8+ cell
   aliases (`bazel_skylib`, `bazel_features`, `rules_cc`, `buck`, `fbcode`,
   `fbcode_macros`, `fbsource`, `shim`, `toolchains`) all point at the same
   `gh_facebook_kuro_shims_meta` cell. `bazel_dep` is keyed by `name`, so
   each `repo_name` alias requires its own `bazel_dep` entry — N→1 aliasing
   fundamentally doesn't fit MODULE.bazel cleanly. Worse, `cells.rs:563`
   says when a root `MODULE.bazel` exists, `.buckconfig [cell_aliases]` are
   **skipped entirely** — so partial migration breaks aliases that haven't
   moved yet. The aliases are mostly defensive fallbacks for Meta-internal
   `.bzl` files in `shim/`, `prelude/`, `app/modifier.bzl`,
   `app/kuro/transition.bzl` — none of which are loaded by OSS users — but
   removing them at the same instant we add MODULE.bazel is a coordination
   problem.
2. **Stale examples**: Most `examples/*` workspaces are demo-only and
   non-functional today (e.g. `examples/with_prelude` errors on
   `prebuilt_cxx_library` after Phase 7d's prelude cleanup, before this
   phase started). Migrating their `.buckconfig` to `MODULE.bazel` form
   without validation is wasted effort if the example will be deleted or
   rewritten anyway.

#### Revised scope

Phase 35.3 now covers only the **tractable, validated** cases:

- Workspaces that already have `MODULE.bazel` and just need their
  `.buckconfig` sections stripped: `examples/bzlmod_local_test/`
  (only one matching this pattern).
- Workspaces whose `.buckconfig` is already comment-only:
  `examples/hello_world/` (already done), `tests/manual_test/`,
  `examples/multi_package/` (already no `.buckconfig`).

**Deferred to Phase 35.6 (now extended to cover examples in addition to
test fixtures)**:

- Root `.buckconfig` + `.buckconfig.d/common.buckconfig` cells/aliases.
  Resolution path: either (a) build out per-aliased-name `bazel_dep` entries
  in root `MODULE.bazel` pointing at the shim cell or real registered
  modules, (b) drop the Meta-internal aliases entirely (since OSS users
  don't load the `.bzl` files that need them), or (c) extend `kuro_bzlmod`
  to support N→1 alias declarations natively.
- `shim/MODULE.bazel`: depends on root resolution.
- All `examples/*/.buckconfig` files with `[cells]` / `[cell_aliases]`:
  classify each example in Phase 35.6a (extended) — bucket (A) delete-stale,
  (B) migrate-and-validate, (C) keep-as-vestigial-demo. CI exercises only
  `examples/with_prelude` (and that's broken on `main` independently of
  this plan).

#### Work

1. For workspaces that don't yet have a `MODULE.bazel`: create one. The kuro
   root needs `module(name = "root")` plus `bazel_dep`s for any directly
   loaded cells (`shim`, `bazel_tools`). `prelude` is bundled
   (Plan 28); no MODULE entry needed for it.
2. For workspaces that already have a `MODULE.bazel`: ensure their cell
   registrations cover everything the `.buckconfig` lists. Use
   `local_path_override` for sibling-path cells.
3. Translate `[cell_aliases]` → `bazel_dep(... repo_name = "alias")`
   (Plan 02 already implements this resolution in
   `kuro_common::legacy_configs::cells`).
4. `[external_cells] X = bundled` becomes a no-op: Plan 28's
   `kuro_external_cells_bundled` Rust crate auto-registers the bundled
   cells.
5. Delete the migrated sections from every `.buckconfig`. Several files
   (e.g. `examples/hello_world/.buckconfig`,
   `tests/manual_test/.buckconfig`) already have a comment "Cell
   definitions are derived from MODULE.bazel" — those just lose their
   `.buckconfig` entirely if no other knob remains.

#### Acceptance

- No active-workspace `.buckconfig` (root, `prelude/`, `shim/`,
  `bazel_tools/`, `tests/manual_test/`, `examples/**`) contains
  `[cells]`, `[repositories]`, `[cell_aliases]`, `[repository_aliases]`,
  or `[external_cells]`. (Test-fixture buckconfigs are still allowed to
  carry these — they're handled in 35.6.)
- `cargo build -p kuro` clean. Test suite + LLVM clean.
- `kuro audit cell` (or equivalent) reports the same cell topology
  pre/post-migration for each active workspace.

### Phase 35.4: `.bazelignore` adoption (active workspaces)  [~½ day]  ✅ COMPLETE 2026-05-01

**Notes**:
- New module `kuro_common::ignores::bazelignore` parses
  `.bazelignore` (one path per line, `#` comment, blank skipped) into the
  comma-separated spec `IgnoreSet::from_ignore_spec` already consumes.
  3 unit tests cover the format.
- Interpreter ignores wired in `app/kuro_common/src/ignores/all_cells.rs`:
  `.bazelignore` (read via `DiceFileComputations::read_file_if_exists`)
  takes precedence; falls back to `[project] ignore` during the
  deprecation window.
- File-watcher ignores wired in `app/kuro_server/src/daemon/state.rs`:
  same precedence, reads via `kuro_fs::fs_util::read_to_string_if_exists`
  at daemon startup.
- Root `./.bazelignore` created with the 4 paths previously listed in
  `[project] ignore`. `[project]` section removed from `./.buckconfig`.
- Verified no active-workspace `.buckconfig` contains `[project]`,
  `package_boundary_exceptions`, or `watchman_merge_base`.


#### Goal

Move `[project] ignore` to `.bazelignore` in active workspaces. The other
`[project]` keys (`package_boundary_exceptions`, `watchman_merge_base`)
become `.bazelrc` flags or get dropped. Test fixtures unaffected.

#### Work

1. Add `.bazelignore` parser to `kuro_common`. Bazel's syntax is one path
   per line, `#` for comments, project-relative paths.
2. Wire the parser into the file-watcher / directory-walk paths that
   currently consume `[project] ignore`.
3. Migrate kuro's root `.buckconfig` ignore list (`app/kuro_explain`,
   `app_dep_graph_rules`, `examples`, `integrations/rust-project/tests`)
   to a new `kuro/.bazelignore`.
4. Drop `[project] ignore` from every `.buckconfig`.
5. Decide `package_boundary_exceptions` and `watchman_merge_base`: both
   become `.bazelrc` flags or get dropped if nobody uses them.

#### Acceptance

- `./.bazelignore` exists and the file watcher honours it.
- No active-workspace `.buckconfig` has a `[project]` section. (Test
  fixtures may still — handled in 35.6.)

### Phase 35.5: Runtime knobs → `.bazelrc` (active workspaces)  [~2-3 days]

#### Goal (revised 2026-05-01)

**Directive**: every Buck2-era flag not actively set in this repo's
`.buckconfig` files is dead code and gets deleted; flags that have a
Bazel equivalent get renamed to the Bazel name; flags actively used in
this repo with no Bazel equivalent get a `--kuro_*` flag.

This dramatically narrows the original scope. Most of the 194 distinct
`(section, property)` pairs in `BuckconfigKeyRef` consumer code are
Buck2-internal knobs that no `.buckconfig` in this repo sets. They go
away wholesale, simplifying the daemon-side wiring.

#### Disposition rule per consumer

For each `BuckconfigKeyRef { section, property }` call site:

1. **Delete** if no active-workspace `.buckconfig` in the kuro repo sets
   this `(section, property)`. The consumer code path (and its default
   value) is dead. Either remove the lookup entirely (returning the
   compile-time default) or delete the surrounding feature.
2. **Rename to Bazel-compat flag** if the key *is* set in an active
   `.buckconfig` AND Bazel has a similar flag (e.g.
   `kuro_re_client.engine_address` → `--remote_executor`,
   `kuro.digest_algorithms` → `--digest_function`, etc.).
3. **`--kuro_*` flag** only if the key is set AND there is no Bazel
   equivalent. Expected to be a small minority.

#### Drain plan (per consumer file)

Process consumers in descending key-count order, deleting dead lookups
and renaming live ones:

1. `app/kuro_re_configuration/src/lib.rs` (~80 keys, all
   `[kuro_re_client]`) — most are Meta-internal RE knobs with no live
   `.buckconfig` source. Delete the dead ones; rename live ones
   (`engine_address`, `action_cache_address`, `cas_address`, `tls`,
   `tls_client_cert`, `tls_ca_certs`, `instance_name`, `http_headers`,
   `capabilities`, `use_fbcode_metadata`) to Bazel-compat CLI flags.
2. `app/kuro_server/src/ctx.rs` — `[kuro]`, `[build]`, `[ui]`, `[scuba]`,
   `[log]`, `[client]` keys.
3. `app/kuro_server/src/daemon/state.rs` — `[kuro]`, `[build]`,
   `[project]` (already covered by 35.4).
4. `app/kuro_common/src/init.rs` — `[http]`, `[kuro_system_warning]`.
5. `app/kuro_file_watcher/src/{file_watcher,watchman/interface,edenfs/interface}.rs` —
   `[kuro]`, `[project]` (.bazelignore-covered).
6. Smaller consumers: `kuro_test/src/command.rs` (`[test]`),
   `kuro_interpreter/src/{factory,allow_relative_paths,import_paths}.rs`,
   `kuro_interpreter_for_build/src/interpreter/{buckconfig,interpreter_for_dir}.rs`,
   `kuro_build_api/src/{configure_dice,materialize,build/build_report,artifact_groups/calculation}.rs`,
   `kuro_execute_impl/src/materializers/deferred/clean_stale.rs`,
   `kuro_configured/src/{nodes,target_platform_resolution}.rs`,
   `kuro_node/src/execution.rs`,
   `kuro_server_commands/src/build.rs`.

#### Acceptance (revised)

- Every `BuckconfigKeyRef` call site outside `legacy_configs/` is
  classified: deleted (key was dead) or renamed to a Bazel-compat CLI
  flag, or kept as `--kuro_*` (rare).
- Every `.bazelrc` flag introduced/renamed has a help string and a
  documented default.
- No active-workspace `.buckconfig` contains any `[kuro*]`, `[build]`,
  `[parser]`, `[log]`, `[sandbox]`, `[test]`, `[ui]`, `[http]`, or
  `[client]` section. (Test fixtures may still — handled in 35.6.)
- Behaviour is preserved for the renamed-flag knobs: targeted regression
  for `digest_algorithms` (→ `--digest_function`), `execution_platforms`
  (→ `--platforms`), and the live RE-client knobs.

### Phase 35.6a: Test fixture + stale example classification + disposition  [~2-3 days]

> **Scope extended 2026-05-01**: Phase 35.6a now classifies stale `examples/*`
> workspaces in addition to test fixtures (`tests/{core,e2e}/**/test_*_data/`).
> The classification rules below apply to both. Most examples are non-functional
> demos as of 2026-05-01 (e.g. `examples/with_prelude` is broken on `main`
> from a prior phase's prelude cleanup); they need a delete-or-migrate
> decision per-workspace. CI gates only `examples/with_prelude`, so the
> validation surface for "migrate" is small.

#### Goal

Decide and execute the fate of every `tests/{core,e2e}/**/test_*_data/.buckconfig`
fixture. This phase exists to **avoid wasted work**: many of these tests
exercise legacy `.buckconfig` parsing precisely because that's what they
test, and they will be deleted along with the parser. Rewriting their
fixtures into `MODULE.bazel` form before deletion is pure churn.

#### Classification rules

For each fixture `.buckconfig`, classify into one of three buckets:

- **(A) Delete with test.** The owning test exercises legacy
  `.buckconfig`-only behaviour we're retiring. Examples (non-exhaustive,
  finalize via grep in step 1):
  - `test_audit_config*` (audits `.buckconfig` content)
  - `test_select_buckconfig` (`select()` on buckconfig values)
  - `test_external_buckconfigs` (external buckconfig file loading)
  - `test_deprecated_config` (`[deprecated_config]` section)
  - `test_read_root_config` (Starlark `read_root_config()`)
  - `test_target_aliases` (`[alias]` section)
  - `test_callstack_size`, `test_peak_allocated_bytes`,
    `test_unhashed_outputs`, etc. — anything that *only* sets a knob via
    `.buckconfig` and verifies the resulting daemon behaviour, where the
    knob has moved to `.bazelrc` in 35.5.
  - Any test already in `collect_ignore` of `tests/conftest.py` (already
    skipped — easy targets for deletion).
- **(B) Migrate to MODULE.bazel.** The fixture's `.buckconfig` is
  effectively just `[cells] root = .` (+ maybe `[external_cells] prelude =
  bundled`, `[buildfile] name = TARGETS.fixture`). The test exercises
  something orthogonal to legacy buckconfig parsing. Bulk of fixtures
  fall here.
- **(C) Keep as-is, defer.** Edge cases: fixture exercises a knob that
  hasn't moved yet, or a knob whose `.bazelrc` equivalent is still in
  flight, or a knob the team wants to re-evaluate. Each (C) entry must
  have a one-line rationale in the disposition appendix and a follow-up
  TODO.

#### Work

1. Build the fixture inventory from Phase 35.1's appendix into a
   classification table. For each fixture: bucket + rationale (one line).
   Iterate with the team if uncertain.
2. **Bucket (A) — delete.** `git rm -r tests/.../test_FOO_data/` and
   delete the corresponding `tests/.../test_FOO.py` (or delete just the
   relevant test functions if the file covers multiple concerns). Update
   `tests/conftest.py::collect_ignore` to remove now-stale entries.
3. **Bucket (B) — migrate via script.** Write a one-shot migration
   script that, given a fixture-style `.buckconfig`, emits a
   `MODULE.bazel` capturing the cells + bundled-cell registrations and
   deletes the `.buckconfig`. The script must:
   - Translate `[cells] X = path` → `bazel_dep(name = "X")` +
     `local_path_override(module_name = "X", path = "path")` (or
     `module(name = "root")` for `root = .`).
   - Translate `[cell_aliases] X = Y` → `bazel_dep(... repo_name = "X")`
     on Y.
   - Translate `[external_cells] X = bundled` → drop (auto-registered).
   - Preserve `[buildfile] name = TARGETS.fixture` by emitting a
     fixture-local `.bazelrc` with `--kuro_buildfile_name=TARGETS.fixture`
     (or whatever flag 35.5 introduces) — **xor** keep the
     `.buckconfig` with only the `[buildfile]` section if the flag isn't
     ready. Pick one approach for the whole bucket; document it.
   - Refuse to run on any fixture not in bucket (B) (require explicit
     allowlist).
4. **Bucket (C) — annotate.** Leave the fixture; add a comment at the
   top of its `.buckconfig` pointing at the disposition appendix entry.
5. Run the full test suite. Triage failures fixture-by-fixture; either
   the migration script missed something (fix script, re-run) or the
   test moved unexpectedly into bucket (A).

#### Acceptance

- Every `tests/{core,e2e}/**/test_*_data/.buckconfig` is in bucket (A),
  (B), or (C) per the appendix.
- Bucket (A) test files and fixtures are deleted; pytest still green
  (modulo (C) deferrals, which are documented xfails or deletions).
- Bucket (B) fixtures have `MODULE.bazel` (and at most a stub
  `.buckconfig` for `[buildfile] name`).
- Bucket (C) is non-empty only with explicit per-entry rationale.

### Phase 35.6b: Delete the legacy config parser  [~2 days]

#### Goal

Remove `app/kuro_common/src/legacy_configs/` once nothing reads from
`.buckconfig` anymore.

#### Work

1. Confirm no live consumer remains. Should be zero `BuckconfigKeyRef`
   call sites outside the parser itself by this phase. Bucket (C)
   fixtures from 35.6a may still need *some* parsing — if so, narrow the
   parser to only those keys before deleting the rest, or escalate (C)
   entries back to (A)/(B).
2. Delete `legacy_configs/` (parser, cells.rs, dice integration, tests).
   Several thousand LOC.
3. Replace the `kuro_common::legacy_configs::cells` cell-resolution path
   with a MODULE.bazel-only path. The bzlmod-based resolver in Plan 02
   already handles the common case; this phase makes it the only path.
4. Delete `./.buckconfig`, `./bazel_tools/.buckconfig`,
   `./shim/.buckconfig`, `./prelude/.buckconfig`, and every active-
   workspace `.buckconfig` that's now empty (test fixtures already
   handled in 35.6a).
5. Add a startup warning: if the daemon finds a `.buckconfig` in the
   workspace root, log a deprecation pointer to migration docs.

#### Acceptance

- `legacy_configs/` directory gone.
- No `.buckconfig` files in the kuro source tree (modulo bucket-(C)
  fixtures, which must each justify their existence in the appendix).
- `cargo build -p kuro` clean. Full pytest + LLVM smoke clean.
- The deprecation warning fires for any user workspace that still ships
  a `.buckconfig`.

### Phase 35.7: Migration docs + kuro init templates  [~½ day]

#### Goal

Update everything that points users at `.buckconfig` / `BUCK`.

#### Work

1. Update the `kuro init` templates in
   `app/kuro_client/src/commands/init.rs` to scaffold `MODULE.bazel`,
   `BUILD.bazel`, `.bazelrc`, `.bazelignore` instead of `.buckconfig` +
   `BUCK`. (The earlier Plan 28 follow-up #3 already deleted the dead
   `initialize_toolchains_build` path; this phase finishes the
   scaffolding.)
2. Migration guide: a single doc in `thoughts/shared/docs/` (or the
   kuro-build user docs) explaining the per-knob mapping. Includes an
   example "before/after" diff.
3. Update Plan 04 (Prelude Architecture), Plan 28 (Builtins), and the
   main 2026-01-21 plan to cite Plan 35 as the BUCK/buckconfig retirement.
4. Cross-check `examples/*` are migrated to the new layout (was already
   covered in 35.3 / 35.5 but verify here).

#### Acceptance

- `kuro init` produces a Bazel-shaped scaffold by default.
- Migration doc covers every knob in the Phase 35.1 audit.
- No remaining cross-references to `.buckconfig` or `BUCK` in
  user-facing documentation.

## Sequencing

Phases 35.1 (audit) and 35.2 (active-workspace BUCK rename) are independent
and ship first. Phases 35.3-35.5 are parallelizable post-35.1, each draining
one major block of the parser surface in **active workspaces only**. Phase
35.6a (test fixture classification) depends on 35.5 because (B)-bucket
fixtures may need the new `.bazelrc` flags introduced in 35.5. Phase 35.6b
(delete parser) depends on 35.6a. Phase 35.7 is documentation cleanup.

```
35.1 audit ──┐
             ├── 35.3 cells → MODULE.bazel (active) ──┐
35.2 BUCK ──┘                                          │
                                                       ├── 35.6a fixtures ── 35.6b parser ── 35.7 docs
35.1 audit ── 35.4 .bazelignore (active) ──────────────┤
35.1 audit ── 35.5 runtime → .bazelrc (active) ────────┘
```

35.3 / 35.4 / 35.5 can run in parallel after 35.1 lands; each touches a
disjoint section of `.buckconfig`. 35.6a is the funnel point where the
~290 test fixtures are processed in batch immediately before parser
deletion.

## Dependencies

- **Plan 02 (bzlmod)**: cell-alias resolution via `bazel_dep(repo_name=...)`
  must be working. Already in.
- **Plan 09 §9d (cell-section opt-out)**: this plan picks up where 9d
  stopped. 9d made `[cells]` / `[cell_aliases]` / `[external_cells]`
  optional when `MODULE.bazel` exists; the parser still reads them when
  present. Plan 35 finishes the cleanup by deleting the parser entries +
  the `.buckconfig` files themselves.
- **Plan 09 §9e (configuration migration)**: 9e proposed moving
  build-config from `.buckconfig` to `.bazelrc`/`MODULE.bazel`. Plan 35.5
  is the concrete execution of that proposal.
- **Plan 28 (Builtins)**: bundled-cell registration via
  `kuro_external_cells_bundled` must auto-register the cells that
  `[external_cells] = bundled` lists today. Done in Plan 28.
- **`.bazelrc` parser**: already shipped at
  `app/kuro_client_ctx/src/bazelrc.rs` (1122 LOC, full Bazel-compatible
  syntax including `import` / `try-import` / `--config=` profiles). The
  flags this plan introduces hook into the existing parser; no new
  infrastructure.

## Risks

- **User workspaces still on `.buckconfig`**: external projects that adopted
  Kuro before this migration. Mitigation: keep parsing for one release with
  the deprecation warning from Phase 35.6b, then delete in the release after.
- **Buried `.buckconfig` consumers**: a knob the audit misses, ripped out in
  35.6b, breaks a corner-case workflow. Mitigation: Phase 35.6b's
  "no consumers remain" gate verifies via `grep`; CI green is the second
  check.
- **`.bazelrc` parser limits**: existing parser at `bazelrc.rs` covers
  Bazel's spec; verify it handles every flag shape we want to introduce
  (especially flags with structured values like comma-separated lists).
- **Test fixture misclassification (high impact, high likelihood)**: a test
  put in bucket (B) and migrated turns out to actually depend on legacy
  parsing — wasted migration work plus a test failure to debug. Or, a
  test put in bucket (A) and deleted turns out to be load-bearing.
  Mitigation: 35.6a runs the full test suite per bucket migration batch,
  not per fixture; mismatches surface immediately. Bucket (A) deletions
  go through code review with the per-test rationale visible in the
  diff.
- **Migration script bugs**: bucket (B) is ~200 fixtures via script; one
  script bug breaks them all. Mitigation: idempotent script, run
  in batches of ~20 with test verification between batches; keep
  `git`-revertable commits per batch.
- **Tests that explicitly construct `LegacyBuckConfig` in Rust**
  (`app/kuro_interpreter_for_build_tests/` and similar): these aren't
  fixture-based — they're inline Rust unit tests. Treat as a fourth
  bucket: rewrite to use `MODULE.bazel` test inputs, or delete with
  35.6b. Inventory in 35.1.
- **Cycle hazard**: 35.5 introduces `.bazelrc` flags; 35.6a's bucket (B)
  migration may need flags 35.5 didn't ship. Mitigation: 35.6a starts
  with a dry-run pass to enumerate required flags, feeds the gap back to
  35.5 if needed.

## Verification

For each phase:

- `cargo build -p kuro` clean.
- `python -m pytest tests/core/analysis/test_native_rules.py
  --deselect tests/core/analysis/test_native_rules.py::test_sh_test_runs
  -q` at or above the pre-phase baseline (currently 122 pass + 5
  pre-existing failures + 3 skip + 1 deselect).
- Full `tests/core/` baseline: 861 pass, 152 skip, 1 xfail (per
  `memory/MEMORY.md`, 2026-03-06). Phases 35.1–35.5 must keep this
  baseline; Phase 35.6a will reduce both pass and skip counts as bucket
  (A) tests are deleted (record the new baseline in the phase commit
  message).
- LLVM Demangle smoke clean (`cd /var/mnt/dev/llvm-project/utils/bazel
  && /var/mnt/dev/kuro/kuro build --config=generic_gcc --remote_executor=
  @llvm-project//llvm:Demangle`). Demangle is the lightweight smoke; LLVM
  Support (~183 actions) is the heavier check at the end of Phase 35.5
  and 35.6b.

## Follow-ups (tracked TODOs)

- [x] **Delete `use_fbcode_metadata` and all Meta-internal-only code paths**
      (added 2026-05-01, completed 2026-05-04). Folded into 35.5
      `kuro_re_configuration` sweep: deleted the field + parse, the
      `with_re_metadata` fbcode branch in `re_grpc/client.rs`, and the
      `examples/remote_execution/internal/` example.
      Remaining audit deferred (low priority): `#[cfg(fbcode_build)]`
      blocks in `app/kuro_execute/src/re/client.rs` and similar — most
      are gated at the workspace level by `is_open_source()` and don't
      compile in OSS today.

## Out of Scope

- Renaming `kuro` itself: this plan keeps the tool name and its CLI shape;
  what changes is the per-workspace config format. Repo-wide renames are
  separate.
- Migrating users' workspaces: this plan only converts the kuro source tree
  (active workspaces + test fixtures we keep). External user workspaces get
  a migration guide (Phase 35.7) but are responsible for their own
  conversions during the deprecation window.
- Test fixtures classified as bucket (C) "keep, defer": each one carries a
  per-entry rationale and a follow-up TODO; the follow-ups are tracked
  outside this plan once 35.6b ships.
- New `.kurorc` format: explicitly rejected (see "Why no `.kurorc`"). If a
  future feature truly needs structured config, file a follow-up plan.
- Buck1 compatibility: kuro long since dropped Buck1 parity; this plan
  doesn't reintroduce it. The `[buildfile] name = TARGETS` knob in some
  Meta-internal `.buckconfig` files is **drop**.
