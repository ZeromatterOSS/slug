//! Coverage-disabled native EMPTY through real configured root contexts.
use super::*;

const TARGET: &str = "@@//subject:coverage_common";
const SOURCE: &str = r#"Proof = provider(fields = ["marker"])
def _dependency(ctx):
    return [DefaultInfo(), coverage_common.instrumented_files_info(ctx)]
dependency = rule(implementation = _dependency)
def _root(ctx):
    info = coverage_common.instrumented_files_info(ctx)
    generated = ctx.actions.declare_file("unused-metadata.gcno")
    explicit = coverage_common.instrumented_files_info(
        ctx = ctx,
        source_attributes = ("nonexistent_source",),
        dependency_attributes = ["nonexistent_dependency"],
        extensions = None,
        metadata_files = [ctx.file.source, generated],
        baseline_coverage_files = (generated, ctx.file.source),
        coverage_support_files = depset(order = "postorder"),
        coverage_environment = {},
        reported_to_actual_sources = depset(order = "preorder"),
    )
    ranged = coverage_common.instrumented_files_info(
        ctx, source_attributes = range(0), dependency_attributes = range(0),
        extensions = range(0), metadata_files = range(0),
        coverage_support_files = range(0), baseline_coverage_files = range(0),
    )
    private = coverage_common.instrumented_files_info(
        ctx, coverage_support_files = [ctx.file.source, depset([generated])],
        coverage_environment = {"COVERAGE": "ignored"},
        reported_to_actual_sources = depset([(1,)]),
    )
    if type(info) != "InstrumentedFilesInfo": fail("native EMPTY type was lost")
    for value in [info, explicit, ranged, private]:
        keyed = {info: 1}
        keyed[value] = 2
        if value != info or len(keyed) != 1 or keyed[info] != 2:
            fail("native EMPTY equality/hash was lost")
        if type(value.instrumented_files) != "depset" or value.instrumented_files.to_list() != []:
            fail("instrumented_files must be empty")
        if type(value.metadata_files) != "depset" or value.metadata_files.to_list() != []:
            fail("metadata_files must be empty")
    if type(ctx.attr.dep) != "Target" or InstrumentedFilesInfo not in ctx.attr.dep:
        fail("dependency must rematerialize native provider key")
    dependency_info = ctx.attr.dep[InstrumentedFilesInfo]
    if dependency_info != info or type(dependency_info) != "InstrumentedFilesInfo":
        fail("dependency must rematerialize native EMPTY")
    keyed = {info: 1}
    keyed[dependency_info] = 2
    if len(keyed) != 1 or keyed[info] != 2:
        fail("rematerialized EMPTY hash was lost")
    return [DefaultInfo(), info, Proof(marker = "__MARKER__")]
subject = rule(implementation = _root, attrs = {
    "source": attr.label(default = "//subject:source.txt", allow_single_file = True),
    "dep": attr.label(default = "//subject:dep", cfg = "exec"),
})
def _sub(ctx): return coverage_common.instrumented_files_info(ctx)
sub = subrule(implementation = _sub)
def _invalid(ctx):
    mode = ctx.attr.mode
    if mode == "empty_support":
        info = coverage_common.instrumented_files_info(ctx, coverage_support_files = [depset()])
        if info != coverage_common.instrumented_files_info(ctx): fail("nested empty support changed EMPTY")
        return [DefaultInfo(), info, Proof(marker = "__MARKER__")]
    if mode == "missing": coverage_common.instrumented_files_info()
    elif mode == "unknown": coverage_common.instrumented_files_info(ctx, unknown = [])
    elif mode == "duplicate": coverage_common.instrumented_files_info(ctx, ctx = ctx)
    elif mode == "positional": coverage_common.instrumented_files_info(ctx, [])
    elif mode == "none_ctx": coverage_common.instrumented_files_info(None)
    elif mode == "fake_ctx": coverage_common.instrumented_files_info(struct())
    elif mode == "subrule": sub()
    elif mode == "private_support":
        coverage_common.instrumented_files_info(ctx, coverage_support_files = depset([ctx.file.source]))
    elif mode == "private_reported":
        coverage_common.instrumented_files_info(ctx, reported_to_actual_sources = depset([(1,)]))
    elif mode == "private_environment":
        coverage_common.instrumented_files_info(ctx, coverage_environment = {"A": "B"})
    elif mode == "private_before_metadata":
        coverage_common.instrumented_files_info(ctx, coverage_environment = {"A": "B"}, metadata_files = [1])
    elif mode == "extensions_before_private":
        coverage_common.instrumented_files_info(ctx, extensions = [1], coverage_environment = {"A": "B"})
    else:
        if mode == "coverage_support_files" and type(ctx.attr.dep[DefaultInfo].files_to_run) != "FilesToRunProvider":
            fail("support refusal requires a genuine FilesToRunProvider")
        invalid = {
            "source_attributes": [1], "source_none": None, "dependency_attributes": "deps",
            "extensions": [1], "metadata_files": [1],
            "baseline_coverage_files": [1], "coverage_environment": {"A": 1},
            "coverage_support_files": [ctx.attr.dep[DefaultInfo].files_to_run],
            "reported_to_actual_sources": depset([1]),
            "support_nested": [depset([1])],
        }
        field = "coverage_support_files" if mode == "support_nested" else "source_attributes" if mode == "source_none" else mode
        coverage_common.instrumented_files_info(ctx, **{field: invalid[mode]})
    fail("invalid coverage call was accepted")
