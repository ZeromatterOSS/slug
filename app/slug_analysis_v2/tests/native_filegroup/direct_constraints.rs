use super::*;

fn declarations(fallback: &str, default: &str) -> String {
    format!(
        r#"constraint_setting(name='os', default_constraint_value=':{default}')
constraint_value(name='none', constraint_setting=':os')
constraint_value(name='linux', constraint_setting=':os')
constraint_value(name='windows', constraint_setting=':os')
constraint_value(name='wasi', constraint_setting=':os')
platform(name='linux_platform', constraint_values=[':linux'])
platform(name='windows_platform', constraint_values=[':windows'])
platform(name='omitted_platform')
alias(name='windows_key', actual=':windows')
filegroup(name='direct', srcs=select({{
    ':none': ['a.txt'],
    ':windows_key': ['b.txt'],
    ':wasi': ['c.txt'],
    '//conditions:default': ['{fallback}'],
}}))
"#
    )
}

fn platform_configuration(platform: &str, execution: bool) -> ConfigurationKey {
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

fn assert_selected_source(result: &ConfiguredNodeResult, name: &str) {
    assert!(result.actions().is_empty());
    let source = CanonicalLabel::parse(&format!("@@//:{name}")).unwrap();
    let expected = AnalysisArtifact::Source(source.clone());
    let default = info(result);
    assert_eq!(default.file_artifacts(), [expected.clone()]);
    assert_eq!(default.executable.as_ref(), Some(&expected));
    assert_eq!(default.files_to_run.executable.as_ref(), Some(&expected));
    assert_eq!(
        default.files_to_run.files().to_list(),
        [AnalysisValue::artifact(expected)]
    );
    assert!(default.files_to_run.support.is_none());
    let edges = result
        .edges()
        .iter()
        .filter(|edge| matches!(edge.kind(), ConfiguredEdgeKind::Attribute { attribute, .. } if attribute == "srcs"))
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 1, "{edges:?}");
    assert_eq!(edges[0].target(), &ConfiguredNodeKey::Null(source));
}

#[tokio::test]
async fn native_filegroup_direct_constraints_track_target_exec_platform_source_and_defaults() {
    let workspace = Workspace::new();
    let build = workspace.0.join("BUILD.bazel");
    let original = declarations("d.txt", "none");
    fs::write(&build, &original).unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for execution in [false, true] {
        let linux = platform_configuration("linux", execution);
        let windows = platform_configuration("windows", execution);
        let omitted = platform_configuration("omitted", execution);
        let first = workspace
            .request(&dice, false, "direct", linux.clone())
            .await
            .unwrap();
        assert_selected_source(&first, "d.txt");
        let selected_windows = workspace
            .request(&dice, false, "direct", windows)
            .await
            .unwrap();
        assert_selected_source(&selected_windows, "b.txt");
        assert_ne!(selected_windows, first);
        assert_eq!(
            workspace
                .request(&dice, false, "direct", linux.clone())
                .await
                .unwrap(),
            first,
            "configuration A/B/A restores the original selected source"
        );
        let selected_default = workspace
            .request(&dice, false, "direct", omitted.clone())
            .await
            .unwrap();
        assert_selected_source(&selected_default, "a.txt");

        fs::write(
            &build,
            original.replace(
                "platform(name='linux_platform', constraint_values=[':linux'])",
                "platform(name='linux_platform', constraint_values=[':windows'])",
            ),
        )
        .unwrap();
        let platform_changed = workspace
            .request(&dice, false, "direct", linux.clone())
            .await
            .unwrap();
        assert_selected_source(&platform_changed, "b.txt");
        assert_ne!(
            platform_changed, first,
            "same-configuration platform edits invalidate selector truth"
        );
        fs::write(&build, &original).unwrap();
        assert_eq!(
            workspace
                .request(&dice, false, "direct", linux.clone())
                .await
                .unwrap(),
            first,
            "platform source A/B/A restores the original configured result"
        );

        fs::write(&build, declarations("c.txt", "none")).unwrap();
        let source_changed = workspace
            .request(&dice, false, "direct", linux.clone())
            .await
            .unwrap();
        assert_selected_source(&source_changed, "c.txt");
        assert_ne!(source_changed, first);
        fs::write(&build, &original).unwrap();
        assert_eq!(
            workspace
                .request(&dice, false, "direct", linux.clone())
                .await
                .unwrap(),
            first,
            "selector source A/B/A restores the original configured result"
        );

        fs::write(&build, declarations("d.txt", "windows")).unwrap();
        let default_changed = workspace
            .request(&dice, false, "direct", omitted.clone())
            .await
            .unwrap();
        assert_selected_source(&default_changed, "b.txt");
        assert_ne!(default_changed, selected_default);
        assert_selected_source(
            &workspace
                .request(&dice, false, "direct", linux.clone())
                .await
                .unwrap(),
            "d.txt",
        );
        fs::write(&build, &original).unwrap();
        assert_eq!(
            workspace
                .request(&dice, false, "direct", omitted)
                .await
                .unwrap(),
            selected_default,
            "setting-default source A/B/A restores omitted-setting matching"
        );
        assert_eq!(
            workspace
                .request(&dice, false, "direct", linux)
                .await
                .unwrap(),
            first,
            "explicit platform values override the edited setting default"
        );
    }
}
