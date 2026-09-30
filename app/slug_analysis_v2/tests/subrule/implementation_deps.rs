//! Pinned private C++ Boolean projection in real root/subrule invocations.
use super::*;

const TARGET: &str = "@@//subject:implementation_deps";

fn epoch(marker: &str) -> PathObservationEpoch {
    let mut epoch = Epoch::fixture();
    epoch.directory("/workspace/tools");
    epoch.directory("/workspace/tools/build_defs");
    epoch.package("tools/build_defs/cc", "");
    epoch.file(
        "/workspace/tools/build_defs/cc/implementation_deps.bzl",
        &format!(
            r#"ImplementationDepsInfo = provider(fields = ["root", "sub", "marker"])
def _sub(ctx):
    return ctx.fragments.cpp.experimental_cc_implementation_deps()
sub = subrule(implementation = _sub, fragments = ["cpp"])
def _root(ctx):
    root = ctx.fragments.cpp.experimental_cc_implementation_deps()
    child = sub()
    if type(root) != "bool" or type(child) != "bool":
        fail("implementation_deps option did not return bool")
    if root != child: fail("root and subrule implementation_deps options diverged")
    return [DefaultInfo(), ImplementationDepsInfo(root = root, sub = child, marker = "{marker}")]
subject = rule(implementation = _root, fragments = ["cpp"], subrules = [sub])
"#
        ),
    );
    epoch.package(
        "subject",
        "load('//tools/build_defs/cc:implementation_deps.bzl', 'subject')\nsubject(name='implementation_deps')\n",
    );
    epoch.build()
}

fn configured(value: bool) -> ConfigurationKey {
    let base = configuration(None).slug_configuration().unwrap().clone();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::ExperimentalCcImplementationDeps,
        Some(if value { "true" } else { "false" }),
        false,
    )]
    .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

async fn request(
    dice: &Arc<Dice>,
    tracker: &Arc<Tracker>,
    route: AnalysisRoute,
    marker: &str,
    configuration: ConfigurationKey,
) -> Arc<slug_analysis_v2::ConfiguredNodeResult> {
    analyze_result(
        dice,
        epoch(marker),
        TARGET,
        configuration,
        tracker.clone(),
        route,
    )
    .await
    .unwrap()
}

fn assert_projection(result: &slug_analysis_v2::ConfiguredNodeResult, value: bool, marker: &str) {
    assert!(result.actions().is_empty());
    let returned = result
        .providers()
        .occurrences()
        .find(|provider| provider.field("marker").is_some())
        .unwrap();
    for field in ["root", "sub"] {
        assert!(
            matches!(returned.field(field).unwrap().kind(), AnalysisValueKind::Boolean(actual) if actual == value)
        );
    }
    assert!(
        matches!(returned.field("marker").unwrap().kind(), AnalysisValueKind::String(actual) if actual == marker)
    );
}

#[tokio::test]
async fn root_subrule_implementation_deps_preserves_target_exec_and_restores_configuration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let default = request(&dice, &tracker, route, "A", configuration(None)).await;
        let disabled = request(&dice, &tracker, route, "A", configured(false)).await;
        assert_projection(&default, true, "A");
        assert_projection(&disabled, false, "A");
        assert_ne!(default, disabled);

        // Configuration A/B/A and repeat restoration share one DICE instance.
        for (configuration, value) in [
            (configured(true), true),
            (configured(false), false),
            (configuration(None), true),
        ] {
            let result = request(&dice, &tracker, route, "A", configuration).await;
            assert_projection(&result, value, "A");
            assert_eq!(
                result,
                if value {
                    default.clone()
                } else {
                    disabled.clone()
                }
            );
        }

        for value in [true, false] {
            let target_configuration = configured(value);
            let target = target_configuration.slug_configuration().unwrap();
            let platform = target.host_platform_label().unwrap();
            let selected_exec = target.to_exec_for_platform(&platform).unwrap();
            assert_eq!(
                selected_exec.to_exec_for_platform(&platform).unwrap(),
                selected_exec
            );
            for execution in [target.to_exec().unwrap(), selected_exec] {
                let result = request(
                    &dice,
                    &tracker,
                    route,
                    "A",
                    ConfigurationKey::from_slug(execution),
                )
                .await;
                assert_projection(&result, value, "A");
            }
            let restored = request(&dice, &tracker, route, "A", target_configuration).await;
            assert_eq!(
                restored,
                if value {
                    default.clone()
                } else {
                    disabled.clone()
                }
            );
        }
    }
}

#[tokio::test]
async fn root_subrule_implementation_deps_restores_source_in_one_dice() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let configuration = configured(false);
        let first = request(&dice, &tracker, route, "A", configuration.clone()).await;
        let changed = request(&dice, &tracker, route, "B", configuration.clone()).await;
        assert_projection(&first, false, "A");
        assert_projection(&changed, false, "B");
        assert_ne!(changed, first);
        let restored = request(&dice, &tracker, route, "A", configuration).await;
        assert_eq!(restored, first);
    }
}
