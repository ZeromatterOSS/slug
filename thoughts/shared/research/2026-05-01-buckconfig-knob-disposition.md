# `.buckconfig` Knob Disposition (Plan 35.1 Audit)

**Date**: 2026-05-01
**Plan**: [35-buckconfig-removal.md](../plans/kuro-bazel-subplans/35-buckconfig-removal.md)
**Phase**: 35.1 — Audit + decision freeze

This document is the per-knob disposition table that Phase 35.1 produces.
It locks down where every `.buckconfig` key in **active workspaces** lands
(MODULE.bazel / .bazelrc / .bazelignore / drop / defer). Test-fixture
buckconfigs are inventoried at the end but not classified key-by-key — they
are processed in batch in Phase 35.6a.

## 1. Summary counts

- **Active-workspace `.buckconfig` files**: 28 (root + prelude/shim/bazel_tools
  + tests/manual_test + 22 under examples/).
- **Test-fixture `.buckconfig` files**: 261 (under
  `tests/{core,e2e}/**/test_*_data/`). Deferred to Phase 35.6a.
- **Distinct sections seen in active workspaces**: `cells`, `repositories`,
  `cell_aliases`, `repository_aliases`, `external_cells`, `oss`, `project`,
  `rust`, `parser`, `buildfile`, `build`, `kuro`, `kuro_re_client`, `alias`,
  `http`, `ovrsource`, `java`, `kotlin`, `not_buildfile`, `not_http`,
  `not_repository_aliases`.
- **Distinct Rust consumer files** (grep `BuckconfigKeyRef` outside
  `legacy_configs/`): 30 files. See §5.

## 2. Per-file knob inventory (active workspaces)

### Root: `./.buckconfig`

| Section | Keys | Disposition |
|---|---|---|
| `[cells]` | `gh_facebook_kuro = .`, `gh_facebook_kuro_shims_meta = shim` | MODULE.bazel — root `module(name = "gh_facebook_kuro")` + `bazel_dep` + `local_path_override` for shim |
| `[cell_aliases]` | `root = gh_facebook_kuro` | MODULE.bazel — `repo_name = "root"` on the self module entry |
| `[oss]` | `internal_cell = fbcode`, `stripped_root_dirs = kuro` | **drop** — Meta-internal Buck1 vestige; `is_open_source()` already gates this in Rust |
| `[project]` | `ignore = app/kuro_explain, app_dep_graph_rules, examples, integrations/rust-project/tests` | `.bazelignore` — one path per line |
| `[rust]` | `default_edition = 2024` | **drop** — kuro's own bootstrap config; Cargo handles edition; not a kuro build-system concern |

### `./prelude/.buckconfig`

| Section | Keys | Disposition |
|---|---|---|
| `[repositories]` | `prelude = .` | MODULE.bazel — `module(name = "prelude")` |
| `[repository_aliases]` | (empty under `[repository_aliases]`; populated under `[not_repository_aliases]` with `config = ovr_config` for non-OSS) | **drop** — `[not_*]` is OSS gating; OSS path is empty. Internal path uses `config = ovr_config` which is Meta-only. |
| `[buildfile] / [not_buildfile]` | `name_v2 = TARGETS,BUCK` (non-OSS only) | **drop** for OSS (default is `BUILD.bazel`) |
| `[ovrsource]` | (no keys) | **drop** — defensive empty section |
| `[java]`, `[kotlin]` | language-specific knobs | **drop from prelude/.buckconfig** — these are Meta-internal language toolchains, no Kuro Rust consumer |
| `[http] / [not_http]` | `maven_repo = ...` (non-OSS only) | **drop** — Meta-internal Maven mirror |

**Action**: Delete `prelude/.buckconfig` entirely after MODULE.bazel
migration. Prelude is loaded as a bundled cell (Plan 28); no buckconfig
needed.

### `./shim/.buckconfig`

