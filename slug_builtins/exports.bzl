# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# Bundled Bazel-Compatible Builtins.
#
# This file is the entry point of the @slug_builtins bundled cell. The
# slug interpreter auto-loads it into every BUILD and `.bzl` evaluation
# context (per `bazel_builtins_autoload` in
# `app/slug_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`).
#
# The export contract (mirrored after Bonanza's
# `builtins_core/exports.bzl`):
#
#   - `exported_toplevels`: symbols visible at the top level of every
#     BUILD and `.bzl` file. Each entry must have a Bazel 9 parity
#     citation (or a `_slug_*` prefix indicating it is slug-internal,
#     e.g. probes for tests).
#   - `exported_native`: BUCK-file-only globals (members are injected as
#     BUCK globals but invisible in `.bzl` files; mirrors Bazel's
#     `native` struct semantics).
#   - `rule_implementation_wrapper` / `aspect_implementation_wrapper` /
#     `subrule_implementation_wrapper`: route Starlark rule/aspect/subrule
#     analysis through `_make_rule_facade`, which installs a Starlark
#     `ctx` facade so `ctx`-method bodies live in this file rather than
#     in Rust.
#
# Adding a symbol here means committing to a single owner: Rust
# primitive, Starlark export, or external ruleset — never two of the
# three.

load(":_host_constants.bzl", "HOST_CONSTRAINT_LABELS")

# -----------------------------------------------------------------------
# Private helpers (not exported, hidden by leading underscore).
# -----------------------------------------------------------------------

# Probe symbol. Not a Bazel builtin — exists solely to verify that the
# autoload mechanism reaches external `.bzl` files.
_slug_builtins_probe_value = "slug-28-2-loader-ok"

# Resolves a label string against the BUILD file's package (the target's
# package), distinct from the `Label()` builtin which resolves against
# the *file* where it appears. When `raw_ctx.label` is `None`
# (dynamic_output / BXL contexts), falls through to the
# file-cell-resolving `Label()` builtin.
def _slug_package_relative_label(raw_ctx, label_str):
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

# Returns the per-build `--collect_code_coverage` flag (via the
# `slug_collect_code_coverage` runtime global). `dep` is accepted for
# Bazel signature parity but ignored; if slug grows per-target
# instrumentation lists, branch on `dep != None` here.
def _slug_coverage_instrumented(dep = None):  # buildifier: disable=unused-variable
    return slug_collect_code_coverage()

# Bourne-shell tokenization for `ctx.tokenize`. Pure function — no
# facade-attr access, no host info, no globals.
#
#   - Single-quoted strings: literal until closing `'`, no escapes.
#   - Double-quoted strings: backslash escapes for `"`, `\`, `$`,
#     `` ` ``; all other characters literal; trailing `\` at end of
#     input dropped silently.
#   - Backslash outside quotes: consume next char literally;
#     trailing `\` at end of input dropped silently.
#   - ASCII whitespace splits tokens (space, `\t`, `\n`, `\f` /
#     `\x0c`, `\r`).
#
# Starlark has no `while` loops, so the iteration uses a for-loop
# over `range(n + 1)` with explicit `i` advancement and `break`
# when `i >= n`. Each outer step consumes at least one input
# character, so the bound is safe.
def _slug_tokenize(option_string):
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

# `ctx.new_file` is a deprecated Bazel API with two call shapes:
#   - `ctx.new_file(filename: str)` — declare a new file by name.
#   - `ctx.new_file(sibling: File, filename: str)` — same, but the
#     sibling is ignored.
def _slug_new_file(raw_ctx, file_or_sibling, filename):
    name = filename if filename != None else file_or_sibling
    if type(name) != "string":
        name = str(name)
    return raw_ctx.actions.declare_file(name)

