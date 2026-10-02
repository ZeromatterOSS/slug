use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use compact_str::CompactString;
use dice::DetectCycles;
use dice::Dice;
use dice::Key;
use dice::UserComputationData;
use dupe::Dupe;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_workspace_v2::ImageDescriptorBudgetError;
use slug_workspace_v2::InputImagePolicy;
use slug_workspace_v2::PathLstat;
use slug_workspace_v2::PathNodeKind;
use slug_workspace_v2::PathObservationDemand;
use slug_workspace_v2::PathObservationEpoch;
use slug_workspace_v2::PathObservationEpochKey;
use slug_workspace_v2::PathObservationInstanceId;
use slug_workspace_v2::PathObservationOperation;
use slug_workspace_v2::PathObservationResult;
use slug_workspace_v2::PathOperationResult;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::GeneratedRepositoryFileEffectPlan;
use crate::HostCanonicalRepositoryRoute;
use crate::HostSelectedExtensionOwner;
use crate::OverrideAttributeValue;
use crate::RepoRuleId;
use crate::RepoSpec;
use crate::RepositoryMaterializationEpochEntry;
use crate::RepositoryMaterializationResult;
use crate::RepositoryMaterializationResultEpoch;
use crate::RepositoryMaterializationResultEpochKey;
use crate::RepositoryMaterializationSuccess;
use crate::RootPackagePolicyInputs;
use crate::RootRepositoryRoute;
use crate::host_canonical_repository_source_input;
use crate::host_package::ExternalRepositoryPackageLookup;
use crate::host_package::ExternalRepositoryPackageLookupObservationKey;
use crate::host_package::HostBuildFileName;
use crate::host_package::ObservedExternalRepositoryPackageLookup;
use crate::inject_root_package_policy_inputs;

fn path(text: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(text).unwrap()
}
fn user_data(budget: &ImageDescriptorBudget) -> UserComputationData {
    let mut data = UserComputationData::default();
    data.data.set(budget.dupe());
    data
}
fn root_recipe(
    label: &CanonicalLabel,
    policy: SelectedLabelSourcePolicy,
    budget: &ImageDescriptorBudget,
) -> SelectedLabelSourceRecipe {
    SelectedLabelSourceRecipe::new(
        ImageChargedPath::pin_existing(path("/workspace"), budget).unwrap(),
        label,
        HostRepositoryLabelPathSource::Root(path("/workspace")),
        policy,
        budget,
    )
    .unwrap()
}
fn root_key(budget: &ImageDescriptorBudget) -> SelectedLabelSourceKey {
    let recipe = root_recipe(
        &CanonicalLabel::parse("@@//pkg:marker").unwrap(),
        SelectedLabelSourcePolicy::plain(),
        budget,
    );
    SelectedLabelSourceKey::new(
        recipe
            .parent(budget)
            .unwrap()
            .child("bin/tool", budget)
            .unwrap(),
    )
}
fn row(
    text: &str,
    kind: Option<PathNodeKind>,
    freshness: i64,
) -> (PathObservationDemand, PathObservationResult) {
    (
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            path(text),
            PathObservationOperation::Lstat,
        ),
        PathObservationResult::Lstat(match kind {
            Some(kind) => PathOperationResult::Present(PathLstat::new(
                kind, 1, freshness, freshness, freshness, 0o755,
            )),
            None => PathOperationResult::Missing,
        }),
    )
}
fn root_epoch(freshness: i64, package_exists: bool) -> PathObservationEpoch {
    PathObservationEpoch::new([
        row("/", Some(PathNodeKind::Directory), 1),
        row("/workspace", Some(PathNodeKind::Directory), 1),
        row("/workspace/REPO.bazel", None, 1),
        row("/workspace/.bazelignore", None, 1),
        row("/workspace/pkg", Some(PathNodeKind::Directory), 1),
        row(
            "/workspace/pkg/BUILD.bazel",
            package_exists.then_some(PathNodeKind::RegularFile),
            freshness,
        ),
        row("/workspace/pkg/BUILD", None, 1),
    ])
    .unwrap()
}
fn root_inputs(updater: &mut dice::DiceTransactionUpdater) {
    inject_root_package_policy_inputs(
        updater,
        RootPackagePolicyInputs::new(
            path("/workspace"),
            vec![path("/workspace")],
            std::iter::empty::<&str>(),
            None,
            Some("warning"),
        )
        .unwrap(),
    )
    .unwrap();
}
fn complete(value: Value) -> ObservedSelectedLabelSource {
    match value {
        SourcePreparationOutcome::Complete(Ok(value)) => value,
        other => panic!("expected normal selection: {other:?}"),
    }
}

