//! Authored compile-output boundary callers; no full compile/discovery admission.
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisValue;

use super::super::cc_configure_features::config;
use super::super::cc_configure_features::field;
use super::*;

const PROOF: &str = "rules_cc/cc/private/compile_outputs_proof.bzl";

fn workspace() -> PathBuf {
    let root = super::workspace_with(
        "load('@rules_cc//cc/private:compile_outputs_proof.bzl','subject','configuration_escape')\n",
        "load(':proof.bzl','configuration_escape')\nconfiguration_escape(name='escape')\nsubject(name='outputs',external='//pkg:external',external_exec='//pkg:external',main='@@//pkg:main',main_exec='@@//pkg:main')\n",
    );
    fs::write(root.join(PROOF), include_str!("compile_outputs.bzl")).unwrap();
    for (directory, name) in [("pkg", "main"), ("rules_rust/pkg", "external")] {
        fs::create_dir_all(root.join(directory)).unwrap();
        fs::write(root.join(directory).join("BUILD.bazel"), format!(
            "load('@rules_cc//cc/private:compile_outputs_proof.bzl','producer')\nproducer(name='{name}',visibility=['//visibility:public'])\n"
        )).unwrap();
    }
    root
}

async fn request(
    dice: &Arc<Dice>,
    root: &std::path::Path,
    configuration: ConfigurationKey,
) -> Arc<ConfiguredNodeResult> {
    super::request_configured(dice, root, root_epoch(root), true, "outputs", configuration)
        .await
        .unwrap()
}

fn member<'a>(value: &'a AnalysisValue, name: &str) -> &'a AnalysisValue {
    let AnalysisValueKind::Struct(fields) = value.kind() else {
        panic!("snapshot struct")
    };
    fields.get(name).unwrap()
}

fn check(result: &ConfiguredNodeResult, changed_purpose: bool) {
    assert!(result.actions().is_empty());
    let AnalysisValueKind::List(rows) = field(result, "snapshot").kind() else {
        panic!("snapshot rows")
    };
    assert_eq!(rows.len(), 4);
    for (index, row) in rows.iter().enumerate() {
        let external = index < 2;
        let exec = index % 2 == 1;
        let repository = if external { "rules_rust+" } else { "" };
        let active_name = if external { "external" } else { "main" };
        let coordinate = if external {
            "external/rules_rust+/pkg"
        } else {
            "pkg"
        };
        let prefix = if external && changed_purpose {
            "arc/"
        } else {
            ""
        };
        assert_eq!(
            member(row, "prefix").as_str(),
            Some(prefix.trim_end_matches('/'))
        );
        assert_eq!(
            member(row, "named").as_str(),
            Some(format!("{prefix}dir/unit.o").as_str())
        );
        let AnalysisValueKind::ArtifactRoot(root) = member(row, "root").kind() else {
            panic!("published root")
        };
        let slug_build_api_v2::AnalysisArtifactRoot::Derived(derived) = root else {
            panic!("compile output root must be derived")
        };
        assert_eq!(
            derived.configuration().kind(),
            if exec {
                slug_configuration_v2::SlugConfigurationKind::Exec
            } else {
                slug_configuration_v2::SlugConfigurationKind::Target
            }
        );
        assert_eq!(root.path().contains("-exec/bin"), exec);
        for (field, label) in [
            ("owner", format!("@@{repository}//pkg:{active_name}")),
            ("location", format!("@@{repository}//pkg:location")),
            ("descendant", format!("@@{repository}//pkg/child:located")),
        ] {
            let AnalysisValueKind::Label(actual) = member(row, field).kind() else {
                panic!("published Label")
            };
            assert_eq!(actual.to_string(), label);
        }
        let AnalysisValueKind::List(files) = member(row, "files").kind() else {
            panic!("published Files")
        };
        assert_eq!(files.len(), 3);
        let relative = [
            format!("{coordinate}/_objs/location/{prefix}dir/unit.o"),
            format!("{coordinate}/child/_objs/located/child.o"),
            format!("{coordinate}/_objs/location/normalized.o"),
        ];
        for (file, expected) in files.iter().zip(relative) {
            let AnalysisValueKind::Artifact(AnalysisArtifact::Derived { owner, output }) =
                file.kind()
            else {
                panic!("derived File")
            };
            assert_eq!(
                owner.label().to_string(),
                format!("@@{repository}//pkg:{active_name}")
            );
            assert_eq!(output.kind(), slug_build_api_v2::ActionOutputKind::File);
            assert_eq!(output.root_relative_path(), expected);
            assert_eq!(output.path(), format!("{}/{expected}", root.path()));
            assert_eq!(
                &slug_build_api_v2::AnalysisArtifactRoot::Derived(output.root().clone()),
                root
            );
        }
    }
}

#[tokio::test]
async fn compile_configuration_facade_publication_remains_unsupported() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let error =
        super::request_configured(&dice, &root, root_epoch(&root), true, "escape", config(&[]))
            .await
            .unwrap_err();
    assert!(
        error.contains("unsupported analysis value of type `configuration`"),
        "{error}"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn compile_outputs_publish_supplied_locations_with_active_main_external_target_exec_owners() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let result = request(&dice, &root, config(&[])).await;
    check(&result, false);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn compile_output_declarations_restore_source_and_configuration_in_same_dice() {
    let root = workspace();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let first = request(&dice, &root, config(&[])).await;
    check(&first, false);
    let build = root.join("rules_rust/pkg/BUILD.bazel");
    let original = fs::read_to_string(&build).unwrap();
    fs::write(
        &build,
        original.replace(
            "name='external',",
            "name='external',purpose='compile_objc_arc',",
        ),
    )
    .unwrap();
    let changed = request(&dice, &root, config(&[])).await;
    check(&changed, true);
    assert!(!field(&first, "snapshot").publication_eq(field(&changed, "snapshot")));
    fs::write(&build, original).unwrap();
    assert_eq!(first, request(&dice, &root, config(&[])).await);
    let configured = request(
        &dice,
        &root,
        config(&[(NativeCommandOption::HostCopt, "unused-root-payload")]),
    )
    .await;
    check(&configured, false);
    assert!(!field(&first, "snapshot").publication_eq(field(&configured, "snapshot")));
    assert_eq!(first, request(&dice, &root, config(&[])).await);
    fs::remove_dir_all(root).unwrap();
}
