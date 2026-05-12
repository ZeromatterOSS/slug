# Phase 35.6b: Parser Deletion Plan

**Date:** 2026-05-04
**Context:** Q1=B decision — keep `read_config()` + `config_setting(values=)` backed by a thin
in-memory KV store from `-c` CLI flags only. Stop parsing `.buckconfig` files. Delete parser
files; keep `LegacyBuckConfig` struct + lookup API.

---

## Section 1: Inventory of `app/slug_common/src/legacy_configs/`

| File | LOC | Role | Disposition |
|------|-----|------|-------------|
| `cells.rs` | 2493 | Cell resolution + bzlmod integration, `BuckConfigBasedCells`, `ExternalBuckconfigData`, `parse_with_config_args`, `parse_single_cell_with_dice` | **Partial keep.** The bzlmod/cell resolution logic (70% of lines) stays. The `.buckconfig` *file* parsing paths — `get_project_buckconfig_paths`, `get_external_buckconfig_paths`, `parse_single_cell_with_file_ops_inner`, `DEFAULT_PROJECT_CONFIG_SOURCES`, `DEFAULT_EXTERNAL_CONFIG_SOURCES` — must change to return empty results or be removed. |
| `configs.rs` | 859 | `LegacyBuckConfig`, `ConfigData`, `ConfigValue`, `LegacyBuckConfigSection` structs + `empty()`, `filter_values()`, `start_parse_for_external_files()`, `finish_parse()`, testing helpers | **Partial keep.** The struct + `empty()` + `filter_values()` + `get`/`parse`/`parse_list` (in `access.rs`) stay. `start_parse_for_external_files()`, `finish_parse()`, and `testing::parse`/`parse_with_config_args` all invoke the file parser and must be replaced with a `from_overrides_only()` constructor. |
| `parser.rs` | 435 | `.buckconfig` line parser — `LegacyConfigParser`, `LegacyConfigFileParser`, line-by-line parsing, includes handling | **Delete** once all callers replaced. |
| `parser/resolver.rs` | 216 | `$(config section.key)` reference resolution (variable substitution in config values) | **Delete** with `parser.rs`. Only needed for file-parsed configs; CLI flags never need interpolation. |
| `dice.rs` | 431 | DICE keys: `LegacyBuckConfigForCellKey`, `LegacyBuckConfigPropertyProjectionKey`, `HasLegacyConfigs`, `SetLegacyConfigs`, `HasInjectedLegacyConfigs`, `OpaqueLegacyBuckConfigOnDice` | **Keep entirely.** The DICE invalidation machinery is correct and still needed — CLI overrides now replace file contents as the injected value, but the key/projection structure remains valid. |
| `file_ops.rs` | 394 | `ConfigPath`, `ConfigParserFileOps` trait, `DefaultConfigParserFileOps`, `DiceConfigFileOps`, `push_all_files_from_a_directory` | **Delete** once `parse_single_cell_with_file_ops_inner` is gone. `ConfigPath` is used in `ExternalPathBuckconfigData.origin_path` (for debug strings); see Step B. |
| `aggregator.rs` | 214 | `CellsAggregator` — assembles `CellResolver` from cell list + aliases | **Keep.** Not file-parser-dependent; called from `cells.rs` bzlmod path. |
| `args.rs` | 239 | `ResolvedLegacyConfigArg`, `ResolvedConfigFlag`, `ResolvedConfigFile`, `ExternalConfigFile`, `resolve_config_args`, `to_proto_config_args` | **Partial keep.** `ResolvedConfigFlag` and `resolve_config_args` (the `Value` arm) stay — they parse `section.key=value` strings. `ExternalConfigFile` and the `File` arm of `resolve_config_args` invoke the file parser (via `LegacyBuckConfig::start_parse_for_external_files`) and must be removed or stubbed. |
| `key.rs` | 20 | `BuckconfigKeyRef<'a>` — the `{section, property}` lookup key | **Keep.** Used by all 5 surviving consumers. |
| `view.rs` | 42 | `LegacyBuckConfigView` trait — `get`, `parse`, `parse_list` | **Keep.** Used by `buildfiles.rs` and the Starlark API path. |
| `access.rs` | 188 | `LegacyBuckConfig` method impls: `get`, `parse`, `parse_list`, `sections`, `all_sections`, `get_section`, `compare`, `iter` | **Keep.** These are the lookup API surface used by all consumers, including `audit config`'s `all_sections()` / `iter()` traversal. |
| `path.rs` | 59 | `DEFAULT_EXTERNAL_CONFIG_SOURCES`, `DEFAULT_PROJECT_CONFIG_SOURCES`, `DOT_BUCKCONFIG_LOCAL` | **Delete** once callers removed. |
| `cells_symlinks.rs` | 151 | `ensure_symlink`, `cleanup_stale_symlinks` — bzlmod symlink management | **Keep.** Unrelated to file-format parsing. |