# `ctx.var` and `ctx.expand_make_variables` share the same `$(VAR)`
# substitution table; building it here keeps the table in one place.
# Priority order, highest to lowest:
#
#   1. User-provided `additional_substitutions` (only
#      `expand_make_variables`; `var` skips this layer).
#   2. Built-in Make variables (BINDIR, GENDIR, TARGET_CPU,
#      COMPILATION_MODE, WORKSPACE_ROOT, CC, CC_FLAGS, JAVA,
#      JAVA_RUNFILES, JAVABASE, ABI_GLIBC_VERSION, ABI,
#      STACK_FRAME_UNLIMITED). `STACK_FRAME_UNLIMITED` is an
#      llvm-project requirement (see memory/ctx_var_builtins.md).
#   3. `TemplateVariableInfo` from each dep in `ctx.attrs.toolchains`.
#   4. `--define KEY=VALUE` flags (lowest priority).
def _slug_make_substitutions(raw_ctx):
    bin_dir = raw_ctx.bin_dir.path
    label = raw_ctx.label
    workspace_root = label.workspace_root if label != None else ""

    subs = {
        "BINDIR": bin_dir,
        "GENDIR": bin_dir,
        "TARGET_CPU": slug_host_target_cpu(),
        "COMPILATION_MODE": slug_compilation_mode_for_label(label),
        "WORKSPACE_ROOT": workspace_root,
        "CC": slug_host_cc_path(),
        "CC_FLAGS": "",
        # Bazel uses "java.exe" on Windows and "/usr/bin/java"
        # elsewhere. Slug is Linux-first; this matches Bazel for the
        # only platform we currently care about.
        "JAVA": "/usr/bin/java",
        "JAVA_RUNFILES": "",
        "JAVABASE": "",
        "ABI_GLIBC_VERSION": "2.17",
        "ABI": "local",
        # Normally seeded by rules_cc's cc_toolchain via
        # TemplateVariableInfo; slug's stub cc_toolchain doesn't
        # publish that provider, so ship the default here.
        "STACK_FRAME_UNLIMITED": "",
    }

    # `ctx.attrs.toolchains`: list of deps whose `TemplateVariableInfo`
    # is exposed to the target. Mirrors Bazel's
    # `RuleContext.getMakeVariables()`. Builtins win on collision.
    attrs = raw_ctx.attrs
    if attrs != None:
        toolchains_attr = getattr(attrs, "toolchains", None)
        if toolchains_attr != None:
            for k, v in slug_collect_toolchains_template_vars(toolchains_attr).items():
                if k not in subs:
                    subs[k] = v

    # `--define KEY=VALUE` flags. Lowest priority — each builtin and
    # each TemplateVariableInfo entry already wins on collision.
    for k, v in slug_get_all_defines().items():
        if k not in subs:
            subs[k] = v

    return subs

def _slug_var(raw_ctx):
    return _slug_make_substitutions(raw_ctx)

# Parses `$(VAR)` patterns in `command` and substitutes from the merged
# table:
#
#   - User `additional_substitutions` (an optional dict) win over all
#     other layers.
#   - Unresolved `$(VAR)` patterns are left in place verbatim.
#   - Unbalanced `$(` (no closing `)`) is left in place verbatim and
#     the scan continues after the `$(`.
#   - The variable name is `.strip()`ed before lookup.
#
# Starlark has no `while` loops, so the outer scan iterates a
# `for _ in range(len(command) + 1)` budget and breaks when the cursor
# reaches the end. Each iteration consumes at least one character (or
# one whole `$(...)` pattern), so the bound is safe.
def _slug_expand_make_variables(raw_ctx, attribute_name, command, additional_substitutions):
    # `attribute_name` is accepted for Bazel signature parity but
    # unused — error messages with it are never emitted today.
    _ = attribute_name  # buildifier: disable=unused-variable

    subs = {}
    if additional_substitutions != None:
        for k, v in additional_substitutions.items():
            subs[k] = v
    for k, v in _slug_make_substitutions(raw_ctx).items():
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

