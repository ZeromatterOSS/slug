load("@rules_rust//rust/private:providers.bzl", "CrateInfo")
load("@rules_rust//rust/private:rustc.bzl", "AliasableDepInfo", "construct_arguments")

_NESTED_ROOT_LABEL = Label("@@//nested:unused")
_NESTED_EXTERNAL_LABEL = Label("//nested:unused")

def _no_coverage():
    return False

def _dependency_crates(ctx):
    files = ctx.attr.dependency_files
    if not files:
        return depset(), depset()
    a = CrateInfo(name = "a", output = files[0], metadata = files[1], metadata_supports_pipelining = True)
    b = CrateInfo(name = "b", output = files[2], metadata = files[3], metadata_supports_pipelining = False)
    # No pipelining field is read when metadata is absent.
    c = CrateInfo(name = "c", output = files[4], metadata = None)
    direct = depset([AliasableDepInfo(name = ctx.attr.dependency_alias, dep = a), b])
    transitive = depset([c], transitive = [depset([a, b])])
    return direct, transitive

def _impl(ctx):
    output = ctx.actions.declare_file("out/probe.rlib")
    if not ctx.attr.src.is_source or output.is_source:
        fail("File.is_source must distinguish source and generated artifacts")
    if "is_source" not in dir(ctx.attr.src) or "is_source" not in dir(output):
        fail("File.is_source must be discoverable")
    if ctx.attr.src.label.workspace_root != "" or ctx.label.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must use the canonical repository, without the package")
    if "workspace_root" not in dir(ctx.label):
        fail("Label.workspace_root must be discoverable")
    if _NESTED_ROOT_LABEL.workspace_root != "" or _NESTED_EXTERNAL_LABEL.workspace_root != "external/rules_rust+":
        fail("Label.workspace_root must ignore package and target components")
    if ctx.attr.src.dirname != "." or output.dirname != "out":
        fail("File.dirname must distinguish the execution root and nested directories")
    root = ctx.attr.src
    if ctx.attr.generated:
        root = ctx.actions.declare_file("generated/input.rs")
        ctx.actions.write(root, "pub fn generated() {}\n")
    stdlib = depset(ctx.attr.stdlib[1:], transitive = [depset(ctx.attr.stdlib[:1])])
    direct_crates, transitive_crates = _dependency_crates(ctx)
    # Exercise the argument-builder API with a real source File and native
    # actions. These explicit inputs select only the admitted callback slice.
    argument_ctx = struct(
        actions = ctx.actions,
        attr = struct(),
        label = ctx.label,
        executable = struct(_process_wrapper = ctx.attr.src),
        var = {"COMPILATION_MODE": "dbg"},
        genfiles_dir = struct(path = "genfiles"),
        configuration = struct(coverage_enabled = False),
        coverage_instrumented = _no_coverage,
    )
    toolchain = struct(
        target_arch = "x86_64",
        target_os = "linux",
        target_flag_value = "x86_64-unknown-linux-gnu",
        compilation_mode_opts = {"dbg": struct(opt_level = "0", debug_info = "2", strip_level = "none")},
        rust_std = stdlib,
        lto = struct(mode = "manual"),
        _codegen_units = 0,
        coverage_supported = False,
        _experimental_link_std_dylib = False,
        _toolchain_generated_sysroot = ctx.attr.sysroot != None,
        sysroot_anchor = ctx.attr.sysroot,
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
        type = "rlib",
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
        dep_info = struct(transitive_build_infos = depset(), direct_crates = direct_crates, transitive_crates = transitive_crates),
        linkstamp_outs = [],
        ambiguous_libs = {},
        output_hash = None,
        rust_flags = [],
        out_dir = None,
        build_env_files = [],
        build_flags_files = depset(),
        tool_path = "rustc",
        emit = [],
        remap_path_prefix = None,
        skip_expanding_rustc_env = True,
        force_depend_on_objects = ctx.attr.force_objects,
        force_all_deps_direct = ctx.attr.force_direct,
    )
    inputs = depset([root] + ctx.attr.dependency_files + ([] if ctx.attr.sysroot == None else [ctx.attr.sysroot]), transitive = [stdlib])
    ctx.actions.run(outputs = [output], executable = "process_wrapper", arguments = args.all, inputs = inputs, env = env)
    return [DefaultInfo(files = depset([output]))]

subject = rule(implementation = _impl, attrs = {
    "src": attr.label(allow_single_file = True),
    "stdlib": attr.label_list(allow_files = True),
    "sysroot": attr.label(allow_single_file = True),
    "generated": attr.bool(default = False),
    "dependency_files": attr.label_list(allow_files = True),
    "dependency_alias": attr.string(default = "renamed"),
    "force_objects": attr.bool(default = False),
    "force_direct": attr.bool(default = False),
})