**Summary of deletable code:** `parser.rs` (435 LOC) + `parser/resolver.rs` (216) + `file_ops.rs` (394) + `path.rs` (59) = ~1104 LOC. Plus significant portions of `cells.rs` (`get_project_buckconfig_paths`, `get_external_buckconfig_paths`, file-parse paths ~200 LOC) and the `finish_parse`/`start_parse_for_external_files` functions in `configs.rs` (~60 LOC) and `ExternalConfigFile`/`File` arm in `args.rs` (~50 LOC).

Total deletable after migration: approximately **1400 LOC** out of 5741.

### cells.rs closer look

`cells.rs` is **not deletable** — it is the home of all bzlmod cell resolution. The parts to remove are:

- `get_external_buckconfig_paths()` (lines 1607–1668): reads `/etc/buckconfig`, `~/.buckconfig.d`, etc. — remove entirely.
- `get_project_buckconfig_paths()` (lines 1670–1699): reads per-cell `.buckconfig`, `.buckconfig.d/`, `.buckconfig.local` — remove.
- `parse_single_cell_with_file_ops_inner()` (lines 1536–1551): calls both of the above + `finish_parse`. Replace with `from_overrides_only(args)`.
- `ExternalBuckconfigData::get_local_config_components()` (lines 170–210): reads `.buckconfig.local` across all cells for the serialized component. After this plan, it should return `Vec::new()`.
- `ExternalBuckconfigData::get_buckconfig_components()` (lines 212–235): calls the above. The `external_path_configs` field becomes empty; this function still serializes `args` to proto. Keep structure but simplify.
- `ExternalPathBuckconfigData` struct (line 127): can be removed once `external_path_configs: Vec<ExternalPathBuckconfigData>` field becomes `Vec<ResolvedConfigFlag>` or is folded into `args`.

The large legacy-only `.buckconfig [cells]` / `[external_cells]` path (lines 537–626) already has an `if !has_module_bazel` guard. In a future step this can be deleted entirely, but it's safe to leave during this plan since it's unreachable for all active workspaces (which all have `MODULE.bazel`).

---

## Section 2: Bundled-Cell Handling

### How prelude/.buckconfig and bazel_tools/.buckconfig are used

`app/slug_external_cells_bundled/build.rs` walks `prelude/` and `bazel_tools/` at **compile time** and emits `prelude_include.rs` / `bazel_tools_include.rs` as `include!()` data arrays. These arrays contain every file in the directory, including `.buckconfig`. The files are embedded as raw bytes into the binary.

At **runtime**, `ExternalCellsImpl::materialize_cell_files()` (in `app/slug_common/src/external_cells.rs`) writes these byte arrays to disk. The `.buckconfig` bytes are written to the materialized cell directory as a real file.

**The runtime parser is then invoked** via `parse_single_cell_with_dice()` (in `dice.rs`, line 195), which calls `BuckConfigBasedCells::parse_single_cell_with_dice()` → `parse_single_cell_with_file_ops_inner()` → reads `.buckconfig` from the cell directory and parses it with `LegacyConfigParser`.

### What the parser extracts from these files