| Section | Keys | Disposition |
|---|---|---|
| `[buildfile]` | `name = BUCK.reindeer,BUCK` | **keep as `[buildfile]`-only `.buckconfig`** until reindeer-side BUILD.bazel support lands. Reindeer hardcodes `BUCK.reindeer`; this is a documented exception in Plan 35.2. |
| `[cells]` | `gh_facebook_kuro_shims_meta = .`, `none = none` | MODULE.bazel |
| `[cell_aliases]` | `root = gh_facebook_kuro_shims_meta`, `bazel_skylib`, `buck`, `fbcode`, `fbcode_macros`, `fbsource`, `shim`, `toolchains` → all alias to `gh_facebook_kuro_shims_meta` | MODULE.bazel via `repo_name` aliases on the self module |
| `[external_cells]` | (commented-out `prelude = bundled`) | **drop** — already handled by Plan 28 auto-registration |
| `[parser]` | `target_platform_detector_spec = ...` | `.bazelrc` flag (`build --platforms=...` per detector spec; or hardcode if spec is trivial) |

### `./bazel_tools/.buckconfig`

| Section | Keys | Disposition |
|---|---|---|
| `[cells]` | `bazel_tools = .` | MODULE.bazel — `module(name = "bazel_tools")`. **Bundled cell auto-registers** (Plan 28); the `.buckconfig` file becomes deletable. |

**Action**: Delete the file entirely after MODULE.bazel migration.

### `./tests/manual_test/.buckconfig`

Only a comment; no keys. **Action**: Delete entirely in Phase 35.6b.

### `./examples/*/.buckconfig` summary

22 example workspaces. Common patterns:

- `[cells]` + `[cell_aliases]` + `[external_cells] prelude = bundled` —
  → MODULE.bazel.
- `[parser] target_platform_detector_spec = target:root//...->prelude//platforms:default` —
  → `.bazelrc` flag (per-example workspace). Almost every example has the
  same value modulo the cell name; auto-register `@local_config_platform//:host`
  already covers most cases (per `memory/MEMORY.md` Phase 17).
- `[build] execution_platforms = ...` —
  → `.bazelrc` flag `build --platforms=<label>`.
- `[external_cells] prelude = bundled` —
  → drop (auto-registered).
- `[buildfile] name = TARGETS.fixture` (in `examples/bxl_tutorial/`) —
  → drop after BUCK rename.
- `[alias] demoapp = root//app:demoapp_debug` (in `examples/android/demoapp/`) —
  → drop (Buck1-style target alias; not a Bazel concept). Verify no scripts
  rely on it.
- `[kuro] file_watcher = notify` (in two go/python toolchain examples) —
  → `.bazelrc` flag (already a CLI option pattern).
- `[kuro_re_client]` (in 5 RE examples + vscode) —
  → `.bazelrc` flag block. See §3.
- `[kuro] digest_algorithms = SHA256` —
  → `.bazelrc` flag `build --kuro_digest_algorithms=SHA256`.

**Empty `.buckconfig` files** (just a comment or zero bytes):
`examples/bootstrap/bootstrap/.buckconfig`,
`examples/bootstrap/toolchains/.buckconfig`,
`examples/toolchains/go_toolchain/toolchains/.buckconfig`,
`examples/with_prelude/toolchains/.buckconfig`,
`examples/hello_world/.buckconfig` (comment-only),
`examples/toolchains/cxx_zig_toolchain/toolchains/.buckconfig` (only
`[cells]` for sub-cell).
→ Delete entirely once parent workspace's MODULE.bazel covers cell
registration.

## 3. Section → destination (master mapping)

This refines the table in Plan 35's "Knob Audit" section. Where the plan
already has an entry, this table just confirms it; where there's a finer
decision (specific flag name, drop vs keep), this table is authoritative.

