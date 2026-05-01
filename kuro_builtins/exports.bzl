# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Plan 28: Bundled Bazel-Compatible Builtins.
#
# This file is the entry point of the @kuro_builtins bundled cell. The
# kuro interpreter auto-loads it into every BUILD and `.bzl` evaluation
# context (per `bazel_builtins_autoload` in
# `app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`).
#
# The export contract follows Plan 28's design (mirrored after Bonanza's
# `builtins_core/exports.bzl`):
#
#   - `exported_toplevels`: symbols visible at the top level of every
#     BUILD and `.bzl` file. Each entry must have a Bazel 9 parity
#     citation (or a `_kuro_*` prefix indicating it is kuro-internal,
#     e.g. probes for tests).
#   - `rule_implementation_wrapper` / `aspect_implementation_wrapper` /
#     `subrule_implementation_wrapper`: identity wrappers that
#     Phase 28.4 will route Starlark rule analysis through, so
#     subsequent stages can move `ctx`-method bodies into Starlark
#     without touching the Rust analysis call site again.
#
# Adding a symbol here means committing to:
#   1. A Bazel 9 parity citation (or `_kuro_*` naming).
#   2. A single owner per Plan 28.7 (Rust primitive, Starlark export, or
#      external ruleset — never two of the three).

load(":_host_constants.bzl", "HOST_CONSTRAINT_LABELS")

# -----------------------------------------------------------------------
# Private helpers (not exported, hidden by leading underscore).
# -----------------------------------------------------------------------

# Phase 28.2 probe symbol. Not a Bazel builtin — exists solely to verify
# that the autoload mechanism reaches external `.bzl` files. Will be
# removed once Phase 28.3 starts moving real compatibility logic.
_kuro_builtins_probe_value = "kuro-28-2-loader-ok"

# Plan 28.4 Stage 3: Starlark replacement for the deleted Rust impl in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`. Until kuro
# has a full target-platform constraint resolver (Plan 19 territory), we
# answer `ctx.target_platform_has_constraint(c)` against the host's
# OS/CPU labels, mirroring the previous Rust shortcut byte-for-byte. The
# host labels are baked at kuro build time by
# `app/kuro_external_cells_bundled/build.rs::imp` and arrive here via
# `_host_constants.bzl`.
# Plan 28.4 Stage 6: Starlark replacement for the deleted Rust impl
# of `ctx.package_relative_label` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
# Resolves a label string against the BUILD file's package (the
# target's package), distinct from the `Label()` builtin which
# resolves against the *file* where it appears. Same input/output
# contract as the previous Rust impl.
#
# When `raw_ctx.label` is `None` (dynamic_output / BXL contexts),
# fall through to the file-cell-resolving `Label()` builtin — which
# is what the old Rust path also did via `BazelLabel::parse(input)`.
def _kuro_package_relative_label(raw_ctx, label_str):
    label = raw_ctx.label
    if label == None:
        return Label(label_str)
    cell = label.cell
    pkg = label.package
    if label_str.startswith("@"):
        # Already fully qualified; pass through unchanged.
        return Label(label_str)
    if label_str.startswith("//"):
        return Label("@" + cell + label_str)
    target = label_str[1:] if label_str.startswith(":") else label_str
    return Label("@" + cell + "//" + pkg + ":" + target)

# Plan 28.4 Stage 8: Starlark replacement for the deleted Rust impl
# of `ctx.coverage_instrumented` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`. Reads
# the per-build `--collect_code_coverage` flag via a kuro-internal
# Starlark global registered in
# `app/kuro_interpreter_for_build/src/interpreter/functions/kuro_runtime.rs`.
#
# The previous Rust impl ignored both `this` and `dep` arguments —
# it always returned the global flag — so the migrated function
# preserves that behaviour. If kuro ever supports per-target
# instrumentation lists, the per-dep branch will land here.
# `dep` is accepted (Bazel signature parity) but ignored — the Rust
# impl ignored it too. If/when kuro tracks per-target instrumentation
# lists, branch on `dep != None` here.
def _kuro_coverage_instrumented(dep = None):  # buildifier: disable=unused-variable
    return kuro_collect_code_coverage()