- `prelude/.buckconfig` contains `[repositories]\nprelude = .` — this registers the cell name "prelude" as an alias for itself. **This is only used by the legacy non-bzlmod path** (`get_cell_aliases_from_config` → only called when `!is_bzlmod`). For bzlmod projects, this is dead.
- `bazel_tools/.buckconfig` contains `[cells]\nbazel_tools = .` — same pattern, same fate.

### Is a runtime parser required?

**No, for bzlmod projects.** The cell names `prelude` and `bazel_tools` are hardcoded in `cells.rs` (lines 485–491, 524). These `.buckconfig` files exist only as artifacts of the Buck v1 cell-resolution era. After bzlmod is the only path, the cell-name info in these files is redundant.

However, there is one complication: the **unit test** `test_bundled_prelude_data` in `lib.rs` at line 175 asserts `file.path == ".buckconfig" && contents.contains("prelude = .")`. This test would need to be deleted or updated.

### Minimum parser code if kept

If any non-bzlmod code path must survive (e.g., `CellAliasResolverKey` on DICE when `!is_bzlmod`), only `[cells]`/`[repositories]`/`[cell_aliases]`/`[repository_aliases]` keys from per-cell `.buckconfig` are needed. That is: strip `get_project_buckconfig_paths` to return only the cell `.buckconfig` file (no `.buckconfig.d`, no `.buckconfig.local`), and strip `get_external_buckconfig_paths` to return empty. That's already possible since the `!is_bzlmod` guard skips external paths. The parser itself would not shrink, but would simply not be invoked in practice.

**Recommended approach:** Do not try to delete the parser in one shot. Instead, hollow it out from the call sites — make the file-reading functions return empty — so the parser compiles but is never called. Then delete the parser files in a follow-up commit once the call sites are clean.

---

## Section 3: Refactor Strategy

### Step A — Add `LegacyBuckConfig::from_overrides_only(args: &[ConfigOverride])` constructor

**Files:** `app/slug_common/src/legacy_configs/configs.rs`

New constructor in `configs.rs`:

```rust
pub fn from_overrides_only(args: &[slug_cli_proto::ConfigOverride]) -> slug_error::Result<Self> {
    use slug_cli_proto::config_override::ConfigType;
    let mut sections: std::collections::BTreeMap<String, SortedMap<String, ConfigValue>> =
        std::collections::BTreeMap::new();
    for arg in args {
        if ConfigType::try_from(arg.config_type)? != ConfigType::Value {
            continue; // skip --config-file args; they are not read from disk
        }
        // parse "section.key=value"
        let raw = &arg.config_override;
        let (sk, v) = raw.split_once('=').ok_or_else(|| { /* ... */ })?;
        let (s, k) = sk.split_once('.').ok_or_else(|| { /* ... */ })?;
        let value = if v.is_empty() { None } else { Some(v.to_owned()) };
        if let Some(v) = value {
            sections.entry(s.to_owned())
                .or_default()
                .insert(k.to_owned(), ConfigValue::new_raw_arg(v));
        }
    }
    Ok(Self(Arc::new(ConfigData {
        values: SortedMap::from_iter(
            sections.into_iter().map(|(s, v)| (s, LegacyBuckConfigSection { values: v }))
        ),
    })))
}
```

Note: `ConfigValue::new_raw_arg` already exists and sets `resolved_value = Unknown`. The `as_str()` method panics if `resolved_value == Unknown`. There are two choices:
- Initialise `resolved_value = Literal` directly (simplest — no `$(config ...)` interpolation in CLI flags).
- Or call `ConfigResolver::resolve()` from `parser/resolver.rs`. Since CLI flags don't support `$(config ...)` syntax, setting `Literal` directly is correct and avoids the resolver dependency.

**API change:** New constructor only; no changes to `get`, `parse`, `parse_list`, `sections`, `all_sections`. `ResolvedValue::Literal` must be set on the `ConfigValue` (not `Unknown`).

**Validation:** Unit test in `configs.rs` that constructs a `from_overrides_only` config and calls `get()`.

---

### Step B — Replace `ExternalBuckconfigData` with a CLI-args-only version

