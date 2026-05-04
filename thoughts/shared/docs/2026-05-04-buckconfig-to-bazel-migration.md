# Migrating from `.buckconfig` to MODULE.bazel + .bazelrc

Plan 35 retired kuro's legacy `.buckconfig` parser. This guide shows
the per-knob mapping for users coming from a kuro / Buck2 workspace.

After the migration, a kuro workspace looks like:

```
my_workspace/
├── MODULE.bazel        ← cell registration (was [cells]/[cell_aliases])
├── .bazelrc            ← runtime knobs (was [build]/[kuro]/[kuro_re_client]/...)
├── .bazelignore        ← ignored paths (was [project] ignore)
└── BUILD.bazel         ← build files (was BUCK; .buckconfig [buildfile] name retired)
```

The daemon logs a deprecation warning if it sees a `.buckconfig` at the
workspace root. Migrating is a one-time per workspace.

## Quick checklist

- [ ] Move `[cells]` / `[repositories]` / `[cell_aliases]` / `[external_cells]` to `MODULE.bazel`.
- [ ] Move `[build] execution_platforms` to `.bazelrc` (`build --extra_execution_platforms=...`).
- [ ] Move `[parser] target_platform_detector_spec` to `.bazelrc` (`build --platforms=...`).
- [ ] Move `[kuro_re_client]` keys to `.bazelrc` (`--remote_executor` / `--remote_header` / etc.).
- [ ] Move `[kuro] digest_algorithms` to `.bazelrc` (`--digest_function`).
- [ ] Move `[kuro] file_watcher` to `.bazelrc` (`--kuro_file_watcher`).
- [ ] Move `[project] ignore` to `.bazelignore`.
- [ ] Drop `[buildfile]` — kuro accepts `BUILD.bazel` or `BUILD` only.
- [ ] Rename `BUCK` files to `BUILD.bazel`.
- [ ] Rename `TARGETS.fixture` files to `BUILD.bazel`.
- [ ] Delete the `.buckconfig`.

## Section-by-section mapping

### Cells

**Before** (`.buckconfig`):

```ini
[cells]
root = .
toolchains = toolchains

[cell_aliases]
config = prelude
```

**After** (`MODULE.bazel`):

```python
module(name = "root")

bazel_dep(name = "toolchains")
local_path_override(module_name = "toolchains", path = "toolchains")

# `[cell_aliases] config = prelude` becomes a `repo_name=` on the
# bazel_dep that already declares `prelude`:
bazel_dep(name = "prelude", repo_name = "config")
local_path_override(module_name = "prelude", path = "prelude")
```

### External / bundled cells

**Before**:

```ini
[external_cells]
prelude = bundled
```

**After**: drop the section entirely. The bundled cells (`prelude`,
`bazel_tools`, `kuro_builtins`, `local_config_platform`,
`local_config_python`) auto-register via the daemon.

### Execution platform

**Before**:

```ini
[build]
execution_platforms = root//platforms:platforms
```

**After** (`.bazelrc`):

```
build --extra_execution_platforms=root//platforms:platforms
```

The `platforms` rule must return `ExecutionPlatformInfo` directly
(i.e. `[DefaultInfo(), platform]`), not wrapped in
`ExecutionPlatformRegistrationInfo`.

### Default target platform

**Before**:

```ini
[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default
```

**After** (`.bazelrc`):

```
build --platforms=prelude//platforms:default
```

Bazel's `--platforms` propagates via configuration inheritance, so
deps of CLI targets pick up the platform automatically. Per-pattern
routing (`target:cell//...->X` with different X per cell) is not
supported; pick a single workspace-default platform or use per-target
`default_target_platform` attrs.

### Remote execution backend

**Before**:

```ini
[kuro] digest_algorithms = SHA256

[kuro_re_client]
engine_address       = grpcs://remote.buildbuddy.io
action_cache_address = grpcs://remote.buildbuddy.io
cas_address          = grpcs://remote.buildbuddy.io
http_headers         = x-buildbuddy-api-key:$BUILDBUDDY_API_KEY
instance_name        = my-instance
tls                  = true
tls_client_cert      = $CERT_PATH
```

**After** (`.bazelrc`):

```
build --digest_function=SHA256
build --remote_executor=grpcs://remote.buildbuddy.io
build --remote_header=x-buildbuddy-api-key=$BUILDBUDDY_API_KEY
build --remote_instance_name=my-instance
build --tls_client_certificate=$CERT_PATH
```

