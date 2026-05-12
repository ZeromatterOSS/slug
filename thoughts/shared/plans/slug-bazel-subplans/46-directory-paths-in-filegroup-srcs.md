# Plan 46: Directory paths in `filegroup.srcs` (and similar `one_of(dep, source)` attrs)

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Discovered while implementing Plan 44 Phase 2.5 (per-action execroot
> for rules_rust runner compatibility). End-to-end verification of
> `crates__zerocopy-0.8.42//:_bs` was blocked because zeromatter's
> `llvm_toolchains//:linux_x86_64_cc_toolchain` analysis fails with
> `Unknown target lib/clang/22` from
> `llvm-toolchain-minimal-22.1.0-linux-amd64//`.

## Status: IN PROGRESS

## Context

Bazel's `filegroup.srcs` accepts directory path strings — e.g.
`srcs = ["lib/clang/22"]` includes every file under that directory. The
LLVM rules use this:

```python
# bazel-external/llvm+0.7.0/directory.bzl::headers_directory
native.filegroup(
    name = name + "_source_directory",
    srcs = [path],          # path = "lib/clang/22"
)
```

Slug's `filegroup` rule rejects this with
`Unknown target lib/clang/22 from package
llvm-toolchain-minimal-22.1.0-linux-amd64//`. The dep-coercion path
synthesizes a target label `:lib/clang/22` that doesn't exist.

This blocks the entire LLVM toolchain analysis chain and, by
extension, every cargo_build_script in zeromatter whose toolchain
selection traverses `llvm_toolchains//:linux_x86_64_cc_toolchain`.

## Three-part bug

### 1. Filegroup's `srcs` attribute disables directory paths

`app/slug_interpreter_for_build/src/interpreter/native_rules.rs:184-216`
declares:

```rust
let srcs_attr = Attribute::new(
    ...,
    AttrType::list(AttrType::one_of(vec![
        AttrType::dep(ProviderIdSet::EMPTY, PluginKindSet::EMPTY),
        AttrType::source(false),    // allow_directory = false
    ])),
);
```

`AttrType::source(false)` causes
`app/slug_interpreter_for_build/src/attrs/coerce/ctx.rs::coerce_path:519-525`
to reject directory paths with `SourceFileIsDirectory`. Bazel's
filegroup permits directory paths in srcs, expanding to the contained
files at analysis time.

### 2. `one_of(dep, source)` ordering picks `dep` for path-shaped strings

The `dep` variant of one_of is tried first.
`coerce_label_no_cache:400-433` accepts a bare slashed name like
`lib/clang/22` as a label `<current_pkg>:lib/clang/22`, even when the
package contains no such target. The `source` variant is never tried.
At analysis time, the dep lookup fails with `Unknown target`.

Note the comment at lines 361-364:

```rust
// Bazel-compatible: bare target names (e.g., "foo_bar") resolve relative to
// the current package. Try prepending ":" if the original parse failed.
// But skip this for bare names that are known source files in the package,
// so that one_of(dep, source) coercion falls through to source coercion.
```

Lines 369-379 compute `is_source_file` for that purpose, but
**`is_source_file` is never used downstream** — the dep coercion
unconditionally synthesizes a label for any bare slashed name. The
comment describes intended behavior that isn't wired up.

### 3. Analysis-time directory expansion (already implemented)

`app/slug_node/src/attrs/coerced_path.rs::CoercedPath::Directory`
already carries the expanded file list (`files: Box<[ArcS<PackageRelativePath>]>`),
populated at coerce time by `coerce_path` line 538. The filegroup
analysis at
`app/slug_analysis/src/analysis/native_rule_analysis.rs::collect_source_files_from_configured_attr:813-820`
iterates via `coerced_path.inputs()`, which returns all files within
a `CoercedPath::Directory`. This piece works once a directory path
reaches it as a `CoercedAttr::SourceFile(CoercedPath::Directory(_))`.

## The fix

Three edits, in order:

### Fix 1: filegroup uses `AttrType::source(true)`

`app/slug_interpreter_for_build/src/interpreter/native_rules.rs:192`
and `:203`: change `AttrType::source(false)` to
`AttrType::source(true)` so `coerce_path` accepts directory paths.

### Fix 2: `coerce_label_no_cache` defers to source for path-shaped existing entries