**Files:** `app/slug_common/src/legacy_configs/cells.rs`, `app/slug_common/src/legacy_configs/args.rs`

Currently `ExternalBuckconfigData` holds:
- `external_path_configs: Vec<ExternalPathBuckconfigData>` — parsed external files
- `args: Vec<ResolvedLegacyConfigArg>` — both `Flag` and `File` variants

After this step:
- `external_path_configs` is always empty (remove field, or replace with a `Vec<()>` stub, or just never populate it).
- `args` retains only `Flag` (i.e. `ResolvedConfigFlag`) variants; the `File` variant can be stubbed or removed.
- `ExternalConfigFile` (which contains a `LegacyConfigParser`) becomes unused; remove it.

In `parse_with_file_ops_and_options_inner`, the two calls to `get_external_buckconfig_paths()` and `LegacyBuckConfig::start_parse_for_external_files()` should be replaced with an empty `Vec`.

In `parse_single_cell_with_file_ops_inner`, the call to `LegacyBuckConfig::finish_parse()` should be replaced with `LegacyBuckConfig::from_overrides_only(...)` using only the `Flag` args.

The `get_buckconfig_components()` proto serialization method still works — it serializes only `args`, which still contains the CLI flags. The `external_path_configs` serialization can be removed or emit an empty list.

`testing_default()` is unchanged (already returns an empty struct).

**API changes:** `ExternalBuckconfigData` loses the `external_path_configs` field. `ExternalPathBuckconfigData` becomes unused. `ExternalConfigFile` is removed. `resolve_config_args`'s `File` arm either errors (with a helpful message "config files not supported") or is silently dropped.

**Validation:** Existing `parse_with_config_args` integration sites continue to compile. The `test_config_file_args_overwrite_config_file` unit test in `configs.rs` will need to be deleted or updated.

---

### Step C — Simplify `parse_single_cell_with_dice` (DICE side)

**Files:** `app/slug_common/src/legacy_configs/dice.rs` (line 195 `LegacyBuckConfigForCellKey::compute`)

The `compute` function currently calls `BuckConfigBasedCells::parse_single_cell_with_dice()` which reads the cell's `.buckconfig`. After Step B, that becomes `from_overrides_only(external_data.args)` for every cell — all cells see the same CLI-derived KV store.

In practice this means every `LegacyBuckConfigForCellKey` computation returns the same config object (the CLI overrides, filtered by `is_config_invisible_to_dice`). That is correct behavior for Q1=B.

The DICE invalidation model still works: when CLI flags change (new invocation), `set_legacy_config_external_data` injects a new `ExternalBuckconfigData`, invalidating all `LegacyBuckConfigForCellKey` computations.

The `filter_values(is_config_invisible_to_dice)` call at line 200 is still correct and should be kept.

**No API change.** `HasLegacyConfigs`, `SetLegacyConfigs`, `OpaqueLegacyBuckConfigOnDice`, projections — all unchanged.

**Validation:** `slug audit config section.key` returns values set via `-c section.key=value` but not from `.buckconfig` files. The existing DICE equality test in `dice.rs` still passes.

---

### Step D — Gut `buildfiles.rs` fallback to hardcoded defaults

**Files:** `app/slug_common/src/buildfiles.rs`

`parse_buildfile_name` currently reads `[buildfile] name` and `[buildfile] extra_for_test` from buckconfig. With Q1=B and no `.buckconfig` files, these keys will never be set unless passed via `-c buildfile.name=...`. That behavior is actually correct and should be kept as-is: the KV store supports CLI overrides for these keys, and `parse_buildfile_name` does the right thing by returning the defaults (`BUILD.bazel`, `BUILD`) when the keys are absent.

**No code change needed.** The `buildfiles.rs` test at line 152 uses `testing_parse_with_file_ops` with inline buckconfig data — update the test to use `from_overrides_only` or `LegacyBuckConfig::empty()`.

---

### Step E — Handle bundled-cell `.buckconfig` in `dice/cells.rs`

**Files:** `app/slug_common/src/dice/cells.rs` (line 143 `CellAliasResolverKey::compute`)

