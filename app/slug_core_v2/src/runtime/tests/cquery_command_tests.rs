use super::*;

struct CqueryExecutablesFixture {
    _workspace: tempfile::TempDir,
    command_policy: BzlmodCommandPolicyKey,
    runtime: WorkspaceRuntime,
}

impl CqueryExecutablesFixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let command_policy = configured_test_command_policy(workspace.path());
        fs::write(
            workspace.path().join("MODULE.bazel"),
            configured_test_module("module(name = \"executable_deps\")\n"),
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            format!(
                r##"{CQUERY_DELEGATING_DEFS}
def _executable(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(out, "#!/bin/sh\n")
    return [DefaultInfo(executable = out)]
executable_rule = rule(implementation = _executable, executable = True, attrs = {{
    "normal": attr.label(),
    "transitioned": attr.label(cfg = to_transition),
    "bridge": attr.label(),
}})
"##
            ),
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            r#"load(":defs.bzl", "executable_rule", "ordinary_rule", "string_setting")
string_setting(name = "setting", build_setting_default = "default")
executable_rule(name = "direct")
executable_rule(name = "leaf")
ordinary_rule(name = "bridge", normal = ":leaf")
executable_rule(
    name = "root",
    normal = ":direct",
    transitioned = ":direct",
    bridge = ":bridge",
)
"#,
        )
        .unwrap();
        let runtime = test_runtime(workspace.path()).unwrap();
        Self {
            _workspace: workspace,
            command_policy,
            runtime,
        }
    }

    fn run(
        &self,
        expression: &str,
    ) -> Result<
        AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
        CqueryCommandError,
    > {
        self.runtime.cquery_command_with_bzlmod_inputs(
            expression,
            false,
            true,
            self.command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
    }
}

#[test]
fn cquery_executables_depth_and_complete_closure() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();

    let depth_zero = run("executables(deps(//:root, 0))");
    let depth_zero = depth_zero.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(depth_zero.starlark_label_stdout(), "@@//:root\n");

    let depth_one = run("executables(deps(//:root, 1))");
    let depth_one = depth_one.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(
        depth_one.starlark_label_stdout(),
        "@@//:root\n@@//:direct\n@@//:direct\n"
    );

    let full = run("executables(deps(//:root))");
    let full = full.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(
        full.starlark_label_stdout(),
        "@@//:root\n@@//:direct\n@@//:direct\n@@//:leaf\n"
    );
    assert_eq!(full.analyses().count(), 4);
    assert_eq!(full.label_kind_stdout().unwrap().lines().count(), 4);
    assert!(
        full.label_kind_stdout()
            .unwrap()
            .lines()
            .all(|line| line.starts_with("executable_rule rule "))
    );
    let graph = full.graph_stdout();
    let nodes = graph
        .lines()
        .filter(|line| line.starts_with("  \"") && !line.contains(" -> "))
        .count();
    let edges = graph
        .lines()
        .filter(|line| line.contains(" -> "))
        .collect::<Vec<_>>();
    assert_eq!(nodes, 4);
    assert_eq!(edges.len(), 2);
    assert!(
        edges
            .iter()
            .all(|edge| edge.contains("//:root") && edge.contains("//:direct"))
    );
    assert!(edges.iter().all(|edge| !edge.contains("//:leaf")));
}

#[rustfmt::skip] #[test] fn cquery_executables_reverse_self_zero_and_empty() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let reverse_self = run("executables(rdeps(//:root, //:root))");
    let reverse_self = reverse_self.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(reverse_self.starlark_label_stdout(), "@@//:root\n");
    let reverse_zero = run("executables(rdeps(//:root, //:direct, 0))");
    let reverse_zero = reverse_zero.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(reverse_zero.starlark_label_stdout(), "@@//:direct\n@@//:direct\n");
    let reverse_empty = run("executables(rdeps(//:root, //:bridge, 0))");
    let reverse_empty = reverse_empty.terminal_for_test().as_ref().as_ref().unwrap();
    assert!(reverse_empty.label_stdout().is_empty());
    assert_eq!(reverse_empty.graph_stdout(), "digraph mygraph {\n  node [shape=box];\n}\n");
}

#[rustfmt::skip] #[test] fn cquery_executables_reverse_full_identity_and_upper_bounds() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let reverse_full = run("executables(rdeps(//:root, //:direct))");
    let reverse_full = reverse_full.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(reverse_full.starlark_label_stdout(), "@@//:direct\n@@//:direct\n@@//:root\n");
    let reverse_keys = reverse_full.analyses().map(|analysis| analysis.key().clone()).collect::<Vec<_>>();
    assert_eq!(reverse_keys.len(), 3);
    assert_ne!(reverse_keys[0], reverse_keys[1]);
    let reverse_graph = reverse_full.graph_stdout();
    assert_eq!(reverse_graph.lines().filter(|line| line.contains(" -> ")).count(), 2);
    assert!(reverse_graph.lines().filter(|line| line.contains(" -> ")).all(|line| line.contains("//:root") && line.contains("//:direct")));
    for depth in ["1", "2147483647"] {
        let bounded = run(&format!("executables(rdeps(//:root, //:direct, {depth}))"));
        let bounded = bounded.terminal_for_test().as_ref().as_ref().unwrap();
        assert_eq!(bounded.graph_stdout(), reverse_full.graph_stdout(), "depth {depth}");
    }
}

