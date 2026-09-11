use dice::DetectCycles;
use dice::Dice;

use super::*;
async fn registration_transaction(dice: &Arc<dice::Dice>, module: String) -> dice::DiceTransaction {
    use slug_bzlmod_v2::*;
    use slug_workspace_v2::*;
    use starlark_map::sorted_map::SortedMap;
    let workspace = NormalizedAbsolutePath::new("/registration-errors").unwrap();
    // The same observed-input shape as Loading's registration EpochBuilder.
    let demand = |path: &str, operation| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(path).unwrap(),
            operation,
        )
    };
    let mut entries = Vec::new();
    for (path, kind) in [
        ("/", PathNodeKind::Directory),
        ("/registration-errors", PathNodeKind::Directory),
        (
            "/registration-errors/MODULE.bazel",
            PathNodeKind::RegularFile,
        ),
    ] {
        entries.push((
            demand(path, PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                kind, 1, 1, 1, 1, 0o755,
            ))),
        ));
    }
    entries.push((
        demand(
            "/registration-errors/MODULE.bazel",
            PathObservationOperation::FileBytes,
        ),
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(
            module.as_bytes(),
        ))),
    ));
    for name in ["MODULE.bazel.lock", "REPO.bazel", ".bazelignore"] {
        entries.push((
            demand(
                &format!("/registration-errors/{name}"),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ));
    }
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceSnapshotKey {
                workspace: workspace.as_path().to_owned(),
            },
            Arc::new(WorkspaceSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    workspace.as_path().join("MODULE.bazel"),
                    WorkspaceFileValue::Present(Arc::new(module)),
                )])),
            }),
        )])
        .unwrap();
    updater
        .changed_to(vec![(
            WorkspaceRawSnapshotKey {
                workspace: workspace.as_path().to_owned(),
            },
            Arc::new(WorkspaceRawSnapshot {
                files: Arc::new(SortedMap::from_iter([(
                    workspace.as_path().join("MODULE.bazel.lock"),
                    WorkspaceRawFileValue::Absent,
                )])),
            }),
        )])
        .unwrap();
    inject_root_package_policy_inputs(
        &mut updater,
        RootPackagePolicyInputs::new(
            workspace.clone(),
            [workspace.clone()],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
    inject_root_module_request_inputs(
        &mut updater,
        workspace.as_path(),
        BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
        LockfileMode::Update,
    )
    .unwrap();
    updater.commit().await
}

async fn registration_error_fixture(
    dice: &Arc<Dice>,
    pattern: &str,
) -> Arc<slug_loading_v2::ModuleRegistrationExpansionError> {
    let mut tx = registration_transaction(
        dice,
        format!("module(name='bazel_tools')\nregister_toolchains({pattern:?})\n"),
    )
    .await;
    let workspace = NormalizedAbsolutePath::new("/registration-errors").unwrap();
    let outcome = tx
        .compute(&slug_loading_v2::ModuleRegistrationExpansionKey::toolchains(workspace))
        .await
        .unwrap();
    let LoadingPreparationOutcome::Complete(value) = outcome else {
        panic!("fixture returned Need")
    };
    let error = value.labels_with_shared_error().unwrap_err();
    assert!(matches!(
        error.kind(),
        slug_loading_v2::ModuleRegistrationExpansionErrorKind::Parse(_)
    ));
    Arc::clone(error)
}

fn assert_registration_cause(error: &AnalysisError, value: &ModuleRegistrationExpansion) {
    let AnalysisErrorKind::Registration(typed) = error.kind() else {
        panic!("production handoff lost typed registration: {error}")
    };
    assert!(std::ptr::eq(
        typed.error(),
        value.labels_with_shared_error().unwrap_err().as_ref()
    ));
}

#[tokio::test]
async fn registration_error_natural_production_handoffs() {
    use slug_configuration_v2::CommandConfigurationOccurrence as Occurrence;
    use slug_configuration_v2::CommandConfigurationOverlay;
    use slug_configuration_v2::SlugConfiguration;
    use slug_configuration_v2::StarlarkOptions;
    use slug_configuration_v2::native::host::AutoCpuToken;
    use slug_configuration_v2::native::host::HostConversionInputs;
    use slug_configuration_v2::native::host::HostPathFlavor;
    let workspace = NormalizedAbsolutePath::new("/registration-errors").unwrap();
    let base = SlugConfiguration::default_target(
        &HostConversionInputs::new(
            Some(AutoCpuToken::K8),
            Some(HostPathFlavor::Unix),
            None,
            Arc::from([]),
            Arc::from([]),
        )
        .unwrap(),
    )
    .unwrap();
    // Each row is the earliest failing source/family; later rows also fail,
    // with distinct causes, so swapping precedence cannot satisfy this matrix.
    for mode in [
        ConfiguredAnalysisMode::Legacy,
        ConfiguredAnalysisMode::Observed,
    ] {
        for first in 0..4 {
            let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
            let mut module = "module(name='bazel_tools')\n".to_owned();
            let mut occurrences = Vec::new();
            for row in first..4 {
                let pattern = format!("@missing_{row}//:bad");
                match row {
                    0 => occurrences.push(Occurrence::extra_execution_platforms(&pattern)),
                    1 => module.push_str(&format!("register_execution_platforms({pattern:?})\n")),
                    2 => occurrences.push(Occurrence::extra_toolchains(&pattern)),
                    3 => module.push_str(&format!("register_toolchains({pattern:?})\n")),
                    _ => unreachable!(),
                }
            }
            let configuration = ConfigurationKey::from_slug(
                base.with_command_configuration(
                    StarlarkOptions::default(),
                    &CommandConfigurationOverlay::from(occurrences),
                )
                .unwrap(),
            );
            let structural = configuration.slug_configuration().unwrap();
            let mut tx = registration_transaction(&dice, module).await;
            let source = if first % 2 == 0 {
                RegistrationExpansionSource::Command
            } else {
                RegistrationExpansionSource::Module
            };
            let RegistrationExpansionInput::Value(expected) = compute_registration_expansion_input(
                &mut tx,
                mode,
                workspace.clone(),
                structural,
                source,
                first < 2,
                "proof",
            )
            .await
            else {
                panic!("natural producer did not complete")
            };
            assert!(matches!(
                expected.labels().unwrap_err().kind(),
                slug_loading_v2::ModuleRegistrationExpansionErrorKind::Parse(_)
            ));
            let count = Arc::strong_count(expected.labels_with_shared_error().unwrap_err());
            let LoadingPreparationOutcome::Complete(Ok(Err(error))) =
                prepare_registrations(&mut tx, mode, &workspace, &configuration, true, false).await
            else {
                panic!("four-source preparation lost semantic failure")
            };
            assert_registration_cause(&error, &expected);
            assert_eq!(
                Arc::strong_count(expected.labels_with_shared_error().unwrap_err()),
                count + 1
            );
            assert!(error.to_string().contains(&format!("@missing_{first}")));
            drop(error);
            assert_eq!(
                Arc::strong_count(expected.labels_with_shared_error().unwrap_err()),
                count
            );
            if first < 2 {
                let LoadingPreparationOutcome::Complete(Ok(Err(error))) =
                    prepare_execution_platform_registrations(
                        &mut tx,
                        mode,
                        &workspace,
                        &configuration,
                    )
                    .await
                else {
                    panic!("execution-only preparation lost semantic failure")
                };
                assert_registration_cause(&error, &expected);
            }
            // The merge conversions are defensive after preparation. Exercise
            // both directly with real successful/error producer results.
            let empty_configuration = base
                .with_command_configuration(
                    StarlarkOptions::default(),
                    &CommandConfigurationOverlay::from(Vec::new()),
                )
                .unwrap();
            let RegistrationExpansionInput::Value(empty) = compute_registration_expansion_input(
                &mut tx,
                mode,
                workspace.clone(),
                &empty_configuration,
                RegistrationExpansionSource::Command,
                true,
                "empty proof",
            )
            .await
            else {
                panic!("empty command producer did not complete")
            };
            assert!(empty.labels().unwrap().is_empty());
            for error in [
                merge_registration_labels(&expected, &empty).unwrap_err(),
                merge_registration_labels(&empty, &expected).unwrap_err(),
                merge_registration_labels(&expected, &expected).unwrap_err(),
            ] {
                assert_registration_cause(&error, &expected);
            }
        }
    }
}

#[tokio::test]
async fn registration_error_shared_identity_restoration_and_release() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let pattern = format!("@{}A", "x".repeat(4096));
    let first = registration_error_fixture(&dice, &pattern).await;
    let weak = Arc::downgrade(&first);
    let count = Arc::strong_count(&first);
    let a = AnalysisError::from_registration(&first);
    assert_eq!(Arc::strong_count(&first), count + 1);
    let AnalysisErrorKind::Registration(wrapped) = a.kind() else {
        panic!("lost typed cause")
    };
    assert!(std::ptr::eq(wrapped.error(), first.as_ref()));
    let b = AnalysisError::from(
        registration_error_fixture(&dice, &format!("@{}B", "x".repeat(4096))).await,
    );
    let restored = AnalysisError::from(registration_error_fixture(&dice, &pattern).await);
    assert!(a != b && a == restored);
    assert_eq!(a.to_string(), b.to_string());
    assert!(a.to_string().len() <= 3072);
    assert!(format!("{a:?}").len() < 8192 && format!("{a:#?}").len() < 8192);
    fn accounted<T: Allocative>() {}
    accounted::<RegistrationAnalysisError>();
    drop(first);
    dice.wait_for_idle().await;
    drop(dice);
    assert!(
        weak.upgrade().is_some(),
        "terminal still owns its semantic cause"
    );
    drop(a);
    drop(b);
    drop(restored);
    assert!(weak.upgrade().is_none(), "final error owner released");
}

