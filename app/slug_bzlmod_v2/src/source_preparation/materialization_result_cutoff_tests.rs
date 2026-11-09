//! Pure selection/cutoff tests over the actual result key and validated epochs.
//! These metadata fixtures mint no native root, receipt or materialization IO.
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dice::DetectCycles;
use dice::Dice;
use dice::DiceEvent;
use dice::DiceEventListener;
use dice::DiceTransaction;
use dice::UserComputationData;

use super::*;

#[derive(Default, Allocative)]
struct ResultComputes {
    #[allocative(skip)]
    count: AtomicUsize,
}
impl DiceEventListener for ResultComputes {
    fn event(&self, event: DiceEvent) {
        if matches!(event, DiceEvent::ComputeStarted { key_type }
            if key_type == "RepositoryMaterializationResultKey")
        {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
}
fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/result-cutoff").unwrap()
}
fn request(name: &str) -> Arc<RepositoryMaterializationRequest> {
    Arc::new(RepositoryMaterializationRequest {
        id: RepositoryMaterializationRequestId {
            workspace: workspace(),
            canonical_repo: CanonicalRepoName::new(name).unwrap(),
        },
        repo_spec: crate::RepoSpec {
            rule_id: crate::RepoRuleId {
                bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                    .unwrap(),
                rule_name: "local_repository".into(),
            },
            attributes: Arc::default(),
        },
        kind: RepositoryMaterializationKind::Local {
            logical_root: NormalizedAbsolutePath::new(format!("/result-cutoff/{name}")).unwrap(),
        },
    })
}
fn local(request: Arc<RepositoryMaterializationRequest>) -> RepositoryMaterializationEpochEntry {
    RepositoryMaterializationEpochEntry {
        request,
        result: RepositoryMaterializationResult::Success(RepositoryMaterializationSuccess::Local),
    }
}
fn epoch(
    entries: impl IntoIterator<Item = RepositoryMaterializationEpochEntry>,
) -> RepositoryMaterializationResultEpoch {
    RepositoryMaterializationResultEpoch::new(workspace(), entries).unwrap()
}
async fn fixture() -> (DiceTransaction, Arc<ResultComputes>) {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let counter = Arc::new(ResultComputes::default());
    let tx = dice
        .updater_with_data(UserComputationData {
            tracker: counter.clone(),
            ..Default::default()
        })
        .commit()
        .await;
    (tx, counter)
}
async fn publish(
    tx: DiceTransaction,
    value: RepositoryMaterializationResultEpoch,
) -> DiceTransaction {
    let mut updater = tx.into_updater();
    updater
        .changed_to([(
            RepositoryMaterializationResultEpochKey {
                workspace: workspace(),
            },
            value,
        )])
        .unwrap();
    updater.commit().await
}
type Outcome = <RepositoryMaterializationResultKey as Key>::Value;
async fn query(
    tx: &mut DiceTransaction,
    request: &Arc<RepositoryMaterializationRequest>,
) -> Outcome {
    tx.compute(&RepositoryMaterializationResultKey {
        request: request.clone(),
    })
    .await
    .unwrap()
}
fn need(value: &Outcome, expected: &Arc<RepositoryMaterializationRequest>) {
    let SourcePreparationOutcome::Need(needs) = value else {
        panic!("expected scheduling Need")
    };
    assert_eq!(
        needs.repository_materializations().get(&expected.id),
        Some(expected)
    );
    assert!(
        !RepositoryMaterializationResultKey::validity(value),
        "Need remains non-cacheable"
    );
}

#[tokio::test]
async fn completed_result_a_stays_cached_across_unrelated_b_and_retained_publication() {
    let (tx, counter) = fixture().await;
    let a = request("a+");
    let b = request("b+");
    let mut tx = publish(tx, epoch([local(a.clone())])).await;
    let first = query(&mut tx, &a).await;
    assert!(matches!(&first, SourcePreparationOutcome::Complete(result) if result.is_ok()));
    let initial = counter.count.load(Ordering::Relaxed);
    assert_eq!(initial, 1);
    for entries in [
        vec![local(a.clone()), local(b.clone())],
        vec![
            local(a.clone()),
            RepositoryMaterializationEpochEntry {
                request: b.clone(),
                result: RepositoryMaterializationResult::SpecError("unrelated failure".into()),
            },
        ],
        vec![local(a.clone())],
    ] {
        tx = publish(tx, epoch(entries)).await;
        assert!(RepositoryMaterializationResultKey::equality(
            &first,
            &query(&mut tx, &a).await
        ));
        assert_eq!(
            counter.count.load(Ordering::Relaxed),
            initial,
            "unrelated B must not recompute completed A"
        );
    }
    let mut generated = (*request("retained+")).clone();
    generated.kind = RepositoryMaterializationKind::GeneratedFileEffects(
        crate::GeneratedRepositoryFileEffectPlan::build([(
            "out".into(),
            Arc::<[u8]>::from(&b"retained"[..]),
            false,
        )])
        .unwrap(),
    );
    let old = epoch([RepositoryMaterializationEpochEntry {
        request: Arc::new(generated),
        result: RepositoryMaterializationResult::Success(
            RepositoryMaterializationSuccess::Immutable {
                source_identity: "retained identity".into(),
                generation_root: PathBuf::from("/retained-output"),
                observation_instance: PathObservationInstanceId::new(91),
                path_policy: RepositoryMaterializationPathPolicy::LegacyUnbounded,
            },
        ),
    }]);
    let with_inventory = epoch([local(a.clone())])
        .retaining_generated_outputs(Some(&old), 4)
        .unwrap();
    let retired = old
        .entry_for_repository(&CanonicalRepoName::new("retained+").unwrap())
        .unwrap()
        .request
        .clone();
    assert_eq!(
        with_inventory
            .borrowed_retained_generated(&workspace(), 4)
            .unwrap()
            .len(),
        1
    );
    tx = publish(tx, with_inventory).await;
    assert!(RepositoryMaterializationResultKey::equality(
        &first,
        &query(&mut tx, &a).await
    ));
    assert_eq!(
        counter.count.load(Ordering::Relaxed),
        initial,
        "retained inventory is not A's current result"
    );
    need(&query(&mut tx, &retired).await, &retired);
}

#[tokio::test]
async fn result_selection_absence_foreign_workspace_and_full_request_conflict_preserve_need() {
    let (mut tx, _) = fixture().await;
    let a = request("a+");
    need(&query(&mut tx, &a).await, &a); // Missing injected epoch.
    tx = publish(tx, epoch([])).await;
    need(&query(&mut tx, &a).await, &a);
    let foreign = NormalizedAbsolutePath::new("/foreign-result-cutoff").unwrap();
    tx = publish(
        tx,
        RepositoryMaterializationResultEpoch::new(foreign, []).unwrap(),
    )
    .await;
    need(&query(&mut tx, &a).await, &a);
    tx = publish(tx, epoch([local(a.clone())])).await;
    assert!(
        matches!(query(&mut tx, &a).await, SourcePreparationOutcome::Complete(result) if result.is_ok())
    );
    tx = publish(tx, epoch([])).await;
    need(&query(&mut tx, &a).await, &a);
    for change_spec in [false, true] {
        let mut other = (*a).clone();
        if change_spec {
            other.repo_spec.rule_id.rule_name = "different_repository".into();
        } else {
            other.kind = RepositoryMaterializationKind::Local {
                logical_root: NormalizedAbsolutePath::new("/other-input").unwrap(),
            };
        }
        tx = publish(tx, epoch([local(Arc::new(other))])).await;
        need(&query(&mut tx, &a).await, &a);
    }
    tx = publish(tx, epoch([local(a.clone())])).await;
    assert!(
        matches!(query(&mut tx, &a).await, SourcePreparationOutcome::Complete(result) if result.is_ok())
    );
}

#[tokio::test]
async fn selected_immutable_result_fields_and_error_changes_invalidate() {
    let (tx, counter) = fixture().await;
    let mut a = (*request("archive+")).clone();
    a.kind = RepositoryMaterializationKind::Immutable;
    let a = Arc::new(a);
    let result =
        |identity: &str, root: &str, instance, path_policy| RepositoryMaterializationEpochEntry {
            request: a.clone(),
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    source_identity: identity.into(),
                    generation_root: root.into(),
                    observation_instance: PathObservationInstanceId::new(instance),
                    path_policy,
                },
            ),
        };
    let mut tx = publish(
        tx,
        epoch([result(
            "a",
            "/archive-a",
            11,
            RepositoryMaterializationPathPolicy::LegacyUnbounded,
        )]),
    )
    .await;
    let mut prior = query(&mut tx, &a).await;
    for entry in [
        result(
            "b",
            "/archive-a",
            11,
            RepositoryMaterializationPathPolicy::LegacyUnbounded,
        ),
        result(
            "b",
            "/archive-b",
            11,
            RepositoryMaterializationPathPolicy::LegacyUnbounded,
        ),
        result(
            "b",
            "/archive-b",
            12,
            RepositoryMaterializationPathPolicy::LegacyUnbounded,
        ),
        result(
            "b",
            "/archive-b",
            12,
            RepositoryMaterializationPathPolicy::GenerationRoot,
        ),
        RepositoryMaterializationEpochEntry {
            request: a.clone(),
            result: RepositoryMaterializationResult::SpecError("selected failure".into()),
        },
    ] {
        let before = counter.count.load(Ordering::Relaxed);
        tx = publish(tx, epoch([entry])).await;
        let next = query(&mut tx, &a).await;
        assert_eq!(counter.count.load(Ordering::Relaxed), before + 1);
        assert!(!RepositoryMaterializationResultKey::equality(&prior, &next));
        assert!(RepositoryMaterializationResultKey::validity(&next));
        prior = next;
    }
}