# Plan 28.4 Stage 7: Starlark replacement for the deleted Rust impl
# of `ctx.tokenize` (and its `shell_tokenize` helper) in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`. Pure
# Bourne-shell tokenization — no facade-attr access, no host info,
# no globals — so it sits as a top-level helper rather than a
# closure inside `_make_rule_facade`.
#
# Behaviour mirrors the Rust impl byte-for-byte on ASCII input:
#
#   - Single-quoted strings: literal until closing `'`, no escapes.
#   - Double-quoted strings: backslash escapes for `"`, `\`, `$`,
#     `` ` ``; all other characters literal; trailing `\` at end of
#     input dropped silently.
#   - Backslash outside quotes: consume next char literally;
#     trailing `\` at end of input dropped silently.
#   - ASCII whitespace splits tokens (matches Rust's
#     `char::is_ascii_whitespace`: space, `\t`, `\n`, `\f` /
#     `\x0c`, `\r`).
#
# Starlark has no `while` loops, so the iteration uses a for-loop
# over `range(n + 1)` with explicit `i` advancement and `break`
# when `i >= n`. Each outer step consumes at least one input
# character, so the bound is safe.
def _kuro_tokenize(option_string):
    tokens = []
    current = ""
    in_token = False
    n = len(option_string)
    i = 0

    for _step in range(n + 1):
        if i >= n:
            break
        c = option_string[i]
        i += 1

        if c == "'":
            in_token = True
            for _step2 in range(n + 1):
                if i >= n:
                    break
                c2 = option_string[i]
                i += 1
                if c2 == "'":
                    break
                current += c2
        elif c == "\"":
            in_token = True
            for _step2 in range(n + 1):
                if i >= n:
                    break
                c2 = option_string[i]
                i += 1
                if c2 == "\"":
                    break
                if c2 == "\\":
                    if i < n:
                        nxt = option_string[i]
                        if nxt == "\"" or nxt == "\\" or nxt == "$" or nxt == "`":
                            current += nxt
                            i += 1
                        else:
                            # Non-escapable: keep literal `\`; the
                            # outer iter handles `nxt` next round.
                            current += "\\"

                    # i >= n: drop `\` silently (matches Rust).
                else:
                    current += c2
        elif c == "\\":
            in_token = True
            if i < n:
                current += option_string[i]
                i += 1

            # i >= n: drop trailing `\` silently.
        elif c == " " or c == "\t" or c == "\n" or c == "\x0c" or c == "\r":
            if in_token:
                tokens.append(current)
                current = ""
                in_token = False
        else:
            in_token = True
            current += c

    if in_token or current:
        tokens.append(current)
    return tokens

# Plan 28.4 Stage 10: Starlark replacement for the deleted Rust impl of
# `ctx.new_file` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
#
# `ctx.new_file` is a deprecated Bazel API with two call shapes:
#   - `ctx.new_file(filename: str)` — declare a new file by name.
#   - `ctx.new_file(sibling: File, filename: str)` — same, but the
#     sibling is ignored (the Rust impl read only the filename string).
#
# The implementation delegates to `ctx.actions.declare_file`, which is
# already a Starlark attribute on the actions struct. No new
# `kuro_runtime` globals are needed. Single-owner per Plan 28.7.
def _kuro_new_file(raw_ctx, file_or_sibling, filename):
    name = filename if filename != None else file_or_sibling
    if type(name) != "string":
        name = str(name)
    return raw_ctx.actions.declare_file(name)