#[tokio::test]
async fn root_need_replay_keeps_origin_transforms_and_label_frontier() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = root_key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    let mut tx = updater.commit().await;
    let need = tx.compute(&key).await.unwrap();
    assert!(
        matches!(&need, SourcePreparationOutcome::Need(needs) if needs.path_observations().is_some()),
        "{need:?}"
    );
    assert!(!SelectedLabelSourceKey::validity(&need));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    let mut tx = updater.commit().await;
    let selected = complete(tx.compute(&key).await.unwrap());
    assert_eq!(selected.recipe(), key.recipe());
    assert_eq!(
        selected.image_key().anchor_root().as_path(),
        PathBuf::from("/workspace")
    );
    assert_eq!(
        selected.image_key().source_root().as_path(),
        PathBuf::from("/workspace/pkg")
    );
    assert_eq!(
        selected.selected_file().as_path(),
        PathBuf::from("/workspace/pkg/bin/tool")
    );
    assert_eq!(
        selected.image_key().namespace(),
        PathObservationNamespace::Host
    );
    assert!(selected.repository().is_none());
    assert!(selected.same_budget(&budget));
    assert!(!selected.observations().observations().is_empty());
    assert!(
        selected
            .observations()
            .observations()
            .keys()
            .all(|demand| !demand.path().as_path().ends_with("marker")
                && !demand.path().as_path().ends_with("bin/tool")),
        "selector must not observe target bytes/kind"
    );
    let reordered = key
        .recipe()
        .parent(&budget)
        .unwrap()
        .child("tool", &budget)
        .unwrap();
    assert_ne!(key, SelectedLabelSourceKey::new(reordered));
    let split = root_recipe(
        &CanonicalLabel::parse("@@//pkg:marker").unwrap(),
        SelectedLabelSourcePolicy::plain(),
        &budget,
    )
    .parent(&budget)
    .unwrap()
    .child("bin", &budget)
    .unwrap()
    .child("tool", &budget)
    .unwrap();
    let split_key = SelectedLabelSourceKey::new(split);
    assert_ne!(
        key, split_key,
        "equal display must not erase ordered operations"
    );
    assert_eq!(
        selected.selected_file(),
        complete(tx.compute(&split_key).await.unwrap()).selected_file()
    );
}

fn repo_spec() -> RepoSpec {
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@bazel_tools//tools/build_defs/repo:local.bzl")
                .unwrap(),
            rule_name: "local_repository".into(),
        },
        attributes: Arc::new(SmallMap::from_iter([(
            CompactString::new("path"),
            OverrideAttributeValue::String("dep".into()),
        )])),
    }
}
fn immutable_route() -> crate::HostRepositorySourceRoute {
    let repo = CanonicalRepoName::new("generated+").unwrap();
    let route = HostCanonicalRepositoryRoute::generated(
        path("/workspace"),
        repo.clone(),
        Arc::new(HostSelectedExtensionOwner::testing("+extension")),
        0,
        "generated",
        repo_spec(),
        repo,
        SmallMap::new(),
    )
    .unwrap();
    crate::HostRepositorySourceRoute::canonical(
        host_canonical_repository_source_input(
            Arc::new(route),
            Some(GeneratedRepositoryFileEffectPlan::build([]).unwrap()),
        )
        .unwrap(),
    )
}
fn external_key(
    route: crate::HostRepositorySourceRoute,
    budget: &ImageDescriptorBudget,
) -> SelectedLabelSourceKey {
    let label = CanonicalLabel::parse(&format!("{}//pkg:marker", route.canonical_repo())).unwrap();
    let recipe = SelectedLabelSourceRecipe::new(
        ImageChargedPath::pin_existing(path("/workspace"), budget).unwrap(),
        &label,
        HostRepositoryLabelPathSource::External(route),
        SelectedLabelSourcePolicy::plain(),
        budget,
    )
    .unwrap();
    SelectedLabelSourceKey::new(
        recipe
            .parent(budget)
            .unwrap()
            .child("bin/tool", budget)
            .unwrap(),
    )
}
fn external_lookup(
    updater: &mut dice::DiceTransactionUpdater,
    route: &crate::HostRepositorySourceRoute,
) {
    let lookup = ExternalRepositoryPackageLookupObservationKey::from_source_route(
        route.clone(),
        slug_identity_v2::PackageIdentifier::new(
            route.canonical_repo().clone(),
            slug_identity_v2::PackagePath::parse("pkg").unwrap(),
        ),
    )
    .unwrap();
    updater
        .changed_to(vec![(
            lookup,
            SourcePreparationOutcome::Complete(Ok(
                ObservedExternalRepositoryPackageLookup::for_test(
                    Ok(ExternalRepositoryPackageLookup::Package(
                        HostBuildFileName::BuildDotBazel,
                    )),
                    PathObservationEpoch::new([row(
                        "/materialized/pkg/BUILD.bazel",
                        Some(PathNodeKind::RegularFile),
                        1,
                    )])
                    .unwrap(),
                ),
            )),
        )])
        .unwrap();
}
fn request(
    route: &crate::HostRepositorySourceRoute,
) -> Arc<crate::RepositoryMaterializationRequest> {
    match route.materialization_disposition().unwrap() {
        HostRepositoryMaterializationDisposition::Request(request) => request,
        _ => panic!("expected request"),
    }
}
fn materialization_epoch(
    request: Arc<crate::RepositoryMaterializationRequest>,
    root: &str,
    instance: u64,
) -> RepositoryMaterializationResultEpoch {
    RepositoryMaterializationResultEpoch::new(
        path("/workspace"),
        [RepositoryMaterializationEpochEntry {
            request,
            result: RepositoryMaterializationResult::Success(
                RepositoryMaterializationSuccess::Immutable {
                    source_identity: Arc::from(root),
                    generation_root: PathBuf::from(root),
                    observation_instance: PathObservationInstanceId::new(instance),
                },
            ),
        }],
    )
    .unwrap()
}