#[rustfmt::skip] #[test] fn cquery_executables_reverse_negative_and_zero_bounds() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let reverse_zero = run("executables(rdeps(//:root, //:direct, 0))");
    let reverse_zero = reverse_zero.terminal_for_test().as_ref().as_ref().unwrap();
    let negative = run("executables(rdeps(//:root, //:direct, '-1'))");
    let negative = negative.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(negative.graph_stdout(), "digraph mygraph {\n  node [shape=box];\n}\n");
    let bounded_zero = run("executables(rdeps(//:root, //:direct, 0))");
    let bounded_zero = bounded_zero.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(bounded_zero.graph_stdout(), reverse_zero.graph_stdout());
}

#[test]
fn cquery_executables_chained_and_named_kind_composition() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let full = run("executables(deps(//:root))");
    let full = full.terminal_for_test().as_ref().as_ref().unwrap();
    let chained_full = run("filter(':(root|direct|leaf)$', executables(deps(//:root)))");
    let chained_full = chained_full.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(chained_full.label_stdout(), full.label_stdout());
    assert_eq!(
        chained_full.starlark_label_stdout(),
        full.starlark_label_stdout()
    );
    assert_eq!(
        chained_full.label_kind_stdout().unwrap(),
        full.label_kind_stdout().unwrap()
    );
    assert_eq!(chained_full.graph_stdout(), full.graph_stdout());

    let named_kind_full =
        run("filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root)))");
    let named_kind_full = named_kind_full
        .terminal_for_test()
        .as_ref()
        .as_ref()
        .unwrap();
    assert_eq!(named_kind_full.label_stdout(), full.label_stdout());
    assert_eq!(
        named_kind_full.starlark_label_stdout(),
        full.starlark_label_stdout()
    );
    assert_eq!(
        named_kind_full.label_kind_stdout().unwrap(),
        full.label_kind_stdout().unwrap()
    );
    assert_eq!(named_kind_full.graph_stdout(), full.graph_stdout());
}

#[test]
fn cquery_executables_upper_depths_match_full() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let full = run("executables(deps(//:root))");
    let full = full.terminal_for_test().as_ref().as_ref().unwrap();
    let depth_two = run("executables(deps(//:root, 2))");
    let depth_max = run("executables(deps(//:root, 2147483647))");
    assert_eq!(
        depth_two
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .graph_stdout(),
        full.graph_stdout()
    );
    assert_eq!(
        depth_max
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .label_stdout(),
        full.label_stdout()
    );
}

#[test]
fn cquery_executables_direct_filter_and_kind_match_full() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let full = run("executables(deps(//:root))");
    let full = full.terminal_for_test().as_ref().as_ref().unwrap();
    let filtered = run("filter(':(root|direct|leaf)$', deps(//:root))");
    let filtered = filtered.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(filtered.label_stdout(), full.label_stdout());
    assert_eq!(filtered.starlark_label_stdout(), full.starlark_label_stdout());
    assert_eq!(
        filtered.label_kind_stdout().unwrap(),
        full.label_kind_stdout().unwrap()
    );
    assert_eq!(filtered.graph_stdout(), full.graph_stdout());
    let kind = run("kind('^executable_rule rule$', deps(//:root))");
    let kind = kind.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(kind.label_stdout(), full.label_stdout());
    assert_eq!(kind.starlark_label_stdout(), full.starlark_label_stdout());
    assert_eq!(
        kind.label_kind_stdout().unwrap(),
        full.label_kind_stdout().unwrap()
    );
    assert_eq!(kind.graph_stdout(), full.graph_stdout());
}

#[rustfmt::skip] macro_rules! cquery_executable_projection_depth_test {
    ($name:ident, $baseline:literal, $projection:literal, $depth:literal) => {
        #[test]
        fn $name() {
            let fixture = CqueryExecutablesFixture::new();
            let expected = fixture.run($baseline).unwrap();
            let expected = expected.terminal_for_test().as_ref().as_ref().unwrap();
            let projected = fixture.run($projection).unwrap();
            let projected = projected.terminal_for_test().as_ref().as_ref().unwrap();
            assert_eq!(projected.graph_stdout(), expected.graph_stdout(), "depth {}", $depth);
        }
    };
}

#[rustfmt::skip] macro_rules! cquery_executable_projection_upper_test {
    ($name:ident, $depth_two:literal, $depth_max:literal) => {
        #[test]
        fn $name() {
            let fixture = CqueryExecutablesFixture::new();
            let full = fixture.run("executables(deps(//:root))").unwrap();
            let full = full.terminal_for_test().as_ref().as_ref().unwrap();
            for (depth, expression) in [(2, $depth_two), (i32::MAX, $depth_max)] {
                let projected = fixture.run(expression).unwrap();
                let projected = projected.terminal_for_test().as_ref().as_ref().unwrap();
                assert_eq!(projected.graph_stdout(), full.graph_stdout(), "depth {depth}");
            }
        }
    };
}

