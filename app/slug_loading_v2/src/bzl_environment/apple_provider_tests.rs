//! Actual bundled exports and initialized providers through the semantic owner.
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::ValueLike;

use super::*;
use crate::bzl_module::BzlModuleContext;
use crate::bzl_module::FrozenBzlModule;
use crate::bzl_module::semantic_evaluation;

const CONFIG: &str = r#"
def config(availability = 'LOCAL', version = '15.4.0', flag = '15.4-vendor-foo', include = True, ios = '17.2'):
    return apple_common.XcodeVersionConfig(
        ios_sdk_version = ios, ios_minimum_os_version = '15.1',
        visionos_sdk_version = '1.3', visionos_minimum_os_version = '1.1',
        watchos_sdk_version = '10.2', watchos_minimum_os_version = '8.1',
        tvos_sdk_version = '17.4', tvos_minimum_os_version = '14.2',
        macos_sdk_version = '14', macos_minimum_os_version = '12.3',
        xcode_version = version, availability = availability,
        xcode_version_flag = flag, include_xcode_execution_info = include)
"#;

async fn consumer(
    tx: &mut DiceComputations<'_>,
    env: &PreparedBzlEnvironment,
    source: &str,
) -> Result<FrozenBzlModule, String> {
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:apple_consumer.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace(
            "/fixture/pkg/apple_consumer.bzl".into(),
        ),
        repository_mapping: Arc::from([]),
    };
    let key = semantic_evaluation::prepare(
        &identity,
        "apple_consumer.bzl",
        &Arc::from(source.as_bytes()),
        BzlModuleContext::Build,
        env,
        std::iter::empty(),
    )?;
    let result = tx.compute(&key).await.map_err(|e| e.to_string())?;
    result.result().cloned().map_err(|e| format!("{e:?}"))
}

#[tokio::test]
async fn original_xcode_nondefault_requirements_catalyst_paths_and_environment() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2),
        &BuiltinRepositoryMapping::new(Arc::from([])).unwrap(),
    )
    .await
    .unwrap();
    let source = format!(
        "{CONFIG}{}",
        r#"
def verify():
    common = {'requires-darwin': '', 'supports-xcode-requirements-set': '', 'requires-xcode:15.4.0': '', 'requires-xcode-label:vendor-foo': ''}
    for availability in ['LOCAL', 'REMOTE', 'BOTH', 'UNKNOWN']:
        value = config(availability = availability)
        expected = dict(common)
        if availability == 'LOCAL': expected['no-remote'] = ''
        if availability == 'REMOTE': expected['no-local'] = ''
        if value.availability() != availability.lower() or value.execution_info() != expected:
            fail('availability/requirements: ' + availability)
    value = config()
    if type(value.xcode_version()) != 'DottedVersion' or value.xcode_version().to_string != '15.4.0': fail('version callback')
    if value.minimum_os_for_platform_type(apple_common.platform_type.catalyst).to_string != '15.1': fail('Catalyst minimum must use iOS')
    platforms = [(apple_common.platform.ios_device, '17.2'), (apple_common.platform.ios_simulator, '17.2'),
        (apple_common.platform.tvos_device, '17.4'), (apple_common.platform.tvos_simulator, '17.4'),
        (apple_common.platform.visionos_device, '1.3'), (apple_common.platform.visionos_simulator, '1.3'),
        (apple_common.platform.watchos_device, '10.2'), (apple_common.platform.watchos_simulator, '10.2'),
        (apple_common.platform.macos, '14'), (apple_common.platform.catalyst, '14')]
    for platform, expected in platforms:
        if value.sdk_version_for_platform(platform).to_string != expected: fail('SDK: ' + platform.name)
    if apple_common.target_apple_env(value, apple_common.platform.catalyst) != {'APPLE_SDK_PLATFORM': 'MacOSX', 'APPLE_SDK_VERSION_OVERRIDE': '14.0'}: fail('target environment')
    if apple_common.apple_host_system_env(value) != {'XCODE_VERSION_OVERRIDE': '15.4.0'}: fail('host environment')
    path = '/Applications/Other Xcode.app/Contents/Developer'
    selected = config(version = path, include = False)
    if selected.xcode_version() != path or type(selected.xcode_version()) != 'string': fail('path selection')
    if selected.execution_info() != {'requires-darwin': '', 'supports-xcode-requirements-set': '', 'no-remote': ''}: fail('execution flag off')
    if config(version = '').xcode_version() != None or apple_common.apple_host_system_env(config(version = '')) != {}: fail('empty selection')
    toolchain = apple_common.apple_toolchain()
    if toolchain.sdk_dir() != '__BAZEL_XCODE_SDKROOT__' or toolchain.developer_dir() != '__BAZEL_XCODE_DEVELOPER_DIR__': fail('toolchain')
    if toolchain.platform_developer_framework_dir(struct(single_arch_platform = apple_common.platform.catalyst)) != '__BAZEL_XCODE_DEVELOPER_DIR__/Platforms/MacOSX.platform/Developer/Library/Frameworks': fail('framework path')
    if apple_common.dotted_version(version = '23.4alpha2').to_string != '23.4alpha2': fail('original facade keyword binding')
verify()
"#
    );
    consumer(&mut tx, &env, &source).await.unwrap();
    let module = Module::new();
    let facade = env.exports[0]
        .value
        .value()
        .get_attr("dotted_version", module.heap())
        .unwrap()
        .unwrap();
    assert!(starlark::eval::inspect_def(facade, None).is_some());
    let native_namespace = env.exports[0]
        .module
        .module
        .get("native_apple_common")
        .unwrap();
    let native = native_namespace
        .value()
        .get_attr("dotted_version", module.heap())
        .unwrap()
        .unwrap();
    assert!(!facade.ptr_eq(native));
    assert!(starlark::eval::inspect_def(native, None).is_none());
}