| Section | Disposition | Notes |
|---|---|---|
| `[cells]` / `[repositories]` | MODULE.bazel | `module()` + `bazel_dep()` + `local_path_override()` |
| `[cell_aliases]` / `[repository_aliases]` | MODULE.bazel | `repo_name` on `bazel_dep` |
| `[external_cells]` | drop | Plan 28 auto-registers bundled cells |
| `[buildfile]` `name` / `name_v2` | drop | Default `BUILD.bazel` covers all post-Phase-35.2 cases. Exception: `shim/.buckconfig` keeps `BUCK.reindeer,BUCK` until reindeer-side migration. |
| `[buildfile]` `extra_for_test` | drop | No active consumer; verify in 35.2 |
| `[buildfile]` `includes` / `package_includes` | drop | No active consumer in OSS path |
| `[parser]` `target_platform_detector_spec` | `.bazelrc` | `build --platforms=<label>` (already supported by bazelrc parser); auto-`@local_config_platform//:host` covers host case |
| `[project]` `ignore` | `.bazelignore` | One path per line |
| `[project]` `package_boundary_exceptions` | `.bazelrc` | New flag `--kuro_package_boundary_exceptions=<list>` |
| `[project]` `watchman_merge_base` | `.bazelrc` | New flag `--kuro_watchman_merge_base=<rev>` |
| `[oss]` | drop | Meta-internal Buck1 vestige |
| `[rust]` | drop | Bootstrap config, Cargo's responsibility |
| `[build]` `execution_platforms` | `.bazelrc` | `build --platforms=<label>` (Bazel-compat) |
| `[build]` `threads` | `.bazelrc` | `build --jobs=<N>` (Bazel-compat) |
| `[build]` `lazy_cycle_detector` | `.bazelrc` | `--kuro_lazy_cycle_detector` |
| `[build]` RE knobs | `.bazelrc` | New `--kuro_*` flags (see §3.1) |
| `[client]` `id` | `.bazelrc` | `--kuro_client_id` |
| `[log]`, `[sandbox]`, `[test]`, `[ui]`, `[http]` | `.bazelrc` | per-knob `.bazelrc` flag (see §3.2) |
| `[kuro]` (~50 keys) | `.bazelrc` | `--kuro_<key>` per key (see §3.3) |
| `[kuro_re_client]` | `.bazelrc` | `--kuro_re_*` per key (see §3.1) |
| `[kuro_resource_control]` | `.bazelrc` | `--kuro_rc_*` per key |
| `[kuro_health_check]` | `.bazelrc` | `--kuro_health_*` per key |
| `[kuro_system_warning]` | `.bazelrc` | `--kuro_warn_*` per key |
| `[build_report]` | drop | No consumers in audit (single grep hit in `build_report.rs:1057` reads `[build_report]` for one string key; verify in 35.5 — if used, becomes `--kuro_build_report_<key>`; otherwise drop) |
| `[deprecated_config]` | drop | Used only in `test_deprecated_config_data` test fixture; tests goes to bucket (A) in 35.6a |
| `[alias]` | drop | Buck1 target aliases; not a Bazel concept. The one in `examples/android/demoapp/` is removed in 35.3/35.5. |
| `[ovrsource]`, `[java]`, `[kotlin]`, `[not_*]` | drop | Meta-internal / OSS gating |

### 3.1 `.bazelrc` flag names — RE client

Existing `.bazelrc` parser (`app/kuro_client_ctx/src/bazelrc.rs`) supports
arbitrary `--name=value` flags. The parser is value-agnostic; flag
acceptance is delegated to the daemon. Plan 35.5 wires each flag to the
existing `BuckconfigKeyRef` consumer.

