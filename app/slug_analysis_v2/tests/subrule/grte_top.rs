//! Pinned CppConfiguration's nullable private method in real root/subrule calls.
use super::*;

const TARGET: &str = "@@//subject:grte_top";

fn epoch(marker: &str) -> PathObservationEpoch {
    let mut epoch = Epoch::fixture();
    epoch.directory("/workspace/tools");
    epoch.directory("/workspace/tools/build_defs");
    epoch.package("tools/build_defs/cc", "");
    epoch.file(
        "/workspace/tools/build_defs/cc/grte_top.bzl",
        &format!(
            r#"GrteInfo = provider(fields = ["root", "sub", "marker"])
def _sub(ctx):
    return ctx.fragments.cpp.grte_top()
sub = subrule(implementation = _sub, fragments = ["cpp"])
def _root(ctx):
    root = ctx.fragments.cpp.grte_top()
    child = sub()
    if root != child: fail("root and subrule libc labels diverged")
    if root != None:
        if type(root) != "Label" or root.name != "everything" or root.package != "libc":
            fail("libc option was not a converted Label: %s" % root)
    return [DefaultInfo(), GrteInfo(root = root, sub = child, marker = "{marker}")]
subject = rule(implementation = _root, fragments = ["cpp"], subrules = [sub])
"#
        ),
    );
    epoch.package(
        "subject",
        "load('//tools/build_defs/cc:grte_top.bzl', 'subject')\nsubject(name='grte_top')\n",
    );
    // The method returns the option label; it must not materialize that target.
    epoch.missing("/workspace/libc");
    epoch.build()
}

fn assert_projection(
    result: &slug_analysis_v2::ConfiguredNodeResult,
    label: Option<&str>,
    marker: &str,
) {
    assert!(result.actions().is_empty());
    assert!(
        result
            .edges()
            .iter()
            .all(|edge| !edge.target().label().to_string().contains("//libc:"))
    );
    let returned = result
        .providers()
        .occurrences()
        .find(|provider| provider.field("marker").is_some())
        .unwrap();
    for field in ["root", "sub"] {
        match (returned.field(field).unwrap().kind(), label) {
            (AnalysisValueKind::None, None) => {}
            (AnalysisValueKind::Label(actual), Some(expected)) => {
                assert_eq!(actual.to_string(), expected);
            }
            (actual, expected) => panic!("{field}: expected {expected:?}, got {actual:?}"),
        }
    }
    assert!(
        matches!(returned.field("marker").unwrap().kind(), AnalysisValueKind::String(actual) if actual == marker)
    );
}

#[tokio::test]
async fn root_subrule_grte_top_projects_labels_and_restores_target_exec_configuration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let request = |configuration| {
            analyze_result(
                &dice,
                epoch("A"),
                TARGET,
                configuration,
                tracker.clone(),
                route,
            )
        };
        let absent = request(configuration(None)).await.unwrap();
        assert_projection(&absent, None, "A");
        let target_configuration = grte_configuration("//libc:ignored");
        let selected = request(target_configuration.clone()).await.unwrap();
        assert_projection(&selected, Some("@@//libc:everything"), "A");
        assert_ne!(selected, absent);
        assert_eq!(
            request(grte_configuration("default")).await.unwrap(),
            absent
        );
        assert_eq!(
            request(target_configuration.clone()).await.unwrap(),
            selected
        );

        let target = target_configuration.slug_configuration().unwrap();
        for execution in [
            target.to_exec().unwrap(),
            target
                .to_exec_for_platform(&target.host_platform_label().unwrap())
                .unwrap(),
        ] {
            let executed = request(ConfigurationKey::from_slug(execution))
                .await
                .unwrap();
            assert_projection(&executed, None, "A");
            assert_ne!(
                executed, selected,
                "exec must reset the target libc override from default host state"
            );
            assert_eq!(
                request(target_configuration.clone()).await.unwrap(),
                selected
            );
        }
    }
}

#[tokio::test]
async fn root_subrule_grte_top_restores_source_in_one_dice_without_libc_lookup() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let configuration = grte_configuration("//libc");
        let first = analyze_result(
            &dice,
            epoch("A"),
            TARGET,
            configuration.clone(),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        assert_projection(&first, Some("@@//libc:everything"), "A");
        let changed = analyze_result(
            &dice,
            epoch("B"),
            TARGET,
            configuration.clone(),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        assert_projection(&changed, Some("@@//libc:everything"), "B");
        assert_ne!(changed, first);
        let restored = analyze_result(&dice, epoch("A"), TARGET, configuration, tracker, route)
            .await
            .unwrap();
        assert_eq!(restored, first);
    }
}