# Bazel API:
#   ctx.runfiles(
#       files=None, *, transitive_files=None, collect_default=False,
#       collect_data=False, symlinks=None, root_symlinks=None,
#   ) -> Runfiles
#
# Builds a Runfiles object via `slug_create_runfiles`, then optionally
# extends it by walking `deps` / `runtime_deps` / `data` attrs via
# `slug_collect_runfiles_into`. Both runtime globals keep the Runfiles
# construction and dep-merging logic on the Rust side.
def _slug_runfiles(raw_ctx, files, transitive_files, collect_default, collect_data, symlinks, root_symlinks):
    rf = slug_create_runfiles(files, transitive_files, symlinks, root_symlinks)
    if collect_default or collect_data:
        attrs = raw_ctx.attrs
        if attrs != None:
            if collect_default:
                v = getattr(attrs, "deps", None)
                if v != None:
                    rf = slug_collect_runfiles_into(rf, v, False)
                v = getattr(attrs, "runtime_deps", None)
                if v != None:
                    rf = slug_collect_runfiles_into(rf, v, False)
            if collect_data:
                v = getattr(attrs, "data", None)
                if v != None:
                    rf = slug_collect_runfiles_into(rf, v, True)
    return rf

# Bazel API: `ctx.resolve_tools(*, tools=None) -> (list_of_files, [])`.
# Iterates `tools` (Dependency values), collects each dep's
# `DefaultInfo.default_outputs` into a flat list, and returns
# `(files_list, [])`. Slug does not use runfiles manifests; the second
# tuple element is always empty.
def _slug_resolve_tools(tools = None):
    tool_files = []
    if tools != None:
        for dep in tools:
            if DefaultInfo in dep:
                tool_files.extend(dep[DefaultInfo].default_outputs)
    return (tool_files, [])

# Deprecated Bazel API:
#   ctx.resolve_command(
#       *, command="", attribute=None, expand_locations=False,
#       make_variables=None, tools=None, label_dict=None,
#       execution_requirements=None,
#   ) -> (inputs_list, command_list, manifests_list)
#
# Collects input files from `tools` and `label_dict`, optionally runs
# $(location ...) expansion, then applies literal `$(KEY)` → value
# replacement from `make_variables`. `attribute` and
# `execution_requirements` are accepted and ignored.
def _slug_resolve_command(
        raw_ctx,
        command,
        attribute,
        expand_locations,
        make_variables,
        tools,
        label_dict,
        execution_requirements):
    _ = (attribute, execution_requirements)  # accepted, ignored

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
        resolved = _slug_expand_location(raw_ctx, resolved, all_targets, False)

    if make_variables != None:
        for key, val in make_variables.items():
            if type(val) == "string":
                resolved = resolved.replace("$(" + key + ")", val)

    return (tool_files, [resolved], [])

# `ctx.expand_location` Starlark impl. The label→paths pool lives
# Rust-side via `slug_collect_location_pool` because it needs
# `StructRef::iter()` over the attrs struct plus `downcast_ref` type
# discrimination over Dependency / StarlarkArtifact /
# StarlarkDeclaredArtifact — none reachable from Starlark.
#
# `slug_lookup_output_path` resolves `attr.output` / `attr.output_list`
# label strings lazily, preserving the deferred-declaration invariant:
# only the specific output attribute whose string list contains the
# query label triggers `CtxOutputs.declare_output`.
#
# The parser (recognising `$(location ...)`, `$(locations ...)`,
# `$(execpath ...)`, `$(execpaths ...)`, `$(rootpath ...)`,
# `$(rootpaths ...)`, `$(rlocationpath ...)`, `$(rlocationpaths ...)`)
# and the label-matching logic (`_find_paths_for_label`) live in
# Starlark.
#
# `short_paths=True` is accepted for Bazel signature parity but is a
# no-op here (all path verbs resolve to the same full artifact path).

