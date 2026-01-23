# Rules Integration Phases (7-10)

> **Parent Plan**: [Kuro Bazel-Compatible Build Tool](../2026-01-21-kuro-bazel-compatible-build-tool.md)

This sub-plan covers integration with the rules_* ecosystem: rules_cc, rules_rust, rules_python, and rules_oci.

---

## Phase 7: rules_cc Integration

### Overview

Get rules*cc working to compile C and C++ code. We target Bazel 9.0.0+ where rules_cc uses Starlark providers. Further in depth research is required to determine if the cc rules are \_actually* pure starlark with fallbacks purely for older bazel versions.
Due to the recency of the release of bazel 9, assumptions about version numbers should should be regularly be double checked by fetching web content, and plans and research should cite links, as well as filenames+line numbers heavily

### Architecture (Bazel 9.0.0+)

TODO: Perform in-depth research &

### Changes Required:

#### 1. Fetch rules_cc from BCR

```python
module(name = "test_cc")
bazel_dep(name = "rules_cc", version = "0.2.16")
```

#### X. Unknown

This must be filled in with further research

#### 6. Test with Real Project

```python
load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library", "cc_test")

cc_library(
    name = "mylib",
    srcs = ["mylib.cc"],
    hdrs = ["mylib.h"],
)

cc_binary(
    name = "main",
    srcs = ["main.cc"],
    deps = [":mylib"],
)

cc_test(
    name = "mylib_test",
    srcs = ["mylib_test.cc"],
    deps = [":mylib", "@googletest//:gtest_main"],
)
```

### Success Criteria:

#### Automated Verification:

- [ ] Native `cc_common` module is available
- [ ] `cc_common.compile()` creates compilation actions
- [ ] `cc_common.link()` creates linking actions
- [ ] rules_cc's `CcInfo` provider works (uses Starlark `provider()`)
- [ ] `kuro build //:main` compiles and links successfully
- [ ] Header dependencies tracked correctly
- [ ] Incremental builds work
- [ ] `kuro test //:mylib_test` runs tests

#### Manual Verification:

- [ ] Build a non-trivial C++ project
- [ ] Verify compile_commands.json generation (via BXL)
- [ ] Test with both gcc and clang

#### Test Migration (Phase 7):

- [ ] ADD `tests/core/cc_common/test_compile.py` for cc_common.compile()
- [ ] ADD `tests/core/cc_common/test_link.py` for cc_common.link()
- [ ] ADD `tests/core/cc_common/test_create_compilation_context.py`
- [ ] ADD `tests/core/rules_cc/test_cc_library.py` for @rules_cc cc_library
- [ ] ADD `tests/core/rules_cc/test_cc_binary.py` for linking

---

## Phase 8: rules_rust Integration

### Overview

Get rules_rust working to compile Rust code.

### Changes Required:

#### 1. Fetch rules_rust from BCR

```python
bazel_dep(name = "rules_rust", version = "0.40.0")
```

#### 2. Rust Toolchain

- Download or detect rustc/cargo
- Handle edition, target triple

#### 3. Test with Real Project

```python
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

rust_library(
    name = "mylib",
    srcs = ["lib.rs"],
)

rust_binary(
    name = "main",
    srcs = ["main.rs"],
    deps = [":mylib"],
)
```

#### 4. crate_universe for Cargo Dependencies

```python
crate = use_extension("@rules_rust//crate_universe:extension.bzl", "crate")
crate.from_cargo(
    name = "crates",
    cargo_lockfile = "//:Cargo.lock",
    manifests = ["//:Cargo.toml"],
)
use_repo(crate, "crates")
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro build //:main` compiles Rust code
- [ ] `kuro test //:rust_test` runs tests
- [ ] crate_universe resolves Cargo dependencies

#### Manual Verification:

- [ ] Build a Rust project with external crates

---

## Phase 9: rules_python Integration

### Overview

Get rules_python working for Python projects.

### Changes Required:

#### 1. Fetch rules_python from BCR

```python
bazel_dep(name = "rules_python", version = "0.31.0")
```

#### 2. Python Toolchain

```python
python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.11")
```

#### 3. pip Integration

```python
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(
    hub_name = "pip",
    python_version = "3.11",
    requirements_lock = "//:requirements_lock.txt",
)
use_repo(pip, "pip")
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro run //:py_main` executes Python
- [ ] `kuro test //:py_test` runs pytest
- [ ] pip dependencies available

#### Manual Verification:

- [ ] Build a Python project with pip dependencies

---

## Phase 10: rules_oci Integration

### Overview

Enable container image building via rules_oci.

### Changes Required:

#### 1. Fetch rules_oci and rules_pkg

```python
bazel_dep(name = "rules_oci", version = "2.0.0")
bazel_dep(name = "rules_pkg", version = "0.9.1")
```

#### 2. Container Building

```python
load("@rules_oci//oci:defs.bzl", "oci_image", "oci_push")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")

pkg_tar(
    name = "app_layer",
    srcs = [":app"],
    package_dir = "/usr/local/bin",
)

oci_image(
    name = "image",
    base = "@distroless_base",
    tars = [":app_layer"],
    entrypoint = ["/usr/local/bin/app"],
)
```

### Success Criteria:

#### Automated Verification:

- [ ] `kuro build //:image` creates OCI image
- [ ] Multi-arch images work

#### Manual Verification:

- [ ] Load image into Docker and run container

---

