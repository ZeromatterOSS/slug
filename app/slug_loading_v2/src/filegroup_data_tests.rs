//! Pinned FilegroupRule:60-62 and RuleClass:2101-2118 retained data boundary.
//! AggregatingAttributeMapper:244-304 distinguishes branch-local duplicates.
use super::*;

#[tokio::test]
async fn filegroup_data_retains_labels_provenance_and_selectors_in_both_packages() {
    const LABELS: &str = "TYPED = Label(':typed')\n";
    for entry in ["filegroup", "native.filegroup"] {
        let build = format!(
            "load(':labels.bzl','TYPED')\n{entry}(name='omitted')\n{entry}(name='none',data=None)\n{entry}(name='empty',data=[])\n{entry}(name='labels',data=[':local',TYPED,'@@//root:root'],tags=['manual'])\n{entry}(name='selected',srcs=select({{':shared':[':source'],':src':[':other_source'],'//conditions:default':[]}}),data=[':fixed']+select({{':shared':[':branch'],':data':[':branch'],'//conditions:default':[]}})+select({{':data':[':extra'],'//conditions:default':[]}}))\n"
        );
        let external = load_repository_package_fixture(
            &[
                ("BUILD.bazel", build.as_bytes()),
                ("labels.bzl", LABELS.as_bytes()),
            ],
            1201,
        )
        .await;
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let root = compute_package(
            &dice,
            EpochBuilder::workspace_sources("", &build, &[("labels.bzl", LABELS)], 1202).build(),
            package_policy(),
        )
        .await;
        for (package, prefix) in [
            (repository_package_terminal(&external), "@@dep+//"),
            (terminal_package(&root), "@@//pkg"),
        ] {
            for (target, provenance) in [
                ("omitted", AttributeProvenance::Default),
                ("none", AttributeProvenance::Default),
                ("empty", AttributeProvenance::Explicit),
            ] {
                let (schema, value) = package
                    .native_attributes(target)
                    .unwrap()
                    .get("data")
                    .unwrap();
                assert_eq!(schema.kind(), AttributeKind::LabelList);
                assert_eq!(value.provenance, provenance);
                assert!(
                    matches!(&value.value, CoercedAttributeValue::LabelList(labels) if labels.is_empty())
                );
            }
            let labels = package.native_attributes("labels").unwrap();
            let (_, data) = labels.get("data").unwrap();
            assert_eq!(data.provenance, AttributeProvenance::Explicit);
            let CoercedAttributeValue::LabelList(data) = &data.value else {
                panic!("literal data must retain its label-list shape");
            };
            assert_eq!(
                data.iter().map(ToString::to_string).collect::<Vec<_>>(),
                [
                    format!("{prefix}:local"),
                    format!("{prefix}:typed"),
                    "@@//root:root".into()
                ]
            );
            assert_eq!(
                labels.get("tags").unwrap().1.value,
                CoercedAttributeValue::StringList(Arc::from(["manual".into()]))
            );
            let selected = package.native_attributes("selected").unwrap();
            let (_, data) = selected.get("data").unwrap();
            assert_eq!(data.provenance, AttributeProvenance::Explicit);
            assert!(matches!(
                &data.value,
                CoercedAttributeValue::Concatenation(_, _)
            ));
            let mut retained_data = Vec::new();
            data.value.labels(&mut retained_data);
            assert_eq!(
                retained_data
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                ["fixed", "branch", "branch", "extra"].map(|name| format!("{prefix}:{name}"))
            );
            let mut data_keys = data
                .value
                .selector_key_labels()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            data_keys.sort();
            assert_eq!(
                data_keys,
                [format!("{prefix}:data"), format!("{prefix}:shared")]
            );
            let CoercedAttributeValue::LabelList(keys) =
                &selected.get("$config_dependencies").unwrap().1.value
            else {
                panic!("src/data selector keys must remain ordinary configuration dependencies");
            };
            let mut keys = keys.iter().map(ToString::to_string).collect::<Vec<_>>();
            keys.sort();
            assert_eq!(
                keys,
                [
                    format!("{prefix}:data"),
                    format!("{prefix}:shared"),
                    format!("{prefix}:src")
                ]
            );
            let target = package
                .targets
                .iter()
                .find(|target| target.name == "selected")
                .unwrap();
            let PackageTargetKind::Filegroup { srcs, .. } = &target.kind else {
                panic!("native filegroup kind was lost")
            };
            assert_eq!(srcs.len(), 2);
            assert!(
                srcs.iter()
                    .all(|label| label.target().as_str().contains("source"))
            );
        }
    }
}

#[tokio::test]
async fn filegroup_data_rejects_duplicate_canonical_labels_and_malformed_values() {
    for entry in ["filegroup", "native.filegroup"] {
        for (data, expected) in [
            ("[':same','@@dep+//:same']", "duplicated in the 'data'"),
            (
                "select({':condition':[':same','@@dep+//:same'],'//conditions:default':[]})",
                "duplicated in the 'data'",
            ),
            ("'not-a-list'", "data"),
            ("[1]", "data"),
            ("select({'//conditions:default':'not-a-list'})", "data"),
            ("select({'//conditions:default':[1]})", "data"),
        ] {
            let build = format!("{entry}(name='bad',data={data})\n");
            let outcome =
                load_repository_package_fixture(&[("BUILD.bazel", build.as_bytes())], 1203).await;
            let error = repository_package_error(&outcome);
            assert!(error.contains(expected), "{entry} data={data}: {error}");
        }
    }
}
