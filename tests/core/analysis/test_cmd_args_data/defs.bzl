def _args_builder_impl(ctx):
    out = ctx.actions.declare_file("args.txt")

    args = ctx.actions.args()
    args.add("one")
    args.add_all(["two", "three"])
    args.add_joined(["four", "five"], join_with = ",")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_builder = rule(
    implementation = _args_builder_impl,
    attrs = {},
)


def _args_terminate_with_impl(ctx):
    out = ctx.actions.declare_file("args_terminate.txt")

    args = ctx.actions.args()
    args.add_all(["a", "b", "c"], terminate_with = "END")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_terminate_with = rule(
    implementation = _args_terminate_with_impl,
    attrs = {},
)


def _args_before_each_impl(ctx):
    out = ctx.actions.declare_file("args_before_each.txt")

    args = ctx.actions.args()
    args.add_all(["a", "b"], before_each = "--flag")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_before_each = rule(
    implementation = _args_before_each_impl,
    attrs = {},
)


def _args_format_each_impl(ctx):
    out = ctx.actions.declare_file("args_format_each.txt")

    args = ctx.actions.args()
    args.add_all(["foo", "bar"], format_each = "--lib=%s")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_format_each = rule(
    implementation = _args_format_each_impl,
    attrs = {},
)


def _to_upper(s):
    return s.upper()


def _split_b(s):
    if s == "b":
        return ["b1", "b2"]
    return s


def _sequence_map(s):
    if s == "skip_none":
        return None
    if s == "skip_tuple":
        return ()
    if s == "tuple":
        return ("tuple1", "tuple2")
    if s == "list":
        return ["list1", "list2"]
    return s


def _args_map_each_impl(ctx):
    out = ctx.actions.declare_file("args_map_each.txt")

    args = ctx.actions.args()
    args.add_all(["hello", "world"], map_each = _to_upper)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_map_each = rule(
    implementation = _args_map_each_impl,
    attrs = {},
)


def _args_map_each_sequence_impl(ctx):
    out = ctx.actions.declare_file("args_map_each_sequence.txt")

    args = ctx.actions.args()
    args.add_all(["skip_none", "skip_tuple", "tuple", "list", "scalar"], map_each = _sequence_map)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_map_each_sequence = rule(
    implementation = _args_map_each_sequence_impl,
    attrs = {},
)


def _args_uniquify_impl(ctx):
    out = ctx.actions.declare_file("args_uniquify.txt")

    args = ctx.actions.args()
    args.add_all(["a", "b", "a", "c", "b"], uniquify = True)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_uniquify = rule(
    implementation = _args_uniquify_impl,
    attrs = {},
)


def _args_omit_if_empty_impl(ctx):
    out = ctx.actions.declare_file("args_omit_if_empty.txt")

    # With omit_if_empty=True (default), empty list adds nothing
    args = ctx.actions.args()
    args.add("before")
    args.add_all([], omit_if_empty = True)
    args.add("after")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_omit_if_empty = rule(
    implementation = _args_omit_if_empty_impl,
    attrs = {},
)


def _args_output_artifact_impl(ctx):
    out = ctx.actions.declare_file("output_artifact_test.txt")

    src = ctx.file.src

    args = ctx.actions.args()
    # Test that format= works with artifact paths
    args.add(src, format = "--input=%s")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_output_artifact = rule(
    implementation = _args_output_artifact_impl,
    attrs = {
        "src": attr.label(allow_single_file = True),
    },
)


def _args_add_two_arg_impl(ctx):
    """Tests args.add() with 2-arg form: add("--flag", value)."""
    out = ctx.actions.declare_file("args_add_two_arg.txt")

    args = ctx.actions.args()
    args.add("--output", "foo.o")
    args.add("--verbose")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_add_two_arg = rule(
    implementation = _args_add_two_arg_impl,
    attrs = {},
)


def _args_add_all_two_arg_impl(ctx):
    """Tests args.add_all() with 2-arg form: add_all("--flag", values)."""
    out = ctx.actions.declare_file("args_add_all_two_arg.txt")

    args = ctx.actions.args()
    args.add_all("--src", ["a.c", "b.c", "c.c"])

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_add_all_two_arg = rule(
    implementation = _args_add_all_two_arg_impl,
    attrs = {},
)


def _args_add_joined_two_arg_impl(ctx):
    """Tests args.add_joined() with 2-arg form: add_joined("--flag", values, join_with=...)."""
    out = ctx.actions.declare_file("args_add_joined_two_arg.txt")

    args = ctx.actions.args()
    args.add_joined("--srcs", ["a.c", "b.c", "c.c"], join_with = ",")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_add_joined_two_arg = rule(
    implementation = _args_add_joined_two_arg_impl,
    attrs = {},
)


def _args_add_joined_uniquify_impl(ctx):
    """Tests args.add_joined() with uniquify=True."""
    out = ctx.actions.declare_file("args_add_joined_uniquify.txt")

    args = ctx.actions.args()
    args.add_joined(["a", "b", "a", "c", "b"], join_with = ",", uniquify = True)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_add_joined_uniquify = rule(
    implementation = _args_add_joined_uniquify_impl,
    attrs = {},
)


def _args_add_joined_map_each_sequence_impl(ctx):
    out = ctx.actions.declare_file("args_add_joined_map_each_sequence.txt")

    args = ctx.actions.args()
    args.add_joined(["skip_none", "skip_tuple", "tuple", "list", "scalar"], join_with = ":", map_each = _sequence_map)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_add_joined_map_each_sequence = rule(
    implementation = _args_add_joined_map_each_sequence_impl,
    attrs = {},
)


def _depset_values():
    return depset(["a", "b", "a"], transitive = [depset(["c", "b"])])


def _args_depset_add_all_transforms_impl(ctx):
    out = ctx.actions.declare_file("args_depset_add_all_transforms.txt")

    args = ctx.actions.args()
    args.add_all(_depset_values(), before_each = "--x")
    args.add("SEP")
    args.add_all(_depset_values(), map_each = _split_b)
    args.add("SEP")
    args.add_all(_depset_values(), terminate_with = "END")

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_depset_add_all_transforms = rule(
    implementation = _args_depset_add_all_transforms_impl,
    attrs = {},
)


def _args_depset_add_joined_transforms_impl(ctx):
    out = ctx.actions.declare_file("args_depset_add_joined_transforms.txt")

    args = ctx.actions.args()
    args.add_joined(_depset_values(), join_with = ":", format_each = "[%s]")
    args.add_joined(_depset_values(), join_with = ":", map_each = _split_b)
    args.add_joined(_depset_values(), join_with = ":", format_joined = "<%s>")
    args.add_joined(_depset_values(), join_with = ":", uniquify = True)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


args_depset_add_joined_transforms = rule(
    implementation = _args_depset_add_joined_transforms_impl,
    attrs = {},
)


def _output_artifact_in_relative_to_impl(ctx):
    source = ctx.file.source
    out = ctx.actions.declare_file("relative_to_test.txt")

    args = ctx.actions.args()
    args.add(source)

    ctx.actions.write(out, args)
    return [DefaultInfo(default_output = out)]


output_artifact_in_relative_to = rule(
    implementation = _output_artifact_in_relative_to_impl,
    attrs = {
        "source": attr.label(allow_single_file = True),
    },
)