def _find_paths_for_label(pool, label_str):
    # Try exact match, then target-name fallback, then path-suffix match.
    # Mirrors the deleted Rust `find_paths` closure byte-for-byte.
    # `pool` is a list of [label_str, [paths]] pairs returned by
    # `slug_collect_location_pool`.
    query_name = label_str.lstrip(":")
    for entry in pool:
        dep_label = entry[0]
        paths = entry[1]
        if dep_label == label_str:
            return paths

        # Target-name match: "//pkg:target" → "target".
        colon_idx = dep_label.rfind(":")
        dep_name = dep_label[colon_idx + 1:] if colon_idx >= 0 else dep_label
        if dep_name == query_name:
            return paths

        # Path-suffix match for source files in external cells whose
        # short_path is `../<repo>/<pkg>/<rel>` while the user wrote
        # `<rel>` in the BUILD file.
        if dep_label.endswith("/" + query_name):
            return paths
    return None

def _slug_expand_location(raw_ctx, input, targets, short_paths):
    # `short_paths` is accepted for Bazel API parity; both the old Rust
    # impl and this Starlark replacement resolve all path verbs to the
    # same full artifact path, so the flag is a no-op.
    _ = short_paths  # buildifier: disable=unused-variable

    # Build the pool: explicit targets + implicit attrs walk (Rust-side).
    targets_val = targets if targets != None else []
    pool = slug_collect_location_pool(raw_ctx, targets_val)

    # Parse and substitute $(verb label) patterns using a
    # for-loop-with-cursor (Starlark has no `while`).
    n = len(input)
    result = ""
    i = 0
    for _step in range(n + 1):
        if i >= n:
            break
        start = input.find("$(", i)
        if start < 0:
            result += input[i:]
            break
        result += input[i:start]

        # Determine which verb (if any) follows "$(". Check longest
        # prefixes first to avoid "locations" matching as "location ".
        tail = input[start:]
        verb = None
        plural = False
        label_offset = 0
        if tail.startswith("$(locations "):
            verb = "locations"
            plural = True
            label_offset = len("$(locations ")
        elif tail.startswith("$(location "):
            verb = "location"
            plural = False
            label_offset = len("$(location ")
        elif tail.startswith("$(execpaths "):
            verb = "execpaths"
            plural = True
            label_offset = len("$(execpaths ")
        elif tail.startswith("$(execpath "):
            verb = "execpath"
            plural = False
            label_offset = len("$(execpath ")
        elif tail.startswith("$(rootpaths "):
            verb = "rootpaths"
            plural = True
            label_offset = len("$(rootpaths ")
        elif tail.startswith("$(rootpath "):
            verb = "rootpath"
            plural = False
            label_offset = len("$(rootpath ")
        elif tail.startswith("$(rlocationpaths "):
            verb = "rlocationpaths"
            plural = True
            label_offset = len("$(rlocationpaths ")
        elif tail.startswith("$(rlocationpath "):
            verb = "rlocationpath"
            plural = False
            label_offset = len("$(rlocationpath ")

        if verb != None:
            close = input.find(")", start + label_offset)
            if close >= 0:
                label_str = input[start + label_offset:close].strip()
                paths = _find_paths_for_label(pool, label_str)
                if paths == None:
                    # Fall through to the lazy output-attr lookup.
                    paths_val = slug_lookup_output_path(raw_ctx, label_str)
                    if paths_val != None:
                        paths = [paths_val]
                if paths != None:
                    if plural:
                        result += " ".join(paths)
                    else:
                        result += paths[0] if paths else ""
                else:
                    # Label not found — keep verbatim (mirrors Rust).
                    result += input[start:close + 1]
                i = close + 1
                continue

        # Not a location pattern — emit "$(" literally and advance.
        result += "$("
        i = start + 2
    return result

def _slug_target_platform_has_constraint(constraint_value):
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

