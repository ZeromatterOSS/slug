# bzlmod Phase 5c: Bundle @bazel_tools Repository

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)
> **Parent Phase**: [Phase 5 Overview](./02-bzlmod-phase-5-overview.md)

## Overview

Bundle the `@bazel_tools` repository from Bazel's source and make it automatically available to all bzlmod projects.

**Why this phase is critical:** Many BCR modules load from `@bazel_tools`:
- `rules_cc` loads `@bazel_tools//tools/cpp:toolchain_utils.bzl`
- Module extensions use `@bazel_tools//tools/build_defs/repo:http.bzl` for `http_archive`
- `bazel_features` uses `@bazel_tools` for version detection

---

## Source

Copy the `tools/` directory from Bazel repository HEAD:
- **Repository**: https://github.com/bazelbuild/bazel
- **Directory**: `tools/`
- **Destination**: `bazel_tools/` in Kuro source tree

---

## Key Directories to Include

| Directory                      | Purpose                                         | Priority |
| ------------------------------ | ----------------------------------------------- | -------- |
| `tools/build_defs/repo/`       | Repository rules (http_archive, git_repository) | Critical |
| `tools/cpp/`                   | C++ toolchain utilities                         | Critical |
| `tools/build_defs/build_info/` | Build info utilities                            | Medium   |
| `tools/osx/`                   | macOS toolchain                                 | Medium   |
| `tools/sh/`                    | Shell utilities                                 | Low      |

---

## Directory Structure

```
kuro/
├── bazel_tools/              # Copied from Bazel via scripts/sync_bazel_tools.sh
│   ├── tools/
│   │   ├── build_defs/repo/  # http.bzl, git.bzl, etc.
│   │   ├── cpp/              # toolchain_utils.bzl, etc.
│   │   └── ...
│   ├── MODULE.bazel
│   └── .buckconfig
├── prelude/
├── scripts/
│   └── sync_bazel_tools.sh
└── app/kuro_external_cells_bundled/
```

---

## Success Criteria

### Automated Verification

- [x] `bazel_tools/` directory exists with tools from Bazel 9.0.0
- [x] `kuro_external_cells_bundled` builds successfully with bazel_tools (3 tests passing)
- [x] `@bazel_tools` cell automatically registered for bzlmod projects
- [x] `load("@bazel_tools//tools/build_defs/repo:cache.bzl", ...)` succeeds
- [ ] `load("@bazel_tools//tools/cpp:toolchain_utils.bzl", ...)` succeeds
    - **Blocker**: File found but loads `@rules_cc` which isn't available in bazel_tools context
- [x] `load("@bazel_tools//tools/build_defs/repo:http.bzl", ...)` succeeds (Test 22)
    - `repository_rule()` and `repository_ctx` implemented
    - `attr.string_keyed_label_dict()` added

### Manual Verification

- [x] Create bzlmod project without explicit bazel_tools configuration
- [x] Verify `@bazel_tools` is available via `kuro audit cell`
- [ ] Load a .bzl file from rules_cc that depends on @bazel_tools
- [x] Build binary size increase is reasonable (~2MB for bazel_tools)

---

## Future Work: Bazel-Specific Starlark APIs

| API                           | Used In                       | Purpose                    | Status      |
| ----------------------------- | ----------------------------- | -------------------------- | ----------- |
| `visibility("public")`        | `cache.bzl`, `http.bzl`, etc. | Package visibility control | Implemented |
| `repository_rule`             | `http.bzl`, `git.bzl`         | Repository rule definition | Phase 5     |
| `repository_ctx` methods      | `http.bzl`, `git.bzl`         | Repository rule context    | Phase 5     |
| Module-level `config_setting` | Various BUILD files           | Configuration transitions  | Future      |

---

## Future: Visibility Enforcement (Research Task)

The current `visibility()` implementation is a no-op stub. Before implementing enforcement, research is needed:

**Research Questions:**
1. How does Bazel's `visibility()` interact with `load()` statements?
2. What happens when loading a `visibility("private")` file from another package?
3. How do package specifications like `"//foo:__subpackages__"` work?
4. Does visibility apply at file level or symbol level?

**References:**
- Bazel source: `src/main/java/com/google/devtools/build/lib/packages/BzlVisibility.java`
- Bazel docs: https://bazel.build/rules/lib/globals/bzl#visibility
- Test cases: `BzlVisibilityTest.java`
