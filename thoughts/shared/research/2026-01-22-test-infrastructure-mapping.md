# Test Infrastructure Mapping: Buck2 vs Bazel

## Executive Summary

This document maps test concepts between the Buck2 (Kuro fork) and Bazel repositories to guide test migration. Our goal is to adopt Bazel semantics while leveraging the existing Buck2 test infrastructure (pytest-based Python tests with async support).

## Test Infrastructure Comparison

### Buck2/Kuro Test Framework

| Aspect | Details |
|--------|---------|
| **Primary Language** | Python (pytest with pytest-asyncio) |
| **Test Location** | `tests/core/`, `tests/e2e/` |
| **Unit Tests** | Rust tests in `app/kuro_*_tests/` |
| **Test Framework** | `@buck_test()` decorator, async/await pattern |
| **Fixtures** | `test_*_data/` directories with `.buckconfig`, `TARGETS.fixture` |
| **Golden Files** | `*.golden` files with hash sanitization |
| **Test Execution** | Via `buck.build()`, `buck.test()`, `buck.query()` async methods |
| **Assertion Helpers** | `expect_failure()`, `golden()`, custom sanitizers |

### Bazel Test Framework

| Aspect | Details |
|--------|---------|
| **Primary Language** | Java (JUnit) + Shell (bash) + Python (limited) |
| **Test Location** | `src/test/java/`, `src/test/shell/` |
| **Unit Tests** | Java JUnit tests with `BuildViewTestCase` base class |
| **Test Framework** | Shell `unittest.bash`, Starlark `skylib` |
| **Fixtures** | Dynamically created via `cat > BUILD <<EOF` |
| **Golden Files** | Less common, uses `expect_log` pattern matching |
| **Test Execution** | Direct `bazel build/test/query` shell commands |
| **Assertion Helpers** | `expect_log()`, `expect_not_log()`, `fail()` |

### Key Architectural Differences

| Feature | Buck2 | Bazel |
|---------|-------|-------|
| Build Files | `BUCK`, `TARGETS` | `BUILD.bazel`, `BUILD` |
| Workspace Config | `.buckconfig`, cells | `MODULE.bazel`, bzlmod |
| Dependency Management | Cells, external cells | bzlmod, registries |
| Extension Language | BXL | Aspects (partial overlap) |
| Attribute API | `attrs.*` | `attr.*` |
| Rule Definition | `impl` param | `implementation` param |
| Visibility | `"PUBLIC"` | `"//visibility:public"` |
| Target Patterns | `//pkg:` | `//pkg:all` |

---

## Test Category Mapping

### Legend
- **KEEP+UPDATE**: Buck2 test exists, covers concept in Bazel - update syntax/semantics
- **DELETE**: Buck2-specific concept not in Bazel
- **ADD**: Bazel concept not in Buck2 - create new test
- **PRESERVE**: Keep as-is (shared concept, similar implementation)

---

## Category 1: Starlark Interpreter Tests

**Buck2 Location**: `tests/core/interpreter/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_attr_default_coercion.py` | Attribute default values | KEEP+UPDATE | Change `attrs.*` to `attr.*` |
| `test_callstack_size.py` | Starlark stack limits | PRESERVE | Same concept |
| `test_cancellation.py` | Interpreter cancellation | PRESERVE | Same concept |
| `test_cpu_instruction_count.py` | Resource limits | PRESERVE | Same concept |
| `test_deprecated_config.py` | Deprecation warnings | KEEP+UPDATE | Bazel has different deprecation patterns |
| `test_load_json.py` | JSON loading | PRESERVE | Both support `json.decode()` |
| `test_load_toml.py` | TOML loading | DELETE | Bazel doesn't have TOML support |
| `test_missing_source_file.py` | Source file errors | PRESERVE | Same concept |
| `test_package_file_alt_name.py` | Package file names | KEEP+UPDATE | `PACKAGE.bzl` vs Buck's PACKAGE |
| `test_package_file_package_values.py` | Package-level values | KEEP+UPDATE | Bazel uses `package()` function |
| `test_package_file_visibility.py` | Default visibility | KEEP+UPDATE | Change visibility syntax |
| `test_package_values_cross_cell.py` | Cross-cell packages | DELETE | Bazel uses bzlmod, not cells |
| `test_package_values_missing_buck_file.py` | Missing build files | KEEP+UPDATE | Change to BUILD.bazel |
| `test_peak_allocated_bytes*.py` | Memory limits | PRESERVE | Same concept |
| `test_prelude_typecheck.py` | Type checking | PRESERVE | Kuro keeps type annotations |
| `test_print.py` | Print function | PRESERVE | Same `print()` function |
| `test_read_root_config.py` | Config reading | DELETE | Uses `.buckconfig`, replace with bzlmod |
| `test_relative_paths.py` | Path handling | PRESERVE | Same concept |
| `test_sub_packages.py` | Subpackage handling | PRESERVE | Same concept |