# Install a Starlark facade around `raw_ctx` so `ctx`-method bodies
# can live in this file rather than in Rust. The facade is a `struct`
# mirroring every public field on the underlying `AnalysisContext`,
# with the migrated methods replaced by Starlark closures.
#
# Two invariants this code relies on:
#
#   1. For user-defined `rule()` impls (the only callers of this
#      wrapper — see `RuleSpec::invoke` in
#      `app/slug_analysis/src/analysis/env.rs`), every attribute below
#      is available without raising. The "not available for
#      `dynamic_output` or BXL" attribute paths are not reachable here.
#
#   2. Bound-method values returned by `raw_ctx.<method>` for
#      non-migrated methods are first-class Starlark values that
#      re-bind to `raw_ctx` when called. Storing them as struct fields
#      preserves call semantics.
#
# Adding a new ctx field in
# `app/slug_build_api/src/interpreter/rule_defs/context.rs` requires
# adding a corresponding line in `_make_rule_facade` below.
#
# `kind` distinguishes which wrapper produced the facade (rule /
# aspect / subrule) so acceptance tests can prove which dispatch path
# ran.
def _make_rule_facade(raw_ctx, kind):
    # Closure binding `raw_ctx` for `package_relative_label`, which
    # needs to read `raw_ctx.label` at call time but takes only the
    # label string from the user — mirrors the Rust impl's signature.
    def _package_relative_label_bound(label_str):
        return _slug_package_relative_label(raw_ctx, label_str)

    # The substitution table reads `raw_ctx.bin_dir.path`,
    # `raw_ctx.label.workspace_root`, and `raw_ctx.attrs.toolchains`.
    # `additional_substitutions` defaults to None to match Bazel's
    # signature (Bazel uses an empty dict default — equivalent here).
    def _expand_make_variables_bound(attribute_name, command, additional_substitutions = None):
        return _slug_expand_make_variables(
            raw_ctx,
            attribute_name,
            command,
            additional_substitutions,
        )

    # `_slug_runfiles` reads `raw_ctx.attrs` when `collect_default` or
    # `collect_data` is True. Signature matches Bazel's `ctx.runfiles`:
    # `files` positional-or-keyword, the rest keyword-only.
    def _runfiles_bound(
            files = None,
            transitive_files = None,
            collect_default = False,
            collect_data = False,
            symlinks = None,
            root_symlinks = None):
        return _slug_runfiles(
            raw_ctx,
            files,
            transitive_files,
            collect_default,
            collect_data,
            symlinks,
            root_symlinks,
        )

    # `_slug_resolve_command` needs raw_ctx for the $(location ...)
    # expansion step. Signature matches Bazel's: all kwargs with defaults.
    def _resolve_command_bound(
            command = "",
            attribute = None,
            expand_locations = False,
            make_variables = None,
            tools = None,
            label_dict = None,
            execution_requirements = None):
        return _slug_resolve_command(
            raw_ctx,
            command,
            attribute,
            expand_locations,
            make_variables,
            tools,
            label_dict,
            execution_requirements,
        )

    # `_slug_new_file` normalises the two-shape API
    # (`new_file(filename)` and `new_file(sibling, filename)`); the
    # sibling is ignored.
    def _new_file_bound(file_or_sibling, filename = None):
        return _slug_new_file(raw_ctx, file_or_sibling, filename)

    # `_slug_expand_location` takes raw_ctx so the two runtime hooks
    # (`slug_collect_location_pool` and `slug_lookup_output_path`) can
    # downcast it to `AnalysisContext` and access attrs / outputs.
    def _expand_location_bound(input, targets = None, short_paths = False):
        return _slug_expand_location(raw_ctx, input, targets, short_paths)

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
        var = _slug_var(raw_ctx),
        build_setting_value = raw_ctx.build_setting_value,
        # ---- AnalysisContext methods served from Starlark ----
        target_platform_has_constraint = _slug_target_platform_has_constraint,
        package_relative_label = _package_relative_label_bound,
        tokenize = _slug_tokenize,
        coverage_instrumented = _slug_coverage_instrumented,
        expand_make_variables = _expand_make_variables_bound,
        expand_location = _expand_location_bound,
        # ---- AnalysisContext methods passed through (bound to raw_ctx) ----
        runfiles = _runfiles_bound,
        resolve_tools = _slug_resolve_tools,
        resolve_command = _resolve_command_bound,
        new_file = _new_file_bound,
        # ---- Acceptance markers (slug_*-prefixed). Used by tests to
        #      prove which wrapper produced the facade. Not Bazel
        #      builtins; not part of the rule-author contract.
        slug_facade_active = True,
        slug_facade_kind = kind,
    )

