//! Genuine configured dependency publication through the original Apple export.
use super::*;

#[tokio::test]
async fn original_apple_provider_and_version_publish_to_a_fresh_configured_consumer() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    let prepared_repositories = builtin_fixture::prepare(&workspace);
    let repositories = prepared_repositories
        .iter()
        .map(|(canonical, path)| (canonical.as_str(), path.as_str()))
        .collect::<Vec<_>>();
    let definitions = |spelling: &str| {
        r#"
Payload = provider(fields = ['value', 'callback', 'version', 'alias', 'objc'])
ObservedInfo = provider(fields = ['value', 'callback', 'version'])
KEY = apple_common.XcodeVersionConfig

def _producer(ctx):
    config = KEY(
        ios_sdk_version = '17.2', ios_minimum_os_version = '15.1',
        visionos_sdk_version = '1.3', visionos_minimum_os_version = '1.1',
        watchos_sdk_version = '10.2', watchos_minimum_os_version = '8.1',
        tvos_sdk_version = '17.4', tvos_minimum_os_version = '14.2',
        macos_sdk_version = '14', macos_minimum_os_version = '12.3',
        xcode_version = '15.4.0', availability = 'REMOTE',
        xcode_version_flag = '15.4-vendor', include_xcode_execution_info = True)
    version = apple_common.dotted_version(version = 'FULLSPELLING')
    compare = version.compare_to
    leaf = apple_common.Objc(source = depset(['leaf.m']), strict_include = depset(['private']))
    objc = apple_common.new_objc_provider(source = depset(['root.m']), strict_include = depset(['local']), providers = [leaf])
    def original(value):
        return config.sdk_version_for_platform(apple_common.platform.catalyst).to_string + ':' + version.to_string + ':' + str(compare(apple_common.dotted_version(value)))
    return [config, Payload(value = 'producer', callback = original, version = version, alias = version, objc = objc)]

def _consumer(ctx):
    info = ctx.attr.dep[KEY]
    payload = ctx.attr.dep[Payload]
    if info.availability() != 'remote' or info.execution_info() != {'requires-darwin': '', 'supports-xcode-requirements-set': '', 'no-local': '', 'requires-xcode:15.4.0': '', 'requires-xcode-label:vendor': ''}: fail('published requirements')
    if info.minimum_os_for_platform_type(apple_common.platform_type.catalyst).to_string != '15.1': fail('Catalyst minimum')
    if payload.version.to_string != payload.alias.to_string or type(payload.version) != 'DottedVersion': fail('native version projection')
    if sorted(payload.objc.source.to_list()) != ['leaf.m', 'root.m'] or payload.objc.strict_include.to_list() != ['local']: fail('original Objc merge')
    result = payload.callback('1.0')
    if result != '14:' + payload.version.to_string + ':0': fail('original callback/bound receiver')
    return [ObservedInfo(value = result, callback = payload.callback, version = payload.version)]

producer = rule(implementation = _producer, provides = [KEY, Payload])
consumer = rule(implementation = _consumer, attrs = {'dep': attr.label(providers = [KEY, Payload])})
"#.replace("FULLSPELLING", spelling)
    };
    let source = workspace.join("defs.bzl");
    fs::write(&source, definitions("01.00.internal.A")).unwrap();
    fs::write(workspace.join("BUILD.bazel"), "load(':defs.bzl', 'producer', 'consumer')\nproducer(name='xcode')\nconsumer(name='observed', dep=':xcode')\n").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let configuration = test_configuration();
    let key = |name| {
        ConfiguredTargetKey::new(
            CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
            configuration.clone(),
        )
    };
    let produced = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("xcode"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    let original_key = ProviderId::new(
        "@@_builtins//:common/xcode/providers.bzl",
        "XcodeVersionInfo",
    )
    .unwrap();
    assert!(
        produced
            .providers()
            .contains(&ProviderIdentity::user(original_key))
    );
    let payload = produced
        .providers()
        .user(&ProviderId::new("//:defs.bzl", "Payload").unwrap())
        .unwrap();
    let version = payload.field("version").unwrap();
    let AnalysisValueKind::DottedVersion(model) = version.kind() else {
        panic!("honest version native model")
    };
    assert_eq!(model.spelling(), "01.00.internal.A");
    let version_origin = version
        .original_origin()
        .expect("actual version original slot");
    assert!(version_origin.original_slot().is_some());
    assert!(
        version_origin.same_original(payload.field("alias").unwrap().original_origin().unwrap())
    );
    let function = payload
        .field("callback")
        .unwrap()
        .original_origin()
        .expect("original callback descriptor");
    let slug_build_api_v2::published_value::PublishedNode::Function(descriptor) = function.node()
    else {
        panic!("original function row")
    };
    let authority = descriptor.code.source.data();
    assert!(
        authority.direct_children.is_empty(),
        "environment is not a fabricated source load"
    );
    assert!(
        authority
            .environment_bindings
            .iter()
            .any(|binding| binding.global == "apple_common"
                && binding.export == "apple_common_bazel"
                && binding.module.source.builtin_catalog().is_some())
    );
    assert_eq!(
        authority
            .reachable
            .iter()
            .filter(|source| source.identity.source.builtin_catalog().is_some())
            .count(),
        6,
        "complete bundled source closure must survive conversion"
    );
    // This is a separate configured evaluation after the producer publication.
    let observed = ProviderId::new("//:defs.bzl", "ObservedInfo").unwrap();
    let initial = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("observed"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&initial, &observed), "14:01.00.internal.A:0");
    fs::write(&source, definitions("01.00.internal.B")).unwrap();
    let changed = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("observed"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    assert_eq!(provider_value(&changed, &observed), "14:01.00.internal.B:0");
    assert!(
        initial != changed,
        "spelling-only native state is publication-visible"
    );
    fs::write(&source, definitions("01.00.internal.A")).unwrap();
    let restored = analyze_request_with_repositories(
        &dice,
        &workspace,
        &key("observed"),
        None,
        false,
        &repositories,
        false,
    )
    .await
    .unwrap();
    assert_eq!(
        provider_value(&restored, &observed),
        "14:01.00.internal.A:0"
    );
    assert!(
        initial == restored,
        "complete original state restores without Debug expansion"
    );
}
