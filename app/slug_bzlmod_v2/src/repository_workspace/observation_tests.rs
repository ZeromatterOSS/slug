use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_identity_v2::CanonicalLabel;
use starlark_map::small_map::SmallMap;

use super::*;

fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/workspace").unwrap()
}
fn limits() -> RepositoryWorkspaceRetentionLimits {
    RepositoryWorkspaceRetentionLimits::new(128, 16 * 1024 * 1024, 1024 * 1024 * 1024).unwrap()
}
fn epoch(lease: u64) -> RepositoryWorkspaceEpoch {
    RepositoryWorkspaceEpoch::fresh(workspace(), lease, limits()).unwrap()
}
fn reservation(id: u64, lease: u64) -> Arc<RepositoryWorkspaceReservation> {
    Arc::new(
        RepositoryWorkspaceReservation::new(
            workspace(),
            Arc::new(HostSelectedExtensionOwner::testing("owner")),
            CanonicalRepoName::new(format!("owner+repo{id}")).unwrap(),
            id as usize,
            Arc::new(RepoSpec {
                rule_id: crate::RepoRuleId {
                    bzl_file: CanonicalLabel::parse("@@//:repo.bzl").unwrap(),
                    rule_name: "repo".into(),
                },
                attributes: Arc::new(SmallMap::new()),
            }),
            NormalizedAbsolutePath::new(format!("/logical/repo{id}")).unwrap(),
            PathObservationInstanceId::new(id),
            lease,
            limits(),
        )
        .unwrap(),
    )
}
fn demand(
    reservation: &RepositoryWorkspaceReservation,
) -> Arc<RepositoryWorkspaceReservationDemand> {
    Arc::new(
        RepositoryWorkspaceReservationDemand::new(
            Arc::new(
                crate::source_preparation::RepositoryWorkspaceStableKey::new(
                    reservation.workspace().clone(),
                    reservation.owner().clone(),
                    reservation.repository().clone(),
                    reservation.ordinal(),
                )
                .unwrap(),
            ),
            reservation.expected_repo_spec().clone(),
        )
        .unwrap(),
    )
}
fn append(
    epoch: &RepositoryWorkspaceEpoch,
    reservation: &RepositoryWorkspaceReservation,
) -> RepositoryWorkspaceEpoch {
    let transcript = demand(reservation).find_in(epoch).unwrap().unwrap();
    epoch
        .complete(
            epoch
                .request_at(
                    reservation,
                    transcript.records().len(),
                    RepositoryWorkspaceOperation::Exists("".into()),
                )
                .unwrap(),
            RepositoryWorkspaceResult::Exists(true),
            RepositoryWorkspaceMode::Archive,
            vec![
                RepositoryWorkspaceEntry::new("".into(), 0o755, RepositoryWorkspaceNode::Directory)
                    .unwrap(),
            ],
        )
        .unwrap()
}