#[tokio::test]
async fn original_xcode_initialization_and_sdk_methods_preserve_errors() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2),
        &BuiltinRepositoryMapping::new(Arc::from([])).unwrap(),
    )
    .await
    .unwrap();
    for (body, expected) in [
        (
            "VALUE = config(ios = '17..2')",
            "Dotted version components must all start",
        ),
        (
            "VALUE = config().sdk_version_for_platform(struct(name = 'unsupported'))",
            "Unhandled platform:",
        ),
        (
            "VALUE = config().minimum_os_for_platform_type('unsupported')",
            "Unhandled platform type: unsupported",
        ),
    ] {
        let error = consumer(&mut tx, &env, &format!("{CONFIG}\n{body}\n"))
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[starlark::starlark_module]
fn collection_globals(builder: &mut GlobalsBuilder) {
    fn collect(
        eval: &mut Evaluator<'_, '_, '_>,
    ) -> anyhow::Result<starlark::values::none::NoneType> {
        // Actual bytecode frame; all mutable roots belong to this module heap.
        unsafe {
            eval.garbage_collect()
                .map_err(starlark::Error::into_anyhow)?;
        }
        Ok(starlark::values::none::NoneType)
    }
}

#[tokio::test]
async fn original_objc_merges_depsets_and_export_closures_outlive_producer() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let env = prepare_build_environment(
        &mut tx,
        &BuiltinSourceCatalogKey::Bundled(BuiltinCatalogSnapshot::Bazel9_2),
        &BuiltinRepositoryMapping::new(Arc::from([])).unwrap(),
    )
    .await
    .unwrap();
    let source = format!(
        "{CONFIG}{}",
        r#"
leaf = apple_common.Objc(j2objc_library = depset(['leaf.a']), module_map = depset(['leaf.map']), source = depset(['leaf.m']), strict_include = depset(['private']), umbrella_header = depset(['leaf.h']))
merged = apple_common.new_objc_provider(j2objc_library = depset(['root.a']), module_map = depset(['root.map']), source = depset(['root.m']), strict_include = depset(['local']), umbrella_header = depset(['root.h']), providers = [leaf, leaf])
def verify():
    if merged.direct_module_maps != ['root.map'] or merged.direct_sources != ['root.m']: fail('direct fields')
    for actual, expected in [(merged.j2objc_library, ['leaf.a', 'root.a']), (merged.module_map, ['leaf.map', 'root.map']), (merged.source, ['leaf.m', 'root.m']), (merged.umbrella_header, ['leaf.h', 'root.h'])]:
        if sorted(actual.to_list()) != sorted(expected): fail('transitive merging')
    if merged.strict_include.to_list() != ['local']: fail('private include propagated')
verify()
selected = config()
def callback():
    return (selected.sdk_version_for_platform(apple_common.platform.catalyst).to_string, merged.source.to_list())
"#
    );
    let completed = consumer(&mut tx, &env, &source).await.unwrap();
    let scratch = Module::new();
    let a = env.exports[0]
        .value
        .value()
        .get_attr("Objc", scratch.heap())
        .unwrap()
        .unwrap();
    let b = env.exports[0]
        .value
        .value()
        .get_attr("new_objc_provider", scratch.heap())
        .unwrap()
        .unwrap();
    assert!(a.ptr_eq(b), "actual initialized constructor alias");
    let callback = completed.module.get("callback").unwrap();
    drop(completed);
    drop(env);
    drop(tx);
    drop(dice);
    let consumer = Module::new();
    let original = callback.owned_value(consumer.frozen_heap());
    drop(callback);
    consumer.set("_callback", original);
    let globals = GlobalsBuilder::new().with(collection_globals).build();
    let ast = starlark::syntax::AstModule::parse(
        "collect.star",
        "collect()\n".into(),
        &starlark::syntax::Dialect::Standard,
    )
    .unwrap();
    Evaluator::new(&consumer)
        .eval_module(ast, &globals)
        .unwrap();
    let original = consumer.get("_callback").unwrap();
    let value = Evaluator::new(&consumer)
        .eval_function(original, &[], &[])
        .unwrap();
    assert_eq!(value.to_repr(), "(\"14\", [\"leaf.m\", \"root.m\"])");
}