`app/slug_interpreter_for_build/src/attrs/coerce/ctx.rs:400-433`: when
`is_bare_name && value.contains('/')`, before synthesizing
`<pkg>:<value>` as a label, check whether `value` resolves to a real
file *or directory* in the enclosing package. If so, fail label
coercion with `RequiredLabel` so the `one_of` falls through to the
source variant. Output-registry check stays first (declared outputs
still win — they're real targets).

Sketch:

```rust
} else if is_bare_name && value.contains('/') {
    // Output registry first: declared output paths route to producer.
    if let Some(producer) = self.output_file_registry.borrow().get(value).cloned() {
        // ... existing handling ...
    }
    // Defer to source-coercion when the path exists on disk under the
    // enclosing package (file OR directory). The one_of(dep, source)
    // wrapper will then route the value through SourceAttrType.
    if let Some((_, listing)) = &self.enclosing_package {
        if let Ok(rel) = <&PackageRelativePath>::try_from(value) {
            if listing.get_file(rel).is_some() || listing.get_dir(rel).is_some() {
                return Err(BuildAttrCoercionContextError::RequiredLabel(
                    value.to_owned()
                ).into());
            }
        }
    }
    // Fall through to existing label synthesis for cases like
    // "include/foo.h" that aren't on disk but might be a generated
    // header registered by another rule.
    if let Some((pkg_label, _listing)) = &self.enclosing_package {
        // ... existing synthesis ...
    }
}
```

This activates the long-standing intent in lines 361-364 ("skip this
for bare names that are known source files"). The `is_source_file`
local can be removed once the new check subsumes it.

### Fix 3: filegroup analysis (no change — verify only)

Confirm via test that
`collect_source_files_from_configured_attr` returns a non-empty list
when `srcs` contains a directory path. Existing
`CoercedPath::Directory.inputs()` already iterates the contained
files; add a unit test in `slug_analysis` to lock in the behavior.

## Tests

### Unit (slug_interpreter_for_build)

In `app/slug_interpreter_for_build/src/attrs/coerce/ctx.rs` under
`#[cfg(test)]`:

- `bare_slashed_path_to_directory_defers_to_source`: build a
  `BuildAttrCoercionContext` with an enclosing package whose listing
  has a directory `lib/clang/22` (no targets named that). Calling
  `coerce_label_no_cache("lib/clang/22")` returns
  `Err(RequiredLabel)`, NOT a synthesized
  `<pkg>:lib/clang/22` label.
- `bare_slashed_path_to_file_also_defers_to_source`: same shape but
  with a file at that path. Same expected outcome.
- `bare_slashed_label_for_unknown_path_still_synthesizes`:
  `coerce_label_no_cache("not/on/disk/foo")` continues to synthesize
  `<pkg>:not/on/disk/foo` (preserves current behavior for
  generated-output references that aren't on disk yet).

### Unit (slug_analysis)

In `native_rule_analysis.rs` test module: build a configured filegroup
node with a `srcs` attr containing
`CoercedAttr::SourceFile(CoercedPath::Directory { files: [a, b, c] })`.
Assert `analyze_filegroup` returns DefaultInfo with three artifacts.

### Integration

A new BUILD file under `tests/core/build/` (or extend an existing
one) that declares:

```python
filegroup(
    name = "dir_filegroup",
    srcs = ["nested_dir"],   # nested_dir/ has a, b, c.txt files
)
```

Plus a target that depends on `:dir_filegroup` and dumps the file
list. Run via `slug test fbcode//slug/tests/core/build:dir_filegroup_test`
(naming TBD).

### End-to-end (zeromatter)

Re-run `slug build crates__zerocopy-0.8.42//:_bs --target-platforms=//bazel/platforms:linux-gnu-host`
in `/var/mnt/dev/zeromatter/`. The previous failure was at
analysis time on `llvm-toolchain-minimal-22.1.0-linux-amd64//:lib/clang/22`.
After the fix, analysis should advance past that point and either
succeed (if Plan 44 Phase 2.5's collision-name filter covers the
runfiles tree) or surface the next blocker.

## Generalization

Several other native rules use `AttrType::source(false)` and would
benefit from the same change if they see directory paths in user
build files. Candidates (grep
`AttrType::source(false)` in
`app/slug_interpreter_for_build/src/attrs/`):

- `attrs_global.rs:1057, 1176` — generic `attr.label`/`attr.label_list`
  defaults. Probably fine to leave at `false`; user rules that want
  directory inputs can opt in via their own attribute declaration.
- `attrs_global.rs:816` — the user-facing `attr.source(...)` factory.
  Already exposes `allow_directory` parameter. Verify the parameter
  threads through; if not, fix here too.

For now, scope to filegroup only. Other rules can opt in as concrete
needs surface.

## Progress

2026-05-08:

- Earlier Plan 46 work had already landed the `filegroup` part:
  native `filegroup` uses `AttrType::source(true)`, and bare path-shaped
  existing files/directories defer from dep coercion to source coercion.
- ZeroMatter Plan 50 verification surfaced the same directory-source class
  through rules_cc: `cc_library(hdrs = ["include"])` in
  `llvm+glibc+glibc_headers_x86_64-linux-gnu.2.28//` reaches
  `attr.label_list(allow_files = True)`, which still used
  `AttrType::source(false)` and rejected directory paths.
- Generalized the fix from filegroup-only to Bazel-compatible
  `attr.label_list(allow_files = True)`: source alternatives now allow
  directories so attrs like rules_cc `hdrs` expand directory paths to the
  contained files.
- Kept `attr.label(allow_single_file = True)` file-only. For
  `attr.label(allow_files = True)`, directories are allowed only when the
  caller did not request single-file semantics.
- Updated `PackageListing::testing_files` to include parent directory
  entries, matching real package listings closely enough for directory
  source coercion tests.
- Added a regression test for a Starlark rule with
  `attr.label_list(allow_files = True)` accepting `hdrs = ["include"]`
  and coercing it to a source directory containing `include/a.h` and
  `include/bits/b.h`.

## Risk

- **Coverage of existing `<pkg>:<slashed_name>` labels that
  intentionally exist**: a build file that legitimately defines
  `target_name = "foo/bar"` and a different rule references it via
  `srcs = ["foo/bar"]` would previously coerce as a dep. After Fix 2,
  if no file or directory `foo/bar` exists in the package, behavior
  is unchanged (label synthesis path still runs). If `foo/bar` exists
  on disk, the source coercion now wins — which is consistent with
  Bazel: bare paths name files, not targets, in `srcs` context.
- **Performance**: the added `listing.get_dir` check is a hash lookup
  in the package listing; trivial overhead per coercion call.
- **Watcher invalidation**: directory paths in srcs cause the file
  listing under the directory to be part of the action's input
  fingerprint. New files added/removed under `lib/clang/22` will
  invalidate downstream actions. Same shape as Bazel; expected.

## Out of scope

- Glob handling in `srcs = native.glob(["lib/clang/*"], ...)` — that
  goes through `glob()` machinery, not directory-path coercion. Plan
  44 / glob plans cover that separately.
- Recursive directory expansion semantics beyond what
  `CoercedDirectory.files` already records. The current expansion
  walks all files within the directory at coerce time; that matches
  Bazel's semantics for filegroup directory srcs.
- User-rule `attr.label_list` directory support (covered conditionally
  in "Generalization" above).

## Effort

Half a day for code + tests. End-to-end verification on zeromatter adds
another half day depending on how many subsequent blockers surface.

## Verification

- `cargo test -p slug_interpreter_for_build --lib` green (existing
  + new tests).
- `cargo test -p slug_analysis --lib` green (existing + new test).
- `cargo test -p slug_node -p slug_common -p slug_configured -p slug_action_impl --lib`
  unchanged (regression guard).
- `examples/multi_package :gen_version_header` builds cleanly.
- ZeroMatter `slug build crates__zerocopy-0.8.42//:_bs` advances past
  the `llvm-toolchain-minimal-...//:lib/clang/22` analysis error.

## Files touched (estimate)

- `app/slug_interpreter_for_build/src/interpreter/native_rules.rs` —
  2 lines (filegroup srcs + data attr types).
- `app/slug_interpreter_for_build/src/attrs/coerce/ctx.rs` — ~15
  lines (defer-to-source check + cleanup of unused
  `is_source_file` local).
- `app/slug_analysis/src/analysis/native_rule_analysis.rs` — test
  only (~30 lines).
- New test fixture under `tests/core/build/` — small.
- `thoughts/shared/plans/slug-bazel-subplans/44-workspace-layout-parity.md`
  — update Phase 2.5 outstanding-work block to note that the LLVM
  blocker is unblocked once this plan lands.
