# Authored caller, not a substitute Cpp/toolchain. The dependency is the complete
# unchanged CcToolchainInfo constructor used by the preceding publication proof.
load("//cc/private/rules_impl:cc_toolchain_info.bzl", "CcToolchainInfo")
load("//cc/private/compile:compile_build_variables.bzl", "get_copts")
load("//cc/private/compile:cc_compilation_helper.bzl", "dotd_files_enabled")

Proof = provider(fields = ["toolchain", "snapshot"])

def _impl(ctx):
    tc = ctx.attr.toolchain[CcToolchainInfo]
    cpp = tc._cpp_configuration
    rows = []
    for source in ctx.attr.sources:
        file = source[DefaultInfo].files.to_list()[0]
        rows.append((file.extension, get_copts(None, cpp, file,
            ["rule-c"], ["rule-common"], ["rule-cxx"], ctx.label)))
    file = ctx.attr.sources[0][DefaultInfo].files.to_list()[0]
    override = get_copts(None, cpp, file, ["rule-c"], ["rule-common"],
        ["rule-cxx"], ctx.label, override_extension = ".cc")
    enabled = tc._toolchain_features.configure_features([])
    showincludes = tc._toolchain_features.configure_features(["parse_showincludes"])
    no_dotd = tc._toolchain_features.configure_features(["no_dotd_file"])
    snapshot = struct(rows = rows, override = override,
        dotd = dotd_files_enabled(None, cpp, enabled),
        cpp_dotd = dotd_files_enabled("cpp", cpp, enabled),
        showincludes_dotd = dotd_files_enabled(None, cpp, showincludes),
        disabled_dotd = dotd_files_enabled(None, cpp, no_dotd),
        save_temps = cpp.save_temps(),
        process_headers = cpp.process_headers_in_dependencies())
    return [Proof(toolchain = tc, snapshot = snapshot)]

subject = rule(implementation = _impl, attrs = {
    "toolchain": attr.label(mandatory = True),
    "sources": attr.label_list(allow_files = True),
})