**Bazel Tests to ADD**:
- `attr.*` function tests (from `AttributeTest.java`)
- `native.*` module tests (from `StarlarkNativeModule.java`)
- `rule(implementation=...)` syntax tests
- `provider()` definition tests
- Module-level `visibility` tests

---

## Category 2: Analysis Phase Tests

**Buck2 Location**: `tests/core/analysis/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_analysis_action_ids_unique.py` | Action ID uniqueness | PRESERVE | Same concept |
| `test_analysis_queries.py` | Analysis queries | KEEP+UPDATE | Different query syntax |
| `test_cmd_args.py` | Command arguments | KEEP+UPDATE | Bazel uses `ctx.actions.args()` |
| `test_template_placeholder.py` | Template expansion | KEEP+UPDATE | Bazel has `ctx.expand_template()` |

**Bazel Tests to ADD** (from `StarlarkRuleContextTest.java`):
- `ctx.attr` access tests
- `ctx.file` / `ctx.files` tests
- `ctx.executable` tests
- `ctx.outputs` tests
- `ctx.actions.run()` tests
- `ctx.actions.run_shell()` tests
- `ctx.actions.write()` tests
- `ctx.actions.declare_file()` tests
- `ctx.actions.declare_directory()` tests
- `ctx.actions.symlink()` tests
- `ctx.runfiles()` tests

---

## Category 3: Build Command Tests

**Buck2 Location**: `tests/core/build/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_action_error_handler_types.py` | Error handling | PRESERVE | Same concept |
| `test_build_configured.py` | Configured builds | KEEP+UPDATE | Bazel config syntax differs |
| `test_build_modifiers*.py` | Build modifiers | DELETE | Buck2-specific concept |
| `test_build_output.py` | Build output | KEEP+UPDATE | Output paths differ |
| `test_build_report*.py` | Build reports | KEEP+UPDATE | Different report format |
| `test_build_response.py` | Build response | PRESERVE | Same concept |
| `test_build_root_executable.py` | Root executables | PRESERVE | Same concept |
| `test_build_rule_type_name_logging.py` | Rule logging | PRESERVE | Same concept |
| `test_build_skip_incompatible_targets.py` | Incompatible targets | KEEP+UPDATE | Bazel uses `target_compatible_with` |
| `test_build_system_info.py` | System info | PRESERVE | Same concept |
| `test_build_with_transition.py` | Transitions | KEEP+UPDATE | Bazel transition syntax differs |
| `test_critical_path.py` | Critical path | PRESERVE | Same concept |
| `test_detailed_aggregated_metrics.py` | Metrics | PRESERVE | Same concept |
| `test_error_categorization.py` | Error categories | PRESERVE | Same concept |
| `test_external_buckconfigs.py` | External configs | DELETE | Replace with bzlmod tests |
| `test_modify.py` | Modify command | DELETE | Buck2-specific |
| `test_nested_subtargets.py` | Subtargets | DELETE | Buck2-specific concept |
| `test_out_flag.py` | Output flag | PRESERVE | `--output` in both |
| `test_overall_timeout.py` | Timeout handling | PRESERVE | Same concept |
| `test_paranoid.py` | Paranoid mode | DELETE | Buck2-specific |
| `test_plugins.py` | Plugins | DELETE | Buck2-specific |
| `test_skip_missing.py` | Skip missing | PRESERVE | Same concept |
| `test_symlinks.py` | Symlink handling | PRESERVE | Same concept |
| `test_target_aliases.py` | Target aliases | KEEP+UPDATE | Bazel uses `alias()` rule |
| `test_uncategorized.py` | Uncategorized | Review | May contain Buck2-specific |
| `test_unhashed_outputs.py` | Unhashed outputs | DELETE | Buck2-specific |
| `test_universe.py` | Target universe | PRESERVE | Same concept |

**Bazel Tests to ADD** (from shell integration tests):
- `--keep_going` behavior tests
- `--output_base` tests
- `--sandbox` mode tests
- `--remote_cache` tests
- `bazel build //...` pattern tests
- Multi-config build tests

---

## Category 4: Query Tests

