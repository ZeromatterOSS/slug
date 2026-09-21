# Authored constructor inputs only. All51 arguments reach the unchanged complete
# CcToolchainInfo initializer. Inert tool/FDO inputs do not claim full toolchain
# rule, FDO production, real tool execution, or compile/link action support.
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "artifact_name_pattern", "env_entry", "env_set", "feature", "tool_path")
load("@rules_cc//cc/private:cc_internal.bzl", "cc_internal")
load("@rules_cc//cc/private/rules_impl:cc_toolchain_info.bzl", "CcToolchainInfo")
load("@rules_cc//cc/private/toolchain_config:cc_toolchain_config_info.bzl", "create_cc_toolchain_config_info")
load("@rules_cc//cc/private/toolchain_config:configure_features.bzl", "configure_features")

Proof = provider(fields = ["toolchain", "pic_alias", "static_alias", "dynamic_alias", "cpp_alias", "snapshot"])

def _file(target):
    return target[DefaultInfo].files.to_list()[0]

def _make(ctx):
    header = _file(ctx.attr.header)
    static_file = _file(ctx.attr.static_file)
    dynamic_file = _file(ctx.attr.dynamic_file)
    all_files = depset([header, static_file, dynamic_file])
    static = None if ctx.attr.runtime == "none" else depset([] if ctx.attr.runtime == "empty" else [static_file])
    dynamic = None if ctx.attr.runtime == "none" else depset([] if ctx.attr.runtime == "empty" else [dynamic_file])
    info = create_cc_toolchain_config_info(ctx = ctx, toolchain_identifier = "complete-proof",
        compiler = "gcc", target_cpu = "k8", target_libc = "local", target_system_name = "proof-system",
        abi_version = "proof-abi", abi_libc_version = "proof-libc",
        artifact_name_patterns = [artifact_name_pattern(category_name = "object_file", prefix = ctx.attr.object_prefix, extension = ctx.attr.object_extension)],
        features = [feature(name = "supports_pic"), feature(name = "static_link_cpp_runtimes"),
            feature(name = "parse_showincludes"), feature(name = "no_dotd_file"),
            feature(name = "probe_env", enabled = True, env_sets = [env_set(actions = ["c++-link-executable"],
                env_entries = [env_entry(key = "SYSROOT", value = "%{sysroot}")])])],
        tool_paths = [tool_path(name = n, path = "bin/" + n) for n in ["gcc", "ar", "strip"]])
    model = cc_internal.cc_toolchain_features(toolchain_config_info = info, tools_directory = "toolchain")
    bindings = {"legacy_link_flags": ["--legacy"], "sysroot": "/sysroot"}
    variables = cc_internal.cc_toolchain_variables(vars = bindings)
    compilation = cc_common.create_compilation_context(headers = depset([header]))
    return CcToolchainInfo(
        built_in_include_directories = ["toolchain/include"],
        static_runtime_lib_depset = static, dynamic_runtime_lib_depset = dynamic,
        sysroot = "/sysroot", toolchain_config_info = info,
        dynamic_runtime_solib_dir = "out/runtime", objcopy_executable = "bin/objcopy",
        compiler_executable = "bin/gcc", preprocessor_executable = "bin/cpp",
        nm_executable = "bin/nm", objdump_executable = "bin/objdump", ar_executable = "bin/ar",
        strip_executable = "bin/strip", ld_executable = "bin/ld", gcov_executable = "bin/gcov",
        tool_paths = {"gcc": "bin/gcc", "ar": "bin/ar"}, solib_dir = "_solib_k8",
        fdo_context = struct(branch_fdo_profile = None, propeller_optimize_info = None),
        legacy_cc_flags_make_variable = "--legacy", additional_make_variables = {"UNUSED": ctx.attr.unused},
        crosstool_top_path = "toolchain", toolchain_features = model, toolchain_label = ctx.label,
        cpp_configuration = ctx.fragments.cpp, is_tool_configuration = False,
        is_sibling_repository_layout = False, stamp_binaries = False,
        builtin_include_files = [header], build_variables_dict = bindings, build_variables = variables,
        cc_info = CcInfo(compilation_context = compilation),
        all_files = all_files, all_files_including_libc = all_files,
        compiler_files = all_files, compiler_files_without_includes = all_files,
        strip_files = all_files, as_files = all_files, ar_files = all_files,
        linker_files = all_files, if_so_builder = header, dwp_files = all_files,
        coverage_files = all_files, supports_param_files = True, supports_header_parsing = False,
        link_dynamic_library_tool = header, grep_includes = None, allowlist_for_layering_check = None,
        build_info_files = OutputGroupInfo(), objcopy_files = all_files,
        aggregate_ddi = None, generate_modmap = None,
    )

