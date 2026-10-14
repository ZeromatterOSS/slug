//! Actual configured publication and launch consumers of RunEnvironmentInfo.
use slug_build_api_v2::RunfilesSupportActionSpec;

use super::*;

const ENV_A: &str = "{'B': 'two', 'A': 'one', 'BOTH': 'fallback'}";
const ENV_B: &str = "{'A': 'one', 'B': 'two', 'BOTH': 'fallback'}";
const DEFS: &str = r#"
Answers = provider(fields=['value'])
Box = provider(fields=['nested'])
ENV = {'B': 'two', 'A': 'one', 'BOTH': 'fallback'}
def _render(info):
    return '|'.join([key + '=' + info.environment[key] for key in info.environment]) + ';' + '|'.join(info.inherited_environment)
def _fresh(info):
    if type(info) != 'RunEnvironmentInfo': fail('lost genuine provider type')
    expected = dict(ENV)
    if info.environment != expected or info.inherited_environment != ['BOTH', 'Z', 'BOTH']:
        fail('snapshot fields changed')
    first = info.environment
    names = info.inherited_environment
    if type(first) != 'dict' or type(names) != 'list': fail('getters must return mutable containers')
    first['MUTATED'] = 'private'
    first.clear()
    names.append('MUTATED')
    names.clear()
    if info.environment != expected or info.inherited_environment != ['BOTH', 'Z', 'BOTH']:
        fail('getter mutation changed retained provider')
    reversed_environment = {}
    for key in reversed(list(ENV)):
        reversed_environment[key] = ENV[key]
    equal = RunEnvironmentInfo(reversed_environment, ['BOTH', 'Z', 'BOTH'])
    if info != equal: fail('field equality depends on dict order')
def _info():
    environment = dict(ENV)
    inherited = ['BOTH', 'Z', 'BOTH']
    info = RunEnvironmentInfo(environment=environment, inherited_environment=inherited)
    environment.clear()
    inherited.clear()
    _fresh(info)
    return info
def _produce(ctx):
    info = _info()
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(executable, 'tool', is_executable=True)
    result = [DefaultInfo(executable=executable), Box(nested=struct(items=[info])), Answers(value=_render(info))]
    if ctx.attr.include_environment:
        result.append(info)
    return result
produce = rule(implementation=_produce, executable=True, attrs={'include_environment': attr.bool(default=True)})
produce_test = rule(implementation=_produce, test=True, attrs={'include_environment': attr.bool(default=True)})
def _carrier(ctx):
    return [Box(nested=struct(items=[_info()]))]
carrier = rule(implementation=_carrier)
def _read(ctx):
    info = ctx.attr.dep[Box].nested.items[0]
    _fresh(info)
    if RunEnvironmentInfo in ctx.attr.dep:
        _fresh(ctx.attr.dep[RunEnvironmentInfo])
        if ctx.attr.dep[RunEnvironmentInfo] != info: fail('top-level and nested provider differ')
    return [Answers(value=_render(info))]
read = rule(implementation=_read, attrs={'dep': attr.label(providers=[Box])})
def _forbidden(ctx):
    info = _info()
    result = [info]
    if ctx.attr.include_file:
        out = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.write(out, 'tool', is_executable=True)
        result.append(DefaultInfo(executable=out))
    return result
forbidden = rule(implementation=_forbidden, attrs={'include_file': attr.bool()})
def _duplicate(ctx):
    out = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(out, 'tool', is_executable=True)
    return [DefaultInfo(executable=out), _info(), _info()]
duplicate = rule(implementation=_duplicate, executable=True)
"#;
const BUILD: &str = r#"load(':defs.bzl', 'produce', 'produce_test', 'carrier', 'read', 'forbidden', 'duplicate')
produce(name='executable')
produce(name='plain', include_environment=False)
produce_test(name='subject_test')
carrier(name='carrier')
read(name='read_executable', dep=':executable')
read(name='read_carrier', dep=':carrier')
read(name='read_sibling', dep=':carrier')
forbidden(name='forbidden')
forbidden(name='file_forbidden', include_file=True)
duplicate(name='duplicate')
"#;

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(root.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    root
}
async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    target: &str,
    configuration: ConfigurationKey,
) -> Result<ConfiguredNodeResult, String> {
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{target}")).unwrap(),
        configuration,
    );
    analyze_request(dice, root, &key, None, false).await
}
fn answer(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Answers").unwrap())
}
fn expected(order: &str) -> String {
    format!("{order}|BOTH=fallback;BOTH|Z|BOTH")
}