def _invoke_rule(implementation, raw_ctx):
    return implementation(_make_rule_facade(raw_ctx, "rule"))

# Subrule-side wrapper. Subrules are invoked from inside a rule impl;
# the dispatch site (`app/slug_interpreter_for_build/src/subrule.rs`)
# reaches the wrapper via TLS set by `RuleSpec::Impl::invoke`. Subrule
# impls have the shape `def _impl(ctx, **kwargs)`. Subrule contexts
# are the same `AnalysisContext` type as the enclosing rule, so the
# facade shares `_make_rule_facade`.
def _invoke_subrule(implementation, raw_ctx, **kwargs):
    return implementation(_make_rule_facade(raw_ctx, "subrule"), **kwargs)

# Aspect-side facade. Mirrors `_invoke_rule` but for `AspectContext`.
# Aspect impls are called as `impl(target, ctx)`. The dispatch site
# lives in `app/slug_analysis/src/analysis/aspect_calculation.rs`.
#
# AspectContext is smaller than rule context — no `attrs`, `outputs`,
# `executable`, etc. The same Starlark
# `_slug_target_platform_has_constraint` shim rule contexts use is
# installed here, so aspects answer the question meaningfully (the
# previous Rust stub returned False unconditionally).
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
        target_platform_has_constraint = _slug_target_platform_has_constraint,
        # ---- AspectContext methods passed through (bound to raw_ctx) ----
        coverage_instrumented = raw_ctx.coverage_instrumented,
        # ---- Acceptance marker (slug_*-prefixed). Disambiguated so
        #      acceptance tests can prove which wrapper produced the
        #      facade.
        slug_facade_active = True,
        slug_facade_kind = "aspect",
    ))

# -----------------------------------------------------------------------
# Export contract.
# -----------------------------------------------------------------------

# Symbols visible at the top level of every BUILD and `.bzl` file. The
# autoload in `interpreter_for_dir.rs::create_env` iterates this dict
# and copies each (name, value) into the consuming module's env.
# Visibility-control lives here, not in the interpreter — adding a name
# is an explicit decision in this file.
exported_toplevels = {
    # Probe symbol; kept under a `slug_builtins_*` name to flag that
    # it is not a Bazel builtin. Used by
    # `tests/core/analysis/test_native_rules.py::test_28_2_slug_builtins_visible_in_external_bzl`.
    "slug_builtins_probe": _slug_builtins_probe_value,
}

# BUCK-file-only globals. Members are injected as BUCK globals by
# `interpreter_for_dir.rs::create_env` and stay invisible in `.bzl`
# files (mirrors Bazel's `native` struct semantics). User `load()`
# bindings at the BUCK use site shadow these via normal Starlark
# scoping.
exported_native = {
    # Probe — proves the BUCK-file-only injection path. `_slug_*`
    # prefix flags the symbol as slug-internal, not a Bazel builtin.
    # Used by `test_28_5_exported_native_visible_in_buck` and
    # `test_28_5_exported_native_hidden_in_bzl`.
    "_slug_exported_native_probe": "slug-28-5-exported-native-ok",
}

# Wrapper hooks. Not in `exported_toplevels` — analysis pulls them
# directly via the bundled module, not via the user-visible env.
# `rule_implementation_wrapper` is read by `RuleSpec::Impl::invoke`,
# `aspect_implementation_wrapper` by `aspect_calculation.rs::execute_aspect`,
# and `subrule_implementation_wrapper` by
# `slug_interpreter_for_build::subrule::FrozenStarlarkSubruleCallable::invoke`
# (via TLS set in `RuleSpec::Impl::invoke`).
rule_implementation_wrapper = _invoke_rule
aspect_implementation_wrapper = _invoke_aspect
subrule_implementation_wrapper = _invoke_subrule
