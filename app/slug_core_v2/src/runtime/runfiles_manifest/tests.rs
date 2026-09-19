use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredActionOwnerContext;
use slug_analysis_v2::ConfiguredNodeResult;
use slug_build_api_v2::ActionKind;
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::ActionSpec;
use slug_build_api_v2::ProviderCollection;
use slug_build_api_v2::RunfilesPackageDepset;
use slug_configuration_v2::SlugConfiguration;
use slug_configuration_v2::native::host::AutoCpuToken;
use slug_configuration_v2::native::host::HostConversionInputs;
use slug_configuration_v2::native::host::HostPathFlavor;
use slug_identity_v2::CanonicalLabel;

use super::*;

fn key(name: &str, exec: bool) -> ConfiguredTargetKey {
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    ConfiguredTargetKey::new(
        CanonicalLabel::parse(&format!("@@//:{name}")).unwrap(),
        ConfigurationKey::from_slug(if exec {
            SlugConfiguration::default_exec(&host).unwrap()
        } else {
            SlugConfiguration::default_target(&host).unwrap()
        }),
    )
}

fn producer(key: ConfiguredTargetKey) -> ConfiguredNodeResult {
    let context = Arc::new(ConfiguredActionOwnerContext::unresolved_default(key.clone()).unwrap());
    ConfiguredNodeResult::new_rule(
        key,
        ProviderCollection::from_values(Vec::new(), false).unwrap(),
        None,
        RunfilesPackageDepset::empty(),
    )
    .with_action_specs(
        vec![ActionSpec::new(
            ActionKind::Write {
                content: "x".into(),
                is_executable: false,
            },
            "FileWrite",
            vec![ActionOutput::new("out", ActionOutputKind::File)],
        )],
        vec![context],
    )
    .unwrap()
}

fn artifact(key: &ConfiguredTargetKey, name: &str, kind: ActionOutputKind) -> AnalysisArtifact {
    AnalysisArtifact::Derived {
        owner: key.artifact_owner(),
        output: ActionOutput::new(name, kind),
    }
}

#[test]
fn generated_targets_require_exact_owner_configuration_and_output_declaration() {
    let workspace = NormalizedAbsolutePath::new(std::path::PathBuf::from("/workspace")).unwrap();
    let target = key("producer", false);
    let exec = key("producer", true);
    let node = producer(target.clone());
    let declared = paths::declared_outputs(std::iter::once(&node)).unwrap();
    let good = artifact(&target, "out", ActionOutputKind::File);
    let expected = crate::runtime::configured_output_root(
        workspace.as_path(),
        target.configuration().slug_configuration().unwrap(),
    )
    .join("out");
    assert_eq!(
        paths::generated_target(&workspace, &declared, &good).unwrap(),
        expected.to_str().unwrap()
    );
    for missing in [
        artifact(&key("unknown", false), "out", ActionOutputKind::File),
        artifact(&exec, "out", ActionOutputKind::File),
        artifact(&target, "absent", ActionOutputKind::File),
        artifact(&target, "out", ActionOutputKind::Directory),
    ] {
        assert!(
            paths::generated_target(&workspace, &declared, &missing)
                .unwrap_err()
                .contains("no exact declared producer")
        );
    }
    // Each admitted producer supplies its own structural output configuration.
    let exec_node = producer(exec.clone());
    let both = paths::declared_outputs([&node, &exec_node].into_iter()).unwrap();
    let exec_path = paths::generated_target(
        &workspace,
        &both,
        &artifact(&exec, "out", ActionOutputKind::File),
    )
    .unwrap();
    assert_ne!(exec_path, expected.to_str().unwrap());
    assert_eq!(
        exec_path,
        crate::runtime::configured_output_root(
            workspace.as_path(),
            exec.configuration().slug_configuration().unwrap()
        )
        .join("out")
        .to_str()
        .unwrap()
    );
}

#[test]
fn source_target_path_spelling_is_absolute() {
    assert_eq!(
        paths::absolute_string(std::path::Path::new("/workspace/link")).unwrap(),
        "/workspace/link"
    );
    assert!(
        paths::absolute_string(std::path::Path::new("relative"))
            .unwrap_err()
            .contains("not absolute")
    );
}
