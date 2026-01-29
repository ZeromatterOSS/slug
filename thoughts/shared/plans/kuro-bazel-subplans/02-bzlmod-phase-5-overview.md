# bzlmod Phase 5: Module Extensions (Overview)

> **Main Plan**: [02-bzlmod.md](./02-bzlmod.md)

This phase implements module extensions which allow custom dependency resolution logic.

## Sub-phases

| Phase | Description | Status | Details |
|-------|-------------|--------|---------|
| **5a** | Extension Parsing | Complete | Below |
| **5b** | Build Integration | In Progress | [02-bzlmod-phase-5b.md](./02-bzlmod-phase-5b.md) |
| **5c** | Bundle @bazel_tools | In Progress | [02-bzlmod-phase-5c.md](./02-bzlmod-phase-5c.md) |
| **5d** | DICE Integration | Not Started | [02-bzlmod-phase-5d.md](./02-bzlmod-phase-5d.md) |
| **5e** | Extension Execution | Future | Below (depends on 5d) |

---

## Phase 5a: Extension Parsing (Complete)

### Overview

Parse `use_extension()` and collect tags from MODULE.bazel files.

### Implementation Status

- `use_extension()` parsing in `kuro_bzlmod/src/globals.rs:614-644`
- `ExtensionProxy` Starlark value for capturing tag method calls (`globals.rs:97-146`)
- `ExtensionTagInvoker` for recording tag invocations (`globals.rs:148-193`)
- `use_repo()` for importing generated repositories (`globals.rs:657-688`)
- Extension data types: `ExtensionUsage`, `ExtensionTag`, `TagValue`, `UseRepo` (`types.rs:317-479`)
- Extension aggregation: `AggregatedExtension`, `aggregate_extensions()` (`extensions.rs:74-155`)
- Placeholder types for execution: `ExtensionResult`, `GeneratedRepo`, `ModuleInfo` (`extensions.rs:158-221`)

### Additional Implementations (Phase 5b repository rule infrastructure)

- `module_extension()` Starlark global in `kuro_interpreter_for_build/src/module_extension.rs`
- `tag_class()` Starlark global with attrs parameter
- `module_ctx` Starlark object in `kuro_interpreter_for_build/src/module_ctx.rs` with:
  - `modules` property returning list of bazel_module objects
  - `os` property returning repository_os struct
  - `root_module_has_non_dev_dependency` property
  - Stub methods for I/O operations (download, execute, etc.)
- Extension execution framework in `kuro_interpreter_for_build/src/extension_execution.rs`:
  - `build_module_context()` - creates module_ctx from aggregated extension data
  - `tag_value_to_serialized()` - converts kuro_bzlmod TagValue to SerializedTagValue
  - `extension_tag_to_serialized()` - converts ExtensionTag to SerializedTag
  - `module_info_to_serialized()` - converts ModuleInfo to SerializedModule
  - `ExtensionExecutor` placeholder for DICE integration
- Real tag data support with `SerializedTag`, `SerializedModule`, `SerializedTagValue` types
- Tags accessible as Starlark structs via `mod.tags.install[0].name`

### Repository Rule Infrastructure

- `repository_rule()` Starlark global in `kuro_interpreter_for_build/src/repository_rule.rs`
  - Supports: implementation, attrs, local, environ, configure, remotable, doc parameters
  - Frozen rules can be invoked (e.g., `http_archive(name = "foo", ...)`)
- `repository_ctx` Starlark object in `kuro_interpreter_for_build/src/repository_ctx.rs` with:
  - `name` property - the repository name
  - `attr` property - access to attribute values
  - `os` property - OS information
  - Stub methods: download(), download_and_extract(), file(), execute(), symlink(), template(), read(), delete(), patch(), extract(), watch(), which(), getenv(), repo_metadata(), report_progress(), path()
- `attr.string_keyed_label_dict()` added to attrs_global.rs (needed by http.bzl)
- **@bazel_tools http.bzl loading works!** Test 22 confirms http_archive is accessible
- Synthetic repo integration in cells.rs via `collect_synthetic_repos()` and `materialize_synthetic_repos()`

### Success Criteria (Phase 5a)

- [x] `use_extension()` parses correctly
- [x] Extension tags collected from all using modules
- [x] `module_extension()` global available in .bzl files
- [x] `tag_class()` global available with attrs parameter
- [x] `module_ctx` Starlark object implemented (modules/os/root_module_has_non_dev_dependency properties work)
- [x] `module_ctx.modules` returns real tag data (not just empty lists)
- [x] Tags accessible as Starlark structs (e.g., `mod.tags.install[0].name`)
- [x] Extension execution framework implemented (`build_module_context()`, conversion functions)
- [x] `repository_rule()` global available in .bzl files (Test 21)
- [x] `repository_ctx` object implemented with functional I/O methods
- [x] `@bazel_tools//tools/build_defs/repo:http.bzl` loads successfully (Test 22)
- [x] `http_archive` available as `repository_rule` type (Test 22)
- [x] `attr.string_keyed_label_dict()` implemented
- [x] Synthetic repos registered as cells (bazel_features_version, etc.)

