use dupe::Dupe;
use slug_identity_v2::CanonicalLabel;

use super::*;
use crate::ActionKind;
use crate::ActionOutput;
use crate::ActionOutputKind;
use crate::ActionRegistry;
use crate::ActionSpec;
use crate::AnalysisArtifact;
use crate::AnalysisConfiguredTargetKey;
use crate::AnalysisValue;

#[test]
fn root_identity_retains_configuration_and_source_repository_not_display() {
    let main = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:a").unwrap()).root();
    let external = AnalysisArtifact::Source(CanonicalLabel::parse("@@dep+//pkg:a").unwrap()).root();
    assert_eq!(main.path(), "");
    assert_eq!(external.path(), "");
    assert_ne!(main, external);
    let a = test_support::root_for(b"a");
    let b = test_support::root_for(b"b");
    let restored = test_support::root_for(b"a");
    assert_ne!(a, b);
    assert_eq!(a, restored);
    assert_eq!(a.path().split('/').count(), 3);
    let root = AnalysisValue::artifact_root(AnalysisArtifactRoot::Derived(a.clone()));
    let restored = AnalysisValue::artifact_root(AnalysisArtifactRoot::Derived(restored));
    assert!(root.publication_eq(&restored));
    assert_eq!(
        root.starlark_hash().unwrap(),
        restored.starlark_hash().unwrap()
    );
    assert!(root.is_starlark_immutable());
    assert!(root.is_valid_depset_leaf());
    assert_eq!(
        std::mem::size_of::<DerivedArtifactRoot>(),
        std::mem::size_of::<usize>()
    );
    assert!(Arc::ptr_eq(&a.0, &a.dupe().0));
}

#[test]
fn output_roots_are_independent_of_owner_and_preserve_coordinates() {
    let root = test_support::root();
    let output = ActionOutput::new(root.clone(), "external/dep+/pkg/a", ActionOutputKind::File);
    assert_eq!(output.root_relative_path(), "external/dep+/pkg/a");
    assert_eq!(
        output.path(),
        format!("{}/external/dep+/pkg/a", root.path())
    );
    assert_eq!(
        std::mem::size_of::<ActionOutput>(),
        std::mem::size_of::<usize>()
    );
    let artifact = AnalysisArtifact::Derived {
        owner: AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@other+//pkg:owner").unwrap(),
            b"independent".as_slice(),
        ),
        output: output.clone(),
    };
    assert_eq!(artifact.root(), AnalysisArtifactRoot::Derived(root.clone()));
    assert_eq!(artifact.short_path(), "../dep+/pkg/a");
    let solib = ActionOutput::new(root.clone(), "_solib_k8/liba.so", ActionOutputKind::File);
    let artifact = AnalysisArtifact::Derived {
        owner: match artifact {
            AnalysisArtifact::Derived { owner, .. } => owner,
            _ => unreachable!(),
        },
        output: solib,
    };
    assert_eq!(artifact.short_path(), "_solib_k8/liba.so");
    let mut registry = ActionRegistry::new();
    let write = |output| {
        ActionSpec::new(
            ActionKind::Write {
                content: "x".into(),
                is_executable: false,
            },
            "FileWrite",
            vec![output],
        )
    };
    registry.register(write(output.clone())).unwrap();
    assert_eq!(registry.structural_output_owner(&output), Some(0));
    let changed_kind = ActionOutput::new(
        root.clone(),
        output.root_relative_path(),
        ActionOutputKind::Directory,
    );
    assert_eq!(registry.structural_output_owner(&changed_kind), Some(0));
    // A deliberately forged display collision cannot become structural identity.
    // Production construction is private and deterministic; this seam verifies
    // the registry's independent fail-closed projection guard.
    let other = test_support::root_for(b"collision");
    let collision = DerivedArtifactRoot(Arc::new(DerivedArtifactRootData {
        configuration: other.configuration().clone(),
        path: root.0.path.clone(),
    }));
    let collision = ActionOutput::new(
        collision,
        output.root_relative_path(),
        ActionOutputKind::File,
    );
    assert_eq!(collision.path(), output.path());
    assert_eq!(registry.structural_output_owner(&collision), None);
    assert!(registry.register(write(collision)).is_err());
    assert!(
        registry
            .register(write(ActionOutput::new(
                root,
                output.root_relative_path(),
                ActionOutputKind::Directory
            )))
            .is_err()
    );
    registry
        .register(write(ActionOutput::new(
            test_support::root_for(b"other"),
            output.root_relative_path(),
            ActionOutputKind::File,
        )))
        .unwrap();
}

#[test]
fn roots_reject_host_like_and_exec_role_is_a_single_component() {
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let host = HostConversionInputs::new(
        Some(AutoCpuToken::K8),
        Some(HostPathFlavor::Unix),
        None,
        Arc::from([]),
        Arc::from([]),
    )
    .unwrap();
    assert!(
        DerivedArtifactRoot::bin(SlugConfiguration::default_host_like(&host).unwrap()).is_err()
    );
    let exec = DerivedArtifactRoot::bin(SlugConfiguration::default_exec(&host).unwrap()).unwrap();
    assert!(exec.configuration_component().ends_with("-exec"));
    assert_eq!(
        exec.path().split('/').collect::<Vec<_>>(),
        vec!["bazel-out", exec.configuration_component(), "bin"]
    );
}