# Plan 28.4 Stage 9: Starlark replacement for the deleted Rust impls
# of `ctx.var` and `ctx.expand_make_variables` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`. Both
# methods read from the same `$(VAR)` substitution table; building
# it here keeps the table in one place. Priority order, highest to
# lowest, mirrors the deleted Rust impls (HashMap::entry().or_insert()):
#
#   1. User-provided `additional_substitutions` (only
#      `expand_make_variables`; `var` skips this layer).
#   2. Built-in Make variables (BINDIR, GENDIR, TARGET_CPU,
#      COMPILATION_MODE, WORKSPACE_ROOT, CC, CC_FLAGS, JAVA,
#      JAVA_RUNFILES, JAVABASE, ABI_GLIBC_VERSION, ABI,
#      STACK_FRAME_UNLIMITED). The constants are kuro-internal —
#      `STACK_FRAME_UNLIMITED` for instance is an llvm-project
#      requirement (see memory/ctx_var_builtins.md).
#   3. `TemplateVariableInfo` from each dep in `ctx.attrs.toolchains`
#      (e.g. llvm-project's `workspace_root` rule publishes
#      `WORKSPACE_ROOT` here; `cc_toolchain_provider_helper.bzl`
#      publishes additional Make variables from rules_cc).
#   4. `--define KEY=VALUE` flags (lowest priority).
#
# `BINDIR`/`GENDIR` are read from `raw_ctx.bin_dir.path` (already a
# Starlark attribute on `CtxDirRoot`) and `WORKSPACE_ROOT` from
# `raw_ctx.label.workspace_root` (already a Starlark attribute on
# `StarlarkConfiguredProvidersLabel`); the cfg hash that `BINDIR`
# embeds is hidden inside `bin_dir_path_from_label` on the Rust side.
# Everything else dispatches through `kuro_*` runtime hooks
# registered in
# `app/kuro_interpreter_for_build/src/interpreter/functions/kuro_runtime.rs`.
def _kuro_make_substitutions(raw_ctx):
    bin_dir = raw_ctx.bin_dir.path
    label = raw_ctx.label
    workspace_root = label.workspace_root if label != None else ""

    subs = {
        "BINDIR": bin_dir,
        "GENDIR": bin_dir,
        "TARGET_CPU": kuro_host_target_cpu(),
        "COMPILATION_MODE": kuro_compilation_mode_for_label(label),
        "WORKSPACE_ROOT": workspace_root,
        "CC": kuro_host_cc_path(),
        "CC_FLAGS": "",
        # Bazel uses "java.exe" on Windows and "/usr/bin/java"
        # elsewhere. Kuro is a Linux-first build for now; the
        # branch here matches the deleted Rust impl byte-for-byte
        # so a Windows kuro can land later without touching this
        # file. (`kuro_host_cc_path` already uses the same OS
        # discrimination via `std::env::consts::OS`.)
        "JAVA": "/usr/bin/java",
        "JAVA_RUNFILES": "",
        "JAVABASE": "",
        "ABI_GLIBC_VERSION": "2.17",
        "ABI": "local",
        # `STACK_FRAME_UNLIMITED` is normally seeded by rules_cc's
        # cc_toolchain via TemplateVariableInfo; kuro's stub
        # cc_toolchain doesn't publish that provider, so we ship
        # the default here. See memory/ctx_var_builtins.md.
        "STACK_FRAME_UNLIMITED": "",
    }

    # `ctx.attrs.toolchains`: list of deps whose `TemplateVariableInfo`
    # is exposed to the target. Mirrors Bazel's
    # `RuleContext.getMakeVariables()`. Builtins win on collision.
    attrs = raw_ctx.attrs
    if attrs != None:
        toolchains_attr = getattr(attrs, "toolchains", None)
        if toolchains_attr != None:
            for k, v in kuro_collect_toolchains_template_vars(toolchains_attr).items():
                if k not in subs:
                    subs[k] = v

    # `--define KEY=VALUE` flags. Lowest priority — each builtin and
    # each TemplateVariableInfo entry already wins on collision.
    for k, v in kuro_get_all_defines().items():
        if k not in subs:
            subs[k] = v

    return subs

def _kuro_var(raw_ctx):
    return _kuro_make_substitutions(raw_ctx)

# Plan 28.4 Stage 9: parses `$(VAR)` patterns in `command` and
# substitutes from the merged table. Mirrors the deleted Rust
# impl's behaviour byte-for-byte:
#
#   - User `additional_substitutions` (an optional dict) win over
#     all other layers.
#   - Unresolved `$(VAR)` patterns are left in place verbatim.
#   - Unbalanced `$(` (no closing `)`) is left in place verbatim
#     and the scan continues after the `$(`.
#   - The variable name is `.strip()`ed (matches Rust's `.trim()`)
#     before lookup.
#
# Starlark has no `while` loops, so the outer scan iterates a
# `for _ in range(len(command) + 1)` budget and breaks when the
# cursor reaches the end. Each iteration consumes at least one
# character (or one whole `$(...)` pattern), so the bound is safe.
def _kuro_expand_make_variables(raw_ctx, attribute_name, command, additional_substitutions):
    # `attribute_name` is accepted for Bazel signature parity (the
    # Rust impl ignored it too — it was only ever used in error
    # messages, none of which were ever emitted).
    _ = attribute_name  # buildifier: disable=unused-variable

    subs = {}
    if additional_substitutions != None:
        for k, v in additional_substitutions.items():
            subs[k] = v
    for k, v in _kuro_make_substitutions(raw_ctx).items():
        if k not in subs:
            subs[k] = v

    n = len(command)
    result = ""
    i = 0
    for _step in range(n + 1):
        if i >= n:
            break
        start = command.find("$(", i)
        if start < 0:
            result += command[i:]
            break
        result += command[i:start]
        end = command.find(")", start + 2)
        if end < 0:
            # Unbalanced `$(`: emit it literally and resume after.
            result += "$("
            i = start + 2
            continue
        name = command[start + 2:end].strip()
        if name in subs:
            result += subs[name]
        else:
            result += command[start:end + 1]
        i = end + 1
    return result

