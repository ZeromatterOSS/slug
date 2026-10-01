# Authored producer inputs and consumers. Toolchain/helper sources stay unchanged.
load('@rules_cc//cc/common:cc_common.bzl', 'cc_common')
load('@rules_cc//cc/common:cc_info.bzl', 'CcInfo')

Proof = provider(fields = ['callback', 'aliases', 'libraries', 'marker'])

def _allocator(ctx):
    return [CcInfo(compilation_context = cc_common.create_compilation_context(),
        linking_context = cc_common.create_linking_context())]
allocator = rule(implementation = _allocator)

def _relay(ctx):
    tc = ctx.attr.dep[platform_common.ToolchainInfo]
    callback = tc.make_libstd_and_allocator_ccinfo
    aliases = (callback, {'callback': callback}, [callback, callback])
    if ctx.attr.nested:
        previous = ctx.attr.dep[Proof]
        if callback != previous.callback or callback != previous.aliases[1]['callback']:
            fail('callback alias changed during relay publication')
    return [tc, Proof(callback = callback, aliases = aliases, libraries = [], marker = 'relay')]
relay = rule(implementation = _relay, attrs = {'dep': attr.label(), 'nested': attr.bool()})

def _consumer(ctx):
    tc = ctx.attr.dep[platform_common.ToolchainInfo]
    proof = ctx.attr.dep[Proof]
    callbacks = [tc.make_libstd_and_allocator_ccinfo, proof.callback,
        proof.aliases[0], proof.aliases[1]['callback']] + proof.aliases[2]
    first = callbacks[0]
    keys = {first: 'old'}
    rows = None
    for callback in callbacks:
        if callback != first or first != callback:
            fail('original/proxy callback equality lost an alias')
        keys[callback] = 'new'
        if len(keys) != 1 or keys[first] != 'new':
            fail('original/proxy callback hash lost an alias')
        result = callback(ctx.label, ctx.actions, struct(allocator_libraries_impl_info = None), 'std')
        if result == None:
            fail('nonempty genuine stdlib unexpectedly returned None')
        current = [library.dynamic_library.basename if library.dynamic_library else library.static_library.basename
            for linker in result.linking_context.linker_inputs.to_list() for library in linker.libraries]
        if rows != None and rows != current:
            fail('aliased callback invocation changed the library graph')
        rows = current
    output = ctx.actions.declare_file(ctx.label.name + '.txt')
    ctx.actions.write(output, 'MARKER:' + ','.join(rows))
    return [DefaultInfo(files = depset([output])), Proof(callback = first,
        aliases = proof.aliases, libraries = rows, marker = 'MARKER')]
consumer = rule(implementation = _consumer, attrs = {'dep': attr.label()},
    exec_groups = {'cpp_link': exec_group()})