---

## Phase 5e: Extension Execution (Future)

> **Depends on**: Phase 5d (DICE Integration for Repository Rule Execution)

### Overview

Execute module extensions and generate repositories. This phase cannot proceed until repository rules can actually execute (Phase 5d).

### Remaining Work

- DICE integration to load extension .bzl files and invoke implementations
  - Challenge: Cell resolution happens before DICE is fully initialized
  - Requires architectural changes to support Starlark evaluation during cell resolution
- `module_ctx.download()` actual file fetching
- `module_ctx.execute()` actual command execution
- Lockfile integration for caching extension results

### Current Workaround

- Synthetic repos created for known extensions (bazel_features_version, bazel_features_globals, cc_compatibility_proxy)
- Works for extensions that just expose version info or simple content
- Does NOT work for extensions that need actual downloads (pip.parse, rules_go toolchains)

### Success Criteria

- [ ] `module_ctx.download()` fetches files (after 5d)
- [ ] `module_ctx.execute()` runs commands (after 5d)
- [ ] Extension implementation function invoked via DICE
- [ ] Generated repositories are accessible via @repo_name
- [ ] Extension results cached in lockfile
- [ ] Lockfile cache hit skips re-execution

### Manual Verification

- [ ] Simple extension creating a filegroup works
- [ ] Extension that downloads a file works
- [ ] Extension that executes a command works
- [ ] rules_python's `pip.parse()` extension works (stretch goal)

---

## Bazel Source References

| Feature                    | Bazel Source File                                                                           |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| Extension definition       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtension.java`             |
| `module_extension()` API   | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionApi.java`          |
| `use_extension()` handling | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java`           |
| Tag classes                | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/TagClass.java`                    |
| Extension evaluation       | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/SingleExtensionEvalFunction.java` |
| `module_ctx` object        | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionContext.java`      |
| Extension lockfile         | `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/LockFileModuleExtension.java`     |

**Key tests:** `src/test/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleExtensionResolutionTest.java`

**Real-world examples:** Study how rules_python implements `pip.parse()` in the rules_python repository.

---

## API Reference

### Extension Definition (module_extension global)

```python
# In extensions.bzl
my_ext = module_extension(
    implementation = _my_ext_impl,
    tag_classes = {
        "install": tag_class(attrs = {"name": attr.string()}),
    },
    os_dependent = False,
    arch_dependent = False,
)
```

### module_ctx Starlark Object

**Data Access Properties:**
```python
module_ctx.modules          # list[bazel_module] - All modules using this extension
module_ctx.os               # repository_os - System info (name, arch, environ)
module_ctx.root_module_has_non_dev_dependency  # bool
```

**File I/O Methods:**
```python
module_ctx.read(path, *, watch='auto')
module_ctx.file(path, content='', executable=True)
module_ctx.extract(archive, output='', strip_prefix='')
module_ctx.watch(path)
```

**Network Operations:**
```python
module_ctx.download(url, output='', sha256='', integrity='', ...)
module_ctx.download_and_extract(url, output='', sha256='', strip_prefix='', ...)
```

**Execution & System:**
```python
module_ctx.execute(arguments, timeout=600, environment={}, quiet=True)
module_ctx.which(program)
module_ctx.getenv(name, default=None)
module_ctx.path(path)
```

### Repository Rule Invocation

Extensions call repository rules to create repositories:
- `http_archive()` - Download and extract archives
- `http_file()` - Download single files
- `git_repository()` - Clone git repos
- `new_local_repository()` - Create repo from local path
- Custom repository rules defined in .bzl files

**repository_ctx I/O methods implemented:**
- `file(path, content, executable)` - Create files with content, chmod +x if executable
- `download(url, output, sha256, integrity, ...)` - Download files with integrity verification
- `download_and_extract(url, output, sha256, strip_prefix, ...)` - Download and extract archives
- `execute(arguments, timeout, environment, ...)` - Run shell commands, capture output
- `symlink(target, link_name)` - Create symbolic links
- `template(path, template, substitutions, ...)` - Create files from templates
- `read(path)` - Read file contents
- `delete(path)` - Delete files/directories
- `patch(patch_file, strip)` - Apply patch files
- `extract(archive, output, strip_prefix)` - Extract local archives
- `which(program)` - Find programs on PATH
- `getenv(name, default)` - Get environment variables

**Test 22 verifies:** `@bazel_tools//tools/build_defs/repo:http.bzl` loads successfully and `http_archive` is type `repository_rule`

**Note:** Full end-to-end repository rule execution requires DICE integration (Phase 5d). The I/O methods are functional and tested via unit tests.