# Plan 28.4 Stage 14: Starlark replacement for the deleted Rust impl
# of `ctx.runfiles` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
#
# Bazel API:
#   ctx.runfiles(
#       files=None, *, transitive_files=None, collect_default=False,
#       collect_data=False, symlinks=None, root_symlinks=None,
#   ) -> Runfiles
#
# Builds a Runfiles object from explicit `files` / `transitive_files` /
# `symlinks` / `root_symlinks` via `kuro_create_runfiles`, then
# optionally extends it by walking `deps` / `runtime_deps` / `data`
# attrs via `kuro_collect_runfiles_into`. Both kuro_runtime globals keep
# the Runfiles construction and dep-merging logic on the Rust side.
#
# `raw_ctx` is captured by the closure `_runfiles_bound` in
# `_make_rule_facade` so the helper can read `raw_ctx.attrs` when
# `collect_default` or `collect_data` is set.
def _kuro_runfiles(raw_ctx, files, transitive_files, collect_default, collect_data, symlinks, root_symlinks):
    rf = kuro_create_runfiles(files, transitive_files, symlinks, root_symlinks)
    if collect_default or collect_data:
        attrs = raw_ctx.attrs
        if attrs != None:
            if collect_default:
                v = getattr(attrs, "deps", None)
                if v != None:
                    rf = kuro_collect_runfiles_into(rf, v, False)
                v = getattr(attrs, "runtime_deps", None)
                if v != None:
                    rf = kuro_collect_runfiles_into(rf, v, False)
            if collect_data:
                v = getattr(attrs, "data", None)
                if v != None:
                    rf = kuro_collect_runfiles_into(rf, v, True)
    return rf

# Plan 28.4 Stage 11: Starlark replacement for the deleted Rust impl
# of `ctx.resolve_tools` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
#
# Bazel API: `ctx.resolve_tools(*, tools=None) -> (list_of_files, [])`.
# Iterates `tools` (a list of Dependency values), collects each dep's
# `DefaultInfo.default_outputs` into a flat list, and returns a tuple
# of `(files_list, empty_manifests_list)`. Kuro does not use runfiles
# manifests, so the second element is always an empty list.
#
# `dep[DefaultInfo].default_outputs` is the canonical form used
# throughout the prelude (see e.g. `prelude/artifacts.bzl`,
# `prelude/utils/utils.bzl`, `prelude/command_alias.bzl`).
def _kuro_resolve_tools(tools = None):
    tool_files = []
    if tools != None:
        for dep in tools:
            if DefaultInfo in dep:
                tool_files.extend(dep[DefaultInfo].default_outputs)
    return (tool_files, [])

