# Test a very simple repository rule that uses repository_ctx
def _simple_repo_impl(ctx):
    # Test ctx.name
    print("ctx.name =", ctx.name)

    # Test ctx.os attributes
    print("ctx.os.name =", ctx.os.name)
    print("ctx.os.arch =", ctx.os.arch)

    # Test ctx.getenv
    home = ctx.getenv("HOME", "/tmp")
    print("HOME =", home)

    # Test ctx.file() - create a BUILD file
    ctx.file("BUILD", 'filegroup(name = "empty", srcs = [])\n')

    # Test ctx.which()
    bash = ctx.which("bash")
    print("bash =", bash)

    # Test ctx.execute()
    result = ctx.execute(["uname", "-s"])
    print("uname -s stdout:", result.stdout.strip())
    print("uname -s return_code:", result.return_code)

simple_repo = repository_rule(
    implementation = _simple_repo_impl,
    attrs = {
        "test_attr": attr.string(default = "hello"),
    },
)
