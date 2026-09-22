# Authored boundary inputs, not the full cc_toolchain rule/attribute envelope.
# The complete release helper and its real FDO subrule execute unchanged.
load(":cc_toolchain_provider_helper.bzl", "get_cc_toolchain_provider")
load("//cc/private/rules_impl/fdo:fdo_context.bzl", "create_fdo_context")
load("//cc/private/toolchain_config:cc_toolchain_config_info.bzl", "create_cc_toolchain_config_info")
load("//cc:cc_toolchain_config_lib.bzl", "feature", "make_variable", "tool_path")

ProviderProof = provider(fields = ["toolchain", "variables", "fdo", "files", "cpp", "snapshot"])

def _impl(ctx):
    tools = ["ar", "cpp", "gcc", "ld", "nm", "objdump", "strip"]
    if ctx.attr.failure == "missing_tool":
        tools = tools[:-1]
    info = create_cc_toolchain_config_info(
        ctx = ctx, toolchain_identifier = "helper-proof", compiler = "gcc",
        target_cpu = "k8", target_libc = "local", target_system_name = "proof",
        abi_version = "proof", abi_libc_version = "proof",
        builtin_sysroot = "/proof-sysroot",
        cxx_builtin_include_directories = ["%sysroot%/include", "%workspace%/toolchain/include", "%crosstool_top%/local/include"],
        features = [feature(name = "supports_pic")],
        tool_paths = [tool_path(name = n, path = "bin/../bad" if ctx.attr.failure == "unnormalized" else "bin/" + n) for n in tools],
        make_variables = [make_variable(name = "CC_FLAGS", value = "--helper-legacy"), make_variable(name = "PRODUCER", value = ctx.attr.metadata)],
    )
    files = depset(ctx.files.inputs)
    libc = depset([ctx.file.libc])
    attributes = struct(
        cc_toolchain_config_info = info, cc_toolchain_label = ctx.label,
        runtime_solib_dir_base = "_solib_helper", libc_top_label = None,
        static_runtime_lib = None, dynamic_runtime_lib = None,
        module_map = None, module_map_artifact = None, zipper = None,
        all_files = files, all_files_including_libc = depset(transitive = [files, libc]),
        compiler_files = files, compiler_files_without_includes = files,
        strip_files = files, objcopy_files = files, as_files = files, ar_files = files,
        linker_files = files, dwp_files = depset(), coverage_files = files, libc = libc,
        if_so_builder = None, link_dynamic_library_tool = None, grep_includes = None,
        aggregate_ddi = None, generate_modmap = None,
        supports_param_files = True, supports_header_parsing = False,
        allowlist_for_layering_check = None, build_info_files = OutputGroupInfo(),
    )
    tc = get_cc_toolchain_provider(ctx, attributes)
    return [tc, platform_common.ToolchainInfo(cc = tc, cc_provider_in_toolchain = True),
        ProviderProof(toolchain = tc, variables = tc._build_variables,
            fdo = tc._fdo_context, files = files, cpp = ctx.fragments.cpp,
            snapshot = struct(mode = ctx.fragments.cpp.compilation_mode(), minimum_os = tc._build_variables_dict.get("minimum_os_version"),
                metadata = tc._additional_make_variables["PRODUCER"],
                stack = tc._additional_make_variables["STACK_FRAME_UNLIMITED"],
                legacy = tc._legacy_cc_flags_make_variable,
                variable_sysroot = tc._build_variables_dict["sysroot"]))]

helper_subject = rule(implementation = _impl, fragments = ["cpp"], subrules = [create_fdo_context], attrs = {
    "inputs": attr.label_list(allow_files = True, default = ["@@//:header.h", "@@//:static.a"]),
    "libc": attr.label(allow_single_file = True, default = "@@//:include/stdc-predef.h"),
    "metadata": attr.string(default = "A"), "failure": attr.string(),
})
