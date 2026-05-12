# Plan 22: CLI flag compatibility with Bazel

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Discovered during Plan 18 (BEP parity) end-to-end testing against
> `@llvm-project//llvm:Demangle --config=remote`: the `--config=remote` flag
> reached slug's buck-style config-override parser and errored with
> `Could not find equals sign in pair 'remote'`. That exposed a broader set
> of CLI-layer parity gaps this plan tracks.

## Scope

Make Slug's command line accept Bazel-shape flags where the semantics have
a clear mapping, without breaking the existing Buck2 flag surface. Each
phase is a targeted, independently landable fix.

## Phases

### 22.1 `--config` shape-based disambiguation (DONE 2026-04-24)

Landed as `filter_bazelrc_only_cli_flags` + `config_flag_value` in
`app/slug_client_ctx/src/bazelrc.rs`. Bare-identifier `--config=NAME` is
stripped from clap's view (after `find_active_configs` records it as a
bazelrc selector); buck-shaped `--config=SECTION.KEY=VALUE` is passed
through unchanged. `find_active_configs` applies the same shape check so
buck overrides never activate bazelrc sections.

Seven new unit tests (`bazelrc_config_is_bare_identifier`,
`buck_config_value_has_dot_and_equals`,
`find_active_configs_accepts_bazelrc_selectors_only`,
`filter_strips_bazelrc_style_config_preserves_buck_style`,
`filter_handles_two_arg_config_form`,
`filter_strips_enable_platform_specific_config`,
`filter_preserves_bare_short_c_flag`,
`filter_ignores_trailing_bare_config_without_value`).

Verified: `slug build @llvm-project//llvm:Demangle --config=remote` now
parses cleanly and proceeds to analysis; the remaining failure is the
`buildbuddy_toolchain` module-extension gap (tracked separately — will
likely spawn Plan 23).

**Problem.** `--config` is overloaded:

- **Bazel** `--config=NAME` → activate `build:NAME` lines in `.bazelrc`.
- **Buck2** `--config=SECTION.KEY=VALUE` (or `-c …`) → override a
  buckconfig value. Requires `SECTION.KEY=VALUE` shape (enforced by
  `legacy_configs::parse_config_section_and_key`).

Both share the `--config` long name. Current state:

- `bazelrc.rs::find_active_configs` reads `--config=X` from CLI args to
  pick bazelrc sections, but does not remove the flag from the arg list.
- `CommonBuildConfigurationOptions::config_values` (clap, short `-c`) sees
  every `--config=X` and tries to parse `X` as `SECTION.KEY=VALUE`.
- Result: Bazel-style selectors fail at clap time.

**Prior attempt.** A stub stripped every `--config=X` from CLI args after
`find_active_configs` had recorded them. That also dropped buck-style
overrides, which is wrong.

**Proper fix — shape check.** Buck-style values always contain both `.`
and `=`; Bazel config names are bare identifiers. The two grammars do not
collide. Walk CLI args once:

- `--config=X` where `X` contains neither `.` nor `=` → bazelrc selector.
  Record in `active_configs`, strip from args before clap parses.
- `--config=X` where `X` contains `.` or `=` → buck override.
  Leave in place.
- `-c X` (short) → always buck override. Untouched.
- `--config X` (two-arg form) → apply the same shape check to the value.

Apply the same discriminator inside `find_active_configs`: only bare
identifiers enter `active_configs`, so a buck-style
`--config=buck2.debug=true` never activates a bazelrc section.

**Risks.**

- A bazelrc config name with `.` or `=` would be misrouted to buck's
  parser and fail. Real-world names are simple identifiers (`remote`,
  `ci`, `linux`, `generic_clang`); incidence ~zero. Document the
  constraint; the existing "Could not find section separator" error is
  self-explanatory if hit.
- Audit existing slug usages of `--config=` to confirm none relied on the
  accidental buck-overrides-via-injected-config behavior:
  `grep -rn '"--config' app/ tests/`.

**Tests.** Add to `bazelrc.rs` `#[cfg(test)]`:

- `--config=remote` → `active_configs=["remote"]`, arg removed from clap
  view.
- `--config=buck2.debug=true` → `active_configs=[]`, arg passes through.
- `-c buck2.debug=true` → `active_configs=[]`, arg passes through.
- `--config=remote --config=buck2.debug=true` (both orders) → both
  honored.
- Bazelrc-internal `build --config=stable` → `active_configs=["stable"]`
  still picked up.

**Acceptance criteria.**

1. `slug build //x --config=remote` parses cleanly. (It then fails at
   Plan 22.2 / module-extension territory — that is the next fix, not
   this phase's concern.)
2. `slug build //x --config=cell.foo=bar` continues to apply the buck
   override.
3. `slug build //x --config=remote --config=cell.foo=bar` applies both.
4. New bazelrc unit tests pass; `cargo test -p slug_client_ctx` green.

**Est. effort.** ~2-3 hours. Logic change is ~30 lines in
`app/slug_client_ctx/src/bazelrc.rs`; the rest is tests.

---

### 22.2 `--remote_header` / `--remote_cache_header` / `--remote_exec_header` plumbing (OPEN)

**Current state.** Accepted-as-stub so llvm-project's `.bazelrc` (which
has `common --remote_header=x-buildbuddy-api-key=…`) stops erroring out.
Not wired to the RE client.

