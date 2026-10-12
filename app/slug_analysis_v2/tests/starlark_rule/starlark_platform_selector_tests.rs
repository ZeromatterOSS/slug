use super::*;

fn definitions(build_setting: Option<&str>) -> String {
    let marker = build_setting.map_or(String::new(), |value| format!(", build_setting={value}"));
    let default = if build_setting.is_some_and(|value| value != "None") {
        ", build_setting_default=False"
    } else {
        ""
    };
    format!(
        "Selected = provider(fields={{'value':''}})\ndef _probe(ctx): return [Selected(value=ctx.attr.text)]\nprobe = rule(implementation=_probe, attrs={{'text':attr.string()}}, toolchains=[]{marker})\ndef subject(name, text): probe(name=name, text=text{default})\n"
    )
}

const BUILD: &str = r#"load(':defs.bzl', 'subject')
constraint_setting(name='os', default_constraint_value=':none')
constraint_value(name='none', constraint_setting=':os')
constraint_value(name='linux', constraint_setting=':os')
constraint_value(name='windows', constraint_setting=':os')
platform(name='linux_platform', constraint_values=[':linux'])
platform(name='windows_platform', constraint_values=[':windows'])
platform(name='omitted_platform')
alias(name='windows_key', actual=':windows')
subject(name='selected', text=select({':none':'none', ':windows_key':'windows', '//conditions:default':'fallback'}))
subject(name='direct', text=select({':linux':'linux', '//conditions:default':'fallback'}))
"#;

fn configuration(platform: &str, execution: bool) -> ConfigurationKey {
    let platform = CanonicalLabel::parse(&format!("@@//:{platform}_platform")).unwrap();
    let target = test_configuration()
        .slug_configuration()
        .unwrap()
        .with_host_platform_label(&platform);
    ConfigurationKey::from_slug(if execution {
        target.to_exec_for_platform(&platform).unwrap()
    } else {
        target
    })
}

async fn request(
    dice: &Arc<Dice>,
    workspace: &PathBuf,
    name: &str,
    configuration: ConfigurationKey,
) -> Result<ConfiguredNodeResult, String> {
    let key = ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
        configuration,
    );
    analyze_request(dice, workspace, &key, None, false).await
}

fn value(result: &ConfiguredNodeResult) -> String {
    provider_value(result, &ProviderId::new("//:defs.bzl", "Selected").unwrap())
}

#[tokio::test]
async fn ordinary_rule_direct_constraints_track_target_exec_defaults_aliases_and_restore() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(workspace.join("defs.bzl"), definitions(None)).unwrap();
    let build = workspace.join("BUILD.bazel");
    fs::write(&build, BUILD).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for execution in [false, true] {
        let linux = configuration("linux", execution);
        let windows = configuration("windows", execution);
        let omitted = configuration("omitted", execution);
        let first = request(&dice, &workspace, "selected", linux.clone())
            .await
            .unwrap();
        assert_eq!(value(&first), "fallback");
        for (name, config, expected) in [
            ("direct", linux.clone(), "linux"),
            ("selected", windows, "windows"),
        ] {
            let selected = request(&dice, &workspace, name, config).await.unwrap();
            assert_eq!(value(&selected), expected);
        }
        assert_eq!(
            request(&dice, &workspace, "selected", linux.clone())
                .await
                .unwrap(),
            first
        );
        let default = request(&dice, &workspace, "selected", omitted.clone())
            .await
            .unwrap();
        assert_eq!(value(&default), "none");
        for (from, to, config, expected, original) in [
            (
                "actual=':windows'",
                "actual=':linux'",
                linux.clone(),
                "windows",
                &first,
            ),
            (
                "default_constraint_value=':none'",
                "default_constraint_value=':windows'",
                omitted.clone(),
                "windows",
                &default,
            ),
            (
                "constraint_values=[':linux']",
                "constraint_values=[':windows']",
                linux.clone(),
                "windows",
                &first,
            ),
        ] {
            fs::write(&build, BUILD.replace(from, to)).unwrap();
            let changed = request(&dice, &workspace, "selected", config.clone())
                .await
                .unwrap();
            assert_eq!(value(&changed), expected);
            assert_ne!(&changed, original);
            fs::write(&build, BUILD).unwrap();
            assert_eq!(
                &request(&dice, &workspace, "selected", config)
                    .await
                    .unwrap(),
                original
            );
        }
    }
}

#[tokio::test]
async fn rule_build_setting_metadata_disables_constraints_and_restores_condition_key_isolation() {
    let workspace = scratch();
    fs::write(workspace.join("MODULE.bazel"), "module(name='root')\n").unwrap();
    fs::write(workspace.join("BUILD.bazel"), BUILD).unwrap();
    let defs = workspace.join("defs.bzl");
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let linux = configuration("linux", false);
    for ordinary in [None, Some("None")] {
        fs::write(&defs, definitions(ordinary)).unwrap();
        let first = request(&dice, &workspace, "direct", linux.clone())
            .await
            .unwrap();
        assert_eq!(value(&first), "linux");
        for flag in ["True", "False"] {
            fs::write(
                &defs,
                definitions(Some(&format!("config.bool(flag={flag})"))),
            )
            .unwrap();
            for (name, platform) in [("direct", "linux"), ("selected", "windows")] {
                let error = request(&dice, &workspace, name, configuration(platform, false))
                    .await
                    .unwrap_err();
                assert!(
                    error.contains("not a valid select() condition"),
                    "{flag}/{name}: {error}"
                );
            }
            fs::write(&defs, definitions(ordinary)).unwrap();
            assert_eq!(
                request(&dice, &workspace, "direct", linux.clone())
                    .await
                    .unwrap(),
                first
            );
        }
        let error = configured_condition_request(&dice, &workspace, "@@//:linux", linux.clone())
            .await
            .unwrap_err();
        assert!(error.contains("not a valid select() condition"), "{error}");
        assert_eq!(
            request(&dice, &workspace, "direct", linux.clone())
                .await
                .unwrap(),
            first
        );
    }
}