**Buck2 Location**: `tests/core/query/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_buildfiles.py` | buildfiles() query | KEEP+UPDATE | Same function name |
| `test_target_call_stacks.py` | Target call stacks | PRESERVE | Same concept |
| `test_target_configuration_toolchain_deps_traversal.py` | Toolchain deps | KEEP+UPDATE | Different toolchain model |

**Bazel Tests to ADD** (from `bazel_query_test.sh` - 50+ tests):
- `deps()` function tests
- `rdeps()` function tests
- `allpaths()` function tests
- `somepath()` function tests
- `kind()` function tests
- `filter()` function tests
- `attr()` function tests
- Set operations (`+`, `-`, `^`) tests
- `--output=label|build|xml|json|proto` tests
- `--universe_scope` tests
- `loadfiles()` function tests
- Cycle detection in queries
- Large query performance tests

---

## Category 5: Configuration Tests

**Buck2 Location**: `tests/core/configurations/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_configuration_dep_uquery_correctness.py` | Config query | KEEP+UPDATE | Different config model |
| `test_configuration_rule_unbound.py` | Unbound rules | PRESERVE | Same concept |
| `test_evaluation_order.py` | Eval order | PRESERVE | Same concept |
| `test_exec_modifier.py` | Exec modifier | DELETE | Buck2-specific |
| `test_nested_select_coercion.py` | select() nesting | KEEP+UPDATE | Same concept, syntax may differ |
| `test_platform_via_alias.py` | Platform aliases | PRESERVE | Same concept |
| `test_platform_wrong_label.py` | Platform errors | PRESERVE | Same concept |
| `test_select_buckconfig.py` | select() with config | DELETE | Replace with select() on flags |
| `test_select_concat.py` | select() concat | PRESERVE | Same concept |
| `test_select_refine.py` | select() refinement | PRESERVE | Same concept |
| `test_subtarget_configuration_dep.py` | Subtarget config | DELETE | Buck2-specific |
| `test_target_incompatible.py` | Incompatibility | KEEP+UPDATE | Bazel uses `target_compatible_with` |
| `test_target_platforms_arg.py` | Platform arg | KEEP+UPDATE | `--platforms` flag |
| `test_toolchain_overconfiguration.py` | Toolchain config | KEEP+UPDATE | Different toolchain model |
| `test_unified_constraint.py` | Constraints | KEEP+UPDATE | Bazel constraint syntax |

**Bazel Tests to ADD** (from `starlark_configurations_test.sh`):
- Starlark build settings tests
- Starlark transitions tests
- `--platforms` flag tests
- `constraint_setting` / `constraint_value` tests
- `config_setting()` tests
- `--define` and `--copt` flag tests

---

## Category 6: Executor/Execution Tests

**Buck2 Location**: `tests/core/executor/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_build_id_env_var.py` | Build ID env | PRESERVE | Same concept |
| `test_cache_uploads.py` | Cache uploads | KEEP+UPDATE | Bazel remote cache API |
| `test_cancellation.py` | Execution cancel | PRESERVE | Same concept |
| `test_content_based_paths.py` | Content paths | PRESERVE | Same concept |
| `test_dep_files.py` | Dep files | KEEP+UPDATE | Bazel uses `.d` files too |
| `test_executor_with_dependencies.py` | Exec deps | PRESERVE | Same concept |
| `test_executor_with_re_gang_workers.py` | RE workers | DELETE | Meta-specific |
| `test_hash_all_commands.py` | Command hashing | PRESERVE | Same concept |
| `test_hybrid_executor.py` | Hybrid exec | DELETE | Buck2-specific concept |
| `test_incremental_actions.py` | Incremental | PRESERVE | Same concept |
| `test_materialization_for_failed_actions.py` | Failed materialize | PRESERVE | Same concept |
| `test_no_executor.py` | No executor | PRESERVE | Same concept |
| `test_output_cleanup.py` | Output cleanup | PRESERVE | Same concept |
| `test_outputs_ordering.py` | Output ordering | PRESERVE | Same concept |
| `test_remote_execution.py` | Remote exec | KEEP+UPDATE | Bazel RE API |

**Bazel Tests to ADD**:
- Sandbox isolation tests
- Process wrapper tests
- Persistent worker tests
- Local execution strategy tests
- Dynamic execution tests

---

## Category 7: Cell/External Cell Tests