#[tokio::test]
#[rustfmt::skip]
async fn registration_error_execution_only_need_outer_and_temporary_release() {
    use slug_configuration_v2::{CommandConfigurationOccurrence, CommandConfigurationOverlay, SlugConfiguration, StarlarkOptions};
    use slug_configuration_v2::native::host::{AutoCpuToken, HostConversionInputs, HostPathFlavor};
    use slug_workspace_v2::{PathObservationEpochError, PathObservationDemand, PathObservationNamespace, PathObservationOperation};
    let workspace = NormalizedAbsolutePath::new("/registration-errors").unwrap();
    let configuration = ConfigurationKey::from_slug(SlugConfiguration::default_target(&HostConversionInputs::new(
        Some(AutoCpuToken::K8), Some(HostPathFlavor::Unix), None, Arc::from([]), Arc::from([])).unwrap()).unwrap()
        .with_command_configuration(StarlarkOptions::default(), &CommandConfigurationOverlay::from(vec![
            CommandConfigurationOccurrence::extra_execution_platforms("@bad//:command")])).unwrap());
    for mode in [ConfiguredAnalysisMode::Legacy, ConfiguredAnalysisMode::Observed] {
        let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
        let mut tx = registration_transaction(&dice,
            "module(name='bazel_tools')\nregister_execution_platforms('//missing:p')\n".into()).await;
        let RegistrationExpansionInput::Value(command) = compute_registration_expansion_input(&mut tx, mode,
            workspace.clone(), configuration.slug_configuration().unwrap(), RegistrationExpansionSource::Command, true, "proof").await
            else { panic!("expected command semantic error") };
        let cause = command.labels_with_shared_error().unwrap_err();
        let count = Arc::strong_count(cause);
        assert!(matches!(prepare_execution_platform_registrations(&mut tx, mode, &workspace, &configuration).await,
            LoadingPreparationOutcome::Need(_)));
        assert_eq!(Arc::strong_count(cause), count, "Need releases discarded typed error");
        if matches!(mode, ConfiguredAnalysisMode::Observed) {
            let outer = ObservedPathFrontierError::from(PathObservationEpochError::DuplicateDemand(
                PathObservationDemand::new(PathObservationNamespace::Host, workspace.clone(), PathObservationOperation::Lstat)));
            let mut updater = tx.into_updater();
            updater.changed_to(vec![(ModuleRegistrationExpansionObservationKey::execution_platforms(workspace.clone()),
                LoadingPreparationOutcome::Complete(Err(ModuleRegistrationExpansionObservationError::Frontier(outer.clone()))))]).unwrap();
            let mut tx = updater.commit().await;
            assert!(matches!(prepare_execution_platform_registrations(&mut tx, mode, &workspace, &configuration).await,
                LoadingPreparationOutcome::Complete(Err(error)) if error == outer));
        }
    }
}