#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_filter_depth_zero, "executables(deps(//:root, 0))", "filter(':(root|direct|leaf)$', deps(//:root, 0))", 0);
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_filter_depth_one, "executables(deps(//:root, 1))", "filter(':(root|direct|leaf)$', deps(//:root, 1))", 1);
#[rustfmt::skip] cquery_executable_projection_upper_test!(cquery_executables_filter_upper_depths, "filter(':(root|direct|leaf)$', deps(//:root, 2))", "filter(':(root|direct|leaf)$', deps(//:root, 2147483647))");
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_kind_depth_zero, "executables(deps(//:root, 0))", "kind('^executable_rule rule$', deps(//:root, 0))", 0);
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_kind_depth_one, "executables(deps(//:root, 1))", "kind('^executable_rule rule$', deps(//:root, 1))", 1);
#[rustfmt::skip] cquery_executable_projection_upper_test!(cquery_executables_kind_upper_depths, "kind('^executable_rule rule$', deps(//:root, 2))", "kind('^executable_rule rule$', deps(//:root, 2147483647))");
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_chained_depth_zero, "executables(deps(//:root, 0))", "filter(':(root|direct|leaf)$', executables(deps(//:root, 0)))", 0);
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_chained_depth_one, "executables(deps(//:root, 1))", "filter(':(root|direct|leaf)$', executables(deps(//:root, 1)))", 1);
#[rustfmt::skip] cquery_executable_projection_upper_test!(cquery_executables_chained_upper_depths, "filter(':(root|direct|leaf)$', executables(deps(//:root, 2)))", "filter(':(root|direct|leaf)$', executables(deps(//:root, 2147483647)))");
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_named_kind_depth_zero, "executables(deps(//:root, 0))", "filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root, 0)))", 0);
#[rustfmt::skip] cquery_executable_projection_depth_test!(cquery_executables_named_kind_depth_one, "executables(deps(//:root, 1))", "filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root, 1)))", 1);
#[rustfmt::skip] cquery_executable_projection_upper_test!(cquery_executables_named_kind_upper_depths, "filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root, 2)))", "filter(':(root|direct|leaf)$', kind('^executable_rule rule$', deps(//:root, 2147483647)))");

#[test]
fn cquery_executables_empty_compositions() {
    let fixture = CqueryExecutablesFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let empty = run("filter('^//:missing$', deps(//:root))");
    let empty = empty.terminal_for_test().as_ref().as_ref().unwrap();
    assert!(empty.label_stdout().is_empty());
    assert_eq!(
        empty.graph_stdout(),
        "digraph mygraph {\n  node [shape=box];\n}\n"
    );
    let chained_empty = run("filter('^//:missing$', executables(deps(//:root)))");
    let chained_empty = chained_empty.terminal_for_test().as_ref().as_ref().unwrap();
    assert_eq!(chained_empty.label_stdout(), empty.label_stdout());
    assert_eq!(chained_empty.graph_stdout(), empty.graph_stdout());
    let named_kind_empty =
        run("filter('^//:missing$', kind('^executable_rule rule$', deps(//:root)))");
    let named_kind_empty = named_kind_empty
        .terminal_for_test()
        .as_ref()
        .as_ref()
        .unwrap();
    assert_eq!(named_kind_empty.label_stdout(), empty.label_stdout());
    assert_eq!(named_kind_empty.graph_stdout(), empty.graph_stdout());
}

#[tokio::test]
async fn cquery_deps_frontier_need_precedes_an_earlier_child_analysis_error() {
    let expression =
        QueryExpression::parse("filter('root$', kind('rule$', deps(//:root, 1)))").unwrap();
    let deps = expression
        .cquery_preactivation_deps_spec()
        .expect("chained closure spec");
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let configuration = build_test_configuration("target");
    let configured = |label: &str| {
        ConfiguredTargetKey::new(CanonicalLabel::parse(label).unwrap(), configuration.clone())
    };
    let mut transaction = build_root_transaction(&dice, delegating_action_closure_epoch(1)).await;
    let root_key = ConfiguredNodeAnalysisKey::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        configured("@@//:root"),
    )
    .unwrap();
    let slug_bzlmod_v2::SourcePreparationOutcome::Complete(root) =
        transaction.compute(&root_key).await.unwrap()
    else {
        panic!("observed root fixture returned Need")
    };
    let root = Arc::new(
        root.as_ref()
            .as_ref()
            .unwrap()
            .as_ref()
            .clone()
            .with_edges(vec![
                slug_analysis_v2::ConfiguredEdge::new(
                    configured("@@//:missing").into(),
                    slug_analysis_v2::ConfiguredEdgeKind::Attribute {
                        attribute: "error".into(),
                        index: 0,
                        hidden: false,
                        dependency: slug_analysis_v2::ConfiguredAttributeDependency::Target,
                    },
                ),
                slug_analysis_v2::ConfiguredEdge::new(
                    ConfiguredNodeKey::null(CanonicalLabel::parse("@@//:source.txt").unwrap()),
                    slug_analysis_v2::ConfiguredEdgeKind::Attribute {
                        attribute: "src".into(),
                        index: 0,
                        hidden: false,
                        dependency: slug_analysis_v2::ConfiguredAttributeDependency::Target,
                    },
                ),
            ]),
    );
    let mut missing_source_epoch = BuildRootEpoch::base(2);
    missing_source_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 2);
    missing_source_epoch.package("", DELEGATING_BUILD, 2);
    let mut transaction = build_root_transaction(&dice, missing_source_epoch.build()).await;
    let outcome = compute_cquery_deps_closure(
        &mut transaction,
        &NormalizedAbsolutePath::new("/workspace").unwrap(),
        root.dupe(),
        deps.depth(),
        true,
    )
    .await
    .unwrap();
    assert!(
        matches!(outcome, slug_bzlmod_v2::SourcePreparationOutcome::Need(_)),
        "{outcome:?}"
    );
    let mut restored_epoch = BuildRootEpoch::base(3);
    restored_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 3);
    restored_epoch.package(
        "",
        &format!("{DELEGATING_BUILD}\nroot_rule(name = \"missing\")\n"),
        3,
    );
    restored_epoch.file("/workspace/source.txt", "source\n", 3);
    let mut restored = build_root_transaction(&dice, restored_epoch.build()).await;
    let restored = compute_cquery_deps_closure(
        &mut restored,
        &NormalizedAbsolutePath::new("/workspace").unwrap(),
        root,
        deps.depth(),
        true,
    )
    .await
    .unwrap();
    assert!(matches!(
        restored,
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(Ok(_))
    ));
}

