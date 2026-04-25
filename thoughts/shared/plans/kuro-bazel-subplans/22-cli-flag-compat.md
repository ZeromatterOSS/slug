# Plan 22: CLI flag compatibility with Bazel

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Discovered during Plan 18 (BEP parity) end-to-end testing against
> `@llvm-project//llvm:Demangle --config=remote`: the `--config=remote` flag
> reached kuro's buck-style config-override parser and errored with
> `Could not find equals sign in pair 'remote'`. That exposed a broader set
> of CLI-layer parity gaps this plan tracks.

## Scope

Make Kuro's command line accept Bazel-shape flags where the semantics have
a clear mapping, without breaking the existing Buck2 flag surface. Each
phase is a targeted, independently landable fix.

## Phases

### 22.1 `--config` shape-based disambiguation (DONE 2026-04-24)

Landed as `filter_bazelrc_only_cli_flags` + `config_flag_value` in
`app/kuro_client_ctx/src/bazelrc.rs`. Bare-identifier `--config=NAME` is
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

Verified: `kuro build @llvm-project//llvm:Demangle --config=remote` now
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
- Audit existing kuro usages of `--config=` to confirm none relied on the
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

1. `kuro build //x --config=remote` parses cleanly. (It then fails at
   Plan 22.2 / module-extension territory — that is the next fix, not
   this phase's concern.)
2. `kuro build //x --config=cell.foo=bar` continues to apply the buck
   override.
3. `kuro build //x --config=remote --config=cell.foo=bar` applies both.
4. New bazelrc unit tests pass; `cargo test -p kuro_client_ctx` green.

**Est. effort.** ~2-3 hours. Logic change is ~30 lines in
`app/kuro_client_ctx/src/bazelrc.rs`; the rest is tests.

---

### 22.2 `--remote_header` / `--remote_cache_header` / `--remote_exec_header` plumbing (OPEN)

**Current state.** Accepted-as-stub so llvm-project's `.bazelrc` (which
has `common --remote_header=x-buildbuddy-api-key=…`) stops erroring out.
Not wired to the RE client.

**Out of scope here.** Actually plumbing custom gRPC metadata into kuro's
remote-execution client and remote cache requires touching
`kuro_re_configuration` + `kuro_execute_impl`. Defer until a companion
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
