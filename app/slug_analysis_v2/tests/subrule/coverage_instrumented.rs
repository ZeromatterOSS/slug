//! Pinned disabled coverage method on real configured rule contexts.
use super::*;

const TARGET: &str = "@@//subject:coverage_instrumented";

fn epoch(marker: &str) -> PathObservationEpoch {
    let mut epoch = Epoch::fixture();
    epoch.package("rules", "");
    epoch.file(
        "/workspace/rules/coverage_instrumented.bzl",
        &format!(
            r#"CoverageInfo = provider(fields = ["values", "marker"])
def _dependency(ctx): return [DefaultInfo()]
dependency = rule(implementation = _dependency)
ExecCoverageInfo = provider(fields = ["instrumented"])
def _exec_dependency(ctx):
    return [DefaultInfo(), ExecCoverageInfo(instrumented = ctx.coverage_instrumented())]
exec_dependency = rule(implementation = _exec_dependency)
def _explicit(ctx):
    if type(ctx.attr.dep) != "Target" or ctx.attr.dep[ExecCoverageInfo].instrumented != False:
        fail("coverage fixture did not materialize an exec-disabled Target")
    value = ctx.coverage_instrumented(ctx.attr.dep)
    return [DefaultInfo(), CoverageInfo(values = [value], marker = "{marker}")]
explicit = rule(implementation = _explicit, attrs = {{
    "dep": attr.label(default = "//subject:exec_dep", cfg = "exec"),
}})
def _root(ctx):
    if type(ctx.attr.dep) != "Target" or type(ctx.attr.source) != "Target":
        fail("coverage fixture did not materialize genuine Targets")
    method = ctx.coverage_instrumented
    values = [
        method(), method(None), method(target = None),
        method(ctx.attr.dep), method(target = ctx.attr.dep),
        method(ctx.attr.source), method(target = ctx.attr.source),
    ]
    for value in values:
        if type(value) != "bool" or value != False:
            fail("disabled coverage did not return primitive False")
    return [DefaultInfo(), CoverageInfo(values = values, marker = "{marker}")]
subject = rule(implementation = _root, attrs = {{
    "dep": attr.label(default = "//subject:dep"),
    "source": attr.label(default = "//subject:source.txt", allow_single_file = True),
}})
def _invalid(ctx):
    mode = ctx.attr.mode
    if mode == "arity":
        ctx.coverage_instrumented(None, None)
    elif mode == "named":
        ctx.coverage_instrumented(unknown = None)
    elif mode == "duplicate":
        ctx.coverage_instrumented(None, target = None)
    else:
        values = {{
            "int": 1, "bool": False, "string": "//subject:dep",
            "list": [], "struct": struct(), "file": ctx.file.source,
            "label": ctx.attr.source.label,
        }}
        ctx.coverage_instrumented(values[mode])
    fail("invalid coverage call was accepted")
invalid = rule(implementation = _invalid, attrs = {{
    "mode": attr.string(),
    "source": attr.label(default = "//subject:source.txt", allow_single_file = True),
}})
"#
        ),
    );
    let mut build = String::from(
        "load('//rules:coverage_instrumented.bzl', 'dependency', 'exec_dependency', 'explicit', 'subject', 'invalid')\nexports_files(['source.txt'])\ndependency(name='dep')\nexec_dependency(name='exec_dep')\nexplicit(name='explicit')\nsubject(name='coverage_instrumented')\n",
    );
    for mode in [
        "int",
        "bool",
        "string",
        "list",
        "struct",
        "file",
        "label",
        "arity",
        "named",
        "duplicate",
    ] {
        build.push_str(&format!("invalid(name='bad_{mode}', mode='{mode}')\n"));
    }
    epoch.package("subject", &build);
    epoch.file("/workspace/subject/source.txt", "inert source\n");
    epoch.build()
}

async fn request(
    dice: &Arc<Dice>,
    tracker: &Arc<Tracker>,
    route: AnalysisRoute,
    marker: &str,
    configuration: ConfigurationKey,
) -> Result<Arc<slug_analysis_v2::ConfiguredNodeResult>, String> {
    analyze_result(
        dice,
        epoch(marker),
        TARGET,
        configuration,
        tracker.clone(),
        route,
    )
    .await
}

fn assert_disabled(result: &slug_analysis_v2::ConfiguredNodeResult, marker: &str) {
    assert_disabled_count(result, marker, 7);
}