**Out of scope here.** Actually plumbing custom gRPC metadata into slug's
remote-execution client and remote cache requires touching
`slug_re_configuration` + `slug_execute_impl`. Defer until a companion
remote-execution plan exists.

---

### 22.3 Remaining `common --*` flags from llvm-project's `.bazelrc` (DEFERRED)

`--enable_platform_specific_config`, `--legacy_external_runfiles`,
`--guard_against_concurrent_changes`, `--incompatible_no_implicit_file_export`,
`--incompatible_disallow_empty_glob`, `--build_runfile_links`,
`--process_headers_in_dependencies`, `--dynamic_mode`, `--strip`,
`--features`, `--force_pic`.

Some are already accepted-but-ignored; others trigger "unexpected
argument" errors that bubble up under `--config=generic_clang` etc.
Triage incrementally as they block concrete workflows, rather than as a
single sweep.

---

### 22.4 Cell-aware `--@cell//pkg:flag=value` build-setting plumbing (DONE 2026-05-06)

Landed in:
- `app/slug_build_api/src/interpreter/rule_defs/context.rs` —
  `ctx.build_setting_value` now constructs `@<cell>//pkg:name` from
  `target.pkg().cell_name()` and falls back to the cell-less form for
  legacy callers.
- `app/slug_configured/src/target_platform_resolution.rs` — new
  `canonicalize_cell_alias` helper rewrites `@<apparent>//pkg:name`
  CLI flags through the root cell's `CellAliasResolver` before storing
  them in `cfg.build_settings`. `apply_cli_build_settings` is now
  async and threads `DiceComputations` to fetch the resolver.

New unit test `cell_alias_is_canonicalized_at_storage_time` exercises
the rules_rust shape (`--@rules_rust//cargo/settings:experimental_symlink_execroot=true`
with apparent→canonical mapping `rules_rust → rules_rust+0.69.0`).

Verified end-to-end: zeromatter `slug build //sdk:sdk_contents` now puts
`RULES_RUST_SYMLINK_EXEC_ROOT=1` in `cargo_build_script_run` action
envs. Build advances past analysis into execution and fails at a
separate downstream issue (`Failed to delete symlink … CHANGELOG.md`)
inside rules_rust's `cargo_build_script_runner` — outside the scope
of this phase.

`cargo test -p slug_node -p slug_common -p slug_configured -p slug_action_impl --lib`
all green (7 + 83 + 5 + 47 = 142 tests pass).

Original write-up below for context:

> Discovered while investigating zeromatter `//sdk:sdk_contents`'s
> `lib/units:build_script` panic. Root cause: rules_rust's `.bazelrc`
> sets `build --@rules_rust//cargo/settings:experimental_symlink_execroot=true`,
> which should put `RULES_RUST_SYMLINK_EXEC_ROOT=1` in the build-script
> action env. In slug the env var is absent: the CLI flag is parsed and
> stored, but the analysis-time lookup never finds it because cell
> prefixes are dropped on the lookup side (and not canonicalized on
> the storage side).

#### Symptom

`bool_flag` / `string_flag` / `int_flag` rules from
`@bazel_skylib//rules:common_settings.bzl` always observe the
`build_setting_default`, regardless of any
`--@cell//pkg:flag=value` override on the CLI or in `.bazelrc`. Any
rule that gates behaviour on `ctx.attr._x[BuildSettingInfo].value` —
including rules_rust's `experimental_symlink_execroot` — sees the
default and silently does the wrong thing.

#### Storage path (works)

1. `app/slug_client_ctx/src/bazelrc.rs:441-468` — `normalize_args`
   peels `--@<cell>//pkg:flag=value` out of CLI args into `STARLARK_FLAGS`.
2. `app/slug_client_ctx/src/client_ctx.rs:258` — propagated to daemon
   via `client_context.starlark_flags`.
3. `app/slug_server/src/ctx.rs:342` — daemon calls `set_starlark_flags`.
4. `app/slug_build_api/src/interpreter/rule_defs/build_config.rs:347-365`
   — stored in process-global map keyed on the raw CLI label
   (`@<cell>//pkg:flag`).
5. `app/slug_configured/src/target_platform_resolution.rs:128-140` —
   folded into `cfg.build_settings` via
   `BuildSettingLabel::from_bazel_label`. Cell name from the CLI alias
   is preserved verbatim through `TargetLabel::testing_parse`
   (`app/slug_core/src/target/label/label.rs:222-244`).

#### Lookup path (broken)