#[derive(Clone, Debug, Allocative)]
struct Consumer {
    projection: RepositoryWorkspaceTranscriptProjection,
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}
impl PartialEq for Consumer {
    fn eq(&self, other: &Self) -> bool {
        self.projection == other.projection && Arc::ptr_eq(&self.counter, &other.counter)
    }
}
impl Eq for Consumer {}
impl Hash for Consumer {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.projection.hash(state);
        Arc::as_ptr(&self.counter).hash(state);
    }
}
impl fmt::Display for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "consumer:{}", self.projection)
    }
}
#[async_trait]
impl Key for Consumer {
    type Value = Result<Arc<RepositoryWorkspaceTranscriptObservation>, RepositoryWorkspaceError>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        self.counter.fetch_add(1, Ordering::Relaxed);
        let opaque = ctx
            .compute_opaque(&RepositoryWorkspaceEpochKey::new(workspace()))
            .await
            .unwrap();
        ctx.projection(&opaque, &self.projection).unwrap()
    }
    fn equality(a: &Self::Value, b: &Self::Value) -> bool {
        a == b
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[tokio::test]
async fn opaque_transcript_cutoff_preserves_presence_own_append_and_freshness() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let a = reservation(1, 7);
    let b = reservation(2, 7);
    let counter = Arc::new(AtomicUsize::new(0));
    let key = Consumer {
        projection: RepositoryWorkspaceTranscriptProjection::new(demand(&a)),
        counter: counter.clone(),
    };
    let base_key = RepositoryWorkspaceEpochKey::new(workspace());
    let uninitialized = dice.updater().commit().await;
    assert!(
        uninitialized.compute_opaque(&base_key).await.is_err(),
        "missing injection must not become active absence"
    );
    drop(uninitialized);
    let mut current = epoch(7);
    let mut update = dice.updater();
    update
        .changed_to([(base_key.clone(), current.clone())])
        .unwrap();
    let mut tx = update.commit().await;
    let absent = tx.compute(&key).await.unwrap().unwrap();
    assert!(absent.transcript().is_none());
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    drop(tx);

    current = current.reserve(a.clone(), 0o755).unwrap();
    current = append(&current, &a);
    let mut update = dice.updater();
    update
        .changed_to([(base_key.clone(), current.clone())])
        .unwrap();
    let mut tx = update.commit().await;
    let present = tx.compute(&key).await.unwrap().unwrap();
    assert_eq!(present.transcript().unwrap().records().len(), 1);
    assert_eq!(counter.load(Ordering::Relaxed), 2);
    drop(tx);

    current = current.reserve(b.clone(), 0o755).unwrap();
    current = append(&current, &b);
    let mut update = dice.updater();
    update
        .changed_to([(base_key.clone(), current.clone())])
        .unwrap();
    let mut tx = update.commit().await;
    assert_eq!(tx.compute(&key).await.unwrap().unwrap(), present);
    assert_eq!(
        counter.load(Ordering::Relaxed),
        2,
        "unrelated B must not activate the consumer"
    );
    drop(tx);

    current = append(&current, &a);
    let mut update = dice.updater();
    update.changed_to([(base_key.clone(), current)]).unwrap();
    let mut tx = update.commit().await;
    assert_eq!(
        tx.compute(&key)
            .await
            .unwrap()
            .unwrap()
            .transcript()
            .unwrap()
            .records()
            .len(),
        2
    );
    assert_eq!(counter.load(Ordering::Relaxed), 3);
    drop(tx);
    let mut update = dice.updater();
    update.changed_to([(base_key.clone(), epoch(8))]).unwrap();
    let mut tx = update.commit().await;
    let fresh = tx.compute(&key).await.unwrap().unwrap();
    assert_eq!(fresh.context().lease(), 8);
    assert!(fresh.transcript().is_none());
    assert_ne!(fresh, absent);
    assert_eq!(counter.load(Ordering::Relaxed), 4);
    drop(tx);
    let mut update = dice.updater();
    update
        .changed_to([(base_key, RepositoryWorkspaceEpoch::revoked())])
        .unwrap();
    let mut tx = update.commit().await;
    assert_eq!(
        tx.compute(&key).await.unwrap(),
        Err(RepositoryWorkspaceError::Revoked)
    );
}

#[test]
fn observation_factories_reject_foreign_spec_and_keep_full_context_identity() {
    let a = reservation(1, 7);
    let current = epoch(7).reserve(a.clone(), 0o755).unwrap();
    let context = RepositoryWorkspaceContext::from_epoch(&current, &workspace()).unwrap();
    assert_eq!(context.limits(), limits());
    assert_eq!(
        RepositoryWorkspaceContext::from_epoch(
            &current,
            &NormalizedAbsolutePath::new("/foreign").unwrap()
        ),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    let mut spec = a.expected_repo_spec().as_ref().clone();
    spec.rule_id.rule_name = "SECRET_SPEC_NOT_DEBUG".into();
    let bad = Arc::new(
        RepositoryWorkspaceReservationDemand::new(demand(&a).key().clone(), Arc::new(spec))
            .unwrap(),
    );
    assert_eq!(
        RepositoryWorkspaceTranscriptObservation::from_epoch(&current, &bad),
        Err(RepositoryWorkspaceError::ForeignReservation)
    );
    let changed = RepositoryWorkspaceTranscriptProjection::new(bad);
    assert!(!format!("{changed:?}").contains("SECRET_SPEC_NOT_DEBUG"));
    assert_ne!(
        RepositoryWorkspaceTranscriptProjection::new(demand(&a)),
        changed
    );
    let different_limits = RepositoryWorkspaceRetentionLimits::new(
        127,
        limits().metadata_bytes(),
        limits().archive_body_bytes(),
    )
    .unwrap();
    let changed_context = RepositoryWorkspaceContext::from_epoch(
        &RepositoryWorkspaceEpoch::fresh(workspace(), 7, different_limits).unwrap(),
        &workspace(),
    )
    .unwrap();
    assert_ne!(context, changed_context);
    assert!(RepositoryWorkspaceTranscriptProjection::validity(
        &RepositoryWorkspaceTranscriptObservation::from_epoch(&epoch(7), &demand(&a))
    ));
    assert!(!RepositoryWorkspaceTranscriptProjection::validity(&Err(
        RepositoryWorkspaceError::Revoked
    )));
}