fn assert_disabled_count(
    result: &slug_analysis_v2::ConfiguredNodeResult,
    marker: &str,
    count: usize,
) {
    assert!(result.actions().is_empty());
    let returned = result
        .providers()
        .occurrences()
        .find(|provider| provider.field("marker").is_some())
        .unwrap();
    let AnalysisValueKind::List(values) = returned.field("values").unwrap().kind() else {
        panic!("coverage results must be a list");
    };
    assert_eq!(values.len(), count);
    for value in values {
        assert!(matches!(value.kind(), AnalysisValueKind::Boolean(false)));
    }
    assert!(
        matches!(returned.field("marker").unwrap().kind(), AnalysisValueKind::String(actual) if actual == marker)
    );
}

fn assert_enabled_refusal(error: &str) {
    assert!(error.contains("coverage_instrumented"), "{error}");
    assert!(error.contains("unsupported/deferred"), "{error}");
}

#[tokio::test]
async fn coverage_instrumented_disabled_target_and_exec_restore_configuration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let default = request(&dice, &tracker, route, "A", configuration(None))
            .await
            .unwrap();
        assert_disabled(&default, "A");
        let disabled = coverage_configuration(false, None);
        let first = request(&dice, &tracker, route, "A", disabled.clone())
            .await
            .unwrap();
        assert_disabled(&first, "A");
        let enabled = coverage_configuration(true, None);
        assert_enabled_refusal(
            &request(&dice, &tracker, route, "A", enabled.clone())
                .await
                .unwrap_err(),
        );
        assert_eq!(
            request(&dice, &tracker, route, "A", disabled)
                .await
                .unwrap(),
            first
        );

        // A supplied exec-disabled Target cannot override its enabled caller.
        for caller_enabled in [false, true, false] {
            let explicit = analyze_result(
                &dice,
                epoch("A"),
                "@@//subject:explicit",
                coverage_configuration(caller_enabled, None),
                tracker.clone(),
                route,
            )
            .await;
            if caller_enabled {
                assert_enabled_refusal(&explicit.unwrap_err());
            } else {
                assert_disabled_count(&explicit.unwrap(), "A", 1);
            }
        }

        let target = enabled.slug_configuration().unwrap();
        for execution in [
            target.to_exec().unwrap(),
            target
                .to_exec_for_platform(&target.host_platform_label().unwrap())
                .unwrap(),
        ] {
            assert!(!execution.coverage_enabled().unwrap());
            let result = request(
                &dice,
                &tracker,
                route,
                "A",
                ConfigurationKey::from_slug(execution),
            )
            .await
            .unwrap();
            assert_disabled(&result, "A");
            assert_enabled_refusal(
                &request(&dice, &tracker, route, "A", enabled.clone())
                    .await
                    .unwrap_err(),
            );
        }
    }
}

#[tokio::test]
async fn coverage_instrumented_disabled_restores_source_in_one_dice() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let configuration = coverage_configuration(false, None);
        let first = request(&dice, &tracker, route, "A", configuration.clone())
            .await
            .unwrap();
        let changed = request(&dice, &tracker, route, "B", configuration.clone())
            .await
            .unwrap();
        assert_disabled(&first, "A");
        assert_disabled(&changed, "B");
        assert_ne!(changed, first);
        assert_eq!(
            request(&dice, &tracker, route, "A", configuration)
                .await
                .unwrap(),
            first
        );
    }
}

#[tokio::test]
async fn coverage_instrumented_validates_target_types_and_binding_before_configuration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        for enabled in [false, true] {
            for mode in [
                "int",
                "bool",
                "string",
                "list",
                "struct",
                "file",
                "label",
                "arity",
                "named",
                "duplicate",
            ] {
                let error = analyze(
                    &dice,
                    epoch("A"),
                    &format!("@@//subject:bad_{mode}"),
                    coverage_configuration(enabled, None),
                    tracker.clone(),
                    route,
                )
                .await;
                if matches!(mode, "arity" | "named" | "duplicate") {
                    assert!(error.contains("coverage_instrumented"), "{mode}: {error}");
                    assert!(!error.contains("unsupported/deferred"), "{mode}: {error}");
                    assert!(
                        !error.contains("invalid coverage call was accepted"),
                        "{mode}: {error}"
                    );
                } else {
                    assert!(
                        error.contains("target must be a Target or None"),
                        "{mode}: {error}"
                    );
                }
            }
        }
    }
}
