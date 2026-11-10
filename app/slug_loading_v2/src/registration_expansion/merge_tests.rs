//! Real observed certificates exercise the private registration merge owner.
use slug_workspace_v2::PathObservationEpochError;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;

use super::*;

fn scratch() -> ExpansionScratch {
    ExpansionScratch::new(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        ModuleRegistrationFamily::Toolchains,
        ObservationMode::Observed,
    )
}
fn reference(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, PathObservationEpochError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .chain(right.observations())
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
}
fn full_facts(actual: &PathObservationEpoch, expected: &PathObservationEpoch) {
    assert_eq!(actual, expected);
    assert_eq!(actual.observations().len(), expected.observations().len());
    for (demand, result) in expected.observations() {
        assert!(Arc::ptr_eq(actual.get(demand).unwrap(), result));
    }
}
fn merge(scratch: &mut ExpansionScratch, incoming: &PathObservationEpoch) {
    assert!(scratch.merge(incoming).is_ok());
}

#[tokio::test]
async fn observed_certificate_noop_merges_retain_registration_map_owner() {
    let certificate =
        crate::registration_expansion_tests::observed_certificate_for_merge_test().await;
    let mut scratch = scratch();
    merge(&mut scratch, &certificate);
    full_facts(&scratch.observations, &certificate);
    let before = scratch.observations.dupe();
    let foreign = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .map(|(demand, result)| (demand.dupe(), Arc::new(result.as_ref().clone()))),
    )
    .unwrap();
    let subset = PathObservationEpoch::from_shared(
        foreign
            .observations()
            .iter()
            .take(1)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let empty = PathObservationEpoch::empty();
    for incoming in [&certificate, &empty, &before, &foreign, &subset] {
        let expected = reference(&before, incoming).unwrap();
        merge(&mut scratch, incoming);
        // Establish every semantic fact and left result representative before
        // the work discriminator. Baseline from_shared fails only map reuse.
        full_facts(&scratch.observations, &expected);
        assert!(std::ptr::eq(
            scratch.observations.observations(),
            before.observations()
        ));
    }
    let mut empty_left = self::scratch();
    merge(&mut empty_left, &certificate);
    full_facts(&empty_left.observations, &certificate);
    assert!(std::ptr::eq(
        empty_left.observations.observations(),
        certificate.observations()
    ));
}

#[tokio::test]
async fn observed_certificate_superset_growth_reuses_complete_right_map() {
    let certificate =
        crate::registration_expansion_tests::observed_certificate_for_merge_test().await;
    assert!(certificate.observations().len() > 1);
    let subset = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .take(1)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let expected = reference(&subset, &certificate).unwrap();
    let mut scratch = scratch();
    merge(&mut scratch, &subset);
    merge(&mut scratch, &certificate);
    // Complete facts and original result owners are established first. The
    // baseline's only failure is reconstructing an already complete carrier.
    full_facts(&scratch.observations, &expected);
    full_facts(&scratch.observations, &certificate);
    assert!(std::ptr::eq(
        scratch.observations.observations(),
        certificate.observations()
    ));
}

#[tokio::test]
async fn observed_certificate_growth_and_sorted_conflict_match_constructor() {
    let certificate =
        crate::registration_expansion_tests::observed_certificate_for_merge_test().await;
    let middle = certificate.observations().len() / 2;
    let left = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .take(middle)
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let right = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .skip(middle)
            .chain(left.observations().iter().take(1))
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
    .unwrap();
    let mut scratch = scratch();
    merge(&mut scratch, &left);
    let before = scratch.observations.dupe();
    let expected = reference(&before, &right).unwrap();
    merge(&mut scratch, &right);
    full_facts(&scratch.observations, &expected);
    full_facts(&scratch.observations, &certificate);
    assert!(!std::ptr::eq(
        scratch.observations.observations(),
        before.observations()
    ));

    let demands = certificate
        .observations()
        .iter()
        .filter(|(demand, result)| {
            demand.operation() == PathObservationOperation::Lstat
                && matches!(
                    result.as_ref(),
                    PathObservationResult::Lstat(PathOperationResult::Present(_))
                )
        })
        .take(2)
        .map(|(demand, _)| demand.dupe())
        .collect::<Vec<_>>();
    assert_eq!(demands.len(), 2);
    // Valid individual facts deliberately disagree with two retained facts.
    // Reverse input order to discriminate sorted conflict selection.
    let conflicts = PathObservationEpoch::from_shared(demands.iter().rev().map(|demand| {
        (
            demand.dupe(),
            Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing)),
        )
    }))
    .unwrap();
    let before = scratch.observations.dupe();
    let expected = reference(&before, &conflicts).unwrap_err();
    assert_eq!(
        expected,
        PathObservationEpochError::ConflictingDemand(demands[0].dupe())
    );
    let Err(DriverBreak::Outer(ModuleRegistrationExpansionObservationError::Frontier(actual))) =
        scratch.merge(&conflicts)
    else {
        panic!("conflict must retain the registration frontier error")
    };
    assert_eq!(actual, ObservedPathFrontierError::Epoch(expected));
    full_facts(&scratch.observations, &before);
    assert!(std::ptr::eq(
        scratch.observations.observations(),
        before.observations()
    ));
}

#[tokio::test]
async fn observed_certificate_unique_disjoint_growth_retains_left_controller() {
    let certificate =
        crate::registration_expansion_tests::observed_certificate_for_merge_test().await;
    let middle = certificate.observations().len() / 2;
    assert!(middle > 0);
    let left = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .take(middle)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    let right = PathObservationEpoch::from_shared(
        certificate
            .observations()
            .iter()
            .skip(middle)
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap();
    assert!(!right.observations().is_empty());
    assert!(
        left.observations()
            .keys()
            .all(|key| right.get(key).is_none())
    );
    let expected = reference(&left, &right).unwrap();
    let mut scratch = scratch();
    merge(&mut scratch, &left);
    full_facts(&scratch.observations, &left);
    // The certificate/reference retain rows, not this subset's map controller.
    // Do not clone the scratch epoch or create a Weak controller observer.
    drop(left);
    let old_controller = std::ptr::from_ref(scratch.observations.observations());
    merge(&mut scratch, &right);
    full_facts(&scratch.observations, &expected);
    full_facts(&scratch.observations, &certificate);
    // This genuine fixture contains ordinary path keys. Establish their exact
    // original shared path buffers and result owners before testing map work.
    for ((actual_key, actual_result), (original_key, original_result)) in scratch
        .observations
        .observations()
        .iter()
        .zip(certificate.observations())
    {
        assert_eq!(actual_key, original_key);
        assert!(actual_key.image_path().is_none());
        assert!(actual_key.windows_long_path_input().is_none());
        assert!(std::ptr::eq(
            actual_key.path().as_path(),
            original_key.path().as_path(),
        ));
        assert!(Arc::ptr_eq(actual_result, original_result));
    }
    // The initial delegating API must fail only this unique-growth discriminator.
    assert!(std::ptr::eq(
        old_controller,
        scratch.observations.observations(),
    ));
}