# Plan 28.4 Stage 12: Starlark replacement for the deleted Rust impl
# of `ctx.resolve_command` in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs`.
#
# Deprecated Bazel API:
#   ctx.resolve_command(
#       *, command="", attribute=None, expand_locations=False,
#       make_variables=None, tools=None, label_dict=None,
#       execution_requirements=None,
#   ) -> (inputs_list, command_list, manifests_list)
#
# Collects input files from `tools` and `label_dict` via
# DefaultInfo.default_outputs, optionally runs $(location ...) expansion
# via raw_ctx.expand_location, then applies literal $(KEY) → value
# replacement for each entry in `make_variables`. Returns a 3-tuple
# (inputs, [resolved_command], []) — the manifests list is always
# empty because Kuro does not use runfiles manifests.
#
# `attribute` and `execution_requirements` are accepted and ignored,
# matching the Rust impl's `let _ = (attribute, execution_requirements)`.
def _kuro_resolve_command(
        raw_ctx,
        command,
        attribute,
        expand_locations,
        make_variables,
        tools,
        label_dict,
        execution_requirements):
    _ = (attribute, execution_requirements)  # accepted, ignored — mirrors Rust impl

    # Collect DefaultInfo.default_outputs from tools and label_dict.
    tool_files = []
    all_targets = []
    for dep in (tools or []):
        all_targets.append(dep)
        if DefaultInfo in dep:
            tool_files.extend(dep[DefaultInfo].default_outputs)
    for dep in (label_dict or []):
        all_targets.append(dep)
        if DefaultInfo in dep:
            tool_files.extend(dep[DefaultInfo].default_outputs)

    resolved = command
    if expand_locations and command:
        resolved = raw_ctx.expand_location(resolved, targets = all_targets)

    if make_variables != None:
        for key, val in make_variables.items():
            if type(val) == "string":
                resolved = resolved.replace("$(" + key + ")", val)

    return (tool_files, [resolved], [])

def _kuro_target_platform_has_constraint(constraint_value):
    # ConstraintValueInfo exposes the constraint's canonical label as
    # `.label`. Anything else (None, missing attr) maps to False, just
    # like the Rust impl.
    label_attr = getattr(constraint_value, "label", None)
    if label_attr == None:
        return False
    label_str = str(label_attr)
    for candidate in HOST_CONSTRAINT_LABELS:
        if not candidate:
            # Tombstone for unsupported host OS/CPU at build.rs time.
            continue
        if label_str == candidate:
            return True
        no_at = candidate[1:] if candidate.startswith("@") else candidate
        if label_str == no_at:
            return True
        idx = no_at.find("//")
        if idx >= 0 and label_str.endswith(no_at[idx:]):
            return True
    return False

