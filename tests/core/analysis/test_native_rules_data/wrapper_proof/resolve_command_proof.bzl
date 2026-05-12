# Plan 28.4 Stage 12 acceptance fixture for `ctx.resolve_command`.
# The Rust impl was deleted; `_slug_resolve_command` in
# `@slug_builtins//:exports.bzl` (bound as `_resolve_command_bound`
# inside `_make_rule_facade`) now serves the call.
#
# The probe uses two deps to verify input collection from both `tools`
# and `label_dict`, and a command with both a $(KEY) make-variable and
# a $(location ...) pattern to verify both substitution paths.

def _resolve_command_leaf_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "leaf-content\n")
    return [DefaultInfo(default_output = out)]

_resolve_command_leaf = rule(
    implementation = _resolve_command_leaf_impl,
    attrs = {},
)

def _resolve_command_proof_impl(ctx):
    leaf_tool = ctx.attr.tool_dep
    leaf_label = ctx.attr.label_dep

    # The command contains a $(KEY) pattern and a $(location ...) pattern.
    # After resolve_command the $(KEY) must be replaced by "VALUE" and
    # $(location ...) must be replaced by the leaf's output path.
    raw_command = "cmd --flag=$(KEY) --input=$(location :resolve_command_leaf_tool)"

    result = ctx.resolve_command(
        command = raw_command,
        tools = [leaf_tool],
        label_dict = [leaf_label],
        make_variables = {"KEY": "VALUE"},
        expand_locations = True,
    )

    # result must be a 3-tuple
    if len(result) != 3:
        fail("Stage 12: expected 3-tuple, got len=%d" % len(result))

    inputs_list, command_list, manifests_list = result

    # inputs must contain files from both tools (tool_dep) and label_dict (label_dep)
    if len(inputs_list) < 2:
        fail("Stage 12: inputs_list has %d entries, want >= 2" % len(inputs_list))

    # command_list must be a list with exactly one string
    if len(command_list) != 1:
        fail("Stage 12: command_list len=%d, want 1" % len(command_list))

    resolved = command_list[0]

    # $(KEY) must have been replaced by "VALUE"
    if "$(KEY)" in resolved:
        fail("Stage 12: $(KEY) not expanded in %r" % resolved)
    if "VALUE" not in resolved:
        fail("Stage 12: 'VALUE' missing from resolved command %r" % resolved)

    # $(location ...) must have been expanded (no literal remaining)
    if "$(location" in resolved:
        fail("Stage 12: $(location ...) not expanded in %r" % resolved)

    # manifests must be empty
    if len(manifests_list) != 0:
        fail("Stage 12: manifests_list not empty: %r" % manifests_list)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "resolve-command-proof-ok\n")
    return [DefaultInfo(default_output = out)]

resolve_command_proof = rule(
    implementation = _resolve_command_proof_impl,
    attrs = {
        "tool_dep": attr.label(),
        "label_dep": attr.label(),
    },
)

resolve_command_leaf = _resolve_command_leaf
