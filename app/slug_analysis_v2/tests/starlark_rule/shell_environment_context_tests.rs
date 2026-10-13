//! Actual configured consumers of the fixed, copied default shell environment.
use super::*;

const DEFS: &str = r#"
Answers = provider(fields=['value'])
Snapshot = provider(fields=['environment'])
def _render(environment):
    return '|'.join([key + '=' + environment[key] for key in environment])
def _probe(ctx):
    first = ctx.configuration.default_shell_env
    if type(first) != 'dict': fail('default_shell_env must be a plain dictionary')
    original = dict(first)
    first['ONLY_FIRST'] = 'mutable'
    first.update({'ADDED': 'mutable'})
    first.clear()
    second = ctx.configuration.default_shell_env
    if second != original: fail('first returned dictionary changed configuration')
    second.setdefault('ONLY_SECOND', 'mutable')
    second.pop('ONLY_SECOND')
    second['ONLY_SECOND'] = 'mutable'
    if ctx.configuration.default_shell_env != original:
        fail('later reads shared mutable dictionary state')
    return [Answers(value=_render(original) + ';source-A'), Snapshot(environment=original)]
probe = rule(implementation=_probe)
def _read(ctx):
    environment = ctx.attr.dep[Snapshot].environment
    if type(environment) != 'dict': fail('published environment changed type')
    copied = dict(environment)
    copied['CONSUMER_ONLY'] = 'mutable'
    if 'CONSUMER_ONLY' in environment or ctx.configuration.default_shell_env != environment:
        fail('consumer copy changed producer or consumer configuration')
    return [Answers(value=_render(environment) + ';source-A')]
read = rule(implementation=_read, attrs={'dep':attr.label(providers=[Snapshot])})
def _bridge(ctx):
    return [Answers(value=_render(ctx.attr.target[Snapshot].environment) + ' || ' + _render(ctx.attr.tool[Snapshot].environment))]
bridge = rule(implementation=_bridge, attrs={
    'target':attr.label(providers=[Snapshot]),
    'tool':attr.label(providers=[Snapshot],cfg='exec'),
})
"#;

const BUILD: &str = "load(':defs.bzl','probe','read','bridge')\nplatform(name='exec_platform')\nprobe(name='producer')\nprobe(name='other')\nread(name='request',dep=':producer')\nread(name='sibling',dep=':producer')\nbridge(name='bridge',target=':producer',tool=':producer')\n";

fn workspace() -> PathBuf {
    let root = scratch();
    fs::write(
        root.join("MODULE.bazel"),
        "module(name='root')\nregister_execution_platforms('//:exec_platform')\n",
    )
    .unwrap();
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    fs::write(root.join("BUILD.bazel"), BUILD).unwrap();
    root
}