| Buckconfig key | `.bazelrc` flag |
|---|---|
| `kuro_re_client.engine_address` | `--remote_executor` (Bazel-compat) or `--kuro_re_engine_address` |
| `kuro_re_client.action_cache_address` | `--remote_cache` (Bazel-compat) or `--kuro_re_action_cache_address` |
| `kuro_re_client.cas_address` | `--kuro_re_cas_address` |
| `kuro_re_client.tls` | `--kuro_re_tls` (default `true`) |
| `kuro_re_client.tls_client_cert` | `--remote_tls_certificate` (Bazel-compat) or `--kuro_re_tls_client_cert` |
| `kuro_re_client.tls_ca_certs` | `--tls_certificate` (Bazel-compat) or `--kuro_re_tls_ca_certs` |
| `kuro_re_client.http_headers` | `--remote_header=K=V` (repeated; Bazel-compat) |
| `kuro_re_client.instance_name` | `--remote_instance_name` (Bazel-compat) |
| `kuro_re_client.capabilities` | `--kuro_re_capabilities` |
| `kuro_re_client.use_fbcode_metadata` | `--kuro_re_use_fbcode_metadata` |

**Recommendation**: Use Bazel-compatible names (`--remote_executor`,
`--remote_cache`, `--remote_header`, `--remote_instance_name`,
`--remote_tls_certificate`, `--tls_certificate`) where Bazel has a
matching flag; Kuro-prefix (`--kuro_re_*`) for kuro-only knobs. This
makes RE-using Bazel projects drop-in compatible.

### 3.2 `.bazelrc` flag names — runtime knobs

| Buckconfig key | `.bazelrc` flag |
|---|---|
| `kuro.digest_algorithms` | `--digest_function=SHA256` (Bazel-compat) — one of SHA1/SHA256/BLAKE3 |
| `kuro.file_watcher` | `--kuro_file_watcher={notify,watchman,fs_hash_crawler}` |
| `kuro.max_concurrent_requests` | `--kuro_max_concurrent_requests=<N>` |
| `kuro.starlark_max_callstack_size` | `--kuro_starlark_max_callstack_size=<N>` |
| `build.threads` | `--jobs=<N>` (Bazel-compat) |
| `build.execution_platforms` | `--platforms=<label>` (Bazel-compat — first platform; multiple platforms use repeated flag) |
| `parser.target_platform_detector_spec` | drop or expose as `--kuro_target_platform_detector_spec=<spec>` (the auto-`@local_config_platform//:host` register already covers most cases per Phase 17) |
| `client.id` | `--kuro_client_id=<string>` |
| `ui.console` | `--curses={on,off,auto}` (Bazel-compat with `--curses`) |
| `log.*` | `--kuro_log_*` |
| `test.*` | `--test_*` (Bazel-compat where possible) |

### 3.3 `[kuro]` section flag names

50+ keys. Strategy: every `kuro.<key>` becomes `--kuro_<key>` literally,
with Bazel-compat aliases for the common ones (`--digest_function`,
`--jobs`, etc.). Per-key listing deferred to a Phase 35.5 sub-spreadsheet
once each key's daemon-side wiring is touched.

## 4. `.bazelignore` migration

Only one knob: `[project] ignore` from `./.buckconfig`. Translates to:

```
# .bazelignore (root)
app/kuro_explain
app_dep_graph_rules
examples
integrations/rust-project/tests
```

No other active workspace uses `[project] ignore`. The `.bazelignore`
parser must be added to `kuro_common` (Phase 35.4). Wire into the file
watcher / directory walker that currently reads `[project] ignore`
(consumers found in `app/kuro_file_watcher/src/edenfs/interface.rs:130`,
`app/kuro_file_watcher/src/watchman/interface.rs:362`,
`app/kuro_server/src/daemon/state.rs:404`).

## 5. Rust consumer cross-reference

`BuckconfigKeyRef` consumers outside `legacy_configs/` (30 files). Every
section mentioned below has at least one Rust reader; Phase 35.5 must
introduce a `.bazelrc` flag that lands in the same place before the
consumer can be removed from the buckconfig path.