#[tokio::test]
async fn external_normal_materialization_need_and_current_namespace_a_b_a() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let route = immutable_route();
    let key = external_key(route.clone(), &budget);
    let request = request(&route);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    external_lookup(&mut updater, &route);
    let mut tx = updater.commit().await;
    let need = tx.compute(&key).await.unwrap();
    assert!(
        matches!(&need, SourcePreparationOutcome::Need(needs) if needs.repository_materializations().values().any(|actual| actual == &request)),
        "{need:?}"
    );
    assert!(!SelectedLabelSourceKey::validity(&need));
    let mut values = Vec::new();
    for (root, instance) in [("/a", 41), ("/b", 42), ("/a", 41)] {
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                RepositoryMaterializationResultEpochKey {
                    workspace: path("/workspace"),
                },
                materialization_epoch(request.dupe(), root, instance),
            )])
            .unwrap();
        tx = updater.commit().await;
        let value = complete(tx.compute(&key).await.unwrap());
        assert_eq!(
            value.image_key().namespace(),
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance))
        );
        assert_eq!(
            value.image_key().source_root().as_path(),
            PathBuf::from(root).join("pkg")
        );
        assert_eq!(
            value.selected_file().as_path(),
            PathBuf::from(root).join("pkg/bin/tool")
        );
        assert_eq!(value.repository().unwrap().0, &request);
        assert!(
            matches!(value.repository().unwrap().1, RepositoryMaterialization::Immutable { generation_root, .. } if generation_root == &PathBuf::from(root))
        );
        values.push(value);
    }
    assert_eq!(values[0], values[2]);
    assert_ne!(values[0], values[1]);
}

#[tokio::test]
async fn label_freshness_and_missing_package_have_owned_dependencies() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = root_key(&budget);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    let mut tx = updater.commit().await;
    let a = complete(tx.compute(&key).await.unwrap());
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(2, true))])
        .unwrap();
    tx = updater.commit().await;
    let b = complete(tx.compute(&key).await.unwrap());
    assert_eq!(a.selected_file(), b.selected_file());
    assert_ne!(
        a, b,
        "ordinary Label observation freshness must not cut off"
    );
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, false))])
        .unwrap();
    tx = updater.commit().await;
    let absent = tx.compute(&key).await.unwrap();
    assert!(
        matches!(&absent, SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::Label(error))) if matches!(error.get(), HostRepositoryLabelPathError::PackageAbsent)),
        "{absent:?}"
    );
    assert!(SelectedLabelSourceKey::validity(&absent));
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    tx = updater.commit().await;
    assert_eq!(a, complete(tx.compute(&key).await.unwrap()));
}