fn configuration(rows: &[(NativeCommandOption, &str)]) -> ConfigurationKey {
    let base = typed_action_test_configuration();
    let base = base.slug_configuration().unwrap();
    let overlay: CommandConfigurationOverlay = rows
        .iter()
        .map(|(option, value)| CommandConfigurationOccurrence::native(*option, Some(*value), false))
        .collect::<Vec<_>>()
        .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

async fn request(
    dice: &Arc<Dice>,
    root: &PathBuf,
    name: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<ConfiguredNodeResult>, String> {
    root_target_request_with_configuration(
        dice,
        root,
        &format!("@@//:{name}"),
        configuration,
        Arc::new(RootActivationTracker::default()),
    )
    .await
}

fn answer(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Answers").unwrap())
}

#[tokio::test]
async fn shell_environment_strict_non_strict_copies_mutate_independently_and_publish() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    for (rows, expected) in [
        (vec![], "PATH=/bin:/usr/bin:/usr/local/bin;source-A"),
        (
            vec![(NativeCommandOption::IncompatibleStrictActionEnv, "false")],
            ";source-A",
        ),
    ] {
        let config = configuration(&rows);
        let first = request(&dice, &root, "request", config.clone())
            .await
            .unwrap();
        assert_eq!(answer(&first), expected);
        for consumer in ["producer", "other", "sibling"] {
            assert_eq!(
                answer(
                    &request(&dice, &root, consumer, config.clone())
                        .await
                        .unwrap()
                ),
                expected,
                "{consumer} must not observe another getter's dictionary mutations"
            );
        }
        assert_eq!(
            first,
            request(&dice, &root, "request", config).await.unwrap()
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shell_environment_fixed_inherited_unset_last_wins_and_utf16_order() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let config = configuration(&[
        (NativeCommandOption::ActionEnv, "PATH"),
        (NativeCommandOption::ActionEnv, "FIXED=first"),
        (NativeCommandOption::ActionEnv, "FIXED=last=with=equals"),
        (NativeCommandOption::ActionEnv, "DROP=before-unset"),
        (NativeCommandOption::ActionEnv, "=DROP"),
        (NativeCommandOption::ActionEnv, "INHERITED=before-inherit"),
        (NativeCommandOption::ActionEnv, "INHERITED"),
        (NativeCommandOption::ActionEnv, "RESTORED"),
        (NativeCommandOption::ActionEnv, "RESTORED=fixed"),
        (NativeCommandOption::ActionEnv, "EMPTY="),
        (NativeCommandOption::ActionEnv, "Z=z"),
        (NativeCommandOption::ActionEnv, "\u{e000}=bmp"),
        (NativeCommandOption::ActionEnv, "\u{10000}=supplementary"),
    ]);
    let expected = "EMPTY=|FIXED=last=with=equals|RESTORED=fixed|Z=z|\u{10000}=supplementary|\u{e000}=bmp;source-A";
    for consumer in ["producer", "request", "sibling"] {
        assert_eq!(
            answer(
                &request(&dice, &root, consumer, config.clone())
                    .await
                    .unwrap()
            ),
            expected,
            "{consumer}: fixed entries must retain UTF16 order, omit inherited/unset entries"
        );
    }
    // Returning to strict defaults on the same engine must discard all rows.
    assert_eq!(
        answer(
            &request(&dice, &root, "request", configuration(&[]))
                .await
                .unwrap()
        ),
        "PATH=/bin:/usr/bin:/usr/local/bin;source-A"
    );
    assert_eq!(
        answer(&request(&dice, &root, "request", config).await.unwrap()),
        expected
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shell_environment_target_exec_uses_host_rows_on_both_projection_paths() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let target = configuration(&[
        (NativeCommandOption::ActionEnv, "MODE=target"),
        (NativeCommandOption::ActionEnv, "TARGET_ONLY=target"),
        (NativeCommandOption::HostActionEnv, "MODE=first"),
        (NativeCommandOption::HostActionEnv, "MODE=exec"),
        (NativeCommandOption::HostActionEnv, "HOST_ONLY=host"),
        (NativeCommandOption::HostActionEnv, "HOST_DYNAMIC"),
        (NativeCommandOption::HostActionEnv, "DROP=before-unset"),
        (NativeCommandOption::HostActionEnv, "=DROP"),
        (NativeCommandOption::HostActionEnv, "PATH"),
    ]);
    let first = request(&dice, &root, "producer", target.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&first),
        "MODE=target|PATH=/bin:/usr/bin:/usr/local/bin|TARGET_ONLY=target;source-A"
    );
    let structural = target.slug_configuration().unwrap();
    let platform = CanonicalLabel::parse("@@//:exec_platform").unwrap();
    for exec in [
        structural.to_exec().unwrap(),
        structural.to_exec_for_platform(&platform).unwrap(),
    ] {
        let result = request(&dice, &root, "producer", ConfigurationKey::from_slug(exec))
            .await
            .unwrap();
        assert_eq!(answer(&result), "HOST_ONLY=host|MODE=exec;source-A");
    }
    let bridge = request(&dice, &root, "bridge", target.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&bridge),
        "MODE=target|PATH=/bin:/usr/bin:/usr/local/bin|TARGET_ONLY=target || HOST_ONLY=host|MODE=exec"
    );
    assert_eq!(
        first,
        request(&dice, &root, "producer", target).await.unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shell_environment_source_configuration_and_retained_host_a_b_a_use_one_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let base = configuration(&[]);
    let first = request(&dice, &root, "request", base.clone())
        .await
        .unwrap();
    let option = configuration(&[(NativeCommandOption::ActionEnv, "OPTION=B")]);
    let changed = request(&dice, &root, "request", option).await.unwrap();
    assert_eq!(
        answer(&changed),
        "OPTION=B|PATH=/bin:/usr/bin:/usr/local/bin;source-A"
    );
    assert_ne!(first, changed);
    assert_eq!(
        first,
        request(&dice, &root, "request", base.clone())
            .await
            .unwrap()
    );
    fs::write(root.join("defs.bzl"), DEFS.replace("source-A", "source-B")).unwrap();
    let changed = request(&dice, &root, "request", base.clone())
        .await
        .unwrap();
    assert_eq!(
        answer(&changed),
        "PATH=/bin:/usr/bin:/usr/local/bin;source-B"
    );
    assert_ne!(first, changed);
    fs::write(root.join("defs.bzl"), DEFS).unwrap();
    assert_eq!(
        first,
        request(&dice, &root, "request", base.clone())
            .await
            .unwrap()
    );
    // Change only the retained Host observation, leaving native options and
    // authored consumers unchanged. Strict PATH is independent of client PATH.
    let host_inputs = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap()
    .with_action_environment_host(ActionEnvironmentHost::windows(
        Some("D:/tools/bin/bash.exe"),
        Some("ignored-client-path"),
        Some("D:\\Windows"),
    ));
    let host = ConfigurationKey::from_slug(
        SlugConfiguration::default_target(&host_inputs)
            .unwrap()
            .with_host_platform_label(&CanonicalLabel::parse("@@//.slug_test_host:host").unwrap()),
    );
    let changed = request(&dice, &root, "request", host).await.unwrap();
    assert_eq!(
        answer(&changed),
        "PATH=D:\\tools\\bin;D:\\tools\\usr\\bin;D:\\Windows;D:\\Windows\\System32;D:\\Windows\\System32\\WindowsPowerShell\\v1.0|RUNFILES_MANIFEST_ONLY=1;source-A"
    );
    assert_ne!(first, changed);
    assert_eq!(first, request(&dice, &root, "request", base).await.unwrap());
    fs::remove_dir_all(root).unwrap();
}