#[tokio::test]
async fn cquery_rdeps_universe_need_precedes_seed_validation() {
    let expression = QueryExpression::parse("rdeps(//:root, //:missing)").unwrap();
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let root = CqueryCommandRoot {
        expression,
        workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        base_configuration: build_test_configuration("target"),
        configuration_overlay: CommandConfigurationOverlay::default(),
        roots: Arc::from([CqueryRootTarget {
            requested: Arc::from("//:root"),
            canonical: CanonicalLabel::parse("@@//:root").unwrap(),
        }]),
        literal_roots: Arc::from([(Arc::from("//:root"), 0)]),
        include_implicit: false,
        include_tool: true,
    };
    let mut missing_source_epoch = BuildRootEpoch::base(2);
    missing_source_epoch.file("/workspace/defs.bzl", DELEGATING_DEFS, 2);
    missing_source_epoch.package("", DELEGATING_BUILD, 2);
    let mut transaction = build_root_transaction(&dice, missing_source_epoch.build()).await;
    let invalid_root = CqueryCommandRoot {
        expression: QueryExpression::parse("filter('(', rdeps(//:root, //:missing))").unwrap(),
        workspace: root.workspace.dupe(),
        base_configuration: root.base_configuration.clone(),
        configuration_overlay: root.configuration_overlay.clone(),
        roots: root.roots.clone(),
        literal_roots: root.literal_roots.clone(),
        include_implicit: root.include_implicit,
        include_tool: root.include_tool,
    };
    let invalid = invalid_root.compute(&mut transaction).await.unwrap();
    assert!(matches!(
        invalid,
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(value)
            if matches!(value.as_ref(), Err(CqueryCommandError::Request(message)) if message.contains("invalid Slug regex"))
    ));
    let outcome = root.compute(&mut transaction).await.unwrap();
    assert!(
        matches!(outcome, slug_bzlmod_v2::SourcePreparationOutcome::Need(_)),
        "{outcome:?}"
    );
}

struct CquerySetFixture {
    _workspace: tempfile::TempDir,
    command_policy: BzlmodCommandPolicyKey,
    activation_audit: Arc<ExternalQueryActivationAudit>,
    runtime: WorkspaceRuntime,
}

impl CquerySetFixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().unwrap();
        let command_policy = configured_test_command_policy(workspace.path());
        fs::write(
            workspace.path().join("MODULE.bazel"),
            "module(name = \"sets\")\nbazel_dep(name = \"platforms\", version = \"1.0.0\")\nlocal_path_override(module_name = \"platforms\", path = \".slug_test_builtin/platforms\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("pkg")).unwrap();
        fs::write(
            workspace.path().join("pkg/defs.bzl"),
            "def _impl(ctx):\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("pkg/BUILD.bazel"),
            "load(\":defs.bzl\", \"probe\")\nprobe(name = \"bin\")\nprobe(name = \"lib\")\n",
        )
        .unwrap();
        let activation_audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(activation_audit.clone());
        Self {
            _workspace: workspace,
            command_policy,
            activation_audit,
            runtime,
        }
    }
}