**Buck2 Location**: `tests/core/cells/`, `tests/core/external_cells/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_buckconfig_paths.py` | Config paths | DELETE | Replace with bzlmod |
| `test_cell_aliases.py` | Cell aliases | DELETE | Replace with bzlmod repos |
| `test_empty_buckconfig.py` | Empty config | DELETE | Replace with bzlmod |
| `test_file_watcher_resolution.py` | File watcher | PRESERVE | Same concept |
| `test_ignore_state_invalidation.py` | Invalidation | PRESERVE | Same concept |
| `test_reuse_current_config.py` | Config reuse | DELETE | Buck2-specific |
| `test_root_cell_command.py` | Root cell | DELETE | Replace with workspace tests |
| `test_bundled.py` | Bundled cells | DELETE | Replace with bzlmod |
| `test_git.py` | Git cells | DELETE | Replace with `git_repository` |
| `test_in_subdir.py` | Subdir cells | DELETE | Replace with bzlmod |
| `test_prelude.py` | Prelude loading | KEEP+UPDATE | Bazel has built-in rules |

**Bazel Tests to ADD** (bzlmod - critical):
- `MODULE.bazel` parsing tests
- `module()` directive tests
- `bazel_dep()` directive tests
- `use_extension()` tests
- `use_repo()` tests
- MVS resolution algorithm tests
- `local_path_override()` tests
- `single_version_override()` tests
- `multiple_version_override()` tests
- `archive_override()` tests
- `git_override()` tests
- BCR registry client tests
- `MODULE.bazel.lock` generation tests
- Lockfile validation tests
- Module extension evaluation tests

---

## Category 8: BXL Tests

**Buck2 Location**: `tests/core/bxl/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| All BXL tests | BXL scripting | PRESERVE | Keep for future tooling |

**Note**: BXL is being preserved for developer tooling (compile_commands.json, IDE integration). These tests should be kept but are not priority for Bazel compatibility.

---

## Category 9: Test Runner Tests

**Buck2 Location**: `tests/core/test/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_build_report.py` | Test reports | KEEP+UPDATE | Different report format |
| `test_content_based_paths.py` | Content paths | PRESERVE | Same concept |
| `test_internal_runner.py` | Internal runner | DELETE | Buck2-specific |
| `test_listing.py` | Test listing | PRESERVE | Same concept |
| `test_local_resources.py` | Local resources | PRESERVE | Same concept |
| `test_modifiers.py` | Test modifiers | DELETE | Buck2-specific |
| `test_platform_resolution.py` | Platform resolution | KEEP+UPDATE | Bazel platform syntax |
| `test_selection.py` | Test selection | PRESERVE | Same concept |
| `test_skip_incompatible_targets.py` | Skip incompatible | PRESERVE | Same concept |
| `test_startup.py` | Test startup | PRESERVE | Same concept |
| `test_test_rule_type_name_logging.py` | Rule logging | PRESERVE | Same concept |

**Bazel Tests to ADD** (from `bazel_test_test.sh`):
- `--test_output=all|errors|summary` tests
- `--test_filter` tests
- Test sharding tests
- Flaky test handling tests
- Test timeout tests
- `--runs_per_test` tests
- Test XML output tests
- `--run_under` tests

---

## Category 10: Run Command Tests

**Buck2 Location**: `tests/core/run/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_build_id_env.py` | Build ID env | PRESERVE | Same concept |
| `test_run_modifiers.py` | Run modifiers | DELETE | Buck2-specific |
| `test_run.py` | Run command | PRESERVE | Same `run` command |
| `test_universe.py` | Target universe | PRESERVE | Same concept |

**Bazel Tests to ADD** (from `run_test.sh`):
- `bazel run` with arguments tests
- `--run_under` tests
- Script vs binary run tests

---

## Category 11: Transitive Sets / Depset Tests

**Buck2 Location**: `tests/core/transitive_sets/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| `test_transitive_sets.py` | Transitive sets | KEEP+UPDATE | Bazel calls this `depset` |

**Bazel Tests to ADD** (from `Depset.java` tests):
- `depset(direct=..., transitive=...)` tests
- `depset(order="postorder|preorder|topological")` tests
- `depset.to_list()` tests
- Depset iteration tests
- Depset memory efficiency tests

---

## Category 12: Provider Tests

**Buck2 Location**: Scattered in various test files

**Bazel Tests to ADD** (critical for rules_* compatibility):
- `DefaultInfo` provider tests
- `RunInfo` provider tests
- `OutputGroupInfo` provider tests
- `InstrumentedFilesInfo` tests
- `CcInfo` provider tests (for rules_cc)
- `PyInfo` provider tests (for rules_python)
- `RustInfo` provider tests (for rules_rust)
- Custom `provider()` definition tests

---

## Category 13: Daemon/Server Tests

**Buck2 Location**: `tests/core/daemon/`

| Buck2 Test | Concept | Action | Notes |
|------------|---------|--------|-------|
| All daemon tests | Daemon management | PRESERVE | Similar architecture |

