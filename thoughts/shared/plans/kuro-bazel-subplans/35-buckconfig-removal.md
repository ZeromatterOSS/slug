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

Today the kuro source tree has 277 `BUCK` files, 13 `.buckconfig` files, and 1
`.bazelrc` (in an unrelated example). Kuro is positioning itself as a
Bazel-compatible build system but its own bootstrap is still Buck-shaped. This
plan converts the source tree to Bazel-native conventions:

- `BUCK` → `BUILD.bazel`
- `.buckconfig` → some combination of `MODULE.bazel` (cells), `.bazelrc`
  (flags), `.bazelignore` (ignored paths), or hardcoded Rust defaults.
- The `legacy_configs` parser in `app/kuro_common/src/legacy_configs/` becomes
  dead code and is deleted.

**No `.kurorc`**, see "Knob disposition" below for the analysis.

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

277 `BUCK` files vs 17 `BUILD.bazel`. The 277 only resolve because kuro's root
`.buckconfig` opts in via `[buildfile] name = BUCK` (and `shim/.buckconfig`
adds `BUCK.reindeer,BUCK` for vendored Cargo deps). Every directory with a
`BUCK` will get a `git mv` to `BUILD.bazel`, except the `BUCK.reindeer` files
which stay (they're explicitly distinct from regular targets and reindeer
hardcodes the name). The buildfile-discovery code already lists `BUILD.bazel`
first, so this is a no-op behavior change post-rename.

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

### Phase 35.1: Audit + decision freeze  [~½ day]

#### Goal

Lock down the per-knob disposition table. No code changes.

#### Work

1. Walk every `.buckconfig` in `kuro/`, `kuro/examples/*`, `kuro/tests/*`.
   Produce a per-key spreadsheet: `(file, section, key) → disposition`.
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
5. Document the per-knob landing in
   `thoughts/shared/research/2026-MM-DD-buckconfig-knob-disposition.md`.

#### Acceptance

- Every knob in every active `.buckconfig` has a documented destination.
- Every section has either a single migration target or an explicit "drop
  with rationale" entry.
- The list of `.bazelrc` flags Kuro must accept is finalized; cross-checked
  against the existing `app/kuro_client_ctx/src/bazelrc.rs` parser.

### Phase 35.2: `BUCK` → `BUILD.bazel` rename  [~1 day]

#### Goal

Drop the Buck-shaped naming opt-in from kuro's tree.

#### Work

1. Codemod: `git mv` every `BUCK` to a sibling `BUILD.bazel` in directories
   where `BUILD.bazel` does not already exist. Skip `BUCK.reindeer` files
   (vendored Cargo) and any explicit `extra_for_test` fixtures.
2. Update any explicit references in source: `grep -rn '"BUCK"' app/` for
   string literals (test fixtures, error messages, completion strings in
   `kuro_cmd_completion_client/src/complete/package.rs`).
3. Drop `[buildfile] name = BUCK` from `kuro/.buckconfig`. Leave the file
   present (other sections still in use); just the `[buildfile]` section
   goes.
4. Verify each cell still resolves: `cargo build -p kuro`,
   `python -m pytest tests/core/analysis/test_native_rules.py`, LLVM
   Demangle smoke. Build files inside cells (`shim/`, `prelude/`,
   `bazel_tools/`) inherit each cell's own `[buildfile] name` setting —
   migrate those too if they opt out of the default.
5. Add a regression test that `BUCK` is NOT in the default
   `parse_buildfile_name` output (already true in `buildfiles.rs:26`,
   make it explicit).

#### Acceptance

- `find . -name BUCK -not -name 'BUCK.reindeer' | wc -l` → 0 in the kuro
  source tree.
- `cargo build -p kuro` clean. Analysis suite at the same baseline as
  pre-rename. LLVM Demangle clean.
- `kuro/.buckconfig` no longer has a `[buildfile]` section.

### Phase 35.3: Cells → MODULE.bazel  [~1-2 days]

#### Goal

Move every `[cells]` / `[cell_aliases]` / `[external_cells]` declaration into
the corresponding workspace's `MODULE.bazel`. Drop those sections from every
`.buckconfig`.

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

- `grep -rn '\[cells\]\|\[repositories\]\|\[cell_aliases\]\|\[repository_aliases\]\|\[external_cells\]' /var/mnt/dev/kuro/{,examples,tests}/.buckconfig 2>/dev/null` → empty.
- `cargo build -p kuro` clean. Test suite + LLVM clean.
- `kuro audit cell` (or equivalent) reports the same cell topology
  pre/post-migration.

### Phase 35.4: `.bazelignore` adoption  [~½ day]

#### Goal

Move `[project] ignore` to `.bazelignore`. The other `[project]` keys
(`package_boundary_exceptions`, `watchman_merge_base`) become `.bazelrc`
flags or get dropped.

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

- `kuro/.bazelignore` exists and the file watcher honours it.
- No `.buckconfig` has a `[project]` section.

### Phase 35.5: Runtime knobs → `.bazelrc`  [~2-3 days]

#### Goal

Drain every remaining `.buckconfig` section into `.bazelrc` flags or hardcoded
defaults.

#### Work