| Section read by | Files |
|---|---|
| `[buildfile]` | `kuro_interpreter/src/import_paths.rs` |
| `[kuro]` | many: `kuro_interpreter/{allow_relative_paths,factory}.rs`, `kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`, `kuro_build_api/{configure_dice,artifact_groups/calculation}.rs`, `kuro_file_watcher/{file_watcher,watchman/interface}.rs`, `kuro_execute_impl/src/materializers/deferred/clean_stale.rs`, `kuro_configured/src/nodes.rs`, `kuro_server/src/{ctx,daemon/{disk_state,io_provider,forkserver,state}}.rs`, `kuro_server_commands/src/build.rs` |
| `[deprecated_config]` | `kuro_interpreter_for_build/src/interpreter/buckconfig.rs:255` (test-only support; goes away with Plan 35.6a bucket-(A) fixture) |
| `[build]` | `kuro_build_api/src/materialize.rs`, `kuro_node/src/execution.rs`, `kuro_server/src/{ctx,daemon/state}.rs` |
| `[build_report]` | `kuro_build_api/src/build/build_report.rs:1057` (single key — verify use; likely candidate to drop) |
| `[parser]` | `kuro_configured/src/target_platform_resolution.rs:75` |
| `[project]` | `kuro_file_watcher/src/{watchman/interface,edenfs/interface}.rs`, `kuro_server/src/daemon/state.rs:404` |
| `[test]` | `kuro_test/src/command.rs:383` |
| `[ui]` | `kuro_server/src/ctx.rs:737` |
| `[sandbox]` | `kuro_server/src/ctx.rs:790` |
| `[scuba]` | `kuro_server/src/ctx.rs:1025` (Meta-internal — drop in OSS) |
| `[log]` | `kuro_server/src/ctx.rs:1039` |
| `[client]` | `kuro_server/src/ctx.rs:1075` |
| `[http]` | `kuro_common/src/init.rs:68-90` |
| `[kuro_system_warning]` | `kuro_common/src/init.rs:168,172` |
| `[kuro_re_client]` | `kuro_re_configuration/src/lib.rs` (~80 BuckconfigKeyRef calls — largest single consumer) |

## 6. Drop list (no Rust consumer in OSS path)

These keys appear in active workspace `.buckconfig`s but have **no Rust
consumer** in the OSS code path. They can be dropped directly without a
`.bazelrc` flag introduction:

- `[oss]` (entire section)
- `[rust]` (entire section)
- `[ovrsource]` (entire section)
- `[java]` (entire section, in prelude/.buckconfig)
- `[kotlin]` (entire section, in prelude/.buckconfig)
- `[not_*]` (OSS-gated sections — empty when OSS-enabled)
- `[alias]` in `examples/android/demoapp/.buckconfig` (Buck1 target alias; verify scripts)

## 7. Defers (open questions)

- **`shim/.buckconfig`** `[buildfile] name = BUCK.reindeer,BUCK`: Reindeer
  hardcodes `BUCK.reindeer`. Until reindeer-side migration, this section
  must remain. Decision: **keep this single section** in
  `shim/.buckconfig` past Phase 35.6b; revisit after reindeer migration.
  Document as a known exception.
- **`build_report` section**: Single grep hit. Phase 35.5 task: verify
  consumer; drop or wire to flag.
- **`[parser] target_platform_detector_spec`**: After Phase 17's
  `@local_config_platform//:host` auto-register, is the detector spec
  ever load-bearing in the OSS path? If yes, needs a flag; if no, drop.
  Phase 35.5 task: trace consumer at
  `kuro_configured/src/target_platform_resolution.rs:75` and decide.
- **`[deprecated_config]` Rust support**: Used only by
  `test_deprecated_config_data`. Bucket (A) deletion in 35.6a removes the
  test; the parser code at
  `kuro_interpreter_for_build/src/interpreter/buckconfig.rs:255` then
  becomes dead and is removed in 35.6b.

## 8. Test fixture inventory (Phase 35.6a seed)

261 `.buckconfig` files under `tests/{core,e2e}/**/test_*_data/`.
Categorized purely by filename pattern + `cat` of first 30 lines (no
key-by-key analysis):