#[tokio::test]
async fn executable_and_nested_consumers_preserve_snapshot_and_fresh_getters() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    // Declared Test/Suite admission is tested at the exact validation leaf.
    // The actual test target still requires its unprepared implicit test tools.
    for target in ["executable"] {
        let result = request(&dice, &root, target, config.clone()).await.unwrap();
        let info = result.providers().run_environment_info().unwrap();
        assert_eq!(
            info.environment()
                .keys()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["B", "A", "BOTH"]
        );
        assert_eq!(
            info.inherited_environment()
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["BOTH", "Z", "BOTH"]
        );
        assert_eq!(answer(&result), expected("B=two|A=one"));
        assert!(
            result
                .providers()
                .default_info()
                .unwrap()
                .files_to_run
                .support
                .is_some()
        );
    }
    let carrier = request(&dice, &root, "carrier", config.clone())
        .await
        .unwrap();
    assert!(carrier.providers().run_environment_info().is_none());
    assert!(carrier.actions().is_empty());
    for target in [
        "read_executable",
        "read_carrier",
        "read_sibling",
        "read_carrier",
    ] {
        let result = request(&dice, &root, target, config.clone()).await.unwrap();
        assert_eq!(answer(&result), expected("B=two|A=one"));
        assert!(result.actions().is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn non_executable_and_duplicate_returns_refuse_then_repair_on_one_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    for target in ["forbidden", "file_forbidden"] {
        // An Err exposes neither a configured provider collection nor actions,
        // including when the implementation registered a write before returning.
        let error = request(&dice, &root, target, config.clone())
            .await
            .unwrap_err();
        assert!(
            error.contains(
                "Returning RunEnvironmentInfo from a non-executable, non-test target has no effect"
            ),
            "{target}: {error}"
        );
    }
    let duplicate = request(&dice, &root, "duplicate", config.clone())
        .await
        .unwrap_err();
    assert!(
        duplicate.contains("RunEnvironmentInfo") && duplicate.contains("twice"),
        "{duplicate}"
    );
    fs::write(
        root.join("defs.bzl"),
        DEFS.replace(
            "forbidden = rule(implementation=_forbidden, attrs=",
            "forbidden = rule(implementation=_forbidden, executable=True, attrs=",
        ),
    )
    .unwrap();
    let repaired = request(&dice, &root, "file_forbidden", config.clone())
        .await
        .unwrap();
    assert!(repaired.providers().run_environment_info().is_some());
    assert_eq!(
        repaired
            .actions()
            .iter()
            .filter(|action| action.mnemonic() == "FileWrite")
            .count(),
        1
    );
    assert_eq!(repaired.actions().len(), 5);
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    let again = request(&dice, &root, "file_forbidden", config)
        .await
        .unwrap_err();
    assert!(again.contains("Returning RunEnvironmentInfo"), "{again}");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn map_order_only_source_a_b_a_changes_publication_and_consumer_iteration() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = typed_action_test_configuration();
    let first = request(&dice, &root, "executable", config.clone())
        .await
        .unwrap();
    let consumer_a = request(&dice, &root, "read_executable", config.clone())
        .await
        .unwrap();
    let nested_a = request(&dice, &root, "read_carrier", config.clone())
        .await
        .unwrap();
    fs::write(root.join("defs.bzl"), DEFS.replace(ENV_A, ENV_B)).unwrap();
    let changed = request(&dice, &root, "executable", config.clone())
        .await
        .unwrap();
    let consumer_b = request(&dice, &root, "read_executable", config.clone())
        .await
        .unwrap();
    let nested_b = request(&dice, &root, "read_carrier", config.clone())
        .await
        .unwrap();
    assert!(first != changed);
    let a = first.providers().run_environment_info().unwrap();
    let b = changed.providers().run_environment_info().unwrap();
    assert!(a.field_eq(b));
    assert_ne!(a, b);
    assert_eq!(answer(&consumer_a), expected("B=two|A=one"));
    assert_eq!(answer(&consumer_b), expected("A=one|B=two"));
    assert_eq!(answer(&nested_a), expected("B=two|A=one"));
    assert_eq!(answer(&nested_b), expected("A=one|B=two"));
    assert!(consumer_a != consumer_b && nested_a != nested_b);
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert!(
        first
            == request(&dice, &root, "executable", config.clone())
                .await
                .unwrap()
    );
    assert!(
        consumer_a
            == request(&dice, &root, "read_executable", config.clone())
                .await
                .unwrap()
    );
    assert!(nested_a == request(&dice, &root, "read_carrier", config).await.unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn launch_channels_preserve_overlap_without_changing_symlink_tree_environment() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = typed_action_test_configuration();
    let base = base.slug_configuration().unwrap();
    let overlay: CommandConfigurationOverlay = [
        ("CFG_FIXED=configured"),
        ("CFG_DYNAMIC"),
        ("BOTH=configured"),
    ]
    .iter()
    .map(|value| {
        CommandConfigurationOccurrence::native(NativeCommandOption::ActionEnv, Some(*value), false)
    })
    .collect::<Vec<_>>()
    .into();
    let structural = base
        .with_command_configuration(base.starlark_options().clone(), &overlay)
        .unwrap();
    let configured = structural.configured_action_environment().unwrap();
    let config = ConfigurationKey::from_slug(structural);
    for target in ["executable", "plain"] {
        let result = request(&dice, &root, target, config.clone()).await.unwrap();
        let support = result
            .providers()
            .default_info()
            .unwrap()
            .files_to_run
            .support
            .as_ref()
            .unwrap();
        let launch = &support.launch_environment;
        if target == "executable" {
            assert_eq!(launch.fixed().get("BOTH"), Some("fallback"));
            assert!(launch.inherited().contains("BOTH"));
            assert_eq!(launch.inherited().iter().collect::<Vec<_>>(), ["BOTH", "Z"]);
            assert_eq!(launch.fixed().get("CFG_FIXED"), None);
            assert!(!launch.inherited().contains("CFG_DYNAMIC"));
        } else {
            assert_eq!(launch.fixed().iter().next(), None);
            assert_eq!(launch.inherited().iter().next(), None);
        }
        let symlink = result
            .actions()
            .iter()
            .find_map(|action| match action.runfiles_support_spec() {
                Some(RunfilesSupportActionSpec::SymlinkTree { environment, .. }) => {
                    Some(environment)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(symlink, &configured);
        assert_eq!(symlink.fixed().get("BOTH"), Some("configured"));
        assert_eq!(symlink.fixed().get("CFG_FIXED"), Some("configured"));
        assert!(symlink.inherited().contains("CFG_DYNAMIC"));
    }
    fs::remove_dir_all(root).unwrap();
}