# Plan 28.4 Stage 3: install a Starlark facade around `raw_ctx` so
# individual `ctx`-method bodies can move from Rust into Starlark
# without touching the analysis call site. The facade is a `struct`
# that mirrors every public field on the underlying `AnalysisContext`,
# with the migrated methods replaced by Starlark closures.
#
# Two invariants this code relies on:
#
#   1. For user-defined `rule()` impls (the only callers of this
#      wrapper — see `RuleSpec::invoke` in
#      `app/kuro_analysis/src/analysis/env.rs`), every attribute below
#      is available without raising. The "not available for
#      `dynamic_output` or BXL" attribute paths are not reachable here.
#
#   2. Bound-method values returned by `raw_ctx.<method>` for
#      non-migrated methods (e.g. `new_file`, `expand_location`)
#      are first-class Starlark values that re-bind to `raw_ctx` when
#      called. Storing them as struct fields preserves call semantics.
#
# Adding a new ctx field anywhere in
# `app/kuro_build_api/src/interpreter/rule_defs/context.rs` requires
# adding a corresponding line in `_make_rule_facade` below; the
# kuro_facade_drift_guard test
# (tests/core/analysis/test_native_rules.py) compares `dir(raw_ctx)`
# against this list and fails loudly when they diverge.
#
# `kind` distinguishes which wrapper produced the facade — Stage 5's
# subrule wrapper reuses the same field set but tags itself
# differently so acceptance tests can prove which dispatch path ran.
def _make_rule_facade(raw_ctx, kind):
    # Closure binding `raw_ctx` for `package_relative_label`, which
    # needs to read `raw_ctx.label` at call time but takes only the
    # label string from the user — mirrors the Rust impl's signature.
    def _package_relative_label_bound(label_str):
        return _kuro_package_relative_label(raw_ctx, label_str)

    # Plan 28.4 Stage 9: closure binding `raw_ctx` for
    # `expand_make_variables`. The substitution table reads
    # `raw_ctx.bin_dir.path`, `raw_ctx.label.workspace_root`, and
    # `raw_ctx.attrs.toolchains`; user-provided `additional_substitutions`
    # is the only argument from the call site. Default is `None` to
    # match Bazel's signature (which uses an empty dict default —
    # treated identically here).
    def _expand_make_variables_bound(attribute_name, command, additional_substitutions = None):
        return _kuro_expand_make_variables(
            raw_ctx,
            attribute_name,
            command,
            additional_substitutions,
        )

    # Plan 28.4 Stage 14: closure binding `raw_ctx` for `runfiles`.
    # `raw_ctx` is captured so `_kuro_runfiles` can access `raw_ctx.attrs`
    # when `collect_default` or `collect_data` is True. Signature matches
    # Bazel's `ctx.runfiles`: `files` positional-or-keyword, the rest keyword-only.
    def _runfiles_bound(
            files = None,
            transitive_files = None,
            collect_default = False,
            collect_data = False,
            symlinks = None,
            root_symlinks = None):
        return _kuro_runfiles(
            raw_ctx,
            files,
            transitive_files,
            collect_default,
            collect_data,
            symlinks,
            root_symlinks,
        )

    # Plan 28.4 Stage 12: closure binding `raw_ctx` for `resolve_command`.
    # The helper needs `raw_ctx.expand_location` for the $(location ...)
    # expansion step; all other args are forwarded verbatim. Signature
    # matches Bazel's: all kwargs, all with defaults.
    def _resolve_command_bound(
            command = "",
            attribute = None,
            expand_locations = False,
            make_variables = None,
            tools = None,
            label_dict = None,
            execution_requirements = None):
        return _kuro_resolve_command(
            raw_ctx,
            command,
            attribute,
            expand_locations,
            make_variables,
            tools,
            label_dict,
            execution_requirements,
        )

    # Plan 28.4 Stage 10: closure binding `raw_ctx` for `new_file`.
    # The deprecated two-shape API (`new_file(filename)` and
    # `new_file(sibling, filename)`) is normalised here; the sibling is
    # ignored to match the deleted Rust impl byte-for-byte.
    def _new_file_bound(file_or_sibling, filename = None):
        return _kuro_new_file(raw_ctx, file_or_sibling, filename)

    return struct(
        # ---- AnalysisContext attributes (#[starlark(attribute)]) ----
        attrs = raw_ctx.attrs,
        actions = raw_ctx.actions,
        label = raw_ctx.label,
        plugins = raw_ctx.plugins,
        attr = raw_ctx.attr,
        split_attr = raw_ctx.split_attr,
        workspace_name = raw_ctx.workspace_name,
        build_file_path = raw_ctx.build_file_path,
        fragments = raw_ctx.fragments,
        host_fragments = raw_ctx.host_fragments,
        toolchains = raw_ctx.toolchains,
        outputs = raw_ctx.outputs,
        features = raw_ctx.features,
        disabled_features = raw_ctx.disabled_features,
        configuration = raw_ctx.configuration,
        files = raw_ctx.files,
        file = raw_ctx.file,
        executable = raw_ctx.executable,
        bin_dir = raw_ctx.bin_dir,
        genfiles_dir = raw_ctx.genfiles_dir,
        version_file = raw_ctx.version_file,
        info_file = raw_ctx.info_file,
        exec_groups = raw_ctx.exec_groups,
        var = _kuro_var(raw_ctx),
        build_setting_value = raw_ctx.build_setting_value,
        # ---- AnalysisContext methods served from Starlark ----
        target_platform_has_constraint = _kuro_target_platform_has_constraint,
        package_relative_label = _package_relative_label_bound,
        tokenize = _kuro_tokenize,
        coverage_instrumented = _kuro_coverage_instrumented,
        expand_make_variables = _expand_make_variables_bound,
        # ---- AnalysisContext methods passed through (bound to raw_ctx) ----
        runfiles = _runfiles_bound,
        resolve_tools = _kuro_resolve_tools,
        resolve_command = _resolve_command_bound,
        new_file = _new_file_bound,
        expand_location = raw_ctx.expand_location,
        # ---- Acceptance markers (kuro_*-prefixed). Used by Stage 3/5
        #      tests to prove which wrapper produced the facade. Not
        #      Bazel builtins; not part of the rule-author contract.
        kuro_facade_active = True,
        kuro_facade_kind = kind,
    )

def _invoke_rule(implementation, raw_ctx):
    return implementation(_make_rule_facade(raw_ctx, "rule"))

