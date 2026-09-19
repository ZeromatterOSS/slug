load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust/private:providers.bzl", "CrateInfo")
load("@rules_rust//rust/private:rustc.bzl", "AliasableDepInfo", "construct_arguments")

_NESTED_ROOT_LABEL = Label("@@//nested:unused")
_NESTED_EXTERNAL_LABEL = Label("//nested:unused")

def _no_coverage():
    return False

def _dependency_crates(ctx):
    files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.dependency_files]
    if not files:
        return depset(), depset()
    a = CrateInfo(name = "a", output = files[0], metadata = files[1], metadata_supports_pipelining = True)
    b = CrateInfo(name = "b", output = files[2], metadata = files[3], metadata_supports_pipelining = False)
    # No pipelining field is read when metadata is absent.
    c = CrateInfo(name = "c", output = files[4], metadata = None)
    direct = depset([AliasableDepInfo(name = ctx.attr.dependency_alias, dep = a), b])
    transitive = depset([c], transitive = [depset([a, b])])
    return direct, transitive

def _native_library(static = None, pic = None, dynamic = None, interface = None, alwayslink = False):
    return struct(static_library = static, pic_static_library = pic, dynamic_library = dynamic, interface_library = interface, alwayslink = alwayslink)

def _cc_native_inputs(ctx):
    files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.native_files]
    static = cc_common.create_library_to_link(actions = ctx.actions, static_library = files[0], pic_static_library = files[1])
    always = cc_common.create_library_to_link(actions = ctx.actions, static_library = files[2], alwayslink = True)
    first = cc_common.create_linker_input(
        owner = ctx.label,
        libraries = depset([static, always]),
        user_link_flags = [[ctx.attr.native_user_flag], "-pthread"],
        additional_inputs = depset([ctx.attr.native_unused_input[DefaultInfo].files.to_list()[0]] if ctx.attr.native_unused_input else []),
    )
    second = cc_common.create_linker_input(owner = ctx.label, libraries = depset([static]))
    linking = cc_common.create_linking_context(linker_inputs = depset([second], transitive = [depset([first])]))
    cc = CcInfo(linking_context = linking)
    return cc.linking_context.linker_inputs, {files[1].short_path: files[4]}, cc

def _native_inputs(ctx):
    if ctx.attr.real_cc:
        return _cc_native_inputs(ctx)
    files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.native_files]
    if not files:
        return depset(), {}, None
    static = _native_library(static = files[0], pic = files[1])
    libraries = (
        static,
        _native_library(static = files[2], alwayslink = True),
        _native_library(dynamic = files[3]),
        _native_library(interface = files[6], dynamic = files[3]),
        _native_library(static = files[5]),
        static,
    )
    first = struct(libraries = libraries, user_link_flags = (ctx.attr.native_user_flag, "-pthread"))
    second = struct(libraries = (static,), user_link_flags = ())
    return depset([second], transitive = [depset([first])]), {files[1].short_path: files[4]}, None