### Likely bucket (A) — delete-with-test (~30–40 fixtures)

Tests that, by name, exist *to test* legacy `.buckconfig` parsing:

- `tests/core/audit/test_audit_config_data/` (3 nested fixtures)
- `tests/core/build/test_external_buckconfigs_data/`
- `tests/core/configurations/test_select_buckconfig_data/`
- `tests/core/interpreter/test_deprecated_config_data/` (2 nested)
- `tests/core/interpreter/test_read_root_config_data/` (2 nested)
- `tests/core/build/test_target_aliases_data/` (uses `[alias]`)
- `tests/core/interpreter/test_callstack_size_data/` (3 nested; tests `[interpreter]` knob)
- `tests/core/interpreter/test_peak_allocated_bytes*_data/` (2)
- `tests/core/build/test_unhashed_outputs_data/`
- `tests/core/log/test_representative_config_flags_data/`
- `tests/core/build/test_paranoid_data/execution_platforms/` (Buck2-specific paranoid mode)
- `tests/core/configurations/test_unified_constraint_data/` (already in collect_ignore per memory)
- `tests/core/interpreter/test_unstable_typecheck_data/`, `test_prelude_typecheck_data/`, `test_peak_allocated_bytes*` (per memory: already in collect_ignore)
- Plus most fixtures under `tests/core/log/`, `tests/core/io/`,
  `tests/core/explain/`, `tests/core/query/uquery/`,
  `tests/core/configurations/test_exec_modifier_data/`,
  `tests/core/test/test_modifiers_data/`, `tests/core/subscribe/`,
  `tests/core/vpnless/`, `tests/core/completion/`, `tests/core/console/`,
  `tests/core/trace_io/`, `tests/core/resource_control/*` — all in
  `collect_ignore` per memory.

### Likely bucket (B) — script-migrate to MODULE.bazel (~200 fixtures)

Almost everything else. Fixtures that have `[cells] root = .` +
`[external_cells] prelude = bundled` + `[buildfile] name = TARGETS.fixture`
and nothing else. The script in Phase 35.6a step 3 handles them.

### Likely bucket (C) — keep with rationale (~10–20 fixtures, TBD)

Fixtures whose tests don't fit cleanly into (A) or (B): edge cases
around `[parser]`, `[build]` knobs that haven't migrated yet, or fixtures
exercising multi-cell setups. Triaged in Phase 35.6a step 1.

## 9. `.bazelrc` parser readiness check

`app/kuro_client_ctx/src/bazelrc.rs` (1122 LOC, per plan §Dependencies):
- Supports `import` / `try-import` / `--config=` profiles. ✓
- Accepts arbitrary `--key=value` flags. ✓
- Parser is value-agnostic; daemon-side acceptance is the constraint.

**Gaps to verify before Phase 35.5**:
- Does the parser handle structured values (comma-separated lists,
  K=V repeated `--remote_header`)? Plan 35 §Risks flags this; Phase
  35.5 step 1 must include a parser smoke test for each new flag shape.
- Does `--remote_header=K=V` need parser changes to allow repeated form?
- `parse_list` consumers (e.g.
  `kuro_re_configuration/src/lib.rs:566 .parse_list`) translate
  cleanly to comma-separated `.bazelrc` flag values; verify.

## 10. Phase 35.1 acceptance check

- [x] Every active-workspace `.buckconfig` knob has a documented
      destination (§2 + §3).
- [x] Every section has either a single migration target or an explicit
      "drop with rationale" entry (§3, §6).
- [x] Test-fixture inventory exists (§8); per-key classification deferred
      to Phase 35.6a per plan.
- [x] Rust consumer cross-reference complete (§5).
- [x] `.bazelrc` parser readiness verified at gap-list level (§9);
      per-flag verification deferred to Phase 35.5.
- [x] Defers/open questions documented (§7).