This DICE key calls `ctx.get_legacy_config_for_cell(self.0)` and then calls `BuckConfigBasedCells::get_cell_aliases_from_config(&config)` — but only when `!is_bzlmod`. For all bzlmod projects, this branch is already skipped (line 160). The fallback path reads cell aliases from per-cell `.buckconfig`, which is the legacy path.

**Decision:** Leave the `!is_bzlmod` branch in place for now. After Step B, `get_legacy_config_for_cell` returns the CLI-flag-only config, which will have no `[cell_aliases]` entries unless explicitly set via `-c`. That is the correct Q1=B behavior — legacy projects using `.buckconfig`-based cell aliases are no longer supported.

The bundled-cell `.buckconfig` files (`prelude/.buckconfig`, `bazel_tools/.buckconfig`) continue to be embedded and materialized on disk, but they are never parsed at runtime. The `test_bundled_prelude_data` assertion that the prelude cell's `.buckconfig` contains `"prelude = ."` should be **removed** (it tests a file-parsing behavior that will no longer happen).

**No code change to `cells.rs`** — the existing `is_bzlmod` guard handles this. Consider adding a comment noting that the `!is_bzlmod` branch is preserved for legacy non-MODULE.bazel projects only and will be deleted in a future step.

---

### Step F — Delete file-parser modules

**Files to delete:**
- `app/slug_common/src/legacy_configs/parser.rs`
- `app/slug_common/src/legacy_configs/parser/` directory (contains `resolver.rs`)
- `app/slug_common/src/legacy_configs/file_ops.rs`
- `app/slug_common/src/legacy_configs/path.rs`

Also delete or inline into Step B:
- `ExternalPathBuckconfigData` struct
- `ExternalConfigFile` struct
- `LegacyConfigParser` usage in `args.rs`

**Validation:** `cargo build -p slug_common` compiles without errors. Remove or rewrite unit tests in `configs.rs` that use `TestConfigParserFileOps` and `testing::parse`.

---

### Step G — Sweep test `.buckconfig` fixtures

**Files:** ~30 `tests/core/**/.buckconfig` files that contain `[repositories]`/`[cells]`/`[external_cells]` stanzas.

These fixtures are used by tests that do NOT have `MODULE.bazel`. They currently rely on `.buckconfig` for cell resolution. Two options:

1. **Add `MODULE.bazel`** to each test fixture directory (replacing the `[repositories]` section). This is the correct long-term fix and enables these tests to pass without the legacy parser.
2. **Mark affected tests as skipped** if they are already in the "Bucket-C" category (tests that require slug-specific knobs not yet CLI-flag-addressable).

Given the test infrastructure comment "current test status: 861 pass, 152 skip" and the Bucket-C classification in the `.buckconfig` headers, most of these fixtures belong to tests that are already skipped or use slug-specific config that hasn't been migrated. A complete audit of these 30 fixtures against the skip list is required before deletion.

**Active `.buckconfigs` that may be load-bearing:**
- `tests/core/errors/test_errors_data/.buckconfig` — no MODULE.bazel; uses `[repositories]` for multi-cell setup
- `tests/core/cycle_detection/test_cycle_detection_data/.buckconfig` — `[slug] detect_cycles = disabled`, a slug-specific flag
- `tests/core/bxl/test_build_data/.buckconfig` + `test_actions_data/.buckconfig` — `[external_cells] nano_prelude = bundled`; these tests are Bucket-C (RE-dependent or slug-specific)
- `tests/manual_test/.buckconfig` — already empty (just a comment); deletable

---

## Section 4: Risks + Open Questions

### R1: `audit config` iterates all sections via `all_sections()` + `iter()`

`slug_cmd_audit_server/src/config.rs` line 288 calls `cell_config.all_sections()` to dump all config keys. After this plan, `audit config` will only show values set via `-c`. This is correct behavior. No code change needed, but it is a user-visible behavior change that should be documented.

### R2: `--config-file` arg support

