def _impl(ctx):
    ctx.file("tools/cpp/empty.cc", "int main() {}")

    cc = ctx.which("gcc")
    if cc == None:
        fail("gcc was not found through the fixture PATH")

    version = ctx.execute([cc, "-v"])
    if version.return_code != 0 or "gcc version " not in version.stderr:
        fail("unexpected gcc -v result: rc=%s, stderr=%r" % (version.return_code, version.stderr))

    identity = ctx.execute([cc, "--version"])
    if identity.return_code != 0 or not (identity.stdout.startswith("gcc ") or identity.stdout.startswith("gcc-")):
        fail("unexpected gcc --version result: rc=%s, stdout=%r" % (identity.return_code, identity.stdout))

    option_probe = ctx.execute([
        cc,
        "-fno-canonical-system-headers",
        "-o",
        "/dev/null",
        "-c",
        str(ctx.path("tools/cpp/empty.cc")),
    ])
    if option_probe.return_code != 0 or option_probe.stdout != "":
        fail("unexpected gcc option probe result: rc=%s, stdout=%r, stderr=%r" % (option_probe.return_code, option_probe.stdout, option_probe.stderr))

    ctx.file("BUILD.bazel", "filegroup(name = 'gcc_probes')\n")

gcc_repo = repository_rule(implementation = _impl, environ = ["PATH", "LC_ALL"])