---

## Category 14: Rust Unit Tests

**Buck2 Location**: `app/kuro_build_api_tests/`

| Test Module | Concept | Action | Notes |
|-------------|---------|--------|-------|
| `actions.rs` | Action creation | KEEP+UPDATE | Bazel action API |
| `analysis.rs` | Analysis phase | KEEP+UPDATE | Bazel analysis model |
| `artifact_groups.rs` | Artifact groups | PRESERVE | Same concept |
| `attrs.rs` | Attribute types | KEEP+UPDATE | `attr.*` vs `attrs.*` |
| `build.rs` | Build execution | PRESERVE | Same concept |
| `interpreter.rs` | Starlark interp | PRESERVE | Same interpreter |
| `nodes.rs` | Build graph nodes | PRESERVE | Same concept |

---

## Test Migration Priority

### Phase 1: Foundation (Aligned with Plan Phase 2-3)
1. Update `attrs.rs` tests for `attr.*` API
2. Update interpreter tests for Bazel Starlark dialect
3. Update build file detection tests for `BUILD.bazel`

### Phase 2: bzlmod (Aligned with Plan Phase 4a-4d)
1. **ADD** MODULE.bazel parsing tests
2. **ADD** bazel_dep resolution tests
3. **ADD** MVS algorithm tests
4. **ADD** Lockfile tests
5. **DELETE** Cell/external_cell tests

### Phase 3: Rule Primitives (Aligned with Plan Phase 6)
1. **ADD** `ctx.*` API tests
2. **ADD** `ctx.actions.*` tests
3. **ADD** Provider tests (DefaultInfo, etc.)
4. **ADD** Depset tests
5. **ADD** Runfiles tests

### Phase 4: Query (Aligned with Plan Phase 14)
1. **ADD** deps/rdeps tests
2. **ADD** kind/attr/filter tests
3. **ADD** Output format tests

### Phase 5: Sandboxing (Aligned with Plan Phase 12)
1. **ADD** Sandbox isolation tests
2. **ADD** Undeclared dependency detection tests

---

## Test Framework Adaptation

### Preserving Buck2 Framework for Bazel Tests

We will keep the existing pytest-based framework because:
1. Python tests are easier to read/write than shell scripts
2. Async support enables parallel test execution
3. Golden file infrastructure is mature
4. Sanitization functions handle non-determinism

### Changes Needed to Test Framework

1. **`buck_workspace.py`**:
   - Support `MODULE.bazel` instead of `.buckconfig`
   - Support `BUILD.bazel` instead of `TARGETS.fixture`
   - Update workspace root detection

2. **`api/buck.py`**:
   - Rename methods to match Bazel semantics (optional)
   - Add `--enable_bzlmod` flag support
   - Update query methods for Bazel syntax

3. **Test data directories**:
   - Replace `.buckconfig` with `MODULE.bazel`
   - Replace `TARGETS.fixture` with `BUILD.bazel`
   - Update visibility syntax in fixtures
   - Update attribute syntax in fixtures

4. **Golden files**:
   - Update expected output formats
   - Add sanitizers for Bazel-specific output

---

## Summary Statistics

| Category | KEEP+UPDATE | DELETE | ADD | PRESERVE |
|----------|-------------|--------|-----|----------|
| Interpreter | 8 | 3 | ~10 | 9 |
| Analysis | 3 | 0 | ~15 | 1 |
| Build | 8 | 10 | ~10 | 11 |
| Query | 2 | 0 | ~20 | 1 |
| Configuration | 6 | 4 | ~10 | 5 |
| Executor | 3 | 2 | ~5 | 10 |
| Cells | 1 | 10 | ~20 | 2 |
| BXL | 0 | 0 | 0 | 20 |
| Test Runner | 2 | 2 | ~10 | 7 |
| Run Command | 0 | 1 | ~3 | 3 |
| Transitive/Depset | 1 | 0 | ~5 | 0 |
| Providers | 0 | 0 | ~15 | 0 |
| **Total** | ~34 | ~32 | ~123 | ~69 |

**Estimated Work**:
- ~34 tests to update (modify Buck2 syntax to Bazel syntax)
- ~32 tests to delete (Buck2-specific concepts)
- ~123 new tests to add (Bazel concepts)
- ~69 tests to preserve as-is

---

## Next Steps

1. Add "Phase 0: Test Infrastructure Migration" to implementation plan
2. Create test migration checklist per implementation phase
3. Set up Bazel repository clone for reference tests
4. Begin with Phase 2 Starlark dialect tests (attr.* API)
