//! LabelBuildSettings.java58–99: NODEP default and late-bound :alias are distinct.

use super::*;

#[tokio::test]
async fn label_flag_graph_preserves_nodep_implicit_attributes_and_unfiltered_traversal() {
    let workspace = scratch();
    write(workspace.join("MODULE.bazel"), "module(name='root')\n");
    write(
        workspace.join("pkg/BUILD.bazel"),
        "label_flag(name='flag', build_setting_default=':input', help='choose input')\n",
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut transaction = transaction(&dice, &workspace).await;
    let graph = transaction
        .compute(&UnconfiguredPackageGraphKey {
            workspace: workspace.clone(),
            package: "pkg".into(),
        })
        .await
        .unwrap();
    let graph = graph.as_ref().as_ref().unwrap();
    let flag = graph
        .nodes
        .values()
        .find(|node| node.label.target() == "flag")
        .unwrap();
    assert_eq!(flag.kind, QueryNodeKind::Rule("label_flag rule".into()));
    assert_eq!(
        flag.edges.iter().map(|edge| edge.kind).collect::<Vec<_>>(),
        [QueryEdgeKind::Implicit, QueryEdgeKind::Nodep]
    );
    assert_eq!(flag.edges[0].target, flag.edges[1].target);
    let default = flag
        .attributes
        .iter()
        .find(|attr| attr.name == "build_setting_default")
        .unwrap();
    let late = flag
        .attributes
        .iter()
        .find(|attr| attr.name == ":alias")
        .unwrap();
    assert!(default.explicit);
    assert!(!late.explicit);
    assert_eq!(default.labels, late.labels);
    assert_eq!(
        default.value.as_ref().unwrap().provenance,
        AttributeProvenance::Explicit
    );
    assert_eq!(
        late.value.as_ref().unwrap().provenance,
        AttributeProvenance::Implicit
    );
    assert!(matches!(
        graph
            .nodes
            .values()
            .find(|node| node.label.target() == "input")
            .unwrap()
            .kind,
        QueryNodeKind::SourceFile
    ));
    // Query filters remain unsupported. Classification is retained without
    // changing the existing unfiltered query contract or labels() reflection.
    for (expression, expected) in [
        ("labels(build_setting_default, //pkg:flag)", "//pkg:input\n"),
        ("labels(':alias', //pkg:flag)", "//pkg:input\n"),
        ("deps(//pkg:flag)", "//pkg:flag\n//pkg:input\n"),
        ("same_pkg_direct_rdeps(//pkg:input)", "//pkg:flag\n"),
    ] {
        let output = evaluate_loading_query(
            &mut transaction,
            workspace.clone(),
            expression,
            QueryOrder::Auto,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout(), expected, "{expression}");
    }
}

#[tokio::test]
async fn external_label_flag_query_preserves_owner_and_explicit_main_default() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut epoch = RootQueryEpochBuilder::external_package(91);
    epoch.file(
        "/workspace/dep/BUILD.bazel",
        "label_flag(name='flag', build_setting_default=':input')\n\
         label_flag(name='main', build_setting_default='@@//root:input')\n",
        91,
    );
    epoch.package("root", "exports_files(['input'])\n", 91);
    let mut transaction =
        root_query_transaction(&dice, epoch.build(), Arc::new(RootAnchorTracker::default())).await;
    for (expression, expected) in [
        (
            "labels(build_setting_default, @dep//:flag)",
            vec!["@dep//:input"],
        ),
        ("labels(':alias', @dep//:flag)", vec!["@dep//:input"]),
        ("deps(@dep//:flag)", vec!["@dep//:flag", "@dep//:input"]),
        (
            "labels(build_setting_default, @dep//:main)",
            vec!["//root:input"],
        ),
        ("labels(':alias', @dep//:main)", vec!["//root:input"]),
        ("deps(@dep//:main)", vec!["//root:input", "@dep//:main"]),
    ] {
        let value = transaction
            .compute(&root_query_key(expression))
            .await
            .unwrap();
        let QueryPreparationOutcome::Complete(result) = value else {
            panic!("{expression} requested unexpected preparation: {value:?}")
        };
        assert_eq!(
            result.as_ref().as_ref().unwrap().labels.as_ref(),
            expected,
            "{expression}"
        );
    }
}