def _impl(ctx):
    output = ctx.actions.declare_file("out/probe.rlib")
    source = ctx.attr.src[DefaultInfo].files.to_list()[0]
    if not source.is_source or output.is_source:
        fail("File.is_source must distinguish source and generated artifacts")
    if "is_source" not in dir(source) or "is_source" not in dir(output):
        fail("File.is_source must be discoverable")
    source_paths = [source.path, source.short_path, source.dirname, source.label.workspace_root]
    if source_paths != ctx.attr.expected_source_paths:
        fail("source File paths: expected {}, got {}".format(ctx.attr.expected_source_paths, source_paths))
    if ctx.label.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must use the canonical repository, without the package")
    if "workspace_root" not in dir(ctx.label):
        fail("Label.workspace_root must be discoverable")
    if _NESTED_ROOT_LABEL.workspace_root != "" or _NESTED_EXTERNAL_LABEL.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must ignore package and target components")
    if output.dirname != "out":
        fail("File.dirname must distinguish the execution root and nested directories")
    root = source
    if ctx.attr.generated:
        root = ctx.actions.declare_file("generated/input.rs")
        ctx.actions.write(root, "pub fn generated() {}\n")
    stdlib_files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.stdlib]
    stdlib = depset(stdlib_files[1:], transitive = [depset(stdlib_files[:1])])
    dependency_files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.dependency_files]
    native_files = [target[DefaultInfo].files.to_list()[0] for target in ctx.attr.native_files]
    sysroot = ctx.attr.sysroot[DefaultInfo].files.to_list()[0] if ctx.attr.sysroot else None
    direct_crates, transitive_crates = _dependency_crates(ctx)
    native_inputs, ambiguous_libs, cc = _native_inputs(ctx)
    # Exercise the argument-builder API with a real source File and native
    # actions. These explicit inputs select only the admitted callback slice.
    argument_ctx = struct(
        actions = ctx.actions,
        attr = struct(),
        label = ctx.label,
        executable = struct(_process_wrapper = source),
        var = {"COMPILATION_MODE": "dbg"},
        genfiles_dir = struct(path = "genfiles"),
        configuration = struct(coverage_enabled = False),
        coverage_instrumented = _no_coverage,
        fragments = struct(cpp = struct(linkopts = [])),
    )
    toolchain = struct(
        target_arch = "x86_64",
        target_abi = "gnu",
        linker = native_files[7] if native_files else None,
        linker_type = "direct" if ctx.attr.direct_linker else "indirect",
        linker_preference = "rust",
        target_os = "linux",
        target_flag_value = "x86_64-unknown-linux-gnu",
        compilation_mode_opts = {"dbg": struct(opt_level = "0", debug_info = "2", strip_level = "none")},
        rust_std = stdlib,
        lto = struct(mode = "manual"),
        _codegen_units = 0,
        coverage_supported = False,
        _experimental_link_std_dylib = False,
        _toolchain_generated_sysroot = sysroot != None,
        sysroot_anchor = sysroot,
        _rename_first_party_crates = False,
        extra_rustc_flags_for_crate_types = {},
        extra_exec_rustc_flags = [],
        extra_rustc_flags = [],
        _no_std = "off",
        env = {},
    )
    crate = struct(
        root = root,
        root_path = "unused-for-regular-file.rs",
        name = "probe",
        type = "bin" if ctx.attr.native_files else "rlib",
        output = output,
        rustc_output = None,
        rustc_env = {},
        compile_data_targets = depset(),
        edition = "2021",
        wrapped_crate_type = None,
        is_test = False,
    )
    args, env = construct_arguments(
        ctx = argument_ctx,
        attr = struct(name = "probe"),
        file = struct(),
        toolchain = toolchain,
        cc_toolchain = None,
        feature_configuration = None,
        crate_info = crate,
        dep_info = struct(transitive_build_infos = depset(), direct_crates = direct_crates, transitive_crates = transitive_crates, transitive_noncrates = native_inputs),
        linkstamp_outs = [],
        ambiguous_libs = ambiguous_libs,
        output_hash = None,
        rust_flags = [],
        out_dir = None,
        build_env_files = [],
        build_flags_files = depset(),
        tool_path = "rustc",
        emit = ["link"] if ctx.attr.native_files else [],
        include_link_flags = ctx.attr.include_native_flags,
        remap_path_prefix = None,
        skip_expanding_rustc_env = True,
        force_depend_on_objects = ctx.attr.force_objects,
        force_all_deps_direct = ctx.attr.force_direct,
    )
    inputs = depset([root] + dependency_files + native_files + ([] if sysroot == None else [sysroot]), transitive = [stdlib])
    ctx.actions.run(outputs = [output], executable = "process_wrapper", arguments = args.all, inputs = inputs, env = env)
    return [DefaultInfo(files = depset([output]))] + ([cc] if cc else [])

subject = rule(implementation = _impl, attrs = {
    "src": attr.label(allow_single_file = True),
    "expected_source_paths": attr.string_list(default = ["lib.rs", "lib.rs", ".", ""]),
    "stdlib": attr.label_list(allow_files = True),
    "sysroot": attr.label(allow_single_file = True),
    "generated": attr.bool(default = False),
    "dependency_files": attr.label_list(allow_files = True),
    "dependency_alias": attr.string(default = "renamed"),
    "native_files": attr.label_list(allow_files = True),
    "real_cc": attr.bool(default = False),
    "native_unused_input": attr.label(allow_single_file = True),
    "direct_linker": attr.bool(default = False),
    "include_native_flags": attr.bool(default = True),
    "native_user_flag": attr.string(default = "-z,now"),
    "force_objects": attr.bool(default = False),
    "force_direct": attr.bool(default = False),
})
