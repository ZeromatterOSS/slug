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

def _prefix_impl(ctx):
    source = ctx.path(Label("@@prefix_source+//:source.bzl"))
    ctx.symlink(source, "cc_toolchain_config.bzl")
    ctx.symlink(source, "armeabi_cc_toolchain_config.bzl")
    ctx.file("tools/cpp/empty.cc", "int main() {}")

    cc = ctx.which("gcc")
    if cc == None:
        fail("gcc was not found through the fixture PATH")

    ctx.file("validate_static_library.sh", "# inert offline prefix\n", executable = True)
    ctx.file("cc_wrapper.sh", "# inert offline prefix\n", executable = True)
    ctx.file("deps_scanner_wrapper.sh", "# inert offline prefix\n", executable = True)

    linker_probe = ctx.execute([
        cc,
        str(ctx.path("tools/cpp/empty.cc")),
        "-o",
        "/dev/null",
        "-Wl,--start-lib",
        "-Wl,--end-lib",
        "-fuse-ld=lld",
        "-v",
    ])
    if linker_probe.stdout != "" or "gcc version " not in linker_probe.stderr:
        fail("unexpected gcc linker probe result: rc=%s, stdout=%r, stderr=%r" % (linker_probe.return_code, linker_probe.stdout, linker_probe.stderr))

    ctx.file("BUILD.bazel", "filegroup(name = 'linker_prefix_probe')\n")

gcc_prefix_repo = repository_rule(implementation = _prefix_impl, environ = ["PATH", "LC_ALL"])

def _include_prefix_impl(ctx):
    source = ctx.path(Label("@@prefix_source+//:source.bzl"))
    ctx.symlink(source, "cc_toolchain_config.bzl")
    ctx.symlink(source, "armeabi_cc_toolchain_config.bzl")
    ctx.file("tools/cpp/empty.cc", "int main() {}")

    cc = ctx.which("gcc")
    if cc == None:
        fail("gcc was not found through the fixture PATH")

    ctx.file("validate_static_library.sh", "# inert offline prefix\n", executable = True)
    ctx.file("cc_wrapper.sh", "# inert offline prefix\n", executable = True)
    ctx.file("deps_scanner_wrapper.sh", "# inert offline prefix\n", executable = True)
    ctx.file("builtin_include_directory_paths", "synthetic offline include paths\n/usr/include\n")

    option_probe = ctx.execute([
        cc,
        "-Wthread-safety",
        "-o",
        "/dev/null",
        "-c",
        str(ctx.path("tools/cpp/empty.cc")),
    ])
    if option_probe.stdout != "":
        fail("unexpected gcc compiler-option stdout: %r" % option_probe.stdout)
    if option_probe.return_code != 0 and option_probe.stderr == "":
        fail("gcc compiler-option failure had no diagnostic")

    ctx.file("BUILD.bazel", "filegroup(name = 'include_prefix_probe')\n")

gcc_include_prefix_repo = repository_rule(implementation = _include_prefix_impl, environ = ["PATH", "LC_ALL"])