#[test]
fn recipe_identity_limits_and_origin_clones_are_bounded() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let label = CanonicalLabel::parse("@@//pkg:marker").unwrap();
    let recipe = root_recipe(&label, SelectedLabelSourcePolicy::plain(), &budget);
    let mapped = |id: &str| {
        ApparentLabel::parse("//pkg:marker")
            .unwrap()
            .resolve(&RepositoryMapping::new(
                RepositoryMappingId::new(id).unwrap(),
            ))
    };
    let a = mapped("mapping-a");
    let b = mapped("mapping-b");
    assert_eq!(a.to_string(), b.to_string());
    assert_ne!(
        SelectedLabelSourceKey::new(root_recipe(&a, recipe.policy(), &budget)),
        SelectedLabelSourceKey::new(root_recipe(&b, recipe.policy(), &budget)),
        "equal spelling must retain the complete mapping-id identity",
    );
    assert!(
        SelectedLabelSourceRecipe::new(
            recipe.workspace().dupe(),
            &label,
            HostRepositoryLabelPathSource::Root(path("/other")),
            recipe.policy(),
            &budget
        )
        .is_err()
    );
    for literal in ["", "../x", "/x", "a//b", "a/./b", "a\\b", "c:/x", "a\0b"] {
        assert!(
            matches!(
                recipe.child(literal, &budget),
                Err(SelectedLabelSourceError::InvalidChild)
            ),
            "{literal:?}"
        );
    }
    let mut sixty_four = recipe.dupe();
    for _ in 0..64 {
        sixty_four = sixty_four.parent(&budget).unwrap();
    }
    let before = budget.used_bytes();
    assert!(matches!(
        sixty_four.parent(&budget),
        Err(SelectedLabelSourceError::LimitExceeded(
            SelectedLabelSourceLimit::Transforms
        ))
    ));
    assert_eq!(
        budget.used_bytes(),
        before,
        "keyed failure precedes allocation"
    );
    assert!(recipe.child(&"x".repeat(4096), &budget).is_ok());
    assert!(matches!(
        recipe.child(&"x".repeat(4097), &budget),
        Err(SelectedLabelSourceError::LimitExceeded(
            SelectedLabelSourceLimit::PathBytes
        ))
    ));
    let mut limits = SelectedLabelSourceLimits::default();
    limits.max_descriptor_bytes = recipe.descriptor_bytes().unwrap() + 16;
    let constrained = root_recipe(
        &label,
        SelectedLabelSourcePolicy::new(limits, InputImagePolicy::plain()).unwrap(),
        &budget,
    );
    let before = budget.used_bytes();
    assert!(constrained.parent(&budget).is_ok());
    assert!(matches!(
        constrained.child("x", &budget),
        Err(SelectedLabelSourceError::LimitExceeded(
            SelectedLabelSourceLimit::DescriptorBytes
        ))
    ));
    assert_eq!(budget.used_bytes(), before);
    let mut alternate = limits;
    alternate.max_descriptor_bytes += 1;
    assert_ne!(
        constrained,
        root_recipe(
            &label,
            SelectedLabelSourcePolicy::new(alternate, InputImagePolicy::plain()).unwrap(),
            &budget
        )
    );
}

#[tokio::test]
async fn scope_escape_is_refused_at_consumption_and_local_is_not_workspace_laundered() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let escaping = SelectedLabelSourceKey::new(
        root_key(&budget)
            .recipe()
            .parent(&budget)
            .unwrap()
            .parent(&budget)
            .unwrap()
            .parent(&budget)
            .unwrap(),
    );
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    let mut tx = updater.commit().await;
    let value = tx.compute(&escaping).await.unwrap();
    assert!(
        matches!(
            &value,
            SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::ScopeEscape))
        ),
        "{value:?}"
    );
    assert!(SelectedLabelSourceKey::validity(&value));
    let route = crate::HostRepositorySourceRoute::root(RootRepositoryRoute::for_test(
        path("/workspace"),
        ApparentRepoName::new("dep").unwrap(),
        "dep".into(),
        CanonicalRepoName::new("dep+").unwrap(),
        repo_spec(),
    ));
    let local = external_key(route.clone(), &budget);
    let mut updater = tx.into_updater();
    external_lookup(&mut updater, &route);
    updater
        .changed_to(vec![(
            RepositoryMaterializationResultKey {
                request: request(&route),
            },
            SourcePreparationOutcome::Complete(Arc::new(Ok(RepositoryMaterialization::Local {
                canonical_repo: route.canonical_repo().clone(),
                repo_spec: repo_spec(),
                source_root: PathBuf::from("/outside"),
            }))),
        )])
        .unwrap();
    tx = updater.commit().await;
    let value = tx.compute(&local).await.unwrap();
    assert!(
        matches!(
            &value,
            SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::UnsupportedLocal))
        ),
        "{value:?}"
    );
}