invalid = rule(implementation = _invalid, subrules = [sub], attrs = {
    "mode": attr.string(),
    "source": attr.label(default = "//subject:source.txt", allow_single_file = True),
    "dep": attr.label(default = "//subject:dep", cfg = "exec"),
})
"#;

const INVALID_CASES: [(&str, &str); 22] = [
    ("missing", "ctx"),
    ("unknown", "unknown"),
    ("duplicate", "ctx"),
    ("positional", "positional"),
    ("none_ctx", "ctx"),
    ("fake_ctx", "ctx"),
    ("subrule", "ctx"),
    ("source_attributes", "source_attributes"),
    ("source_none", "source_attributes"),
    ("dependency_attributes", "dependency_attributes"),
    ("extensions", "extensions"),
    ("metadata_files", "metadata_files"),
    ("baseline_coverage_files", "baseline_coverage_files"),
    ("coverage_environment", "coverage_environment"),
    ("coverage_support_files", "coverage_support_files"),
    ("reported_to_actual_sources", "reported_to_actual_sources"),
    ("support_nested", "coverage_support_files"),
    ("private_support", "private API"),
    ("private_reported", "private API"),
    ("private_environment", "private API"),
    ("private_before_metadata", "private API"),
    ("extensions_before_private", "extensions"),
];

fn epoch(marker: &str) -> PathObservationEpoch {
    let mut epoch = Epoch::fixture();
    epoch.directory("/workspace/tools");
    epoch.directory("/workspace/tools/build_defs");
    for package in ["tools/build_defs/cc", "ordinary"] {
        epoch.package(package, "");
        epoch.file(
            &format!("/workspace/{package}/coverage_common.bzl"),
            &SOURCE.replace("__MARKER__", marker),
        );
    }
    let mut build = String::from(
        "load('//tools/build_defs/cc:coverage_common.bzl', 'subject', 'dependency')\nload('//ordinary:coverage_common.bzl', 'invalid')\nexports_files(['source.txt'])\ndependency(name='dep')\nsubject(name='coverage_common')\ninvalid(name='empty_support', mode='empty_support')\n",
    );
    for (mode, _) in INVALID_CASES {
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

fn assert_empty(result: &slug_analysis_v2::ConfiguredNodeResult, marker: &str) {
    assert!(
        result.actions().is_empty(),
        "declaring unused metadata registers no action"
    );
    let info = result
        .providers()
        .occurrences()
        .find(|info| info.identity().is_builtin("InstrumentedFilesInfo"))
        .unwrap();
    for name in ["instrumented_files", "metadata_files"] {
        let AnalysisValueKind::Depset(files) = info.field(name).unwrap().kind() else {
            panic!("native EMPTY fields must be depsets");
        };
        assert!(files.is_empty());
        assert_eq!(files.order(), slug_build_api_v2::DepsetOrder::Default);
    }
    let proof = result
        .providers()
        .occurrences()
        .find(|info| info.field("marker").is_some())
        .unwrap();
    assert!(
        matches!(proof.field("marker").unwrap().kind(), AnalysisValueKind::String(value) if value == marker)
    );
}

#[tokio::test]
async fn disabled_instrumented_files_info_preserves_native_empty_and_dice_restoration() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let first = request(&dice, &tracker, route, "A", configuration(None))
            .await
            .unwrap();
        assert_empty(&first, "A");
        let disabled = coverage_configuration(false, None);
        assert_eq!(
            request(&dice, &tracker, route, "A", disabled.clone())
                .await
                .unwrap(),
            first
        );
        let enabled = coverage_configuration(true, None);
        let error = request(&dice, &tracker, route, "A", enabled.clone())
            .await
            .unwrap_err();
        assert!(
            error.contains("coverage enabled") && error.contains("unsupported/deferred"),
            "{error}"
        );
        assert_eq!(
            request(&dice, &tracker, route, "A", disabled.clone())
                .await
                .unwrap(),
            first
        );
        let target = enabled.slug_configuration().unwrap();
        for exec in [
            target.to_exec().unwrap(),
            target
                .to_exec_for_platform(&target.host_platform_label().unwrap())
                .unwrap(),
        ] {
            let result = request(
                &dice,
                &tracker,
                route,
                "A",
                ConfigurationKey::from_slug(exec),
            )
            .await
            .unwrap();
            assert_empty(&result, "A");
        }
        assert_eq!(
            request(&dice, &tracker, route, "A", disabled.clone())
                .await
                .unwrap(),
            first
        );
        let changed = request(&dice, &tracker, route, "B", disabled.clone())
            .await
            .unwrap();
        assert_empty(&changed, "B");
        assert_ne!(first, changed);
        assert_eq!(
            request(&dice, &tracker, route, "A", disabled)
                .await
                .unwrap(),
            first
        );
    }
}

#[tokio::test]
async fn disabled_instrumented_files_info_validates_binding_fields_and_private_extras() {
    for route in [AnalysisRoute::Legacy, AnalysisRoute::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let tracker = Arc::new(Tracker::default());
        let nested_empty = analyze_result(
            &dice,
            epoch("A"),
            "@@//subject:empty_support",
            coverage_configuration(false, None),
            tracker.clone(),
            route,
        )
        .await
        .unwrap();
        assert_empty(&nested_empty, "A");
        for (mode, diagnostic) in INVALID_CASES {
            let error = analyze_result(
                &dice,
                epoch("A"),
                &format!("@@//subject:bad_{mode}"),
                coverage_configuration(false, None),
                tracker.clone(),
                route,
            )
            .await
            .unwrap_err();
            assert!(error.contains(diagnostic), "{mode}: {error}");
            assert!(
                !error.contains("invalid coverage call was accepted"),
                "{error}"
            );
        }
    }
}