# Plan 28.4 Stage 5: subrule-side wrapper. Subrules are invoked from
# inside a rule impl; the dispatch site
# (`app/kuro_interpreter_for_build/src/subrule.rs`) reaches the
# wrapper via TLS set by `RuleSpec::Impl::invoke`. Subrule impls have
# the shape `def _impl(ctx, **kwargs)`, so the wrapper signature is
# `wrapper(impl, ctx, **kwargs)` — kwargs forward verbatim.
#
# Subrule contexts are the same `AnalysisContext` type as the
# enclosing rule, so the facade shares `_make_rule_facade`. Only the
# `kuro_facade_kind` tag differs so tests can confirm which dispatch
# path produced the struct.
def _invoke_subrule(implementation, raw_ctx, **kwargs):
    return implementation(_make_rule_facade(raw_ctx, "subrule"), **kwargs)

# Plan 28.4 Stage 4: aspect-side facade. Mirrors
# `_invoke_rule` but for `AspectContext`. Aspect impls are called as
# `impl(target, ctx)` (two positional args) so the wrapper signature is
# `wrapper(impl, target, raw_ctx)`. The dispatch site for aspects lives
# in `app/kuro_analysis/src/analysis/aspect_calculation.rs` (see Stage 4
# wiring in this commit).
#
# Field set is the AspectContext public surface in
# `app/kuro_build_api/src/interpreter/rule_defs/aspect/context.rs`.
# Smaller than rule context — no `attrs`, `outputs`, `executable`, etc.
# `target_platform_has_constraint` was deleted in Stage 3 from the Rust
# AspectContext too; here we install the same Starlark shim the rule
# facade uses, which means aspects can now answer the question
# meaningfully (instead of the previous unconditional `False`).
def _invoke_aspect(implementation, target, raw_ctx):
    return implementation(target, struct(
        # ---- AspectContext attributes (#[starlark(attribute)]) ----
        attr = raw_ctx.attr,
        actions = raw_ctx.actions,
        label = raw_ctx.label,
        rule = raw_ctx.rule,
        fragments = raw_ctx.fragments,
        host_fragments = raw_ctx.host_fragments,
        toolchains = raw_ctx.toolchains,
        features = raw_ctx.features,
        disabled_features = raw_ctx.disabled_features,
        bin_dir = raw_ctx.bin_dir,
        genfiles_dir = raw_ctx.genfiles_dir,
        configuration = raw_ctx.configuration,
        aspect_ids = raw_ctx.aspect_ids,
        build_file_path = raw_ctx.build_file_path,
        workspace_name = raw_ctx.workspace_name,
        # ---- AspectContext methods served from Starlark ----
        target_platform_has_constraint = _kuro_target_platform_has_constraint,
        # ---- AspectContext methods passed through (bound to raw_ctx) ----
        coverage_instrumented = raw_ctx.coverage_instrumented,
        # ---- Stage 4 acceptance marker (kuro_*-prefixed). Same shape as
        #      Stage 3's rule-facade marker but disambiguated so the
        #      acceptance test can prove which wrapper ran.
        kuro_facade_active = True,
        kuro_facade_kind = "aspect",
    ))

# -----------------------------------------------------------------------
# Plan 28 export contract.
# -----------------------------------------------------------------------

# Symbols visible at the top level of every BUILD and `.bzl` file. The
# autoload in `interpreter_for_dir.rs::create_env` iterates this dict
# and copies each (name, value) into the consuming module's env.
# Visibility-control lives here, not in the interpreter — adding a name
# is an explicit decision in this file.
exported_toplevels = {
    # Phase 28.2 probe; kept under a `kuro_builtins_*` name to flag that
    # it is not a Bazel builtin. Used by
    # `tests/core/analysis/test_native_rules.py::test_28_2_kuro_builtins_visible_in_external_bzl`.
    "kuro_builtins_probe": _kuro_builtins_probe_value,
}

# Phase 28.4 wrapper hook. Not in `exported_toplevels` — analysis pulls
# it directly via the bundled module, not via the user-visible env.
rule_implementation_wrapper = _invoke_rule

# Phase 28.4 Stage 4 aspect-wrapper hook. Picked up by
# `aspect_calculation.rs::execute_aspect`; same not-exported semantics
# as `rule_implementation_wrapper`.
aspect_implementation_wrapper = _invoke_aspect

# Phase 28.4 Stage 5 subrule-wrapper hook. Picked up by
# `kuro_interpreter_for_build::subrule::FrozenStarlarkSubruleCallable::invoke`
# via TLS set in `RuleSpec::Impl::invoke`. Same not-exported semantics
# as the rule/aspect hooks.
subrule_implementation_wrapper = _invoke_subrule