#[tokio::test]
async fn foreign_missing_and_exhausted_budget_are_invalid_and_repairable() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let key = root_key(&budget);
    for data in [
        UserComputationData::default(),
        user_data(&ImageDescriptorBudget::new(1 << 20)),
    ] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut tx = dice.updater_with_data(data).commit().await;
        let value = tx.compute(&key).await.unwrap();
        assert!(
            matches!(
                &value,
                SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::MissingBudget
                    | SelectedLabelSourceError::Budget(ImageDescriptorBudgetError::ForeignBudget)))
            ),
            "{value:?}"
        );
        assert!(!SelectedLabelSourceKey::validity(&value));
        assert!(!SelectedLabelSourceKey::equality(&value, &value));
    }
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    let mut tx = updater.commit().await;
    let pressure = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    let value = tx.compute(&key).await.unwrap();
    assert!(
        matches!(
            &value,
            SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::Budget(
                ImageDescriptorBudgetError::ResourceLimit
            )))
        ),
        "{value:?}"
    );
    assert!(!SelectedLabelSourceKey::validity(&value));
    drop(pressure);
    drop(tx);
    dice.wait_for_idle().await;
    let mut tx = dice.updater_with_data(user_data(&budget)).commit().await;
    let selected = complete(tx.compute(&key).await.unwrap());
    drop(tx);
    let foreign = ImageDescriptorBudget::new(1 << 20);
    let mut tx = dice.updater_with_data(user_data(&foreign)).commit().await;
    let warm = complete(tx.compute(&key).await.unwrap());
    assert_eq!(selected, warm);
    assert!(warm.same_budget(&budget));
    assert!(
        !warm.same_budget(&foreign),
        "warm consumption must reject a foreign service"
    );
}

