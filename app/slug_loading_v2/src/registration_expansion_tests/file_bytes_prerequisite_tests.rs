//! Actual observed registration rows with known metadata and unobserved bytes.
//! These are Loading/DICE certificates, without native capture authority.
use super::*;

const BUILD: &str = "print('BYTE_ROW')\nfilegroup(name='impl')\ntoolchain_type(name='type')\ntoolchain(name='tc',toolchain_type=':type',toolchain=':impl')\nplatform(name='platform')\n";
fn byte(name: &str) -> PathObservationDemand {
    EpochBuilder::demand(
        &format!("/workspace/{name}/BUILD.bazel"),
        PathObservationOperation::FileBytes,
    )
}
fn complete(module: &str) -> PathObservationEpoch {
    let mut builder = EpochBuilder::root_package_builder(module, 70);
    for name in ["a", "b"] {
        builder.directory(&format!("/workspace/{name}"), 70);
        builder.file(&format!("/workspace/{name}/BUILD.bazel"), BUILD, 70);
        builder.entries.insert(
            EpochBuilder::demand(
                &format!("/workspace/{name}/BUILD.bazel"),
                PathObservationOperation::Lstat,
            ),
            PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                BUILD.len() as i64,
                70,
                70,
                70,
                0o755,
            ))),
        );
    }
    builder.build()
}
fn without(full: &PathObservationEpoch, missing: &[PathObservationDemand]) -> PathObservationEpoch {
    PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| !missing.contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap()
}
fn fulfilled(
    full: &PathObservationEpoch,
    cold: &PathObservationEpoch,
    need: &slug_bzlmod_v2::SourcePreparationNeeds,
) -> PathObservationEpoch {
    let returned = need.only_path_observations().unwrap().demands();
    let supplied = PathObservationEpoch::from_shared(
        full.observations()
            .iter()
            .filter(|(d, _)| returned.contains(d))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    assert_eq!(supplied.observations().len(), returned.len());
    cold.try_union(&supplied).unwrap()
}
fn expected() -> slug_bzlmod_v2::SourcePreparationNeeds {
    slug_bzlmod_v2::SourcePreparationNeeds::path(
        slug_workspace_v2::NeedPathObservations::try_from_iter([byte("a"), byte("b")]).unwrap(),
    )
}
fn no_family_batch(rows: &[TrackedActivation], key: &impl ToString) {
    assert!(
        rows.iter()
            .filter(|row| row.key == key.to_string())
            .all(|row| row.batch.is_none())
    );
    // Both actual package children activate, but their BUILD bytes are absent,
    // so no child evaluator can print while the family is pending.
    for name in ["a", "b"] {
        assert!(rows.iter().any(|row| {
            row.key
                .contains(&format!("host-package-load:\"/workspace\"//{name}"))
        }));
    }
    assert!(rows.iter().filter_map(|row|row.batch.as_ref()).all(|batch| !batch.events().iter().any(|event|matches!(event,EvaluationEvent::StarlarkPrint{text,..} if text.as_str()=="BYTE_ROW"))));
}

#[tokio::test]
async fn observed_module_registration_known_bytes_discover_both_rows_and_finish_one_round() {
    let module = "module(name='bazel_tools')\nregister_toolchains('//a:tc','//b:tc')\n";
    let full = complete(module);
    let cold = without(&full, &[byte("a"), byte("b")]);
    let tracker = Arc::new(ExpansionTracker::default());
    let mut tx = transaction_with_tracker(module, cold.dupe(), true, Some(tracker.dupe())).await;
    let key = ModuleRegistrationExpansionObservationKey::toolchains(workspace());
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Need(need) = &value else {
        panic!("known row bytes must remain a Need: {value:?}")
    };
    assert_eq!(need, &expected());
    assert!(!ModuleRegistrationExpansionObservationKey::validity(&value));
    assert!(!ModuleRegistrationExpansionObservationKey::equality(
        &value, &value
    ));
    no_family_batch(&tracker.take(), &key);
    let mut tx = update_module_epoch(tx, module, fulfilled(&full, &cold, need)).await;
    let value = tx.compute(&key).await.unwrap();
    let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
        panic!("one round must complete registrations: {value:?}")
    };
    assert_eq!(labels(observed.result()), ["@@//a:tc", "@@//b:tc"]);
    for (d, r) in observed.observations().observations() {
        assert!(Arc::ptr_eq(r, full.get(d).unwrap()));
    }
    let rows = tracker.take();
    assert!(
        activation_for(&rows, &key)
            .batch
            .as_ref()
            .unwrap()
            .events()
            .is_empty()
    );
    assert_eq!(rows.iter().filter_map(|row|row.batch.as_ref()).flat_map(|batch|batch.events()).filter(|event|matches!(event,EvaluationEvent::StarlarkPrint{text,..} if text.as_str()=="BYTE_ROW")).count(),2);
}

#[tokio::test]
async fn observed_command_registration_known_bytes_keep_signed_order_and_toolchain_reversal() {
    let module = "module(name='bazel_tools')\n";
    let full = complete(module);
    let cold = without(&full, &[byte("a"), byte("b")]);
    for toolchains in [true, false] {
        let configuration = if toolchains {
            command_configuration([CommandConfigurationOccurrence::extra_toolchains(
                "//a:tc,//b:tc,-//a:tc",
            )])
        } else {
            command_configuration([CommandConfigurationOccurrence::extra_execution_platforms(
                "//a:platform,//b:platform,-//a:platform",
            )])
        };
        let key = if toolchains {
            CommandRegistrationExpansionObservationKey::toolchains(workspace(), configuration)
        } else {
            CommandRegistrationExpansionObservationKey::execution_platforms(
                workspace(),
                configuration,
            )
        };
        let tracker = Arc::new(ExpansionTracker::default());
        let mut tx =
            transaction_with_tracker(module, cold.dupe(), true, Some(tracker.dupe())).await;
        let value = tx.compute(&key).await.unwrap();
        let LoadingPreparationOutcome::Need(need) = &value else {
            panic!("both command rows must suspend: {value:?}")
        };
        assert_eq!(need, &expected());
        assert!(!CommandRegistrationExpansionObservationKey::validity(
            &value
        ));
        no_family_batch(&tracker.take(), &key);
        let mut tx = update_module_epoch(tx, module, fulfilled(&full, &cold, need)).await;
        let value = tx.compute(&key).await.unwrap();
        let LoadingPreparationOutcome::Complete(Ok(observed)) = &value else {
            panic!("command byte retry must complete: {value:?}")
        };
        assert_eq!(
            labels(observed.result()),
            if toolchains {
                vec!["@@//b:tc", "@@//a:tc"]
            } else {
                vec!["@@//b:platform"]
            }
        );
        for (d, r) in observed.observations().observations() {
            assert!(Arc::ptr_eq(r, full.get(d).unwrap()));
        }
    }
}
