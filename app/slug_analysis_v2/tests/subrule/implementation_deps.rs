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
            r#"ImplementationDepsInfo = provider(fields = ["root", "sub", "root_start_end", "sub_start_end", "root_specific", "sub_specific", "marker"])
def _sub(ctx):
    return ctx.fragments.cpp.experimental_cc_implementation_deps(), ctx.fragments.cpp.start_end_lib(), ctx.fragments.cpp.incompatible_use_specific_tool_files()
sub = subrule(implementation = _sub, fragments = ["cpp"])
def _root(ctx):
    root = ctx.fragments.cpp.experimental_cc_implementation_deps()
    start_end = ctx.fragments.cpp.start_end_lib()
    specific = ctx.fragments.cpp.incompatible_use_specific_tool_files()
    child, child_start_end, child_specific = sub()
    if type(root) != "bool" or type(child) != "bool":
        fail("implementation_deps option did not return bool")
    if root != child: fail("root and subrule implementation_deps options diverged")
    if type(start_end) != "bool" or type(child_start_end) != "bool":
        fail("start_end_lib option did not return bool")
    if start_end != child_start_end: fail("root and subrule start_end_lib options diverged")
    if type(specific) != "bool" or type(child_specific) != "bool":
        fail("incompatible_use_specific_tool_files option did not return bool")
    if specific != child_specific: fail("root and subrule specific tool files options diverged")
    return [DefaultInfo(), ImplementationDepsInfo(root = root, sub = child, root_start_end = start_end, sub_start_end = child_start_end, root_specific = specific, sub_specific = child_specific, marker = "{marker}")]
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

fn configured_options(implementation_deps: bool, start_end: bool) -> ConfigurationKey {
    let base = configured(implementation_deps)
        .slug_configuration()
        .unwrap()
        .clone();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::StartEndLib,
        Some(if start_end { "true" } else { "false" }),
        false,
    )]
    .into();
    ConfigurationKey::from_slug(
        base.with_command_configuration(base.starlark_options().clone(), &overlay)
            .unwrap(),
    )
}

fn configured_policies(
    implementation_deps: bool,
    start_end: bool,
    specific: bool,
) -> ConfigurationKey {
    let base = configured_options(implementation_deps, start_end)
        .slug_configuration()
        .unwrap()
        .clone();
    let overlay: CommandConfigurationOverlay = vec![CommandConfigurationOccurrence::native(
        NativeCommandOption::IncompatibleUseSpecificToolFiles,
        Some(if specific { "true" } else { "false" }),
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

fn assert_projection(
    result: &slug_analysis_v2::ConfiguredNodeResult,
    value: bool,
    start_end: bool,
    specific: bool,
    marker: &str,
) {
    assert!(result.actions().is_empty());
    let returned = result
        .providers()
        .occurrences()
        .find(|provider| provider.field("marker").is_some())
        .unwrap();
    for (field, expected) in [
        ("root", value),
        ("sub", value),
        ("root_start_end", start_end),
        ("sub_start_end", start_end),
        ("root_specific", specific),
        ("sub_specific", specific),
    ] {
        assert!(
            matches!(returned.field(field).unwrap().kind(), AnalysisValueKind::Boolean(actual) if actual == expected)
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
        assert_projection(&default, true, true, true, "A");
        assert_projection(&disabled, false, true, true, "A");
        assert_ne!(default, disabled);

        // Configuration A/B/A and repeat restoration share one DICE instance.
        for (configuration, value) in [
            (configured(true), true),
            (configured(false), false),
            (configuration(None), true),
        ] {
            let result = request(&dice, &tracker, route, "A", configuration).await;
            assert_projection(&result, value, true, true, "A");
            assert_eq!(
                result,
                if value {
                    default.clone()
                } else {
                    disabled.clone()
                }
            );
        }

        for (value, start_end, specific) in [
            (true, true, true),
            (false, true, true),
            (true, false, true),
            (false, false, true),
            (true, true, false),
            (false, true, false),
            (true, false, false),
            (false, false, false),
        ] {
            let target_configuration = configured_policies(value, start_end, specific);
            let expected = request(&dice, &tracker, route, "A", target_configuration.clone()).await;
            assert_projection(&expected, value, start_end, specific, "A");
            if start_end && specific {
                assert_eq!(
                    expected,
                    if value {
                        default.clone()
                    } else {
                        disabled.clone()
                    }
                );
            } else {
                assert_ne!(
                    expected,
                    if value {
                        default.clone()
                    } else {
                        disabled.clone()
                    }
                );
            }
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
                assert_projection(&result, value, start_end, specific, "A");
            }
            let restored = request(&dice, &tracker, route, "A", target_configuration).await;
            assert_eq!(restored, expected);
        }
        assert_eq!(
            request(&dice, &tracker, route, "A", configuration(None)).await,
            default
        );
    }
}

#[tokio::test]
async fn root_subrule_implementation_deps_restores_source_in_one_dice() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let configuration = configured_policies(false, false, false);
        let first = request(&dice, &tracker, route, "A", configuration.clone()).await;
        let changed = request(&dice, &tracker, route, "B", configuration.clone()).await;
        assert_projection(&first, false, false, false, "A");
        assert_projection(&changed, false, false, false, "B");
        assert_ne!(changed, first);
        let restored = request(&dice, &tracker, route, "A", configuration).await;
        assert_eq!(restored, first);
    }
}