Notes:
- `--remote_executor` covers all three of `engine_address` /
  `action_cache_address` / `cas_address`.
- `tls` is no longer a flag; kuro infers it from the URL scheme
  (`grpcs://` → TLS, `grpc://` → no TLS).
- `--remote_header` uses `=` as the key/value separator
  (`name=value`), Bazel-compatible. The buckconfig used `:` (`name:value`).
- Env variables in CLI flag values (`$BUILDBUDDY_API_KEY`,
  `$CERT_PATH`) are substituted by the RE client at connection time;
  values flow from the shell environment.

### File watcher

**Before**:

```ini
[kuro]
file_watcher = notify
```

**After** (`.bazelrc`):

```
build --kuro_file_watcher=notify
```

Default is `watchman` on Meta-internal builds, `notify` in OSS. No
Bazel equivalent.

### Project ignores

**Before**:

```ini
[project]
ignore = node_modules,build/,target/
```

**After** (`.bazelignore`):

```
node_modules
build
target
```

One path per line, no globs, project-relative.

### Build files

The `[buildfile] name` knob is retired. Kuro accepts `BUILD.bazel`
(preferred) or `BUILD` only. Rename `BUCK` files to `BUILD.bazel`:

```sh
find . -name BUCK -not -path '*/buck-out/*' | while read f; do
  git mv "$f" "$(dirname $f)/BUILD.bazel"
done
```

For workspaces using `TARGETS.fixture` (kuro test-fixture convention)
or any other custom name: same migration. The default search order
(`BUILD.bazel`, `BUILD`) is the only one kuro supports.

## Knobs that went away

These had no live consumers in this repo and were dropped without a
replacement. If your workspace relied on any of them, file an issue.

- `[oss] internal_cell`, `stripped_root_dirs` — Meta-internal Buck1 vestiges.
- `[rust] default_edition` — kuro's own bootstrap config; not a build-system concern.
- `[deprecated_config]` — a Buck2-only opt-in mechanism.
- `[kuro] allow_eden_io` — Eden integration is `#[cfg(fbcode_build)]`-gated; OSS path now hardcoded to `false`.
- `[buck2] error_on_dep_only_incompatible{,_excluded}` — the gate is gone; behaviour is the default (no errors).

## Knobs surviving as kuro-specific extensions

- `read_config(section, key)` Starlark API — still works, but reads
  from CLI overrides (`-c section.key=value`) only. The function's
  buckconfig file source is gone.
- `config_setting(values = {"section.key": "value"})` — same: matches
  CLI overrides only. Prefer `define_values` (Bazel-shape) for new
  code.

## Per-knob references

| Old `.buckconfig` knob | New location | Notes |
|---|---|---|
| `[cells] X = path` | `bazel_dep + local_path_override` | `MODULE.bazel` |
| `[cell_aliases] X = Y` | `bazel_dep(name=Y, repo_name=X, ...)` | `MODULE.bazel` |
| `[external_cells] X = bundled` | drop | auto-registered |
| `[build] execution_platforms` | `--extra_execution_platforms` | `.bazelrc` |
| `[parser] target_platform_detector_spec` | `--platforms` | `.bazelrc` |
| `[kuro] digest_algorithms` | `--digest_function` | `.bazelrc` |
| `[kuro] file_watcher` | `--kuro_file_watcher` | `.bazelrc` |
| `[kuro_re_client] engine_address` | `--remote_executor` | `.bazelrc` |
| `[kuro_re_client] action_cache_address` | `--remote_executor` (covers all three) | `.bazelrc` |
| `[kuro_re_client] cas_address` | `--remote_executor` (covers all three) | `.bazelrc` |
| `[kuro_re_client] http_headers` | `--remote_header=K=V` | `.bazelrc` |
| `[kuro_re_client] instance_name` | `--remote_instance_name` | `.bazelrc` |
| `[kuro_re_client] tls` | infer from URL scheme | drop |
| `[kuro_re_client] tls_client_cert` | `--tls_client_certificate` | `.bazelrc` |
| `[project] ignore` | one line per path | `.bazelignore` |
| `[buildfile] name` | drop | rename files to `BUILD.bazel` |

## See also

- The plan: `thoughts/shared/plans/kuro-bazel-subplans/35-buckconfig-removal.md`
- The audit that drove the dispositions:
  `thoughts/shared/research/2026-05-01-buckconfig-knob-disposition.md`
- The bucket classification for test fixtures:
  `thoughts/shared/research/2026-05-04-fixture-bucket-classification.md`