1. For each `[kuro*]` and `[build]` / `[parser]` / `[log]` / `[sandbox]` /
   `[test]` / `[ui]` / `[http]` / `[client]` key from the audit:
   - If a CLI flag already exists, use it.
   - If not, add a `--kuro_<key>` flag (or Bazel-compatible name) at the
     same precedence as the existing config read.
   - Update the `BuckconfigKeyRef` consumer to read the flag value first,
     fall through to the `.buckconfig` value during the deprecation window,
     then drop the fallback.
2. Add a workspace-default `.bazelrc` to the kuro root with the moved
   defaults (e.g. `build --kuro_starlark_max_callstack_size=512`).
3. Examples/tests with non-default knobs (e.g.
   `examples/vscode/.buckconfig` has `[kuro_re_client]` for a local RE
   server) get their own `.bazelrc`.
4. Drop the migrated sections from every `.buckconfig`.

#### Acceptance

- Every `.bazelrc` flag introduced has a help string and a documented
  default.
- `grep -rn '\[kuro' /var/mnt/dev/kuro/**/.buckconfig` → empty.
- Behaviour is preserved: targeted regression tests for the most-used
  knobs (`max_concurrent_requests`, `digest_algorithms`,
  `execution_platforms`).

### Phase 35.6: Delete the legacy config parser  [~2 days]

#### Goal

Remove `app/kuro_common/src/legacy_configs/` once nothing reads from
`.buckconfig` anymore.

#### Work

1. Confirm no live consumer remains. Should be zero `BuckconfigKeyRef`
   call sites outside the parser itself by this phase.
2. Delete `legacy_configs/` (parser, cells.rs, dice integration, tests).
   Several thousand LOC.
3. Replace the `kuro_common::legacy_configs::cells` cell-resolution path
   with a MODULE.bazel-only path. The bzlmod-based resolver in Plan 02
   already handles the common case; this phase makes it the only path.
4. Delete `kuro/.buckconfig`, `kuro/bazel_tools/.buckconfig`,
   `kuro/shim/.buckconfig`, `kuro/prelude/.buckconfig`, and every
   example/test `.buckconfig` that's now empty.
5. Add a startup warning: if the daemon finds a `.buckconfig` in the
   workspace root, log a deprecation pointer to migration docs.

#### Acceptance

- `legacy_configs/` directory gone.
- No `.buckconfig` files in the kuro source tree.
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

Phases 35.1 (audit) and 35.2 (BUCK rename) are independent and ship first.
Phases 35.3-35.5 are sequential, each draining one major block of the parser
surface. Phase 35.6 is the payoff (delete the parser) and depends on all
preceding phases. Phase 35.7 is documentation cleanup at the end.

```
35.1 audit ──┐
             ├── 35.3 cells → MODULE.bazel ──┐
35.2 BUCK ──┘                                 │
                                              ├── 35.6 delete parser ── 35.7 docs
35.1 audit ── 35.4 .bazelignore ──────────────┤
35.1 audit ── 35.5 runtime → .bazelrc ────────┘
```

35.3 / 35.4 / 35.5 can run in parallel after 35.1 lands; each touches a
disjoint section of `.buckconfig`.

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
  the deprecation warning from Phase 35.6, then delete in the release after.
- **Buried `.buckconfig` consumers**: a knob the audit misses, ripped out in
  35.6, breaks a corner-case workflow. Mitigation: Phase 35.6's
  "no consumers remain" gate verifies via `grep`; CI green is the second
  check.
- **`.bazelrc` parser limits**: existing parser at `bazelrc.rs` covers
  Bazel's spec; verify it handles every flag shape we want to introduce
  (especially flags with structured values like comma-separated lists).
- **Test fixture divergence**: tests that explicitly construct
  `LegacyBuckConfig` instances (`tests/core/...`, `app/kuro_interpreter_for_build_tests/`)
  need to migrate to whatever replaces it — likely a fixture builder that
  emits `MODULE.bazel` + `.bazelrc` test inputs. Expect a chunk of test
  rework in Phase 35.6.

## Verification

For each phase:

- `cargo build -p kuro` clean.
- `python -m pytest tests/core/analysis/test_native_rules.py
  --deselect tests/core/analysis/test_native_rules.py::test_sh_test_runs
  -q` at or above the pre-phase baseline (currently 122 pass + 5
  pre-existing failures + 3 skip + 1 deselect).
- LLVM Demangle smoke clean (`cd /var/mnt/dev/llvm-project/utils/bazel
  && /var/mnt/dev/kuro/kuro build --config=generic_gcc --remote_executor=
  @llvm-project//llvm:Demangle`). Demangle is the lightweight smoke; LLVM
  Support (~183 actions) is the heavier check at the end of Phase 35.5
  and 35.6.

## Out of Scope

- Renaming `kuro` itself: this plan keeps the tool name and its CLI shape;
  what changes is the per-workspace config format. Repo-wide renames are
  separate.
- Migrating users' workspaces: this plan only converts the kuro source tree
  + bundled examples + tests. External user workspaces get a migration
  guide (Phase 35.7) but are responsible for their own conversions during
  the deprecation window.
- New `.kurorc` format: explicitly rejected (see "Why no `.kurorc`"). If a
  future feature truly needs structured config, file a follow-up plan.
- Buck1 compatibility: kuro long since dropped Buck1 parity; this plan
  doesn't reintroduce it. The `[buildfile] name = TARGETS` knob in some
  Meta-internal `.buckconfig` files is **drop**.
