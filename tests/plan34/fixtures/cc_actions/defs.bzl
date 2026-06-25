def _cc_smoke_binary_impl(ctx):
    obj = ctx.actions.declare_output(ctx.label.name + ".o")
    main_obj = ctx.actions.declare_output(ctx.label.name + "_main.o")
    exe = ctx.actions.declare_output(ctx.label.name)

    ctx.actions.run(
        [
            "sh",
            "-c",
            "printf 'compiled:%s\n' \"$1\" > \"$2\"",
            "--",
            ctx.file.src,
            obj.as_output(),
        ],
        category = "cc_compile",
        identifier = "hello",
    )
    ctx.actions.run(
        [
            "sh",
            "-c",
            "printf 'compiled:%s\n' \"$1\" > \"$2\"",
            "--",
            ctx.file.main,
            main_obj.as_output(),
        ],
        category = "cc_compile",
        identifier = "main",
    )
    ctx.actions.run(
        [
            "sh",
            "-c",
            "printf 'linked:%s:%s\n' \"$1\" \"$2\" > \"$3\"",
            "--",
            obj,
            main_obj,
            exe.as_output(),
        ],
        category = "cc_link",
        identifier = "hello",
    )

    return [DefaultInfo(exe, other_outputs = [obj, main_obj])]

cc_smoke_binary = rule(
    impl = _cc_smoke_binary_impl,
    attrs = {
        "src": attr.label(allow_single_file = True),
        "main": attr.label(allow_single_file = True),
    },
)