#[test]
fn cquery_set_empty_and_count_preflight() {
    let fixture = CquerySetFixture::new();
    let empty = |expression: &str| {
        fixture
            .runtime
            .cquery_command_with_bzlmod_inputs(
                expression,
                true,
                true,
                fixture.command_policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                root_setting_overlay(None),
            )
            .unwrap()
    };
    for expression in ["set()", "let x = set() in $x"] {
        let accepted = empty(expression);
        let evaluation = accepted.terminal_for_test().as_ref().as_ref().unwrap();
        assert!(evaluation.label_stdout().is_empty());
        assert_eq!(evaluation.analyses().count(), 0);
        assert!(evaluation.starlark_label_stdout().is_empty());
        assert!(fixture.activation_audit.take_configured_roots().is_empty());
    }
    let invalid_count = fixture
        .runtime
        .cquery_command_with_bzlmod_inputs(
            "some(//pkg:missing, 2147483648)",
            true,
            true,
            fixture.command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
        .unwrap_err();
    assert_eq!(invalid_count.exit_code(), 2);
    assert!(
        invalid_count
            .to_string()
            .contains("expected an integer literal: '2147483648'")
    );
    assert!(fixture.activation_audit.take_configured_roots().is_empty());
}

#[rustfmt::skip] macro_rules! cquery_set_output_test {
    ($name:ident, |$run:ident, $labels:ident| $body:block) => {
        #[test]
        fn $name() {
            let fixture = CquerySetFixture::new();
            let $run = |expression: &str| fixture.runtime.cquery_command_with_bzlmod_inputs(
                expression, true, true, fixture.command_policy.clone(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update, &[], root_setting_overlay(None),
            ).unwrap().terminal_for_test().as_ref().as_ref().unwrap().label_stdout();
            let $labels = |output: String| output.lines().map(|line| line.split_once(" (").unwrap().0.to_owned()).collect::<Vec<_>>();
            $body
        }
    };
}

cquery_set_output_test!(cquery_set_union_set_and_let, |run, labels| {
    assert_eq!(labels(run("//pkg:bin union //pkg:lib")), ["//pkg:bin", "//pkg:lib"]);
    assert_eq!(labels(run("set(//pkg:bin //pkg:lib //pkg:bin)")), ["//pkg:bin", "//pkg:lib"]);
    assert_eq!(labels(run("let x = //pkg:bin in $x union //pkg:lib")), ["//pkg:bin", "//pkg:lib"]);
});

cquery_set_output_test!(cquery_set_intersect_and_except, |run, labels| {
    assert!(run("//pkg:bin intersect //pkg:lib").is_empty());
    assert_eq!(labels(run("//pkg:bin except //pkg:lib")), ["//pkg:bin"]);
});

cquery_set_output_test!(cquery_set_exact_prefix_and_missing_filters, |run, labels| {
    assert_eq!(labels(run("filter('^//pkg:bin$', set(//pkg:lib //pkg:bin //pkg:lib))")), ["//pkg:bin"]);
    assert_eq!(labels(run("filter('^//pkg:', set(//pkg:lib //pkg:bin //pkg:lib))")), ["//pkg:lib", "//pkg:bin"]);
    assert!(run("filter('^//missing:', set(//pkg:lib //pkg:bin))").is_empty());
});

cquery_set_output_test!(cquery_set_default_counted_and_filtered_some, |run, labels| {
    assert_eq!(labels(run("some(set(//pkg:lib //pkg:bin))")), ["//pkg:lib"]);
    assert_eq!(labels(run("some(set(//pkg:lib //pkg:bin //pkg:lib), 2)")), ["//pkg:lib", "//pkg:bin"]);
    assert_eq!(labels(run("some(filter('^//pkg:bin$', set(//pkg:lib //pkg:bin)), 10)")), ["//pkg:bin"]);
});

#[test]
fn cquery_some_empty_zero_and_negative_errors() {
    let fixture = CquerySetFixture::new();
    for expression in ["some(set())", "some(//pkg:bin, 0)", "some(//pkg:bin, '-1')"] {
        let accepted = fixture
            .runtime
            .cquery_command_with_bzlmod_inputs(
                expression,
                true,
                true,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[],
                root_setting_overlay(None),
            )
            .unwrap();
        let error = accepted.terminal_for_test().as_ref().as_ref().unwrap_err();
        assert!(
            error.to_string().contains("argument set is empty"),
            "{expression}"
        );
    }
}

#[test]
fn cquery_set_starlark_and_error_precedence() {
    let fixture = CquerySetFixture::new();
    let starlark = fixture
        .runtime
        .cquery_command_with_bzlmod_inputs(
            "let x = set(//pkg:bin //pkg:lib //pkg:bin) in ($x except //pkg:lib) union //pkg:lib",
            true,
            true,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
        .unwrap();
    assert_eq!(
        starlark
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .unwrap()
            .starlark_label_stdout(),
        "@@//pkg:bin\n@@//pkg:lib\n"
    );

    let missing = fixture
        .runtime
        .cquery_command_with_bzlmod_inputs(
            "//pkg:missing union //pkg:also_missing",
            true,
            true,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
        .unwrap();
    let error = missing.terminal_for_test().as_ref().as_ref().unwrap_err();
    assert!(error.missing_stderr().unwrap().contains("//pkg:missing"));

    let missing_before_malformed = fixture
        .runtime
        .cquery_command_with_bzlmod_inputs(
            "filter('(', //pkg:missing)",
            true,
            true,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
        .unwrap();
    let error = missing_before_malformed
        .terminal_for_test()
        .as_ref()
        .as_ref()
        .unwrap_err();
    assert!(error.missing_stderr().unwrap().contains("//pkg:missing"));

    let malformed = fixture
        .runtime
        .cquery_command_with_bzlmod_inputs(
            "filter('(', //pkg:bin)",
            true,
            true,
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(None),
        )
        .unwrap();
    let error = malformed.terminal_for_test().as_ref().as_ref().unwrap_err();
    assert!(error.to_string().contains("invalid Slug regex"));
}

struct CqueryConfigurationFixture {
    workspace: tempfile::TempDir,
    command_policy: BzlmodCommandPolicyKey,
}

impl CqueryConfigurationFixture {
    fn new() -> Self {
        let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-cquery-restores-structural-configuration");
        fs::create_dir_all(&stable_parent).unwrap();
        let workspace = tempfile::tempdir_in(stable_parent).unwrap();
        let command_policy = configured_test_command_policy(workspace.path());
        fs::write(
            workspace.path().join("MODULE.bazel"),
            configured_test_module("module(name = \"cquery_configuration\")\n"),
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            r#"SettingInfo = provider(fields = {"value": "value"})
def _setting(ctx):
    return [SettingInfo(value = ctx.build_setting_value)]
string_setting = rule(implementation = _setting, build_setting = config.string(flag = True))
def _consumer(ctx):
    print("CONSUMER_ANALYSIS")
    return [DefaultInfo(files = depset([]))]
consumer = rule(implementation = _consumer, attrs = {"_setting": attr.label(default = "//:setting")})
def _left(settings, attr):
    return {"//:setting": "left"}
left = transition(implementation = _left, inputs = [], outputs = ["//:setting"])
def _parent(ctx):
    return [DefaultInfo(files = depset([]))]
parent = rule(implementation = _parent, attrs = {"child": attr.label(cfg = left)})
"#,
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "load(\":defs.bzl\", \"consumer\", \"parent\", \"string_setting\")\nstring_setting(name = \"setting\", build_setting_default = \"default\")\nconsumer(name = \"consumer\")\nparent(name = \"parent\", child = \":consumer\")\n",
        )
        .unwrap();
        Self {
            workspace,
            command_policy,
        }
    }
}

#[test]
fn cquery_configuration_c0_c1_c0_projection_and_topology() {
    let fixture = CqueryConfigurationFixture::new();
    let workspace = &fixture.workspace;
    let command_policy = fixture.command_policy.clone();
    let target = "//:consumer";
    let run = |runtime: &WorkspaceRuntime, target: &str, setting: Option<&str>| {
        runtime.cquery_command_with_bzlmod_inputs(
            target,
            true,
            true,
            command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(setting),
        )
    };
    let evaluation =
        |accepted: &AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>| {
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .clone()
        };
    let topology = |evaluation: &CqueryCommandEvaluation| {
        let analysis = evaluation.analyses().next().unwrap();
        (
            analysis.key().label().to_string(),
            analysis
                .providers()
                .names()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
            analysis.declared_outputs().to_vec(),
            analysis.actions().len(),
            analysis
                .configured_dependencies()
                .map(|dependency| dependency.label().to_string())
                .collect::<Vec<_>>(),
        )
    };

    let retained = test_runtime(workspace.path()).unwrap();
    let c0_command = run(&retained, target, None).unwrap();
    let c0 = evaluation(&c0_command);
    let c0_analysis = c0.analyses().next().unwrap();
    assert_eq!(
        c0_analysis
            .configured_target_key()
            .expect("current cquery analysis only contains configured nodes")
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//:setting").unwrap())
            .and_then(|option| option.value().as_str()),
        None
    );
    assert!(c0.label_stdout().starts_with("//:consumer (slugcfg-v2:"));
    assert_eq!(
        c0.label_stdout().len(),
        "//:consumer (slugcfg-v2:)\n".len() + 64
    );
    assert_eq!(c0.starlark_label_stdout(), "@@//:consumer\n");
    assert_eq!(accepted_output_text(&c0_command), ["CONSUMER_ANALYSIS"]);
    let c0_stdout = c0.label_stdout();
    let c0_topology = topology(&c0);

    let c1_command = run(&retained, target, Some("command")).unwrap();
    let c1 = evaluation(&c1_command);
    let c1_analysis = c1.analyses().next().unwrap();
    assert_eq!(
        c1_analysis
            .configured_target_key()
            .expect("current cquery analysis only contains configured nodes")
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//:setting").unwrap())
            .and_then(|option| option.value().as_str()),
        Some("command")
    );
    assert_ne!(c0_stdout, c1.label_stdout());
    assert_eq!(c1.starlark_label_stdout(), "@@//:consumer\n");
    assert_eq!(c0_topology, topology(&c1));
    assert_eq!(accepted_output_text(&c1_command), ["CONSUMER_ANALYSIS"]);

    let restored_command = run(&retained, target, None).unwrap();
    let restored = evaluation(&restored_command);
    assert_eq!(c0_stdout, restored.label_stdout());
    assert_eq!(c0_topology, topology(&restored));
}

#[test]
fn cquery_configuration_missing_fresh_and_setting_default() {
    let fixture = CqueryConfigurationFixture::new();
    let workspace = &fixture.workspace;
    let command_policy = fixture.command_policy.clone();
    let target = "//:consumer";
    let run = |runtime: &WorkspaceRuntime, target: &str, setting: Option<&str>| {
        runtime.cquery_command_with_bzlmod_inputs(
            target,
            true,
            true,
            command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(setting),
        )
    };
    let evaluation =
        |accepted: &AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>| {
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .clone()
        };
    let retained = test_runtime(workspace.path()).unwrap();
    let c0_command = run(&retained, target, None).unwrap();
    let c0_stdout = evaluation(&c0_command).label_stdout();
    let missing = "//:missing";
    let missing_command = run(&retained, missing, Some("command")).unwrap();
    let missing_error = missing_command
        .terminal_for_test()
        .as_ref()
        .as_ref()
        .unwrap_err();
    assert_eq!(missing_error.missing_stderr().unwrap().lines().count(), 3);

    let fresh = test_runtime(workspace.path()).unwrap();
    let one_shot = run(&fresh, target, None).unwrap();
    assert_eq!(c0_stdout, evaluation(&one_shot).label_stdout());

    let setting = "//:setting";
    let setting_command = run(&retained, setting, None).unwrap();
    assert_eq!(
        evaluation(&setting_command)
            .analyses()
            .next()
            .unwrap()
            .configured_target_key()
            .expect("current cquery analysis only contains configured nodes")
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//:setting").unwrap())
            .and_then(|option| option.value().as_str()),
        None
    );
}

#[test]
fn cquery_configuration_transitioned_child() {
    let fixture = CqueryConfigurationFixture::new();
    let command_policy = fixture.command_policy.clone();
    let run = |runtime: &WorkspaceRuntime, target: &str, setting: Option<&str>| {
        runtime.cquery_command_with_bzlmod_inputs(
            target,
            true,
            true,
            command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            root_setting_overlay(setting),
        )
    };
    let evaluation =
        |accepted: &AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>| {
            accepted
                .terminal_for_test()
                .as_ref()
                .as_ref()
                .unwrap()
                .clone()
        };
    let retained = test_runtime(fixture.workspace.path()).unwrap();
    let parent = "//:parent";
    let parent_command = run(&retained, parent, None).unwrap();
    let parent = evaluation(&parent_command);
    let child = parent
        .analyses()
        .next()
        .unwrap()
        .configured_dependencies()
        .next()
        .unwrap();
    assert_eq!(child.label().to_string(), "@@//:consumer");
    assert_eq!(
        child
            .configuration()
            .starlark_option(&CanonicalLabel::parse("@@//:setting").unwrap())
            .and_then(|option| option.value().as_str()),
        Some("left")
    );
}

fn cquery_test_outer(path: &str) -> ObservedPathFrontierError {
    PathObservationEpoch::from_shared([(
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            PathObservationOperation::Lstat,
        ),
        Arc::new(PathObservationResult::FileBytes(
            PathOperationResult::Missing,
        )),
    )])
    .unwrap_err()
    .into()
}

#[test]
fn cquery_batch_reduction_inspects_all_children_before_terminal_precedence() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let first = local_native_request(&workspace, "dep+", "first");
    let conflicting = local_native_request(&workspace, "dep+", "second");
    let mut batch = CqueryBatchAccumulator::new();
    batch.semantic(CqueryCommandError::request("earlier semantic"));
    batch.need(build_test_need("/need"), "path need");
    batch.need(
        slug_bzlmod_v2::SourcePreparationNeeds::repository(first.as_ref().clone()),
        "repository need",
    );
    batch.need(
        slug_bzlmod_v2::SourcePreparationNeeds::repository(conflicting.as_ref().clone()),
        "repository need",
    );
    batch.outer(cquery_test_outer("/first-outer"));
    batch.outer(cquery_test_outer("/later-outer"));
    let Err(error) = batch.finish() else {
        panic!("typed outer must dominate Need union and semantic terminals")
    };
    let error = error.to_string();
    assert!(error.contains("/first-outer"), "{error}");
    assert!(!error.contains("/later-outer"), "{error}");

    let mut without_outer = CqueryBatchAccumulator::new();
    without_outer.semantic(CqueryCommandError::request("semantic"));
    without_outer.need(
        slug_bzlmod_v2::SourcePreparationNeeds::repository(first.as_ref().clone()),
        "repository need",
    );
    without_outer.need(
        slug_bzlmod_v2::SourcePreparationNeeds::repository(conflicting.as_ref().clone()),
        "repository need",
    );
    let Err(error) = without_outer.finish() else {
        panic!("incompatible Needs must dominate semantic terminals")
    };
    assert!(error.to_string().contains("ConflictingRepositoryRequest"));
}

struct CqueryObservedFixture {
    _workspace: tempfile::TempDir,
    command_policy: BzlmodCommandPolicyKey,
    audit: Arc<ExternalQueryActivationAudit>,
    runtime: WorkspaceRuntime,
}

impl CqueryObservedFixture {
    fn new() -> Self {
        let stable_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/slug-cquery-observed-family-events");
        fs::create_dir_all(&stable_parent).unwrap();
        let workspace = tempfile::tempdir_in(stable_parent).unwrap();
        let command_policy = configured_test_command_policy(workspace.path());
        fs::write(
            workspace.path().join("MODULE.bazel"),
            configured_test_module(
                "print(\"MODULE_EVENT\")\nmodule(name = \"observed_cquery\")\n",
            ),
        )
        .unwrap();
        fs::write(
            workspace.path().join("defs.bzl"),
            r#"print("BZL_EVENT")
def _leaf(ctx):
    print("LEAF_ANALYSIS")
    return [DefaultInfo(files = depset([]))]
leaf = rule(implementation = _leaf)
def _root(ctx):
    print("ROOT_ANALYSIS")
    return [DefaultInfo(files = depset([]))]
root = rule(implementation = _root, attrs = {"child": attr.label()})
"#,
        )
        .unwrap();
        fs::write(
            workspace.path().join("BUILD.bazel"),
            "print(\"BUILD_EVENT\")\nload(\":defs.bzl\", \"leaf\", \"root\")\nleaf(name = \"leaf\")\nroot(name = \"root\", child = \":leaf\")\n",
        )
        .unwrap();
        fs::create_dir(workspace.path().join("bazel-out")).unwrap();
        let audit = Arc::new(ExternalQueryActivationAudit::default());
        let runtime = test_runtime(workspace.path())
            .unwrap()
            .with_activation_audit(audit.clone());
        Self {
            _workspace: workspace,
            command_policy,
            audit,
            runtime,
        }
    }

    fn run(
        &self,
        expression: &str,
    ) -> Result<
        AcceptedCommand<Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
        CqueryCommandError,
    > {
        self.runtime.cquery_command_with_bzlmod_inputs(
            expression,
            false,
            true,
            self.command_policy.clone(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Off,
            &[],
            root_setting_overlay(None),
        )
    }
}

#[test]
fn cquery_observed_cold_warm_events_and_epoch_reuse() {
    let fixture = CqueryObservedFixture::new();
    let run = |expression: &str| {
        fixture.run(expression).unwrap()
    };

    let cold = run("deps(//:root)");
    assert!(cold.terminal_for_test().as_ref().is_ok());
    assert_eq!(
        accepted_output_text(&cold),
        [
            "MODULE_EVENT",
            "BZL_EVENT",
            "BUILD_EVENT",
            "LEAF_ANALYSIS",
            "ROOT_ANALYSIS",
        ]
    );
    let cold_snapshot = accepted_native_snapshot(&fixture.runtime);
    assert!(!cold_snapshot.path_observations.observations().is_empty());
    let warm = run("deps(//:root)");
    assert!(accepted_output_text(&warm).is_empty());
    let warm_snapshot = accepted_native_snapshot(&fixture.runtime);
    assert_eq!(
        cold_snapshot.path_observations.observations().len(),
        warm_snapshot.path_observations.observations().len()
    );
    for ((cold_demand, cold_result), (warm_demand, warm_result)) in cold_snapshot
        .path_observations
        .observations()
        .iter()
        .zip(warm_snapshot.path_observations.observations().iter())
    {
        assert_eq!(cold_demand, warm_demand);
        assert_eq!(cold_result, warm_result, "{cold_demand:?}");
        assert!(Arc::ptr_eq(cold_result, warm_result));
    }
}

#[test]
fn cquery_observed_family_route_isolation() {
    let fixture = CqueryObservedFixture::new();
    let run = |expression: &str| fixture.run(expression).unwrap();
    let mut prior = fixture.audit.cquery_family_counts();
    for (expression, observes_seed_package) in [
        ("//:root", false),
        ("//:root union //:leaf", false),
        ("rdeps(//:root, //:leaf)", true),
    ] {
        let accepted = run(expression);
        assert!(
            accepted.terminal_for_test().as_ref().is_ok(),
            "{expression}"
        );
        let counts = fixture.audit.cquery_family_counts();
        assert_eq!(counts.0, 0, "legacy package activation: {expression}");
        assert_eq!(counts.2, 0, "legacy analysis activation: {expression}");
        assert_eq!(
            counts.1 > prior.1,
            observes_seed_package,
            "observed seed-package activation: {expression}; prior={prior:?}, current={counts:?}"
        );
        assert!(
            counts.3 > prior.3,
            "observed analysis activation: {expression}; prior={prior:?}, current={counts:?}"
        );
        prior = counts;
    }
}

#[tokio::test]
async fn cquery_observed_compute_cancellation_discards_child_events_and_recovers() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let tracker = Arc::new(ActionClosureTracker::default());
    let mut epoch = BuildRootEpoch::base(81);
    epoch.file(
        "/workspace/MODULE.bazel",
        "print('MODULE')\nmodule(name = 'cancelled_cquery')\n",
        81,
    );
    epoch.file(
            "/workspace/defs.bzl",
            "print('BZL')\ndef _impl(ctx):\n    print('ANALYSIS')\n    return [DefaultInfo(files = depset([]))]\nprobe = rule(implementation = _impl)\n",
            81,
        );
    epoch.package(
        "",
        "print('BUILD')\nload(':defs.bzl', 'probe')\nprobe(name = 'root')\n",
        81,
    );
    let epoch = epoch.build();
    let root = CqueryCommandRoot {
        expression: QueryExpression::parse("//:root").unwrap(),
        workspace: NormalizedAbsolutePath::new("/workspace").unwrap(),
        base_configuration: build_test_configuration("target"),
        configuration_overlay: CommandConfigurationOverlay::default(),
        roots: Arc::from([CqueryRootTarget {
            requested: Arc::from("//:root"),
            canonical: CanonicalLabel::parse("@@//:root").unwrap(),
        }]),
        literal_roots: Arc::from([(Arc::from("//:root"), 0)]),
        include_implicit: true,
        include_tool: true,
    };
    let data = |tracker: Arc<ActionClosureTracker>| {
        let mut data = UserComputationData {
            activation_tracker: Some(tracker),
            ..Default::default()
        };
        data.data.set(CaptureEvaluationEvents);
        data
    };
    let take_texts = || {
        tracker
            .take()
            .into_iter()
            .filter_map(|(_, _, batch)| batch)
            .flat_map(|batch| batch.events().iter().cloned().collect::<Vec<_>>())
            .filter_map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => Some(text),
                EvaluationEvent::Diagnostic { .. } => None,
            })
            .collect::<Vec<_>>()
    };
    let mut cancelled =
        build_root_transaction_with_data(&dice, epoch.dupe(), data(tracker.dupe())).await;
    let mut future = Box::pin(root.compute(&mut cancelled));
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(future.as_mut(), context).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(future);
    assert!(take_texts().is_empty());
    drop(cancelled);

    let mut recovered =
        build_root_transaction_with_data(&dice, epoch.dupe(), data(tracker.dupe())).await;
    assert!(matches!(
        root.compute(&mut recovered).await.unwrap(),
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()
    ));
    assert_eq!(take_texts(), ["BUILD", "ANALYSIS"]);
    drop(recovered);
    let mut warm = build_root_transaction_with_data(&dice, epoch, data(tracker.dupe())).await;
    assert!(matches!(
        root.compute(&mut warm).await.unwrap(),
        slug_bzlmod_v2::SourcePreparationOutcome::Complete(value) if value.as_ref().is_ok()
    ));
    assert!(take_texts().is_empty());
}