#[tokio::test]
async fn selected_transport_and_materialization_errors_keep_generation_retry_dependency() {
    for transport in [true, false] {
        let a = request("a+");
        let generation = RepositoryMaterializationGeneration(7);
        let result = if transport {
            RepositoryMaterializationResult::TransportError {
                generation,
                message: "transport failed".into(),
            }
        } else {
            RepositoryMaterializationResult::MaterializationError {
                generation,
                message: "materialization failed".into(),
            }
        };
        let selected = epoch([RepositoryMaterializationEpochEntry {
            request: a.clone(),
            result,
        }]);
        let (missing, _) = fixture().await;
        let mut missing = publish(missing, selected.clone()).await;
        assert!(
            matches!(query(&mut missing, &a).await, SourcePreparationOutcome::Complete(result)
            if matches!(result.as_ref(), Err(RepositoryMaterializationError::MissingGeneration(_))))
        );
        drop(missing);

        // A failed lookup of an uninjected key does not establish the same
        // dependency as a successful injected read. Exercise retry on a fresh
        // fixture with generation 7 installed before its first result query.
        let (tx, _) = fixture().await;
        let mut tx = publish(tx, selected).await;
        for current in [7, 8, 7] {
            let mut update = tx.into_updater();
            update
                .changed_to([(
                    RepositoryMaterializationGenerationKey {
                        workspace: workspace().as_path().to_owned(),
                    },
                    RepositoryMaterializationGeneration(current),
                )])
                .unwrap();
            tx = update.commit().await;
            let value = query(&mut tx, &a).await;
            if current != 7 {
                need(&value, &a);
            } else {
                let SourcePreparationOutcome::Complete(result) = value else {
                    panic!("matching generation must return selected error")
                };
                assert!(matches!(
                    (transport, result.as_ref()),
                    (true, Err(RepositoryMaterializationError::Transport(_)))
                        | (
                            false,
                            Err(RepositoryMaterializationError::Materialization(_))
                        )
                ));
            }
        }
    }
}
