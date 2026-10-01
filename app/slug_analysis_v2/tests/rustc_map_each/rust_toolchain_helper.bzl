# Authored inputs only; the allocator helper and CC builders are unchanged.
load('@rules_cc//cc/common:cc_common.bzl', 'cc_common')
load('@rules_cc//cc/common:cc_info.bzl', 'CcInfo')
load('@rules_cc//cc/private:toolchain_proof.bzl', 'make_compile_toolchain')
load('@rules_cc//cc/private/toolchain_config:configure_features.bzl', 'configure_features')
load('//rust/private:common.bzl', 'rust_common')
load('//rust/private:providers.bzl', 'AllocatorLibrariesImplInfo')
load('//rust/private:rust_allocator_libraries.bzl', 'make_libstd_and_allocator_ccinfo')

Proof = provider(fields = ['rows', 'marker', 'empty'])

def _stdlib(ctx):
    files = {f.basename: f for f in ctx.files.files}
    def layer(name):
        return [] if ctx.attr.empty else [files['lib' + name + '.a']]
    info = rust_common.stdlib_info(
        alloc_files = layer('alloc'), between_alloc_and_core_files = layer('builtins'),
        core_files = layer('core'), between_core_and_std_files = layer('panic_abort') + layer('panic_unwind') + layer('middle'),
        memchr_files = layer('memchr'), panic_files = layer('panic_unwind') + layer('panic_abort'),
        std_files = layer('std'), test_files = layer('test'), std_rlibs = layer('std'),
        std_dylib = files['libstd.so'], self_contained_files = [files['self.o']] if ctx.attr.self_contained else [],
        dot_a_files = depset(), srcs = [], has_profiler_builtins = False)
    return [DefaultInfo(files = depset(ctx.files.files)), info]
stdlib = rule(implementation = _stdlib, attrs = {
    'files': attr.label_list(allow_files = True), 'empty': attr.bool(), 'self_contained': attr.bool(),
})

def _impl(ctx):
    tc = make_compile_toolchain(ctx)
    configuration = configure_features(ctx = ctx, cc_toolchain = tc,
        requested_features = ctx.features, unsupported_features = ctx.disabled_features)
    archive = ctx.file.allocator
    if ctx.attr.allocator_kind == 'cc':
        library = cc_common.create_library_to_link(actions = ctx.actions, cc_toolchain = tc,
            feature_configuration = configuration, static_library = archive, pic_static_library = archive)
        allocator = struct(cc_info = CcInfo(linking_context = cc_common.create_linking_context(
            linker_inputs = depset([cc_common.create_linker_input(owner = ctx.label, libraries = depset([library]))]))))
    elif ctx.attr.allocator_kind == 'impl':
        allocator = struct(allocator_libraries_impl_info = AllocatorLibrariesImplInfo(static_archive = archive))
    elif ctx.attr.allocator_kind == 'bad_type':
        allocator = False
    elif ctx.attr.allocator_kind == 'bad_fields':
        allocator = struct()
    else:
        allocator = struct(allocator_libraries_impl_info = None)
    result = make_libstd_and_allocator_ccinfo(cc_toolchain = tc, feature_configuration = configuration,
        label = ctx.label, actions = ctx.actions, experimental_link_std_dylib = ctx.attr.dynamic,
        rust_std = ctx.attr.stdlib, allocator_library = allocator, std = ctx.attr.mode)
    rows = []
    if result != None:
        for linker in result.linking_context.linker_inputs.to_list():
            libraries = []
            for library in linker.libraries:
                file = library.dynamic_library if library.dynamic_library else library.static_library
                libraries.append(struct(name = file.basename, path = file.path,
                    resolved = library.resolved_symlink_dynamic_library.path if library.resolved_symlink_dynamic_library else None,
                    pic = library.pic_static_library.path if library.pic_static_library else None))
            rows.append(struct(owner = str(linker.owner), libraries = libraries))
    return [Proof(rows = rows, empty = result == None, marker = 'MARKER:' + ctx.fragments.cpp.compilation_mode())]
subject = rule(implementation = _impl, fragments = ['cpp'], exec_groups = {'cpp_link': exec_group()}, attrs = {
    'stdlib': attr.label(allow_files = True), 'mode': attr.string(default = 'std'), 'dynamic': attr.bool(),
    'allocator_kind': attr.string(default = 'impl'),
    'allocator': attr.label(default = '@@//:liballocator.a', allow_single_file = True),
    'compiler': attr.string(default = 'gcc'), 'runtime': attr.string(default = 'empty'), 'unused': attr.string(),
    'object_prefix': attr.string(), 'object_extension': attr.string(default = '.o'),
    'header': attr.label(default = '@@//:header.h', allow_single_file = True),
    'static_file': attr.label(default = '@@//:static.a', allow_single_file = True),
    'dynamic_file': attr.label(default = '@@//:dynamic.so', allow_single_file = True),
})
