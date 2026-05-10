# Plan 55: Symbolic Macro `inherit_attrs` Parity

> Parent: [15-bazel-9-parity.md](15-bazel-9-parity.md)
>
> Per repo-local `AGENTS.md`: target Bazel 9 only. Do not add Bazel 8
> compatibility shims or target-specific workarounds.

## Status

Implementation complete as of 2026-05-09. The `//sdk:sdk_contents` smoke
confirmed the original missing inherited-attr blocker is gone; remaining
frontier moved back to later analysis work owned by Plan 15.

## Current Blocker

The `//sdk:sdk_contents` smoke
`/tmp/plan15-cmdargs-frozen-list-1.log` cleared the prior cc_common
frozen-list command-line issue, then failed while loading
`llvm+0.7.0//runtimes:BUILD.bazel`:

```text
copy_to_resource_directory(...)
error: Missing parameter `target_triple` for call to
`copy_to_resource_directory.bzl._copy_to_resource_directory_macro_impl`
```

The macro is declared as:

```python
copy_to_resource_directory = macro(
    implementation = _copy_to_resource_directory_macro_impl,
    inherit_attrs = copy_to_resource_directory_rule,
)
```

`target_triple` is an inherited `attr.string()` on the backing rule. The BUILD
call omits it, and the implementation intentionally falls back when the value is
falsy:

```python
target_triple = target_triple if target_triple else select(TRIPLE_SELECT_DICT)
```

Kuro previously ignored `inherit_attrs` in `macro()`, so omitted inherited
attributes were not passed to the implementation at all. Starlark therefore
reported a missing required function parameter instead of passing Bazel's
default value.

## Bazel 9 Source Of Truth

Local Bazel 9.1.0 probe:

```python
r = rule(
    implementation = _r_impl,
    attrs = {"s": attr.string()},
)

def _m_impl(name, visibility, s, **kwargs):
    fail("s=%r type=%s kwargs=%r" % (s, type(s), kwargs))

m = macro(
    implementation = _m_impl,
    inherit_attrs = r,
)
```

Calling `m(name = "x")` fails with:

```text
s=None type=NoneType kwargs={... common attrs ...}
```

So, for Bazel 9 parity, an omitted inherited `attr.string()` with no explicit
default is passed to the symbolic macro implementation as `None`, not treated as
a missing function parameter. Common inherited attrs are also passed as keyword
arguments with `None` defaults.

## Implementation Notes

- Store `inherit_attrs` on Kuro's symbolic macro callable instead of discarding
  it in the `macro()` native.
- For `inherit_attrs = <rule>`, inspect the frozen rule's `AttributeSpec` during
  BUILD-file macro invocation.
- Inject every omitted inherited attr as a named argument:
  - use the attr default when present;
  - otherwise pass `None`, matching the Bazel probe above.
- Keep explicit `attrs = {...}` behavior aligned with inherited attrs: omitted
  explicit macro attrs without defaults should also be passed as `None`.
- Preserve the `StarlarkAttribute::implicit_default` marker on rule callables so
  inherited attrs declared as `attr.string()` without an explicit default inject
  `None` into symbolic macro implementations while normal rule target coercion
  still sees Kuro's existing coerced default.
- Do not implement unrelated symbolic macro semantics in this slice, such as
  visibility encapsulation, finalizers, or name-prefix enforcement.

## Verification

Focused coverage should include:

```sh
cargo fmt -- app/kuro_interpreter_for_build/src/rule.rs \
  app/kuro_interpreter_for_build/src/macro_callable.rs \
  app/kuro_interpreter_for_build/src/interpreter/natives.rs
cargo check -p kuro_interpreter_for_build
pytest -q tests/core/analysis/test_symbolic_macros.py::test_symbolic_macro_inherited_rule_attr_default
cargo build -p kuro
git diff --check
```

Then rerun a bounded `//sdk:sdk_contents` smoke and confirm the
`copy_to_resource_directory` missing `target_triple` error is gone.

Verified locally after implementation:

```sh
cargo fmt -- app/kuro_interpreter_for_build/src/rule.rs \
  app/kuro_interpreter_for_build/src/macro_callable.rs \
  app/kuro_interpreter_for_build/src/interpreter/natives.rs
cargo check -p kuro_interpreter_for_build
cargo test -p kuro_build_api_tests map_each_sequence_returns_expand_as_items -- --nocapture
cargo build -p kuro
pytest -q tests/core/analysis/test_symbolic_macros.py::test_symbolic_macro_inherited_rule_attr_default
git diff --check
```

Bounded zeromatter smoke:

```sh
bash -o pipefail -c 'timeout 260s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan55-symbolic-macro-inherit-1'\'' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan55-symbolic-macro-inherit-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan55-symbolic-macro-inherit-1.log'
```

Outcome:

- Timed out with exit `124` after reaching analysis; peak sampled total RSS
  was about `864 MiB`.
- The prior `copy_to_resource_directory` / missing `target_triple` load-time
  error did not recur.
- Latest visible frontier was waiting on
  `bazel_tools//tools/cpp:malloc (...#8d4033f8c19b9f73) -- running analysis
  [evaluate_rule], and 9 other actions`, which matches the later Plan 15 C++
  toolchain-analysis frontier rather than this symbolic-macro inheritance
  slice.

Known remaining symbolic-macro parity gaps outside this blocker:

- `inherit_attrs` currently implements frozen rule inheritance only; Bazel's
  broader accepted `inherit_attrs` forms such as symbolic macro inheritance or
  `"common"` still need source-checked treatment.
- Inherited attr filtering is currently all frozen rule attrs except `name` and
  `visibility`; public/private/common attr filtering should be tightened
  against Bazel 9 probes before broadening.
- Complex default conversion for label/select-style attrs is still partial and
  should be expanded only with focused Bazel probes/tests.