The `File` variant of `ResolvedLegacyConfigArg` (reading an external `.buckconfig`-format file via `--config-file`) currently invokes the full file parser. With Q1=B, this feature is effectively dropped. Before removing `ExternalConfigFile`, check whether any CI or user invocations use `--config-file`. If yes, either keep a minimal parser for this case or return an error with a deprecation message.

The `resolve_config_file_arg` function in `args.rs` would need to be removed or made to return an error/warning. The proto field `ConfigType::File` in `slug_cli_proto` may still be sent by old clients — returning an error is safer than silently ignoring.

### R3: `$(config section.key)` variable interpolation in config values

The `parser/resolver.rs` implements `$(config ...)` reference expansion. This is currently used for values set in `.buckconfig` files. CLI flag values do not go through this resolver (they are inserted as `ConfigValue::new_raw_arg` with `resolved_value = Unknown`, then the parser's `finish()` call runs the resolver). After this plan, since there are no parsed files, the resolver is unused. CLI overrides should use `ResolvedValue::Literal` directly (no interpolation). This is safe because no user would reasonably use `$(config ...)` syntax in a `-c` arg.

### R4: Bucket-C test fixtures still need a cell-resolution story

The ~28 `tests/core/**/.buckconfig` files without `MODULE.bazel` are all in skipped or slug-specific tests. They are not immediately blocking for parser deletion, but they are technical debt. The safest approach is to leave the legacy `!has_module_bazel` code path in `cells.rs` compiling (even if the parser is hollowed out — an empty `from_overrides_only` result is returned for these cells, meaning their `[repositories]` stanzas are silently ignored). This will break those tests' cell resolution, but since they are already skipped, it's not a regression.

If any Bucket-C test is passing today via `.buckconfig` cell resolution, it will break. The test audit (task #11) should identify these.

### R5: `slug_re_client` and `scuba` in `CONFIGS_INVISIBLE_TO_DICE`

These two keys in `dice.rs` are referenced in `CONFIGS_INVISIBLE_TO_DICE`. After the parser deletion, these keys can only come from `-c slug_re_client.address=...` CLI flags (which does happen — see `common.rs:1251`). The `is_config_invisible_to_dice` filter is still correct: these RE-client knobs should not cause DICE invalidations. No change needed.

### R6: `LegacyBuckConfig::compare()` in DICE equality checks

The `LegacyBuckConfigForCellKey` equality function calls `config.compare()`, which does a deep value comparison. After this plan, all cells share the same CLI-flag config, so equality is trivially true across cells. This is correct and actually improves DICE cache utilization.

### R7: Is `nodes.rs` item 4 in the audit doc still alive?

Searching `nodes.rs` for `BuckconfigKeyRef` returns no results. The `ConfigPatternCalculation` consumer mentioned in the audit doc appears to have already been drained in a prior commit. Confirmed dead.

---

## Section 5: Estimated Commit Breakdown

### Commit 1 — Add `LegacyBuckConfig::from_overrides_only()` + unit test

- Add constructor in `configs.rs` using `ConfigValue::new_raw_arg` + set `resolved_value = Literal`
- Add a unit test that constructs from `ConfigOverride::flag_no_cell("section.key=value")` and verifies `get()`
- Build green; no behavior change (constructor not yet called from production code)

### Commit 2 — Wire `parse_single_cell_with_file_ops_inner` to `from_overrides_only`

- In `cells.rs` `parse_single_cell_with_file_ops_inner`: replace `LegacyBuckConfig::finish_parse(...)` with `LegacyBuckConfig::from_overrides_only(flag_args_only)`
- Remove `get_project_buckconfig_paths()` call
- Remove `file_ops` parameter from `parse_single_cell_with_file_ops_inner`
- In `parse_with_file_ops_and_options_inner`: remove `get_external_buckconfig_paths()` and `start_parse_for_external_files()` calls; set `started_parse = Vec::new()`
- Verify: `cargo test -p slug_common`; fix any test that provided inline `.buckconfig` data for config values (most should work since defaults are unchanged)

### Commit 3 — Remove `ExternalPathBuckconfigData` + `ExternalConfigFile` + `File` arm of `resolve_config_args`

- In `cells.rs`: `ExternalBuckconfigData.external_path_configs` field becomes `Vec<ResolvedConfigFlag>` or is removed entirely; `get_buckconfig_components` simplified
- In `args.rs`: `ExternalConfigFile` struct removed; `ResolvedConfigFile` and `ResolvedLegacyConfigArg::File` variant: either removed or emits a warning and is skipped
- Delete `LegacyConfigParser::combine()` call in `args.rs`
- Verify: build + run `slug audit config buck2.log_to_json` — still works since it reads from DICE which now has the CLI-flag config

### Commit 4 — Delete `file_ops.rs`, `path.rs`, `parser.rs`, `parser/resolver.rs`

- Remove `mod parser`, `mod file_ops`, `mod path` from `mod.rs` (or wherever modules are declared)
- Remove all `use crate::legacy_configs::file_ops::*` / `parser::*` / `path::*` imports from `cells.rs`, `args.rs`, `configs.rs`
- Remove `DefaultConfigParserFileOps` and `DiceConfigFileOps` from `file_ops.rs` (they were only needed for `parse_*_with_file_ops` entry points)
- Remove `ConfigParserFileOps` trait (used as function parameters in now-deleted functions)
- Verify: `cargo build -p slug_common --all-targets`

### Commit 5 — Clean up test helpers and update unit tests

- In `configs.rs` `testing` module: replace `parse()` / `parse_with_config_args()` with versions using `from_overrides_only` (file data parameter dropped; only config args remain)
- Update or delete `TestConfigParserFileOps`
- Update `buildfiles.rs` unit test (lines 130–188) to use inline `from_overrides_only` or `LegacyBuckConfig::empty()`
- Delete `test_config_file_args_overwrite_config_file` (exercises `--config-file` behavior)
- Delete `test_bundled_prelude_data` `.buckconfig` assertion in `lib.rs`
- Verify: `cargo test --workspace`; pytest baseline holds at 861/152/1

### Commit 6 — Add `tests/manual_test` MODULE.bazel migration (if applicable) + sweep vestigial `[repositories]` from passing test fixtures

- For each passing test that has a `NO_MODULE` `.buckconfig`, either add `MODULE.bazel` or accept that `[repositories]` is silently ignored (cells already registered by bzlmod path don't need `.buckconfig` cell declarations)
- `tests/manual_test/.buckconfig`: already a comment-only file; delete the file
- Verify: pytest baseline unchanged

---

## Appendix: Call-site map for `LegacyBuckConfig` construction (production code only)

| File | How config is obtained | After plan |
|------|----------------------|------------|
| `slug_client_ctx/src/immediate_config.rs:47` | `BuckConfigBasedCells::parse_with_config_args(project_fs, args)` | Still calls this; gets CLI-only config |
| `slug_server/src/ctx.rs:535` | `BuckConfigBasedCells::parse_with_config_args(project_fs, args)` | Same |
| `slug_server/src/daemon/state.rs:292` | `parse_with_config_args(&fs, &[])` | Returns `LegacyBuckConfig::empty()` effectively |
| `slug_cmd_completion_client` (3 sites) | `parse_with_config_args(...)` | Same |
| DICE (`dice.rs:195`) | `parse_single_cell_with_dice` → `from_overrides_only` | Changed in Commit 2 |
| `buildfiles.rs:86` | `get_legacy_config_on_dice` → projection | Unchanged; reads CLI-flag config via DICE |
| `interpreter/buckconfig.rs` | `OpaqueLegacyBuckConfigOnDice.lookup()` | Unchanged; reads CLI-flag config via DICE |
| `slug_configured/src/configuration.rs:104` | `get_legacy_config_property` via DICE | Unchanged |
| `slug_analysis/src/analysis/calculation.rs:466` | `get_legacy_config_property` via DICE | Unchanged |
| `slug_cmd_audit_server/src/config.rs:343,352,368` | `get_legacy_config_for_cell` via DICE | Unchanged; `all_sections()` returns only CLI-set keys |
