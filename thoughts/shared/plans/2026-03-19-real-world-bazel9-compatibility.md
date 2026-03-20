# Real-World Bazel 9 Compatibility Plan

## Overview

Make kuro work as a drop-in replacement for Bazel 9 on real-world projects
that use complex MODULE.bazel configurations, external rulesets, and
platform-specific .bazelrc setups. This plan is driven by iterative testing
against actual Bazel 9 projects.

## Methodology

1. Run `kuro build //<target>` on a real Bazel 9 project
2. Read the first error message
3. Identify the root cause (missing feature, wrong behavior, etc.)
4. Consult Bazel 9 documentation + Bazel source for correct behavior
5. Implement the fix in kuro
6. Repeat from step 1

## Gap Analysis

### Phase 1: .bazelrc Parsing Gaps (COMPLETE)

- [x] `%workspace%` substitution in paths and flag values
- [x] `--enable_platform_specific_config` auto-applies `build:<os>` configs
- [x] Accept 20+ Bazel-specific flags as no-ops (heap_dump_on_oom, curses, etc.)
- [x] `--no<flag_name>` boolean negation pattern handling
- [x] Strip test-only `common` flags when injecting into non-test commands
- [x] Strip `--config=NAME` from bazelrc injection (conflicts with Buck2 `-c`)
- [x] Filter `--enable_platform_specific_config` from injected flags (processed during loading)

### Phase 2: Bzlmod Globals (COMPLETE)

- [x] `override_repo(extension_proxy, repo_name="dep_name")` - overrides extension repos
- [x] `inject_repo(extension_proxy, "dep_name")` - makes repos visible to extensions

### Phase 3: Archive Format Support (COMPLETE)

- [x] XZ-compressed tar (.tar.xz) extraction support
- [x] BZip2-compressed tar (.tar.bz2) extraction support
- [x] Better error messages for unknown archive formats

### Phase 4: Windows Path Fixes (COMPLETE)

- [x] Empty version display uses "override" instead of "<empty>" (avoids invalid `<>` in Windows paths)

### Phase 5: Starlark Compatibility (IN PROGRESS)

- [ ] `attr.string(values=["PY2", "PY3"])` - allow uppercase enum values
- [ ] More attr validation relaxation as discovered
- [ ] rules_python Starlark compatibility
- [ ] rules_rust Starlark compatibility
- [ ] bazel_lib Starlark compatibility

### Phase 6: Module Extension Execution

- [ ] Crate universe extension (rules_rs `crate.from_cargo()`)
- [ ] Rust toolchain extension (rules_rust `rust.toolchain()`)
- [ ] LLVM toolchain extension
- [ ] `register_toolchains()` wiring

### Phase 7: Rule Execution

- [ ] `cargo_build_script` support
- [ ] `rust_shared_library` support
- [ ] Complex dep graphs through crate universe
- [ ] `label_keyed_string_dict` attr type

### Phase 8: End-to-End Build

- [ ] Full dependency resolution
- [ ] All Rust crates compile
- [ ] C++ SDK build
- [ ] SDK tarball assembly

## What We're NOT Doing

1. Worker strategy (local execution only)
2. Disk cache (accept flags, ignore caching)
3. Clippy/rustfmt aspects (accept flags, skip execution)
4. Remote execution
5. Convenience symlinks

## Success Criteria

- `kuro build //<target>` completes with exit code 0 on a non-trivial Bazel 9 project
- Build output matches Bazel 9 output (same artifacts)
