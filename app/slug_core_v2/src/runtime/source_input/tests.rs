use dice::DetectCycles;
use dice::Dice;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use slug_workspace_v2::ResolvedPathObservationKey;

use super::*;

fn path(text: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(text).unwrap()
}
fn label() -> CanonicalLabel {
    CanonicalLabel::parse("@@//:input").unwrap()
}
fn key() -> SourceArtifactInputObservationKey {
    SourceArtifactInputObservationKey::new(path("/workspace"), &AnalysisArtifact::Source(label()))
        .unwrap()
}
fn epoch(
    namespace: PathObservationNamespace,
    revision: i64,
    kind: Option<PathNodeKind>,
    digest: Option<PathOperationResult<FileContentDigest>>,
) -> PathObservationEpoch {
    let demand = |name, op| PathObservationDemand::new(namespace, path(name), op);
    let mut rows = ["/", "/workspace"]
        .map(|name| {
            (
                demand(name, PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::Directory,
                    0,
                    revision,
                    1,
                    1,
                    0o755,
                ))),
            )
        })
        .to_vec();
    rows.push((
        demand("/workspace/input", PathObservationOperation::Lstat),
        PathObservationResult::Lstat(match kind {
            Some(kind) => {
                PathOperationResult::Present(PathLstat::new(kind, 3, revision, 1, 1, 0o644))
            }
            None => PathOperationResult::Missing,
        }),
    ));
    if let Some(digest) = digest {
        rows.push((
            demand("/workspace/input", PathObservationOperation::FileDigest),
            PathObservationResult::FileDigest(digest),
        ));
    }
    PathObservationEpoch::new(rows).unwrap()
}
fn content() -> FileContentDigest {
    FileContentDigest::new([7; 32], 3).unwrap()
}
async fn transaction(dice: &Arc<Dice>, epoch: PathObservationEpoch) -> dice::DiceTransaction {
    let mut updater = dice.updater();
    updater
        .changed_to(slug_workspace_v2::path_observation_shards(&epoch))
        .unwrap();
    updater
        .changed_to([(PathObservationEpochKey, epoch)])
        .unwrap();
    updater.commit().await
}
fn observed(value: &SourceArtifactInputOutcome) -> &Arc<ObservedSourceArtifactInput> {
    let LoadingPreparationOutcome::Complete(Ok(value)) = value else {
        panic!("{value:?}")
    };
    value
}

#[tokio::test]
async fn source_fact_tracks_metadata_separately_and_preserves_selected_arcs() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let mut values = Vec::new();
    for revision in [1, 2, 1] {
        let epoch = epoch(
            PathObservationNamespace::Host,
            revision,
            Some(PathNodeKind::RegularFile),
            Some(PathOperationResult::Present(content())),
        );
        let mut tx = transaction(&dice, epoch.dupe()).await;
        let value = tx.compute(&key()).await.unwrap();
        assert!(SourceArtifactInputObservationKey::validity(&value));
        let fact = observed(&value);
        for (demand, result) in fact.observations().observations() {
            let PathOutcome::Complete(selected) = tx
                .compute(&slug_workspace_v2::PathObservationKey::new(demand.dupe()))
                .await
                .unwrap()
            else {
                panic!()
            };
            assert!(Arc::ptr_eq(result, &selected));
        }
        assert_eq!(fact.observations().observations().len(), 4);
        assert_eq!(fact.result().as_ref().unwrap().digest(), content());
        values.push(value);
    }
    assert_eq!(observed(&values[0]).result(), observed(&values[1]).result());
    assert!(!SourceArtifactInputObservationKey::equality(
        &values[0], &values[1]
    ));
    assert!(SourceArtifactInputObservationKey::equality(
        &values[0], &values[2]
    ));
}

#[tokio::test]
async fn source_fact_needs_and_failures_are_never_cacheable() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    for (kind, digest) in [
        (Some(PathNodeKind::RegularFile), None),
        (None, None),
        (Some(PathNodeKind::Directory), None),
        (
            Some(PathNodeKind::RegularFile),
            Some(PathOperationResult::Missing),
        ),
        (
            Some(PathNodeKind::RegularFile),
            Some(PathOperationResult::Present(
                FileContentDigest::new([0; 32], 4).unwrap(),
            )),
        ),
    ] {
        let needs_digest = kind == Some(PathNodeKind::RegularFile) && digest.is_none();
        let mut tx = transaction(
            &dice,
            epoch(PathObservationNamespace::Host, 1, kind, digest),
        )
        .await;
        let value = tx.compute(&key()).await.unwrap();
        assert!(!SourceArtifactInputObservationKey::validity(&value));
        assert!(!SourceArtifactInputObservationKey::equality(&value, &value));
        if needs_digest {
            let LoadingPreparationOutcome::Need(need) = value else {
                panic!("{value:?}")
            };
            assert_eq!(
                need.path_observations().unwrap().demands()[0].operation(),
                PathObservationOperation::FileDigest
            );
        } else {
            assert!(observed(&value).result().is_err());
        }
    }
}

// Exercise the production digest join with a path delivered by the immutable
// repository path owner. Namespace routing itself has Bzlmod owner regressions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct ImmutableProbe;
impl fmt::Display for ImmutableProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("immutable-source-input-probe")
    }
}
#[async_trait]
impl Key for ImmutableProbe {
    type Value = SourceArtifactInputOutcome;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let namespace =
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(73));
        let PathOutcome::Complete(Ok(resolved)) = ctx
            .compute(&ResolvedPathObservationKey::new(
                namespace,
                path("/workspace/input"),
            ))
            .await
            .unwrap()
        else {
            panic!()
        };
        finish_source_input(
            ctx,
            &label(),
            resolved.result().as_ref().unwrap(),
            resolved.observations(),
        )
        .await
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        SourceArtifactInputObservationKey::equality(x, y)
    }
}

#[tokio::test]
async fn source_digest_join_keeps_immutable_namespace() {
    let dice = Arc::new(Dice::builder().build(DetectCycles::Enabled));
    let namespace = PathObservationNamespace::Materialization(PathObservationInstanceId::new(73));
    // No Host observations: choosing Host would leave an unfulfilled Need.
    let input = epoch(
        namespace,
        1,
        Some(PathNodeKind::RegularFile),
        Some(PathOperationResult::Present(content())),
    );
    let mut tx = transaction(&dice, input.dupe()).await;
    let value = tx.compute(&ImmutableProbe).await.unwrap();
    let value = observed(&value);
    assert_eq!(value.result().as_ref().unwrap().namespace(), namespace);
    assert_eq!(value.observations(), &input);
}

#[test]
fn source_input_constructor_rejects_derived_files_and_trees() {
    for kind in [
        slug_build_api_v2::ActionOutputKind::File,
        slug_build_api_v2::ActionOutputKind::Directory,
    ] {
        let artifact = AnalysisArtifact::Derived {
            owner: slug_build_api_v2::AnalysisConfiguredTargetKey::new(label(), b"cfg".as_slice()),
            output: slug_build_api_v2::ActionOutput::new("input", kind),
        };
        assert_eq!(
            SourceArtifactInputObservationKey::new(path("/workspace"), &artifact),
            Err(SourceArtifactInputError::NotSource)
        );
    }
}