#[tokio::test]
async fn long_origin_and_mapping_id_are_shared_and_history_keeps_actual_credit() {
    let budget = ImageDescriptorBudget::new(24_000);
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("m".repeat(18_000)).unwrap());
    let label = ApparentLabel::parse(&format!("//{}:{}", "p".repeat(256), "t".repeat(256)))
        .unwrap()
        .resolve(&mapping);
    let recipe = root_recipe(&label, SelectedLabelSourcePolicy::plain(), &budget);
    let initial = budget.used_bytes();
    assert!(
        initial >= 18_512,
        "all four owned string slots, including mapping-id, credited before Clone"
    );
    let derived = recipe
        .parent(&budget)
        .unwrap()
        .child("bin/tool", &budget)
        .unwrap();
    assert!(
        std::ptr::eq(recipe.label(), derived.label()),
        "appends must share actual immutable Origin"
    );
    assert!(
        budget.used_bytes() - initial < 2048,
        "append cost must not clone long origin"
    );
    assert!(
        budget.try_reserve(18_512).is_err(),
        "tight service could not admit another deep-cloned origin"
    );
    let key = SelectedLabelSourceKey::new(derived);
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    let mut tx = updater.commit().await;
    let need = tx.compute(&key).await.unwrap();
    assert!(
        matches!(&need, SourcePreparationOutcome::Need(_)),
        "{need:?}"
    );
    drop(need);
    drop(label);
    drop(mapping);
    drop(recipe);
    drop(key);
    drop(tx);
    dice.wait_for_idle().await;
    assert!(
        budget.used_bytes() >= 18_512,
        "historical key retains Origin after request retirement"
    );
    drop(dice);
    tokio::time::timeout(Duration::from_secs(2), async {
        while budget.used_bytes() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual engine/key lastDrop must release all new credit");
}

#[tokio::test]
async fn producer_path_and_descriptor_limits_precede_selector_growth() {
    let budget = ImageDescriptorBudget::new(1 << 20);
    let label = CanonicalLabel::parse("@@//pkg:marker").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(user_data(&budget));
    root_inputs(&mut updater);
    updater
        .changed_to(vec![(PathObservationEpochKey, root_epoch(1, true))])
        .unwrap();
    let mut tx = updater.commit().await;
    for max_path_bytes in [
        "/workspace/pkg/bin/tool".len(),
        "/workspace/pkg/bin/tool".len() - 1,
    ] {
        let limits = SelectedLabelSourceLimits {
            max_path_bytes,
            ..SelectedLabelSourceLimits::default()
        };
        let recipe = root_recipe(
            &label,
            SelectedLabelSourcePolicy::new(limits, InputImagePolicy::plain()).unwrap(),
            &budget,
        );
        let key = SelectedLabelSourceKey::new(
            recipe
                .parent(&budget)
                .unwrap()
                .child("bin/tool", &budget)
                .unwrap(),
        );
        let value = tx.compute(&key).await.unwrap();
        if max_path_bytes == "/workspace/pkg/bin/tool".len() {
            assert_eq!(
                complete(value).selected_file().as_path(),
                std::path::Path::new("/workspace/pkg/bin/tool")
            );
        } else {
            assert!(
                matches!(
                    &value,
                    SourcePreparationOutcome::Complete(Err(
                        SelectedLabelSourceError::LimitExceeded(
                            SelectedLabelSourceLimit::PathBytes
                        )
                    ))
                ),
                "{value:?}"
            );
            assert!(SelectedLabelSourceKey::validity(&value));
        }
    }
    let recipe = root_recipe(&label, SelectedLabelSourcePolicy::plain(), &budget);
    let semantic_bytes = recipe.descriptor_bytes().unwrap() + "/workspace/pkg/marker".len();
    let policy = SelectedLabelSourcePolicy::new(
        SelectedLabelSourceLimits {
            max_descriptor_bytes: semantic_bytes,
            ..SelectedLabelSourceLimits::default()
        },
        InputImagePolicy::plain(),
    )
    .unwrap();
    let key = SelectedLabelSourceKey::new(root_recipe(&label, policy, &budget));
    let value = tx.compute(&key).await.unwrap();
    assert!(
        matches!(
            &value,
            SourcePreparationOutcome::Complete(Err(SelectedLabelSourceError::LimitExceeded(
                SelectedLabelSourceLimit::DescriptorBytes
            )))
        ),
        "known parent occurrence exceeds semantic bound despite roomy physical service: {value:?}"
    );
    assert!(SelectedLabelSourceKey::validity(&value));
}

#[test]
fn origin_parent_resets_transforms_without_recloning_credited_origin() {
    let budget = ImageDescriptorBudget::new(64 << 10);
    let label = CanonicalLabel::parse(&format!("@@//pkg:{}", "x".repeat(3000))).unwrap();
    let original = root_recipe(&label, SelectedLabelSourcePolicy::plain(), &budget);
    let derived = original
        .parent(&budget)
        .unwrap()
        .child("bin/tool", &budget)
        .unwrap();
    let before = budget.used_bytes();
    let parent = derived.origin_parent(&budget).unwrap();
    let delta = budget.used_bytes() - before;
    assert!(
        delta < 512,
        "origin reset allocates only one transform/header, not the long Label"
    );
    assert_eq!(parent.label(), original.label());
    assert_eq!(parent.source(), original.source());
    assert_eq!(parent.transforms(), &[SelectedLabelSourceTransform::Parent]);
    let child = parent.child("lib", &budget).unwrap();
    assert_eq!(child.transforms().len(), 2);
    assert_eq!(child.transforms()[1].child(), Some("lib"));
    assert_eq!(
        derived.transforms().len(),
        2,
        "reset does not mutate retained tool recipe"
    );
    let foreign = ImageDescriptorBudget::new(64 << 10);
    assert!(matches!(
        derived.origin_parent(&foreign),
        Err(SelectedLabelSourceError::Budget(
            ImageDescriptorBudgetError::ForeignBudget
        ))
    ));
    let held = budget
        .try_reserve(budget.limit_bytes() - budget.used_bytes())
        .unwrap();
    assert!(matches!(
        derived.origin_parent(&budget),
        Err(SelectedLabelSourceError::Budget(
            ImageDescriptorBudgetError::ResourceLimit
        ))
    ));
    drop(held);
    drop(original);
    drop(derived);
    drop(parent);
    assert!(
        budget.used_bytes() > 3000,
        "reset child retains shared origin's original credit"
    );
    drop(child);
    assert_eq!(budget.used_bytes(), 0);
}