def _check_mixed(received, fresh):
    left = received.needs_pic_for_dynamic_libraries
    right = fresh.needs_pic_for_dynamic_libraries
    force_pic = fresh._cpp_configuration.force_pic()
    if (left == right) != (not force_pic) or (right == left) != (not force_pic):
        fail("restored/global versus fresh PIC function equality is not symmetric")
    if not force_pic:
        if {left: "left"}[right] != "left" or {right: "right"}[left] != "right":
            fail("original/restored global function dictionary hashes disagree")
        values = {left: "old"}
        values[right] = "new"
        if len(values) != 1 or values[left] != "new":
            fail("equal global function failed dictionary key replacement")
    elif len({left: 1, right: 2}) != 2:
        fail("different force-PIC constructor lambdas collapsed")
    for a, b in [(received.static_runtime_lib, fresh.static_runtime_lib), (received.dynamic_runtime_lib, fresh.dynamic_runtime_lib)]:
        if a == b or b == a or len({a: 1, b: 2}) != 2:
            fail("fresh runtime closures collapsed despite separate constructors")

def _snapshot(ctx, tc, get_linker_and_args):
    empty = tc._toolchain_features.configure_features([])
    enabled = tc._toolchain_features.configure_features(["supports_pic", "static_link_cpp_runtimes"])
    static = None
    dynamic = None
    if tc._static_runtime_lib_depset != None:
        static = tc.static_runtime_lib(feature_configuration = enabled)
        if static != tc._static_runtime_lib_depset:
            fail("static runtime callback replaced its captured depset")
    if tc._dynamic_runtime_lib_depset != None:
        dynamic = tc.dynamic_runtime_lib(feature_configuration = enabled)
        if dynamic != tc._dynamic_runtime_lib_depset:
            fail("dynamic runtime callback replaced its captured depset")
    configured = configure_features(ctx = ctx, cc_toolchain = tc,
        requested_features = ctx.features, unsupported_features = ctx.disabled_features)
    tool, direct, argv, env = get_linker_and_args(ctx, "bin",
        struct(linker_preference = "cc"), tc, configured, depset())
    return struct(tool = tool, direct = direct, argv = argv, env = env,
        pic_without_feature = tc.needs_pic_for_dynamic_libraries(feature_configuration = empty),
        pic_with_feature = tc.needs_pic_for_dynamic_libraries(feature_configuration = enabled),
        static = static, dynamic = dynamic,
        disabled_static = tc.static_runtime_lib(feature_configuration = empty),
        disabled_dynamic = tc.dynamic_runtime_lib(feature_configuration = empty))

def _impl(ctx, get_linker_and_args):
    if ctx.attr.dep:
        previous = ctx.attr.dep[Proof]
        tc = ctx.attr.dep[CcToolchainInfo]
        if ctx.attr.dep[platform_common.ToolchainInfo].cc != tc:
            fail("ToolchainInfo did not retain the complete CcToolchainInfo")
        for a, b in [(previous.pic_alias, tc.needs_pic_for_dynamic_libraries),
                     (previous.static_alias, tc.static_runtime_lib),
                     (previous.dynamic_alias, tc.dynamic_runtime_lib),
                     (previous.cpp_alias, tc._cpp_configuration)]:
            if a != b or b != a or {a: True}[b] != True:
                fail("extracted provider field alias did not survive publication")
        fresh = _make(ctx)
        _check_mixed(tc, fresh)
        if ctx.attr.failure:
            enabled = tc._toolchain_features.configure_features(["static_link_cpp_runtimes"])
            callback = tc.static_runtime_lib if ctx.attr.failure == "static" else tc.dynamic_runtime_lib
            callback(feature_configuration = enabled)
            fail("missing runtime capture unexpectedly succeeded")
    else:
        tc = _make(ctx)
        _check_mixed(tc, _make(ctx))
    return [tc, platform_common.ToolchainInfo(cc = tc, cc_provider_in_toolchain = True),
        Proof(toolchain = tc, pic_alias = tc.needs_pic_for_dynamic_libraries,
            static_alias = tc.static_runtime_lib, dynamic_alias = tc.dynamic_runtime_lib,
            cpp_alias = tc._cpp_configuration, snapshot = _snapshot(ctx, tc, get_linker_and_args))]

def make_subject(get_linker_and_args):
    # The rules_rust caller supplies its unchanged helper through its existing
    # repository route. Constructor/private API calls remain defined here.
    # The callback is implementation state, never a published provider field.
    def implementation(ctx):
        return _impl(ctx, get_linker_and_args)

    return rule(implementation = implementation, fragments = ["cpp"], attrs = {
        "compiler": attr.string(default = "gcc"), "dep": attr.label(), "failure": attr.string(),
        "runtime": attr.string(default = "files"), "unused": attr.string(default = "A"),
        "object_prefix": attr.string(), "object_extension": attr.string(default = ".o"),
        "header": attr.label(default = "@@//:header.h", allow_single_file = True),
        "static_file": attr.label(default = "@@//:static.a", allow_single_file = True),
        "dynamic_file": attr.label(default = "@@//:dynamic.so", allow_single_file = True),
    })