`app/slug_build_api/src/interpreter/rule_defs/context.rs:958-984` —
`ctx.build_setting_value`:

```rust
let pkg_path = target.pkg().cell_relative_path().as_str();
let target_name = target.name().as_str();
let label_str = if pkg_path.is_empty() {
    format!("//:{}", target_name)
} else {
    format!("//{}:{}", pkg_path, target_name)
};
```

The cell prefix is **omitted**.
`BuildSettingLabel::from_bazel_label("//pkg:flag")`
(`app/slug_core/src/configuration/build_setting.rs:52-69`) routes
unprefixed labels to a synthetic `@slug_settings` cell. Storage key is
`@<cell>//pkg:flag`; lookup key is `@slug_settings//pkg:flag` — miss.

The same lookup also tries the raw process-global map at
`context.rs:977` via `get_starlark_flag(&label_str)` with the same
cell-less string — also a miss.

#### Secondary concern: bzlmod cell-alias canonicalization

Even after the lookup includes a cell prefix, the CLI alias
(`rules_rust`) may not match the canonical cell where the build_setting
target actually loads (e.g. `rules_rust+0.69.0` if bzlmod has
canonicalized it, or some apparent-name aliased form). The two paths
must agree on the cell-name basis, which means the storage side has to
resolve CLI cell aliases through bzlmod's apparent-name table — see
`collect_transitive_repo_aliases` in
`app/slug_common/src/legacy_configs/cells.rs` and the BazelDep
`apparent_name()` machinery in `app/slug_bzlmod/src/types.rs`.

#### Fix shape

1. **Lookup-side cell inclusion** —
   `app/slug_build_api/src/interpreter/rule_defs/context.rs:961-984`:
   build the lookup label as
   `@{cell}//{pkg}:{name}` using `target.pkg().cell_name().as_str()`
   (`PackageLabel::cell_name()` exists at
   `app/slug_core/src/package.rs:155`). Same fix for the
   `get_starlark_flag` fallback at `context.rs:977`.

2. **Storage-side alias canonicalization** —
   `app/slug_configured/src/target_platform_resolution.rs:128-140` and
   `app/slug_build_api/src/interpreter/rule_defs/build_config.rs:347-365`:
   resolve the CLI cell alias through the active bzlmod apparent-name
   table before keying. `BuildSettingLabel::from_bazel_label` currently
   funnels unprefixed labels through a synthetic `@slug_settings` cell
   (`app/slug_core/src/configuration/build_setting.rs:48-65`) — this
   was added as a stub; the canonical-cell routing in Plan 37 plus the
   apparent-name table are the right source of truth here.

3. **Synthetic-cell follow-up** — once both sides go through the real
   resolver, audit whether `@slug_settings` is still needed for any
   call sites (transitions declaring inputs/outputs as raw
   `"//command_line_option:..."` strings, etc.) or can be retired.

#### Tests

- Unit test in
  `app/slug_build_api/src/interpreter/rule_defs/context.rs` (or a
  dedicated test crate): analyze a `bool_flag` target in a non-root
  cell with a CLI override; assert
  `ctx.attr._x[BuildSettingInfo].value` reflects the override.
- Integration test under `tests/core/` that reproduces the
  rules_rust `experimental_symlink_execroot` shape:
  `--@cell//pkg:flag=true` → `bool_flag` target's
  `BuildSettingInfo.value == True`. Run with the CLI alias and with
  the canonical cell name; both should match.
- Round-trip test that the apparent-name resolver agrees on both
  sides: storing under `--@apparent//...` and looking up via the
  canonical cell name (and vice versa) both succeed.

#### Acceptance criteria

1. `slug build` of a target whose analysis reads
   `BuildSettingInfo.value` from a CLI-overridden flag in a
   non-root cell observes the override (not the default).
2. Specifically: `cd zeromatter && slug build //sdk:sdk_contents`
   produces a `lib/units:build_script` action env containing
   `RULES_RUST_SYMLINK_EXEC_ROOT=1`. (Other blockers in that build
   are out of scope here — the verification is the env var, not
   the full build succeeding.)
3. `cargo test -p slug_configured -p slug_build_api -p slug_common --lib`
   stays green.
4. The `starlark_flags_land_in_build_settings` test in
   `target_platform_resolution.rs` is extended to exercise the
   lookup path, not just the storage path.

#### Out of scope

- Rebuilding the `@slug_settings` synthetic-cell mechanism — keep it
  for now, just route real cells correctly first.
- Transition output overrides (writing `--@cell//pkg:flag=value` from
  a Starlark transition). Storage side already handles arbitrary
  string keys via `set_starlark_flag`; that path will inherit the
  same canonicalization fix.

#### Effort

Medium. Lookup fix is ~10 lines plus a test. Cell-alias
canonicalization on the storage side requires plumbing the apparent-
name resolver into `apply_cli_build_settings_with` and
`set_starlark_flags` (currently both run before any cell context is
materialized). Likely 1-2 days including the integration test.
