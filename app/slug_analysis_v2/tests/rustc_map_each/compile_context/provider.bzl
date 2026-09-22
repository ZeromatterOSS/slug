# Authored boundary inputs, not the full cc_toolchain rule/attribute envelope.
# The complete release helper and its real FDO subrule execute unchanged.
load(":cc_toolchain_provider_helper.bzl", "get_cc_toolchain_provider")
load("//cc/private/rules_impl/fdo:fdo_context.bzl", "create_fdo_context")
load("//cc/private/toolchain_config:cc_toolchain_config_info.bzl", "create_cc_toolchain_config_info")
load("//cc:cc_toolchain_config_lib.bzl", "feature", "make_variable", "tool_path")
load("//cc/common:cc_common.bzl", "cc_common")
load("//cc/common:cc_helper.bzl", "cc_helper")

ProviderProof = provider(fields = ["toolchain", "variables", "fdo", "files", "cpp", "snapshot", "template", "make_variables"])

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
        make_variables = [make_variable(name = "CC_FLAGS", value = "--helper-legacy"), make_variable(name = "PRODUCER", value = ctx.attr.metadata), make_variable(name = "CC", value = "shadowed-by-global-helper")],
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
        allowlist_for_layering_check = ctx.attr.allowlist[PackageSpecificationInfo], build_info_files = OutputGroupInfo(),
    )
    tc = get_cc_toolchain_provider(ctx, attributes)
    # Authored composition of unchanged public helpers, matching cc_toolchain.bzl
    # 129-136. This is not a claim to execute that rule's attribute envelope.
    feature_configuration = cc_common.configure_features(ctx = ctx, cc_toolchain = tc,
        requested_features = ctx.features, unsupported_features = ctx.disabled_features)
    make_variables = tc._additional_make_variables | cc_helper.get_toolchain_global_make_variables(tc) | cc_helper.get_cc_flags_make_variable(ctx, feature_configuration, tc)
    template = platform_common.TemplateVariableInfo(make_variables)
    return [tc, template, platform_common.ToolchainInfo(cc = tc, cc_provider_in_toolchain = True),
        ProviderProof(toolchain = tc, variables = tc._build_variables,
            template = template, make_variables = make_variables,
            fdo = tc._fdo_context, files = files, cpp = ctx.fragments.cpp,
            snapshot = struct(mode = ctx.fragments.cpp.compilation_mode(), minimum_os = tc._build_variables_dict.get("minimum_os_version"),
                metadata = tc._additional_make_variables["PRODUCER"],
                stack = tc._additional_make_variables["STACK_FRAME_UNLIMITED"],
                legacy = tc._legacy_cc_flags_make_variable,
                variable_sysroot = tc._build_variables_dict["sysroot"]))]

helper_subject = rule(implementation = _impl, fragments = ["cpp"], subrules = [create_fdo_context], attrs = {
    "inputs": attr.label_list(allow_files = True, default = ["@@//:header.h", "@@//:static.a"]),
    "libc": attr.label(allow_single_file = True, default = "@@//:include/stdc-predef.h"),
    "allowlist": attr.label(mandatory = True),
    "metadata": attr.string(default = "A"), "failure": attr.string(),
})

def _consume_template(ctx):
    template = ctx.attr.dep[platform_common.TemplateVariableInfo]
    proof = ctx.attr.dep[ProviderProof]
    if template != proof.template:
        fail("direct and nested template provider aliases split")
    first = template.variables
    first["CC"] = "local mutation"
    if template.variables["CC"] == "local mutation":
        fail("template getter did not allocate a fresh dict")
    return [template, ProviderProof(template = template, make_variables = template.variables)]

template_consumer = rule(implementation = _consume_template, attrs = {"dep": attr.label()})
